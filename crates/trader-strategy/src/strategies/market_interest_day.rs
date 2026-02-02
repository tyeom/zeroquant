//! Market Interest Day Trading Strategy
//!
//! 시장 관심종목 단타 전략입니다.
//!
//! ## 전략 개요
//!
//! 거래량이 급증하고 가격이 상승하는 "시장 관심 종목"을 포착하여
//! 단기 트레이딩을 수행합니다.
//!
//! ## 진입 조건
//!
//! 1. 거래량 급증 (평균 대비 2배 이상)
//! 2. 가격 상승 모멘텀 (N분봉 연속 상승)
//! 3. 변동성 확대 (ATR 증가)
//!
//! ## 청산 조건
//!
//! 1. 트레일링 스톱 (고점 대비 N% 하락)
//! 2. 목표 수익률 도달
//! 3. 장 마감 전 강제 청산
//! 4. 모멘텀 약화

use crate::strategies::common::deserialize_symbol;
use crate::Strategy;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;
use tracing::{debug, info};
use trader_core::{
    MarketData, MarketDataType, MarketType, Order, Position, Side, Signal, SignalType, Symbol,
};

/// 캔들 데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CandleData {
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
    pub open_time: i64,
    pub close_time: i64,
}

/// Market Interest Day 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketInterestDayConfig {
    /// 대상 심볼
    #[serde(deserialize_with = "deserialize_symbol")]
    pub symbol: String,

    /// 거래 금액
    #[serde(default = "default_trade_amount")]
    pub trade_amount: Decimal,

    /// 거래량 급증 배수 (평균 대비)
    #[serde(default = "default_volume_multiplier")]
    pub volume_multiplier: Decimal,

    /// 거래량 평균 기간
    #[serde(default = "default_volume_period")]
    pub volume_period: usize,

    /// 연속 상승봉 수
    #[serde(default = "default_consecutive_up")]
    pub consecutive_up_candles: usize,

    /// 트레일링 스톱 비율 (%)
    #[serde(default = "default_trailing_stop")]
    pub trailing_stop_pct: Decimal,

    /// 익절 목표 (%)
    #[serde(default = "default_take_profit")]
    pub take_profit_pct: Decimal,

    /// 손절 기준 (%)
    #[serde(default = "default_stop_loss")]
    pub stop_loss_pct: Decimal,

    /// ATR 기간
    #[serde(default = "default_atr_period")]
    pub atr_period: usize,

    /// ATR 급증 배수
    #[serde(default = "default_atr_multiplier")]
    pub atr_multiplier: Decimal,

    /// 최대 보유 시간 (분)
    #[serde(default = "default_max_hold_minutes")]
    pub max_hold_minutes: u32,

    /// RSI 과열 기준
    #[serde(default = "default_rsi_overbought")]
    pub rsi_overbought: Decimal,

    /// RSI 기간
    #[serde(default = "default_rsi_period")]
    pub rsi_period: usize,
}

fn default_trade_amount() -> Decimal {
    dec!(1000000)
}
fn default_volume_multiplier() -> Decimal {
    dec!(2.0)
}
fn default_volume_period() -> usize {
    20
}
fn default_consecutive_up() -> usize {
    3
}
fn default_trailing_stop() -> Decimal {
    dec!(1.5)
}
fn default_take_profit() -> Decimal {
    dec!(3)
}
fn default_stop_loss() -> Decimal {
    dec!(2)
}
fn default_atr_period() -> usize {
    14
}
fn default_atr_multiplier() -> Decimal {
    dec!(1.5)
}
fn default_max_hold_minutes() -> u32 {
    120
}
fn default_rsi_overbought() -> Decimal {
    dec!(80)
}
fn default_rsi_period() -> usize {
    14
}

impl Default for MarketInterestDayConfig {
    fn default() -> Self {
        Self {
            symbol: "005930".to_string(),
            trade_amount: default_trade_amount(),
            volume_multiplier: default_volume_multiplier(),
            volume_period: default_volume_period(),
            consecutive_up_candles: default_consecutive_up(),
            trailing_stop_pct: default_trailing_stop(),
            take_profit_pct: default_take_profit(),
            stop_loss_pct: default_stop_loss(),
            atr_period: default_atr_period(),
            atr_multiplier: default_atr_multiplier(),
            max_hold_minutes: default_max_hold_minutes(),
            rsi_overbought: default_rsi_overbought(),
            rsi_period: default_rsi_period(),
        }
    }
}

/// Market Interest Day 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketInterestDayState {
    /// 포지션 보유 여부
    pub has_position: bool,
    /// 진입 가격
    pub entry_price: Option<Decimal>,
    /// 진입 시간
    pub entry_time: Option<DateTime<Utc>>,
    /// 현재 수량
    pub quantity: Decimal,
    /// 최고가 (트레일링 스톱용)
    pub highest_price: Decimal,
    /// 오늘 거래 횟수
    pub trades_today: usize,
    /// 오늘 수익
    pub profit_today: Decimal,
}

impl Default for MarketInterestDayState {
    fn default() -> Self {
        Self {
            has_position: false,
            entry_price: None,
            entry_time: None,
            quantity: Decimal::ZERO,
            highest_price: Decimal::ZERO,
            trades_today: 0,
            profit_today: Decimal::ZERO,
        }
    }
}

/// Market Interest Day 전략
pub struct MarketInterestDayStrategy {
    config: Option<MarketInterestDayConfig>,
    symbol: Option<Symbol>,
    state: MarketInterestDayState,
    /// 캔들 히스토리
    candles: VecDeque<CandleData>,
    /// 거래량 히스토리
    volumes: VecDeque<Decimal>,
    /// ATR 값
    current_atr: Option<Decimal>,
    /// 평균 ATR
    avg_atr: Option<Decimal>,
    initialized: bool,
}

impl MarketInterestDayStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            symbol: None,
            state: MarketInterestDayState::default(),
            candles: VecDeque::new(),
            volumes: VecDeque::new(),
            current_atr: None,
            avg_atr: None,
            initialized: false,
        }
    }

    /// 평균 거래량 계산
    fn calculate_avg_volume(&self) -> Decimal {
        let config = match &self.config {
            Some(c) => c,
            None => return Decimal::MAX,
        };

        if self.volumes.len() < config.volume_period {
            return Decimal::MAX;
        }

        let sum: Decimal = self
            .volumes
            .iter()
            .skip(1) // 현재 봉 제외
            .take(config.volume_period)
            .sum();

        sum / Decimal::from(config.volume_period)
    }

    /// 거래량 급증 확인
    fn is_volume_surge(&self, current_volume: Decimal) -> bool {
        let config = match &self.config {
            Some(c) => c,
            None => return false,
        };

        let avg = self.calculate_avg_volume();
        if avg == Decimal::MAX {
            return false;
        }

        current_volume >= avg * config.volume_multiplier
    }

    /// 연속 상승봉 확인
    fn has_consecutive_up_candles(&self) -> bool {
        let config = match &self.config {
            Some(c) => c,
            None => return false,
        };

        if self.candles.len() < config.consecutive_up_candles {
            return false;
        }

        self.candles
            .iter()
            .take(config.consecutive_up_candles)
            .all(|k| k.close > k.open)
    }

    /// ATR 계산
    fn calculate_atr(&self) -> Option<Decimal> {
        let config = match &self.config {
            Some(c) => c,
            None => return None,
        };

        if self.candles.len() < config.atr_period + 1 {
            return None;
        }

        let mut tr_sum = Decimal::ZERO;

        for i in 0..config.atr_period {
            let current = &self.candles[i];
            let prev = &self.candles[i + 1];

            let tr1 = current.high - current.low;
            let tr2 = (current.high - prev.close).abs();
            let tr3 = (current.low - prev.close).abs();

            let tr = tr1.max(tr2).max(tr3);
            tr_sum += tr;
        }

        Some(tr_sum / Decimal::from(config.atr_period))
    }

    /// RSI 계산
    fn calculate_rsi(&self) -> Option<Decimal> {
        let config = match &self.config {
            Some(c) => c,
            None => return None,
        };

        if self.candles.len() < config.rsi_period + 1 {
            return None;
        }

        let mut gains = Decimal::ZERO;
        let mut losses = Decimal::ZERO;

        for i in 0..config.rsi_period {
            let current = &self.candles[i];
            let prev = &self.candles[i + 1];

            let change = current.close - prev.close;
            if change > Decimal::ZERO {
                gains += change;
            } else {
                losses += change.abs();
            }
        }

        let period = Decimal::from(config.rsi_period);
        let avg_gain = gains / period;
        let avg_loss = losses / period;

        if avg_loss == Decimal::ZERO {
            return Some(dec!(100));
        }

        let rs = avg_gain / avg_loss;
        Some(dec!(100) - (dec!(100) / (dec!(1) + rs)))
    }

    /// 진입 조건 확인
    fn check_entry_conditions(&self, candle: &CandleData) -> bool {
        let config = match &self.config {
            Some(c) => c,
            None => return false,
        };

        // 이미 포지션 있으면 제외
        if self.state.has_position {
            return false;
        }

        // 1. 거래량 급증
        if !self.is_volume_surge(candle.volume) {
            return false;
        }

        // 2. 연속 상승봉
        if !self.has_consecutive_up_candles() {
            return false;
        }

        // 3. RSI 과열 아닌지 확인
        if let Some(rsi) = self.calculate_rsi() {
            if rsi >= config.rsi_overbought {
                return false;
            }
        }

        true
    }

    /// 청산 조건 확인
    fn check_exit_conditions(&self, candle: &CandleData) -> Option<String> {
        let config = match &self.config {
            Some(c) => c,
            None => return None,
        };

        if !self.state.has_position {
            return None;
        }

        let entry_price = self.state.entry_price?;
        let current_price = candle.close;

        // 1. 익절 확인
        let profit_pct = (current_price - entry_price) / entry_price * dec!(100);
        if profit_pct >= config.take_profit_pct {
            return Some(format!("익절: +{:.2}%", profit_pct));
        }

        // 2. 손절 확인
        if profit_pct <= -config.stop_loss_pct {
            return Some(format!("손절: {:.2}%", profit_pct));
        }

        // 3. 트레일링 스톱 확인
        if self.state.highest_price > Decimal::ZERO {
            let drop_from_high =
                (self.state.highest_price - current_price) / self.state.highest_price * dec!(100);
            if drop_from_high >= config.trailing_stop_pct && profit_pct > Decimal::ZERO {
                return Some(format!("트레일링 스톱: 고점 대비 -{:.2}%", drop_from_high));
            }
        }

        // 4. 최대 보유 시간 초과
        if let Some(entry_time) = self.state.entry_time {
            let hold_minutes = ((candle.close_time - entry_time.timestamp()) / 60) as u32;
            if hold_minutes >= config.max_hold_minutes {
                return Some(format!("최대 보유 시간 초과: {}분", hold_minutes));
            }
        }

        // 5. 모멘텀 약화 (음봉 연속)
        if self.candles.len() >= 2 {
            let last_two_bearish = self.candles.iter().take(2).all(|k| k.close < k.open);

            if last_two_bearish && profit_pct > Decimal::ZERO {
                return Some("모멘텀 약화: 연속 음봉".to_string());
            }
        }

        None
    }

    /// 신호 생성
    fn generate_signals(&mut self, candle: &CandleData) -> Vec<Signal> {
        let config = match &self.config {
            Some(c) => c,
            None => return Vec::new(),
        };
        let symbol = match &self.symbol {
            Some(s) => s,
            None => return Vec::new(),
        };
        let mut signals = Vec::new();

        // 최고가 업데이트 (포지션 있을 때)
        if self.state.has_position && candle.high > self.state.highest_price {
            self.state.highest_price = candle.high;
        }

        // 청산 조건 확인
        if let Some(reason) = self.check_exit_conditions(candle) {
            let entry_price = self.state.entry_price.unwrap_or(candle.close);
            let profit = (candle.close - entry_price) * self.state.quantity;

            let _quantity = self.state.quantity;

            // 상태 리셋
            self.state.has_position = false;
            self.state.entry_price = None;
            self.state.entry_time = None;
            self.state.quantity = Decimal::ZERO;
            self.state.highest_price = Decimal::ZERO;
            self.state.trades_today += 1;
            self.state.profit_today += profit;

            let signal = Signal::new(
                "market_interest_day",
                symbol.clone(),
                Side::Sell,
                SignalType::Exit,
            )
            .with_strength(1.0)
            .with_metadata("reason", json!(reason))
            .with_metadata("profit", json!(profit.to_string()))
            .with_metadata("trades_today", json!(self.state.trades_today))
            .with_metadata("profit_today", json!(self.state.profit_today.to_string()));

            signals.push(signal);
            info!("[MarketInterestDay] {}", reason);
            return signals;
        }

        // 진입 조건 확인
        if self.check_entry_conditions(candle) {
            let quantity = config.trade_amount / candle.close;

            self.state.has_position = true;
            self.state.entry_price = Some(candle.close);
            self.state.entry_time = Some(Utc::now());
            self.state.quantity = quantity;
            self.state.highest_price = candle.high;

            let rsi = self.calculate_rsi().unwrap_or(Decimal::ZERO);

            let signal = Signal::new(
                "market_interest_day",
                symbol.clone(),
                Side::Buy,
                SignalType::Entry,
            )
            .with_strength(1.0)
            .with_metadata("volume", json!(candle.volume.to_string()))
            .with_metadata("avg_volume", json!(self.calculate_avg_volume().to_string()))
            .with_metadata("rsi", json!(rsi.to_string()))
            .with_metadata("atr", json!(self.current_atr));

            signals.push(signal);
            info!(
                "[MarketInterestDay] 진입: 거래량 급증 + 연속 {}개 양봉",
                config.consecutive_up_candles
            );
        }

        signals
    }
}

impl Default for MarketInterestDayStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for MarketInterestDayStrategy {
    fn name(&self) -> &str {
        "Market Interest Day"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "거래량 급증 종목의 단기 모멘텀 트레이딩"
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mid_config: MarketInterestDayConfig = serde_json::from_value(config)?;

        info!(
            symbol = %mid_config.symbol,
            volume_multiplier = %mid_config.volume_multiplier,
            consecutive_up = %mid_config.consecutive_up_candles,
            "Initializing Market Interest Day strategy"
        );

        self.symbol = Symbol::from_string(&mid_config.symbol, MarketType::KrStock);
        self.config = Some(mid_config);
        self.state = MarketInterestDayState::default();
        self.candles.clear();
        self.volumes.clear();
        self.current_atr = None;
        self.avg_atr = None;
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

        // 캔들 데이터 추출
        let candle = match &data.data {
            MarketDataType::Kline(kline) => CandleData {
                open: kline.open,
                high: kline.high,
                low: kline.low,
                close: kline.close,
                volume: kline.volume,
                open_time: kline.open_time.timestamp(),
                close_time: kline.close_time.timestamp(),
            },
            _ => return Ok(vec![]),
        };

        // 캔들 히스토리 업데이트
        self.candles.push_front(candle.clone());
        if self.candles.len() > 50 {
            self.candles.pop_back();
        }

        // 거래량 히스토리 업데이트
        self.volumes.push_front(candle.volume);
        if self.volumes.len() > config.volume_period + 5 {
            self.volumes.pop_back();
        }

        // ATR 업데이트
        self.current_atr = self.calculate_atr();
        if self.current_atr.is_some() && self.avg_atr.is_none() {
            self.avg_atr = self.current_atr;
        } else if let (Some(curr), Some(avg)) = (self.current_atr, self.avg_atr) {
            // 지수이동평균으로 avg_atr 업데이트
            self.avg_atr = Some(avg * dec!(0.95) + curr * dec!(0.05));
        }

        // 신호 생성
        let signals = self.generate_signals(&candle);

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
            "[MarketInterestDay] 주문 체결: {:?} {} @ {}",
            order.side, order.quantity, fill_price
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
        info!("Market Interest Day strategy shutdown");
        self.initialized = false;
        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "config": self.config,
            "state": self.state,
            "current_atr": self.current_atr,
            "avg_atr": self.avg_atr,
            "rsi": self.calculate_rsi(),
            "initialized": self.initialized,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = MarketInterestDayConfig::default();
        assert_eq!(config.volume_multiplier, dec!(2.0));
        assert_eq!(config.consecutive_up_candles, 3);
    }

    #[tokio::test]
    async fn test_initialization() {
        let mut strategy = MarketInterestDayStrategy::new();

        let config = json!({
            "symbol": "005930",
            "volume_multiplier": "3.0"
        });

        strategy.initialize(config).await.unwrap();
        assert!(strategy.initialized);
    }

    #[test]
    fn test_strategy_creation() {
        let strategy = MarketInterestDayStrategy::new();
        assert_eq!(strategy.name(), "Market Interest Day");
    }

    #[test]
    fn test_initial_state() {
        let strategy = MarketInterestDayStrategy::new();
        assert!(!strategy.state.has_position);
        assert_eq!(strategy.state.trades_today, 0);
    }
}

// 전략 레지스트리에 자동 등록
use crate::register_strategy;

register_strategy! {
    id: "market_interest_day",
    aliases: [],
    name: "단타 시장관심",
    description: "시장 관심 종목을 대상으로 단기 매매를 수행합니다.",
    timeframe: "1d",
    symbols: [],
    category: Daily,
    markets: [KrStock],
    type: MarketInterestDayStrategy
}
