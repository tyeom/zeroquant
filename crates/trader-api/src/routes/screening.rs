//! 종목 스크리닝 API 라우트
//!
//! Fundamental + OHLCV 데이터 기반 종목 필터링 API를 제공합니다.
//!
//! # 엔드포인트
//!
//! - `POST /api/v1/screening` - 커스텀 스크리닝 실행
//! - `GET /api/v1/screening/presets` - 사용 가능한 프리셋 목록
//! - `GET /api/v1/screening/presets/{preset}` - 프리셋 스크리닝 실행
//! - `GET /api/v1/screening/momentum` - 모멘텀 기반 스크리닝

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, warn};
use utoipa::ToSchema;

use crate::repository::{
    MomentumScreenResult, ScreeningFilter, ScreeningPreset, ScreeningRepository, ScreeningResult,
};
use crate::state::AppState;

// ==================== Request/Response 타입 ====================

/// 커스텀 스크리닝 요청
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ScreeningRequest {
    /// 시장 필터 (KR, US, CRYPTO)
    #[serde(default)]
    pub market: Option<String>,
    /// 거래소 필터
    #[serde(default)]
    pub exchange: Option<String>,
    /// 섹터 필터
    #[serde(default)]
    pub sector: Option<String>,

    // 시가총액 필터
    #[serde(default)]
    pub min_market_cap: Option<String>,
    #[serde(default)]
    pub max_market_cap: Option<String>,

    // 밸류에이션 필터
    #[serde(default)]
    pub min_per: Option<String>,
    #[serde(default)]
    pub max_per: Option<String>,
    #[serde(default)]
    pub min_pbr: Option<String>,
    #[serde(default)]
    pub max_pbr: Option<String>,

    // 수익성 필터
    #[serde(default)]
    pub min_roe: Option<String>,
    #[serde(default)]
    pub max_roe: Option<String>,
    #[serde(default)]
    pub min_roa: Option<String>,
    #[serde(default)]
    pub max_roa: Option<String>,

    // 배당 필터
    #[serde(default)]
    pub min_dividend_yield: Option<String>,
    #[serde(default)]
    pub max_dividend_yield: Option<String>,

    // 안정성 필터
    #[serde(default)]
    pub max_debt_ratio: Option<String>,

    // 성장성 필터
    #[serde(default)]
    pub min_revenue_growth: Option<String>,
    #[serde(default)]
    pub min_earnings_growth: Option<String>,

    // 52주 고저가 필터
    #[serde(default)]
    pub max_distance_from_52w_high: Option<String>,
    #[serde(default)]
    pub min_distance_from_52w_low: Option<String>,

    // 거래량 필터
    #[serde(default)]
    pub min_volume_ratio: Option<String>,

    // 정렬 및 페이지네이션
    #[serde(default)]
    pub sort_by: Option<String>,
    #[serde(default)]
    pub sort_order: Option<String>,
    #[serde(default)]
    pub limit: Option<i32>,
    #[serde(default)]
    pub offset: Option<i32>,
}

/// 스크리닝 결과 응답
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ScreeningResponse {
    /// 총 결과 수
    pub total: usize,
    /// 결과 목록
    pub results: Vec<ScreeningResultDto>,
    /// 적용된 필터 요약
    pub filter_summary: String,
}

/// 스크리닝 결과 DTO
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ScreeningResultDto {
    pub ticker: String,
    pub name: String,
    pub market: String,
    pub exchange: Option<String>,
    pub sector: Option<String>,

    // Fundamental
    pub market_cap: Option<String>,
    pub per: Option<String>,
    pub pbr: Option<String>,
    pub roe: Option<String>,
    pub roa: Option<String>,
    pub eps: Option<String>,
    pub dividend_yield: Option<String>,
    pub operating_margin: Option<String>,
    pub debt_ratio: Option<String>,
    pub revenue_growth_yoy: Option<String>,
    pub earnings_growth_yoy: Option<String>,

    // 가격 정보
    pub current_price: Option<String>,
    pub week_52_high: Option<String>,
    pub week_52_low: Option<String>,
    pub distance_from_52w_high: Option<String>,
    pub distance_from_52w_low: Option<String>,
}

/// 프리셋 목록 응답
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PresetsListResponse {
    pub presets: Vec<ScreeningPreset>,
}

/// 프리셋 스크리닝 쿼리
#[derive(Debug, Clone, Deserialize)]
pub struct PresetQuery {
    /// 시장 필터 (KR, US)
    #[serde(default)]
    pub market: Option<String>,
    /// 결과 제한
    #[serde(default)]
    pub limit: Option<i32>,
}

/// 모멘텀 스크리닝 쿼리
#[derive(Debug, Clone, Deserialize)]
pub struct MomentumQuery {
    /// 시장 필터
    #[serde(default)]
    pub market: Option<String>,
    /// 조회 기간 (일)
    #[serde(default = "default_momentum_days")]
    pub days: i32,
    /// 최소 변동률 (%)
    #[serde(default = "default_min_change")]
    pub min_change_pct: String,
    /// 최소 거래량 배율
    #[serde(default)]
    pub min_volume_ratio: Option<String>,
    /// 결과 제한
    #[serde(default = "default_momentum_limit")]
    pub limit: i32,
}

fn default_momentum_days() -> i32 {
    5
}

fn default_min_change() -> String {
    "5".to_string()
}

fn default_momentum_limit() -> i32 {
    50
}

/// 모멘텀 스크리닝 결과 응답
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MomentumResponse {
    pub total: usize,
    pub days: i32,
    pub min_change_pct: String,
    pub results: Vec<MomentumResultDto>,
}

/// 모멘텀 결과 DTO
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MomentumResultDto {
    pub symbol: String,
    pub name: String,
    pub market: String,
    pub exchange: Option<String>,
    pub start_price: String,
    pub end_price: String,
    pub change_pct: String,
    pub avg_volume: String,
    pub current_volume: String,
    pub volume_ratio: String,
}

/// 에러 응답
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
}

// ==================== 헬퍼 함수 ====================

fn parse_decimal(s: &Option<String>) -> Option<Decimal> {
    s.as_ref().and_then(|v| v.parse::<Decimal>().ok())
}

fn decimal_to_string(d: Option<Decimal>) -> Option<String> {
    d.map(|v| v.to_string())
}

fn to_screening_filter(req: &ScreeningRequest) -> ScreeningFilter {
    ScreeningFilter {
        market: req.market.clone(),
        exchange: req.exchange.clone(),
        sector: req.sector.clone(),
        min_market_cap: parse_decimal(&req.min_market_cap),
        max_market_cap: parse_decimal(&req.max_market_cap),
        min_per: parse_decimal(&req.min_per),
        max_per: parse_decimal(&req.max_per),
        min_pbr: parse_decimal(&req.min_pbr),
        max_pbr: parse_decimal(&req.max_pbr),
        min_roe: parse_decimal(&req.min_roe),
        max_roe: parse_decimal(&req.max_roe),
        min_roa: parse_decimal(&req.min_roa),
        max_roa: parse_decimal(&req.max_roa),
        min_dividend_yield: parse_decimal(&req.min_dividend_yield),
        max_dividend_yield: parse_decimal(&req.max_dividend_yield),
        max_debt_ratio: parse_decimal(&req.max_debt_ratio),
        min_revenue_growth: parse_decimal(&req.min_revenue_growth),
        min_earnings_growth: parse_decimal(&req.min_earnings_growth),
        max_distance_from_52w_high: parse_decimal(&req.max_distance_from_52w_high),
        min_distance_from_52w_low: parse_decimal(&req.min_distance_from_52w_low),
        min_volume_ratio: parse_decimal(&req.min_volume_ratio),
        sort_by: req.sort_by.clone(),
        sort_order: req.sort_order.clone(),
        limit: req.limit,
        offset: req.offset,
        ..Default::default()
    }
}

fn to_result_dto(r: ScreeningResult) -> ScreeningResultDto {
    ScreeningResultDto {
        ticker: r.ticker,
        name: r.name,
        market: r.market,
        exchange: r.exchange,
        sector: r.sector,
        market_cap: decimal_to_string(r.market_cap),
        per: decimal_to_string(r.per),
        pbr: decimal_to_string(r.pbr),
        roe: decimal_to_string(r.roe),
        roa: decimal_to_string(r.roa),
        eps: decimal_to_string(r.eps),
        dividend_yield: decimal_to_string(r.dividend_yield),
        operating_margin: decimal_to_string(r.operating_margin),
        debt_ratio: decimal_to_string(r.debt_ratio),
        revenue_growth_yoy: decimal_to_string(r.revenue_growth_yoy),
        earnings_growth_yoy: decimal_to_string(r.earnings_growth_yoy),
        current_price: decimal_to_string(r.current_price),
        week_52_high: decimal_to_string(r.week_52_high),
        week_52_low: decimal_to_string(r.week_52_low),
        distance_from_52w_high: decimal_to_string(r.distance_from_52w_high),
        distance_from_52w_low: decimal_to_string(r.distance_from_52w_low),
    }
}

fn to_momentum_dto(r: MomentumScreenResult) -> MomentumResultDto {
    MomentumResultDto {
        symbol: r.symbol,
        name: r.name,
        market: r.market,
        exchange: r.exchange,
        start_price: r.start_price.to_string(),
        end_price: r.end_price.to_string(),
        change_pct: r.change_pct.to_string(),
        avg_volume: r.avg_volume.to_string(),
        current_volume: r.current_volume.to_string(),
        volume_ratio: r.volume_ratio.to_string(),
    }
}

fn error_response(code: &str, message: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            code: code.to_string(),
            message: message.to_string(),
        }),
    )
}

// ==================== 핸들러 ====================

/// 커스텀 스크리닝 실행
///
/// POST /api/v1/screening
#[utoipa::path(
    post,
    path = "/api/v1/screening",
    request_body = ScreeningRequest,
    responses(
        (status = 200, description = "스크리닝 성공", body = ScreeningResponse),
        (status = 500, description = "서버 오류", body = ErrorResponse)
    ),
    tag = "screening"
)]
pub async fn run_screening(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ScreeningRequest>,
) -> impl IntoResponse {
    debug!("커스텀 스크리닝 요청: {:?}", request);

    let db_pool = match &state.db_pool {
        Some(pool) => pool,
        None => {
            return error_response("DATABASE_ERROR", "Database not available").into_response();
        }
    };

    let filter = to_screening_filter(&request);

    let results = match ScreeningRepository::screen(db_pool, &filter).await {
        Ok(r) => r,
        Err(e) => {
            warn!("스크리닝 실패: {}", e);
            return error_response("SCREENING_ERROR", &format!("스크리닝 실패: {}", e))
                .into_response();
        }
    };

    let filter_summary = build_filter_summary(&request);
    let total = results.len();
    let dto_results: Vec<ScreeningResultDto> = results.into_iter().map(to_result_dto).collect();

    Json(ScreeningResponse {
        total,
        results: dto_results,
        filter_summary,
    })
    .into_response()
}

/// 프리셋 목록 조회
///
/// GET /api/v1/screening/presets
#[utoipa::path(
    get,
    path = "/api/v1/screening/presets",
    responses(
        (status = 200, description = "프리셋 목록", body = PresetsListResponse)
    ),
    tag = "screening"
)]
pub async fn list_presets() -> impl IntoResponse {
    let presets = ScreeningRepository::available_presets();
    Json(PresetsListResponse { presets })
}

/// 프리셋 스크리닝 실행
///
/// GET /api/v1/screening/presets/{preset}
#[utoipa::path(
    get,
    path = "/api/v1/screening/presets/{preset}",
    params(
        ("preset" = String, Path, description = "프리셋 ID (value, dividend, growth, snowball, large_cap, near_52w_low)")
    ),
    responses(
        (status = 200, description = "프리셋 스크리닝 성공", body = ScreeningResponse),
        (status = 500, description = "서버 오류", body = ErrorResponse)
    ),
    tag = "screening"
)]
pub async fn run_preset_screening(
    State(state): State<Arc<AppState>>,
    Path(preset): Path<String>,
    Query(query): Query<PresetQuery>,
) -> impl IntoResponse {
    debug!(
        "프리셋 스크리닝 요청: preset={}, market={:?}",
        preset, query.market
    );

    let db_pool = match &state.db_pool {
        Some(pool) => pool,
        None => {
            return error_response("DATABASE_ERROR", "Database not available").into_response();
        }
    };

    let results =
        match ScreeningRepository::screen_preset(db_pool, &preset, query.market.as_deref()).await {
            Ok(r) => r,
            Err(e) => {
                warn!("프리셋 스크리닝 실패: {}", e);
                return error_response("SCREENING_ERROR", &format!("프리셋 스크리닝 실패: {}", e))
                    .into_response();
            }
        };

    let filter_summary = format!(
        "프리셋: {}, 시장: {}",
        preset,
        query.market.as_deref().unwrap_or("전체")
    );
    let total = results.len();
    let dto_results: Vec<ScreeningResultDto> = results.into_iter().map(to_result_dto).collect();

    Json(ScreeningResponse {
        total,
        results: dto_results,
        filter_summary,
    })
    .into_response()
}

/// 모멘텀 스크리닝 실행
///
/// GET /api/v1/screening/momentum
#[utoipa::path(
    get,
    path = "/api/v1/screening/momentum",
    params(
        ("market" = Option<String>, Query, description = "시장 필터 (KR, US)"),
        ("days" = Option<i32>, Query, description = "조회 기간 (일)"),
        ("min_change_pct" = Option<String>, Query, description = "최소 변동률 (%)"),
        ("min_volume_ratio" = Option<String>, Query, description = "최소 거래량 배율"),
        ("limit" = Option<i32>, Query, description = "결과 제한")
    ),
    responses(
        (status = 200, description = "모멘텀 스크리닝 성공", body = MomentumResponse),
        (status = 400, description = "잘못된 요청", body = ErrorResponse),
        (status = 500, description = "서버 오류", body = ErrorResponse)
    ),
    tag = "screening"
)]
pub async fn run_momentum_screening(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MomentumQuery>,
) -> impl IntoResponse {
    debug!(
        "모멘텀 스크리닝 요청: days={}, min_change={}, market={:?}",
        query.days, query.min_change_pct, query.market
    );

    let db_pool = match &state.db_pool {
        Some(pool) => pool,
        None => {
            return error_response("DATABASE_ERROR", "Database not available").into_response();
        }
    };

    let min_change: Decimal = match query.min_change_pct.parse() {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    code: "BAD_REQUEST".to_string(),
                    message: "Invalid min_change_pct value".to_string(),
                }),
            )
                .into_response();
        }
    };

    let min_volume_ratio = query
        .min_volume_ratio
        .as_ref()
        .and_then(|v| v.parse::<Decimal>().ok());

    let results = match ScreeningRepository::screen_momentum(
        db_pool,
        query.market.as_deref(),
        query.days,
        min_change,
        min_volume_ratio,
        query.limit,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!("모멘텀 스크리닝 실패: {}", e);
            return error_response("SCREENING_ERROR", &format!("모멘텀 스크리닝 실패: {}", e))
                .into_response();
        }
    };

    let total = results.len();
    let dto_results: Vec<MomentumResultDto> = results.into_iter().map(to_momentum_dto).collect();

    Json(MomentumResponse {
        total,
        days: query.days,
        min_change_pct: query.min_change_pct,
        results: dto_results,
    })
    .into_response()
}

/// 필터 요약 문자열 생성
fn build_filter_summary(req: &ScreeningRequest) -> String {
    let mut parts = Vec::new();

    if let Some(ref m) = req.market {
        parts.push(format!("시장={}", m));
    }
    if let Some(ref e) = req.exchange {
        parts.push(format!("거래소={}", e));
    }
    if req.max_per.is_some() {
        parts.push(format!("PER≤{}", req.max_per.as_ref().unwrap()));
    }
    if req.max_pbr.is_some() {
        parts.push(format!("PBR≤{}", req.max_pbr.as_ref().unwrap()));
    }
    if req.min_roe.is_some() {
        parts.push(format!("ROE≥{}", req.min_roe.as_ref().unwrap()));
    }
    if req.min_dividend_yield.is_some() {
        parts.push(format!(
            "배당수익률≥{}",
            req.min_dividend_yield.as_ref().unwrap()
        ));
    }

    if parts.is_empty() {
        "필터 없음".to_string()
    } else {
        parts.join(", ")
    }
}

// ==================== 라우터 ====================

/// 스크리닝 라우터 생성
pub fn screening_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", post(run_screening))
        .route("/presets", get(list_presets))
        .route("/presets/{preset}", get(run_preset_screening))
        .route("/momentum", get(run_momentum_screening))
}
