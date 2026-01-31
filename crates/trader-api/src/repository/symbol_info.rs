//! 심볼 정보 저장소.
//!
//! 티커-회사명 매핑 및 검색 기능을 제공합니다.
//! DB에 없는 종목은 KRX 또는 Yahoo Finance에서 자동으로 조회하여 저장합니다.

use chrono::{DateTime, Utc};
use reqwest::Client;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::symbol_fundamental::{NewSymbolFundamental, SymbolFundamentalRepository};

/// Fundamental 정보 갱신 주기 (일).
/// 이 기간이 지난 Fundamental 정보는 새로 조회합니다.
pub const FUNDAMENTAL_REFRESH_DAYS: i64 = 7;

/// 심볼 정보 레코드.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SymbolInfo {
    pub id: Uuid,
    pub ticker: String,
    pub name: String,
    pub name_en: Option<String>,
    pub market: String,
    pub exchange: Option<String>,
    pub sector: Option<String>,
    pub yahoo_symbol: Option<String>,
    pub is_active: Option<bool>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// 검색 결과용 간소화된 심볼 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolSearchResult {
    pub ticker: String,
    pub name: String,
    pub market: String,
    pub yahoo_symbol: Option<String>,
}

/// 새 심볼 정보 삽입용.
#[derive(Debug, Clone)]
pub struct NewSymbolInfo {
    pub ticker: String,
    pub name: String,
    pub name_en: Option<String>,
    pub market: String,
    pub exchange: Option<String>,
    pub sector: Option<String>,
    pub yahoo_symbol: Option<String>,
}

/// 심볼 정보 저장소.
pub struct SymbolInfoRepository;

impl SymbolInfoRepository {
    /// 심볼 검색 (티커 + 회사명).
    ///
    /// 검색어가 티커나 회사명에 포함되면 매칭됩니다.
    pub async fn search(
        pool: &PgPool,
        query: &str,
        limit: i64,
    ) -> Result<Vec<SymbolSearchResult>, sqlx::Error> {
        let query_upper = query.to_uppercase();
        let query_pattern = format!("%{}%", query_upper);

        let results = sqlx::query_as::<_, (String, String, String, Option<String>)>(
            r#"
            SELECT ticker, name, market, yahoo_symbol
            FROM symbol_info
            WHERE is_active = true
              AND (
                  UPPER(ticker) LIKE $1
                  OR UPPER(name) LIKE $1
                  OR UPPER(COALESCE(name_en, '')) LIKE $1
              )
            ORDER BY
                CASE WHEN UPPER(ticker) = $2 THEN 0
                     WHEN UPPER(ticker) LIKE $3 THEN 1
                     ELSE 2
                END,
                ticker
            LIMIT $4
            "#,
        )
        .bind(&query_pattern)
        .bind(&query_upper)
        .bind(format!("{}%", query_upper))
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(results
            .into_iter()
            .map(|(ticker, name, market, yahoo_symbol)| SymbolSearchResult {
                ticker,
                name,
                market,
                yahoo_symbol,
            })
            .collect())
    }

    /// 티커로 심볼 정보 조회.
    pub async fn get_by_ticker(
        pool: &PgPool,
        ticker: &str,
        market: Option<&str>,
    ) -> Result<Option<SymbolInfo>, sqlx::Error> {
        let mut query = String::from(
            "SELECT * FROM symbol_info WHERE UPPER(ticker) = UPPER($1) AND is_active = true",
        );

        if market.is_some() {
            query.push_str(" AND market = $2");
        }

        query.push_str(" LIMIT 1");

        if let Some(m) = market {
            sqlx::query_as::<_, SymbolInfo>(&query)
                .bind(ticker)
                .bind(m)
                .fetch_optional(pool)
                .await
        } else {
            sqlx::query_as::<_, SymbolInfo>(&query)
                .bind(ticker)
                .fetch_optional(pool)
                .await
        }
    }

    /// Yahoo 심볼로 조회.
    pub async fn get_by_yahoo_symbol(
        pool: &PgPool,
        yahoo_symbol: &str,
    ) -> Result<Option<SymbolInfo>, sqlx::Error> {
        sqlx::query_as::<_, SymbolInfo>(
            "SELECT * FROM symbol_info WHERE yahoo_symbol = $1 AND is_active = true LIMIT 1",
        )
        .bind(yahoo_symbol)
        .fetch_optional(pool)
        .await
    }

    /// 심볼 정보 일괄 삽입 (upsert).
    pub async fn upsert_batch(
        pool: &PgPool,
        symbols: &[NewSymbolInfo],
    ) -> Result<usize, sqlx::Error> {
        if symbols.is_empty() {
            return Ok(0);
        }

        let mut inserted = 0;

        for symbol in symbols {
            let result = sqlx::query(
                r#"
                INSERT INTO symbol_info (ticker, name, name_en, market, exchange, sector, yahoo_symbol)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (ticker, market) DO UPDATE SET
                    name = EXCLUDED.name,
                    name_en = EXCLUDED.name_en,
                    exchange = EXCLUDED.exchange,
                    sector = EXCLUDED.sector,
                    yahoo_symbol = EXCLUDED.yahoo_symbol,
                    updated_at = NOW()
                "#,
            )
            .bind(&symbol.ticker)
            .bind(&symbol.name)
            .bind(&symbol.name_en)
            .bind(&symbol.market)
            .bind(&symbol.exchange)
            .bind(&symbol.sector)
            .bind(&symbol.yahoo_symbol)
            .execute(pool)
            .await?;

            if result.rows_affected() > 0 {
                inserted += 1;
            }
        }

        Ok(inserted)
    }

    /// 시장별 심볼 수 조회.
    pub async fn count_by_market(pool: &PgPool) -> Result<Vec<(String, i64)>, sqlx::Error> {
        sqlx::query_as::<_, (String, i64)>(
            "SELECT market, COUNT(*) FROM symbol_info WHERE is_active = true GROUP BY market ORDER BY market",
        )
        .fetch_all(pool)
        .await
    }

    /// 전체 심볼 수 조회.
    pub async fn count_all(pool: &PgPool) -> Result<i64, sqlx::Error> {
        let row = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM symbol_info WHERE is_active = true",
        )
        .fetch_one(pool)
        .await?;

        Ok(row)
    }

    /// 단일 심볼 정보 삽입 (upsert) 및 ID 반환.
    pub async fn upsert_single(
        pool: &PgPool,
        symbol: &NewSymbolInfo,
    ) -> Result<Uuid, sqlx::Error> {
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO symbol_info (ticker, name, name_en, market, exchange, sector, yahoo_symbol)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (ticker, market) DO UPDATE SET
                name = EXCLUDED.name,
                name_en = EXCLUDED.name_en,
                exchange = EXCLUDED.exchange,
                sector = EXCLUDED.sector,
                yahoo_symbol = EXCLUDED.yahoo_symbol,
                updated_at = NOW()
            RETURNING id
            "#,
        )
        .bind(&symbol.ticker)
        .bind(&symbol.name)
        .bind(&symbol.name_en)
        .bind(&symbol.market)
        .bind(&symbol.exchange)
        .bind(&symbol.sector)
        .bind(&symbol.yahoo_symbol)
        .fetch_one(pool)
        .await?;

        Ok(id)
    }

    /// 티커로 조회, 없으면 외부 API에서 가져와서 저장.
    /// Fundamental 정보가 오래된 경우(FUNDAMENTAL_REFRESH_DAYS 초과) 자동으로 갱신합니다.
    ///
    /// 한국 주식(6자리 숫자)은 KRX에서, 그 외는 Yahoo Finance에서 조회합니다.
    pub async fn get_or_fetch(
        pool: &PgPool,
        ticker: &str,
    ) -> Result<Option<SymbolSearchResult>, ExternalFetchError> {
        // 1. DB에서 심볼 정보와 Fundamental 정보 함께 조회
        let query_upper = ticker.to_uppercase();
        let existing = sqlx::query_as::<_, (Uuid, String, String, String, Option<String>)>(
            r#"
            SELECT id, ticker, name, market, yahoo_symbol
            FROM symbol_info
            WHERE UPPER(ticker) = $1 AND is_active = true
            LIMIT 1
            "#,
        )
        .bind(&query_upper)
        .fetch_optional(pool)
        .await
        .map_err(ExternalFetchError::Database)?;

        if let Some((symbol_id, ticker_db, name, market, yahoo_symbol)) = existing {
            // Fundamental 정보 갱신 주기 확인
            let should_refresh = Self::should_refresh_fundamental(pool, symbol_id).await;

            if should_refresh {
                debug!(ticker = %ticker_db, "Fundamental 정보 갱신 필요, 외부 API에서 조회");

                // 백그라운드에서 갱신 (조회 결과는 즉시 반환)
                let pool_clone = pool.clone();
                let ticker_clone = ticker_db.clone();
                tokio::spawn(async move {
                    if let Err(e) = Self::refresh_fundamental(&pool_clone, symbol_id, &ticker_clone).await {
                        warn!(ticker = %ticker_clone, error = %e, "Fundamental 갱신 실패");
                    }
                });
            }

            return Ok(Some(SymbolSearchResult {
                ticker: ticker_db,
                name,
                market,
                yahoo_symbol,
            }));
        }

        // 2. 외부 API에서 조회 (신규 심볼)
        debug!(ticker = ticker, "DB에 없음, 외부 API에서 조회 시도");

        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| ExternalFetchError::Network(e.to_string()))?;

        // 한국 주식인지 판단 (6자리 숫자)
        let is_korean = ticker.len() == 6 && ticker.chars().all(|c| c.is_ascii_digit());

        let result = if is_korean {
            Self::fetch_from_krx(&client, ticker).await
        } else {
            Self::fetch_from_yahoo(&client, ticker).await
        };

        match result {
            Ok(Some((symbol_info, fundamental))) => {
                // DB에 저장
                info!(
                    ticker = ticker,
                    name = symbol_info.name,
                    source = if is_korean { "KRX" } else { "Yahoo" },
                    "외부 API에서 종목 정보 획득, DB에 저장"
                );

                let symbol_id = Self::upsert_single(pool, &symbol_info)
                    .await
                    .map_err(ExternalFetchError::Database)?;

                // Fundamental 정보가 있으면 저장
                if let Some(mut fund) = fundamental {
                    fund.symbol_info_id = symbol_id;
                    if let Err(e) = SymbolFundamentalRepository::upsert(pool, &fund).await {
                        warn!(error = %e, "Fundamental 정보 저장 실패");
                    }
                }

                Ok(Some(SymbolSearchResult {
                    ticker: symbol_info.ticker,
                    name: symbol_info.name,
                    market: symbol_info.market,
                    yahoo_symbol: symbol_info.yahoo_symbol,
                }))
            }
            Ok(None) => {
                debug!(ticker = ticker, "외부 API에서도 종목 정보를 찾을 수 없음");
                Ok(None)
            }
            Err(e) => {
                warn!(ticker = ticker, error = %e, "외부 API 조회 실패");
                Err(e)
            }
        }
    }

    /// KRX에서 한국 주식 정보 조회.
    async fn fetch_from_krx(
        client: &Client,
        ticker: &str,
    ) -> Result<Option<(NewSymbolInfo, Option<NewSymbolFundamental>)>, ExternalFetchError> {
        #[derive(Debug, Deserialize)]
        struct KrxResponse {
            #[serde(rename = "OutBlock_1")]
            out_block: Option<Vec<KrxStock>>,
        }

        #[derive(Debug, Deserialize)]
        struct KrxStock {
            #[serde(rename = "ISU_SRT_CD")]
            ticker: String,
            #[serde(rename = "ISU_ABBRV")]
            name: String,
            #[serde(rename = "ISU_ENG_NM", default)]
            name_en: Option<String>,
            #[serde(rename = "MKT_NM", default)]
            market_name: Option<String>,
            #[serde(rename = "SECT_TP_NM", default)]
            sector: Option<String>,
        }

        // KOSPI와 KOSDAQ 둘 다 시도
        for (market_code, exchange, suffix) in [("STK", "KRX", ".KS"), ("KSQ", "KOSDAQ", ".KQ")] {
            let params = [
                ("bld", "dbms/MDC/STAT/standard/MDCSTAT01501"),
                ("mktId", market_code),
                ("share", "1"),
                ("csvxls_isNo", "false"),
            ];

            let response = client
                .post("http://data.krx.co.kr/comm/bldAttendant/getJsonData.cmd")
                .form(&params)
                .send()
                .await
                .map_err(|e| ExternalFetchError::Network(e.to_string()))?;

            if !response.status().is_success() {
                continue;
            }

            let data: KrxResponse = response
                .json()
                .await
                .map_err(|e| ExternalFetchError::Parse(e.to_string()))?;

            if let Some(stocks) = data.out_block {
                // 티커 매칭
                if let Some(stock) = stocks.into_iter().find(|s| s.ticker == ticker) {
                    let symbol_info = NewSymbolInfo {
                        ticker: stock.ticker.clone(),
                        name: stock.name,
                        name_en: stock.name_en,
                        market: "KR".to_string(),
                        exchange: Some(exchange.to_string()),
                        sector: stock.sector,
                        yahoo_symbol: Some(format!("{}{}", stock.ticker, suffix)),
                    };

                    // TODO: KRX에서 Fundamental 데이터도 조회하여 반환
                    // 현재는 심볼 정보만 반환
                    return Ok(Some((symbol_info, None)));
                }
            }
        }

        Ok(None)
    }

    /// Yahoo Finance에서 주식 정보 조회.
    async fn fetch_from_yahoo(
        client: &Client,
        ticker: &str,
    ) -> Result<Option<(NewSymbolInfo, Option<NewSymbolFundamental>)>, ExternalFetchError> {
        // Yahoo Finance API v1 quote 엔드포인트
        let url = format!(
            "https://query1.finance.yahoo.com/v7/finance/quote?symbols={}",
            ticker
        );

        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| ExternalFetchError::Network(e.to_string()))?;

        if !response.status().is_success() {
            return Ok(None);
        }

        #[derive(Debug, Deserialize)]
        struct YahooQuoteResponse {
            #[serde(rename = "quoteResponse")]
            quote_response: QuoteResponseInner,
        }

        #[derive(Debug, Deserialize)]
        struct QuoteResponseInner {
            result: Vec<YahooQuote>,
        }

        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct YahooQuote {
            symbol: String,
            #[serde(default)]
            short_name: Option<String>,
            #[serde(default)]
            long_name: Option<String>,
            #[serde(default)]
            exchange: Option<String>,
            #[serde(default)]
            quote_type: Option<String>,
            #[serde(default)]
            market: Option<String>,
            #[serde(default)]
            currency: Option<String>,
            // Fundamental 데이터
            #[serde(default)]
            market_cap: Option<f64>,
            #[serde(default)]
            trailing_pe: Option<f64>,
            #[serde(default)]
            forward_pe: Option<f64>,
            #[serde(default)]
            price_to_book: Option<f64>,
            #[serde(default)]
            trailing_eps: Option<f64>,
            #[serde(default)]
            book_value: Option<f64>,
            #[serde(default)]
            dividend_yield: Option<f64>,
            #[serde(default)]
            fifty_two_week_high: Option<f64>,
            #[serde(default)]
            fifty_two_week_low: Option<f64>,
            #[serde(default)]
            average_daily_volume_10_day: Option<i64>,
            #[serde(default)]
            average_daily_volume_3_month: Option<i64>,
            #[serde(default)]
            shares_outstanding: Option<i64>,
            #[serde(default)]
            float_shares: Option<i64>,
        }

        let data: YahooQuoteResponse = response
            .json()
            .await
            .map_err(|e| ExternalFetchError::Parse(e.to_string()))?;

        if let Some(quote) = data.quote_response.result.into_iter().next() {
            let name = quote
                .long_name
                .or(quote.short_name)
                .unwrap_or_else(|| ticker.to_string());

            // 시장 결정
            let market = if quote.symbol.ends_with(".KS") || quote.symbol.ends_with(".KQ") {
                "KR"
            } else if quote.symbol.ends_with(".T") {
                "JP"
            } else if quote.symbol.ends_with(".L") {
                "UK"
            } else if quote.symbol.contains("-") && quote.quote_type.as_deref() == Some("CRYPTOCURRENCY") {
                "CRYPTO"
            } else {
                "US"
            };

            let symbol_info = NewSymbolInfo {
                ticker: ticker.to_uppercase(),
                name: name.clone(),
                name_en: Some(name),
                market: market.to_string(),
                exchange: quote.exchange,
                sector: None, // Yahoo quote API에서는 섹터 정보 미제공
                yahoo_symbol: Some(quote.symbol),
            };

            // Fundamental 데이터 생성
            let fundamental = NewSymbolFundamental {
                symbol_info_id: Uuid::nil(), // 나중에 설정됨
                market_cap: quote.market_cap.map(|v| Decimal::from_f64_retain(v).unwrap_or_default()),
                shares_outstanding: quote.shares_outstanding,
                float_shares: quote.float_shares,
                week_52_high: quote.fifty_two_week_high.map(|v| Decimal::from_f64_retain(v).unwrap_or_default()),
                week_52_low: quote.fifty_two_week_low.map(|v| Decimal::from_f64_retain(v).unwrap_or_default()),
                avg_volume_10d: quote.average_daily_volume_10_day,
                avg_volume_3m: quote.average_daily_volume_3_month,
                per: quote.trailing_pe.map(|v| Decimal::from_f64_retain(v).unwrap_or_default()),
                forward_per: quote.forward_pe.map(|v| Decimal::from_f64_retain(v).unwrap_or_default()),
                pbr: quote.price_to_book.map(|v| Decimal::from_f64_retain(v).unwrap_or_default()),
                eps: quote.trailing_eps.map(|v| Decimal::from_f64_retain(v).unwrap_or_default()),
                bps: quote.book_value.map(|v| Decimal::from_f64_retain(v).unwrap_or_default()),
                dividend_yield: quote.dividend_yield.map(|v| Decimal::from_f64_retain(v * 100.0).unwrap_or_default()), // % 변환
                data_source: Some("Yahoo".to_string()),
                currency: quote.currency,
                ..Default::default()
            };

            return Ok(Some((symbol_info, Some(fundamental))));
        }

        Ok(None)
    }

    /// Fundamental 정보 갱신 필요 여부 확인.
    ///
    /// fetched_at이 없거나 FUNDAMENTAL_REFRESH_DAYS보다 오래된 경우 true 반환.
    async fn should_refresh_fundamental(pool: &PgPool, symbol_info_id: Uuid) -> bool {
        let result = sqlx::query_scalar::<_, Option<DateTime<Utc>>>(
            "SELECT fetched_at FROM symbol_fundamental WHERE symbol_info_id = $1",
        )
        .bind(symbol_info_id)
        .fetch_optional(pool)
        .await;

        match result {
            Ok(Some(Some(fetched_at))) => {
                let now = Utc::now();
                let age_days = (now - fetched_at).num_days();
                age_days >= FUNDAMENTAL_REFRESH_DAYS
            }
            Ok(Some(None)) => true, // fetched_at이 NULL
            Ok(None) => true,       // Fundamental 레코드가 없음
            Err(_) => false,        // DB 오류 시 갱신 안 함
        }
    }

    /// Fundamental 정보 갱신.
    ///
    /// 외부 API에서 최신 정보를 가져와 DB에 저장합니다.
    async fn refresh_fundamental(
        pool: &PgPool,
        symbol_info_id: Uuid,
        ticker: &str,
    ) -> Result<(), ExternalFetchError> {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| ExternalFetchError::Network(e.to_string()))?;

        // 한국 주식인지 판단 (6자리 숫자)
        let is_korean = ticker.len() == 6 && ticker.chars().all(|c| c.is_ascii_digit());

        let result = if is_korean {
            // KRX는 현재 종목 정보만 제공, Fundamental은 Yahoo에서 조회
            let yahoo_ticker = format!("{}.KS", ticker);
            Self::fetch_from_yahoo(&client, &yahoo_ticker).await
        } else {
            Self::fetch_from_yahoo(&client, ticker).await
        };

        match result {
            Ok(Some((_, Some(mut fundamental)))) => {
                fundamental.symbol_info_id = symbol_info_id;
                SymbolFundamentalRepository::upsert(pool, &fundamental)
                    .await
                    .map_err(ExternalFetchError::Database)?;

                info!(ticker = ticker, "Fundamental 정보 갱신 완료");
                Ok(())
            }
            Ok(Some((_, None))) => {
                debug!(ticker = ticker, "Fundamental 정보 없음");
                Ok(())
            }
            Ok(None) => {
                debug!(ticker = ticker, "종목 정보 조회 실패");
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

/// 외부 API 조회 에러.
#[derive(Debug, thiserror::Error)]
pub enum ExternalFetchError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Network error: {0}")]
    Network(String),
    #[error("Parse error: {0}")]
    Parse(String),
}
