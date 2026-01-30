//! 데이터 관리 및 저장.
//!
//! 이 crate는 다음을 제공합니다:
//! - 실시간 및 과거 데이터를 위한 데이터 관리자
//! - TimescaleDB 저장소
//! - Redis 캐싱
//! - OHLCV 캔들 데이터 캐싱 (증분 업데이트 지원)
//! - 데이터 가져오기 유틸리티

pub mod cache;
pub mod error;
pub mod manager;
pub mod provider;
pub mod storage;

pub use error::{DataError, Result};
pub use manager::*;

// 저장소 타입 재내보내기
pub use storage::redis::{CacheStats, MetricsCache, RedisCache, RedisConfig};
pub use storage::timescale::{
    Database, DatabaseConfig, KlineRecord, KlineRepository, OrderRecord, OrderRepository,
    PositionRecord, PositionRepository, SymbolRecord, SymbolRepository, TradeRecord,
    TradeRepository, TradeTickRecord, TradeTickRepository,
};

// OHLCV 캔들 캐시 재내보내기
pub use storage::ohlcv::{OhlcvCache, OhlcvRecord, OhlcvMetadataRecord};
pub use cache::historical::{CachedHistoricalDataProvider, CacheStats as HistoricalCacheStats};

// KRX 데이터 소스 재내보내기
pub use storage::krx::KrxDataSource;

// 심볼 정보 Provider 재내보내기
pub use provider::{
    BinanceSymbolProvider, CompositeSymbolProvider, KrxSymbolProvider, SymbolInfoProvider,
    SymbolMetadata, SymbolResolver,
};
