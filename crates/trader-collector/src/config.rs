//! 환경변수 기반 설정 모듈.

use crate::Result;
use std::time::Duration;

/// Collector 전체 설정
#[derive(Debug, Clone)]
pub struct CollectorConfig {
    /// 데이터베이스 URL
    pub database_url: String,
    /// 심볼 동기화 설정
    pub symbol_sync: SymbolSyncConfig,
    /// OHLCV 수집 설정
    pub ohlcv_collect: OhlcvCollectConfig,
    /// Fundamental 수집 설정
    pub fundamental_collect: FundamentalCollectConfig,
    /// 데몬 모드 설정
    pub daemon: DaemonConfig,
}

/// 심볼 동기화 설정
#[derive(Debug, Clone)]
pub struct SymbolSyncConfig {
    /// 최소 심볼 수 (이 수 이하일 때만 동기화 실행)
    pub min_symbol_count: i64,
    /// KRX 동기화 활성화
    pub enable_krx: bool,
    /// Binance 동기화 활성화
    pub enable_binance: bool,
    /// Yahoo 동기화 활성화
    pub enable_yahoo: bool,
    /// Yahoo 최대 수집 종목 수
    pub yahoo_max_symbols: usize,
}

/// OHLCV 수집 설정
#[derive(Debug, Clone)]
pub struct OhlcvCollectConfig {
    /// 배치당 심볼 수
    pub batch_size: i64,
    /// 갱신 기준 일수 (마지막 수집 후 N일 경과 시 재수집)
    pub stale_days: i64,
    /// API 요청 간 딜레이 (밀리초)
    pub request_delay_ms: u64,
    /// 수집 시작 날짜 (YYYYMMDD)
    pub start_date: Option<String>,
    /// 수집 종료 날짜 (YYYYMMDD)
    pub end_date: Option<String>,
}

/// Fundamental 수집 설정
#[derive(Debug, Clone)]
pub struct FundamentalCollectConfig {
    /// 배치당 심볼 수
    pub batch_size: i64,
    /// 갱신 기준 일수 (기본: 7일)
    pub stale_days: i64,
    /// API 요청 간 딜레이 (밀리초)
    pub request_delay_ms: u64,
    /// OHLCV 데이터 함께 수집 여부
    pub include_ohlcv: bool,
}

/// 데몬 모드 설정
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// 워크플로우 실행 주기 (분 단위)
    pub interval_minutes: u64,
}

impl CollectorConfig {
    /// 환경변수에서 설정 로드
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let database_url = std::env::var("DATABASE_URL").map_err(|_| {
            crate::error::CollectorError::Config(
                "DATABASE_URL 환경변수가 설정되지 않았습니다".to_string(),
            )
        })?;

        Ok(Self {
            database_url,
            symbol_sync: SymbolSyncConfig {
                min_symbol_count: env_var_parse("SYMBOL_SYNC_MIN_COUNT", 100),
                enable_krx: env_var_bool("SYMBOL_SYNC_KRX", true),
                enable_binance: env_var_bool("SYMBOL_SYNC_BINANCE", false),
                enable_yahoo: env_var_bool("SYMBOL_SYNC_YAHOO", true),
                yahoo_max_symbols: env_var_parse("SYMBOL_SYNC_YAHOO_MAX", 500),
            },
            ohlcv_collect: OhlcvCollectConfig {
                batch_size: env_var_parse("OHLCV_BATCH_SIZE", 50),
                stale_days: env_var_parse("OHLCV_STALE_DAYS", 1),
                request_delay_ms: env_var_parse("OHLCV_REQUEST_DELAY_MS", 500),
                start_date: std::env::var("OHLCV_START_DATE").ok(),
                end_date: std::env::var("OHLCV_END_DATE").ok(),
            },
            fundamental_collect: FundamentalCollectConfig {
                batch_size: env_var_parse("FUNDAMENTAL_BATCH_SIZE", 50),
                stale_days: env_var_parse("FUNDAMENTAL_STALE_DAYS", 7),
                request_delay_ms: env_var_parse("FUNDAMENTAL_REQUEST_DELAY_MS", 2000),
                include_ohlcv: env_var_bool("FUNDAMENTAL_INCLUDE_OHLCV", true),
            },
            daemon: DaemonConfig {
                interval_minutes: env_var_parse("DAEMON_INTERVAL_MINUTES", 60),
            },
        })
    }
}

impl OhlcvCollectConfig {
    /// API 요청 간 딜레이를 Duration으로 반환
    pub fn request_delay(&self) -> Duration {
        Duration::from_millis(self.request_delay_ms)
    }
}

impl FundamentalCollectConfig {
    /// API 요청 간 딜레이를 Duration으로 반환
    pub fn request_delay(&self) -> Duration {
        Duration::from_millis(self.request_delay_ms)
    }
}

impl DaemonConfig {
    /// 워크플로우 실행 주기를 Duration으로 반환
    pub fn interval(&self) -> Duration {
        Duration::from_secs(self.interval_minutes * 60)
    }
}

/// 환경변수에서 값을 파싱 (실패 시 기본값 사용)
fn env_var_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// 환경변수에서 bool 값 파싱
fn env_var_bool(key: &str, default: bool) -> bool {
    std::env::var(key)
        .map(|v| v == "true" || v == "1")
        .unwrap_or(default)
}
