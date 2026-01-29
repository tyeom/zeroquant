//! 단순 이동평균 크로스오버 전략.
//!
//! 단기 이동평균이 장기 이동평균을 상향 돌파하면 매수,
//! 하향 돌파하면 매도하는 클래식한 추세 추종 전략입니다.
//!
//! # 전략 로직
//! - 골든 크로스 (단기 SMA > 장기 SMA): 매수 신호
//! - 데드 크로스 (단기 SMA < 장기 SMA): 매도 신호

use crate::strategies::common::deserialize_symbol;
use crate::Strategy;
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;
use trader_core::{MarketData, MarketDataType, MarketType, Order, Position, Side, Signal, SignalType, Symbol};
use tracing::info;

/// SMA 크로스오버 전략 설정.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SmaConfig {
    /// 거래할 심볼
    #[serde(deserialize_with = "deserialize_symbol")]
    pub symbol: String,

    /// 단기 이동평균 기간
    #[serde(default = "default_short_period")]
    pub short_period: usize,

    /// 장기 이동평균 기간
    #[serde(default = "default_long_period")]
    pub long_period: usize,

    /// 거래 금액
    #[serde(default = "default_amount")]
    pub amount: Decimal,
}

fn default_short_period() -> usize {
    10
}

fn default_long_period() -> usize {
    20
}

fn default_amount() -> Decimal {
    Decimal::from(100000)
}

impl Default for SmaConfig {
    fn default() -> Self {
        Self {
            symbol: "BTC/USDT".to_string(),
            short_period: 10,
            long_period: 20,
            amount: Decimal::from(100000),
        }
    }
}

/// SMA 크로스오버 전략.
pub struct SmaStrategy {
    /// 설정
    config: Option<SmaConfig>,
    /// 심볼
    symbol: Option<Symbol>,
    /// 가격 히스토리
    prices: VecDeque<Decimal>,
    /// 포지션 오픈 여부
    position_open: bool,
    /// 이전 단기 SMA
    prev_short_sma: Option<Decimal>,
    /// 이전 장기 SMA
    prev_long_sma: Option<Decimal>,
    /// 초기화 플래그
    initialized: bool,
}

impl SmaStrategy {
    /// 새 SMA 전략 생성.
    pub fn new() -> Self {
        Self {
            config: None,
            symbol: None,
            prices: VecDeque::new(),
            position_open: false,
            prev_short_sma: None,
            prev_long_sma: None,
            initialized: false,
        }
    }

    /// SMA 계산.
    fn calculate_sma(&self, period: usize) -> Option<Decimal> {
        if self.prices.len() < period {
            return None;
        }

        let sum: Decimal = self.prices.iter().take(period).sum();
        Some(sum / Decimal::from(period))
    }
}

impl Default for SmaStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for SmaStrategy {
    fn name(&self) -> &str {
        "SMA Crossover"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "Simple Moving Average crossover strategy. Buy on golden cross, sell on death cross."
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let sma_config: SmaConfig = serde_json::from_value(config)?;

        info!(
            symbol = %sma_config.symbol,
            short_period = sma_config.short_period,
            long_period = sma_config.long_period,
            "Initializing SMA Crossover strategy"
        );

        self.symbol = Symbol::from_string(&sma_config.symbol, MarketType::Stock);
        self.config = Some(sma_config);
        self.prices.clear();
        self.position_open = false;
        self.prev_short_sma = None;
        self.prev_long_sma = None;
        self.initialized = true;

        Ok(())
    }

    async fn on_market_data(
        &mut self,
        data: &MarketData,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
        if !self.initialized {
            return Ok(vec![]);
        }

        let config = self.config.as_ref().unwrap();

        // 심볼 확인
        if data.symbol.to_string() != config.symbol {
            return Ok(vec![]);
        }

        // 가격 추출
        let price = match &data.data {
            MarketDataType::Kline(kline) => kline.close,
            MarketDataType::Ticker(ticker) => ticker.last,
            MarketDataType::Trade(trade) => trade.price,
            _ => return Ok(vec![]),
        };

        // 가격 히스토리 업데이트
        self.prices.push_front(price);
        let max_len = config.long_period + 1;
        while self.prices.len() > max_len {
            self.prices.pop_back();
        }

        // SMA 계산
        let short_sma = match self.calculate_sma(config.short_period) {
            Some(sma) => sma,
            None => return Ok(vec![]),
        };

        let long_sma = match self.calculate_sma(config.long_period) {
            Some(sma) => sma,
            None => return Ok(vec![]),
        };

        let mut signals = vec![];

        // 크로스오버 감지 (이전 값이 있을 때만)
        if let (Some(prev_short), Some(prev_long)) = (self.prev_short_sma, self.prev_long_sma) {
            // 골든 크로스: 단기가 장기를 상향 돌파
            let golden_cross = prev_short <= prev_long && short_sma > long_sma;
            // 데드 크로스: 단기가 장기를 하향 돌파
            let death_cross = prev_short >= prev_long && short_sma < long_sma;

            if golden_cross && !self.position_open {
                info!(
                    short_sma = %short_sma,
                    long_sma = %long_sma,
                    price = %price,
                    "Golden cross - BUY signal"
                );

                signals.push(
                    Signal::new("sma_crossover", data.symbol.clone(), Side::Buy, SignalType::Entry)
                        .with_strength(1.0)
                        .with_prices(Some(price), None, None)
                        .with_metadata("short_sma", json!(short_sma.to_string()))
                        .with_metadata("long_sma", json!(long_sma.to_string()))
                );
                self.position_open = true;
            } else if death_cross && self.position_open {
                info!(
                    short_sma = %short_sma,
                    long_sma = %long_sma,
                    price = %price,
                    "Death cross - SELL signal"
                );

                signals.push(
                    Signal::new("sma_crossover", data.symbol.clone(), Side::Sell, SignalType::Exit)
                        .with_strength(1.0)
                        .with_prices(Some(price), None, None)
                        .with_metadata("short_sma", json!(short_sma.to_string()))
                        .with_metadata("long_sma", json!(long_sma.to_string()))
                );
                self.position_open = false;
            }
        }

        // 이전 SMA 저장
        self.prev_short_sma = Some(short_sma);
        self.prev_long_sma = Some(long_sma);

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            side = ?order.side,
            quantity = %order.quantity,
            "SMA order filled"
        );
        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.position_open = position.quantity > Decimal::ZERO;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("SMA Crossover strategy shutdown");
        self.prices.clear();
        self.initialized = false;
        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "initialized": self.initialized,
            "symbol": self.config.as_ref().map(|c| &c.symbol),
            "prices_count": self.prices.len(),
            "position_open": self.position_open,
            "current_short_sma": self.prev_short_sma.map(|s| s.to_string()),
            "current_long_sma": self.prev_long_sma.map(|s| s.to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use trader_core::Timeframe;

    fn create_kline(symbol: &Symbol, close: Decimal) -> MarketData {
        use trader_core::Kline;

        let kline = Kline::new(
            symbol.clone(),
            Timeframe::D1,
            Utc::now(),
            close,
            close + dec!(10),
            close - dec!(10),
            close,
            dec!(100),
            Utc::now(),
        );

        MarketData::from_kline("test", kline)
    }

    #[tokio::test]
    async fn test_sma_initialization() {
        let mut strategy = SmaStrategy::new();

        let config = json!({
            "symbol": "005930",
            "short_period": 5,
            "long_period": 10,
            "amount": "100000"
        });

        strategy.initialize(config).await.unwrap();
        assert!(strategy.initialized);
    }

    #[tokio::test]
    async fn test_sma_golden_cross() {
        let mut strategy = SmaStrategy::new();

        let config = json!({
            "symbol": "005930",
            "short_period": 3,
            "long_period": 5,
            "amount": "100000"
        });

        strategy.initialize(config).await.unwrap();

        let symbol = Symbol::stock("005930", "KRW");

        // 하락 추세 데이터 (장기 > 단기)
        for price in [100, 98, 96, 94, 92] {
            let data = create_kline(&symbol, Decimal::from(price));
            let _ = strategy.on_market_data(&data).await.unwrap();
        }

        // 상승 추세로 전환 (골든 크로스 발생)
        for price in [95, 100, 105, 110] {
            let data = create_kline(&symbol, Decimal::from(price));
            let signals = strategy.on_market_data(&data).await.unwrap();

            // 골든 크로스에서 매수 신호 발생
            if !signals.is_empty() {
                assert_eq!(signals[0].side, Side::Buy);
                break;
            }
        }
    }
}
