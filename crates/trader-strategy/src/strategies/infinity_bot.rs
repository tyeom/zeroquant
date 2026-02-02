//! Infinity Bot (무한매수봇) Strategy
//!
//! 양방향 이동평균 무한매수봇 전략입니다.
//!
//! ## 전략 개요
//!
//! 50라운드 피라미드 구조로 하락 시 분할 매수하고,
//! 이동평균 모멘텀이 돌아오면 익절하는 전략입니다.
//!
//! ## 핵심 로직
//!
//! 1. **라운드별 진입 조건**:
//!    - 1-5라운드: 무조건 매수 (모멘텀 양호 시)
//!    - 6-20라운드: MA 확인 필요
//!    - 21-30라운드: MA + 양봉 확인
//!    - 31-40라운드: MA + 양봉 + 이평 상승 추세
//!    - 40라운드 이상: MA 반전 시 절반 손절
//!
//! 2. **익절 조건**: 평균 단가 대비 목표 수익률 달성
//!
//! 3. **물타기**: MA 변곡점에서 추가 매수

use crate::strategies::common::deserialize_symbol;
use crate::Strategy;
use async_trait::async_trait;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;
use tracing::{debug, info, warn};
use trader_core::{
    MarketData, MarketDataType, MarketType, Order, Position, Side, Signal, SignalType, Symbol,
};

/// 라운드 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundInfo {
    pub round: usize,
    pub entry_price: Decimal,
    pub quantity: Decimal,
    pub timestamp: i64,
}

/// Infinity Bot 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfinityBotConfig {
    /// 대상 심볼
    #[serde(deserialize_with = "deserialize_symbol")]
    pub symbol: String,

    /// 총 투자 금액
    #[serde(default = "default_total_amount")]
    pub total_amount: Decimal,

    /// 최대 라운드 수
    #[serde(default = "default_max_rounds")]
    pub max_rounds: usize,

    /// 라운드당 투자 금액 비율 (%)
    #[serde(default = "default_round_amount_pct")]
    pub round_amount_pct: Decimal,

    /// 추가 매수 트리거 하락률 (%)
    #[serde(default = "default_dip_trigger")]
    pub dip_trigger_pct: Decimal,

    /// 익절 목표 수익률 (%)
    #[serde(default = "default_take_profit")]
    pub take_profit_pct: Decimal,

    /// 손절 기준 (40라운드 이후)
    #[serde(default = "default_stop_loss")]
    pub stop_loss_pct: Decimal,

    /// 이동평균 기간 (단기)
    #[serde(default = "default_short_ma")]
    pub short_ma_period: usize,

    /// 이동평균 기간 (중기)
    #[serde(default = "default_mid_ma")]
    pub mid_ma_period: usize,

    /// 이동평균 기간 (장기)
    #[serde(default = "default_long_ma")]
    pub long_ma_period: usize,

    /// 모멘텀 가중치 (장기, 중기, 단기)
    #[serde(default = "default_momentum_weights")]
    pub momentum_weights: [Decimal; 3],
}

fn default_total_amount() -> Decimal {
    dec!(10000000)
}
fn default_max_rounds() -> usize {
    50
}
fn default_round_amount_pct() -> Decimal {
    dec!(2)
}
fn default_dip_trigger() -> Decimal {
    dec!(2)
}
fn default_take_profit() -> Decimal {
    dec!(3)
}
fn default_stop_loss() -> Decimal {
    dec!(20)
}
fn default_short_ma() -> usize {
    10
}
fn default_mid_ma() -> usize {
    100
}
fn default_long_ma() -> usize {
    200
}
fn default_momentum_weights() -> [Decimal; 3] {
    [dec!(0.3), dec!(0.2), dec!(0.3)]
}

impl Default for InfinityBotConfig {
    fn default() -> Self {
        Self {
            symbol: "005930".to_string(),
            total_amount: default_total_amount(),
            max_rounds: default_max_rounds(),
            round_amount_pct: default_round_amount_pct(),
            dip_trigger_pct: default_dip_trigger(),
            take_profit_pct: default_take_profit(),
            stop_loss_pct: default_stop_loss(),
            short_ma_period: default_short_ma(),
            mid_ma_period: default_mid_ma(),
            long_ma_period: default_long_ma(),
            momentum_weights: default_momentum_weights(),
        }
    }
}

/// Infinity Bot 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfinityBotState {
    /// 현재 라운드
    pub current_round: usize,
    /// 라운드 히스토리
    pub rounds: Vec<RoundInfo>,
    /// 총 투자 금액
    pub total_invested: Decimal,
    /// 총 수량
    pub total_quantity: Decimal,
    /// 평균 매입가
    pub avg_price: Decimal,
    /// 마지막 매수 가격
    pub last_buy_price: Option<Decimal>,
    /// 사이클 완료 횟수
    pub completed_cycles: usize,
    /// 누적 수익
    pub cumulative_profit: Decimal,
}

impl Default for InfinityBotState {
    fn default() -> Self {
        Self {
            current_round: 0,
            rounds: Vec::new(),
            total_invested: Decimal::ZERO,
            total_quantity: Decimal::ZERO,
            avg_price: Decimal::ZERO,
            last_buy_price: None,
            completed_cycles: 0,
            cumulative_profit: Decimal::ZERO,
        }
    }
}

/// Infinity Bot 전략
pub struct InfinityBotStrategy {
    config: Option<InfinityBotConfig>,
    symbol: Option<Symbol>,
    state: InfinityBotState,
    /// 가격 히스토리
    prices: VecDeque<Decimal>,
    /// 이동평균값 캐시
    short_ma: Option<Decimal>,
    mid_ma: Option<Decimal>,
    long_ma: Option<Decimal>,
    /// 이전 MA 값 (추세 판단용)
    prev_short_ma: Option<Decimal>,
    /// 마지막 캔들이 양봉인지
    last_candle_bullish: bool,
    initialized: bool,
}

impl InfinityBotStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            symbol: None,
            state: InfinityBotState::default(),
            prices: VecDeque::new(),
            short_ma: None,
            mid_ma: None,
            long_ma: None,
            prev_short_ma: None,
            last_candle_bullish: false,
            initialized: false,
        }
    }

    /// 이동평균 계산
    fn calculate_ma(&self, period: usize) -> Option<Decimal> {
        if self.prices.len() < period {
            return None;
        }

        let sum: Decimal = self.prices.iter().take(period).sum();
        Some(sum / Decimal::from(period))
    }

    /// 가중 모멘텀 스코어 계산
    fn calculate_momentum_score(&self) -> Decimal {
        let config = match &self.config {
            Some(c) => c,
            None => return Decimal::ZERO,
        };

        let long_momentum = self.calculate_period_momentum(config.long_ma_period);
        let mid_momentum = self.calculate_period_momentum(config.mid_ma_period);
        let short_momentum = self.calculate_period_momentum(config.short_ma_period);

        long_momentum * config.momentum_weights[0]
            + mid_momentum * config.momentum_weights[1]
            + short_momentum * config.momentum_weights[2]
    }

    /// 기간별 모멘텀 계산
    fn calculate_period_momentum(&self, period: usize) -> Decimal {
        if self.prices.len() < period {
            return Decimal::ZERO;
        }

        let current = *self.prices.front().unwrap();
        let past = *self.prices.get(period - 1).unwrap();

        if past > Decimal::ZERO {
            (current - past) / past * dec!(100)
        } else {
            Decimal::ZERO
        }
    }

    /// 라운드별 진입 조건 확인
    fn can_enter_round(&self, round: usize, current_price: Decimal) -> bool {
        let momentum = self.calculate_momentum_score();

        match round {
            1..=5 => {
                // 1-5라운드: 모멘텀 양호 시 무조건 매수
                momentum > dec!(-5)
            }
            6..=20 => {
                // 6-20라운드: MA 확인 필요
                if let Some(short_ma) = self.short_ma {
                    current_price > short_ma || momentum > Decimal::ZERO
                } else {
                    false
                }
            }
            21..=30 => {
                // 21-30라운드: MA + 양봉 확인
                if let Some(short_ma) = self.short_ma {
                    self.last_candle_bullish
                        && (current_price > short_ma || momentum > Decimal::ZERO)
                } else {
                    false
                }
            }
            31..=40 => {
                // 31-40라운드: MA + 양봉 + 이평 상승 추세
                let ma_rising = self
                    .prev_short_ma
                    .zip(self.short_ma)
                    .map(|(prev, curr)| curr > prev)
                    .unwrap_or(false);

                self.last_candle_bullish && ma_rising
            }
            _ => {
                // 40라운드 이상: 매우 보수적
                let ma_rising = self
                    .prev_short_ma
                    .zip(self.short_ma)
                    .map(|(prev, curr)| curr > prev)
                    .unwrap_or(false);

                self.last_candle_bullish && ma_rising && momentum > Decimal::ZERO
            }
        }
    }

    /// 추가 매수 필요 여부 확인
    fn should_add_position(&self, current_price: Decimal) -> bool {
        let config = match &self.config {
            Some(c) => c,
            None => return false,
        };

        if self.state.current_round >= config.max_rounds {
            return false;
        }

        if let Some(last_price) = self.state.last_buy_price {
            let drop_pct = (last_price - current_price) / last_price * dec!(100);
            drop_pct >= config.dip_trigger_pct
        } else {
            // 첫 매수
            true
        }
    }

    /// 익절 확인
    fn should_take_profit(&self, current_price: Decimal) -> bool {
        let config = match &self.config {
            Some(c) => c,
            None => return false,
        };

        if self.state.avg_price == Decimal::ZERO {
            return false;
        }

        let profit_pct = (current_price - self.state.avg_price) / self.state.avg_price * dec!(100);
        profit_pct >= config.take_profit_pct
    }

    /// 손절 확인 (40라운드 이상)
    fn should_stop_loss(&self, current_price: Decimal) -> bool {
        let config = match &self.config {
            Some(c) => c,
            None => return false,
        };

        if self.state.current_round < 40 || self.state.avg_price == Decimal::ZERO {
            return false;
        }

        let loss_pct = (self.state.avg_price - current_price) / self.state.avg_price * dec!(100);
        loss_pct >= config.stop_loss_pct
    }

    /// 라운드당 투자 금액 계산
    fn get_round_amount(&self) -> Decimal {
        let config = match &self.config {
            Some(c) => c,
            None => return Decimal::ZERO,
        };
        config.total_amount * config.round_amount_pct / dec!(100)
    }

    /// 평균가 재계산
    fn update_avg_price(&mut self) {
        if self.state.total_quantity > Decimal::ZERO {
            self.state.avg_price = self.state.total_invested / self.state.total_quantity;
        }
    }

    /// 사이클 리셋
    fn reset_cycle(&mut self) {
        self.state.current_round = 0;
        self.state.rounds.clear();
        self.state.total_invested = Decimal::ZERO;
        self.state.total_quantity = Decimal::ZERO;
        self.state.avg_price = Decimal::ZERO;
        self.state.last_buy_price = None;
    }

    /// 신호 생성
    fn generate_signals(
        &mut self,
        current_price: Decimal,
        open_price: Decimal,
        timestamp: i64,
    ) -> Vec<Signal> {
        let symbol = match &self.symbol {
            Some(s) => s.clone(),
            None => return Vec::new(),
        };
        let mut signals = Vec::new();

        // 양봉 확인
        self.last_candle_bullish = current_price > open_price;

        // 1. 익절 확인
        if self.state.total_quantity > Decimal::ZERO && self.should_take_profit(current_price) {
            let profit = (current_price - self.state.avg_price) * self.state.total_quantity;
            let _quantity = self.state.total_quantity;

            self.state.cumulative_profit += profit;
            self.state.completed_cycles += 1;
            self.reset_cycle();

            let signal = Signal::new("infinity_bot", symbol.clone(), Side::Sell, SignalType::Exit)
                .with_strength(1.0)
                .with_metadata("reason", json!("take_profit"))
                .with_metadata("completed_cycles", json!(self.state.completed_cycles))
                .with_metadata("profit", json!(profit.to_string()))
                .with_metadata(
                    "cumulative_profit",
                    json!(self.state.cumulative_profit.to_string()),
                );

            signals.push(signal);
            info!(
                "[InfinityBot] 익절: 사이클 #{} 완료, 수익: {:.2}",
                self.state.completed_cycles, profit
            );
            return signals;
        }

        // 2. 손절 확인 (40라운드 이상)
        if self.state.current_round >= 40 && self.should_stop_loss(current_price) {
            // 절반만 손절
            let sell_quantity = self.state.total_quantity / dec!(2);
            let loss = (self.state.avg_price - current_price) * sell_quantity;

            self.state.total_quantity -= sell_quantity;
            self.state.total_invested -= sell_quantity * self.state.avg_price;
            self.state.cumulative_profit -= loss;

            let signal = Signal::new(
                "infinity_bot",
                symbol.clone(),
                Side::Sell,
                SignalType::ReducePosition,
            )
            .with_strength(1.0)
            .with_metadata("reason", json!("stop_loss_half"))
            .with_metadata("current_round", json!(self.state.current_round))
            .with_metadata("loss", json!(loss.to_string()))
            .with_metadata(
                "remaining_quantity",
                json!(self.state.total_quantity.to_string()),
            );

            signals.push(signal);
            warn!(
                "[InfinityBot] 절반 손절: 라운드 {}, 손실: {:.2}",
                self.state.current_round, loss
            );
            return signals;
        }

        // 3. 추가 매수 확인
        if self.should_add_position(current_price) {
            let next_round = self.state.current_round + 1;

            if self.can_enter_round(next_round, current_price) {
                let invest_amount = self.get_round_amount();
                let quantity = invest_amount / current_price;

                self.state.current_round = next_round;
                self.state.total_invested += invest_amount;
                self.state.total_quantity += quantity;
                self.state.last_buy_price = Some(current_price);
                self.update_avg_price();

                self.state.rounds.push(RoundInfo {
                    round: next_round,
                    entry_price: current_price,
                    quantity,
                    timestamp,
                });

                let signal =
                    Signal::new("infinity_bot", symbol.clone(), Side::Buy, SignalType::Entry)
                        .with_strength(1.0)
                        .with_metadata("round", json!(next_round))
                        .with_metadata("avg_price", json!(self.state.avg_price.to_string()))
                        .with_metadata(
                            "total_invested",
                            json!(self.state.total_invested.to_string()),
                        )
                        .with_metadata(
                            "momentum_score",
                            json!(self.calculate_momentum_score().to_string()),
                        );

                signals.push(signal);
                info!(
                    "[InfinityBot] 라운드 {} 매수: 평균가 {:.4}",
                    next_round, self.state.avg_price
                );
            }
        }

        signals
    }
}

impl Default for InfinityBotStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for InfinityBotStrategy {
    fn name(&self) -> &str {
        "Infinity Bot"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "50라운드 피라미드 물타기 전략, 이동평균 모멘텀 기반 익절"
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ib_config: InfinityBotConfig = serde_json::from_value(config)?;

        info!(
            symbol = %ib_config.symbol,
            max_rounds = %ib_config.max_rounds,
            take_profit_pct = %ib_config.take_profit_pct,
            "Initializing Infinity Bot strategy"
        );

        self.symbol = Symbol::from_string(&ib_config.symbol, MarketType::KrStock);
        self.config = Some(ib_config);
        self.state = InfinityBotState::default();
        self.prices.clear();
        self.short_ma = None;
        self.mid_ma = None;
        self.long_ma = None;
        self.prev_short_ma = None;
        self.last_candle_bullish = false;
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

        // 가격 및 시간 추출
        let (current_price, open_price, timestamp) = match &data.data {
            MarketDataType::Kline(kline) => (kline.close, kline.open, kline.close_time.timestamp()),
            MarketDataType::Ticker(ticker) => (ticker.last, ticker.last, 0i64),
            MarketDataType::Trade(trade) => (trade.price, trade.price, trade.timestamp.timestamp()),
            _ => return Ok(vec![]),
        };

        // 가격 히스토리 업데이트
        self.prices.push_front(current_price);
        if self.prices.len() > 250 {
            self.prices.pop_back();
        }

        // 이동평균 업데이트
        self.prev_short_ma = self.short_ma;
        self.short_ma = self.calculate_ma(config.short_ma_period);
        self.mid_ma = self.calculate_ma(config.mid_ma_period);
        self.long_ma = self.calculate_ma(config.long_ma_period);

        // 신호 생성
        let signals = self.generate_signals(current_price, open_price, timestamp);

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let fill_price = order
            .average_fill_price
            .or(order.price)
            .unwrap_or(Decimal::ZERO);

        info!(
            "[InfinityBot] 주문 체결: {:?} {} @ {} (라운드 {})",
            order.side, order.quantity, fill_price, self.state.current_round
        );

        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!(
            quantity = %position.quantity,
            pnl = %position.realized_pnl,
            "Position updated"
        );

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Infinity Bot strategy shutdown");
        self.initialized = false;
        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "config": self.config,
            "state": self.state,
            "short_ma": self.short_ma,
            "mid_ma": self.mid_ma,
            "long_ma": self.long_ma,
            "momentum_score": self.calculate_momentum_score(),
            "initialized": self.initialized,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = InfinityBotConfig::default();
        assert_eq!(config.max_rounds, 50);
        assert_eq!(config.take_profit_pct, dec!(3));
    }

    #[tokio::test]
    async fn test_initialization() {
        let mut strategy = InfinityBotStrategy::new();

        let config = json!({
            "symbol": "005930",
            "max_rounds": 30,
            "take_profit_pct": "5"
        });

        strategy.initialize(config).await.unwrap();
        assert!(strategy.initialized);
    }

    #[test]
    fn test_strategy_creation() {
        let strategy = InfinityBotStrategy::new();
        assert_eq!(strategy.name(), "Infinity Bot");
    }

    #[test]
    fn test_round_amount() {
        let mut strategy = InfinityBotStrategy::new();
        strategy.config = Some(InfinityBotConfig {
            total_amount: dec!(10000000),
            round_amount_pct: dec!(2),
            ..Default::default()
        });
        assert_eq!(strategy.get_round_amount(), dec!(200000));
    }

    #[test]
    fn test_initial_state() {
        let strategy = InfinityBotStrategy::new();
        assert_eq!(strategy.state.current_round, 0);
        assert_eq!(strategy.state.total_invested, Decimal::ZERO);
        assert_eq!(strategy.state.completed_cycles, 0);
    }
}

// 전략 레지스트리에 자동 등록
use crate::register_strategy;

register_strategy! {
    id: "infinity_bot",
    aliases: [],
    name: "무한매수",
    description: "가격 하락 시 자동으로 분할 매수하는 물타기 전략입니다.",
    timeframe: "1m",
    symbols: [],
    category: Realtime,
    markets: [Crypto],
    type: InfinityBotStrategy
}
