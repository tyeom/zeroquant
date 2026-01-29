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
}
