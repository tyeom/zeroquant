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
//! - `/api/v1/journal` - 매매일지 (체결 내역, 포지션 현황, 손익 분석)
//! - `/api/v1/screening` - 종목 스크리닝 (Fundamental + 기술적 필터)

pub mod analytics;
pub mod backtest;
pub mod backtest_results;
pub mod credentials;
pub mod dataset;
pub mod equity_history;
pub mod health;
pub mod journal;
pub mod market;
pub mod ml;
#[cfg(feature = "notifications")]
pub mod notifications;
pub mod orders;
pub mod patterns;
pub mod portfolio;
pub mod positions;
pub mod screening;
pub mod simulation;
pub mod strategies;

pub use analytics::{analytics_router, PerformanceResponse, EquityCurveResponse, ChartResponse, MonthlyReturnsResponse};
pub use backtest::{backtest_router, BacktestRunRequest, BacktestRunResponse, BacktestStrategiesResponse, BacktestMultiRunRequest, BacktestMultiRunResponse};
pub use backtest_results::{backtest_results_router, BacktestResultResponse, ListResultsResponse, SaveBacktestResultRequest};
pub use credentials::{credentials_router, ExchangeCredentialResponse, TelegramSettingsResponse, SupportedExchangesResponse, EncryptedCredentials};
pub use dataset::{dataset_router, DatasetListResponse, DatasetSummary, FetchDatasetRequest};
pub use health::{health_router, HealthResponse, ComponentHealth, ComponentStatus};
pub use journal::{journal_router, JournalPositionsResponse, ExecutionsListResponse, PnLSummaryResponse, SyncResponse};
pub use market::{market_router, MarketStatusResponse};
pub use ml::{ml_router, TrainingJob, TrainedModel, ModelType, TrainingStatus};
#[cfg(feature = "notifications")]
pub use notifications::{notifications_router, TelegramTestRequest, TelegramTestResponse};
pub use orders::{orders_router, OrdersListResponse, OrderResponse, CancelOrderResponse};
pub use patterns::{patterns_router, PatternTypesResponse, CandlestickPatternsResponse, ChartPatternsResponse};
pub use portfolio::{portfolio_router, PortfolioSummaryResponse, BalanceResponse, HoldingsResponse};
pub use positions::{positions_router, PositionsListResponse, PositionResponse, PositionSummaryResponse};
pub use screening::{screening_router, ScreeningRequest, ScreeningResponse, MomentumResponse};
pub use simulation::{simulation_router, SimulationStartRequest, SimulationStatusResponse, SimulationOrderRequest};
pub use strategies::{strategies_router, ApiError, StrategiesListResponse, StrategyDetailResponse};

use axum::Router;
use std::sync::Arc;

use crate::state::AppState;

/// 전체 API 라우터 생성.
///
/// 모든 서브 라우터를 조합하여 하나의 라우터로 반환합니다.
///
/// # Feature Flags
/// - `notifications`: 알림 라우터 활성화 (`/api/v1/notifications`)
pub fn create_api_router() -> Router<Arc<AppState>> {
    let router = Router::new()
        // 헬스 체크 엔드포인트
        .nest("/health", health_router())
        // API v1 엔드포인트
        .nest("/api/v1/strategies", strategies_router())
        .nest("/api/v1/orders", orders_router())
        .nest("/api/v1/positions", positions_router())
        .nest("/api/v1/backtest", backtest_router())
        .nest("/api/v1/backtest/results", backtest_results_router())
        .nest("/api/v1/simulation", simulation_router())
        .nest("/api/v1/analytics", analytics_router())
        .nest("/api/v1/patterns", patterns_router())
        .nest("/api/v1/portfolio", portfolio_router())
        .nest("/api/v1/market", market_router())
        .nest("/api/v1/credentials", credentials_router())
        .nest("/api/v1/ml", ml_router())
        .nest("/api/v1/dataset", dataset_router())
        .nest("/api/v1/journal", journal_router())
        .nest("/api/v1/screening", screening_router());

    // Feature: notifications - 텔레그램/이메일 알림
    #[cfg(feature = "notifications")]
    let router = router.nest("/api/v1/notifications", notifications_router());

    router
}
