//! 캐시 기반 과거 데이터 제공자.
//!
//! Yahoo Finance와 DB 캐시를 통합하여 효율적인 데이터 접근을 제공합니다.
//!
//! # 주요 기능
//!
//! - **동시성 제어**: 같은 심볼+타임프레임 중복 요청 방지
//! - **시장 시간 체크**: 마감 후 불필요한 API 호출 방지
//! - **갭 감지**: 누락된 캔들 자동 감지
//! - **증분 업데이트**: 새 데이터만 가져와 캐시
//!
//! # 동작 흐름
//!
//! ```text
//! 요청 (symbol, timeframe, limit)
//!         │
//!         ▼
//! ┌───────────────────┐
//! │ 1. 동시성 Lock 획득 │ ← 같은 심볼+TF는 하나만 처리
//! └─────────┬─────────┘
//!           │
//! ┌─────────▼─────────┐
//! │ 2. 시장 시간 체크   │ ← 마감 후 1시간 이내인가?
//! └─────────┬─────────┘
//!           │
//!     ┌─────┴─────┐
//!     │ 캐시 충분? │
//!     └─────┬─────┘
//!       YES │ NO
//!           │   │
//!           │   ▼
//!           │ ┌─────────────────────┐
//!           │ │ 3. Yahoo Finance    │
//!           │ │    증분 업데이트     │
//!           │ └──────────┬──────────┘
//!           │            │
//!           │   ┌────────▼────────┐
//!           │   │ 4. 갭 감지/경고  │
//!           │   └────────┬────────┘
//!           │            │
//!           ▼            ▼
//!     ┌─────────────────────┐
//!     │ 5. 캐시에서 반환     │
//!     └─────────────────────┘
//! ```

use crate::error::{DataError, Result};
use crate::provider::krx_api::KrxApiClient;
use crate::provider::SymbolResolver;
use crate::storage::krx::KrxDataSource;
use crate::storage::ohlcv::{timeframe_to_string, OhlcvCache};
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Timelike, Utc, Weekday};
use chrono_tz::Tz;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use time::OffsetDateTime;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};
use trader_core::{CredentialEncryptor, Kline, Timeframe};

// =============================================================================
// 상장폐지 감지 상수 및 함수
// =============================================================================

/// 상장폐지 오류 패턴.
/// Yahoo Finance 및 기타 데이터 소스에서 반환하는 상장폐지 관련 오류 메시지 패턴.
pub const DELISTED_ERROR_PATTERNS: &[&str] = &[
    "symbol may be delisted",
    "No data found",
    "Not Found",
    "delisted",
    "invalid symbol",
    "No timezone found",
    "status code: 404",
];

/// 오류 메시지가 상장폐지 관련인지 확인.
pub fn is_delisted_error(error_message: &str) -> bool {
    let lower = error_message.to_lowercase();
    DELISTED_ERROR_PATTERNS
        .iter()
        .any(|p| lower.contains(&p.to_lowercase()))
}

/// 거래소 API Rate Limit 설정.
pub struct ExchangeRateLimits {
    /// 요청 간 최소 대기 시간 (밀리초)
    pub min_delay_ms: u64,
    /// 분당 최대 요청 수
    pub max_requests_per_minute: u32,
}

impl Default for ExchangeRateLimits {
    fn default() -> Self {
        Self {
            min_delay_ms: 500,           // 500ms 기본 딜레이
            max_requests_per_minute: 10, // 분당 10회
        }
    }
}

/// 심볼+타임프레임별 페칭 상태를 추적하는 Lock 맵.
type FetchLockMap = Arc<RwLock<HashMap<String, Arc<RwLock<()>>>>>;

/// 캐시 기반 과거 데이터 제공자.
///
/// 요청 기반 자동 캐싱과 증분 업데이트를 제공합니다.
/// 모든 심볼은 canonical 형식으로 처리되며, SymbolResolver를 통해
/// 각 데이터 소스에 맞는 형식으로 변환됩니다.
pub struct CachedHistoricalDataProvider {
    cache: OhlcvCache,
    pool: PgPool,
    /// 심볼 변환 서비스
    symbol_resolver: SymbolResolver,
    /// 캐시 유효 기간 (이 시간 이내면 신선하다고 간주)
    cache_freshness: Duration,
    /// 동시성 제어를 위한 Lock 맵
    fetch_locks: FetchLockMap,
    /// KRX 정보데이터시스템 클라이언트 (재사용)
    krx_data_source: KrxDataSource,
    /// KRX Open API 클라이언트 (lazy init, credential 기반)
    krx_api_client: tokio::sync::OnceCell<Option<KrxApiClient>>,
    /// 암호화 키 (credential 복호화용)
    encryption_key: Option<String>,
}

impl CachedHistoricalDataProvider {
    /// 새로운 캐시 기반 제공자 생성.
    pub fn new(pool: PgPool) -> Self {
        // 환경변수에서 암호화 키 로드 (한 번만)
        let encryption_key = std::env::var("ENCRYPTION_MASTER_KEY").ok();
        if encryption_key.is_some() {
            debug!("ENCRYPTION_MASTER_KEY 로드됨 - KRX Open API 사용 가능");
        }

        Self {
            cache: OhlcvCache::new(pool.clone()),
            symbol_resolver: SymbolResolver::new(pool.clone()),
            pool,
            cache_freshness: Duration::minutes(5),
            fetch_locks: Arc::new(RwLock::new(HashMap::new())),
            krx_data_source: KrxDataSource::new(),
            krx_api_client: tokio::sync::OnceCell::new(),
            encryption_key,
        }
    }

    /// 캐시 유효 기간 설정.
    pub fn with_freshness(mut self, duration: Duration) -> Self {
        self.cache_freshness = duration;
        self
    }

    /// 캔들 데이터 조회 (캐시 우선, 증분 업데이트).
    ///
    /// # 인자
    /// - `symbol`: canonical 심볼 (예: "005930", "AAPL", "BTC/USDT")
    ///
    /// 내부적으로 SymbolResolver를 통해 데이터 소스에 맞는 심볼로 변환합니다.
    #[instrument(skip(self))]
    pub async fn get_klines(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<Vec<Kline>> {
        // SymbolResolver를 통해 데이터 소스 심볼 조회
        let (ticker, _yahoo_symbol, _market) = self.resolve_symbol(symbol).await?;
        let lock_key = format!("{}:{}", ticker, timeframe_to_string(timeframe));

        // 1. 동시성 제어: Lock 획득
        let lock = self.get_or_create_lock(&lock_key).await;
        let _guard = lock.write().await;

        // 2. 캐시 상태 확인
        let cached_count = self.cache.get_cached_count(&ticker, timeframe).await?;
        let last_cached_time = self.cache.get_last_cached_time(&ticker, timeframe).await?;

        // 3. 업데이트 필요 여부 판단 (시장 시간 고려)
        let needs_update = self.should_update(
            &ticker,
            timeframe,
            cached_count as usize,
            limit,
            last_cached_time,
        );

        // 4. 필요시 데이터 소스에서 새 데이터 가져오기
        if needs_update {
            debug!(
                canonical = %symbol,
                ticker = %ticker,
                timeframe = %timeframe_to_string(timeframe),
                cached = cached_count,
                requested = limit,
                "캐시 업데이트 시작"
            );

            // 원본 심볼로 데이터 소스 선택, ticker로 캐시 저장
            match self
                .fetch_and_cache(symbol, &ticker, timeframe, limit, last_cached_time)
                .await
            {
                Ok(fetched) => {
                    info!(
                        canonical = %symbol,
                        ticker = %ticker,
                        fetched = fetched,
                        "데이터 캐시 완료"
                    );
                }
                Err(e) => {
                    warn!(
                        canonical = %symbol,
                        ticker = %ticker,
                        error = %e,
                        "데이터 가져오기 실패, 캐시 데이터 사용"
                    );
                }
            }
        }

        // 5. 갭 감지
        self.detect_and_warn_gaps(&ticker, timeframe, limit).await;

        // 6. 캐시에서 데이터 조회
        let records = self
            .cache
            .get_cached_klines(&ticker, timeframe, limit)
            .await?;

        // 7. canonical 심볼로 Kline 변환
        // Symbol 생성자를 통해 country 필드 자동 추론
        let klines: Vec<Kline> = records
            .into_iter()
            .map(|kline| Kline {
                ticker: symbol.to_string(),
                ..kline
            })
            .collect();

        debug!(
            canonical = %symbol,
            ticker = %ticker,
            returned = klines.len(),
            "캔들 데이터 반환"
        );

        Ok(klines)
    }

    /// 심볼 정보 조회.
    ///
    /// DB의 symbol_info 테이블에서 조회:
    /// - ticker: 저장/조회 키 (모든 곳에서 사용)
    /// - yahoo_symbol: Yahoo Finance API 호출 시에만 사용
    ///
    /// 반환: (ticker, yahoo_symbol, market)
    async fn resolve_symbol(&self, canonical: &str) -> Result<(String, Option<String>, String)> {
        // DB에서 심볼 정보 조회 (필수)
        let info = self
            .symbol_resolver
            .get_symbol_info(canonical)
            .await
            .map_err(|e| DataError::QueryError(format!("DB 조회 실패: {}", e)))?
            .ok_or_else(|| {
                DataError::NotFound(format!("심볼을 찾을 수 없습니다: {}", canonical))
            })?;

        Ok((
            info.ticker.clone(),
            info.yahoo_symbol.clone(),
            info.market.clone(),
        ))
    }

    /// 날짜 범위로 캔들 데이터 조회.
    ///
    /// # 인자
    /// - `symbol`: 심볼 (예: 005930, AAPL)
    /// - `timeframe`: 타임프레임
    /// - `start_date`: 시작 날짜
    /// - `end_date`: 종료 날짜
    ///
    /// # 반환
    /// 지정된 기간의 캔들 데이터 (캐시에 저장됨)
    ///
    /// # 인자
    /// - `symbol`: canonical 심볼 (예: "005930", "AAPL", "BTC/USDT")
    #[instrument(skip(self))]
    pub async fn get_klines_range(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<Kline>> {
        // SymbolResolver를 통해 데이터 소스 심볼 조회
        let (ticker, _yahoo_symbol, _market) = self.resolve_symbol(symbol).await?;
        let lock_key = format!("{}:{}:range", ticker, timeframe_to_string(timeframe));

        debug!(
            canonical = %symbol,
            ticker = %ticker,
            timeframe = %timeframe_to_string(timeframe),
            start = %start_date,
            end = %end_date,
            "날짜 범위 데이터 조회 요청"
        );

        // 1. 동시성 제어: Lock 획득
        let lock = self.get_or_create_lock(&lock_key).await;
        let _guard = lock.write().await;

        // 2. 캐시에서 먼저 조회
        let start_dt = Utc.from_utc_datetime(&start_date.and_hms_opt(0, 0, 0).unwrap());
        let end_dt = Utc.from_utc_datetime(&end_date.and_hms_opt(23, 59, 59).unwrap());

        let cached_klines = self
            .cache
            .get_cached_klines_range(&ticker, timeframe, start_dt, end_dt)
            .await?;

        // 3. 캐시 데이터로 충분한지 확인 (요청 기간의 80% 이상 커버 시 캐시만 사용)
        let requested_days = (end_date - start_date).num_days() as usize;
        let cached_days = cached_klines.len();
        let coverage_ratio = if requested_days > 0 {
            cached_days as f64 / (requested_days as f64 * 5.0 / 7.0) // 거래일 기준
        } else {
            0.0
        };

        if coverage_ratio >= 0.8 && !cached_klines.is_empty() {
            debug!(
                canonical = %symbol,
                cached = cached_days,
                coverage = format!("{:.1}%", coverage_ratio * 100.0),
                "캐시 데이터 사용 (충분한 커버리지)"
            );
            // canonical 심볼로 변환하여 반환
            let klines: Vec<Kline> = cached_klines
                .into_iter()
                .map(|k| Kline {
                    ticker: symbol.to_string(),
                    ..k
                })
                .collect();
            return Ok(klines);
        }

        // 4. 캐시 메타데이터 확인하여 누락 구간만 요청
        let metadata = self.cache.get_cache_metadata(&ticker, timeframe).await?;

        // 메타데이터는 있지만 실제 데이터가 없으면 메타데이터 정리 (비정상 상태 복구)
        if metadata.is_some() && cached_klines.is_empty() {
            warn!(
                canonical = %symbol,
                ticker = %ticker,
                "메타데이터-캐시 불일치 감지: 메타데이터 정리"
            );
            let _ = sqlx::query("DELETE FROM ohlcv_metadata WHERE symbol = $1")
                .bind(&ticker)
                .execute(&self.pool)
                .await;
        }

        let (fetch_start, fetch_end) = if let Some(meta) = &metadata {
            // 캐시된 범위 확인
            let cached_start = meta.first_cached_time.map(|t| t.date_naive());
            let cached_end = meta.last_cached_time.map(|t| t.date_naive());

            match (cached_start, cached_end) {
                (Some(cs), Some(ce))
                    if cs <= start_date && ce >= end_date && !cached_klines.is_empty() =>
                {
                    // 전체 범위가 캐시됨 && 실제 데이터도 있음 - 캐시만 사용
                    debug!(canonical = %symbol, cached_count = cached_klines.len(), "전체 범위 캐시됨, API 호출 스킵");
                    let klines: Vec<Kline> = cached_klines
                        .into_iter()
                        .map(|k| Kline {
                            ticker: symbol.to_string(),
                            ..k
                        })
                        .collect();
                    return Ok(klines);
                }
                (Some(cs), Some(ce)) => {
                    // 일부만 캐시됨 - 누락 구간 계산
                    let fetch_start = if start_date < cs { start_date } else { ce };
                    let fetch_end = if end_date > ce { end_date } else { cs };
                    debug!(
                        canonical = %symbol,
                        cached_range = format!("{} ~ {}", cs, ce),
                        fetch_range = format!("{} ~ {}", fetch_start, fetch_end),
                        "누락 구간만 요청"
                    );
                    (fetch_start, fetch_end)
                }
                _ => (start_date, end_date),
            }
        } else {
            (start_date, end_date)
        };

        // 5. 외부 데이터 소스에서 누락 구간만 가져와 캐시
        let raw_klines = if is_pure_korean_stock_code(symbol) {
            debug!(canonical = symbol, fetch_start = %fetch_start, fetch_end = %fetch_end, "KRX 데이터 소스 시도 (누락 구간)");
            match self
                .fetch_from_krx_range(symbol, timeframe, fetch_start, fetch_end)
                .await
            {
                Ok(data) if !data.is_empty() => {
                    debug!(
                        canonical = symbol,
                        count = data.len(),
                        "KRX 날짜 범위 데이터 성공"
                    );
                    data
                }
                Ok(_) | Err(_) => {
                    warn!(canonical = symbol, ticker = %ticker, "KRX 실패, Yahoo Finance Fallback");
                    let provider =
                        YahooProviderWrapper::new(SymbolResolver::new(self.pool.clone()))?;
                    provider
                        .get_klines_range(&ticker, timeframe, fetch_start, fetch_end)
                        .await?
                }
            }
        } else {
            debug!(ticker = %ticker, fetch_start = %fetch_start, fetch_end = %fetch_end, "Yahoo Finance 누락 구간 조회");
            let provider = YahooProviderWrapper::new(SymbolResolver::new(self.pool.clone()))?;
            provider
                .get_klines_range(&ticker, timeframe, fetch_start, fetch_end)
                .await?
        };

        if raw_klines.is_empty() {
            info!(canonical = %symbol, ticker = %ticker, "날짜 범위에 데이터 없음");
            return Ok(Vec::new());
        }

        // 3. 캐시에 저장
        let saved = self
            .batch_insert_klines(&ticker, timeframe, &raw_klines)
            .await?;
        info!(
            canonical = %symbol,
            ticker = %ticker,
            fetched = raw_klines.len(),
            saved = saved,
            "날짜 범위 데이터 캐시 완료"
        );

        // 4. canonical 심볼로 Kline 변환
        // Symbol 생성자를 통해 country 필드 자동 추론
        let klines: Vec<Kline> = raw_klines
            .into_iter()
            .map(|kline| Kline {
                ticker: symbol.to_string(),
                ..kline
            })
            .collect();

        Ok(klines)
    }

    /// KRX에서 날짜 범위로 데이터 가져오기.
    ///
    /// KRX Open API (암호화 credential)를 우선 사용하고,
    /// 실패 시 KRX 정보데이터시스템으로 fallback합니다.
    async fn fetch_from_krx_range(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<Kline>> {
        // KRX는 일봉만 지원
        if timeframe != Timeframe::D1 {
            warn!(
                symbol = symbol,
                timeframe = %timeframe_to_string(timeframe),
                "KRX는 일봉(1d)만 지원합니다."
            );
        }

        let start_str = start_date.format("%Y%m%d").to_string();
        let end_str = end_date.format("%Y%m%d").to_string();

        // 1. KRX Open API 시도 (암호화된 credential 사용)
        if let Some(klines) = self.try_krx_api(symbol, &start_str, &end_str).await {
            debug!(
                symbol = symbol,
                start = %start_str,
                end = %end_str,
                count = klines.len(),
                "KRX Open API 데이터 가져오기 완료"
            );
            return Ok(klines);
        }

        // 2. Fallback: KRX 정보데이터시스템 (캐시된 클라이언트 사용)
        debug!(
            symbol = symbol,
            "KRX Open API 실패, 정보데이터시스템으로 fallback"
        );
        let klines = self
            .krx_data_source
            .get_ohlcv(symbol, &start_str, &end_str)
            .await?;

        debug!(
            symbol = symbol,
            start = %start_str,
            end = %end_str,
            count = klines.len(),
            "KRX 정보데이터시스템 데이터 가져오기 완료"
        );

        Ok(klines)
    }

    /// KRX Open API 클라이언트 초기화 (lazy, 한 번만 실행).
    async fn get_krx_api_client(&self) -> Option<&KrxApiClient> {
        // OnceCell로 한 번만 초기화
        let client_opt = self
            .krx_api_client
            .get_or_init(|| async {
                let master_key = match &self.encryption_key {
                    Some(key) => key,
                    None => {
                        debug!("ENCRYPTION_MASTER_KEY 미설정 - KRX Open API 비활성화");
                        return None;
                    }
                };

                let encryptor = match CredentialEncryptor::new(master_key) {
                    Ok(enc) => enc,
                    Err(e) => {
                        warn!(error = %e, "CredentialEncryptor 생성 실패");
                        return None;
                    }
                };

                match KrxApiClient::from_credential(&self.pool, &encryptor).await {
                    Ok(Some(client)) => {
                        info!("KRX Open API 클라이언트 초기화 완료");
                        Some(client)
                    }
                    Ok(None) => {
                        debug!("KRX API credential이 등록되지 않음");
                        None
                    }
                    Err(e) => {
                        warn!(error = %e, "KRX API credential 로드 실패");
                        None
                    }
                }
            })
            .await;

        client_opt.as_ref()
    }

    /// KRX Open API로 데이터 조회 시도 (캐시된 클라이언트 사용).
    ///
    /// ENCRYPTION_MASTER_KEY 환경변수가 설정되어 있고,
    /// exchange_credentials 테이블에 KRX API 키가 등록된 경우에만 작동합니다.
    async fn try_krx_api(
        &self,
        symbol: &str,
        start_date: &str,
        end_date: &str,
    ) -> Option<Vec<Kline>> {
        // 캐시된 클라이언트 사용
        let client = self.get_krx_api_client().await?;

        // API 호출
        match client.fetch_daily_ohlcv(symbol, start_date, end_date).await {
            Ok(ohlcvs) if !ohlcvs.is_empty() => {
                // KrxOhlcv -> Kline 변환
                let klines: Vec<Kline> = ohlcvs
                    .into_iter()
                    .map(|o| Kline {
                        ticker: symbol.to_string(),
                        timeframe: Timeframe::D1,
                        open_time: Utc.from_utc_datetime(&o.date.and_hms_opt(0, 0, 0).unwrap()),
                        open: o.open,
                        high: o.high,
                        low: o.low,
                        close: o.close,
                        volume: Decimal::from(o.volume),
                        close_time: Utc.from_utc_datetime(&o.date.and_hms_opt(23, 59, 59).unwrap()),
                        quote_volume: o.trading_value,
                        num_trades: None,
                    })
                    .collect();
                Some(klines)
            }
            Ok(_) => {
                debug!(symbol = symbol, "KRX Open API: 데이터 없음");
                None
            }
            Err(e) => {
                debug!(symbol = symbol, error = %e, "KRX Open API 조회 실패");
                None
            }
        }
    }

    /// 동시성 제어를 위한 Lock 획득 또는 생성.
    async fn get_or_create_lock(&self, key: &str) -> Arc<RwLock<()>> {
        let locks = self.fetch_locks.read().await;
        if let Some(lock) = locks.get(key) {
            return lock.clone();
        }
        drop(locks);

        let mut locks = self.fetch_locks.write().await;
        locks
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(RwLock::new(())))
            .clone()
    }

    /// 캐시 업데이트 필요 여부 판단 (시장 시간 고려).
    fn should_update(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        cached_count: usize,
        requested: usize,
        last_cached_time: Option<DateTime<Utc>>,
    ) -> bool {
        // 캐시된 데이터가 요청량보다 적으면 업데이트 필요
        if cached_count < requested {
            return true;
        }

        // 마지막 캐시 시간 확인
        let last_time = match last_cached_time {
            Some(t) => t,
            None => return true,
        };

        let now = Utc::now();
        let expected_interval = timeframe_to_duration(timeframe);

        // 마지막 캔들 시간 + 간격 + 유효기간 < 현재 시간이면 업데이트 필요
        let stale_threshold = last_time + expected_interval + self.cache_freshness;

        if stale_threshold >= now {
            // 아직 신선함
            return false;
        }

        // 시장 마감 체크: 마감 후 일정 시간 이후면 업데이트 안함
        if !self.is_market_active(symbol, timeframe) {
            debug!(symbol = symbol, "시장 마감 상태, 캐시 업데이트 스킵");
            return false;
        }

        true
    }

    /// 시장이 활성 상태인지 확인.
    ///
    /// - 미국 주식: 월~금 09:30-16:00 EST + 마감 후 1시간
    /// - 한국 주식: 월~금 09:00-15:30 KST + 마감 후 1시간
    /// - 암호화폐: 항상 활성
    fn is_market_active(&self, symbol: &str, timeframe: Timeframe) -> bool {
        // 일봉 이상은 항상 업데이트 (하루에 한 번 정도)
        if !is_intraday(timeframe) {
            return true;
        }

        let now = Utc::now();

        // 한국 주식 (.KS, .KQ)
        if symbol.ends_with(".KS") || symbol.ends_with(".KQ") {
            return is_korean_market_active(now);
        }

        // 일본 주식 (.T)
        if symbol.ends_with(".T") {
            return is_japanese_market_active(now);
        }

        // 기본값: 미국 주식
        is_us_market_active(now)
    }

    /// 외부 데이터 소스에서 데이터 가져와 캐시에 저장.
    ///
    /// # 3단계 Fallback 정책
    /// 1. KRX (한국 주식만, 일봉 지원)
    /// 2. Yahoo Finance (전 세계 주식, 모든 타임프레임)
    /// 3. 거래소 API (KIS/Binance, Rate Limit 딜레이 적용)
    ///
    /// 모든 소스 실패 시 상장폐지로 판단하여 심볼을 비활성화합니다.
    ///
    /// # 인자
    /// - `original_symbol`: 원본 심볼 (데이터 소스 선택용)
    /// - `cache_symbol`: 캐시 저장용 심볼 (Yahoo 형식)
    async fn fetch_and_cache(
        &self,
        original_symbol: &str,
        cache_symbol: &str,
        timeframe: Timeframe,
        limit: usize,
        last_cached_time: Option<DateTime<Utc>>,
    ) -> Result<usize> {
        // =========================================
        // 1단계: KRX (한국 주식만)
        // =========================================
        let mut krx_tried = false;
        let klines = if is_pure_korean_stock_code(original_symbol) {
            krx_tried = true;
            debug!(
                symbol = original_symbol,
                "1단계: KRX 데이터 소스 시도 (한국 주식)"
            );
            match self.fetch_from_krx(original_symbol, timeframe, limit).await {
                Ok(data) if !data.is_empty() => {
                    debug!(
                        symbol = original_symbol,
                        count = data.len(),
                        "KRX 데이터 가져오기 성공"
                    );
                    Some(data)
                }
                Ok(_) => {
                    info!(
                        symbol = original_symbol,
                        "KRX 빈 데이터, Yahoo Finance로 Fallback"
                    );
                    None
                }
                Err(e) => {
                    warn!(
                        symbol = original_symbol,
                        error = %e,
                        "KRX 실패, Yahoo Finance로 Fallback"
                    );
                    None
                }
            }
        } else {
            None
        };

        // =========================================
        // 2단계: Yahoo Finance
        // =========================================
        let klines = match klines {
            Some(data) => data,
            None => {
                debug!(
                    symbol = cache_symbol,
                    "2단계: Yahoo Finance 데이터 소스 시도"
                );
                let provider = YahooProviderWrapper::new(SymbolResolver::new(self.pool.clone()))?;
                match provider
                    .get_klines_internal(cache_symbol, timeframe, limit)
                    .await
                {
                    Ok(data) if !data.is_empty() => {
                        debug!(
                            symbol = cache_symbol,
                            count = data.len(),
                            "Yahoo Finance 데이터 가져오기 성공"
                        );
                        data
                    }
                    Ok(_) => {
                        warn!(symbol = cache_symbol, "Yahoo Finance 빈 데이터 반환");
                        // 빈 데이터도 상장폐지 가능성
                        Vec::new()
                    }
                    Err(e) => {
                        let error_str = e.to_string();
                        // "delisted" 관련 오류면 즉시 상장폐지 처리
                        if is_delisted_error(&error_str) {
                            warn!(
                                symbol = original_symbol,
                                error = %error_str,
                                "Yahoo Finance에서 상장폐지 오류 감지"
                            );
                            self.mark_as_delisted(original_symbol, &error_str).await?;
                            return Err(DataError::SymbolDelisted(original_symbol.to_string()));
                        }
                        warn!(
                            symbol = cache_symbol,
                            error = %e,
                            "Yahoo Finance 실패"
                        );
                        Vec::new()
                    }
                }
            }
        };

        // 데이터 있으면 저장 후 반환
        if !klines.is_empty() {
            return self
                .save_klines_to_cache(cache_symbol, timeframe, klines, last_cached_time)
                .await;
        }

        // =========================================
        // 3단계: 모든 소스 실패 → 상장폐지 판단
        // =========================================
        // KRX와 Yahoo Finance 모두 실패/빈 데이터면 상장폐지로 판단
        let source_info = if krx_tried {
            "KRX + Yahoo Finance"
        } else {
            "Yahoo Finance"
        };

        error!(
            symbol = original_symbol,
            sources = source_info,
            "🚨 모든 데이터 소스 실패 - 상장폐지로 판단"
        );
        self.mark_as_delisted(
            original_symbol,
            &format!("모든 데이터 소스({}) 실패", source_info),
        )
        .await?;

        Err(DataError::SymbolDelisted(original_symbol.to_string()))
    }

    /// KRX에서 데이터 가져오기.
    async fn fetch_from_krx(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<Vec<Kline>> {
        // KRX는 일봉만 지원
        if timeframe != Timeframe::D1 {
            warn!(
                symbol = symbol,
                timeframe = %timeframe_to_string(timeframe),
                "KRX는 일봉(1d)만 지원합니다. 일봉으로 대체합니다."
            );
        }

        // 기간 계산 (limit 일수 + 여유분)
        let end_date = Utc::now();
        let start_date = end_date - Duration::days((limit as i64) + 30);

        let start_str = start_date.format("%Y%m%d").to_string();
        let end_str = end_date.format("%Y%m%d").to_string();

        // 캐시된 KRX 정보데이터시스템 클라이언트 사용
        let klines = self
            .krx_data_source
            .get_ohlcv(symbol, &start_str, &end_str)
            .await?;

        // limit만큼만 반환 (최신순)
        let result: Vec<Kline> = klines
            .into_iter()
            .rev()
            .take(limit)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        Ok(result)
    }

    /// 배치 INSERT로 캔들 저장.
    async fn batch_insert_klines(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        klines: &[Kline],
    ) -> Result<usize> {
        if klines.is_empty() {
            return Ok(0);
        }

        let tf_str = timeframe_to_string(timeframe);
        let mut total_inserted = 0;

        for chunk in klines.chunks(500) {
            let mut query = String::from(
                r#"INSERT INTO ohlcv
                   (symbol, timeframe, open_time, open, high, low, close, volume, close_time, fetched_at)
                   VALUES "#,
            );

            // VALUES 절 구성: ($1, $2, ...), ($10, $11, ...), ...
            let value_tuples: Vec<String> = chunk
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    let base = i * 9;
                    format!(
                        "(${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, NOW())",
                        base + 1,
                        base + 2,
                        base + 3,
                        base + 4,
                        base + 5,
                        base + 6,
                        base + 7,
                        base + 8,
                        base + 9
                    )
                })
                .collect();
            query.push_str(&value_tuples.join(", "));

            query.push_str(
                r#" ON CONFLICT (symbol, timeframe, open_time) DO UPDATE SET
                    high = GREATEST(ohlcv.high, EXCLUDED.high),
                    low = LEAST(ohlcv.low, EXCLUDED.low),
                    close = EXCLUDED.close,
                    volume = EXCLUDED.volume,
                    close_time = EXCLUDED.close_time,
                    fetched_at = NOW()"#,
            );

            let mut sql_query = sqlx::query(&query);

            for kline in chunk {
                sql_query = sql_query
                    .bind(symbol)
                    .bind(&tf_str)
                    .bind(kline.open_time)
                    .bind(kline.open)
                    .bind(kline.high)
                    .bind(kline.low)
                    .bind(kline.close)
                    .bind(kline.volume)
                    .bind(kline.close_time);
            }

            let result = sql_query
                .execute(&self.pool)
                .await
                .map_err(|e| DataError::InsertError(e.to_string()))?;

            total_inserted += result.rows_affected() as usize;
        }

        // 메타데이터 업데이트
        self.update_cache_metadata(symbol, timeframe).await?;

        Ok(total_inserted)
    }

    /// 캔들 데이터를 캐시에 저장 (증분 업데이트 적용).
    async fn save_klines_to_cache(
        &self,
        cache_symbol: &str,
        timeframe: Timeframe,
        klines: Vec<Kline>,
        last_cached_time: Option<DateTime<Utc>>,
    ) -> Result<usize> {
        if klines.is_empty() {
            return Ok(0);
        }

        // 증분 업데이트: 마지막 캐시 시간 이후 데이터만 저장
        let new_klines: Vec<Kline> = if let Some(last_time) = last_cached_time {
            klines
                .into_iter()
                .filter(|k| k.open_time > last_time)
                .collect()
        } else {
            klines
        };

        if new_klines.is_empty() {
            debug!(symbol = cache_symbol, "새 데이터 없음");
            return Ok(0);
        }

        // 배치 INSERT로 캐시에 저장 (Yahoo 형식 심볼 사용)
        let saved = self
            .batch_insert_klines(cache_symbol, timeframe, &new_klines)
            .await?;
        Ok(saved)
    }

    /// 상장폐지 추정 심볼을 비활성화.
    ///
    /// symbol_info 테이블에 해당 심볼이 있으면 is_active를 FALSE로 설정하고,
    /// 실패 횟수 및 오류 메시지를 기록합니다.
    async fn mark_as_delisted(&self, symbol: &str, reason: &str) -> Result<()> {
        // 먼저 순수 6자리 한국 주식 코드인지, .KS/.KQ 포함인지 확인
        let ticker_variants = if is_pure_korean_stock_code(symbol) {
            vec![
                symbol.to_string(),
                format!("{}.KS", symbol),
                format!("{}.KQ", symbol),
            ]
        } else if symbol.ends_with(".KS") || symbol.ends_with(".KQ") {
            let base = &symbol[..6];
            vec![symbol.to_string(), base.to_string()]
        } else {
            vec![symbol.to_string()]
        };

        for ticker in &ticker_variants {
            let result = sqlx::query(
                r#"
                UPDATE symbol_info
                SET is_active = FALSE,
                    fetch_fail_count = COALESCE(fetch_fail_count, 0) + 1,
                    last_fetch_error = $2,
                    last_fetch_attempt = NOW(),
                    updated_at = NOW()
                WHERE ticker = $1
                "#,
            )
            .bind(ticker)
            .bind(format!("상장폐지 추정: {}", reason))
            .execute(&self.pool)
            .await;

            match result {
                Ok(res) if res.rows_affected() > 0 => {
                    error!(
                        symbol = ticker,
                        reason = reason,
                        "🚨 심볼 비활성화됨 (상장폐지 추정)"
                    );
                }
                Ok(_) => {
                    // 해당 ticker가 symbol_info에 없음 - 정상 케이스
                    debug!(symbol = ticker, "symbol_info에 없는 심볼, 건너뜀");
                }
                Err(e) => {
                    warn!(
                        symbol = ticker,
                        error = %e,
                        "상장폐지 마킹 실패 (DB 오류)"
                    );
                }
            }
        }

        Ok(())
    }

    /// 캐시 메타데이터 업데이트.
    async fn update_cache_metadata(&self, symbol: &str, timeframe: Timeframe) -> Result<()> {
        let tf_str = timeframe_to_string(timeframe);

        sqlx::query(
            r#"
            INSERT INTO ohlcv_metadata (symbol, timeframe, first_cached_time, last_cached_time, total_candles, last_updated_at)
            SELECT $1, $2, MIN(open_time), MAX(open_time), COUNT(*), NOW()
            FROM ohlcv
            WHERE symbol = $1 AND timeframe = $2
            ON CONFLICT (symbol, timeframe) DO UPDATE SET
                first_cached_time = EXCLUDED.first_cached_time,
                last_cached_time = EXCLUDED.last_cached_time,
                total_candles = EXCLUDED.total_candles,
                last_updated_at = NOW()
            "#
        )
        .bind(symbol)
        .bind(&tf_str)
        .execute(&self.pool)
        .await
        .map_err(|e| DataError::InsertError(e.to_string()))?;

        Ok(())
    }

    /// 데이터 갭 감지 및 경고.
    async fn detect_and_warn_gaps(&self, symbol: &str, timeframe: Timeframe, limit: usize) {
        let expected_duration = timeframe_to_duration(timeframe);

        // 캐시된 데이터 조회
        let klines: Vec<Kline> = match self.cache.get_cached_klines(symbol, timeframe, limit).await
        {
            Ok(k) => k,
            Err(_) => return,
        };

        if klines.len() < 2 {
            return;
        }

        let mut gap_count = 0;
        for window in klines.windows(2) {
            let prev = &window[0];
            let curr = &window[1];

            let actual_gap = curr.open_time - prev.open_time;
            // 예상 간격의 1.5배를 초과하면 갭으로 간주
            let threshold = expected_duration + (expected_duration / 2);

            if actual_gap > threshold {
                gap_count += 1;
            }
        }

        if gap_count > 0 {
            warn!(
                symbol = symbol,
                timeframe = %timeframe_to_string(timeframe),
                gap_count = gap_count,
                "데이터 갭 감지 (주말/휴장일 제외 시 정상일 수 있음)"
            );
        }
    }

    /// 캐시 통계 조회.
    pub async fn get_cache_stats(&self) -> Result<Vec<CacheStats>> {
        use crate::storage::ohlcv::OhlcvMetadataRecord;
        let records: Vec<OhlcvMetadataRecord> = self.cache.get_all_cache_stats().await?;
        Ok(records
            .into_iter()
            .map(|r| CacheStats {
                symbol: r.symbol,
                timeframe: r.timeframe,
                first_time: r.first_cached_time,
                last_time: r.last_cached_time,
                candle_count: r.total_candles.unwrap_or(0) as i64,
                last_updated: r.last_updated_at,
            })
            .collect())
    }

    /// 특정 심볼 캐시 삭제.
    ///
    /// # 인자
    /// - `symbol`: canonical 심볼 (예: "005930", "AAPL")
    pub async fn clear_cache(&self, symbol: &str) -> Result<u64> {
        let (ticker, _, _) = self.resolve_symbol(symbol).await?;
        self.cache.clear_symbol_cache(&ticker).await
    }

    /// 캐시 Warmup (주요 심볼 미리 캐시).
    pub async fn warmup(&self, symbols: &[(&str, Timeframe, usize)]) -> Result<usize> {
        let mut total = 0;
        for (symbol, timeframe, limit) in symbols {
            match self.get_klines(symbol, *timeframe, *limit).await {
                Ok(klines) => {
                    total += klines.len();
                    info!(symbol = symbol, count = klines.len(), "Warmup 완료");
                }
                Err(e) => {
                    warn!(symbol = symbol, error = %e, "Warmup 실패");
                }
            }
        }
        Ok(total)
    }

    /// 다중 타임프레임 캐시 Warmup (병렬 처리).
    ///
    /// 단일 심볼에 대해 여러 타임프레임의 데이터를 병렬로 미리 캐시합니다.
    ///
    /// # 인자
    ///
    /// * `symbol` - canonical 심볼 (예: "005930", "BTCUSDT")
    /// * `config` - 다중 타임프레임 설정
    ///
    /// # 반환
    ///
    /// 타임프레임별 로드된 캔들 수
    ///
    /// # 예시
    ///
    /// ```rust,ignore
    /// use trader_core::{domain::MultiTimeframeConfig, Timeframe};
    ///
    /// let config = MultiTimeframeConfig::new()
    ///     .with_timeframe(Timeframe::M5, 60)
    ///     .with_timeframe(Timeframe::H1, 24)
    ///     .with_timeframe(Timeframe::D1, 14);
    ///
    /// let counts = provider.warmup_multi_timeframe("BTCUSDT", &config).await?;
    /// for (tf, count) in &counts {
    ///     println!("{:?}: {} candles", tf, count);
    /// }
    /// ```
    pub async fn warmup_multi_timeframe(
        &self,
        symbol: &str,
        config: &trader_core::domain::MultiTimeframeConfig,
    ) -> Result<std::collections::HashMap<Timeframe, usize>> {
        use futures::future::join_all;

        let timeframes: Vec<_> = config.timeframes.iter().collect();

        // 각 타임프레임별 병렬 로드
        let futures: Vec<_> = timeframes
            .iter()
            .map(|(&tf, &limit)| {
                let symbol = symbol.to_string();
                async move {
                    let result = self.get_klines(&symbol, tf, limit).await;
                    (tf, result)
                }
            })
            .collect();

        let results = join_all(futures).await;

        let mut counts = std::collections::HashMap::new();
        for (tf, result) in results {
            match result {
                Ok(klines) => {
                    let count = klines.len();
                    counts.insert(tf, count);
                    info!(
                        symbol = symbol,
                        timeframe = ?tf,
                        count = count,
                        "다중 TF Warmup 완료"
                    );
                }
                Err(e) => {
                    counts.insert(tf, 0);
                    warn!(
                        symbol = symbol,
                        timeframe = ?tf,
                        error = %e,
                        "다중 TF Warmup 실패"
                    );
                }
            }
        }

        Ok(counts)
    }

    /// 여러 타임프레임의 캔들 데이터를 병렬로 조회.
    ///
    /// # 인자
    ///
    /// * `symbol` - canonical 심볼
    /// * `config` - 다중 타임프레임 설정
    ///
    /// # 반환
    ///
    /// 타임프레임별 캔들 데이터
    pub async fn get_multi_timeframe_klines(
        &self,
        symbol: &str,
        config: &trader_core::domain::MultiTimeframeConfig,
    ) -> Result<std::collections::HashMap<Timeframe, Vec<Kline>>> {
        use futures::future::join_all;

        let timeframes: Vec<_> = config.timeframes.iter().collect();

        let futures: Vec<_> = timeframes
            .iter()
            .map(|(&tf, &limit)| {
                let symbol = symbol.to_string();
                async move {
                    let result = self.get_klines(&symbol, tf, limit).await;
                    (tf, result)
                }
            })
            .collect();

        let results = join_all(futures).await;

        let mut map = std::collections::HashMap::new();
        for (tf, result) in results {
            match result {
                Ok(klines) => {
                    map.insert(tf, klines);
                }
                Err(e) => {
                    warn!(
                        symbol = symbol,
                        timeframe = ?tf,
                        error = %e,
                        "다중 TF 조회 실패, 빈 데이터 반환"
                    );
                    map.insert(tf, Vec::new());
                }
            }
        }

        Ok(map)
    }
}

/// 캐시 통계.
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub symbol: String,
    pub timeframe: String,
    pub first_time: Option<DateTime<Utc>>,
    pub last_time: Option<DateTime<Utc>>,
    pub candle_count: i64,
    pub last_updated: Option<DateTime<Utc>>,
}

// =============================================================================
// 시장 시간 체크 함수
// =============================================================================

/// 미국 시장 활성 여부 (09:30-16:00 EST + 마감 후 1시간).
fn is_us_market_active(now: DateTime<Utc>) -> bool {
    let est: Tz = "America/New_York".parse().unwrap();
    let now_est = now.with_timezone(&est);

    // 주말 체크
    if matches!(now_est.weekday(), Weekday::Sat | Weekday::Sun) {
        return false;
    }

    let hour = now_est.hour();
    let minute = now_est.minute();
    let time_minutes = hour * 60 + minute;

    // 09:30 ~ 17:00 (마감 후 1시간 포함)
    let market_open = 9 * 60 + 30; // 09:30
    let market_close_extended = 17 * 60; // 17:00

    time_minutes >= market_open && time_minutes <= market_close_extended
}

/// 한국 시장 활성 여부 (09:00-15:30 KST + 마감 후 1시간).
fn is_korean_market_active(now: DateTime<Utc>) -> bool {
    let kst: Tz = "Asia/Seoul".parse().unwrap();
    let now_kst = now.with_timezone(&kst);

    // 주말 체크
    if matches!(now_kst.weekday(), Weekday::Sat | Weekday::Sun) {
        return false;
    }

    let hour = now_kst.hour();
    let minute = now_kst.minute();
    let time_minutes = hour * 60 + minute;

    // 09:00 ~ 16:30 (마감 후 1시간 포함)
    let market_open = 9 * 60; // 09:00
    let market_close_extended = 16 * 60 + 30; // 16:30

    time_minutes >= market_open && time_minutes <= market_close_extended
}

/// 일본 시장 활성 여부 (09:00-15:00 JST + 마감 후 1시간).
fn is_japanese_market_active(now: DateTime<Utc>) -> bool {
    let jst: Tz = "Asia/Tokyo".parse().unwrap();
    let now_jst = now.with_timezone(&jst);

    // 주말 체크
    if matches!(now_jst.weekday(), Weekday::Sat | Weekday::Sun) {
        return false;
    }

    let hour = now_jst.hour();
    let minute = now_jst.minute();
    let time_minutes = hour * 60 + minute;

    // 09:00 ~ 16:00 (마감 후 1시간 포함)
    let market_open = 9 * 60; // 09:00
    let market_close_extended = 16 * 60; // 16:00

    time_minutes >= market_open && time_minutes <= market_close_extended
}

// =============================================================================
// 헬퍼 함수
// =============================================================================

/// Timeframe의 Duration 계산.
fn timeframe_to_duration(timeframe: Timeframe) -> Duration {
    match timeframe {
        Timeframe::M1 => Duration::minutes(1),
        Timeframe::M3 => Duration::minutes(3),
        Timeframe::M5 => Duration::minutes(5),
        Timeframe::M15 => Duration::minutes(15),
        Timeframe::M30 => Duration::minutes(30),
        Timeframe::H1 => Duration::hours(1),
        Timeframe::H2 => Duration::hours(2),
        Timeframe::H4 => Duration::hours(4),
        Timeframe::H6 => Duration::hours(6),
        Timeframe::H8 => Duration::hours(8),
        Timeframe::H12 => Duration::hours(12),
        Timeframe::D1 => Duration::days(1),
        Timeframe::D3 => Duration::days(3),
        Timeframe::W1 => Duration::weeks(1),
        Timeframe::MN1 => Duration::days(30),
    }
}

/// 분봉/시간봉인지 확인.
fn is_intraday(timeframe: Timeframe) -> bool {
    matches!(
        timeframe,
        Timeframe::M1
            | Timeframe::M3
            | Timeframe::M5
            | Timeframe::M15
            | Timeframe::M30
            | Timeframe::H1
            | Timeframe::H2
            | Timeframe::H4
            | Timeframe::H6
            | Timeframe::H8
            | Timeframe::H12
    )
}

/// 순수 한국 주식 코드인지 확인 (6자리 숫자, .KS/.KQ 접미사 없음).
///
/// KRX 데이터 소스를 사용할 심볼인지 판단합니다:
/// - "005930" → true (KRX 사용)
/// - "005930.KS" → false (Yahoo Finance 사용)
/// - "AAPL" → false (Yahoo Finance 사용)
fn is_pure_korean_stock_code(symbol: &str) -> bool {
    // .KS, .KQ 접미사가 있으면 Yahoo Finance 사용
    if symbol.ends_with(".KS") || symbol.ends_with(".KQ") {
        return false;
    }

    // 정확히 6자리 숫자면 KRX 사용
    symbol.len() == 6 && symbol.chars().all(|c| c.is_ascii_digit())
}

/// 심볼에서 통화 코드 추정.
pub(crate) fn guess_currency(symbol: &str) -> &'static str {
    if symbol.ends_with(".KS") || symbol.ends_with(".KQ") {
        "KRW"
    } else if symbol.ends_with(".T") {
        "JPY"
    } else if symbol.ends_with(".L") {
        "GBP"
    } else {
        "USD"
    }
}

// =============================================================================
// Yahoo Finance Provider 래퍼
// =============================================================================

/// Yahoo Finance Provider 래퍼.
///
/// `SymbolResolver`를 통해 ticker에서 yahoo_symbol을 조회합니다.
pub struct YahooProviderWrapper {
    connector: yahoo_finance_api::YahooConnector,
    symbol_resolver: SymbolResolver,
}

impl YahooProviderWrapper {
    pub fn new(symbol_resolver: SymbolResolver) -> Result<Self> {
        let connector = yahoo_finance_api::YahooConnector::new()
            .map_err(|e| DataError::ConnectionError(format!("Yahoo Finance 연결 실패: {}", e)))?;
        Ok(Self {
            connector,
            symbol_resolver,
        })
    }

    /// ticker를 Yahoo Finance API 호출용 심볼로 변환.
    ///
    /// `SymbolResolver`를 통해 DB에서 정확한 yahoo_symbol을 조회합니다.
    /// DB에 없으면 fallback으로 6자리 숫자는 `.KS` 추가.
    async fn resolve_yahoo_symbol(&self, ticker: &str) -> String {
        // DB에서 yahoo_symbol 조회 시도
        if let Ok(Some(info)) = self.symbol_resolver.get_symbol_info(ticker).await {
            if let Some(yahoo_symbol) = info.yahoo_symbol {
                return yahoo_symbol;
            }
        }
        // Fallback: 6자리 숫자 한국 주식인 경우 .KS 추가
        if ticker.len() == 6 && ticker.chars().all(|c| c.is_ascii_digit()) {
            format!("{}.KS", ticker)
        } else {
            ticker.to_string()
        }
    }

    pub async fn get_klines_internal(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<Vec<Kline>> {
        let interval = match timeframe {
            Timeframe::M1 => "1m",
            Timeframe::M3 | Timeframe::M5 => "5m",
            Timeframe::M15 => "15m",
            Timeframe::M30 => "30m",
            Timeframe::H1
            | Timeframe::H2
            | Timeframe::H4
            | Timeframe::H6
            | Timeframe::H8
            | Timeframe::H12 => "1h",
            Timeframe::D1 | Timeframe::D3 => "1d",
            Timeframe::W1 => "1wk",
            Timeframe::MN1 => "1mo",
        };

        let range = calculate_range_string(timeframe, limit);

        // SymbolResolver를 통해 yahoo_symbol 조회
        let yahoo_symbol = self.resolve_yahoo_symbol(symbol).await;

        debug!(
            ticker = symbol,
            yahoo_symbol = %yahoo_symbol,
            interval = interval,
            range = range,
            "Yahoo Finance API 호출"
        );

        let response = self
            .connector
            .get_quote_range(&yahoo_symbol, interval, range)
            .await
            .map_err(|e| {
                DataError::FetchError(format!("Yahoo Finance API 오류 ({}): {}", yahoo_symbol, e))
            })?;

        let quotes = response
            .quotes()
            .map_err(|e| DataError::ParseError(format!("Quote 파싱 오류: {}", e)))?;

        if quotes.is_empty() {
            return Ok(Vec::new());
        }

        let _currency = guess_currency(symbol);
        let symbol_obj = symbol.to_string();

        let klines: Vec<Kline> = quotes
            .iter()
            .map(|q| {
                let open_time = Utc
                    .timestamp_opt(q.timestamp, 0)
                    .single()
                    .unwrap_or_else(Utc::now);
                let close_time = open_time + timeframe_to_duration(timeframe);

                Kline {
                    ticker: symbol_obj.clone(),
                    timeframe,
                    open_time,
                    open: Decimal::from_f64_retain(q.open).unwrap_or_default(),
                    high: Decimal::from_f64_retain(q.high).unwrap_or_default(),
                    low: Decimal::from_f64_retain(q.low).unwrap_or_default(),
                    close: Decimal::from_f64_retain(q.close).unwrap_or_default(),
                    volume: Decimal::from(q.volume),
                    close_time,
                    quote_volume: None,
                    num_trades: None,
                }
            })
            .collect();

        let mut sorted = klines;
        sorted.sort_by_key(|k| k.open_time);

        if sorted.len() > limit {
            let skip = sorted.len() - limit;
            sorted = sorted.into_iter().skip(skip).collect();
        }

        Ok(sorted)
    }

    /// 날짜 범위로 캔들 데이터 조회.
    ///
    /// # Arguments
    /// * `ticker` - 순수 ticker (예: "005930", "AAPL")
    ///
    /// 내부에서 Yahoo Finance API 호출용 심볼로 변환 (한국 주식: .KS 추가)
    pub async fn get_klines_range(
        &self,
        ticker: &str,
        timeframe: Timeframe,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<Kline>> {
        let interval = match timeframe {
            Timeframe::M1 => "1m",
            Timeframe::M3 | Timeframe::M5 => "5m",
            Timeframe::M15 => "15m",
            Timeframe::M30 => "30m",
            Timeframe::H1
            | Timeframe::H2
            | Timeframe::H4
            | Timeframe::H6
            | Timeframe::H8
            | Timeframe::H12 => "1h",
            Timeframe::D1 | Timeframe::D3 => "1d",
            Timeframe::W1 => "1wk",
            Timeframe::MN1 => "1mo",
        };

        // chrono::NaiveDate → time::OffsetDateTime 변환
        let start = naive_date_to_offset_datetime(start_date);
        let end = naive_date_to_offset_datetime(end_date);

        // SymbolResolver를 통해 yahoo_symbol 조회
        let yahoo_symbol = self.resolve_yahoo_symbol(ticker).await;

        debug!(
            ticker = ticker,
            yahoo_symbol = %yahoo_symbol,
            interval = interval,
            start = %start_date,
            end = %end_date,
            "Yahoo Finance API 날짜 범위 호출"
        );

        let response = self
            .connector
            .get_quote_history_interval(&yahoo_symbol, start, end, interval)
            .await
            .map_err(|e| {
                DataError::FetchError(format!("Yahoo Finance API 오류 ({}): {}", yahoo_symbol, e))
            })?;

        let quotes = response
            .quotes()
            .map_err(|e| DataError::ParseError(format!("Quote 파싱 오류: {}", e)))?;

        if quotes.is_empty() {
            return Ok(Vec::new());
        }

        // 저장용 ticker 사용
        let klines: Vec<Kline> = quotes
            .iter()
            .map(|q| {
                let open_time = Utc
                    .timestamp_opt(q.timestamp, 0)
                    .single()
                    .unwrap_or_else(Utc::now);
                let close_time = open_time + timeframe_to_duration(timeframe);

                Kline {
                    ticker: ticker.to_string(),
                    timeframe,
                    open_time,
                    open: Decimal::from_f64_retain(q.open).unwrap_or_default(),
                    high: Decimal::from_f64_retain(q.high).unwrap_or_default(),
                    low: Decimal::from_f64_retain(q.low).unwrap_or_default(),
                    close: Decimal::from_f64_retain(q.close).unwrap_or_default(),
                    volume: Decimal::from(q.volume),
                    close_time,
                    quote_volume: None,
                    num_trades: None,
                }
            })
            .collect();

        let mut sorted = klines;
        sorted.sort_by_key(|k| k.open_time);

        Ok(sorted)
    }
}

/// NaiveDate를 OffsetDateTime으로 변환.
fn naive_date_to_offset_datetime(date: NaiveDate) -> OffsetDateTime {
    let (year, month, day) = (date.year(), date.month() as u8, date.day() as u8);
    time::Date::from_calendar_date(year, time::Month::try_from(month).unwrap(), day)
        .unwrap()
        .midnight()
        .assume_utc()
}

fn calculate_range_string(timeframe: Timeframe, limit: usize) -> &'static str {
    match timeframe {
        Timeframe::M1 | Timeframe::M3 | Timeframe::M5 | Timeframe::M15 | Timeframe::M30 => {
            if limit <= 100 {
                "5d"
            } else if limit <= 500 {
                "1mo"
            } else {
                "3mo"
            }
        }
        Timeframe::H1
        | Timeframe::H2
        | Timeframe::H4
        | Timeframe::H6
        | Timeframe::H8
        | Timeframe::H12 => {
            if limit <= 50 {
                "5d"
            } else if limit <= 200 {
                "1mo"
            } else {
                "3mo"
            }
        }
        Timeframe::D1 => {
            if limit <= 5 {
                "5d"
            } else if limit <= 20 {
                "1mo"
            } else if limit <= 60 {
                "3mo"
            } else if limit <= 120 {
                "6mo"
            } else if limit <= 250 {
                "1y"
            } else if limit <= 500 {
                "2y"
            } else if limit <= 1250 {
                "5y"
            } else {
                "10y"
            }
        }
        Timeframe::D3 => {
            if limit <= 10 {
                "1mo"
            } else if limit <= 30 {
                "3mo"
            } else if limit <= 60 {
                "6mo"
            } else {
                "1y"
            }
        }
        Timeframe::W1 => {
            if limit <= 4 {
                "1mo"
            } else if limit <= 12 {
                "3mo"
            } else if limit <= 26 {
                "6mo"
            } else if limit <= 52 {
                "1y"
            } else if limit <= 104 {
                "2y"
            } else {
                "5y"
            }
        }
        Timeframe::MN1 => {
            if limit <= 3 {
                "3mo"
            } else if limit <= 6 {
                "6mo"
            } else if limit <= 12 {
                "1y"
            } else if limit <= 24 {
                "2y"
            } else if limit <= 60 {
                "5y"
            } else {
                "10y"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // FIXME: to_yahoo_symbol은 Symbol::to_yahoo_symbol(ticker, market)로 두 인자 필요
    // #[test]
    // fn test_to_yahoo_symbol() {
    //     assert_eq!(to_yahoo_symbol("005930"), "005930.KS");
    //     assert_eq!(to_yahoo_symbol("AAPL"), "AAPL");
    // }

    #[test]
    fn test_is_intraday() {
        assert!(is_intraday(Timeframe::M1));
        assert!(is_intraday(Timeframe::H1));
        assert!(!is_intraday(Timeframe::D1));
    }
}
