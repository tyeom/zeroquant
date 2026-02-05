//! 데이터 임포트 명령어.
//!
//! Yahoo Finance에서 과거 데이터를 다운로드하여 TimescaleDB에 저장합니다.
//!
//! # 사용 예시
//!
//! ```bash
//! # 삼성전자 일봉 데이터를 DB에 저장
//! trader import-db -m KR -s 005930 -f 2024-01-01 -t 2024-12-31
//!
//! # SPY ETF 데이터를 DB에 저장
//! trader import-db -m US -s SPY -f 2024-01-01 -t 2024-12-31
//! ```

use anyhow::{anyhow, Result};
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use indicatif::{ProgressBar, ProgressStyle};
use rust_decimal::Decimal;
use serde::Deserialize;
use std::str::FromStr;
use tracing::{debug, info, warn};

use trader_core::{Kline, Symbol, Timeframe};
use trader_data::{Database, DatabaseConfig, KlineRepository, SymbolRepository};

use crate::commands::download::{Interval, Market};

/// Market을 문자열로 변환
fn market_to_str(market: Market) -> &'static str {
    match market {
        Market::KR => "KR",
        Market::US => "US",
    }
}

/// 데이터베이스 임포트 설정.
#[derive(Debug, Clone)]
pub struct ImportDbConfig {
    pub market: Market,
    pub symbol: String,
    pub interval: Interval,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub is_kosdaq: bool,
    pub db_url: Option<String>,
}

/// Yahoo Finance에서 데이터를 다운로드하여 TimescaleDB에 저장합니다.
pub async fn import_to_db(config: ImportDbConfig) -> Result<usize> {
    info!(
        "Importing {} {} data to database ({} to {})",
        market_to_str(config.market),
        config.symbol,
        config.start_date,
        config.end_date
    );

    // 1. Yahoo Finance에서 데이터 다운로드
    let klines = download_from_yahoo(&config).await?;

    if klines.is_empty() {
        warn!("No data downloaded from Yahoo Finance");
        return Ok(0);
    }

    info!("Downloaded {} candles from Yahoo Finance", klines.len());

    // 2. 데이터베이스 연결
    let db_url = config.db_url.clone().unwrap_or_else(|| {
        std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://trader:trader@localhost:5432/trader".to_string())
    });

    let db_config = DatabaseConfig {
        url: db_url,
        ..Default::default()
    };

    info!("Connecting to database...");
    let db = Database::connect(&db_config).await?;

    // 3. 심볼 조회 또는 생성
    let symbol_repo = SymbolRepository::new(db.clone());
    let kline_repo = KlineRepository::new(db.clone());

    let exchange = match config.market {
        Market::KR => "KIS_KR",
        Market::US => "KIS_US",
    };

    let symbol = create_symbol(&config);
    let market_type_str = match config.market {
        Market::KR => "stock",
        Market::US => "stock",
    };
    let symbol_id = symbol_repo
        .get_or_create(&symbol.base, &symbol.quote, market_type_str, exchange)
        .await?;

    info!("Symbol ID: {} for {}", symbol_id, symbol);

    // 4. 진행률 표시와 함께 일괄 삽입
    let pb = ProgressBar::new(klines.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
            )?
            .progress_chars("#>-"),
    );

    let inserted = kline_repo.insert_batch(symbol_id, &klines).await?;

    pb.finish_with_message("Import completed");

    info!("Successfully imported {} candles to database", inserted);

    Ok(inserted)
}

/// 심볼 객체 생성.
///
/// 시장 정보에 따라 적절한 Symbol 생성자를 사용하여
/// Country 필드가 자동 설정되도록 합니다.
fn create_symbol(config: &ImportDbConfig) -> Symbol {
    match config.market {
        Market::KR => Symbol::kr_stock(config.symbol.to_uppercase(), "KRW"),
        Market::US => Symbol::us_stock(config.symbol.to_uppercase(), "USD"),
    }
}

/// Yahoo Finance에서 OHLCV 데이터 다운로드.
#[allow(clippy::needless_range_loop)]
async fn download_from_yahoo(config: &ImportDbConfig) -> Result<Vec<Kline>> {
    // Yahoo Finance 심볼 변환
    let yahoo_symbol = match config.market {
        Market::KR => {
            let suffix = if config.is_kosdaq { ".KQ" } else { ".KS" };
            format!("{}{}", config.symbol, suffix)
        }
        Market::US => config.symbol.to_uppercase(),
    };

    // 타임스탬프 계산
    let start_ts = config
        .start_date
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();
    let end_ts = config
        .end_date
        .and_hms_opt(23, 59, 59)
        .unwrap()
        .and_utc()
        .timestamp();

    let interval_str = match config.interval {
        Interval::D1 => "1d",
        Interval::W1 => "1wk",
        Interval::M1 => "1mo",
    };

    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{}?period1={}&period2={}&interval={}",
        yahoo_symbol, start_ts, end_ts, interval_str
    );

    debug!("Fetching from Yahoo Finance: {}", url);

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()?;

    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Yahoo Finance API error: {} for symbol {}",
            response.status(),
            yahoo_symbol
        ));
    }

    let data: YahooResponse = response.json().await?;

    // 응답 파싱
    let chart = data
        .chart
        .result
        .first()
        .ok_or_else(|| anyhow!("No data returned for symbol {}", yahoo_symbol))?;

    let timestamps = &chart.timestamp;
    let quotes = chart
        .indicators
        .quote
        .first()
        .ok_or_else(|| anyhow!("No quote data"))?;

    let symbol = create_symbol(config);
    let timeframe = match config.interval {
        Interval::D1 => Timeframe::D1,
        Interval::W1 => Timeframe::W1,
        Interval::M1 => Timeframe::MN1,
    };

    let mut klines = Vec::with_capacity(timestamps.len());

    for i in 0..timestamps.len() {
        // null 값 스킵
        let open = match quotes.open.get(i).and_then(|v| *v) {
            Some(v) => Decimal::from_str(&format!("{:.4}", v))?,
            None => continue,
        };
        let high = match quotes.high.get(i).and_then(|v| *v) {
            Some(v) => Decimal::from_str(&format!("{:.4}", v))?,
            None => continue,
        };
        let low = match quotes.low.get(i).and_then(|v| *v) {
            Some(v) => Decimal::from_str(&format!("{:.4}", v))?,
            None => continue,
        };
        let close = match quotes.close.get(i).and_then(|v| *v) {
            Some(v) => Decimal::from_str(&format!("{:.4}", v))?,
            None => continue,
        };
        let volume = quotes
            .volume
            .get(i)
            .and_then(|v| *v)
            .map(|v| Decimal::from(v as i64))
            .unwrap_or(Decimal::ZERO);

        let open_time = Utc.timestamp_opt(timestamps[i], 0).unwrap();
        let close_time = calculate_close_time(open_time, config.interval);

        klines.push(Kline {
            ticker: symbol.to_string(),
            timeframe,
            open_time,
            open,
            high,
            low,
            close,
            volume,
            close_time,
            quote_volume: None,
            num_trades: None,
        });
    }

    Ok(klines)
}

/// 종가 시간 계산.
fn calculate_close_time(open_time: DateTime<Utc>, interval: Interval) -> DateTime<Utc> {
    match interval {
        Interval::D1 => open_time + chrono::Duration::days(1) - chrono::Duration::seconds(1),
        Interval::W1 => open_time + chrono::Duration::weeks(1) - chrono::Duration::seconds(1),
        Interval::M1 => {
            // 다음 달 1일 - 1초
            let next_month = if open_time.month() == 12 {
                Utc.with_ymd_and_hms(open_time.year() + 1, 1, 1, 0, 0, 0)
                    .unwrap()
            } else {
                Utc.with_ymd_and_hms(open_time.year(), open_time.month() + 1, 1, 0, 0, 0)
                    .unwrap()
            };
            next_month - chrono::Duration::seconds(1)
        }
    }
}

// ==================== Yahoo Finance 응답 타입 ====================

#[derive(Debug, Deserialize)]
struct YahooResponse {
    chart: YahooChart,
}

#[derive(Debug, Deserialize)]
struct YahooChart {
    result: Vec<YahooResult>,
}

#[derive(Debug, Deserialize)]
struct YahooResult {
    timestamp: Vec<i64>,
    indicators: YahooIndicators,
}

#[derive(Debug, Deserialize)]
struct YahooIndicators {
    quote: Vec<YahooQuote>,
}

#[derive(Debug, Deserialize)]
struct YahooQuote {
    open: Vec<Option<f64>>,
    high: Vec<Option<f64>>,
    low: Vec<Option<f64>>,
    close: Vec<Option<f64>>,
    volume: Vec<Option<u64>>,
}

// ==================== 테스트 ====================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Timelike};

    #[test]
    fn test_create_symbol_kr() {
        let config = ImportDbConfig {
            market: Market::KR,
            symbol: "005930".to_string(),
            interval: Interval::D1,
            start_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
            is_kosdaq: false,
            db_url: None,
        };

        let symbol = create_symbol(&config);
        assert_eq!(symbol.base, "005930");
        assert_eq!(symbol.quote, "KRW");
    }

    #[test]
    fn test_create_symbol_us() {
        let config = ImportDbConfig {
            market: Market::US,
            symbol: "spy".to_string(),
            interval: Interval::D1,
            start_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
            is_kosdaq: false,
            db_url: None,
        };

        let symbol = create_symbol(&config);
        assert_eq!(symbol.base, "SPY");
        assert_eq!(symbol.quote, "USD");
    }

    #[test]
    fn test_calculate_close_time_daily() {
        let open_time = Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap();
        let close_time = calculate_close_time(open_time, Interval::D1);

        assert_eq!(close_time.day(), 15);
        assert_eq!(close_time.hour(), 23);
        assert_eq!(close_time.minute(), 59);
        assert_eq!(close_time.second(), 59);
    }
}
