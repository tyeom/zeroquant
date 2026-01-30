//! 트레일링 스탑 시스템 전략
//!
//! 최고가 기준으로 설정된 비율 이상 하락 시 자동 청산하는 손절매 시스템입니다.
//!
//! # 핵심 로직
//! - 진입 후 최고가를 지속적으로 추적
//! - 최고가 대비 설정된 비율(trailing_stop_pct) 이상 하락 시 청산
//! - 수익 실현 시 트레일링 스탑 비율을 동적으로 조정 가능
//!
//! # 파라미터
//! - `trailing_stop_pct`: 트레일링 스탑 비율 (기본 5%)
//! - `activation_price`: 트레일링 스탑 활성화 가격 (선택)
//! - `profit_lock_threshold`: 수익 확정 임계값 (선택)

use crate::strategies::common::deserialize_symbol;
use crate::Strategy;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use trader_core::{MarketData, MarketDataType, MarketType, Order, Position, Side, Signal, SignalType, Symbol};
use tracing::{debug, info, warn};

/// 트레일링 스탑 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrailingStopConfig {
    /// 대상 심볼
    #[serde(deserialize_with = "deserialize_symbol")]
    pub symbol: String,

    /// 초기 트레일링 스탑 비율 (%)
    #[serde(default = "default_trailing_stop_pct")]
    pub trailing_stop_pct: Decimal,

    /// 최대 트레일링 스탑 비율 (%)
    #[serde(default = "default_max_trailing_stop_pct")]
    pub max_trailing_stop_pct: Decimal,

    /// 수익률 조정 기준 (%) - 이 수익률 도달 시 트레일링 스탑 축소
    #[serde(default = "default_profit_rate_adjustment")]
    pub profit_rate_adjustment: Decimal,

    /// 트레일링 스탑 활성화 가격 (선택)
    pub activation_price: Option<Decimal>,

    /// 수익 확정 임계값 (%) - 이 수익률 도달 시 일부 익절
    pub profit_lock_threshold: Option<Decimal>,

    /// 익절 시 매도 비율 (%)
    #[serde(default = "default_profit_lock_sell_pct")]
    pub profit_lock_sell_pct: Decimal,

    /// 주문당 투자 금액
    #[serde(default = "default_amount")]
    pub amount: Decimal,
}

fn default_trailing_stop_pct() -> Decimal { dec!(5.0) }
fn default_max_trailing_stop_pct() -> Decimal { dec!(10.0) }
fn default_profit_rate_adjustment() -> Decimal { dec!(2.0) }
fn default_profit_lock_sell_pct() -> Decimal { dec!(50.0) }
fn default_amount() -> Decimal { dec!(1000000) }

impl Default for TrailingStopConfig {
    fn default() -> Self {
        Self {
            symbol: "005930".to_string(),
            trailing_stop_pct: default_trailing_stop_pct(),
            max_trailing_stop_pct: default_max_trailing_stop_pct(),
            profit_rate_adjustment: default_profit_rate_adjustment(),
            activation_price: None,
            profit_lock_threshold: None,
            profit_lock_sell_pct: default_profit_lock_sell_pct(),
            amount: default_amount(),
        }
    }
}

/// 포지션 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrailingPositionState {
    /// 진입 가격
    pub entry_price: Decimal,
    /// 진입 시간
    pub entry_time: DateTime<Utc>,
    /// 보유 수량
    pub quantity: Decimal,
    /// 최고가 (트레일링 스탑 기준)
    pub highest_price: Decimal,
    /// 현재 트레일링 스탑 비율
    pub current_trailing_stop_pct: Decimal,
    /// 트레일링 스탑 활성화 여부
    pub trailing_active: bool,
    /// 익절 실행 여부
    pub profit_locked: bool,
}

/// 트레일링 스탑 전략
pub struct TrailingStopStrategy {
    config: Option<TrailingStopConfig>,
    symbol: Option<Symbol>,
    position: Option<TrailingPositionState>,
    last_price: Option<Decimal>,
    initialized: bool,
}

impl TrailingStopStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            symbol: None,
            position: None,
            last_price: None,
            initialized: false,
        }
    }

    /// 트레일링 스탑 가격 계산 (순수 함수)
    fn calculate_stop_price(highest_price: Decimal, current_trailing_stop_pct: Decimal) -> Decimal {
        let stop_pct = current_trailing_stop_pct / dec!(100);
        highest_price * (dec!(1) - stop_pct)
    }

    /// 수익률 계산 (순수 함수)
    fn calculate_profit_rate(current_price: Decimal, entry_price: Decimal) -> Decimal {
        if entry_price.is_zero() {
            return dec!(0);
        }
        ((current_price - entry_price) / entry_price) * dec!(100)
    }

    /// 트레일링 스탑 비율 동적 조정 (config를 직접 받음)
    fn adjust_trailing_stop(config: &TrailingStopConfig, profit_rate: Decimal) -> Decimal {
        // 수익률이 조정 기준을 넘으면 트레일링 스탑 축소
        if profit_rate >= config.profit_rate_adjustment {
            // 수익률 2%당 0.5% 축소 (최소 2%까지)
            let reduction = (profit_rate / config.profit_rate_adjustment).floor() * dec!(0.5);
            let new_stop = config.trailing_stop_pct - reduction;
            new_stop.max(dec!(2.0))
        } else {
            config.trailing_stop_pct
        }
    }

    /// 신호 생성
    fn generate_signals(&mut self, current_price: Decimal) -> Vec<Signal> {
        let config = self.config.as_ref().unwrap();
        let symbol = self.symbol.as_ref().unwrap();
        let mut signals = Vec::new();

        match &mut self.position {
            None => {
                // 포지션 없음 - 진입 신호 확인
                if let Some(activation) = config.activation_price {
                    if current_price < activation {
                        return signals;
                    }
                }

                let signal = Signal::new(
                    "trailing_stop",
                    symbol.clone(),
                    Side::Buy,
                    SignalType::Entry,
                )
                .with_strength(1.0)
                .with_metadata("reason", json!("initial_entry"))
                .with_metadata("trailing_stop_pct", json!(config.trailing_stop_pct));

                signals.push(signal);
                info!("[TrailingStop] 진입 신호 생성: 가격 {}", current_price);
            }
            Some(state) => {
                // 필요한 값들을 먼저 복사 (borrow checker 회피)
                let entry_price = state.entry_price;
                let trailing_active = state.trailing_active;
                let highest_price_for_stop = state.highest_price;
                let current_stop_pct = state.current_trailing_stop_pct;

                // 최고가 업데이트
                if current_price > state.highest_price {
                    state.highest_price = current_price;
                    debug!(
                        "[TrailingStop] 최고가 갱신: {}",
                        current_price
                    );
                }

                // 수익률 계산 (순수 함수 호출)
                let profit_rate = Self::calculate_profit_rate(current_price, entry_price);

                // 트레일링 스탑 활성화 확인
                if !state.trailing_active {
                    if let Some(activation) = config.activation_price {
                        if current_price >= activation {
                            state.trailing_active = true;
                            info!("[TrailingStop] 트레일링 스탑 활성화 (가격: {})", current_price);
                        }
                    } else {
                        state.trailing_active = true;
                    }
                }

                // 트레일링 스탑 비율 동적 조정 (config 직접 전달)
                let new_trailing_stop_pct = Self::adjust_trailing_stop(config, profit_rate);
                state.current_trailing_stop_pct = new_trailing_stop_pct;

                // 익절 확인 (한 번만 실행)
                if !state.profit_locked {
                    if let Some(threshold) = config.profit_lock_threshold {
                        if profit_rate >= threshold {
                            state.profit_locked = true;

                            let signal = Signal::new(
                                "trailing_stop",
                                symbol.clone(),
                                Side::Sell,
                                SignalType::ReducePosition,
                            )
                            .with_strength(1.0)
                            .with_metadata("reason", json!("profit_lock"))
                            .with_metadata("profit_rate", json!(profit_rate.to_string()));

                            signals.push(signal);
                            info!(
                                "[TrailingStop] 부분 익절 실행: 수익률 {:.2}%, 매도 비율 {}%",
                                profit_rate, config.profit_lock_sell_pct
                            );
                        }
                    }
                }

                // 트레일링 스탑 확인 (업데이트된 값 사용)
                if state.trailing_active {
                    let stop_price = Self::calculate_stop_price(state.highest_price, state.current_trailing_stop_pct);

                    if current_price <= stop_price {
                        let signal = Signal::new(
                            "trailing_stop",
                            symbol.clone(),
                            Side::Sell,
                            SignalType::Exit,
                        )
                        .with_strength(1.0)
                        .with_metadata("reason", json!("trailing_stop_triggered"))
                        .with_metadata("entry_price", json!(state.entry_price.to_string()))
                        .with_metadata("highest_price", json!(state.highest_price.to_string()))
                        .with_metadata("stop_price", json!(stop_price.to_string()));

                        signals.push(signal);

                        warn!(
                            "[TrailingStop] 트레일링 스탑 발동! 현재가: {}, 스탑가: {}, 최고가: {}",
                            current_price, stop_price, state.highest_price
                        );
                    }
                }
            }
        }

        signals
    }
}

impl Default for TrailingStopStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for TrailingStopStrategy {
    fn name(&self) -> &str {
        "Trailing Stop System"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "최고가 기준 트레일링 스탑 손절매 시스템. 수익 시 트레일링 비율 자동 조정."
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ts_config: TrailingStopConfig = serde_json::from_value(config)?;

        info!(
            symbol = %ts_config.symbol,
            trailing_stop_pct = %ts_config.trailing_stop_pct,
            "Initializing Trailing Stop strategy"
        );

        self.symbol = Some(Symbol::stock(&ts_config.symbol, "KRW"));
        self.config = Some(ts_config);
        self.position = None;
        self.last_price = None;
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
        let current_price = match &data.data {
            MarketDataType::Kline(kline) => kline.close,
            MarketDataType::Ticker(ticker) => ticker.last,
            MarketDataType::Trade(trade) => trade.price,
            _ => return Ok(vec![]),
        };

        self.last_price = Some(current_price);

        // 신호 생성
        let signals = self.generate_signals(current_price);

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

        match order.side {
            Side::Buy => {
                // 진입
                self.position = Some(TrailingPositionState {
                    entry_price: fill_price,
                    entry_time: Utc::now(),
                    quantity: order.quantity,
                    highest_price: fill_price,
                    current_trailing_stop_pct: config.trailing_stop_pct,
                    trailing_active: config.activation_price.is_none(),
                    profit_locked: false,
                });
                info!("[TrailingStop] 진입 완료: 가격 {}, 수량 {}", fill_price, order.quantity);
            }
            Side::Sell => {
                // 청산
                if let Some(state) = &self.position {
                    let pnl = (fill_price - state.entry_price) * order.quantity;
                    info!(
                        "[TrailingStop] 청산 완료: 진입가 {}, 청산가 {}, PnL {}",
                        state.entry_price, fill_price, pnl
                    );
                }

                // 전량 청산 확인
                if let Some(state) = &mut self.position {
                    state.quantity -= order.quantity;
                    if state.quantity <= dec!(0) {
                        self.position = None;
                    }
                }
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
            "Trailing Stop strategy shutdown"
        );
        self.initialized = false;
        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "config": self.config,
            "position": self.position,
            "last_price": self.last_price,
            "initialized": self.initialized,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trader_core::{Kline, Timeframe};

    fn create_kline(symbol: &Symbol, close: Decimal) -> MarketData {
        let kline = Kline::new(
            symbol.clone(),
            Timeframe::D1,
            Utc::now(),
            close,
            close * dec!(1.01),
            close * dec!(0.99),
            close,
            dec!(1000000),
            Utc::now(),
        );
        MarketData::from_kline("kis", kline)
    }

    #[tokio::test]
    async fn test_initialization() {
        let mut strategy = TrailingStopStrategy::new();

        let config = json!({
            "symbol": "005930/KRW",
            "trailing_stop_pct": "5",
            "amount": "1000000"
        });

        strategy.initialize(config).await.unwrap();
        assert!(strategy.initialized);
    }

    #[tokio::test]
    async fn test_trailing_stop_trigger() {
        let mut strategy = TrailingStopStrategy::new();

        let config = json!({
            "symbol": "005930/KRW",
            "trailing_stop_pct": "5",
            "amount": "1000000"
        });

        strategy.initialize(config).await.unwrap();

        let symbol = Symbol::stock("005930", "KRW");

        // 진입
        let data = create_kline(&symbol, dec!(100000));
        let signals = strategy.on_market_data(&data).await.unwrap();
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].signal_type, SignalType::Entry);
    }

    #[test]
    fn test_dynamic_trailing_adjustment() {
        let mut strategy = TrailingStopStrategy::new();
        strategy.config = Some(TrailingStopConfig {
            trailing_stop_pct: dec!(5),
            profit_rate_adjustment: dec!(2),
            ..Default::default()
        });

        let config = strategy.config.as_ref().unwrap();

        // 수익률 0% - 기본 트레일링 5%
        assert_eq!(TrailingStopStrategy::adjust_trailing_stop(config, dec!(0)), dec!(5));

        // 수익률 2% - 트레일링 4.5%
        assert_eq!(TrailingStopStrategy::adjust_trailing_stop(config, dec!(2)), dec!(4.5));

        // 수익률 4% - 트레일링 4%
        assert_eq!(TrailingStopStrategy::adjust_trailing_stop(config, dec!(4)), dec!(4));
    }
}
