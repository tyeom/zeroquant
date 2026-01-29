//! 전략 관리 endpoint.
//!
//! 전략 목록 조회, 시작/중지, 설정 변경을 위한 REST API를 제공합니다.
//!
//! # 엔드포인트
//!
//! - `GET /api/v1/strategies` - 전략 목록 조회
//! - `POST /api/v1/strategies` - 전략 생성
//! - `GET /api/v1/strategies/:id` - 특정 전략 상세 조회
//! - `DELETE /api/v1/strategies/:id` - 전략 삭제
//! - `POST /api/v1/strategies/:id/start` - 전략 시작
//! - `POST /api/v1/strategies/:id/stop` - 전략 중지
//! - `PUT /api/v1/strategies/:id/config` - 전략 설정 변경

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

use crate::state::AppState;
use crate::websocket::{ServerMessage, StrategyUpdateData};
use trader_strategy::{
    strategies::{
        BollingerStrategy, GridStrategy, HaaStrategy, MagicSplitStrategy,
        RsiStrategy, SimplePowerStrategy, SmaStrategy, StockRotationStrategy,
        VolatilityBreakoutStrategy, XaaStrategy,
    },
    EngineError, EngineStats, Strategy, StrategyStatus,
};

// ==================== 응답 타입 ====================

/// 전략 목록 응답.
#[derive(Debug, Serialize, Deserialize)]
pub struct StrategiesListResponse {
    /// 전략 목록
    pub strategies: Vec<StrategyListItem>,
    /// 전체 전략 수
    pub total: usize,
    /// 실행 중인 전략 수
    pub running: usize,
}

/// 전략 목록 항목.
#[derive(Debug, Serialize, Deserialize)]
pub struct StrategyListItem {
    /// 전략 ID
    pub id: String,
    /// 전략 이름
    pub name: String,
    /// 전략 상태 ("Running", "Stopped", "Error")
    pub status: String,
    /// 시장 ("KR", "US", "CRYPTO")
    pub market: String,
    /// 거래 심볼 목록
    pub symbols: Vec<String>,
    /// 손익
    pub pnl: f64,
    /// 승률
    #[serde(rename = "winRate")]
    pub win_rate: f64,
    /// 거래 횟수
    #[serde(rename = "tradesCount")]
    pub trades_count: u64,
}

/// 전략 상세 응답.
#[derive(Debug, Serialize)]
pub struct StrategyDetailResponse {
    /// 전략 ID
    pub id: String,
    /// 전략 타입 (예: "grid_trading", "rsi")
    pub strategy_type: String,
    /// 전략 상태 정보
    #[serde(flatten)]
    pub status: StrategyStatus,
    /// 전략 설정 (편집용)
    pub config: Value,
}

/// 전략 시작/중지 응답.
#[derive(Debug, Serialize)]
pub struct StrategyActionResponse {
    /// 성공 여부
    pub success: bool,
    /// 전략 ID
    pub strategy_id: String,
    /// 수행된 액션
    pub action: String,
    /// 메시지
    pub message: String,
}

/// 전략 설정 변경 요청.
#[derive(Debug, Deserialize)]
pub struct UpdateConfigRequest {
    /// 새로운 설정 (JSON)
    pub config: Value,
}

/// 전략 생성 요청.
#[derive(Debug, Deserialize)]
pub struct CreateStrategyRequest {
    /// 전략 타입 (예: "grid_trading", "rsi", "bollinger" 등)
    pub strategy_type: String,
    /// 전략 이름 (사용자 지정, 옵션)
    pub name: Option<String>,
    /// 전략 파라미터
    pub parameters: Value,
}

/// 전략 생성 응답.
#[derive(Debug, Serialize)]
pub struct CreateStrategyResponse {
    /// 성공 여부
    pub success: bool,
    /// 생성된 전략 ID
    pub strategy_id: String,
    /// 전략 이름
    pub name: String,
    /// 메시지
    pub message: String,
}

/// 엔진 통계 응답.
#[derive(Debug, Serialize, Deserialize)]
pub struct EngineStatsResponse {
    /// 전체 전략 수
    pub total_strategies: usize,
    /// 실행 중인 전략 수
    pub running_strategies: usize,
    /// 총 생성된 신호 수
    pub total_signals_generated: u64,
    /// 총 체결된 주문 수
    pub total_orders_filled: u64,
    /// 총 처리된 시장 데이터 수
    pub total_market_data_processed: u64,
}

impl From<EngineStats> for EngineStatsResponse {
    fn from(stats: EngineStats) -> Self {
        Self {
            total_strategies: stats.total_strategies,
            running_strategies: stats.running_strategies,
            total_signals_generated: stats.total_signals_generated,
            total_orders_filled: stats.total_orders_filled,
            total_market_data_processed: stats.total_market_data_processed,
        }
    }
}

/// API 에러 응답.
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiError {
    /// 에러 코드
    pub code: String,
    /// 에러 메시지
    pub message: String,
}

impl ApiError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

// ==================== 전략 팩토리 ====================

/// 전략 타입에 따라 전략 인스턴스를 생성.
fn create_strategy_instance(strategy_type: &str) -> Result<Box<dyn Strategy>, String> {
    match strategy_type {
        "rsi" | "rsi_mean_reversion" => Ok(Box::new(RsiStrategy::new())),
        "grid" | "grid_trading" => Ok(Box::new(GridStrategy::new())),
        "bollinger" | "bollinger_bands" => Ok(Box::new(BollingerStrategy::new())),
        "volatility_breakout" | "volatility" => Ok(Box::new(VolatilityBreakoutStrategy::new())),
        "magic_split" | "split" => Ok(Box::new(MagicSplitStrategy::new())),
        "simple_power" => Ok(Box::new(SimplePowerStrategy::new())),
        "haa" => Ok(Box::new(HaaStrategy::new())),
        "xaa" => Ok(Box::new(XaaStrategy::new())),
        "sma" | "sma_crossover" | "ma_crossover" => Ok(Box::new(SmaStrategy::new())),
        "stock_rotation" | "rotation" => Ok(Box::new(StockRotationStrategy::new())),
        _ => Err(format!("Unknown strategy type: {}", strategy_type)),
    }
}

/// 전략 타입에서 기본 이름 가져오기.
fn get_strategy_default_name(strategy_type: &str) -> &'static str {
    match strategy_type {
        "rsi" | "rsi_mean_reversion" => "RSI 평균회귀",
        "grid" | "grid_trading" => "그리드 트레이딩",
        "bollinger" | "bollinger_bands" => "볼린저 밴드",
        "volatility_breakout" | "volatility" => "변동성 돌파",
        "magic_split" | "split" => "Magic Split",
        "simple_power" => "Simple Power",
        "haa" => "HAA",
        "xaa" => "XAA",
        "sma" | "sma_crossover" | "ma_crossover" => "이동평균 크로스오버",
        "stock_rotation" | "rotation" => "종목 갈아타기",
        _ => "Unknown Strategy",
    }
}

// ==================== 에러 처리 ====================

/// EngineError를 HTTP 응답으로 변환.
fn engine_error_to_response(err: EngineError) -> (StatusCode, Json<ApiError>) {
    let (status, code) = match &err {
        EngineError::StrategyNotFound(_) => (StatusCode::NOT_FOUND, "STRATEGY_NOT_FOUND"),
        EngineError::StrategyAlreadyExists(_) => (StatusCode::CONFLICT, "STRATEGY_EXISTS"),
        EngineError::InitializationFailed(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INIT_FAILED"),
        EngineError::NotRunning(_) => (StatusCode::BAD_REQUEST, "NOT_RUNNING"),
        EngineError::AlreadyRunning(_) => (StatusCode::BAD_REQUEST, "ALREADY_RUNNING"),
        EngineError::ChannelError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "CHANNEL_ERROR"),
        EngineError::InternalError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
    };

    (status, Json(ApiError::new(code, err.to_string())))
}

// ==================== handler ====================

/// 전략 생성.
///
/// POST /api/v1/strategies
pub async fn create_strategy(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateStrategyRequest>,
) -> Result<Json<CreateStrategyResponse>, (StatusCode, Json<ApiError>)> {
    // 전략 인스턴스 생성
    let strategy = create_strategy_instance(&request.strategy_type).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("INVALID_STRATEGY_TYPE", e)),
        )
    })?;

    // 전략 ID 생성 (UUID)
    let strategy_id = format!("{}_{}", request.strategy_type, Uuid::new_v4().to_string()[..8].to_string());

    // 전략 이름 (커스텀 이름이 있으면 사용, 없으면 기본 이름)
    let custom_name = request.name.clone();
    let display_name = custom_name
        .clone()
        .unwrap_or_else(|| get_strategy_default_name(&request.strategy_type).to_string());

    // 엔진에 전략 등록 (커스텀 이름 전달)
    let engine = state.strategy_engine.read().await;
    engine
        .register_strategy(&strategy_id, strategy, request.parameters.clone(), custom_name)
        .await
        .map_err(engine_error_to_response)?;

    // WebSocket 브로드캐스트: 전략 생성 알림
    state.broadcast(ServerMessage::StrategyUpdate(StrategyUpdateData {
        strategy_id: strategy_id.clone(),
        name: display_name.clone(),
        running: false,
        event: "created".to_string(),
        data: Some(serde_json::json!({
            "strategy_type": request.strategy_type
        })),
        timestamp: Utc::now().timestamp_millis(),
    }));

    Ok(Json(CreateStrategyResponse {
        success: true,
        strategy_id: strategy_id.clone(),
        name: display_name,
        message: format!("Strategy '{}' created successfully", strategy_id),
    }))
}

/// 전략 삭제.
///
/// DELETE /api/v1/strategies/:id
pub async fn delete_strategy(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<StrategyActionResponse>, (StatusCode, Json<ApiError>)> {
    let engine = state.strategy_engine.read().await;

    // 삭제 전 전략 정보 가져오기 (브로드캐스트용)
    let strategy_name = engine
        .get_strategy_status(&id)
        .await
        .map(|s| s.name)
        .unwrap_or_else(|_| id.clone());

    engine
        .unregister_strategy(&id)
        .await
        .map_err(engine_error_to_response)?;

    // WebSocket 브로드캐스트: 전략 삭제 알림
    state.broadcast(ServerMessage::StrategyUpdate(StrategyUpdateData {
        strategy_id: id.clone(),
        name: strategy_name,
        running: false,
        event: "deleted".to_string(),
        data: None,
        timestamp: Utc::now().timestamp_millis(),
    }));

    Ok(Json(StrategyActionResponse {
        success: true,
        strategy_id: id.clone(),
        action: "delete".to_string(),
        message: format!("Strategy '{}' deleted successfully", id),
    }))
}

/// 전략 목록 조회.
///
/// GET /api/v1/strategies
pub async fn list_strategies(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let engine = state.strategy_engine.read().await;
    let all_statuses = engine.get_all_statuses().await;

    let mut strategies: Vec<StrategyListItem> = all_statuses
        .into_iter()
        .map(|(id, status)| {
            // 전략 상태 문자열 변환
            let status_str = if status.running {
                "Running".to_string()
            } else {
                "Stopped".to_string()
            };

            // 전략 ID에서 시장 추론 (향후 설정에서 가져오도록 개선 필요)
            let market = if id.contains("kis") || id.contains("kr") {
                "KR".to_string()
            } else if id.contains("binance") || id.contains("crypto") {
                "CRYPTO".to_string()
            } else {
                "KR".to_string() // 기본값
            };

            // 심볼 목록 (향후 설정에서 가져오도록 개선 필요)
            let symbols = vec!["005930".to_string()]; // 기본값

            StrategyListItem {
                id,
                name: status.name,
                status: status_str,
                market,
                symbols,
                pnl: 0.0, // 향후 실제 PnL 계산 연동
                win_rate: 0.0,
                trades_count: status.stats.signals_generated, // 신호 수를 거래 수로 사용
            }
        })
        .collect();

    // ID로 정렬
    strategies.sort_by(|a, b| a.id.cmp(&b.id));

    let running_count = strategies.iter().filter(|s| s.status == "Running").count();
    let total = strategies.len();

    Json(StrategiesListResponse {
        strategies,
        total,
        running: running_count,
    })
}

/// 특정 전략 상세 조회.
///
/// GET /api/v1/strategies/:id
pub async fn get_strategy(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<StrategyDetailResponse>, (StatusCode, Json<ApiError>)> {
    let engine = state.strategy_engine.read().await;

    // 상태 조회
    let status = engine
        .get_strategy_status(&id)
        .await
        .map_err(engine_error_to_response)?;

    // 설정 조회
    let config = engine
        .get_strategy_config(&id)
        .await
        .map_err(engine_error_to_response)?;

    // 전략 타입 조회
    let strategy_type = engine
        .get_strategy_type(&id)
        .await
        .map_err(engine_error_to_response)?;

    Ok(Json(StrategyDetailResponse {
        id,
        strategy_type,
        status,
        config,
    }))
}

/// 전략 시작.
///
/// POST /api/v1/strategies/:id/start
pub async fn start_strategy(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<StrategyActionResponse>, (StatusCode, Json<ApiError>)> {
    let engine = state.strategy_engine.read().await;

    // 전략 이름 가져오기 (브로드캐스트용)
    let strategy_name = engine
        .get_strategy_status(&id)
        .await
        .map(|s| s.name)
        .unwrap_or_else(|_| id.clone());

    match engine.start_strategy(&id).await {
        Ok(()) => {
            // WebSocket 브로드캐스트: 전략 시작 알림
            state.broadcast(ServerMessage::StrategyUpdate(StrategyUpdateData {
                strategy_id: id.clone(),
                name: strategy_name,
                running: true,
                event: "started".to_string(),
                data: None,
                timestamp: Utc::now().timestamp_millis(),
            }));

            Ok(Json(StrategyActionResponse {
                success: true,
                strategy_id: id.clone(),
                action: "start".to_string(),
                message: format!("Strategy '{}' started successfully", id),
            }))
        }
        Err(err) => Err(engine_error_to_response(err)),
    }
}

/// 전략 중지.
///
/// POST /api/v1/strategies/:id/stop
pub async fn stop_strategy(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<StrategyActionResponse>, (StatusCode, Json<ApiError>)> {
    let engine = state.strategy_engine.read().await;

    // 전략 이름 가져오기 (브로드캐스트용)
    let strategy_name = engine
        .get_strategy_status(&id)
        .await
        .map(|s| s.name)
        .unwrap_or_else(|_| id.clone());

    match engine.stop_strategy(&id).await {
        Ok(()) => {
            // WebSocket 브로드캐스트: 전략 중지 알림
            state.broadcast(ServerMessage::StrategyUpdate(StrategyUpdateData {
                strategy_id: id.clone(),
                name: strategy_name,
                running: false,
                event: "stopped".to_string(),
                data: None,
                timestamp: Utc::now().timestamp_millis(),
            }));

            Ok(Json(StrategyActionResponse {
                success: true,
                strategy_id: id.clone(),
                action: "stop".to_string(),
                message: format!("Strategy '{}' stopped successfully", id),
            }))
        }
        Err(err) => Err(engine_error_to_response(err)),
    }
}

/// 전략 설정 변경.
///
/// PUT /api/v1/strategies/:id/config
pub async fn update_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<UpdateConfigRequest>,
) -> Result<Json<StrategyActionResponse>, (StatusCode, Json<ApiError>)> {
    let engine = state.strategy_engine.read().await;

    // 전략 상태 가져오기 (브로드캐스트용)
    let (strategy_name, is_running) = engine
        .get_strategy_status(&id)
        .await
        .map(|s| (s.name, s.running))
        .unwrap_or_else(|_| (id.clone(), false));

    match engine.update_strategy_config(&id, request.config.clone()).await {
        Ok(()) => {
            // WebSocket 브로드캐스트: 설정 변경 알림
            state.broadcast(ServerMessage::StrategyUpdate(StrategyUpdateData {
                strategy_id: id.clone(),
                name: strategy_name,
                running: is_running,
                event: "config_updated".to_string(),
                data: Some(request.config),
                timestamp: Utc::now().timestamp_millis(),
            }));

            Ok(Json(StrategyActionResponse {
                success: true,
                strategy_id: id.clone(),
                action: "update_config".to_string(),
                message: format!("Strategy '{}' configuration updated successfully", id),
            }))
        }
        Err(err) => Err(engine_error_to_response(err)),
    }
}

/// 엔진 통계 조회.
///
/// GET /api/v1/strategies/stats
pub async fn get_engine_stats(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let engine = state.strategy_engine.read().await;
    let stats = engine.get_engine_stats().await;

    Json(EngineStatsResponse::from(stats))
}

// ==================== router ====================

/// 전략 관리 라우터 생성.
pub fn strategies_router() -> Router<Arc<AppState>> {
    Router::new()
        // 목록, 생성, 통계
        .route("/", get(list_strategies).post(create_strategy))
        .route("/stats", get(get_engine_stats))
        // 개별 전략 조작
        .route("/:id", get(get_strategy).delete(delete_strategy))
        .route("/:id/start", post(start_strategy))
        .route("/:id/stop", post(stop_strategy))
        .route("/:id/config", put(update_config))
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
    async fn test_list_strategies_empty() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/strategies", get(list_strategies))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/strategies")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let list: StrategiesListResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(list.total, 0);
        assert_eq!(list.running, 0);
        assert!(list.strategies.is_empty());
    }

    #[tokio::test]
    async fn test_get_strategy_not_found() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/strategies/:id", get(get_strategy))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/strategies/nonexistent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let error: ApiError = serde_json::from_slice(&body).unwrap();

        assert_eq!(error.code, "STRATEGY_NOT_FOUND");
    }

    #[tokio::test]
    async fn test_start_strategy_not_found() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/strategies/:id/start", post(start_strategy))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/strategies/nonexistent/start")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_engine_stats() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/strategies/stats", get(get_engine_stats))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/strategies/stats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let stats: EngineStatsResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(stats.total_strategies, 0);
        assert_eq!(stats.running_strategies, 0);
    }

    #[test]
    fn test_api_error_creation() {
        let error = ApiError::new("TEST_ERROR", "Test message");
        assert_eq!(error.code, "TEST_ERROR");
        assert_eq!(error.message, "Test message");
    }

    #[test]
    fn test_engine_stats_conversion() {
        let stats = EngineStats {
            total_strategies: 5,
            running_strategies: 2,
            total_signals_generated: 100,
            total_orders_filled: 50,
            total_market_data_processed: 1000,
        };

        let response: EngineStatsResponse = stats.into();
        assert_eq!(response.total_strategies, 5);
        assert_eq!(response.running_strategies, 2);
        assert_eq!(response.total_signals_generated, 100);
    }
}
