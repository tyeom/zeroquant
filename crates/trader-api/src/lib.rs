//! REST API 및 WebSocket 서버.
//!
//! 이 크레이트는 다음을 제공합니다:
//! - Axum 기반 REST API
//! - 실시간 업데이트를 위한 WebSocket 서버
//! - JWT 인증
//! - 헬스 체크 엔드포인트
//! - Prometheus 메트릭
//! - 백그라운드 데이터 수집 태스크
//!
//! # 모듈 구성
//!
//! - [`state`]: 애플리케이션 공유 상태 (AppState)
//! - [`routes`]: REST API 엔드포인트
//! - [`auth`]: JWT 인증 및 권한 관리
//! - [`websocket`]: 실시간 WebSocket 서버
//! - [`metrics`]: Prometheus 메트릭 수집
//! - [`middleware`]: HTTP 미들웨어
//! - [`openapi`]: OpenAPI 문서 및 Swagger UI
//! - [`tasks`]: 백그라운드 태스크 (Fundamental 데이터 수집 등)

pub mod auth;
pub mod error;
pub mod metrics;
pub mod middleware;
pub mod monitoring;
pub mod openapi;
pub mod repository;
pub mod routes;
pub mod state;
pub mod tasks;
pub mod types;
pub mod utils;
pub mod websocket;

pub use auth::{Claims, Role, Permission, JwtAuth, JwtAuthError, hash_password, verify_password};
pub use error::{ApiErrorResponse, ApiResult};
pub use monitoring::{
    global_tracker, init_global_tracker, ErrorTracker, ErrorTrackerConfig,
    ErrorRecord, ErrorRecordBuilder, ErrorSeverity, ErrorCategory, ErrorStats, SourceLocation,
};
pub use routes::*;
pub use state::AppState;
pub use tasks::{start_fundamental_collector, FundamentalCollectorConfig};
pub use websocket::{
    websocket_handler, websocket_router, ClientMessage, ServerMessage, WsError,
    Subscription, SubscriptionManager, subscriptions::create_subscription_manager,
    handler::WsState,
};
pub use metrics::setup_metrics_recorder;
pub use middleware::metrics_layer;
pub use types::StrategyType;

#[cfg(any(test, feature = "test-utils"))]
pub use state::create_test_state;
