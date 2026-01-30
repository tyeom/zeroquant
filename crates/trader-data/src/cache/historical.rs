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
use crate::provider::SymbolResolver;
use crate::storage::krx::KrxDataSource;
use crate::storage::ohlcv::{timeframe_to_string, OhlcvCache};
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Timelike, Utc, Weekday};
use chrono_tz::Tz;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use time::OffsetDateTime;
use tokio::sync::RwLock;
use trader_core::{Kline, MarketType, Symbol, Timeframe};
use tracing::{debug, info, instrument, warn};

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
}

impl CachedHistoricalDataProvider {
    /// 새로운 캐시 기반 제공자 생성.
    pub fn new(pool: PgPool) -> Self {
        Self {
            cache: OhlcvCache::new(pool.clone()),
            symbol_resolver: SymbolResolver::new(pool.clone()),
            pool,
            cache_freshness: Duration::minutes(5),
            fetch_locks: Arc::new(RwLock::new(HashMap::new())),
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
        let (source_symbol, quote_currency, market_type) = self.resolve_symbol(symbol).await;
        let lock_key = format!("{}:{}", source_symbol, timeframe_to_string(timeframe));

        // 1. 동시성 제어: Lock 획득
        let lock = self.get_or_create_lock(&lock_key).await;
        let _guard = lock.write().await;

        // 2. 캐시 상태 확인
        let cached_count = self.cache.get_cached_count(&source_symbol, timeframe).await?;
        let last_cached_time = self.cache.get_last_cached_time(&source_symbol, timeframe).await?;

        // 3. 업데이트 필요 여부 판단 (시장 시간 고려)
        let needs_update = self.should_update(
            &source_symbol,
            timeframe,
            cached_count as usize,
            limit,
            last_cached_time,
        );

        // 4. 필요시 데이터 소스에서 새 데이터 가져오기
        if needs_update {
            debug!(
                canonical = %symbol,
                source_symbol = %source_symbol,
                timeframe = %timeframe_to_string(timeframe),
                cached = cached_count,
                requested = limit,
                "캐시 업데이트 시작"
            );

            // 원본 심볼로 데이터 소스 선택, source_symbol로 캐시 저장
            match self.fetch_and_cache(symbol, &source_symbol, timeframe, limit, last_cached_time).await {
                Ok(fetched) => {
                    info!(
                        canonical = %symbol,
                        source_symbol = %source_symbol,
                        fetched = fetched,
                        "데이터 캐시 완료"
                    );
                }
                Err(e) => {
                    warn!(
                        canonical = %symbol,
                        source_symbol = %source_symbol,
                        error = %e,
                        "데이터 가져오기 실패, 캐시 데이터 사용"
                    );
                }
            }
        }

        // 5. 갭 감지
        self.detect_and_warn_gaps(&source_symbol, timeframe, limit).await;

        // 6. 캐시에서 데이터 조회
        let records = self.cache.get_cached_klines(&source_symbol, timeframe, limit).await?;

        // 7. canonical 심볼로 Kline 변환
        let klines: Vec<Kline> = records.into_iter().map(|kline| {
            Kline {
                symbol: Symbol {
                    base: symbol.to_string(),  // canonical 심볼 사용
                    quote: quote_currency.clone(),
                    market_type,
                    exchange_symbol: Some(source_symbol.clone()),
                },
                ..kline
            }
        }).collect();

        debug!(
            canonical = %symbol,
            source_symbol = %source_symbol,
            returned = klines.len(),
            "캔들 데이터 반환"
        );

        Ok(klines)
    }

    /// 심볼을 데이터 소스 형식으로 변환.
    ///
    /// SymbolResolver를 통해 DB에서 조회하고, 없으면 기본 변환 규칙 적용.
    async fn resolve_symbol(&self, canonical: &str) -> (String, String, MarketType) {
        // 1. DB에서 심볼 정보 조회 시도
        if let Ok(Some(info)) = self.symbol_resolver.get_symbol_info(canonical).await {
            let source_symbol = info.yahoo_symbol.unwrap_or_else(|| canonical.to_string());
            let quote = match info.market.as_str() {
                "KR" => "KRW".to_string(),
                "CRYPTO" => "USDT".to_string(),
                _ => "USD".to_string(),
            };
            let market_type = match info.market.as_str() {
                "KR" => MarketType::KrStock,
                "US" => MarketType::UsStock,
                "CRYPTO" => MarketType::Crypto,
                _ => MarketType::Stock,
            };
            return (source_symbol, quote, market_type);
        }

        // 2. 기본 변환 규칙 (DB에 정보가 없는 경우)
        // 6자리 숫자 → 한국 주식
        if canonical.len() == 6 && canonical.chars().all(|c| c.is_ascii_digit()) {
            return (format!("{}.KS", canonical), "KRW".to_string(), MarketType::KrStock);
        }

        // "/" 포함 → 암호화폐
        if canonical.contains('/') {
            let parts: Vec<&str> = canonical.split('/').collect();
            let quote = parts.get(1).map(|s| s.to_string()).unwrap_or_else(|| "USDT".to_string());
            return (canonical.replace("/", "-"), quote, MarketType::Crypto);
        }

        // 기본: 미국 주식
        (canonical.to_string(), "USD".to_string(), MarketType::UsStock)
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
        let (source_symbol, quote_currency, market_type) = self.resolve_symbol(symbol).await;
        let lock_key = format!("{}:{}:range", source_symbol, timeframe_to_string(timeframe));

        info!(
            canonical = %symbol,
            source_symbol = %source_symbol,
            timeframe = %timeframe_to_string(timeframe),
            start = %start_date,
            end = %end_date,
            "날짜 범위 데이터 조회 요청"
        );

        // 1. 동시성 제어: Lock 획득
        let lock = self.get_or_create_lock(&lock_key).await;
        let _guard = lock.write().await;

        // 2. 외부 데이터 소스에서 데이터 가져와 캐시
        let raw_klines = if is_pure_korean_stock_code(symbol) {
            debug!(canonical = symbol, "KRX 데이터 소스 시도 (날짜 범위)");
            match self.fetch_from_krx_range(symbol, timeframe, start_date, end_date).await {
                Ok(data) if !data.is_empty() => {
                    debug!(canonical = symbol, count = data.len(), "KRX 날짜 범위 데이터 성공");
                    data
                }
                Ok(_) | Err(_) => {
                    warn!(canonical = symbol, "KRX 실패, Yahoo Finance Fallback");
                    let provider = YahooProviderWrapper::new()?;
                    provider.get_klines_range(&source_symbol, timeframe, start_date, end_date).await?
                }
            }
        } else {
            debug!(source_symbol = %source_symbol, "Yahoo Finance 날짜 범위 조회");
            let provider = YahooProviderWrapper::new()?;
            provider.get_klines_range(&source_symbol, timeframe, start_date, end_date).await?
        };

        if raw_klines.is_empty() {
            info!(canonical = %symbol, source_symbol = %source_symbol, "날짜 범위에 데이터 없음");
            return Ok(Vec::new());
        }

        // 3. 캐시에 저장
        let saved = self.batch_insert_klines(&source_symbol, timeframe, &raw_klines).await?;
        info!(
            canonical = %symbol,
            source_symbol = %source_symbol,
            fetched = raw_klines.len(),
            saved = saved,
            "날짜 범위 데이터 캐시 완료"
        );

        // 4. canonical 심볼로 Kline 변환
        let klines: Vec<Kline> = raw_klines.into_iter().map(|kline| {
            Kline {
                symbol: Symbol {
                    base: symbol.to_string(),  // canonical 심볼 사용
                    quote: quote_currency.clone(),
                    market_type,
                    exchange_symbol: Some(source_symbol.clone()),
                },
                ..kline
            }
        }).collect();

        Ok(klines)
    }

    /// KRX에서 날짜 범위로 데이터 가져오기.
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

        let krx = KrxDataSource::new();

        let start_str = start_date.format("%Y%m%d").to_string();
        let end_str = end_date.format("%Y%m%d").to_string();

        let klines = krx.get_ohlcv(symbol, &start_str, &end_str).await?;

        debug!(
            symbol = symbol,
            start = %start_str,
            end = %end_str,
            count = klines.len(),
            "KRX 날짜 범위 데이터 가져오기 완료"
        );

        Ok(klines)
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
            debug!(
                symbol = symbol,
                "시장 마감 상태, 캐시 업데이트 스킵"
            );
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
    /// 심볼에 따라 적절한 데이터 소스를 선택합니다:
    /// - 6자리 숫자 (예: 005930) → KRX (한국 주식 우선)
    /// - 그 외 (예: AAPL, 005930.KS) → Yahoo Finance
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
        // 원본 심볼로 데이터 소스 선택
        // 한국 주식: KRX → Yahoo Finance Fallback
        // 해외 주식: Yahoo Finance
        let klines = if is_pure_korean_stock_code(original_symbol) {
            debug!(symbol = original_symbol, "KRX 데이터 소스 시도 (한국 주식)");
            match self.fetch_from_krx(original_symbol, timeframe, limit).await {
                Ok(data) if !data.is_empty() => {
                    debug!(symbol = original_symbol, count = data.len(), "KRX 데이터 가져오기 성공");
                    data
                }
                Ok(_) | Err(_) => {
                    // KRX 실패 시 Yahoo Finance Fallback
                    warn!(symbol = original_symbol, "KRX 데이터 없음/실패, Yahoo Finance로 Fallback");
                    let provider = YahooProviderWrapper::new()?;
                    provider.get_klines_internal(cache_symbol, timeframe, limit).await?
                }
            }
        } else {
            debug!(symbol = cache_symbol, "Yahoo Finance 데이터 소스 사용 (해외 주식)");
            let provider = YahooProviderWrapper::new()?;
            provider.get_klines_internal(cache_symbol, timeframe, limit).await?
        };

        if klines.is_empty() {
            return Ok(0);
        }

        // 증분 업데이트: 마지막 캐시 시간 이후 데이터만 저장
        let new_klines: Vec<Kline> = if let Some(last_time) = last_cached_time {
            klines.into_iter()
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
        let saved = self.batch_insert_klines(cache_symbol, timeframe, &new_klines).await?;
        Ok(saved)
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

        let krx = KrxDataSource::new();

        // 기간 계산 (limit 일수 + 여유분)
        let end_date = Utc::now();
        let start_date = end_date - Duration::days((limit as i64) + 30);

        let start_str = start_date.format("%Y%m%d").to_string();
        let end_str = end_date.format("%Y%m%d").to_string();

        let klines = krx.get_ohlcv(symbol, &start_str, &end_str).await?;

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
                   VALUES "#
            );

            // VALUES 절 구성: ($1, $2, ...), ($10, $11, ...), ...
            let value_tuples: Vec<String> = chunk
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    let base = i * 9;
                    format!(
                        "(${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, NOW())",
                        base + 1, base + 2, base + 3, base + 4, base + 5,
                        base + 6, base + 7, base + 8, base + 9
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
                    fetched_at = NOW()"#
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
        let klines: Vec<Kline> = match self.cache.get_cached_klines(symbol, timeframe, limit).await {
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
        Ok(records.into_iter().map(|r| CacheStats {
            symbol: r.symbol,
            timeframe: r.timeframe,
            first_time: r.first_cached_time,
            last_time: r.last_cached_time,
            candle_count: r.total_candles.unwrap_or(0) as i64,
            last_updated: r.last_updated_at,
        }).collect())
    }

    /// 특정 심볼 캐시 삭제.
    ///
    /// # 인자
    /// - `symbol`: canonical 심볼 (예: "005930", "AAPL")
    pub async fn clear_cache(&self, symbol: &str) -> Result<u64> {
        let (source_symbol, _, _) = self.resolve_symbol(symbol).await;
        self.cache.clear_symbol_cache(&source_symbol).await
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
    let market_open = 9 * 60 + 30;   // 09:30
    let market_close_extended = 17 * 60;  // 17:00

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
    let market_open = 9 * 60;         // 09:00
    let market_close_extended = 16 * 60 + 30;  // 16:30

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
    let market_open = 9 * 60;         // 09:00
    let market_close_extended = 16 * 60;  // 16:00

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
        Timeframe::M1 | Timeframe::M3 | Timeframe::M5 | Timeframe::M15 | Timeframe::M30 |
        Timeframe::H1 | Timeframe::H2 | Timeframe::H4 | Timeframe::H6 | Timeframe::H8 | Timeframe::H12
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
fn guess_currency(symbol: &str) -> &'static str {
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
pub struct YahooProviderWrapper {
    connector: yahoo_finance_api::YahooConnector,
}

impl YahooProviderWrapper {
    pub fn new() -> Result<Self> {
        let connector = yahoo_finance_api::YahooConnector::new()
            .map_err(|e| DataError::ConnectionError(format!("Yahoo Finance 연결 실패: {}", e)))?;
        Ok(Self { connector })
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
            Timeframe::H1 | Timeframe::H2 | Timeframe::H4 |
            Timeframe::H6 | Timeframe::H8 | Timeframe::H12 => "1h",
            Timeframe::D1 | Timeframe::D3 => "1d",
            Timeframe::W1 => "1wk",
            Timeframe::MN1 => "1mo",
        };

        let range = calculate_range_string(timeframe, limit);

        debug!(symbol = symbol, interval = interval, range = range, "Yahoo Finance API 호출");

        let response = self.connector
            .get_quote_range(symbol, interval, range)
            .await
            .map_err(|e| DataError::FetchError(format!("Yahoo Finance API 오류 ({}): {}", symbol, e)))?;

        let quotes = response.quotes()
            .map_err(|e| DataError::ParseError(format!("Quote 파싱 오류: {}", e)))?;

        if quotes.is_empty() {
            return Ok(Vec::new());
        }

        let currency = guess_currency(symbol);
        let symbol_obj = Symbol::stock(symbol, currency);

        let klines: Vec<Kline> = quotes.iter().map(|q| {
            let open_time = Utc.timestamp_opt(q.timestamp as i64, 0)
                .single()
                .unwrap_or_else(|| Utc::now());
            let close_time = open_time + timeframe_to_duration(timeframe);

            Kline {
                symbol: symbol_obj.clone(),
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
        }).collect();

        let mut sorted = klines;
        sorted.sort_by_key(|k| k.open_time);

        if sorted.len() > limit {
            let skip = sorted.len() - limit;
            sorted = sorted.into_iter().skip(skip).collect();
        }

        Ok(sorted)
    }

    /// 날짜 범위로 캔들 데이터 조회.
    pub async fn get_klines_range(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<Kline>> {
        let interval = match timeframe {
            Timeframe::M1 => "1m",
            Timeframe::M3 | Timeframe::M5 => "5m",
            Timeframe::M15 => "15m",
            Timeframe::M30 => "30m",
            Timeframe::H1 | Timeframe::H2 | Timeframe::H4 |
            Timeframe::H6 | Timeframe::H8 | Timeframe::H12 => "1h",
            Timeframe::D1 | Timeframe::D3 => "1d",
            Timeframe::W1 => "1wk",
            Timeframe::MN1 => "1mo",
        };

        // chrono::NaiveDate → time::OffsetDateTime 변환
        let start = naive_date_to_offset_datetime(start_date);
        let end = naive_date_to_offset_datetime(end_date);

        debug!(
            symbol = symbol,
            interval = interval,
            start = %start_date,
            end = %end_date,
            "Yahoo Finance API 날짜 범위 호출"
        );

        let response = self.connector
            .get_quote_history_interval(symbol, start, end, interval)
            .await
            .map_err(|e| DataError::FetchError(format!("Yahoo Finance API 오류 ({}): {}", symbol, e)))?;

        let quotes = response.quotes()
            .map_err(|e| DataError::ParseError(format!("Quote 파싱 오류: {}", e)))?;

        if quotes.is_empty() {
            return Ok(Vec::new());
        }

        let currency = guess_currency(symbol);
        let symbol_obj = Symbol::stock(symbol, currency);

        let klines: Vec<Kline> = quotes.iter().map(|q| {
            let open_time = Utc.timestamp_opt(q.timestamp as i64, 0)
                .single()
                .unwrap_or_else(|| Utc::now());
            let close_time = open_time + timeframe_to_duration(timeframe);

            Kline {
                symbol: symbol_obj.clone(),
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
        }).collect();

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
        Timeframe::M1 | Timeframe::M3 | Timeframe::M5 |
        Timeframe::M15 | Timeframe::M30 => {
            if limit <= 100 { "5d" }
            else if limit <= 500 { "1mo" }
            else { "3mo" }
        }
        Timeframe::H1 | Timeframe::H2 | Timeframe::H4 |
        Timeframe::H6 | Timeframe::H8 | Timeframe::H12 => {
            if limit <= 50 { "5d" }
            else if limit <= 200 { "1mo" }
            else { "3mo" }
        }
        Timeframe::D1 => {
            if limit <= 5 { "5d" }
            else if limit <= 20 { "1mo" }
            else if limit <= 60 { "3mo" }
            else if limit <= 120 { "6mo" }
            else if limit <= 250 { "1y" }
            else if limit <= 500 { "2y" }
            else if limit <= 1250 { "5y" }
            else { "10y" }
        }
        Timeframe::D3 => {
            if limit <= 10 { "1mo" }
            else if limit <= 30 { "3mo" }
            else if limit <= 60 { "6mo" }
            else { "1y" }
        }
        Timeframe::W1 => {
            if limit <= 4 { "1mo" }
            else if limit <= 12 { "3mo" }
            else if limit <= 26 { "6mo" }
            else if limit <= 52 { "1y" }
            else if limit <= 104 { "2y" }
            else { "5y" }
        }
        Timeframe::MN1 => {
            if limit <= 3 { "3mo" }
            else if limit <= 6 { "6mo" }
            else if limit <= 12 { "1y" }
            else if limit <= 24 { "2y" }
            else if limit <= 60 { "5y" }
            else { "10y" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_yahoo_symbol() {
        assert_eq!(to_yahoo_symbol("005930"), "005930.KS");
        assert_eq!(to_yahoo_symbol("AAPL"), "AAPL");
    }

    #[test]
    fn test_is_intraday() {
        assert!(is_intraday(Timeframe::M1));
        assert!(is_intraday(Timeframe::H1));
        assert!(!is_intraday(Timeframe::D1));
    }
}
