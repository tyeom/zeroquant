//! RSI 평균회귀 전략.
//!
//! RSI가 과매도 상태(< 30)일 때 매수하고
//! RSI가 과매수 상태(> 70)일 때 매도하는 전략입니다.
//!
//! RSI (Relative Strength Index)는 최근 가격 변동의 크기를 측정하여
//! 과매수 또는 과매도 상태를 평가합니다.

use crate::strategies::common::deserialize_symbol;
use crate::Strategy;
use async_trait::async_trait;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;
use trader_core::{MarketData, MarketDataType, MarketType, Order, Position, Side, Signal, SignalType, Symbol};
use tracing::{debug, info, warn};

/// RSI 전략 설정.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RsiConfig {
    /// 거래할 심볼
    #[serde(deserialize_with = "deserialize_symbol")]
    pub symbol: String,

    /// RSI 기간 (일반적으로 14)
    #[serde(default = "default_period")]
    pub period: usize,

    /// 과매도 임계값 (RSI가 이 값 아래로 떨어지면 매수)
    #[serde(default = "default_oversold")]
    pub oversold_threshold: f64,

    /// 과매수 임계값 (RSI가 이 값 위로 올라가면 매도)
    #[serde(default = "default_overbought")]
    pub overbought_threshold: f64,

    /// 거래 금액 (호가 통화 기준)
    pub amount: Decimal,

    /// RSI에 EMA 스무딩 사용 (false면 Cutler's RSI)
    #[serde(default = "default_true")]
    pub use_ema_smoothing: bool,

    /// 진입 전 확인 캔들 필요 수
    #[serde(default)]
    pub confirmation_candles: usize,

    /// RSI가 중립(50)을 교차할 때 청산
    #[serde(default)]
    pub exit_on_neutral: bool,

    /// 손절 비율 (진입 가격 기준)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_loss_pct: Option<f64>,

    /// 익절 비율 (진입 가격 기준)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub take_profit_pct: Option<f64>,

    /// 최대 포지션 크기
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_position_size: Option<Decimal>,

    /// 거래 후 쿨다운 기간 (캔들 수 기준)
    #[serde(default = "default_cooldown")]
    pub cooldown_candles: usize,
}

fn default_period() -> usize {
    14
}
fn default_oversold() -> f64 {
    30.0
}
fn default_overbought() -> f64 {
    70.0
}
fn default_true() -> bool {
    true
}
fn default_cooldown() -> usize {
    5
}

impl Default for RsiConfig {
    fn default() -> Self {
        Self {
            symbol: "BTC/USDT".to_string(),
            period: 14,
            oversold_threshold: 30.0,
            overbought_threshold: 70.0,
            amount: dec!(100),
            use_ema_smoothing: true,
            confirmation_candles: 0,
            exit_on_neutral: false,
            stop_loss_pct: None,
            take_profit_pct: None,
            max_position_size: None,
            cooldown_candles: 5,
        }
    }
}

/// 전략의 포지션 상태.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PositionState {
    /// 포지션 없음
    Flat,
    /// 롱 포지션
    Long,
    /// 숏 포지션 (지원되는 경우)
    Short,
}

/// RSI 평균회귀 전략.
pub struct RsiStrategy {
    /// 전략 설정
    config: Option<RsiConfig>,

    /// 거래 중인 심볼
    symbol: Option<Symbol>,

    /// 가격 히스토리 (종가)
    close_history: VecDeque<Decimal>,

    /// RSI 계산을 위한 상승폭 히스토리
    gains: VecDeque<Decimal>,

    /// RSI 계산을 위한 하락폭 히스토리
    losses: VecDeque<Decimal>,

    /// 현재 RSI 값
    current_rsi: Option<f64>,

    /// 이전 RSI 값 (크로스오버 감지용)
    previous_rsi: Option<f64>,

    /// 확인을 위한 RSI 히스토리
    rsi_history: VecDeque<f64>,

    /// 현재 포지션 상태
    position_state: PositionState,

    /// 진입 가격
    entry_price: Option<Decimal>,

    /// 현재 가격
    current_price: Option<Decimal>,

    /// 쿨다운 카운터
    cooldown_counter: usize,

    /// 총 실행된 거래 수
    trades_count: u32,

    /// 승리 횟수
    wins: u32,

    /// 손실 횟수
    trade_losses: u32,

    /// 총 손익
    total_pnl: Decimal,

    /// 평균 상승폭 (Wilder's 스무딩용)
    avg_gain: Option<Decimal>,

    /// 평균 하락폭 (Wilder's 스무딩용)
    avg_loss: Option<Decimal>,

    /// 초기화 플래그
    initialized: bool,
}

impl RsiStrategy {
    /// 새 RSI 전략 생성.
    pub fn new() -> Self {
        Self {
            config: None,
            symbol: None,
            close_history: VecDeque::new(),
            gains: VecDeque::new(),
            losses: VecDeque::new(),
            current_rsi: None,
            previous_rsi: None,
            rsi_history: VecDeque::new(),
            position_state: PositionState::Flat,
            entry_price: None,
            current_price: None,
            cooldown_counter: 0,
            trades_count: 0,
            wins: 0,
            trade_losses: 0,
            total_pnl: Decimal::ZERO,
            avg_gain: None,
            avg_loss: None,
            initialized: false,
        }
    }

    /// Wilder's 스무딩 방식으로 RSI 계산.
    fn calculate_rsi(&mut self) {
        let config = self.config.as_ref().unwrap();
        let period = config.period;

        if self.close_history.len() < period + 1 {
            return;
        }

        // 가격 변동 계산
        let mut gains = Vec::new();
        let mut losses = Vec::new();

        for i in 0..period {
            if i + 1 >= self.close_history.len() {
                break;
            }
            let current = self.close_history[i];
            let previous = self.close_history[i + 1];
            let change = current - previous;

            if change > Decimal::ZERO {
                gains.push(change);
                losses.push(Decimal::ZERO);
            } else {
                gains.push(Decimal::ZERO);
                losses.push(change.abs());
            }
        }

        if gains.len() < period {
            return;
        }

        let period_dec = Decimal::from(period as u32);

        if config.use_ema_smoothing {
            if let (Some(prev_avg_gain), Some(prev_avg_loss)) = (self.avg_gain, self.avg_loss) {
                // Wilder's 스무딩 (EMA 유사)
                let current_gain = gains[0];
                let current_loss = losses[0];

                let new_avg_gain =
                    (prev_avg_gain * (period_dec - dec!(1)) + current_gain) / period_dec;
                let new_avg_loss =
                    (prev_avg_loss * (period_dec - dec!(1)) + current_loss) / period_dec;

                self.avg_gain = Some(new_avg_gain);
                self.avg_loss = Some(new_avg_loss);
            } else {
                // 초기 SMA (첫 번째 계산)
                let sum_gain: Decimal = gains.iter().sum();
                let sum_loss: Decimal = losses.iter().sum();

                self.avg_gain = Some(sum_gain / period_dec);
                self.avg_loss = Some(sum_loss / period_dec);
            }
        } else {
            // 초기 SMA
            let sum_gain: Decimal = gains.iter().sum();
            let sum_loss: Decimal = losses.iter().sum();

            self.avg_gain = Some(sum_gain / period_dec);
            self.avg_loss = Some(sum_loss / period_dec);
        }

        // RSI 계산
        let avg_gain = self.avg_gain.unwrap();
        let avg_loss = self.avg_loss.unwrap();

        let rsi = if avg_loss.is_zero() {
            100.0
        } else if avg_gain.is_zero() {
            0.0
        } else {
            let rs = avg_gain / avg_loss;
            let rs_f64 = rs.to_string().parse::<f64>().unwrap_or(1.0);
            100.0 - (100.0 / (1.0 + rs_f64))
        };

        // 이전 RSI 저장
        self.previous_rsi = self.current_rsi;
        self.current_rsi = Some(rsi);

        // RSI 히스토리 업데이트
        self.rsi_history.push_front(rsi);
        if self.rsi_history.len() > 10 {
            self.rsi_history.pop_back();
        }
    }

    /// RSI가 과매도/과매수 구간에서 확인되었는지 체크.
    fn is_confirmed(&self, threshold: f64, below: bool) -> bool {
        let config = self.config.as_ref().unwrap();

        if config.confirmation_candles == 0 {
            return true;
        }

        if self.rsi_history.len() < config.confirmation_candles {
            return false;
        }

        // RSI가 필요한 캔들 수 동안 해당 구간에 있었는지 확인
        for i in 0..config.confirmation_candles {
            let rsi = self.rsi_history.get(i).unwrap_or(&50.0);
            if below && *rsi >= threshold {
                return false;
            }
            if !below && *rsi <= threshold {
                return false;
            }
        }

        true
    }

    /// RSI를 기반으로 트레이딩 신호 생성.
    fn generate_signals(&mut self) -> Vec<Signal> {
        let config = self.config.as_ref().unwrap();
        let symbol = self.symbol.as_ref().unwrap();
        let mut signals = Vec::new();

        let rsi = match self.current_rsi {
            Some(r) => r,
            None => return signals,
        };

        let current_price = match self.current_price {
            Some(p) => p,
            None => return signals,
        };

        // 쿨다운 확인
        if self.cooldown_counter > 0 {
            self.cooldown_counter -= 1;
            return signals;
        }

        match self.position_state {
            PositionState::Flat => {
                // 과매도 확인 (매수 신호)
                if rsi < config.oversold_threshold
                    && self.is_confirmed(config.oversold_threshold, true)
                {
                    // RSI가 과매도에서 상향 크로스하는지 확인
                    let crossing_up = self
                        .previous_rsi
                        .map(|prev| prev < config.oversold_threshold && rsi > prev)
                        .unwrap_or(true);

                    if crossing_up {
                        let mut signal =
                            Signal::new("rsi_mean_reversion", symbol.clone(), Side::Buy, SignalType::Entry)
                                .with_strength((config.oversold_threshold - rsi) / config.oversold_threshold)
                                .with_metadata("rsi", json!(rsi))
                                .with_metadata("reason", json!("oversold"));

                        // 설정된 경우 손절 및 익절 추가
                        if let Some(sl_pct) = config.stop_loss_pct {
                            let sl_price = current_price * (dec!(1) - Decimal::from_f64_retain(sl_pct / 100.0).unwrap_or(dec!(0.02)));
                            signal.stop_loss = Some(sl_price);
                        }

                        if let Some(tp_pct) = config.take_profit_pct {
                            let tp_price = current_price * (dec!(1) + Decimal::from_f64_retain(tp_pct / 100.0).unwrap_or(dec!(0.05)));
                            signal.take_profit = Some(tp_price);
                        }

                        signals.push(signal);

                        info!(
                            rsi = rsi,
                            price = %current_price,
                            "RSI oversold - BUY signal"
                        );
                    }
                }

                // 과매수 확인 (매도/숏 신호 - 지원되는 경우)
                if rsi > config.overbought_threshold
                    && self.is_confirmed(config.overbought_threshold, false)
                {
                    let crossing_down = self
                        .previous_rsi
                        .map(|prev| prev > config.overbought_threshold && rsi < prev)
                        .unwrap_or(true);

                    if crossing_down {
                        let signal =
                            Signal::new("rsi_mean_reversion", symbol.clone(), Side::Sell, SignalType::Entry)
                                .with_strength((rsi - config.overbought_threshold) / (100.0 - config.overbought_threshold))
                                .with_metadata("rsi", json!(rsi))
                                .with_metadata("reason", json!("overbought"));

                        signals.push(signal);

                        info!(
                            rsi = rsi,
                            price = %current_price,
                            "RSI overbought - SELL signal"
                        );
                    }
                }
            }

            PositionState::Long => {
                // 롱 포지션 청산 조건
                let should_exit = if config.exit_on_neutral {
                    // RSI가 중립(50) 위로 크로스할 때 청산
                    rsi >= 50.0 && self.previous_rsi.unwrap_or(50.0) < 50.0
                } else {
                    // RSI가 과매수에 도달할 때 청산
                    rsi >= config.overbought_threshold
                };

                // 손절 확인
                let stop_hit = if let (Some(entry), Some(sl_pct)) =
                    (self.entry_price, config.stop_loss_pct)
                {
                    let sl_price = entry * (dec!(1) - Decimal::from_f64_retain(sl_pct / 100.0).unwrap_or(dec!(0.02)));
                    current_price <= sl_price
                } else {
                    false
                };

                // 익절 확인
                let tp_hit = if let (Some(entry), Some(tp_pct)) =
                    (self.entry_price, config.take_profit_pct)
                {
                    let tp_price = entry * (dec!(1) + Decimal::from_f64_retain(tp_pct / 100.0).unwrap_or(dec!(0.05)));
                    current_price >= tp_price
                } else {
                    false
                };

                if should_exit || stop_hit || tp_hit {
                    let reason = if stop_hit {
                        "stop_loss"
                    } else if tp_hit {
                        "take_profit"
                    } else {
                        "rsi_exit"
                    };

                    let signal =
                        Signal::new("rsi_mean_reversion", symbol.clone(), Side::Sell, SignalType::Exit)
                            .with_strength(1.0)
                            .with_metadata("rsi", json!(rsi))
                            .with_metadata("reason", json!(reason));

                    signals.push(signal);

                    info!(
                        rsi = rsi,
                        price = %current_price,
                        reason = reason,
                        "Exiting LONG position"
                    );
                }
            }

            PositionState::Short => {
                // 숏 포지션 청산 조건 (롱의 반대)
                let should_exit = if config.exit_on_neutral {
                    rsi <= 50.0 && self.previous_rsi.unwrap_or(50.0) > 50.0
                } else {
                    rsi <= config.oversold_threshold
                };

                if should_exit {
                    let signal =
                        Signal::new("rsi_mean_reversion", symbol.clone(), Side::Buy, SignalType::Exit)
                            .with_strength(1.0)
                            .with_metadata("rsi", json!(rsi))
                            .with_metadata("reason", json!("rsi_exit"));

                    signals.push(signal);

                    info!(
                        rsi = rsi,
                        price = %current_price,
                        "Exiting SHORT position"
                    );
                }
            }
        }

        signals
    }

    /// 전략 통계 조회.
    fn get_stats(&self) -> RsiStats {
        let win_rate = if self.trades_count > 0 {
            (self.wins as f64 / self.trades_count as f64) * 100.0
        } else {
            0.0
        };

        RsiStats {
            current_rsi: self.current_rsi,
            previous_rsi: self.previous_rsi,
            position_state: format!("{:?}", self.position_state),
            entry_price: self.entry_price,
            current_price: self.current_price,
            trades_count: self.trades_count,
            wins: self.wins,
            losses: self.trade_losses,
            win_rate,
            total_pnl: self.total_pnl,
            cooldown_remaining: self.cooldown_counter,
        }
    }
}

impl Default for RsiStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for RsiStrategy {
    fn name(&self) -> &str {
        "RSI Mean Reversion"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "RSI-based mean reversion strategy. Buys on oversold (RSI < 30), sells on overbought (RSI > 70)"
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let rsi_config: RsiConfig = serde_json::from_value(config)?;

        info!(
            symbol = %rsi_config.symbol,
            period = rsi_config.period,
            oversold = rsi_config.oversold_threshold,
            overbought = rsi_config.overbought_threshold,
            "Initializing RSI Mean Reversion strategy"
        );

        self.symbol = Symbol::from_string(&rsi_config.symbol, MarketType::Crypto);
        self.config = Some(rsi_config);
        self.initialized = true;

        // 상태 초기화
        self.close_history.clear();
        self.gains.clear();
        self.losses.clear();
        self.rsi_history.clear();
        self.current_rsi = None;
        self.previous_rsi = None;
        self.avg_gain = None;
        self.avg_loss = None;
        self.position_state = PositionState::Flat;
        self.entry_price = None;
        self.cooldown_counter = 0;

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

        // 종가 추출
        let close = match &data.data {
            MarketDataType::Kline(kline) => kline.close,
            MarketDataType::Ticker(ticker) => ticker.last,
            MarketDataType::Trade(trade) => trade.price,
            _ => return Ok(vec![]),
        };

        self.current_price = Some(close);

        // 가격 히스토리 업데이트
        self.close_history.push_front(close);
        let max_history = config.period + 10;
        while self.close_history.len() > max_history {
            self.close_history.pop_back();
        }

        // RSI 계산
        self.calculate_rsi();

        debug!(
            rsi = ?self.current_rsi,
            price = %close,
            history_len = self.close_history.len(),
            "RSI updated"
        );

        // 신호 생성
        let signals = self.generate_signals();

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let config = self.config.as_ref().unwrap();
        let fill_price = order.average_fill_price
            .or(order.price)
            .unwrap_or(Decimal::ZERO);

        match (order.side, self.position_state) {
            (Side::Buy, PositionState::Flat) => {
                // 롱 포지션 진입
                self.position_state = PositionState::Long;
                self.entry_price = Some(fill_price);
                self.trades_count += 1;

                info!(
                    price = %fill_price,
                    "Entered LONG position"
                );
            }
            (Side::Sell, PositionState::Long) => {
                // 롱 포지션 청산
                if let Some(entry) = self.entry_price {
                    let pnl = (fill_price - entry) * order.quantity;
                    self.total_pnl += pnl;

                    if pnl > Decimal::ZERO {
                        self.wins += 1;
                    } else {
                        self.trade_losses += 1;
                    }

                    info!(
                        entry = %entry,
                        exit = %fill_price,
                        pnl = %pnl,
                        "Exited LONG position"
                    );
                }

                self.position_state = PositionState::Flat;
                self.entry_price = None;
                self.cooldown_counter = config.cooldown_candles;
            }
            (Side::Sell, PositionState::Flat) => {
                // 숏 포지션 진입
                self.position_state = PositionState::Short;
                self.entry_price = Some(fill_price);
                self.trades_count += 1;

                info!(
                    price = %fill_price,
                    "Entered SHORT position"
                );
            }
            (Side::Buy, PositionState::Short) => {
                // 숏 포지션 청산
                if let Some(entry) = self.entry_price {
                    let pnl = (entry - fill_price) * order.quantity;
                    self.total_pnl += pnl;

                    if pnl > Decimal::ZERO {
                        self.wins += 1;
                    } else {
                        self.trade_losses += 1;
                    }

                    info!(
                        entry = %entry,
                        exit = %fill_price,
                        pnl = %pnl,
                        "Exited SHORT position"
                    );
                }

                self.position_state = PositionState::Flat;
                self.entry_price = None;
                self.cooldown_counter = config.cooldown_candles;
            }
            _ => {
                warn!(
                    side = ?order.side,
                    state = ?self.position_state,
                    "Unexpected order fill"
                );
            }
        }

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
        info!(
            trades = self.trades_count,
            wins = self.wins,
            losses = self.trade_losses,
            pnl = %self.total_pnl,
            "RSI Mean Reversion strategy shutdown"
        );

        self.initialized = false;

        Ok(())
    }

    fn get_state(&self) -> Value {
        let stats = self.get_stats();

        json!({
            "initialized": self.initialized,
            "symbol": self.config.as_ref().map(|c| &c.symbol),
            "stats": stats,
        })
    }

    fn save_state(&self) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let state = RsiState {
            position_state: format!("{:?}", self.position_state),
            entry_price: self.entry_price,
            trades_count: self.trades_count,
            wins: self.wins,
            trade_losses: self.trade_losses,
            total_pnl: self.total_pnl,
            close_history: self.close_history.iter().cloned().collect(),
            avg_gain: self.avg_gain,
            avg_loss: self.avg_loss,
        };

        Ok(serde_json::to_vec(&state)?)
    }

    fn load_state(&mut self, data: &[u8]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let state: RsiState = serde_json::from_slice(data)?;

        self.position_state = match state.position_state.as_str() {
            "Long" => PositionState::Long,
            "Short" => PositionState::Short,
            _ => PositionState::Flat,
        };
        self.entry_price = state.entry_price;
        self.trades_count = state.trades_count;
        self.wins = state.wins;
        self.trade_losses = state.trade_losses;
        self.total_pnl = state.total_pnl;
        self.close_history = state.close_history.into();
        self.avg_gain = state.avg_gain;
        self.avg_loss = state.avg_loss;

        Ok(())
    }
}

/// RSI 전략 통계.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RsiStats {
    pub current_rsi: Option<f64>,
    pub previous_rsi: Option<f64>,
    pub position_state: String,
    pub entry_price: Option<Decimal>,
    pub current_price: Option<Decimal>,
    pub trades_count: u32,
    pub wins: u32,
    pub losses: u32,
    pub win_rate: f64,
    pub total_pnl: Decimal,
    pub cooldown_remaining: usize,
}

/// 영속성을 위한 직렬화 가능한 RSI 상태.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RsiState {
    position_state: String,
    entry_price: Option<Decimal>,
    trades_count: u32,
    wins: u32,
    trade_losses: u32,
    total_pnl: Decimal,
    close_history: Vec<Decimal>,
    avg_gain: Option<Decimal>,
    avg_loss: Option<Decimal>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use trader_core::{Kline, Timeframe};

    fn create_kline(symbol: &Symbol, close: Decimal) -> MarketData {
        let kline = Kline::new(
            symbol.clone(),
            Timeframe::M1,
            Utc::now(),
            close,
            close + dec!(10),
            close - dec!(10),
            close,
            dec!(100),
            Utc::now(),
        );

        MarketData::from_kline("binance", kline)
    }

    #[tokio::test]
    async fn test_rsi_initialization() {
        let mut strategy = RsiStrategy::new();

        let config = json!({
            "symbol": "BTC/USDT",
            "period": 14,
            "oversold_threshold": 30.0,
            "overbought_threshold": 70.0,
            "amount": "100"
        });

        strategy.initialize(config).await.unwrap();

        assert!(strategy.initialized);
        assert_eq!(strategy.position_state, PositionState::Flat);
    }

    #[tokio::test]
    async fn test_rsi_calculation() {
        let mut strategy = RsiStrategy::new();

        let config = json!({
            "symbol": "BTC/USDT",
            "period": 14,
            "amount": "100"
        });

        strategy.initialize(config).await.unwrap();

        let symbol = Symbol::crypto("BTC", "USDT");

        // Feed 20 data points to calculate RSI
        let prices = vec![
            50000, 50100, 50050, 49900, 49800, 49850, 49950, 50000, 50100, 50200,
            50150, 50100, 50000, 49900, 49800, 49750, 49700, 49650, 49600, 49550,
        ];

        for price in prices {
            let data = create_kline(&symbol, Decimal::from(price));
            strategy.on_market_data(&data).await.unwrap();
        }

        // RSI should be calculated now
        assert!(strategy.current_rsi.is_some());
        let rsi = strategy.current_rsi.unwrap();
        assert!(rsi >= 0.0 && rsi <= 100.0);
    }

    #[tokio::test]
    async fn test_oversold_signal() {
        let mut strategy = RsiStrategy::new();

        let config = json!({
            "symbol": "BTC/USDT",
            "period": 3,  // Short period for testing
            "oversold_threshold": 30.0,
            "overbought_threshold": 70.0,
            "amount": "100"
        });

        strategy.initialize(config).await.unwrap();

        let symbol = Symbol::crypto("BTC", "USDT");

        // Feed declining prices to get oversold
        let prices = vec![50000, 49000, 48000, 47000, 46000, 45000];

        let mut signals = Vec::new();
        for price in prices {
            let data = create_kline(&symbol, Decimal::from(price));
            let s = strategy.on_market_data(&data).await.unwrap();
            signals.extend(s);
        }

        // Should have generated a buy signal when RSI went oversold
        if let Some(rsi) = strategy.current_rsi {
            if rsi < 30.0 {
                assert!(!signals.is_empty());
                assert_eq!(signals[0].side, Side::Buy);
            }
        }
    }

    #[test]
    fn test_rsi_config_defaults() {
        let config = RsiConfig::default();

        assert_eq!(config.period, 14);
        assert_eq!(config.oversold_threshold, 30.0);
        assert_eq!(config.overbought_threshold, 70.0);
        assert!(config.use_ema_smoothing);
    }
}
