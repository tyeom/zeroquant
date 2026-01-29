//! 거래소 연결 및 시장 데이터 처리.
//!
//! 이 크레이트는 다음을 제공합니다:
//! - Exchange trait: 통합 거래소 인터페이스
//! - Binance 커넥터 (REST + WebSocket)
//! - 시뮬레이션 거래소 (백테스팅 및 모의투자용)
//! - 시장 데이터 정규화
//! - Rate limiting 및 에러 처리
//! - Circuit breaker: 장애 허용을 위한 회로 차단기

pub mod circuit_breaker;
pub mod connector;
pub mod error;
pub mod historical;
pub mod simulated;
pub mod stream;
pub mod traits;
pub mod websocket;
pub mod yahoo;

pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerMetrics, CircuitState};
pub use error::*;
pub use historical::{HistoricalDataProvider, UnifiedHistoricalProvider};
pub use yahoo::YahooFinanceProvider;
pub use simulated::{
    DataFeed, DataFeedConfig, FillType, MatchingEngine, OrderMatch, SimulatedConfig,
    SimulatedExchange, SimulatedMarketStream, SimulatedUserStream,
};
pub use stream::{KisKrMarketStream, KisUsMarketStream, UnifiedMarketStream};
pub use traits::*;
