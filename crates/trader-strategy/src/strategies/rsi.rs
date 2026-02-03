//! RSI 평균회귀 전략.
//!
//! RSI가 과매도 상태(< 30)일 때 매수하고
//! RSI가 과매수 상태(> 70)일 때 매도하는 전략입니다.
//!
//! # StrategyContext 활용
//!
//! - `StructuralFeatures.rsi`: RSI 값 (Context 동기화)
//! - `RouteState`: 진입 가능 여부 판단
//! - `GlobalScore`: 종목 품질 필터링

use crate::strategies::common::deserialize_symbol;
use crate::Strategy;
use async_trait::async_trait;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use trader_core::{
    domain::{RouteState, StrategyContext},
    MarketData, MarketDataType, MarketType, Order, Position, Side, Signal, SignalType, Symbol,
};

/// RSI 전략 설정.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RsiConfig {
    /// 거래할 심볼 (ticker)
    #[serde(deserialize_with = "deserialize_symbol")]
    pub symbol: String,

    /// 과매도 임계값 (RSI가 이 값 아래로 떨어지면 매수)
    #[serde(default = "default_oversold")]
    pub oversold_threshold: Decimal,

    /// 과매수 임계값 (RSI가 이 값 위로 올라가면 매도)
    #[serde(default = "default_overbought")]
    pub overbought_threshold: Decimal,

    /// 거래 금액 (호가 통화 기준)
    pub amount: Decimal,

    /// RSI가 중립(50)을 교차할 때 청산
    #[serde(default)]
    pub exit_on_neutral: bool,

    /// 손절 비율 (진입 가격 기준)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_loss_pct: Option<Decimal>,

    /// 익절 비율 (진입 가격 기준)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub take_profit_pct: Option<Decimal>,

    /// 최소 GlobalScore (기본값: 60)
    #[serde(default = "default_min_score")]
    pub min_global_score: Decimal,

    /// 거래 후 쿨다운 기간 (캔들 수 기준)
    #[serde(default = "default_cooldown")]
    pub cooldown_candles: usize,
}

fn default_oversold() -> Decimal {
    dec!(30)
}
fn default_overbought() -> Decimal {
    dec!(70)
}
fn default_min_score() -> Decimal {
    dec!(60)
}
fn default_cooldown() -> usize {
    5
}

impl Default for RsiConfig {
    fn default() -> Self {
        Self {
            symbol: "BTC/USDT".to_string(),
            oversold_threshold: dec!(30),
            overbought_threshold: dec!(70),
            amount: dec!(100),
            exit_on_neutral: false,
            stop_loss_pct: None,
            take_profit_pct: None,
            min_global_score: dec!(60),
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
///
/// StrategyContext를 통해 RSI, RouteState, GlobalScore를 조회합니다.
/// 직접 RSI를 계산하지 않고 Context에서 동기화된 데이터를 사용합니다.
pub struct RsiStrategy {
    /// 전략 설정
    config: Option<RsiConfig>,

    /// 거래 중인 심볼
    symbol: Option<Symbol>,

    /// 전략 실행 컨텍스트
    context: Option<Arc<RwLock<StrategyContext>>>,

    /// 현재 포지션 상태
    position_state: PositionState,

    /// 진입 가격
    entry_price: Option<Decimal>,

    /// 현재 가격
    current_price: Option<Decimal>,

    /// 이전 RSI (크로스오버 감지용)
    previous_rsi: Option<Decimal>,

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

    /// 초기화 플래그
    initialized: bool,
}

impl RsiStrategy {
    /// 새 RSI 전략 생성.
    pub fn new() -> Self {
        Self {
            config: None,
            symbol: None,
            context: None,
            position_state: PositionState::Flat,
            entry_price: None,
            current_price: None,
            previous_rsi: None,
            cooldown_counter: 0,
            trades_count: 0,
            wins: 0,
            trade_losses: 0,
            total_pnl: Decimal::ZERO,
            initialized: false,
        }
    }

    /// StrategyContext에서 현재 RSI 조회.
    ///
    /// StructuralFeatures.rsi를 사용합니다.
    fn get_current_rsi(&self) -> Option<Decimal> {
        let config = self.config.as_ref()?;
        let ticker = &config.symbol;

        let ctx = self.context.as_ref()?;
        let ctx_lock = ctx.try_read().ok()?;

        ctx_lock
            .get_features(ticker)
            .map(|f| f.rsi)
    }

    /// RouteState와 GlobalScore를 체크하여 진입 가능 여부 반환.
    ///
    /// # 진입 조건
    ///
    /// - RouteState::Attack: 적극 진입 가능
    /// - RouteState::Armed: 조건부 허용
    /// - RouteState::Overheat/Wait/Neutral: 진입 금지
    /// - GlobalScore >= min_global_score: 진입 허용
    fn can_enter(&self) -> bool {
        let Some(config) = self.config.as_ref() else {
            return false;
        };
        let ticker = &config.symbol;

        let Some(ctx) = self.context.as_ref() else {
            warn!("StrategyContext not available - entry blocked");
            return false;
        };

        let Ok(ctx_lock) = ctx.try_read() else {
            warn!("Failed to acquire context lock - entry blocked");
            return false;
        };

        // RouteState 체크
        if let Some(route_state) = ctx_lock.get_route_state(ticker) {
            match route_state {
                RouteState::Overheat | RouteState::Wait | RouteState::Neutral => {
                    debug!(
                        ticker = %ticker,
                        route_state = ?route_state,
                        "RouteState blocks entry"
                    );
                    return false;
                }
                RouteState::Armed => {
                    debug!(ticker = %ticker, "RouteState::Armed - conditional entry");
                }
                RouteState::Attack => {
                    debug!(ticker = %ticker, "RouteState::Attack - aggressive entry");
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
                    "Low GlobalScore - skip entry"
                );
                return false;
            }
            debug!(
                ticker = %ticker,
                score = %score.overall_score,
                "GlobalScore pass"
            );
        }

        true
    }

    /// Overheat 상태인지 체크 (청산 고려용).
    fn is_overheat(&self) -> bool {
        let Some(config) = self.config.as_ref() else {
            return false;
        };
        let ticker = &config.symbol;

        let Some(ctx) = self.context.as_ref() else {
            return false;
        };

        let Ok(ctx_lock) = ctx.try_read() else {
            return false;
        };

        matches!(
            ctx_lock.get_route_state(ticker),
            Some(RouteState::Overheat)
        )
    }

    /// RSI를 기반으로 트레이딩 신호 생성.
    fn generate_signals(&mut self) -> Vec<Signal> {
        let Some(config) = self.config.as_ref() else {
            return Vec::new();
        };
        let Some(symbol) = self.symbol.as_ref() else {
            return Vec::new();
        };

        // Context에서 RSI 조회
        let rsi = match self.get_current_rsi() {
            Some(r) => r,
            None => {
                debug!("RSI not available from Context");
                return Vec::new();
            }
        };

        let current_price = match self.current_price {
            Some(p) => p,
            None => return Vec::new(),
        };

        let mut signals = Vec::new();

        // 쿨다운 확인
        if self.cooldown_counter > 0 {
            self.cooldown_counter -= 1;
            return signals;
        }

        // RSI 중립점
        let neutral = dec!(50);

        match self.position_state {
            PositionState::Flat => {
                // StrategyContext 기반 필터링
                if !self.can_enter() {
                    // 이전 RSI 저장
                    self.previous_rsi = Some(rsi);
                    return signals;
                }

                // 과매도 확인 (매수 신호)
                if rsi < config.oversold_threshold {
                    // RSI가 과매도에서 상향 크로스하는지 확인
                    let crossing_up = self
                        .previous_rsi
                        .map(|prev| prev < config.oversold_threshold && rsi > prev)
                        .unwrap_or(true);

                    if crossing_up {
                        let strength = ((config.oversold_threshold - rsi) / config.oversold_threshold)
                            .to_string()
                            .parse::<f64>()
                            .unwrap_or(0.5);

                        let mut signal = Signal::new(
                            "rsi_mean_reversion",
                            symbol.clone(),
                            Side::Buy,
                            SignalType::Entry,
                        )
                        .with_strength(strength)
                        .with_metadata("rsi", json!(rsi.to_string()))
                        .with_metadata("reason", json!("oversold"));

                        // 손절 설정
                        if let Some(sl_pct) = config.stop_loss_pct {
                            let sl_price = current_price * (Decimal::ONE - sl_pct / dec!(100));
                            signal.stop_loss = Some(sl_price);
                        }

                        // 익절 설정
                        if let Some(tp_pct) = config.take_profit_pct {
                            let tp_price = current_price * (Decimal::ONE + tp_pct / dec!(100));
                            signal.take_profit = Some(tp_price);
                        }

                        signals.push(signal);

                        info!(
                            rsi = %rsi,
                            price = %current_price,
                            "RSI oversold - BUY signal"
                        );
                    }
                }

                // 과매수 확인 (매도/숏 신호)
                if rsi > config.overbought_threshold {
                    let crossing_down = self
                        .previous_rsi
                        .map(|prev| prev > config.overbought_threshold && rsi < prev)
                        .unwrap_or(true);

                    if crossing_down {
                        let strength = ((rsi - config.overbought_threshold) / (dec!(100) - config.overbought_threshold))
                            .to_string()
                            .parse::<f64>()
                            .unwrap_or(0.5);

                        let signal = Signal::new(
                            "rsi_mean_reversion",
                            symbol.clone(),
                            Side::Sell,
                            SignalType::Entry,
                        )
                        .with_strength(strength)
                        .with_metadata("rsi", json!(rsi.to_string()))
                        .with_metadata("reason", json!("overbought"));

                        signals.push(signal);

                        info!(
                            rsi = %rsi,
                            price = %current_price,
                            "RSI overbought - SELL signal"
                        );
                    }
                }
            }

            PositionState::Long => {
                // Overheat 상태면 즉시 청산
                let overheat = self.is_overheat();

                // 롱 포지션 청산 조건
                let should_exit = if overheat {
                    debug!("Overheat detected - exit signal");
                    true
                } else if config.exit_on_neutral {
                    // RSI가 중립(50) 위로 크로스할 때 청산
                    rsi >= neutral && self.previous_rsi.unwrap_or(neutral) < neutral
                } else {
                    // RSI가 과매수에 도달할 때 청산
                    rsi >= config.overbought_threshold
                };

                // 손절 확인
                let stop_hit = if let (Some(entry), Some(sl_pct)) =
                    (self.entry_price, config.stop_loss_pct)
                {
                    let sl_price = entry * (Decimal::ONE - sl_pct / dec!(100));
                    current_price <= sl_price
                } else {
                    false
                };

                // 익절 확인
                let tp_hit = if let (Some(entry), Some(tp_pct)) =
                    (self.entry_price, config.take_profit_pct)
                {
                    let tp_price = entry * (Decimal::ONE + tp_pct / dec!(100));
                    current_price >= tp_price
                } else {
                    false
                };

                if should_exit || stop_hit || tp_hit {
                    let reason = if stop_hit {
                        "stop_loss"
                    } else if tp_hit {
                        "take_profit"
                    } else if overheat {
                        "overheat"
                    } else {
                        "rsi_exit"
                    };

                    let signal = Signal::new(
                        "rsi_mean_reversion",
                        symbol.clone(),
                        Side::Sell,
                        SignalType::Exit,
                    )
                    .with_strength(1.0)
                    .with_metadata("rsi", json!(rsi.to_string()))
                    .with_metadata("reason", json!(reason));

                    signals.push(signal);

                    info!(
                        rsi = %rsi,
                        price = %current_price,
                        reason = reason,
                        "Exiting LONG position"
                    );
                }
            }

            PositionState::Short => {
                // 숏 포지션 청산 조건
                let should_exit = if config.exit_on_neutral {
                    rsi <= neutral && self.previous_rsi.unwrap_or(neutral) > neutral
                } else {
                    rsi <= config.oversold_threshold
                };

                if should_exit {
                    let signal = Signal::new(
                        "rsi_mean_reversion",
                        symbol.clone(),
                        Side::Buy,
                        SignalType::Exit,
                    )
                    .with_strength(1.0)
                    .with_metadata("rsi", json!(rsi.to_string()))
                    .with_metadata("reason", json!("rsi_exit"));

                    signals.push(signal);

                    info!(
                        rsi = %rsi,
                        price = %current_price,
                        "Exiting SHORT position"
                    );
                }
            }
        }

        // 이전 RSI 저장
        self.previous_rsi = Some(rsi);

        signals
    }

    /// 전략 통계 조회.
    fn get_stats(&self) -> RsiStats {
        let win_rate = if self.trades_count > 0 {
            Decimal::from(self.wins) / Decimal::from(self.trades_count) * dec!(100)
        } else {
            Decimal::ZERO
        };

        RsiStats {
            current_rsi: self.get_current_rsi(),
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
        "2.0.0"
    }

    fn description(&self) -> &str {
        "RSI-based mean reversion strategy using StrategyContext for data synchronization"
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let rsi_config: RsiConfig = serde_json::from_value(config)?;

        info!(
            symbol = %rsi_config.symbol,
            oversold = %rsi_config.oversold_threshold,
            overbought = %rsi_config.overbought_threshold,
            min_score = %rsi_config.min_global_score,
            "Initializing RSI Mean Reversion strategy v2.0"
        );

        self.symbol = Symbol::from_string(&rsi_config.symbol, MarketType::Crypto);
        self.config = Some(rsi_config);
        self.initialized = true;

        // 상태 초기화
        self.position_state = PositionState::Flat;
        self.entry_price = None;
        self.previous_rsi = None;
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

        let Some(config) = self.config.as_ref() else {
            return Ok(vec![]);
        };

        // 심볼 확인
        if data.symbol.to_string() != config.symbol {
            return Ok(vec![]);
        }

        // 가격 추출
        let close = match &data.data {
            MarketDataType::Kline(kline) => kline.close,
            MarketDataType::Ticker(ticker) => ticker.last,
            MarketDataType::Trade(trade) => trade.price,
            _ => return Ok(vec![]),
        };

        self.current_price = Some(close);

        debug!(
            price = %close,
            rsi = ?self.get_current_rsi(),
            "Market data received"
        );

        // 신호 생성
        let signals = self.generate_signals();

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(config) = self.config.as_ref() else {
            warn!("Order filled but strategy not configured");
            return Ok(());
        };

        let fill_price = order
            .average_fill_price
            .or(order.price)
            .unwrap_or(Decimal::ZERO);

        match (order.side, self.position_state) {
            (Side::Buy, PositionState::Flat) => {
                self.position_state = PositionState::Long;
                self.entry_price = Some(fill_price);
                self.trades_count += 1;

                info!(
                    price = %fill_price,
                    "Entered LONG position"
                );
            }
            (Side::Sell, PositionState::Long) => {
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
                self.position_state = PositionState::Short;
                self.entry_price = Some(fill_price);
                self.trades_count += 1;

                info!(
                    price = %fill_price,
                    "Entered SHORT position"
                );
            }
            (Side::Buy, PositionState::Short) => {
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

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("StrategyContext injected into RSI strategy");
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
            previous_rsi: self.previous_rsi,
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
        self.previous_rsi = state.previous_rsi;

        Ok(())
    }
}

/// RSI 전략 통계.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RsiStats {
    pub current_rsi: Option<Decimal>,
    pub previous_rsi: Option<Decimal>,
    pub position_state: String,
    pub entry_price: Option<Decimal>,
    pub current_price: Option<Decimal>,
    pub trades_count: u32,
    pub wins: u32,
    pub losses: u32,
    pub win_rate: Decimal,
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
    previous_rsi: Option<Decimal>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use trader_core::{Kline, Timeframe};

    fn create_kline(symbol: &Symbol, close: Decimal) -> MarketData {
        let now = Utc::now();
        let kline = Kline::new(
            symbol.clone(),
            Timeframe::M1,
            now,
            close,
            close + dec!(10),
            close - dec!(10),
            close,
            dec!(1000),
            now, // close_time
        );

        MarketData {
            exchange: "test".to_string(),
            symbol: symbol.clone(),
            timestamp: now,
            data: MarketDataType::Kline(kline),
        }
    }

    #[test]
    fn test_default_config() {
        let config = RsiConfig::default();
        assert_eq!(config.oversold_threshold, dec!(30));
        assert_eq!(config.overbought_threshold, dec!(70));
        assert_eq!(config.min_global_score, dec!(60));
    }

    #[tokio::test]
    async fn test_strategy_initialization() {
        let mut strategy = RsiStrategy::new();

        let config = json!({
            "symbol": "BTC/USDT",
            "amount": "100",
            "oversold_threshold": 25,
            "overbought_threshold": 75
        });

        let result = strategy.initialize(config).await;
        assert!(result.is_ok());
        assert!(strategy.initialized);
    }

    #[test]
    fn test_position_state() {
        let strategy = RsiStrategy::new();
        assert_eq!(strategy.position_state, PositionState::Flat);
    }
}
