//! MacroDataProvider - 매크로 경제 지표 데이터 제공자.
//!
//! Yahoo Finance API를 통해 USD/KRW 환율과 나스닥 지수를 조회합니다.
//!
//! # 지원 심볼
//!
//! - **USD/KRW**: "KRW=X"
//! - **NASDAQ**: "^IXIC"
//!
//! # 사용 예시
//!
//! ```rust,ignore
//! use trader_data::cache::MacroDataProvider;
//!
//! let provider = MacroDataProvider::new()?;
//! let data = provider.fetch_macro_data().await?;
//!
//! println!("USD/KRW: {} ({:+.2}%)", data.usd_krw, data.usd_change_pct);
//! println!("NASDAQ: {:+.2}%", data.nasdaq_change_pct);
//! ```

use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info};
use yahoo_finance_api as yahoo;

/// 매크로 데이터 조회 에러.
#[derive(Debug, Error)]
pub enum MacroDataError {
    #[error("Yahoo Finance 연결 실패: {0}")]
    ConnectionError(String),

    #[error("API 요청 실패 ({symbol}): {message}")]
    ApiError { symbol: String, message: String },

    #[error("데이터 파싱 실패: {0}")]
    ParseError(String),

    #[error("데이터 없음: {0}")]
    NoData(String),
}

/// 매크로 경제 지표 데이터.
///
/// # 필드
///
/// - `usd_krw`: 현재 USD/KRW 환율
/// - `usd_prev_close`: 전일 종가
/// - `usd_change_pct`: 전일 대비 변동률 (%)
/// - `nasdaq_close`: 현재 나스닥 지수
/// - `nasdaq_prev_close`: 전일 종가
/// - `nasdaq_change_pct`: 전일 대비 변동률 (%)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroData {
    /// 현재 USD/KRW 환율
    pub usd_krw: Decimal,

    /// 전일 USD/KRW 종가
    pub usd_prev_close: Decimal,

    /// 전일 대비 환율 변동률 (%)
    pub usd_change_pct: f64,

    /// 현재 나스닥 지수
    pub nasdaq_close: Decimal,

    /// 전일 나스닥 종가
    pub nasdaq_prev_close: Decimal,

    /// 전일 대비 나스닥 변동률 (%)
    pub nasdaq_change_pct: f64,
}

impl MacroData {
    /// 변동률 계산 (%)
    fn calculate_change_pct(current: Decimal, previous: Decimal) -> f64 {
        if previous.is_zero() {
            return 0.0;
        }

        let change = current - previous;
        let pct = (change / previous) * Decimal::from(100);

        pct.to_string()
            .parse::<f64>()
            .unwrap_or(0.0)
    }
}

/// 매크로 데이터 제공자 트레잇.
#[async_trait]
pub trait MacroDataProviderTrait: Send + Sync {
    /// 매크로 경제 지표 데이터 조회.
    async fn fetch_macro_data(&self) -> Result<MacroData, MacroDataError>;
}

/// Yahoo Finance 기반 매크로 데이터 제공자.
pub struct MacroDataProvider {
    connector: yahoo::YahooConnector,
}

impl MacroDataProvider {
    /// 새로운 MacroDataProvider 생성.
    pub fn new() -> Result<Self, MacroDataError> {
        let connector = yahoo::YahooConnector::new()
            .map_err(|e| MacroDataError::ConnectionError(format!("{}", e)))?;

        Ok(Self { connector })
    }

    /// 심볼의 최근 2일 데이터 조회 (현재가 + 전일 종가).
    async fn fetch_quotes(&self, symbol: &str) -> Result<Vec<yahoo::Quote>, MacroDataError> {
        info!("매크로 데이터 조회: {}", symbol);

        // 최근 5일 데이터 조회 (주말 등을 고려하여 여유 있게)
        let response = self
            .connector
            .get_quote_range(symbol, "1d", "5d")
            .await
            .map_err(|e| MacroDataError::ApiError {
                symbol: symbol.to_string(),
                message: format!("{}", e),
            })?;

        let quotes = response
            .quotes()
            .map_err(|e| MacroDataError::ParseError(format!("{}", e)))?;

        if quotes.is_empty() {
            return Err(MacroDataError::NoData(format!("심볼 {} 데이터 없음", symbol)));
        }

        debug!("{} 캔들 {} 개 수신", symbol, quotes.len());
        Ok(quotes)
    }

    /// USD/KRW 환율 데이터 조회.
    async fn fetch_usd_krw(&self) -> Result<(Decimal, Decimal), MacroDataError> {
        let quotes = self.fetch_quotes("KRW=X").await?;

        if quotes.len() < 2 {
            return Err(MacroDataError::NoData(
                "USD/KRW 이전 데이터 부족 (최소 2개 필요)".to_string(),
            ));
        }

        // 최근 2개 데이터 추출 (마지막이 최신)
        let current = &quotes[quotes.len() - 1];
        let previous = &quotes[quotes.len() - 2];

        let current_price = Decimal::from_f64_retain(current.close)
            .ok_or_else(|| MacroDataError::ParseError("USD/KRW 현재가 변환 실패".to_string()))?;

        let prev_price = Decimal::from_f64_retain(previous.close)
            .ok_or_else(|| MacroDataError::ParseError("USD/KRW 전일가 변환 실패".to_string()))?;

        debug!("USD/KRW: {} (전일: {})", current_price, prev_price);
        Ok((current_price, prev_price))
    }

    /// 나스닥 지수 데이터 조회.
    async fn fetch_nasdaq(&self) -> Result<(Decimal, Decimal), MacroDataError> {
        let quotes = self.fetch_quotes("^IXIC").await?;

        if quotes.len() < 2 {
            return Err(MacroDataError::NoData(
                "나스닥 이전 데이터 부족 (최소 2개 필요)".to_string(),
            ));
        }

        // 최근 2개 데이터 추출
        let current = &quotes[quotes.len() - 1];
        let previous = &quotes[quotes.len() - 2];

        let current_price = Decimal::from_f64_retain(current.close)
            .ok_or_else(|| MacroDataError::ParseError("나스닥 현재가 변환 실패".to_string()))?;

        let prev_price = Decimal::from_f64_retain(previous.close)
            .ok_or_else(|| MacroDataError::ParseError("나스닥 전일가 변환 실패".to_string()))?;

        debug!("NASDAQ: {} (전일: {})", current_price, prev_price);
        Ok((current_price, prev_price))
    }
}

#[async_trait]
impl MacroDataProviderTrait for MacroDataProvider {
    async fn fetch_macro_data(&self) -> Result<MacroData, MacroDataError> {
        info!("매크로 경제 지표 데이터 수집 시작");

        // USD/KRW 환율 조회
        let (usd_krw, usd_prev_close) = self.fetch_usd_krw().await?;
        let usd_change_pct = MacroData::calculate_change_pct(usd_krw, usd_prev_close);

        // 나스닥 지수 조회
        let (nasdaq_close, nasdaq_prev_close) = self.fetch_nasdaq().await?;
        let nasdaq_change_pct = MacroData::calculate_change_pct(nasdaq_close, nasdaq_prev_close);

        let data = MacroData {
            usd_krw,
            usd_prev_close,
            usd_change_pct,
            nasdaq_close,
            nasdaq_prev_close,
            nasdaq_change_pct,
        };

        info!(
            "매크로 데이터 수집 완료: USD/KRW {} ({:+.2}%), NASDAQ {:+.2}%",
            data.usd_krw, data.usd_change_pct, data.nasdaq_change_pct
        );

        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_change_pct() {
        // 정상 상승
        let current = Decimal::from(1350);
        let previous = Decimal::from(1300);
        let pct = MacroData::calculate_change_pct(current, previous);
        assert!((pct - 3.846).abs() < 0.01); // ~3.846%

        // 정상 하락
        let current = Decimal::from(1250);
        let previous = Decimal::from(1300);
        let pct = MacroData::calculate_change_pct(current, previous);
        assert!((pct + 3.846).abs() < 0.01); // ~-3.846%

        // 변동 없음
        let current = Decimal::from(1300);
        let previous = Decimal::from(1300);
        let pct = MacroData::calculate_change_pct(current, previous);
        assert_eq!(pct, 0.0);

        // 0으로 나누기 방지
        let current = Decimal::from(100);
        let previous = Decimal::ZERO;
        let pct = MacroData::calculate_change_pct(current, previous);
        assert_eq!(pct, 0.0);
    }

    #[tokio::test]
    #[ignore] // 실제 API 호출 필요
    async fn test_fetch_macro_data_integration() {
        let provider = MacroDataProvider::new().expect("Provider 생성 실패");
        let result = provider.fetch_macro_data().await;

        match result {
            Ok(data) => {
                println!("USD/KRW: {} ({:+.2}%)", data.usd_krw, data.usd_change_pct);
                println!(
                    "NASDAQ: {} ({:+.2}%)",
                    data.nasdaq_close, data.nasdaq_change_pct
                );
                assert!(data.usd_krw > Decimal::ZERO);
                assert!(data.nasdaq_close > Decimal::ZERO);
            }
            Err(e) => {
                eprintln!("API 호출 실패: {}", e);
            }
        }
    }
}
