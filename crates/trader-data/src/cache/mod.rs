//! 캐싱 레이어.
//!
//! - Redis 캐시: 실시간 데이터 캐싱
//! - Historical 캐시: Yahoo Finance 캔들 데이터 캐싱
//! - Fundamental 캐시: Yahoo Finance 펀더멘털 데이터 수집

pub mod fundamental;
pub mod historical;

pub use crate::storage::redis::{CacheStats, MetricsCache, RedisCache, RedisConfig};
pub use fundamental::{FetchResult, FundamentalData, FundamentalFetcher};
pub use historical::{CacheStats as HistoricalCacheStats, CachedHistoricalDataProvider};
