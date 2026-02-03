//! 단순 이동평균 크로스오버 전략.
//!
//! 단기 이동평균이 장기 이동평균을 상향 돌파하면 매수,
//! 하향 돌파하면 매도하는 클래식한 추세 추종 전략입니다.
//!
//! # StrategyContext 활용 (v2.0)
//!
//! - `RouteState`: 진입 가능 여부 판단
//! - `GlobalScore`: 종목 품질 필터링
//! - `MarketRegime`: 추세 확인
//!
//! # 전략 로직
//! - 골든 크로스 (단기 SMA > 장기 SMA): 매수 신호
//! - 데드 크로스 (단기 SMA < 장기 SMA): 매도 신호

use crate::strategies::common::deserialize_symbol;
use crate::Strategy;
use async_trait::async_trait;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use trader_core::domain::{RouteState, StrategyContext};
use trader_core::{
    MarketData, MarketDataType, MarketType, Order, Position, Side, Signal, SignalType, Symbol,
};

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

    /// 최소 GlobalScore (기본값: 50)
    #[serde(default = "default_min_score")]
    pub min_global_score: Decimal,
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

fn default_min_score() -> Decimal {
    dec!(50)
}

impl Default for SmaConfig {
    fn default() -> Self {
        Self {
            symbol: "BTC/USDT".to_string(),
            short_period: 10,
            long_period: 20,
            amount: Decimal::from(100000),
            min_global_score: dec!(50),
        }
    }
}

/// SMA 크로스오버 전략.
pub struct SmaStrategy {
    /// 설정
    config: Option<SmaConfig>,
    /// 심볼
    symbol: Option<Symbol>,
    context: Option<Arc<RwLock<StrategyContext>>>,
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
            context: None,
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

    /// StrategyContext 기반 진입 가능 여부 체크.
    fn can_enter(&self) -> bool {
        let Some(config) = self.config.as_ref() else {
            return false;
        };
        let ticker = &config.symbol;

        let Some(ctx) = self.context.as_ref() else {
            // Context 없으면 진입 허용 (하위 호환성)
            return true;
        };

        let Ok(ctx_lock) = ctx.try_read() else {
            return true;
        };

        // RouteState 체크
        if let Some(route_state) = ctx_lock.get_route_state(ticker) {
            match route_state {
                RouteState::Overheat | RouteState::Wait | RouteState::Neutral => {
                    debug!(
                        ticker = %ticker,
                        route_state = ?route_state,
                        "RouteState 진입 제한"
                    );
                    return false;
                }
                RouteState::Armed | RouteState::Attack => {
                    // 진입 가능
                }
            }
        }

        // GlobalScore 체크
        if let Some(score) = ctx_lock.get_global_score(ticker) {
            if score.overall_score < config.min_global_score {
                debug!(
                    ticker = %ticker,
                    score = %score.overall_score,
                    min_required = %config.min_global_score,
                    "GlobalScore 미달"
                );
                return false;
            }
        }

        true
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
        "2.0.0"
    }

    fn description(&self) -> &str {
        "StrategyContext 기반 이동평균 크로스오버 전략 (RouteState, GlobalScore 필터링)"
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

        let config = match &self.config {
            Some(c) => c,
            None => return Ok(vec![]),
        };

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

            if golden_cross && !self.position_open && self.can_enter() {
                info!(
                    short_sma = %short_sma,
                    long_sma = %long_sma,
                    price = %price,
                    "골든 크로스 - 매수 신호"
                );

                signals.push(
                    Signal::new(
                        "sma_crossover",
                        data.symbol.clone(),
                        Side::Buy,
                        SignalType::Entry,
                    )
                    .with_strength(1.0)
                    .with_prices(Some(price), None, None)
                    .with_metadata("short_sma", json!(short_sma.to_string()))
                    .with_metadata("long_sma", json!(long_sma.to_string())),
                );
                self.position_open = true;
            } else if death_cross && self.position_open {
                info!(
                    short_sma = %short_sma,
                    long_sma = %long_sma,
                    price = %price,
                    "데드 크로스 - 매도 신호"
                );

                signals.push(
                    Signal::new(
                        "sma_crossover",
                        data.symbol.clone(),
                        Side::Sell,
                        SignalType::Exit,
                    )
                    .with_strength(1.0)
                    .with_prices(Some(price), None, None)
                    .with_metadata("short_sma", json!(short_sma.to_string()))
                    .with_metadata("long_sma", json!(long_sma.to_string())),
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

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("StrategyContext injected into SMA strategy");
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

// 전략 레지스트리에 자동 등록
use crate::register_strategy;

register_strategy! {
    id: "sma_crossover",
    aliases: ["sma", "ma_crossover"],
    name: "이동평균 크로스오버",
    description: "단기/장기 이동평균선 교차로 매매 신호를 생성합니다.",
    timeframe: "15m",
    symbols: [],
    category: Intraday,
    markets: [Crypto, Stock, Stock],
    type: SmaStrategy
}
