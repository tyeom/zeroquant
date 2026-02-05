//! 자산 곡선 동기화 핸들러.
//!
//! KIS API에서 체결 내역을 가져와 자산 곡선 데이터를 재구성합니다.

use axum::{extract::State, response::IntoResponse, Json};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::repository::{
    create_kis_kr_client_from_credential, EquityHistoryRepository, ExecutionCacheRepository,
    ExecutionForSync, NewExecution,
};
use crate::state::AppState;
use trader_core::Side;

use super::types::{SyncEquityCurveRequest, SyncEquityCurveResponse};

/// 거래소 체결 내역으로 자산 곡선 동기화.
///
/// POST /api/v1/analytics/sync-equity
///
/// KIS API에서 체결 내역을 가져와 자산 곡선 데이터를 재구성합니다.
pub async fn sync_equity_curve(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SyncEquityCurveRequest>,
) -> impl IntoResponse {
    // 1. credential_id 파싱
    let credential_id = match Uuid::parse_str(&request.credential_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(SyncEquityCurveResponse {
                    success: false,
                    synced_count: 0,
                    execution_count: 0,
                    start_date: request.start_date,
                    end_date: request.end_date,
                    message: "Invalid credential_id format".to_string(),
                }),
            );
        }
    };

    // 2. DB 연결 및 암호화 관리자 확인
    let pool = match state.db_pool.as_ref() {
        Some(p) => p,
        None => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(SyncEquityCurveResponse {
                    success: false,
                    synced_count: 0,
                    execution_count: 0,
                    start_date: request.start_date.clone(),
                    end_date: request.end_date.clone(),
                    message: "DB pool이 없습니다".to_string(),
                }),
            );
        }
    };

    let encryptor = match state.encryptor.as_ref() {
        Some(e) => e,
        None => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(SyncEquityCurveResponse {
                    success: false,
                    synced_count: 0,
                    execution_count: 0,
                    start_date: request.start_date.clone(),
                    end_date: request.end_date.clone(),
                    message: "Encryptor가 없습니다".to_string(),
                }),
            );
        }
    };

    // 3. KIS 클라이언트 생성 (체결 내역 조회는 KIS-specific API)
    let kr_client = match create_kis_kr_client_from_credential(pool, encryptor, credential_id).await
    {
        Ok(client) => client,
        Err(e) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(SyncEquityCurveResponse {
                    success: false,
                    synced_count: 0,
                    execution_count: 0,
                    start_date: request.start_date.clone(),
                    end_date: request.end_date.clone(),
                    message: format!("KIS 클라이언트 생성 실패: {}", e),
                }),
            );
        }
    };

    // 4. Credential 정보 조회 (ISA 계좌 판단용)
    #[derive(sqlx::FromRow)]
    struct CredentialInfo {
        is_testnet: bool,
        settings: Option<serde_json::Value>,
        exchange_name: String,
    }

    let cred_info: CredentialInfo = match sqlx::query_as(
        "SELECT is_testnet, settings, exchange_name FROM exchange_credentials WHERE id = $1",
    )
    .bind(credential_id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(info)) => info,
        Ok(None) => {
            return (
                axum::http::StatusCode::NOT_FOUND,
                Json(SyncEquityCurveResponse {
                    success: false,
                    synced_count: 0,
                    execution_count: 0,
                    start_date: request.start_date.clone(),
                    end_date: request.end_date.clone(),
                    message: "Credential을 찾을 수 없습니다".to_string(),
                }),
            );
        }
        Err(e) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(SyncEquityCurveResponse {
                    success: false,
                    synced_count: 0,
                    execution_count: 0,
                    start_date: request.start_date.clone(),
                    end_date: request.end_date.clone(),
                    message: format!("Credential 조회 실패: {}", e),
                }),
            );
        }
    };

    // 5. 캐시 확인 및 조회 범위 결정
    let exchange_name = "kis";
    let is_isa_account = {
        // settings에 account_type 필드 확인
        if let Some(settings) = &cred_info.settings {
            if let Some(account_type) = settings.get("account_type").and_then(|v| v.as_str()) {
                account_type == "isa"
            } else {
                cred_info.exchange_name.to_uppercase().contains("ISA")
            }
        } else {
            cred_info.exchange_name.to_uppercase().contains("ISA")
        }
    };

    // 요청된 날짜 파싱
    let requested_start = NaiveDate::parse_from_str(&request.start_date, "%Y-%m-%d")
        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
    let requested_end = NaiveDate::parse_from_str(&request.end_date, "%Y-%m-%d")
        .unwrap_or_else(|_| chrono::Utc::now().date_naive());

    // DB에서 마지막 캐시 일자 확인
    let (actual_start, cached_executions) = if let Some(pool) = &state.db_pool {
        match ExecutionCacheRepository::get_latest_cached_date(pool, credential_id, exchange_name)
            .await
        {
            Ok(Some(latest_date)) => {
                // 캐시가 있으면 그 다음날부터 조회
                let new_start = latest_date + chrono::Duration::days(1);
                info!(
                    "Cache found: latest_date={}, querying from {}",
                    latest_date, new_start
                );

                // 기존 캐시 데이터 조회
                let cached = ExecutionCacheRepository::get_all_executions(
                    pool,
                    credential_id,
                    exchange_name,
                )
                .await
                .unwrap_or_default();
                (new_start, cached)
            }
            Ok(None) => {
                info!(
                    "No cache found, querying from requested start: {}",
                    requested_start
                );
                (requested_start, Vec::new())
            }
            Err(e) => {
                warn!("Failed to check cache: {}, querying full range", e);
                (requested_start, Vec::new())
            }
        }
    } else {
        (requested_start, Vec::new())
    };

    // 캐시된 데이터를 ExecutionForSync로 변환
    let mut all_executions: Vec<ExecutionForSync> = cached_executions
        .iter()
        .map(|c| ExecutionForSync {
            execution_time: c.executed_at,
            amount: c.amount,
            is_buy: c.side == Side::Buy,
            symbol: c.symbol.clone(),
        })
        .collect();

    info!("Starting with {} cached executions", all_executions.len());

    // KIS API Rate Limit (2024.04.01 변경):
    // - 실계좌: 200ms (초당 5건)
    // - 모의계좌: 510ms (초당 2건) = 200ms + 310ms
    let api_call_delay_ms: u64 = if cred_info.is_testnet {
        520 // 510ms + 안전 마진 10ms
    } else {
        200
    };

    // 이미 최신 데이터가 있으면 API 호출 스킵
    if actual_start > requested_end {
        info!("Cache is up to date, skipping API call");
    } else {
        // 날짜 형식 변환 (ISO 8601 -> YYYYMMDD)
        let start_date_yyyymmdd = actual_start.format("%Y%m%d").to_string();
        let end_date_yyyymmdd = requested_end.format("%Y%m%d").to_string();
        debug!(
            "Date range for API: {} ~ {}",
            start_date_yyyymmdd, end_date_yyyymmdd
        );

        // 날짜 범위 생성 (ISA: 1년 단위, 일반: 3개월 단위로 분할)
        let date_ranges: Vec<(String, String)> = {
            let mut ranges = Vec::new();
            let mut current_start = actual_start;

            // ISA 계좌: 1년 단위, 일반 계좌: 3개월 단위 (API 제한에 맞춤)
            let max_days = if is_isa_account { 365 } else { 90 };

            while current_start <= requested_end {
                let current_end = std::cmp::min(
                    current_start + chrono::Duration::days(max_days - 1),
                    requested_end,
                );
                ranges.push((
                    current_start.format("%Y%m%d").to_string(),
                    current_end.format("%Y%m%d").to_string(),
                ));
                current_start = current_end + chrono::Duration::days(1);
            }

            if ranges.is_empty() {
                ranges.push((start_date_yyyymmdd.clone(), end_date_yyyymmdd.clone()));
            }

            ranges
        };

        info!(
            "Date range split into {} chunks for {} account",
            date_ranges.len(),
            if is_isa_account { "ISA" } else { "general" }
        );

        // 4. 체결 내역 조회 (연속 조회로 전체 가져오기)
        // KIS API는 초당 요청 수를 제한하므로 Rate Limiting 필요
        let mut new_executions_for_cache: Vec<NewExecution> = Vec::new();
        const MAX_PAGES: usize = 50; // 무한 루프 방지 (날짜 범위당)
        debug!(
            "Using API delay: {}ms (is_testnet: {})",
            api_call_delay_ms, cred_info.is_testnet
        );

        // 각 날짜 범위에 대해 체결 내역 조회
        for (range_idx, (range_start, range_end)) in date_ranges.iter().enumerate() {
            debug!(
                "Fetching date range {}/{}: {} ~ {}",
                range_idx + 1,
                date_ranges.len(),
                range_start,
                range_end
            );

            let mut ctx_fk = String::new();
            let mut ctx_nk = String::new();
            let mut prev_ctx_nk = String::new();
            let mut page_count = 0;

            loop {
                // Rate Limiting: 첫 번째 호출 이후에는 지연 적용
                if page_count > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(api_call_delay_ms)).await;
                }
                page_count += 1;

                // 무한 루프 방지
                if page_count > MAX_PAGES {
                    warn!(
                        "Max pagination limit reached ({} pages), stopping",
                        MAX_PAGES
                    );
                    break;
                }

                debug!(
                    "Fetching order history page {} (ctx_fk={}, ctx_nk={})",
                    page_count,
                    ctx_fk.len(),
                    ctx_nk.len()
                );

                let history = match kr_client
                    .get_order_history(
                        range_start,
                        range_end,
                        "00", // 전체 (매수+매도)
                        &ctx_fk,
                        &ctx_nk,
                    )
                    .await
                {
                    Ok(h) => h,
                    Err(e) => {
                        // Rate Limit 에러인 경우 잠시 대기 후 재시도
                        let error_msg = e.to_string();
                        if error_msg.contains("초당")
                            || error_msg.contains("건수")
                            || error_msg.contains("exceeded")
                        {
                            warn!("Rate limit hit, waiting 2 seconds before retry...");
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                            // 재시도
                            match kr_client
                                .get_order_history(range_start, range_end, "00", &ctx_fk, &ctx_nk)
                                .await
                            {
                                Ok(h) => h,
                                Err(e2) => {
                                    return (
                                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                                        Json(SyncEquityCurveResponse {
                                            success: false,
                                            synced_count: 0,
                                            execution_count: all_executions.len(),
                                            start_date: request.start_date,
                                            end_date: request.end_date,
                                            message: format!(
                                                "Failed to fetch order history after retry: {}",
                                                e2
                                            ),
                                        }),
                                    );
                                }
                            }
                        } else {
                            return (
                                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                                Json(SyncEquityCurveResponse {
                                    success: false,
                                    synced_count: 0,
                                    execution_count: 0,
                                    start_date: request.start_date,
                                    end_date: request.end_date,
                                    message: format!("Failed to fetch order history: {}", e),
                                }),
                            );
                        }
                    }
                };

                debug!(
                    "Received {} executions in page {}",
                    history.executions.len(),
                    page_count
                );

                // 체결 내역 변환
                for exec in history.executions {
                    // 체결 시간 파싱 (order_date: YYYYMMDD, order_time: HHMMSS)
                    let exec_date = format!("{}{}", exec.order_date, exec.order_time);
                    let execution_time =
                        chrono::NaiveDateTime::parse_from_str(&exec_date, "%Y%m%d%H%M%S")
                            .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
                            .unwrap_or_else(|_| Utc::now());

                    let amount = exec.filled_amount; // 총 체결 금액
                    let is_buy = exec.side_code == "02"; // 02: 매수
                    let side = if is_buy { Side::Buy } else { Side::Sell };

                    // 동기화용 데이터 추가
                    all_executions.push(ExecutionForSync {
                        execution_time,
                        amount,
                        is_buy,
                        symbol: exec.stock_code.clone(),
                    });

                    // 캐시용 데이터 추가
                    new_executions_for_cache.push(NewExecution {
                        credential_id,
                        exchange: exchange_name.to_string(),
                        executed_at: execution_time,
                        symbol: exec.stock_code.clone(),
                        normalized_symbol: Some(format!("{}.KS", exec.stock_code)),
                        side,
                        quantity: exec.filled_qty,
                        price: exec.avg_price, // 체결평균가
                        amount,
                        fee: None,
                        fee_currency: Some("KRW".to_string()),
                        order_id: exec.order_no.clone(),
                        trade_id: None,
                        order_type: None,
                        raw_data: None,
                    });
                }

                // 연속 조회 확인
                // 1. 데이터가 더 없으면 종료
                if !history.has_more {
                    debug!(
                        "No more pages (has_more=false), total {} executions collected",
                        all_executions.len()
                    );
                    break;
                }

                // 2. 이전 키와 현재 키가 같으면 종료 (무한 루프 방지)
                if prev_ctx_nk == history.ctx_area_nk100 && !prev_ctx_nk.is_empty() {
                    debug!("Same ctx_nk as previous, stopping (infinite loop prevention)");
                    break;
                }

                // 3. NK 키가 비어있으면 종료
                if history.ctx_area_nk100.is_empty() {
                    debug!("ctx_nk is empty, no more pages");
                    break;
                }

                prev_ctx_nk = ctx_nk.clone();
                ctx_fk = history.ctx_area_fk100;
                ctx_nk = history.ctx_area_nk100;
            }
        } // end of date range for loop

        // 새로 조회한 체결 내역을 캐시에 저장
        if !new_executions_for_cache.is_empty() {
            if let Some(pool) = &state.db_pool {
                info!(
                    "Saving {} new executions to cache",
                    new_executions_for_cache.len()
                );

                match ExecutionCacheRepository::upsert_executions(pool, &new_executions_for_cache)
                    .await
                {
                    Ok(count) => {
                        info!("Successfully cached {} executions", count);

                        // 캐시 메타데이터 업데이트
                        let earliest = new_executions_for_cache
                            .iter()
                            .map(|e| e.executed_at.date_naive())
                            .min();
                        let latest = new_executions_for_cache
                            .iter()
                            .map(|e| e.executed_at.date_naive())
                            .max();

                        if let (Some(earliest_date), Some(latest_date)) = (earliest, latest) {
                            if let Err(e) = ExecutionCacheRepository::update_cache_meta(
                                pool,
                                credential_id,
                                exchange_name,
                                Some(earliest_date),
                                Some(latest_date),
                                "success",
                                None,
                            )
                            .await
                            {
                                warn!("Failed to update cache meta: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to cache executions: {}", e);
                        // 캐시 실패해도 동기화는 계속 진행
                    }
                }
            }
        }
    } // end of else block (API 호출 필요한 경우)

    let execution_count = all_executions.len();

    // 4. 현재 잔고 조회 (Rate Limit 방지를 위한 지연)
    tokio::time::sleep(std::time::Duration::from_millis(api_call_delay_ms)).await;

    let (current_equity, current_cash) = match kr_client.get_balance().await {
        Ok(balance) => {
            // summary에서 총 평가금액과 현금 잔고 추출
            let equity = balance
                .summary
                .as_ref()
                .map(|s| s.total_eval_amount)
                .unwrap_or_else(|| {
                    // summary가 없으면 holdings의 평가금액 합산
                    balance.holdings.iter().map(|h| h.eval_amount).sum()
                });
            let cash = balance
                .summary
                .as_ref()
                .map(|s| s.cash_balance)
                .unwrap_or(Decimal::ZERO);
            (equity, cash)
        }
        Err(e) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(SyncEquityCurveResponse {
                    success: false,
                    synced_count: 0,
                    execution_count,
                    start_date: request.start_date,
                    end_date: request.end_date,
                    message: format!("Failed to fetch balance: {}", e),
                }),
            );
        }
    };

    tracing::info!(
        "Current balance - equity: {}, cash: {}",
        current_equity,
        current_cash
    );

    // 5. DB에 자산 곡선 저장
    if let Some(pool) = &state.db_pool {
        // 종가 기반 계산 vs 현금 흐름 기반 계산
        if request.use_market_prices {
            // 현재 실제 현금 잔고를 기준으로 과거 자산 역산
            // (initial_capital 지정 시 해당 값을 현재 현금으로 사용 - 테스트용)
            let cash_for_sync = request.initial_capital.unwrap_or(current_cash);

            tracing::info!(
                "Using market prices for equity calculation (current_cash: {})",
                cash_for_sync
            );

            match EquityHistoryRepository::sync_with_market_prices(
                pool,
                credential_id,
                cash_for_sync, // 현재 실제 현금 잔고
                "KRW",
                "KR",
                Some("real"),
            )
            .await
            {
                Ok(synced_count) => {
                    return (
                        axum::http::StatusCode::OK,
                        Json(SyncEquityCurveResponse {
                            success: true,
                            synced_count,
                            execution_count,
                            start_date: request.start_date,
                            end_date: request.end_date,
                            message: format!(
                                "Successfully synced {} equity points with market prices from {} executions",
                                synced_count, execution_count
                            ),
                        }),
                    );
                }
                Err(e) => {
                    return (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        Json(SyncEquityCurveResponse {
                            success: false,
                            synced_count: 0,
                            execution_count,
                            start_date: request.start_date,
                            end_date: request.end_date,
                            message: format!(
                                "Failed to save equity curve with market prices: {}",
                                e
                            ),
                        }),
                    );
                }
            }
        } else {
            // 기존 현금 흐름 기반 계산
            match EquityHistoryRepository::sync_from_executions(
                pool,
                credential_id,
                all_executions,
                current_equity,
                "KRW",
                "KR",
                Some("real"),
            )
            .await
            {
                Ok(synced_count) => {
                    return (
                        axum::http::StatusCode::OK,
                        Json(SyncEquityCurveResponse {
                            success: true,
                            synced_count,
                            execution_count,
                            start_date: request.start_date,
                            end_date: request.end_date,
                            message: format!(
                                "Successfully synced {} equity points from {} executions",
                                synced_count, execution_count
                            ),
                        }),
                    );
                }
                Err(e) => {
                    return (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        Json(SyncEquityCurveResponse {
                            success: false,
                            synced_count: 0,
                            execution_count,
                            start_date: request.start_date,
                            end_date: request.end_date,
                            message: format!("Failed to save equity curve: {}", e),
                        }),
                    );
                }
            }
        }
    }

    (
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        Json(SyncEquityCurveResponse {
            success: false,
            synced_count: 0,
            execution_count,
            start_date: request.start_date,
            end_date: request.end_date,
            message: "Database not available".to_string(),
        }),
    )
}

/// 자산 곡선 캐시 삭제 요청.
#[derive(Debug, serde::Deserialize)]
pub struct ClearEquityCacheRequest {
    /// 자격증명 ID
    pub credential_id: String,
}

/// 자산 곡선 캐시 삭제 응답.
#[derive(Debug, serde::Serialize)]
pub struct ClearEquityCacheResponse {
    pub success: bool,
    pub deleted_count: u64,
    pub message: String,
}

/// 자산 곡선 캐시 삭제.
///
/// DELETE /api/v1/analytics/equity-cache
///
/// 특정 credential의 자산 곡선 데이터를 삭제합니다.
pub async fn clear_equity_cache(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ClearEquityCacheRequest>,
) -> impl IntoResponse {
    // 1. credential_id 파싱
    let credential_id = match Uuid::parse_str(&request.credential_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(ClearEquityCacheResponse {
                    success: false,
                    deleted_count: 0,
                    message: "Invalid credential_id format".to_string(),
                }),
            );
        }
    };

    // 2. DB 연결 확인
    let pool = match state.db_pool.as_ref() {
        Some(p) => p,
        None => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(ClearEquityCacheResponse {
                    success: false,
                    deleted_count: 0,
                    message: "DB pool이 없습니다".to_string(),
                }),
            );
        }
    };

    // 3. 캐시 삭제
    match EquityHistoryRepository::clear_cache(pool, credential_id).await {
        Ok(deleted_count) => {
            info!(
                "자산 곡선 캐시 삭제 완료: credential={}, 삭제={}건",
                credential_id, deleted_count
            );
            (
                axum::http::StatusCode::OK,
                Json(ClearEquityCacheResponse {
                    success: true,
                    deleted_count,
                    message: format!("{}건의 자산 곡선 데이터가 삭제되었습니다.", deleted_count),
                }),
            )
        }
        Err(e) => {
            warn!("자산 곡선 캐시 삭제 실패: {}", e);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(ClearEquityCacheResponse {
                    success: false,
                    deleted_count: 0,
                    message: format!("캐시 삭제 실패: {}", e),
                }),
            )
        }
    }
}
