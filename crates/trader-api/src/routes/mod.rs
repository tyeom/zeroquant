//! API 라우트.
//!
//! 모든 REST API 엔드포인트를 정의하고 라우터를 구성합니다.
//!
//! # 라우트 구조
//!
//! - `/health` - 헬스 체크 (liveness)
//! - `/health/ready` - 상세 헬스 체크 (readiness)
//! - `/api/v1/strategies` - 전략 관리
//! - `/api/v1/orders` - 주문 관리
//! - `/api/v1/positions` - 포지션 관리
//! - `/api/v1/notifications` - 알림 설정
//! - `/api/v1/backtest` - 백테스트 실행
//! - `/api/v1/analytics` - 포트폴리오 분석
//! - `/api/v1/patterns` - 패턴 인식 (캔들스틱/차트)
//! - `/api/v1/portfolio` - 포트폴리오 요약/잔고/보유종목
//! - `/api/v1/market` - 시장 상태
//! - `/api/v1/credentials` - 자격증명 관리 (API 키, 텔레그램 설정)
//! - `/api/v1/ml` - ML 훈련 관리

pub mod analytics;
pub mod backtest;
pub mod credentials;
pub mod equity_history;
pub mod health;
pub mod market;
pub mod ml;
pub mod notifications;
pub mod orders;
pub mod patterns;
pub mod portfolio;
pub mod positions;
pub mod simulation;
pub mod strategies;

pub use analytics::{analytics_router, PerformanceResponse, EquityCurveResponse, ChartResponse, MonthlyReturnsResponse};
pub use backtest::{backtest_router, BacktestRunRequest, BacktestRunResponse, BacktestStrategiesResponse, BacktestMultiRunRequest, BacktestMultiRunResponse};
pub use credentials::{credentials_router, ExchangeCredentialResponse, TelegramSettingsResponse, SupportedExchangesResponse, EncryptedCredentials};
pub use health::{health_router, HealthResponse, ComponentHealth, ComponentStatus};
pub use market::{market_router, MarketStatusResponse};
pub use ml::{ml_router, TrainingJob, TrainedModel, ModelType, TrainingStatus};
pub use notifications::{notifications_router, TelegramTestRequest, TelegramTestResponse};
pub use orders::{orders_router, OrdersListResponse, OrderResponse, CancelOrderResponse};
pub use patterns::{patterns_router, PatternTypesResponse, CandlestickPatternsResponse, ChartPatternsResponse};
pub use portfolio::{portfolio_router, PortfolioSummaryResponse, BalanceResponse, HoldingsResponse};
pub use positions::{positions_router, PositionsListResponse, PositionResponse, PositionSummaryResponse};
pub use simulation::{simulation_router, SimulationStartRequest, SimulationStatusResponse, SimulationOrderRequest};
pub use strategies::{strategies_router, ApiError, StrategiesListResponse, StrategyDetailResponse};

use axum::Router;
use std::sync::Arc;

use crate::state::AppState;

/// 전체 API 라우터 생성.
///
/// 모든 서브 라우터를 조합하여 하나의 라우터로 반환합니다.
pub fn create_api_router() -> Router<Arc<AppState>> {
    Router::new()
        // 헬스 체크 엔드포인트
        .nest("/health", health_router())
        // API v1 엔드포인트
        .nest("/api/v1/strategies", strategies_router())
        .nest("/api/v1/orders", orders_router())
        .nest("/api/v1/positions", positions_router())
        .nest("/api/v1/notifications", notifications_router())
        .nest("/api/v1/backtest", backtest_router())
        .nest("/api/v1/simulation", simulation_router())
        .nest("/api/v1/analytics", analytics_router())
        .nest("/api/v1/patterns", patterns_router())
        .nest("/api/v1/portfolio", portfolio_router())
        .nest("/api/v1/market", market_router())
        .nest("/api/v1/credentials", credentials_router())
        .nest("/api/v1/ml", ml_router())
}
