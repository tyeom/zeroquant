//! 트레이딩 전략 엔진 및 플러그인 시스템.
//!
//! 이 크레이트가 제공하는 기능:
//! - 트레이딩 전략 구현을 위한 Strategy trait
//! - 동적 전략 로딩을 위한 플러그인 로더
//! - 전략 실행 엔진
//! - 내장 전략 (그리드 트레이딩, RSI 평균 회귀)
//!
//! # 예제
//!
//! ```rust,ignore
//! use trader_strategy::{StrategyEngine, EngineConfig, GridStrategy};
//! use serde_json::json;
//!
//! #[tokio::main]
//! async fn main() {
//!     let engine = StrategyEngine::new(EngineConfig::default());
//!
//!     // 그리드 트레이딩 전략 등록
//!     let strategy = Box::new(GridStrategy::new());
//!     let config = json!({
//!         "symbol": "BTC/USDT",
//!         "grid_levels": 10,
//!         "grid_spacing_pct": 1.0,
//!         "amount_per_level": "100"
//!     });
//!
//!     engine.register_strategy("btc_grid", strategy, config).await.unwrap();
//!     engine.start_strategy("btc_grid").await.unwrap();
//! }
//! ```

pub mod engine;
pub mod plugin;
pub mod strategies;
pub mod traits;

// 주요 타입 재내보내기
pub use engine::{EngineConfig, EngineError, EngineStats, StrategyEngine, StrategyStats, StrategyStatus};
pub use plugin::{BuiltinStrategyFactory, LoaderConfig, PluginError, PluginLoader, PluginMetadata};
pub use strategies::{GridConfig, GridStats, GridStrategy, RsiConfig, RsiStats, RsiStrategy};
pub use traits::{Strategy, StrategyMetadata};
