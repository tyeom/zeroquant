//! 시장 상태 endpoint.
//!
//! 한국 및 미국 시장의 운영 상태를 조회합니다.
//!
//! # 엔드포인트
//!
//! - `GET /api/v1/market/:market/status` - 시장 상태 조회
//! - `GET /api/v1/market/klines` - 캔들스틱 데이터 조회 (실시간 거래소 데이터)
//! - `GET /api/v1/market/ticker` - 현재가 조회

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::{Datelike, NaiveTime, Timelike, Utc, Weekday};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, info};

use trader_core::Timeframe;
use trader_data::cache::CachedHistoricalDataProvider;
use trader_exchange::connector::kis::{KisConfig, KisOAuth, KisKrClient, KisUsClient, KisAccountType};
use trader_exchange::historical::HistoricalDataProvider;
use trader_exchange::YahooFinanceProvider;

use crate::routes::credentials::EncryptedCredentials;
use crate::routes::strategies::ApiError;
use crate::state::AppState;

// ==================== 응답 타입 ====================

/// 시장 상태 응답.
///
/// Frontend의 MarketStatus 타입과 매칭됩니다.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketStatusResponse {
    /// 시장 코드 (KR/US)
    pub market: String,
    /// 시장 개장 여부
    pub is_open: bool,
    /// 다음 개장 시간 (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_open: Option<String>,
    /// 다음 폐장 시간 (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_close: Option<String>,
    /// 현재 세션 (Regular/PreMarket/AfterHours)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

/// 시장 세션 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketSession {
    Regular,
    PreMarket,
    AfterHours,
    Closed,
}

impl MarketSession {
    pub fn as_str(&self) -> Option<&'static str> {
        match self {
            MarketSession::Regular => Some("Regular"),
            MarketSession::PreMarket => Some("PreMarket"),
            MarketSession::AfterHours => Some("AfterHours"),
            MarketSession::Closed => None,
        }
    }
}

// ==================== Handler ====================

/// 시장 상태 조회.
///
/// GET /api/v1/market/:market/status
///
/// 한국 시장 (KR):
/// - 정규장: 09:00-15:30 KST (월-금)
///
/// 미국 시장 (US):
/// - 프리마켓: 04:00-09:30 EST
/// - 정규장: 09:30-16:00 EST (월-금)
/// - 애프터아워: 16:00-20:00 EST
pub async fn get_market_status(
    State(_state): State<Arc<AppState>>,
    Path(market): Path<String>,
) -> Result<Json<MarketStatusResponse>, (StatusCode, Json<ApiError>)> {
    let market_upper = market.to_uppercase();

    match market_upper.as_str() {
        "KR" => Ok(Json(get_kr_market_status())),
        "US" => Ok(Json(get_us_market_status())),
        _ => Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                code: "INVALID_MARKET".to_string(),
                message: format!("Invalid market: {}. Supported: KR, US", market),
            }),
        )),
    }
}

/// 한국 시장 상태 계산.
///
/// 정규장: 09:00-15:30 KST (UTC+9)
fn get_kr_market_status() -> MarketStatusResponse {
    let now = Utc::now();
    // KST = UTC + 9시간
    let kst_hour = (now.hour() + 9) % 24;
    let kst_minute = now.minute();

    let is_weekday = !matches!(now.weekday(), Weekday::Sat | Weekday::Sun);

    // 정규장 시간: 09:00-15:30 KST
    let market_open = NaiveTime::from_hms_opt(9, 0, 0).unwrap();
    let market_close = NaiveTime::from_hms_opt(15, 30, 0).unwrap();
    let current_time = NaiveTime::from_hms_opt(kst_hour, kst_minute, 0).unwrap();

    let is_open = is_weekday && current_time >= market_open && current_time < market_close;

    let session = if is_open {
        Some("Regular".to_string())
    } else {
        None
    };

    debug!("KR market status: is_open={}, kst_hour={}, kst_minute={}", is_open, kst_hour, kst_minute);

    MarketStatusResponse {
        market: "KR".to_string(),
        is_open,
        next_open: None,  // TODO: 다음 개장 시간 계산
        next_close: None, // TODO: 다음 폐장 시간 계산
        session,
    }
}

/// 미국 시장 상태 계산.
///
/// - 프리마켓: 04:00-09:30 EST
/// - 정규장: 09:30-16:00 EST
/// - 애프터아워: 16:00-20:00 EST
fn get_us_market_status() -> MarketStatusResponse {
    let now = Utc::now();
    // EST = UTC - 5시간 (DST 미적용시)
    // EDT = UTC - 4시간 (DST 적용시, 3월 둘째 일요일 ~ 11월 첫째 일요일)
    // 간단히 -5로 계산 (정확한 DST 계산은 추후 개선)
    let est_hour = if now.hour() >= 5 {
        now.hour() - 5
    } else {
        24 + now.hour() - 5
    };
    let est_minute = now.minute();

    let is_weekday = !matches!(now.weekday(), Weekday::Sat | Weekday::Sun);

    let current_time = NaiveTime::from_hms_opt(est_hour, est_minute, 0).unwrap();

    // 시간대 정의
    let premarket_open = NaiveTime::from_hms_opt(4, 0, 0).unwrap();
    let regular_open = NaiveTime::from_hms_opt(9, 30, 0).unwrap();
    let regular_close = NaiveTime::from_hms_opt(16, 0, 0).unwrap();
    let afterhours_close = NaiveTime::from_hms_opt(20, 0, 0).unwrap();

    let (is_open, session) = if !is_weekday {
        (false, MarketSession::Closed)
    } else if current_time >= premarket_open && current_time < regular_open {
        (true, MarketSession::PreMarket)
    } else if current_time >= regular_open && current_time < regular_close {
        (true, MarketSession::Regular)
    } else if current_time >= regular_close && current_time < afterhours_close {
        (true, MarketSession::AfterHours)
    } else {
        (false, MarketSession::Closed)
    };

    debug!("US market status: is_open={}, session={:?}, est_hour={}", is_open, session, est_hour);

    MarketStatusResponse {
        market: "US".to_string(),
        is_open,
        next_open: None,
        next_close: None,
        session: session.as_str().map(|s| s.to_string()),
    }
}

// ==================== 캔들스틱 데이터 ====================

/// 캔들스틱 데이터 쿼리.
#[derive(Debug, Deserialize)]
pub struct KlinesQuery {
    /// 심볼 (예: BTC/USDT, 005930)
    pub symbol: String,
    /// 타임프레임 (1m, 5m, 15m, 1h, 4h, 1d)
    #[serde(default = "default_timeframe")]
    pub timeframe: String,
    /// 데이터 개수 (기본: 100)
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_timeframe() -> String {
    "1d".to_string()
}

fn default_limit() -> usize {
    100
}

/// 캔들스틱 데이터 응답.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KlinesResponse {
    pub symbol: String,
    pub timeframe: String,
    pub data: Vec<CandleData>,
}

/// 단일 캔들 데이터.
#[derive(Debug, Serialize)]
pub struct CandleData {
    /// 타임스탬프 (ISO 8601 날짜)
    pub time: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

// =============================================================================
// DB에서 KIS 자격증명 로드 및 클라이언트 생성
// =============================================================================

/// DB에서 조회한 거래소 자격증명 레코드.
#[derive(Debug, sqlx::FromRow)]
struct KisCredentialRowExtended {
    encrypted_credentials: Vec<u8>,
    encryption_nonce: Vec<u8>,
    is_testnet: bool,
    settings: Option<serde_json::Value>,
    exchange_name: String,
}

/// 계좌가 ISA 계좌인지 확인.
///
/// ISA 계좌는 다음 조건 중 하나로 식별:
/// 1. settings 필드에 account_type이 "isa"인 경우
/// 2. exchange_name에 "ISA"가 포함된 경우 (대소문자 무관)
fn is_isa_account(settings: &Option<serde_json::Value>, exchange_name: &str) -> bool {
    // settings에서 account_type 확인
    if let Some(settings) = settings {
        if let Some(account_type) = settings.get("account_type") {
            if let Some(type_str) = account_type.as_str() {
                if type_str.to_lowercase() == "isa" {
                    return true;
                }
            }
        }
    }

    // exchange_name에서 ISA 확인
    exchange_name.to_uppercase().contains("ISA")
}

/// DB에서 KIS 자격증명을 로드하고 클라이언트를 생성.
///
/// # 인자
/// - `state` - 애플리케이션 상태
/// - `for_us_market` - 해외 주식용 클라이언트 여부
///   - `true`: 해외 주식 조회용 (ISA 계좌 제외)
///   - `false`: 국내 주식 조회용 (모든 계좌 사용 가능)
///
/// # 반환값
/// - `Ok((KisKrClient, KisUsClient))` - 성공 시 국내/해외 클라이언트 쌍
/// - `Err(String)` - 실패 시 에러 메시지
///
/// # 계좌 선택 로직
/// 1. 모의투자 계좌 (is_testnet=true): 테스트 환경용
/// 2. 일반 계좌 (is_testnet=false, 비-ISA): 국내/해외 모두 가능
/// 3. ISA 계좌 (is_testnet=false, ISA): 국내 전용
async fn load_kis_clients_from_db(
    state: &AppState,
    for_us_market: bool,
) -> Result<(Arc<KisKrClient>, Arc<KisUsClient>), String> {
    // 먼저 state에 이미 클라이언트가 있으면 사용 (국내 전용인 경우만)
    // 해외용은 ISA 제외 로직 때문에 매번 확인 필요
    if !for_us_market {
        if let (Some(kr_client), Some(us_client)) = (&state.kis_kr_client, &state.kis_us_client) {
            return Ok((kr_client.clone(), us_client.clone()));
        }
    }

    // DB 연결 확인
    let pool = state.db_pool.as_ref()
        .ok_or("데이터베이스 연결이 설정되지 않았습니다.")?;

    // 암호화 관리자 확인
    let encryptor = state.encryptor.as_ref()
        .ok_or("암호화 설정이 없습니다. ENCRYPTION_MASTER_KEY를 설정하세요.")?;

    // DB에서 KIS 자격증명 조회 (활성화된 실전투자 계좌 모두)
    let rows: Vec<KisCredentialRowExtended> = sqlx::query_as(
        r#"
        SELECT encrypted_credentials, encryption_nonce, is_testnet, settings, exchange_name
        FROM exchange_credentials
        WHERE exchange_id = 'kis' AND is_active = true AND is_testnet = false
        ORDER BY created_at DESC
        "#
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("자격증명 조회 실패: {}", e))?;

    if rows.is_empty() {
        return Err("KIS 자격증명이 등록되지 않았습니다. 설정에서 한국투자증권 API 키를 등록하세요.".to_string());
    }

    // 적절한 계좌 선택
    let selected_row = if for_us_market {
        // 해외 주식용: ISA가 아닌 계좌 선택
        rows.iter()
            .find(|r| !is_isa_account(&r.settings, &r.exchange_name))
            .ok_or("해외 주식 거래가 가능한 계좌가 없습니다. ISA 계좌는 국내 전용입니다.")?
    } else {
        // 국내 주식용: 아무 계좌나 사용 (첫 번째)
        rows.first()
            .ok_or("사용 가능한 KIS 계좌가 없습니다.")?
    };

    info!(
        "KIS 계좌 선택: name={}, for_us_market={}, is_isa={}",
        selected_row.exchange_name,
        for_us_market,
        is_isa_account(&selected_row.settings, &selected_row.exchange_name)
    );

    // 복호화
    let credentials: EncryptedCredentials = encryptor
        .decrypt_json(&selected_row.encrypted_credentials, &selected_row.encryption_nonce)
        .map_err(|e| format!("자격증명 복호화 실패: {}", e))?;

    // 추가 필드에서 계좌번호 추출
    let account_number = credentials.additional
        .as_ref()
        .and_then(|a| a.get("account_number").cloned())
        .unwrap_or_else(|| "00000000-01".to_string());

    // 계좌 유형 결정
    let account_type = if selected_row.is_testnet {
        KisAccountType::Paper
    } else if is_isa_account(&selected_row.settings, &selected_row.exchange_name) {
        KisAccountType::RealIsa
    } else {
        KisAccountType::RealGeneral
    };

    info!(
        "KIS 클라이언트 생성: account_type={:?}, testnet={}, account={}",
        account_type, selected_row.is_testnet,
        if account_number.len() > 4 { &account_number[..4] } else { &account_number }
    );

    // 국내 클라이언트용 KisConfig 생성
    let kr_config = KisConfig::new(
        credentials.api_key.clone(),
        credentials.api_secret.clone(),
        account_number.clone(),
        account_type,
    );
    let kr_oauth = KisOAuth::new(kr_config);
    let kr_client = Arc::new(KisKrClient::new(kr_oauth));

    // 해외 클라이언트용 KisConfig 생성 (동일한 자격증명 사용)
    let us_config = KisConfig::new(
        credentials.api_key,
        credentials.api_secret,
        account_number,
        account_type,
    );
    let us_oauth = KisOAuth::new(us_config);
    let us_client = Arc::new(KisUsClient::new(us_oauth));

    Ok((kr_client, us_client))
}

/// 심볼이 한국 주식인지 확인.
///
/// 한국 주식 코드는 6자리 숫자입니다 (예: 005930, 373220).
fn is_korean_symbol(symbol: &str) -> bool {
    symbol.len() == 6 && symbol.chars().all(|c| c.is_ascii_digit())
}

/// 캔들스틱 데이터 조회.
///
/// GET /api/v1/market/klines
///
/// **Yahoo Finance API**를 사용하여 과거 캔들 데이터를 조회합니다.
/// - 백테스트와 라이브에서 동일한 데이터셋 사용
/// - DB 캐시를 통한 효율적인 데이터 접근
/// - 분봉/시간봉: 최근 60일 제한
/// - 일봉 이상: 수년간 데이터 가능
/// - 한국 주식: ".KS" 접미사 자동 추가 (코스피)
///
/// # 캐싱 전략
/// - 요청 기반 자동 캐싱 및 증분 업데이트
/// - 동일 심볼+타임프레임 동시 요청 시 중복 API 호출 방지
/// - 시장 마감 후에는 불필요한 업데이트 생략
///
/// # 지원 간격
/// - 분봉: 1m, 5m, 15m, 30m
/// - 시간봉: 1h
/// - 일봉 이상: 1d, 1wk, 1mo
pub async fn get_klines(
    State(state): State<Arc<AppState>>,
    Query(query): Query<KlinesQuery>,
) -> Result<Json<KlinesResponse>, (StatusCode, Json<ApiError>)> {
    // 타임프레임 문자열을 Timeframe enum으로 변환
    let timeframe = parse_timeframe(&query.timeframe);

    debug!(
        symbol = %query.symbol,
        timeframe = %query.timeframe,
        limit = query.limit,
        "캔들 데이터 조회 시작"
    );

    // DB 연결이 있으면 캐시 기반 제공자 사용, 없으면 직접 Yahoo Finance 사용
    let klines = if let Some(pool) = &state.db_pool {
        let cached_provider = CachedHistoricalDataProvider::new(pool.clone());

        debug!(
            symbol = %query.symbol,
            "캐시 기반 데이터 제공자 사용"
        );

        cached_provider
            .get_klines(&query.symbol, timeframe, query.limit)
            .await
            .map_err(|e| {
                error!(
                    symbol = %query.symbol,
                    timeframe = %query.timeframe,
                    error = %e,
                    "캐시 데이터 조회 실패"
                );
                (
                    StatusCode::BAD_GATEWAY,
                    Json(ApiError::new("DATA_FETCH_ERROR", &format!("차트 데이터 조회 실패: {}", e))),
                )
            })?
    } else {
        // Fallback: DB 연결 없이 직접 Yahoo Finance 사용
        debug!(
            symbol = %query.symbol,
            "DB 연결 없음, 직접 Yahoo Finance 사용"
        );

        let provider = YahooFinanceProvider::new().map_err(|e| {
            error!("Yahoo Finance 연결 실패: {}", e);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ApiError::new("YAHOO_FINANCE_ERROR", &format!("Yahoo Finance 연결 실패: {}", e))),
            )
        })?;

        provider
            .get_klines(&query.symbol, timeframe, query.limit)
            .await
            .map_err(|e| {
                error!(
                    symbol = %query.symbol,
                    timeframe = %query.timeframe,
                    error = %e,
                    "Yahoo Finance 데이터 조회 실패"
                );
                (
                    StatusCode::BAD_GATEWAY,
                    Json(ApiError::new("YAHOO_FINANCE_ERROR", &format!("차트 데이터 조회 실패: {}", e))),
                )
            })?
    };

    info!(
        symbol = %query.symbol,
        timeframe = %query.timeframe,
        count = klines.len(),
        "캔들 데이터 조회 성공"
    );

    let candles = klines
        .into_iter()
        .map(|k| CandleData {
            time: k.open_time.format("%Y-%m-%d").to_string(),
            open: k.open.to_string().parse().unwrap_or(0.0),
            high: k.high.to_string().parse().unwrap_or(0.0),
            low: k.low.to_string().parse().unwrap_or(0.0),
            close: k.close.to_string().parse().unwrap_or(0.0),
            volume: k.volume.to_string().parse().unwrap_or(0.0),
        })
        .collect();

    Ok(Json(KlinesResponse {
        symbol: query.symbol,
        timeframe: query.timeframe,
        data: candles,
    }))
}

/// 타임프레임 문자열을 Timeframe enum으로 변환.
fn parse_timeframe(tf: &str) -> Timeframe {
    match tf.to_lowercase().as_str() {
        "1m" => Timeframe::M1,
        "3m" => Timeframe::M3,
        "5m" => Timeframe::M5,
        "15m" => Timeframe::M15,
        "30m" => Timeframe::M30,
        "1h" => Timeframe::H1,
        "2h" => Timeframe::H2,
        "4h" => Timeframe::H4,
        "6h" => Timeframe::H6,
        "8h" => Timeframe::H8,
        "12h" => Timeframe::H12,
        "1d" | "d" => Timeframe::D1,
        "3d" => Timeframe::D3,
        "1w" | "w" => Timeframe::W1,
        "1M" | "M" | "1mn" | "mn" => Timeframe::MN1,
        _ => Timeframe::D1, // 기본값: 일봉
    }
}

// ==================== 현재가 (Ticker) ====================

/// 현재가 응답.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TickerResponse {
    pub symbol: String,
    pub price: String,
    pub change_24h: String,
    pub change_24h_percent: String,
    pub high_24h: String,
    pub low_24h: String,
    pub volume_24h: String,
    pub timestamp: i64,
}

/// 현재가 쿼리.
#[derive(Debug, Deserialize)]
pub struct TickerQuery {
    pub symbol: String,
}

/// 현재가 조회.
///
/// GET /api/v1/market/ticker
///
/// DB에 저장된 KIS 자격증명을 사용하여 실시간 현재가를 조회합니다.
///
/// # 계좌 선택
/// - 한국 주식 (6자리 숫자): 모든 KIS 계좌 사용 가능
/// - 해외 주식 (알파벳): ISA 계좌 제외, 해외투자 가능 계좌만 사용
pub async fn get_ticker(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TickerQuery>,
) -> Result<Json<TickerResponse>, (StatusCode, Json<ApiError>)> {
    // 심볼 유형 확인 (한국 vs 해외)
    let is_korean = is_korean_symbol(&query.symbol);
    let for_us_market = !is_korean;

    debug!(
        symbol = %query.symbol,
        is_korean = is_korean,
        for_us_market = for_us_market,
        "현재가 조회 - 심볼 유형 확인"
    );

    // DB에서 KIS 클라이언트 로드 (심볼 유형에 맞는 계좌 선택)
    let (kr_client, us_client) = match load_kis_clients_from_db(&state, for_us_market).await {
        Ok(clients) => clients,
        Err(e) => {
            error!("KIS 클라이언트 로드 실패: {}", e);
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ApiError::new("KIS_NOT_CONFIGURED", &e)),
            ));
        }
    };

    if is_korean {
        // 국내 주식 현재가 조회
        match kr_client.get_price(&query.symbol).await {
            Ok(price_data) => {
                info!(
                    symbol = %query.symbol,
                    price = %price_data.current_price,
                    "국내 주식 현재가 조회 성공"
                );

                Ok(Json(TickerResponse {
                    symbol: query.symbol,
                    price: price_data.current_price.to_string(),
                    change_24h: price_data.price_change.to_string(),
                    change_24h_percent: price_data.change_rate.to_string(),
                    high_24h: price_data.high.to_string(),
                    low_24h: price_data.low.to_string(),
                    volume_24h: price_data.volume.to_string(),
                    timestamp: Utc::now().timestamp(),
                }))
            }
            Err(e) => {
                error!(
                    symbol = %query.symbol,
                    error = %e,
                    "국내 주식 현재가 조회 실패"
                );
                Err((
                    StatusCode::BAD_GATEWAY,
                    Json(ApiError::new("EXCHANGE_ERROR", &format!("현재가 조회 실패: {}", e))),
                ))
            }
        }
    } else {
        // 해외 주식 현재가 조회
        // exchange_code는 None으로 전달하면 클라이언트가 자동 감지
        match us_client.get_price(&query.symbol, None).await {
            Ok(price_data) => {
                info!(
                    symbol = %query.symbol,
                    price = %price_data.current_price,
                    "해외 주식 현재가 조회 성공"
                );

                Ok(Json(TickerResponse {
                    symbol: query.symbol,
                    price: price_data.current_price.to_string(),
                    change_24h: price_data.price_change.to_string(),
                    change_24h_percent: price_data.change_rate.to_string(),
                    high_24h: price_data.high.to_string(),
                    low_24h: price_data.low.to_string(),
                    volume_24h: price_data.volume.to_string(),
                    timestamp: Utc::now().timestamp(),
                }))
            }
            Err(e) => {
                error!(
                    symbol = %query.symbol,
                    error = %e,
                    "해외 주식 현재가 조회 실패"
                );
                Err((
                    StatusCode::BAD_GATEWAY,
                    Json(ApiError::new("EXCHANGE_ERROR", &format!("현재가 조회 실패: {}", e))),
                ))
            }
        }
    }
}

// ==================== 라우터 ====================

/// 시장 상태 라우터 생성.
pub fn market_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/klines", get(get_klines))
        .route("/ticker", get(get_ticker))
        .route("/:market/status", get(get_market_status))
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
    async fn test_get_kr_market_status() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/market/:market/status", get(get_market_status))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/market/KR/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let status: MarketStatusResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(status.market, "KR");
    }

    #[tokio::test]
    async fn test_get_us_market_status() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/market/:market/status", get(get_market_status))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/market/US/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let status: MarketStatusResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(status.market, "US");
    }

    #[tokio::test]
    async fn test_invalid_market() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/market/:market/status", get(get_market_status))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/market/INVALID/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
