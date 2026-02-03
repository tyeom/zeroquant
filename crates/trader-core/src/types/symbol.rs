//! 심볼 및 시장 유형 정의.
//!
//! 이 모듈은 트레이딩 심볼 관련 타입을 정의합니다:
//! - `MarketType` - 시장 유형 (암호화폐, 주식, 외환 등)
//! - `Symbol` - 거래 가능한 상품을 나타내는 심볼

use serde::{Deserialize, Serialize};
use std::fmt;

/// 시장 유형 분류.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "utoipa-support", derive(utoipa::ToSchema))]
pub enum MarketType {
    /// 암호화폐 현물 시장
    Crypto,
    /// 주식 시장 (일반)
    Stock,
    /// 미국 주식 시장
    UsStock,
    /// 한국 주식 시장
    KrStock,
    /// 외환 시장
    Forex,
    /// 선물/파생상품 시장
    Futures,
}

impl fmt::Display for MarketType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MarketType::Crypto => write!(f, "crypto"),
            MarketType::Stock => write!(f, "stock"),
            MarketType::UsStock => write!(f, "us_stock"),
            MarketType::KrStock => write!(f, "kr_stock"),
            MarketType::Forex => write!(f, "forex"),
            MarketType::Futures => write!(f, "futures"),
        }
    }
}

/// 거래 가능한 상품을 나타내는 트레이딩 심볼.
///
/// 심볼은 기준 자산, 호가 자산, 시장 유형으로 구성됩니다.
/// 예: 암호화폐의 BTC/USDT, 주식의 AAPL/USD.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa-support", derive(utoipa::ToSchema))]
pub struct Symbol {
    /// 기준 자산 (예: BTC, AAPL, EUR)
    pub base: String,
    /// 호가 자산 (예: USDT, USD, JPY)
    pub quote: String,
    /// 시장 유형
    pub market_type: MarketType,
    /// 거래소별 심볼 형식 (선택)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exchange_symbol: Option<String>,
}

impl Symbol {
    /// 새 심볼을 생성합니다.
    pub fn new(base: impl Into<String>, quote: impl Into<String>, market_type: MarketType) -> Self {
        Self {
            base: base.into().to_uppercase(),
            quote: quote.into().to_uppercase(),
            market_type,
            exchange_symbol: None,
        }
    }

    /// 암호화폐 심볼을 생성합니다.
    pub fn crypto(base: impl Into<String>, quote: impl Into<String>) -> Self {
        Self::new(base, quote, MarketType::Crypto)
    }

    /// 주식 심볼을 생성합니다.
    pub fn stock(base: impl Into<String>, quote: impl Into<String>) -> Self {
        Self::new(base, quote, MarketType::Stock)
    }

    /// 외환 심볼을 생성합니다.
    pub fn forex(base: impl Into<String>, quote: impl Into<String>) -> Self {
        Self::new(base, quote, MarketType::Forex)
    }

    /// 거래소별 심볼 형식을 설정합니다.
    pub fn with_exchange_symbol(mut self, exchange_symbol: impl Into<String>) -> Self {
        self.exchange_symbol = Some(exchange_symbol.into());
        self
    }

    /// "BASE/QUOTE" 형식 문자열에서 심볼을 파싱합니다.
    pub fn from_string(s: &str, market_type: MarketType) -> Option<Self> {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() == 2 {
            Some(Self::new(parts[0], parts[1], market_type))
        } else {
            None
        }
    }

    /// 표준 심볼 문자열 형식을 반환합니다.
    pub fn to_standard_string(&self) -> String {
        format!("{}/{}", self.base, self.quote)
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.base, self.quote)
    }
}

/// Yahoo Finance 심볼 변환 유틸리티.
pub struct YahooSymbolConverter;

impl YahooSymbolConverter {
    /// 한국 거래소 판별 (KOSPI/KOSDAQ).
    ///
    /// 종목 코드 첫 글자로 판별:
    /// - `0`: KOSPI
    /// - `1~4`: KOSDAQ
    /// - 기타: KOSDAQ (기본값)
    ///
    /// # Returns
    /// (거래소명, Yahoo 접미사)
    pub fn determine_kr_exchange(ticker: &str) -> (&'static str, &'static str) {
        if ticker.is_empty() {
            return ("KOSPI", ".KS");
        }

        let first_char = ticker.chars().next().unwrap();
        match first_char {
            '0' => ("KOSPI", ".KS"),
            '1'..='4' => ("KOSDAQ", ".KQ"),
            _ => ("KOSDAQ", ".KQ"),
        }
    }

    /// Canonical ticker를 Yahoo Finance 심볼로 변환.
    ///
    /// 시장별 변환 규칙:
    /// - **KR**: 첫 글자로 KOSPI(.KS) / KOSDAQ(.KQ) 판별
    /// - **US**: 그대로 사용
    /// - **CRYPTO**: 그대로 사용 (Yahoo는 암호화폐 미지원)
    /// - **기타**: 그대로 사용
    ///
    /// # Arguments
    /// * `ticker` - Canonical ticker (예: "005930", "AAPL", "BTC/USDT")
    /// * `market` - 시장 코드 (예: "KR", "US", "CRYPTO")
    ///
    /// # Returns
    /// Yahoo Finance 심볼 (예: "005930.KS", "AAPL", "BTC/USDT")
    ///
    /// # Note
    /// 이 함수는 fallback 용도로만 사용하세요.
    /// **DB의 `symbol_info.yahoo_symbol` 컬럼을 최우선으로 사용**해야 합니다.
    pub fn to_yahoo_symbol(ticker: &str, market: &str) -> String {
        // 이미 Yahoo 접미사가 있으면 그대로 반환
        if ticker.ends_with(".KS")
            || ticker.ends_with(".KQ")
            || ticker.ends_with(".AX")
            || ticker.ends_with(".L")
            || ticker.ends_with(".T")
            || ticker.ends_with(".HK")
            || ticker.ends_with(".SI")
            || ticker.ends_with(".TO")
        {
            return ticker.to_string();
        }

        match market.to_uppercase().as_str() {
            "KR" => {
                let (_, suffix) = Self::determine_kr_exchange(ticker);
                format!("{}{}", ticker, suffix)
            }
            _ => ticker.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_creation() {
        let symbol = Symbol::crypto("btc", "usdt");
        assert_eq!(symbol.base, "BTC");
        assert_eq!(symbol.quote, "USDT");
        assert_eq!(symbol.market_type, MarketType::Crypto);
    }

    #[test]
    fn test_symbol_display() {
        let symbol = Symbol::crypto("BTC", "USDT");
        assert_eq!(symbol.to_string(), "BTC/USDT");
    }

    #[test]
    fn test_symbol_from_string() {
        let symbol = Symbol::from_string("ETH/USDT", MarketType::Crypto).unwrap();
        assert_eq!(symbol.base, "ETH");
        assert_eq!(symbol.quote, "USDT");
    }

    #[test]
    fn test_determine_kr_exchange() {
        // KOSPI (0으로 시작)
        assert_eq!(
            YahooSymbolConverter::determine_kr_exchange("005930"),
            ("KOSPI", ".KS")
        );
        assert_eq!(
            YahooSymbolConverter::determine_kr_exchange("000660"),
            ("KOSPI", ".KS")
        );
        assert_eq!(
            YahooSymbolConverter::determine_kr_exchange("03473K"),
            ("KOSPI", ".KS")
        );

        // KOSDAQ (1~4로 시작)
        assert_eq!(
            YahooSymbolConverter::determine_kr_exchange("124560"),
            ("KOSDAQ", ".KQ")
        );
        assert_eq!(
            YahooSymbolConverter::determine_kr_exchange("209640"),
            ("KOSDAQ", ".KQ")
        );
        assert_eq!(
            YahooSymbolConverter::determine_kr_exchange("340930"),
            ("KOSDAQ", ".KQ")
        );
        assert_eq!(
            YahooSymbolConverter::determine_kr_exchange("413390"),
            ("KOSDAQ", ".KQ")
        );

        // 빈 문자열 (기본값: KOSPI)
        assert_eq!(
            YahooSymbolConverter::determine_kr_exchange(""),
            ("KOSPI", ".KS")
        );
    }

    #[test]
    fn test_to_yahoo_symbol() {
        // KR 시장 - KOSPI
        assert_eq!(
            YahooSymbolConverter::to_yahoo_symbol("005930", "KR"),
            "005930.KS"
        );
        assert_eq!(
            YahooSymbolConverter::to_yahoo_symbol("03473K", "KR"),
            "03473K.KS"
        );

        // KR 시장 - KOSDAQ
        assert_eq!(
            YahooSymbolConverter::to_yahoo_symbol("124560", "KR"),
            "124560.KQ"
        );
        assert_eq!(
            YahooSymbolConverter::to_yahoo_symbol("209640", "KR"),
            "209640.KQ"
        );

        // 이미 접미사가 있는 경우 (그대로 반환)
        assert_eq!(
            YahooSymbolConverter::to_yahoo_symbol("005930.KS", "KR"),
            "005930.KS"
        );
        assert_eq!(
            YahooSymbolConverter::to_yahoo_symbol("124560.KQ", "KR"),
            "124560.KQ"
        );

        // US 시장 (그대로 반환)
        assert_eq!(YahooSymbolConverter::to_yahoo_symbol("AAPL", "US"), "AAPL");
        assert_eq!(
            YahooSymbolConverter::to_yahoo_symbol("GOOGL", "US"),
            "GOOGL"
        );

        // CRYPTO 시장 (그대로 반환)
        assert_eq!(
            YahooSymbolConverter::to_yahoo_symbol("BTC/USDT", "CRYPTO"),
            "BTC/USDT"
        );
    }
}
