//! 변동성 돌파 전략 (래리 윌리엄스)
//!
//! 가격이 변동성 범위를 돌파할 때 진입하는 모멘텀 전략.
//! 래리 윌리엄스가 선물 거래를 위해 개발한 전략.
//!
//! # 전략 로직
//! - **레인지**: 이전 기간의 (고가 - 저가)
//! - **진입**: 시가 + (레인지 × K), K는 일반적으로 0.5
//! - **청산**: 기간 종료 시 또는 손절
//!
//! # 암호화폐 거래 장점
//! - 강한 모멘텀 움직임 포착
//! - 추세장에서 효과적
//! - 단순하고 견고한 로직
//! - 낮은 거래 빈도 (수수료 절감)
//!
//! # 권장 타임프레임
//! - 암호화폐: 1H, 4H, 1D
//! - K 계수 조정으로 더 짧은 타임프레임에도 적용 가능

use std::sync::Arc;
use tokio::sync::RwLock;
use trader_core::domain::{RouteState, StrategyContext};
use crate::strategies::common::deserialize_symbol;
use crate::Strategy;
use async_trait::async_trait;
use chrono::{DateTime, Timelike, Utc};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;
use tracing::{debug, info};
use trader_core::{MarketData, MarketDataType, MarketType, Order, Position, Side, Signal, Symbol};

/// 변동성 돌파 전략 설정.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VolatilityBreakoutConfig {
    /// 거래 심볼 (예: "BTC/USDT")
    #[serde(deserialize_with = "deserialize_symbol")]
    pub symbol: String,

    /// 돌파 K 계수 (기본값: 0.5)
    /// K가 높을수록 신호는 적지만 더 강한 신호
    #[serde(default = "default_k_factor")]
    pub k_factor: f64,

    /// 레인지 계산을 위한 룩백 기간 (기본값: 1)
    #[serde(default = "default_lookback")]
    pub lookback_period: usize,

    /// 단순 레인지 대신 ATR 사용 (기본값: false)
    #[serde(default)]
    pub use_atr: bool,

    /// ATR 사용 시 ATR 기간 (기본값: 14)
    #[serde(default = "default_atr_period")]
    pub atr_period: usize,

    /// 레인지의 배수로 손절 설정 (기본값: 1.0)
    #[serde(default = "default_stop_multiplier")]
    pub stop_loss_multiplier: f64,

    /// 레인지의 배수로 익절 설정 (기본값: 2.0)
    #[serde(default = "default_tp_multiplier")]
    pub take_profit_multiplier: f64,

    /// 기간 종료 시 청산 (기본값: true)
    #[serde(default = "default_exit_at_close")]
    pub exit_at_period_close: bool,

    /// 롱/숏 양방향 거래 (기본값: true)
    #[serde(default = "default_trade_both")]
    pub trade_both_directions: bool,

    /// 가격 대비 최소 레인지 비율 (기본값: 0.5%)
    #[serde(default = "default_min_range")]
    pub min_range_pct: f64,

    /// 가격 대비 최대 레인지 비율 (기본값: 10%)
    #[serde(default = "default_max_range")]
    pub max_range_pct: f64,

    /// 거래량 필터 사용 (고거래량에서만 거래)
    #[serde(default)]
    pub use_volume_filter: bool,

    /// 거래량 배수 임계값 (기본값: 평균의 1.5배)
    #[serde(default = "default_volume_multiplier")]
    pub volume_multiplier: f64,

    /// 최소 GlobalScore (기본값: 50)
    #[serde(default = "default_min_global_score")]
    pub min_global_score: Decimal,
}

fn default_k_factor() -> f64 {
    0.5
}
fn default_lookback() -> usize {
    1
}
fn default_atr_period() -> usize {
    14
}
fn default_stop_multiplier() -> f64 {
    1.0
}
fn default_tp_multiplier() -> f64 {
    2.0
}
fn default_exit_at_close() -> bool {
    true
}
fn default_trade_both() -> bool {
    true
}
fn default_min_range() -> f64 {
    0.5
}
fn default_max_range() -> f64 {
    10.0
}
fn default_volume_multiplier() -> f64 {
    1.5
}
fn default_min_global_score() -> Decimal {
    dec!(50)
}

impl Default for VolatilityBreakoutConfig {
    fn default() -> Self {
        Self {
            symbol: "BTC/USDT".to_string(),
            k_factor: 0.5,
            lookback_period: 1,
            use_atr: false,
            atr_period: 14,
            stop_loss_multiplier: 1.0,
            take_profit_multiplier: 2.0,
            exit_at_period_close: true,
            trade_both_directions: true,
            min_range_pct: 0.5,
            max_range_pct: 10.0,
            use_volume_filter: false,
            volume_multiplier: 1.5,
            min_global_score: dec!(50),
        }
    }
}

/// 기간별 OHLCV 데이터.
#[derive(Debug, Clone)]
struct PeriodData {
    open: Decimal,
    high: Decimal,
    low: Decimal,
    close: Decimal,
    volume: Decimal,
    timestamp: DateTime<Utc>,
}

/// 포지션 상태.
#[derive(Debug, Clone)]
struct PositionState {
    side: Side,
    entry_price: Decimal,
    stop_loss: Decimal,
    take_profit: Decimal,
    #[allow(dead_code)]
    entry_time: DateTime<Utc>,
}

/// 변동성 돌파 전략.
pub struct VolatilityBreakoutStrategy {
    config: Option<VolatilityBreakoutConfig>,
    symbol: Option<Symbol>,
    context: Option<Arc<RwLock<StrategyContext>>>,

    /// 과거 기간 데이터
    period_history: VecDeque<PeriodData>,

    /// 현재 기간 데이터 (구축 중)
    current_period: Option<PeriodData>,

    /// 이전 기간의 레인지
    prev_range: Option<Decimal>,

    /// 현재 ATR 값
    current_atr: Option<Decimal>,

    /// ATR 계산을 위한 True Range 히스토리
    tr_history: VecDeque<Decimal>,

    /// 평균 거래량
    avg_volume: Option<Decimal>,
    volume_history: VecDeque<Decimal>,

    /// 현재 포지션
    position: Option<PositionState>,

    /// 현재 기간의 돌파 레벨
    upper_breakout: Option<Decimal>,
    lower_breakout: Option<Decimal>,

    /// 현재 기간에 이미 트리거됨
    triggered_this_period: bool,

    /// 통계
    trades_count: u32,
    wins: u32,
    losses_count: u32,
    total_pnl: Decimal,

    initialized: bool,
}

impl VolatilityBreakoutStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            symbol: None,
            context: None,
            period_history: VecDeque::new(),
            current_period: None,
            prev_range: None,
            current_atr: None,
            tr_history: VecDeque::new(),
            avg_volume: None,
            volume_history: VecDeque::new(),
            position: None,
            upper_breakout: None,
            lower_breakout: None,
            triggered_this_period: false,
            trades_count: 0,
            wins: 0,
            losses_count: 0,
            total_pnl: Decimal::ZERO,
            initialized: false,
        }
    }

    /// 새 기간이 시작되었는지 확인 (날짜 또는 시간 기반).
    fn is_new_period(&self, current_time: DateTime<Utc>) -> bool {
        match &self.current_period {
            Some(period) => {
                // 먼저 날짜가 다른지 확인 (일봉 데이터 지원)
                if current_time.date_naive() != period.timestamp.date_naive() {
                    return true;
                }
                // 같은 날이면 시간이 다른지 확인 (시간봉 데이터 지원)
                current_time.hour() != period.timestamp.hour()
            }
            None => true,
        }
    }

    /// True Range 계산.
    fn calculate_true_range(
        &self,
        high: Decimal,
        low: Decimal,
        prev_close: Option<Decimal>,
    ) -> Decimal {
        let hl = high - low;

        match prev_close {
            Some(pc) => {
                let hc = (high - pc).abs();
                let lc = (low - pc).abs();
                hl.max(hc).max(lc)
            }
            None => hl,
        }
    }

    /// ATR 계산.
    fn calculate_atr(&mut self) {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return, // 초기화되지 않은 경우
        };

        if self.tr_history.len() >= config.atr_period {
            let sum: Decimal = self.tr_history.iter().take(config.atr_period).sum();
            self.current_atr = Some(sum / Decimal::from(config.atr_period));
        }
    }

    /// 평균 거래량 계산.
    fn calculate_avg_volume(&mut self) {
        if self.volume_history.len() >= 20 {
            let sum: Decimal = self.volume_history.iter().take(20).sum();
            self.avg_volume = Some(sum / dec!(20));
        }
    }

    /// 사용할 레인지 반환 (단순 레인지 또는 ATR).
    fn get_range(&self) -> Option<Decimal> {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return None, // 초기화되지 않은 경우
        };

        if config.use_atr {
            self.current_atr
        } else {
            self.prev_range
        }
    }

    /// 레인지가 허용 범위 내인지 확인.
    fn is_range_valid(&self, range: Decimal, current_price: Decimal) -> bool {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return false, // 초기화되지 않은 경우
        };

        if current_price == Decimal::ZERO {
            return false;
        }

        let range_pct = (range / current_price * dec!(100)).to_f64().unwrap_or(0.0);

        range_pct >= config.min_range_pct && range_pct <= config.max_range_pct
    }

    /// 거래량 필터 통과 여부 확인.
    fn passes_volume_filter(&self, current_volume: Decimal) -> bool {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return true, // 초기화되지 않은 경우 필터 통과
        };

        if !config.use_volume_filter {
            return true;
        }

        match self.avg_volume {
            Some(avg) if avg > Decimal::ZERO => {
                // f64 변환 실패 시 기본값 1.5 사용
                let threshold =
                    avg * Decimal::from_f64_retain(config.volume_multiplier).unwrap_or(dec!(1.5));
                current_volume >= threshold
            }
            _ => true, // 아직 평균 없음, 거래 허용
        }
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
            return true;
        };

        let Ok(ctx_lock) = ctx.try_read() else {
            return true;
        };

        // RouteState 체크
        if let Some(route_state) = ctx_lock.get_route_state(ticker) {
            match route_state {
                RouteState::Overheat | RouteState::Wait | RouteState::Neutral => {
                    return false;
                }
                RouteState::Armed | RouteState::Attack => {}
            }
        }

        // GlobalScore 체크
        if let Some(score) = ctx_lock.get_global_score(ticker) {
            if score.overall_score < config.min_global_score {
                return false;
            }
        }

        true
    }

    /// 새 기간 처리 (기간 변경 시 호출).
    fn on_period_close(&mut self) {
        if let Some(period) = self.current_period.take() {
            // borrow 충돌 방지를 위해 설정값 미리 가져오기
            let (atr_period, lookback_period) = {
                let config = match self.config.as_ref() {
                    Some(c) => c,
                    None => return, // 초기화되지 않은 경우
                };
                (config.atr_period, config.lookback_period)
            };

            // True Range 계산
            let prev_close = self.period_history.front().map(|p| p.close);
            let tr = self.calculate_true_range(period.high, period.low, prev_close);

            // True Range 저장
            self.tr_history.push_front(tr);
            while self.tr_history.len() > atr_period + 1 {
                self.tr_history.pop_back();
            }

            // 거래량 저장
            self.volume_history.push_front(period.volume);
            while self.volume_history.len() > 21 {
                self.volume_history.pop_back();
            }

            // 지표 계산
            self.calculate_atr();
            self.calculate_avg_volume();

            // 단순 레인지 저장
            self.prev_range = Some(period.high - period.low);

            // 기간 데이터 저장
            self.period_history.push_front(period);
            while self.period_history.len() > lookback_period + 1 {
                self.period_history.pop_back();
            }

            // 새 기간을 위한 트리거 플래그 리셋
            self.triggered_this_period = false;
        }
    }

    /// 현재 가격 기반으로 신호 생성.
    fn generate_signals(
        &mut self,
        current_price: Decimal,
        current_volume: Decimal,
        current_time: DateTime<Utc>,
    ) -> Vec<Signal> {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return Vec::new(), // 초기화되지 않은 경우
        };
        let symbol = match self.symbol.as_ref() {
            Some(s) => s.clone(),
            None => return Vec::new(), // 초기화되지 않은 경우
        };
        let mut signals = Vec::new();

        // 기존 포지션 처리
        if let Some(pos) = &self.position {
            // 손절 확인
            let hit_stop = match pos.side {
                Side::Buy => current_price <= pos.stop_loss,
                Side::Sell => current_price >= pos.stop_loss,
            };

            // 익절 확인
            let hit_tp = match pos.side {
                Side::Buy => current_price >= pos.take_profit,
                Side::Sell => current_price <= pos.take_profit,
            };

            if hit_stop || hit_tp {
                let exit_side = match pos.side {
                    Side::Buy => Side::Sell,
                    Side::Sell => Side::Buy,
                };

                let reason = if hit_stop { "stop_loss" } else { "take_profit" };

                signals.push(
                    Signal::exit("volatility_breakout", symbol.clone(), exit_side)
                        .with_strength(1.0)
                        .with_prices(Some(current_price), None, None)
                        .with_metadata("exit_reason", json!(reason)),
                );

                let pnl = match pos.side {
                    Side::Buy => current_price - pos.entry_price,
                    Side::Sell => pos.entry_price - current_price,
                };

                if pnl > Decimal::ZERO {
                    self.wins += 1;
                } else {
                    self.losses_count += 1;
                }
                self.total_pnl += pnl;
                self.trades_count += 1;

                info!(
                    reason = reason,
                    entry = %pos.entry_price,
                    exit = %current_price,
                    pnl = %pnl,
                    "포지션 종료"
                );

                self.position = None;
            }

            return signals;
        }

        // 포지션 없음 - 돌파 진입 확인
        if self.triggered_this_period {
            return signals;
        }

        // StrategyContext 기반 필터링 (RouteState, GlobalScore)
        if !self.can_enter() {
            debug!("can_enter() returned false - skipping entry");
            return signals;
        }

        // 레인지 가져오기 및 돌파 레벨 계산
        let range = match self.get_range() {
            Some(r) => r,
            None => return signals,
        };

        // 레인지 유효성 검증
        if !self.is_range_valid(range, current_price) {
            debug!(range = %range, price = %current_price, "레인지가 허용 범위 밖");
            return signals;
        }

        // 거래량 필터 확인
        if !self.passes_volume_filter(current_volume) {
            debug!(volume = %current_volume, "거래량이 임계값 미만");
            return signals;
        }

        // 현재 기간의 시가 가져오기
        let period_open = match &self.current_period {
            Some(p) => p.open,
            None => return signals,
        };

        // 돌파 레벨 계산
        let k = Decimal::from_f64_retain(config.k_factor).unwrap_or(dec!(0.5));
        let upper_breakout = period_open + range * k;
        let lower_breakout = period_open - range * k;

        self.upper_breakout = Some(upper_breakout);
        self.lower_breakout = Some(lower_breakout);

        // 롱 돌파
        if current_price >= upper_breakout {
            // f64 변환 실패 시 기본값 사용 (손절: 1.0, 익절: 2.0)
            let stop_mult =
                Decimal::from_f64_retain(config.stop_loss_multiplier).unwrap_or(dec!(1.0));
            let tp_mult =
                Decimal::from_f64_retain(config.take_profit_multiplier).unwrap_or(dec!(2.0));

            let stop_loss = current_price - range * stop_mult;
            let take_profit = current_price + range * tp_mult;

            signals.push(
                Signal::entry("volatility_breakout", symbol.clone(), Side::Buy)
                    .with_strength(0.5)
                    .with_prices(Some(current_price), Some(stop_loss), Some(take_profit))
                    .with_metadata("breakout_level", json!(upper_breakout.to_string()))
                    .with_metadata("range", json!(range.to_string())),
            );

            self.position = Some(PositionState {
                side: Side::Buy,
                entry_price: current_price,
                stop_loss,
                take_profit,
                entry_time: current_time,
            });

            self.triggered_this_period = true;

            info!(
                price = %current_price,
                breakout = %upper_breakout,
                range = %range,
                "롱 돌파 신호"
            );
        }

        // 숏 돌파
        if config.trade_both_directions
            && current_price <= lower_breakout
            && self.position.is_none()
        {
            // f64 변환 실패 시 기본값 사용 (손절: 1.0, 익절: 2.0)
            let stop_mult =
                Decimal::from_f64_retain(config.stop_loss_multiplier).unwrap_or(dec!(1.0));
            let tp_mult =
                Decimal::from_f64_retain(config.take_profit_multiplier).unwrap_or(dec!(2.0));

            let stop_loss = current_price + range * stop_mult;
            let take_profit = current_price - range * tp_mult;

            signals.push(
                Signal::entry("volatility_breakout", symbol.clone(), Side::Sell)
                    .with_strength(0.5)
                    .with_prices(Some(current_price), Some(stop_loss), Some(take_profit))
                    .with_metadata("breakout_level", json!(lower_breakout.to_string()))
                    .with_metadata("range", json!(range.to_string())),
            );

            self.position = Some(PositionState {
                side: Side::Sell,
                entry_price: current_price,
                stop_loss,
                take_profit,
                entry_time: current_time,
            });

            self.triggered_this_period = true;

            info!(
                price = %current_price,
                breakout = %lower_breakout,
                range = %range,
                "숏 돌파 신호"
            );
        }

        signals
    }
}

impl Default for VolatilityBreakoutStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for VolatilityBreakoutStrategy {
    fn name(&self) -> &str {
        "Volatility Breakout"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "래리 윌리엄스 변동성 돌파 전략. 가격이 변동성 범위를 돌파할 때 \
         진입 (시가 + 레인지 × K). 추세장에 최적화."
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let vb_config: VolatilityBreakoutConfig = serde_json::from_value(config)?;

        info!(
            symbol = %vb_config.symbol,
            k_factor = vb_config.k_factor,
            use_atr = vb_config.use_atr,
            "변동성 돌파 전략 초기화"
        );

        self.symbol = Symbol::from_string(&vb_config.symbol, MarketType::Crypto);
        self.config = Some(vb_config);
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

        let symbol_str = match self.config.as_ref() {
            Some(c) => c.symbol.clone(),
            None => return Ok(vec![]), // 초기화되지 않은 경우
        };

        if data.symbol.to_string() != symbol_str {
            return Ok(vec![]);
        }

        // kline에서 OHLCV 추출
        let (open, high, low, close, volume, timestamp) = match &data.data {
            MarketDataType::Kline(kline) => (
                kline.open,
                kline.high,
                kline.low,
                kline.close,
                kline.volume,
                kline.open_time,
            ),
            _ => return Ok(vec![]),
        };

        // 새 기간 확인
        if self.is_new_period(timestamp) {
            self.on_period_close();

            // 새 기간 시작
            self.current_period = Some(PeriodData {
                open,
                high,
                low,
                close,
                volume,
                timestamp,
            });
        } else {
            // 현재 기간 업데이트
            if let Some(period) = &mut self.current_period {
                period.high = period.high.max(high);
                period.low = period.low.min(low);
                period.close = close;
                period.volume += volume;
            }
        }

        // 신호 생성
        let signals = self.generate_signals(close, volume, timestamp);

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!(
            side = ?order.side,
            quantity = %order.quantity,
            "주문 체결"
        );
        Ok(())
    }

    async fn on_position_update(
        &mut self,
        _position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let win_rate = if self.trades_count > 0 {
            (self.wins as f64 / self.trades_count as f64) * 100.0
        } else {
            0.0
        };

        info!(
            trades = self.trades_count,
            wins = self.wins,
            losses = self.losses_count,
            win_rate = %format!("{:.1}%", win_rate),
            total_pnl = %self.total_pnl,
            "변동성 돌파 전략 종료"
        );

        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "initialized": self.initialized,
            "has_position": self.position.is_some(),
            "position_side": self.position.as_ref().map(|p| format!("{:?}", p.side)),
            "current_range": self.get_range().map(|v| v.to_string()),
            "current_atr": self.current_atr.map(|v| v.to_string()),
            "upper_breakout": self.upper_breakout.map(|v| v.to_string()),
            "lower_breakout": self.lower_breakout.map(|v| v.to_string()),
            "triggered_this_period": self.triggered_this_period,
            "trades_count": self.trades_count,
            "wins": self.wins,
            "losses": self.losses_count,
            "total_pnl": self.total_pnl.to_string(),
        })
    }
    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("StrategyContext injected into VolatilityBreakout strategy");
    }


}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use trader_core::{Kline, Timeframe};

    fn create_kline_at_time(
        symbol: &Symbol,
        open: Decimal,
        high: Decimal,
        low: Decimal,
        close: Decimal,
        hour: u32,
    ) -> MarketData {
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, hour, 0, 0).unwrap();
        let kline = Kline::new(
            symbol.clone(),
            Timeframe::H1,
            timestamp,
            open,
            high,
            low,
            close,
            dec!(1000),
            timestamp,
        );
        MarketData::from_kline("test", kline)
    }

    #[tokio::test]
    async fn test_volatility_breakout_initialization() {
        let mut strategy = VolatilityBreakoutStrategy::new();

        let config = json!({
            "symbol": "BTC/USDT",
            "k_factor": 0.5,
            "use_atr": false
        });

        strategy.initialize(config).await.unwrap();
        assert!(strategy.initialized);
    }

    #[tokio::test]
    async fn test_breakout_signal_generation() {
        let mut strategy = VolatilityBreakoutStrategy::new();

        let config = json!({
            "symbol": "BTC/USDT",
            "k_factor": 0.5,
            "min_range_pct": 0.1,
            "max_range_pct": 20.0
        });

        strategy.initialize(config).await.unwrap();

        let symbol = Symbol::crypto("BTC", "USDT");

        // First period: establish range (high-low = 1000)
        let data1 = create_kline_at_time(
            &symbol,
            dec!(50000),
            dec!(50500),
            dec!(49500),
            dec!(50200),
            0,
        );
        strategy.on_market_data(&data1).await.unwrap();

        // New period starts
        let data2 = create_kline_at_time(
            &symbol,
            dec!(50200),
            dec!(50200),
            dec!(50200),
            dec!(50200),
            1,
        );
        strategy.on_market_data(&data2).await.unwrap();

        // 레인지는 1000, K=0.5, 따라서 상단 돌파 = 50200 + 500 = 50700
        assert!(strategy.prev_range.is_some());

        // 가격이 상방 돌파
        let data3 = create_kline_at_time(
            &symbol,
            dec!(50200),
            dec!(51000),
            dec!(50200),
            dec!(50800),
            1,
        );
        let signals = strategy.on_market_data(&data3).await.unwrap();

        // 롱 신호가 생성되거나 포지션이 있어야 함
        let has_signal = !signals.is_empty() || strategy.position.is_some();
        assert!(has_signal);
    }

    #[test]
    fn test_true_range_calculation() {
        let strategy = VolatilityBreakoutStrategy::new();

        // 단순 케이스: 이전 종가 없음
        let tr1 = strategy.calculate_true_range(dec!(105), dec!(95), None);
        assert_eq!(tr1, dec!(10));

        // 갭상승
        let tr2 = strategy.calculate_true_range(dec!(115), dec!(105), Some(dec!(100)));
        assert_eq!(tr2, dec!(15)); // max(10, 15, 5) = 15

        // 갭하락
        let tr3 = strategy.calculate_true_range(dec!(95), dec!(85), Some(dec!(100)));
        assert_eq!(tr3, dec!(15)); // max(10, 5, 15) = 15
    }
}

// 전략 레지스트리에 자동 등록
use crate::register_strategy;

register_strategy! {
    id: "volatility_breakout",
    aliases: ["volatility"],
    name: "변동성 돌파",
    description: "전일 변동폭을 돌파할 때 진입하는 전략입니다.",
    timeframe: "1d",
    symbols: [],
    category: Daily,
    markets: [Crypto, Stock, Stock],
    type: VolatilityBreakoutStrategy
}
