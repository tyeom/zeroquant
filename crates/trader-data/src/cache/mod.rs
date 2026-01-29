//! 캐싱 레이어.
//!
//! - Redis 캐시: 실시간 데이터 캐싱
//! - Historical 캐시: Yahoo Finance 캔들 데이터 캐싱

pub mod historical;

pub use crate::storage::redis::{CacheStats, MetricsCache, RedisCache, RedisConfig};
pub use historical::{CachedHistoricalDataProvider, CacheStats as HistoricalCacheStats};
