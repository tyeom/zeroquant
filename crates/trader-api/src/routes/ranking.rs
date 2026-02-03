//! Ranking API 라우트
//!
//! GlobalScore 기반 종목 랭킹 API를 제공합니다.
//!
//! # 엔드포인트
//!
//! - `POST /api/v1/ranking/global` - 모든 심볼 GlobalScore 계산
//! - `GET /api/v1/ranking/top` - 상위 랭킹 조회

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};
use utoipa::ToSchema;

use crate::repository::{GlobalScoreRepository, RankedSymbol, RankingFilter};
use crate::state::AppState;

// ================================================================================================
// Request/Response Types
// ================================================================================================

/// GlobalScore 계산 응답
#[derive(Debug, Serialize, ToSchema)]
pub struct CalculateResponse {
    /// 처리된 종목 수
    pub processed: i32,
    /// 계산 시작 시간
    pub started_at: String,
    /// 계산 완료 시간
    pub completed_at: String,
}

/// 랭킹 조회 쿼리
#[derive(Debug, Deserialize, ToSchema)]
pub struct RankingQuery {
    /// 시장 필터 (KR, US 등)
    #[serde(default)]
    pub market: Option<String>,

    /// 등급 필터 (BUY, WATCH 등)
    #[serde(default)]
    pub grade: Option<String>,

    /// 최소 점수
    #[serde(default)]
    pub min_score: Option<String>,

    /// 반환 개수 (기본 50, 최대 500)
    #[serde(default)]
    pub limit: Option<i64>,
}

/// 랭킹 조회 응답
#[derive(Debug, Serialize, ToSchema)]
pub struct RankingResponse {
    /// 종목 목록
    pub symbols: Vec<RankedSymbol>,
    /// 총 개수
    pub total: usize,
    /// 필터 정보
    pub filter: FilterInfo,
}

/// 필터 정보
#[derive(Debug, Serialize, ToSchema)]
pub struct FilterInfo {
    pub market: Option<String>,
    pub grade: Option<String>,
    pub min_score: Option<String>,
    pub limit: i64,
}

// ================================================================================================
// Handlers
// ================================================================================================

/// POST /api/v1/ranking/global - 모든 심볼 GlobalScore 계산
async fn calculate_global(
    State(state): State<Arc<AppState>>,
) -> Result<Json<CalculateResponse>, (StatusCode, String)> {
    let started_at = chrono::Utc::now();

    info!("GlobalScore 계산 요청 수신");

    // DB 연결 확인
    let db_pool = state
        .db_pool
        .as_ref()
        .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, "Database not available".to_string()))?;

    let processed = GlobalScoreRepository::calculate_all(db_pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("GlobalScore 계산 실패: {}", e),
            )
        })?;

    let completed_at = chrono::Utc::now();

    info!("GlobalScore 계산 완료: {} 종목", processed);

    Ok(Json(CalculateResponse {
        processed,
        started_at: started_at.to_rfc3339(),
        completed_at: completed_at.to_rfc3339(),
    }))
}

/// GET /api/v1/ranking/top - 상위 랭킹 조회
async fn get_top_ranked(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RankingQuery>,
) -> Result<Json<RankingResponse>, (StatusCode, String)> {
    debug!("랭킹 조회 요청: {:?}", query);

    // DB 연결 확인
    let db_pool = state
        .db_pool
        .as_ref()
        .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, "Database not available".to_string()))?;

    // min_score 파싱
    let min_score = query
        .min_score
        .as_ref()
        .and_then(|s| s.parse::<Decimal>().ok());

    let filter = RankingFilter {
        market: query.market.clone(),
        grade: query.grade.clone(),
        min_score,
        limit: query.limit,
    };

    let symbols = GlobalScoreRepository::get_top_ranked(db_pool, filter)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("랭킹 조회 실패: {}", e),
            )
        })?;

    let total = symbols.len();

    Ok(Json(RankingResponse {
        symbols,
        total,
        filter: FilterInfo {
            market: query.market,
            grade: query.grade,
            min_score: query.min_score,
            limit: query.limit.unwrap_or(50),
        },
    }))
}

// ================================================================================================
// Router
// ================================================================================================

/// Ranking 라우터 생성
pub fn ranking_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/global", post(calculate_global))
        .route("/top", get(get_top_ranked))
}
