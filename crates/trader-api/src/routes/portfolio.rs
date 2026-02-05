//! 포트폴리오 관리 endpoint.
//!
//! 포트폴리오 요약, 잔고 조회, 수익률 정보를 위한 REST API를 제공합니다.
//! KIS API를 통해 실제 계좌 데이터를 조회합니다.
//!
//! # 엔드포인트
//!
//! - `GET /api/v1/portfolio/summary` - 포트폴리오 요약
//! - `GET /api/v1/portfolio/balance` - 상세 잔고 조회
//! - `GET /api/v1/portfolio/holdings` - 보유 종목 목록
//!
//! # 쿼리 파라미터
//!
//! - `credential_id` (선택): 특정 거래소 자격증명 ID로 조회

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::repository::{
    EquityHistoryRepository, ExchangeProviderPair, HoldingPosition, PortfolioSnapshot,
    PositionRepository,
};
use crate::routes::strategies::ApiError;
use crate::state::AppState;
use chrono::Utc;
use trader_core::{ExecutionHistoryRequest, ExecutionRecord};

// ==================== 응답 타입 ====================

/// 포트폴리오 요약 응답.
///
/// Frontend의 PortfolioSummary 타입과 매칭됩니다.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioSummaryResponse {
    /// 총 자산 가치 (현금 + 평가액)
    pub total_value: Decimal,
    /// 총 손익
    pub total_pnl: Decimal,
    /// 총 수익률 (%)
    pub total_pnl_percent: Decimal,
    /// 당일 손익
    pub daily_pnl: Decimal,
    /// 당일 수익률 (%)
    pub daily_pnl_percent: Decimal,
    /// 현금 잔고
    pub cash_balance: Decimal,
    /// 사용 중인 마진/증거금
    pub margin_used: Decimal,
}

/// 상세 잔고 응답.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceResponse {
    /// 한국 주식 잔고
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kr: Option<KrBalanceInfo>,
    /// 미국 주식 잔고
    #[serde(skip_serializing_if = "Option::is_none")]
    pub us: Option<UsBalanceInfo>,
    /// 총 자산 가치
    pub total_value: Decimal,
}

/// 한국 주식 잔고 정보.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KrBalanceInfo {
    /// 예수금 (현금)
    pub cash_balance: Decimal,
    /// 총 평가금액
    pub total_eval_amount: Decimal,
    /// 총 평가손익
    pub total_profit_loss: Decimal,
    /// 보유 종목 수
    pub holdings_count: usize,
}

/// 미국 주식 잔고 정보.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsBalanceInfo {
    /// 총 평가금액 (USD)
    pub total_eval_amount: Option<Decimal>,
    /// 총 평가손익 (USD)
    pub total_profit_loss: Option<Decimal>,
    /// 보유 종목 수
    pub holdings_count: usize,
}

/// 보유 종목 응답.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HoldingsResponse {
    /// 한국 주식 보유 종목
    pub kr_holdings: Vec<HoldingInfo>,
    /// 미국 주식 보유 종목
    pub us_holdings: Vec<HoldingInfo>,
    /// 총 보유 종목 수
    pub total_count: usize,
}

/// 개별 보유 종목 정보.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HoldingInfo {
    /// 종목 코드/심볼
    pub symbol: String,
    /// 표시 이름 (예: "005930(삼성전자)")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// 종목명 (KIS API에서 받아온 원본)
    pub name: String,
    /// 보유 수량
    pub quantity: Decimal,
    /// 매입 평균가
    pub avg_price: Decimal,
    /// 현재가
    pub current_price: Decimal,
    /// 평가금액
    pub eval_amount: Decimal,
    /// 평가손익
    pub profit_loss: Decimal,
    /// 수익률 (%)
    pub profit_loss_rate: Decimal,
    /// 시장 (KR/US)
    pub market: String,
}

// ==================== 쿼리 파라미터 ====================

/// 포트폴리오 API 쿼리 파라미터.
#[derive(Debug, Deserialize)]
pub struct PortfolioQuery {
    /// 특정 자격증명 ID로 조회 (선택)
    pub credential_id: Option<Uuid>,
}

/// 체결 내역 조회 쿼리 파라미터.
#[derive(Debug, Deserialize)]
pub struct OrderHistoryQuery {
    /// 자격증명 ID (필수)
    pub credential_id: Uuid,
    /// 조회 시작일 (YYYYMMDD, 기본: 30일 전)
    pub start_date: Option<String>,
    /// 조회 종료일 (YYYYMMDD, 기본: 오늘)
    pub end_date: Option<String>,
    /// 매수/매도 구분 ("00"=전체, "01"=매도, "02"=매수, 기본: 전체)
    pub side: Option<String>,
    /// 페이지 커서 (연속 조회용)
    pub cursor: Option<String>,
}

// ==================== 헬퍼 함수 ====================

/// 특정 credential_id로 거래소 Provider 조회 (캐시 우선) 또는 생성 (거래소 중립).
///
/// # Single Source of Truth
///
/// 이 함수는 `create_exchange_providers_from_credential()`를 통해서만 Provider를 생성합니다.
/// 토큰 재사용을 위해 AppState의 캐시를 먼저 확인합니다.
///
/// # Returns
///
/// 거래소 Provider 쌍 (KR, US)
pub async fn get_or_create_exchange_providers(
    state: &AppState,
    credential_id: Uuid,
) -> Result<Arc<ExchangeProviderPair>, String> {
    // 1. Provider 캐시 확인
    {
        let cache = state.exchange_providers_cache.read().await;
        if let Some(pair) = cache.get(&credential_id) {
            debug!("거래소 Provider 캐시 히트: credential_id={}", credential_id);
            return Ok(Arc::clone(pair));
        }
    }

    // 2. 캐시 미스 - 쓰기 락으로 전환하여 생성
    info!(
        "거래소 Provider 캐시 미스, 새로 생성: credential_id={}",
        credential_id
    );

    // Double-Check Locking: 쓰기 락 획득 후 다시 확인
    let mut cache = state.exchange_providers_cache.write().await;

    // 다른 스레드가 이미 생성했을 수 있으므로 다시 확인
    if let Some(pair) = cache.get(&credential_id) {
        debug!(
            "거래소 Provider 캐시 히트 (재확인): credential_id={}",
            credential_id
        );
        return Ok(Arc::clone(pair));
    }

    // DB 연결 확인
    let pool = state
        .db_pool
        .as_ref()
        .ok_or("데이터베이스 연결이 설정되지 않았습니다.")?;

    // 암호화 관리자 확인
    let encryptor = state
        .encryptor
        .as_ref()
        .ok_or("암호화 설정이 없습니다. ENCRYPTION_MASTER_KEY를 설정하세요.")?;

    // 3. Repository 함수를 통해 Provider 생성 (Single Source of Truth)
    let provider_pair = crate::repository::create_exchange_providers_from_credential(
        pool,
        encryptor,
        credential_id,
        None, // OAuth 캐시는 repository에서 관리
    )
    .await?;

    // 4. Provider 캐시에 저장
    let pair_arc = Arc::new(provider_pair);
    cache.insert(credential_id, Arc::clone(&pair_arc));
    info!(
        "거래소 Provider 캐시 저장: credential_id={}, 캐시 크기={}",
        credential_id,
        cache.len()
    );

    Ok(pair_arc)
}

// ==================== Handler ====================

/// 포트폴리오 요약 조회.
///
/// GET /api/v1/portfolio/summary?credential_id=...
///
/// KIS API에서 실제 계좌 정보를 조회하여 반환합니다.
/// credential_id가 제공되면 해당 계정의 데이터를 조회합니다.
pub async fn get_portfolio_summary(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PortfolioQuery>,
) -> Result<Json<PortfolioSummaryResponse>, (StatusCode, Json<ApiError>)> {
    let mut total_value = Decimal::ZERO;
    let mut total_pnl = Decimal::ZERO;
    let mut cash_balance = Decimal::ZERO;

    // credential_id가 제공된 경우 동적으로 클라이언트 생성
    if let Some(credential_id) = params.credential_id {
        info!("포트폴리오 조회: credential_id={}", credential_id);

        match get_or_create_exchange_providers(&state, credential_id).await {
            Ok(providers) => {
                // 한국 주식 잔고 조회 (ExchangeProvider 사용)
                match providers.kr.fetch_account().await {
                    Ok(account_info) => {
                        debug!(
                            "KR account info fetched for credential {}: total={}, available={}",
                            credential_id,
                            account_info.total_balance,
                            account_info.available_balance
                        );

                        cash_balance = account_info.available_balance;
                        total_value = account_info.total_balance;
                        total_pnl = account_info.unrealized_pnl;
                    }
                    Err(e) => {
                        error!(
                            "KR account fetch failed for credential {}: {:?}",
                            credential_id, e
                        );
                        return Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(ApiError::new(
                                "BALANCE_FETCH_ERROR",
                                format!("계좌 조회 실패: {:?}", e),
                            )),
                        ));
                    }
                }
            }
            Err(e) => {
                error!("거래소 Provider 생성 실패: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError::new("CLIENT_ERROR", &e)),
                ));
            }
        }
    } else {
        // credential_id가 없으면 기존 방식 (환경변수 기반 클라이언트)
        if let Some(kr_client) = &state.kis_kr_client {
            match kr_client.get_balance().await {
                Ok(balance) => {
                    debug!("KR balance fetched: {:?}", balance);

                    if let Some(summary) = &balance.summary {
                        cash_balance += summary.cash_balance;
                        total_value += summary.total_eval_amount;
                        total_pnl += summary.total_profit_loss;
                    }
                }
                Err(e) => {
                    warn!("Failed to fetch KR balance: {}", e);
                }
            }
        }

        if let Some(us_client) = &state.kis_us_client {
            match us_client.get_balance("USD").await {
                Ok(balance) => {
                    debug!("US balance fetched: {:?}", balance);

                    if let Some(summary) = &balance.summary {
                        if let Some(eval) = summary.total_eval_amount {
                            total_value += eval;
                        }
                        if let Some(pnl) = summary.total_profit_loss {
                            total_pnl += pnl;
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to fetch US balance: {}", e);
                }
            }
        }

        // KIS 클라이언트가 없고 credential_id도 없으면 Mock 데이터
        if !state.has_kis_client() {
            return Ok(Json(mock_portfolio_summary()));
        }
    }

    // 수익률 계산
    let total_pnl_percent = if total_value > Decimal::ZERO && total_value != total_pnl {
        (total_pnl / (total_value - total_pnl)) * Decimal::from(100)
    } else {
        Decimal::ZERO
    };

    // 포트폴리오 스냅샷 저장 (자산 곡선 데이터 축적)
    if let (Some(db_pool), Some(credential_id)) = (&state.db_pool, params.credential_id) {
        let securities_value = total_value - cash_balance;

        // 검증: 총 자산이 0보다 크고, 합계가 맞는지 확인
        // (총 자산 0인 스냅샷은 MDD 계산을 왜곡하므로 저장하지 않음)
        let is_valid = total_value > Decimal::ZERO
            && cash_balance >= Decimal::ZERO
            && securities_value >= Decimal::ZERO
            && (cash_balance + securities_value - total_value).abs() < Decimal::ONE; // 허용 오차 1원

        if !is_valid {
            warn!(
                "포트폴리오 스냅샷 검증 실패: total={}, cash={}, securities={}, credential_id={}",
                total_value, cash_balance, securities_value, credential_id
            );
        } else {
            let snapshot = PortfolioSnapshot {
                credential_id,
                snapshot_time: Utc::now(),
                total_equity: total_value,
                cash_balance,
                securities_value,
                total_pnl,
                daily_pnl: Decimal::ZERO, // TODO: 전일 대비 계산
                currency: "KRW".to_string(),
                market: "KR".to_string(),
                account_type: None, // 계좌 타입은 credential에서 가져올 수 있음
            };

            // 비동기로 저장 (실패해도 API 응답에 영향 없음)
            let pool = db_pool.clone();
            tokio::spawn(async move {
                match EquityHistoryRepository::save_snapshot(&pool, &snapshot).await {
                    Ok(_) => debug!(
                        "포트폴리오 스냅샷 저장 성공: credential_id={}",
                        credential_id
                    ),
                    Err(e) => warn!("포트폴리오 스냅샷 저장 실패: {}", e),
                }
            });
        }
    }

    Ok(Json(PortfolioSummaryResponse {
        total_value,
        total_pnl,
        total_pnl_percent,
        daily_pnl: Decimal::ZERO, // TODO: 당일 손익 계산 필요
        daily_pnl_percent: Decimal::ZERO,
        cash_balance,
        margin_used: Decimal::ZERO, // 현금 계좌는 마진 없음
    }))
}

/// 상세 잔고 조회.
///
/// GET /api/v1/portfolio/balance?credential_id=...
pub async fn get_balance(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PortfolioQuery>,
) -> Result<Json<BalanceResponse>, (StatusCode, Json<ApiError>)> {
    let mut kr_info: Option<KrBalanceInfo> = None;
    let mut us_info: Option<UsBalanceInfo> = None;
    let mut total_value = Decimal::ZERO;

    // credential_id가 제공된 경우 동적으로 클라이언트 생성
    if let Some(credential_id) = params.credential_id {
        match get_or_create_exchange_providers(&state, credential_id).await {
            Ok(providers) => {
                // 계좌 정보 및 포지션 조회 (ExchangeProvider 사용)
                if let Ok(account_info) = providers.kr.fetch_account().await {
                    if let Ok(positions) = providers.kr.fetch_positions().await {
                        let holdings_count = positions.len();
                        total_value = account_info.total_balance;

                        kr_info = Some(KrBalanceInfo {
                            cash_balance: account_info.available_balance,
                            total_eval_amount: account_info.total_balance,
                            total_profit_loss: account_info.unrealized_pnl,
                            holdings_count,
                        });
                    }
                }
            }
            Err(e) => {
                error!("거래소 Provider 생성 실패: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError::new("CLIENT_ERROR", &e)),
                ));
            }
        }
    } else {
        // credential_id가 없으면 기존 방식
        if let Some(kr_client) = &state.kis_kr_client {
            if let Ok(balance) = kr_client.get_balance().await {
                let holdings_count = balance.holdings.len();

                if let Some(summary) = balance.summary {
                    total_value += summary.total_eval_amount;

                    kr_info = Some(KrBalanceInfo {
                        cash_balance: summary.cash_balance,
                        total_eval_amount: summary.total_eval_amount,
                        total_profit_loss: summary.total_profit_loss,
                        holdings_count,
                    });
                }
            }
        }

        if let Some(us_client) = &state.kis_us_client {
            if let Ok(balance) = us_client.get_balance("USD").await {
                let holdings_count = balance.holdings.len();

                if let Some(summary) = balance.summary {
                    if let Some(eval) = summary.total_eval_amount {
                        total_value += eval;
                    }

                    us_info = Some(UsBalanceInfo {
                        total_eval_amount: summary.total_eval_amount,
                        total_profit_loss: summary.total_profit_loss,
                        holdings_count,
                    });
                }
            }
        }
    }

    Ok(Json(BalanceResponse {
        kr: kr_info,
        us: us_info,
        total_value,
    }))
}

/// 보유 종목 목록 조회.
///
/// GET /api/v1/portfolio/holdings?credential_id=...
pub async fn get_holdings(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PortfolioQuery>,
) -> Result<Json<HoldingsResponse>, (StatusCode, Json<ApiError>)> {
    let mut kr_holdings = Vec::new();
    let mut us_holdings = Vec::new();

    // credential_id가 제공된 경우 동적으로 클라이언트 생성
    if let Some(credential_id) = params.credential_id {
        info!("보유종목 조회: credential_id={}", credential_id);

        match get_or_create_exchange_providers(&state, credential_id).await {
            Ok(providers) => {
                // 한국 주식 보유 종목 (ExchangeProvider 사용)
                match providers.kr.fetch_positions().await {
                    Ok(positions) => {
                        for position in positions {
                            let symbol_str = position.ticker.clone();
                            let eval_amount = position.quantity * position.current_price;
                            let profit_loss_rate = if position.avg_entry_price > Decimal::ZERO {
                                ((position.current_price - position.avg_entry_price)
                                    / position.avg_entry_price)
                                    * Decimal::from(100)
                            } else {
                                Decimal::ZERO
                            };

                            kr_holdings.push(HoldingInfo {
                                symbol: symbol_str.clone(),
                                display_name: Some(symbol_str.clone()),
                                name: symbol_str,
                                quantity: position.quantity,
                                avg_price: position.avg_entry_price,
                                current_price: position.current_price,
                                eval_amount,
                                profit_loss: position.unrealized_pnl,
                                profit_loss_rate,
                                market: "KR".to_string(),
                            });
                        }
                    }
                    Err(e) => {
                        warn!("보유종목 조회 실패 (credential {}): {:?}", credential_id, e);
                    }
                }
            }
            Err(e) => {
                error!("거래소 Provider 생성 실패: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError::new("CLIENT_ERROR", &e)),
                ));
            }
        }
    } else {
        // credential_id가 없으면 기존 방식
        if let Some(kr_client) = &state.kis_kr_client {
            if let Ok(balance) = kr_client.get_balance().await {
                for holding in balance.holdings {
                    let display_name = format!("{}({})", holding.stock_code, holding.stock_name);
                    kr_holdings.push(HoldingInfo {
                        symbol: holding.stock_code,
                        display_name: Some(display_name),
                        name: holding.stock_name,
                        quantity: holding.quantity,
                        avg_price: holding.avg_price,
                        current_price: holding.current_price,
                        eval_amount: holding.eval_amount,
                        profit_loss: holding.profit_loss,
                        profit_loss_rate: holding.profit_loss_rate,
                        market: "KR".to_string(),
                    });
                }
            }
        }

        if let Some(us_client) = &state.kis_us_client {
            if let Ok(balance) = us_client.get_balance("USD").await {
                for holding in balance.holdings {
                    let display_name = format!("{}({})", holding.symbol, holding.name);
                    us_holdings.push(HoldingInfo {
                        symbol: holding.symbol,
                        display_name: Some(display_name),
                        name: holding.name,
                        quantity: holding.quantity,
                        avg_price: holding.avg_price,
                        current_price: holding.current_price,
                        eval_amount: holding.eval_amount,
                        profit_loss: holding.profit_loss,
                        profit_loss_rate: holding.profit_loss_rate,
                        market: "US".to_string(),
                    });
                }
            }
        }
    }

    let total_count = kr_holdings.len() + us_holdings.len();

    // 거래소 데이터를 positions 테이블에 동기화
    if let (Some(db_pool), Some(credential_id)) = (&state.db_pool, params.credential_id) {
        // 동기화할 holdings 데이터 준비
        let mut sync_holdings = Vec::new();

        for h in &kr_holdings {
            sync_holdings.push(HoldingPosition {
                credential_id,
                exchange: "kis".to_string(),
                symbol: h.symbol.clone(),
                symbol_name: h.name.clone(),
                quantity: h.quantity,
                avg_price: h.avg_price,
                current_price: h.current_price,
                profit_loss: h.profit_loss,
                profit_loss_rate: h.profit_loss_rate,
                market: h.market.clone(),
            });
        }

        for h in &us_holdings {
            sync_holdings.push(HoldingPosition {
                credential_id,
                exchange: "kis".to_string(),
                symbol: h.symbol.clone(),
                symbol_name: h.name.clone(),
                quantity: h.quantity,
                avg_price: h.avg_price,
                current_price: h.current_price,
                profit_loss: h.profit_loss,
                profit_loss_rate: h.profit_loss_rate,
                market: h.market.clone(),
            });
        }

        // 비동기로 동기화 (API 응답 지연 방지)
        let pool = db_pool.clone();
        tokio::spawn(async move {
            match PositionRepository::sync_holdings(&pool, credential_id, "kis", sync_holdings)
                .await
            {
                Ok(result) => {
                    debug!(
                        "포지션 동기화 완료: credential_id={}, synced={}, closed={}",
                        credential_id, result.synced, result.closed
                    );
                }
                Err(e) => {
                    warn!(
                        "포지션 동기화 실패: credential_id={}, error={}",
                        credential_id, e
                    );
                }
            }
        });
    }

    Ok(Json(HoldingsResponse {
        kr_holdings,
        us_holdings,
        total_count,
    }))
}

/// 체결 내역 조회 응답.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderHistoryResponse {
    /// 체결 내역 목록
    pub records: Vec<ExecutionRecordDto>,
    /// 추가 데이터 존재 여부
    pub has_more: bool,
    /// 다음 페이지 커서
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// 총 레코드 수 (현재 페이지)
    pub count: usize,
}

/// 체결 내역 DTO (프론트엔드용 직렬화).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionRecordDto {
    /// 거래소
    pub exchange: String,
    /// 주문 ID
    pub order_id: String,
    /// 심볼
    pub symbol: String,
    /// 종목명
    pub asset_name: String,
    /// 매수/매도
    pub side: String,
    /// 주문 유형
    pub order_type: String,
    /// 주문 수량
    pub order_qty: Decimal,
    /// 주문 가격
    pub order_price: Decimal,
    /// 체결 수량
    pub filled_qty: Decimal,
    /// 체결 평균가
    pub filled_price: Decimal,
    /// 체결 금액
    pub filled_amount: Decimal,
    /// 상태
    pub status: String,
    /// 취소 여부
    pub is_cancelled: bool,
    /// 주문 일시 (ISO 8601)
    pub ordered_at: String,
}

impl From<&ExecutionRecord> for ExecutionRecordDto {
    fn from(record: &ExecutionRecord) -> Self {
        Self {
            exchange: record.exchange.clone(),
            order_id: record.order_id.clone(),
            symbol: record.symbol.to_string(),
            asset_name: record.asset_name.clone(),
            side: format!("{:?}", record.side),
            order_type: record.order_type.clone(),
            order_qty: record.order_qty,
            order_price: record.order_price,
            filled_qty: record.filled_qty,
            filled_price: record.filled_price,
            filled_amount: record.filled_amount,
            status: format!("{:?}", record.status),
            is_cancelled: record.is_cancelled,
            ordered_at: record.ordered_at.to_rfc3339(),
        }
    }
}

/// 체결 내역 조회.
///
/// GET /api/v1/portfolio/orders?credential_id=...&start_date=...&end_date=...
///
/// 거래소 중립적인 ExecutionHistory를 반환합니다.
pub async fn get_order_history(
    State(state): State<Arc<AppState>>,
    Query(params): Query<OrderHistoryQuery>,
) -> Result<Json<OrderHistoryResponse>, (StatusCode, Json<ApiError>)> {
    info!("체결 내역 조회: credential_id={}", params.credential_id);

    // 기본 날짜 설정 (30일 전 ~ 오늘)
    let today = chrono::Utc::now() + chrono::Duration::hours(9); // KST
    let default_start = (today - chrono::Duration::days(30))
        .format("%Y%m%d")
        .to_string();
    let default_end = today.format("%Y%m%d").to_string();

    let start_date = params.start_date.unwrap_or(default_start);
    let end_date = params.end_date.unwrap_or(default_end);
    let side = params.side.unwrap_or_else(|| "00".to_string());

    // 커서 파싱 (format: "ctx_fk100|ctx_nk100")
    let (ctx_fk100, ctx_nk100) = if let Some(cursor) = &params.cursor {
        let parts: Vec<&str> = cursor.split('|').collect();
        if parts.len() == 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            (String::new(), String::new())
        }
    } else {
        (String::new(), String::new())
    };

    // ExchangeProvider 획득
    let providers = get_or_create_exchange_providers(&state, params.credential_id)
        .await
        .map_err(|e| {
            error!("거래소 Provider 생성 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("CLIENT_ERROR", &e)),
            )
        })?;

    // 체결 내역 조회 (ExchangeProvider 사용)
    let mut request = ExecutionHistoryRequest::new(&start_date, &end_date).with_side(&side);
    if !ctx_fk100.is_empty() && !ctx_nk100.is_empty() {
        request = request.with_cursor(format!("{}|{}", ctx_fk100, ctx_nk100));
    }

    let history_response = providers
        .kr
        .fetch_execution_history(&request)
        .await
        .map_err(|e| {
            error!("체결 내역 조회 실패: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "HISTORY_FETCH_ERROR",
                    format!("체결 내역 조회 실패: {:?}", e),
                )),
            )
        })?;

    // Trade를 ExecutionRecordDto로 변환
    let records: Vec<ExecutionRecordDto> = history_response
        .trades
        .iter()
        .map(|trade| ExecutionRecordDto {
            exchange: trade.exchange.clone(),
            order_id: trade.exchange_trade_id.clone(),
            symbol: trade.ticker.clone(),
            asset_name: trade
                .metadata
                .get("stock_name")
                .and_then(|v| v.as_str())
                .unwrap_or(&trade.ticker)
                .to_string(),
            side: match trade.side {
                trader_core::Side::Buy => "BUY".to_string(),
                trader_core::Side::Sell => "SELL".to_string(),
            },
            order_type: "LIMIT".to_string(), // KIS API는 주문 유형을 제공하지 않음
            order_qty: trade.quantity,
            order_price: trade.price,
            filled_qty: trade.quantity, // Trade는 이미 체결된 것
            filled_price: trade.price,
            filled_amount: trade.quantity * trade.price,
            status: "FILLED".to_string(),
            is_cancelled: false,
            ordered_at: trade.executed_at.to_rfc3339(),
        })
        .collect();

    let count = records.len();
    let has_more = history_response.next_cursor.is_some();

    info!("체결 내역 조회 완료: {} 건, has_more={}", count, has_more);

    Ok(Json(OrderHistoryResponse {
        records,
        has_more,
        next_cursor: history_response.next_cursor,
        count,
    }))
}

// ==================== Mock 데이터 ====================

/// Mock 포트폴리오 요약 (KIS 클라이언트 미설정 시)
fn mock_portfolio_summary() -> PortfolioSummaryResponse {
    use rust_decimal_macros::dec;

    PortfolioSummaryResponse {
        total_value: dec!(10000000), // 1천만원
        total_pnl: dec!(250000),     // 25만원 수익
        total_pnl_percent: dec!(2.56),
        daily_pnl: dec!(15000), // 일 1.5만원
        daily_pnl_percent: dec!(0.15),
        cash_balance: dec!(3000000), // 현금 300만원
        margin_used: dec!(0),
    }
}

// ==================== 라우터 ====================

/// 포트폴리오 관리 라우터 생성.
pub fn portfolio_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/summary", get(get_portfolio_summary))
        .route("/balance", get(get_balance))
        .route("/holdings", get(get_holdings))
        .route("/orders", get(get_order_history))
}

// ==================== 테스트 ====================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_get_portfolio_summary_mock() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/portfolio/summary", get(get_portfolio_summary))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/portfolio/summary")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let summary: PortfolioSummaryResponse = serde_json::from_slice(&body).unwrap();

        // Mock 데이터 확인
        assert!(summary.total_value > Decimal::ZERO);
    }

    #[tokio::test]
    async fn test_get_holdings_empty() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/portfolio/holdings", get(get_holdings))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/portfolio/holdings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let holdings: HoldingsResponse = serde_json::from_slice(&body).unwrap();

        // KIS 클라이언트 미설정 시 빈 목록
        assert_eq!(holdings.total_count, 0);
    }
}
