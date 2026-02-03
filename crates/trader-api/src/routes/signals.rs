//! SignalMarker API 라우트
//!
//! 백테스트 및 실거래에서 발생한 기술 신호를 조회하고 검색합니다.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

use crate::error::ApiErrorResponse;
use crate::repository::SignalMarkerRepository;
use crate::AppState;
use trader_core::{SignalIndicators, SignalMarker};

// ==================== Request/Response 타입 ====================

/// 지표 기반 검색 요청
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SignalSearchRequest {
    /// 지표 필터 (JSONB 쿼리)
    ///
    /// # 예시
    /// ```json
    /// {
    ///   "rsi": {"$gte": 70.0},
    ///   "macd": {"$gt": 0}
    /// }
    /// ```
    pub indicator_filter: JsonValue,

    /// 신호 유형 필터 (선택)
    #[serde(default)]
    pub signal_type: Option<String>,

    /// 최대 결과 개수 (기본 100, 최대 1000)
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    100
}

/// 심볼별 신호 조회 요청
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct SymbolSignalsQuery {
    /// 심볼 (예: "005930")
    pub symbol: String,

    /// 거래소 (예: "KRX")
    pub exchange: String,

    /// 시작 시각 (ISO 8601)
    #[serde(default)]
    pub start_time: Option<DateTime<Utc>>,

    /// 종료 시각 (ISO 8601)
    #[serde(default)]
    pub end_time: Option<DateTime<Utc>>,

    /// 최대 결과 개수
    #[serde(default = "default_limit")]
    pub limit: i64,
}

/// 전략별 신호 조회 요청
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct StrategySignalsQuery {
    /// 전략 ID
    pub strategy_id: String,

    /// 시작 시각 (ISO 8601)
    #[serde(default)]
    pub start_time: Option<DateTime<Utc>>,

    /// 종료 시각 (ISO 8601)
    #[serde(default)]
    pub end_time: Option<DateTime<Utc>>,

    /// 최대 결과 개수
    #[serde(default = "default_limit")]
    pub limit: i64,
}

/// 신호 마커 응답 DTO
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SignalMarkerDto {
    /// 신호 ID
    pub id: String,

    /// 심볼
    pub symbol: String,

    /// 타임스탬프
    pub timestamp: DateTime<Utc>,

    /// 신호 유형
    pub signal_type: String,

    /// 방향 (Buy/Sell)
    pub side: Option<String>,

    /// 가격
    pub price: String,

    /// 신호 강도 (0.0 ~ 1.0)
    pub strength: f64,

    /// 지표 정보
    pub indicators: SignalIndicators,

    /// 신호 이유
    pub reason: String,

    /// 전략 ID
    pub strategy_id: String,

    /// 전략 이름
    pub strategy_name: String,

    /// 실행 여부
    pub executed: bool,
}

impl From<SignalMarker> for SignalMarkerDto {
    fn from(marker: SignalMarker) -> Self {
        Self {
            id: marker.id.to_string(),
            symbol: marker.symbol.to_string(),
            timestamp: marker.timestamp,
            signal_type: marker.signal_type.to_string(),
            side: marker.side.map(|s| s.to_string()),
            price: marker.price.to_string(),
            strength: marker.strength,
            indicators: marker.indicators,
            reason: marker.reason,
            strategy_id: marker.strategy_id,
            strategy_name: marker.strategy_name,
            executed: marker.executed,
        }
    }
}

/// 신호 검색 응답
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SignalSearchResponse {
    /// 총 결과 수
    pub total: usize,

    /// 신호 목록
    pub signals: Vec<SignalMarkerDto>,
}

// ==================== API 핸들러 ====================

/// 지표 기반 신호 검색
///
/// JSONB 쿼리를 사용하여 특정 지표 조건을 만족하는 신호를 검색합니다.
///
/// # 지원 연산자
/// - `$gte`: >=
/// - `$lte`: <=
/// - `$gt`: >
/// - `$lt`: <
/// - `$eq`: =
#[utoipa::path(
    post,
    path = "/api/v1/signals/search",
    request_body = SignalSearchRequest,
    responses(
        (status = 200, description = "검색 성공", body = SignalSearchResponse),
        (status = 400, description = "잘못된 요청", body = ApiErrorResponse),
        (status = 500, description = "서버 오류", body = ApiErrorResponse)
    ),
    tag = "signals"
)]
pub async fn search_signals(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SignalSearchRequest>,
) -> Result<Json<SignalSearchResponse>, (StatusCode, Json<ApiErrorResponse>)> {
    let db_pool = match &state.db_pool {
        Some(pool) => pool,
        None => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new("DATABASE_ERROR", "Database not available")),
            ))
        }
    };

    let repo = SignalMarkerRepository::new(db_pool.clone());

    let markers = repo
        .search_by_indicator(
            req.indicator_filter,
            req.signal_type.as_deref(),
            Some(req.limit),
        )
        .await?;

    let total = markers.len();
    let signals = markers.into_iter().map(SignalMarkerDto::from).collect();

    Ok(Json(SignalSearchResponse { total, signals }))
}

/// 심볼별 신호 조회
#[utoipa::path(
    get,
    path = "/api/v1/signals/by-symbol",
    params(SymbolSignalsQuery),
    responses(
        (status = 200, description = "조회 성공", body = SignalSearchResponse),
        (status = 400, description = "잘못된 요청", body = ApiErrorResponse),
        (status = 500, description = "서버 오류", body = ApiErrorResponse)
    ),
    tag = "signals"
)]
pub async fn get_signals_by_symbol(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SymbolSignalsQuery>,
) -> Result<Json<SignalSearchResponse>, (StatusCode, Json<ApiErrorResponse>)> {
    let db_pool = match &state.db_pool {
        Some(pool) => pool,
        None => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new("DATABASE_ERROR", "Database not available")),
            ))
        }
    };

    let repo = SignalMarkerRepository::new(db_pool.clone());

    let markers = repo
        .find_by_symbol(
            &query.symbol,
            &query.exchange,
            query.start_time,
            query.end_time,
            Some(query.limit),
        )
        .await?;

    let total = markers.len();
    let signals = markers.into_iter().map(SignalMarkerDto::from).collect();

    Ok(Json(SignalSearchResponse { total, signals }))
}

/// 전략별 신호 조회
#[utoipa::path(
    get,
    path = "/api/v1/signals/by-strategy",
    params(StrategySignalsQuery),
    responses(
        (status = 200, description = "조회 성공", body = SignalSearchResponse),
        (status = 400, description = "잘못된 요청", body = ApiErrorResponse),
        (status = 500, description = "서버 오류", body = ApiErrorResponse)
    ),
    tag = "signals"
)]
pub async fn get_signals_by_strategy(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StrategySignalsQuery>,
) -> Result<Json<SignalSearchResponse>, (StatusCode, Json<ApiErrorResponse>)> {
    let db_pool = match &state.db_pool {
        Some(pool) => pool,
        None => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new("DATABASE_ERROR", "Database not available")),
            ))
        }
    };

    let repo = SignalMarkerRepository::new(db_pool.clone());

    let markers = repo
        .find_by_strategy(
            &query.strategy_id,
            query.start_time,
            query.end_time,
            Some(query.limit),
        )
        .await?;

    let total = markers.len();
    let signals = markers.into_iter().map(SignalMarkerDto::from).collect();

    Ok(Json(SignalSearchResponse { total, signals }))
}

// ==================== 라우터 ====================

/// SignalMarker API 라우터
pub fn signals_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/search", post(search_signals))
        .route("/by-symbol", get(get_signals_by_symbol))
        .route("/by-strategy", get(get_signals_by_strategy))
}
