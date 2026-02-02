//! SDUI (Server-Driven UI) 스키마 API 라우트.
//!
//! 전략 설정 UI를 자동 생성하기 위한 스키마 엔드포인트를 제공합니다.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde_json::json;
use std::sync::Arc;
use trader_core::FragmentCategory;
use trader_strategy::{FragmentRegistry, SchemaComposer, StrategyRegistry};

use crate::{error::ApiErrorResponse, state::AppState};

/// GET /api/v1/strategies/meta
///
/// 모든 전략의 메타데이터 목록을 반환합니다.
pub async fn list_strategy_meta(
    State(_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let strategies = StrategyRegistry::to_json();
    Ok(Json(strategies))
}

/// GET /api/v1/strategies/{id}/schema
///
/// 특정 전략의 완성된 SDUI 스키마를 반환합니다.
///
/// # Path Parameters
/// - `id`: 전략 ID (예: "grid_trading", "rsi_mean_reversion")
///
/// # Returns
/// 프론트엔드에서 렌더링할 수 있는 완전한 SDUI JSON
pub async fn get_strategy_schema(
    State(_state): State<Arc<AppState>>,
    Path(strategy_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    // 전략 메타데이터 조회
    let strategy_meta = StrategyRegistry::find(&strategy_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ApiErrorResponse::new(
                "STRATEGY_NOT_FOUND",
                format!("Strategy not found: {}", strategy_id),
            )),
        )
    })?;

    // 전략 인스턴스 생성 (TODO: Strategy trait에 ui_schema() 메서드 추가 필요)
    let _strategy = (strategy_meta.factory)();

    // TODO: Strategy trait에 ui_schema() 메서드 추가 필요
    // 현재는 기본 스키마 반환
    let composer = SchemaComposer::with_default_registry();

    // 임시로 기본 스키마 생성
    let schema = trader_core::StrategyUISchema::new(
        strategy_meta.id,
        strategy_meta.name,
        format!("{:?}", strategy_meta.category),
    )
    .with_description(strategy_meta.description.to_string());

    let json = composer.compose(&schema);

    Ok(Json(json))
}

/// GET /api/v1/schema/fragments
///
/// 사용 가능한 모든 Fragment 카탈로그를 반환합니다.
pub async fn list_fragments(
    State(_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let composer = SchemaComposer::with_default_registry();
    let catalog = composer.get_fragment_catalog();

    Ok(Json(catalog))
}

/// GET /api/v1/schema/fragments/{category}
///
/// 특정 카테고리의 Fragment 목록을 반환합니다.
///
/// # Path Parameters
/// - `category`: Fragment 카테고리 (예: "indicator", "filter", "risk_management")
///
/// # Supported Categories
/// - `indicator`: 기술적 지표 (RSI, MACD, Bollinger Bands 등)
/// - `filter`: 필터 조건 (RouteState, MarketRegime, Volume 등)
/// - `risk_management`: 리스크 관리 (손절, 익절, 트레일링 스탑)
/// - `position_sizing`: 포지션 크기 결정
/// - `timing`: 타이밍 설정
/// - `asset`: 자산 선택
pub async fn list_fragments_by_category(
    State(_state): State<Arc<AppState>>,
    Path(category_str): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    // 카테고리 문자열을 enum으로 변환
    let category = match category_str.to_lowercase().as_str() {
        "indicator" => FragmentCategory::Indicator,
        "filter" => FragmentCategory::Filter,
        "risk_management" | "riskmanagement" => FragmentCategory::RiskManagement,
        "position_sizing" | "positionsizing" => FragmentCategory::PositionSizing,
        "timing" => FragmentCategory::Timing,
        "asset" => FragmentCategory::Asset,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiErrorResponse::new(
                    "INVALID_CATEGORY",
                    format!(
                        "Invalid category: {}. Supported: indicator, filter, risk_management, position_sizing, timing, asset",
                        category_str
                    ),
                )),
            ));
        }
    };

    let composer = SchemaComposer::with_default_registry();
    let fragments = composer.get_fragments_by_category(category);

    Ok(Json(fragments))
}

/// GET /api/v1/schema/fragments/{fragment_id}/detail
///
/// 특정 Fragment의 상세 정보를 반환합니다.
pub async fn get_fragment_detail(
    State(_state): State<Arc<AppState>>,
    Path(fragment_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let registry = FragmentRegistry::with_builtins();

    let fragment = registry.get(&fragment_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ApiErrorResponse::new(
                "FRAGMENT_NOT_FOUND",
                format!("Fragment not found: {}", fragment_id),
            )),
        )
    })?;

    // Fragment를 JSON으로 변환
    let json = json!({
        "id": fragment.id,
        "name": fragment.name,
        "description": fragment.description,
        "category": format!("{:?}", fragment.category),
        "dependencies": fragment.dependencies,
        "fields": fragment.fields.iter().map(|field| {
            json!({
                "name": field.name,
                "type": format!("{:?}", field.field_type).to_lowercase(),
                "label": field.label,
                "description": field.description,
                "default": field.default,
                "min": field.min,
                "max": field.max,
                "options": field.options,
                "condition": field.condition,
                "required": field.required,
            })
        }).collect::<Vec<_>>(),
    });

    Ok(Json(json))
}

/// 스키마 라우터 생성.
pub fn schema_router() -> axum::Router<Arc<AppState>> {
    use axum::routing::get;

    axum::Router::new()
        .route("/fragments", get(list_fragments))
        .route("/fragments/{:category}", get(list_fragments_by_category))
        .route("/fragments/{:fragment_id}/detail", get(get_fragment_detail))
}

#[cfg(test)]
mod tests {
    use super::*;

    // 테스트는 통합 테스트에서 수행
}
