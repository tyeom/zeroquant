//! 코스피 양방향 매매 전략 (KOSPI BothSide)
//!
//! 코스피 레버리지/인버스 ETF를 활용한 양방향 투자 전략.
//! 이동평균선과 이격도, RSI를 조합하여 추세 판단.
//!
//! # 전략 로직
//! - **기본 배분**: 레버리지 70% + 인버스 30%
//! - **추세 판단**: MA3, MA6, MA19, MA60 조합
//! - **진입 조건**:
//!   - 레버리지 매수: MA60 상향 돌파, 이격도 적정
//!   - 인버스 매수: MA3 < MA6 < MA19, RSI 과매도
//! - **청산 조건**: 반대 신호 또는 손절
//!
//! # 대상 ETF
//! - **레버리지**: 122630 (KODEX 레버리지)
//! - **인버스**: 252670 (KODEX 200선물인버스2X)
//!
//! # 권장 타임프레임
//! - 일봉 (1D)

use crate::Strategy;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use trader_core::domain::{RouteState, StrategyContext};
use trader_core::{MarketData, MarketDataType, Order, Position, Side, Signal, Symbol};

/// 코스피 양방향 전략 설정.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KospiBothSideConfig {
    /// 레버리지 ETF 티커 (기본값: 122630)
    #[serde(default = "default_leverage_ticker")]
    pub leverage_ticker: String,

    /// 인버스 ETF 티커 (기본값: 252670)
    #[serde(default = "default_inverse_ticker")]
    pub inverse_ticker: String,

    /// 레버리지 목표 비율 (기본값: 0.7)
    #[serde(default = "default_leverage_ratio")]
    pub leverage_ratio: f64,

    /// 인버스 목표 비율 (기본값: 0.3)
    #[serde(default = "default_inverse_ratio")]
    pub inverse_ratio: f64,

    /// MA3 기간 (기본값: 3)
    #[serde(default = "default_ma3")]
    pub ma3_period: usize,

    /// MA6 기간 (기본값: 6)
    #[serde(default = "default_ma6")]
    pub ma6_period: usize,

    /// MA19 기간 (기본값: 19)
    #[serde(default = "default_ma19")]
    pub ma19_period: usize,

    /// MA60 기간 (기본값: 60)
    #[serde(default = "default_ma60")]
    pub ma60_period: usize,

    /// 이격도 상한 (기본값: 106%)
    #[serde(default = "default_disparity_upper")]
    pub disparity_upper: f64,

    /// 이격도 하한 (기본값: 94%)
    #[serde(default = "default_disparity_lower")]
    pub disparity_lower: f64,

    /// RSI 기간 (기본값: 14)
    #[serde(default = "default_rsi_period")]
    pub rsi_period: usize,

    /// RSI 과매도 (기본값: 30)
    #[serde(default = "default_rsi_oversold")]
    pub rsi_oversold: f64,

    /// RSI 과매수 (기본값: 70)
    #[serde(default = "default_rsi_overbought")]
    pub rsi_overbought: f64,

    /// 손절 비율 (기본값: 5%)
    #[serde(default = "default_stop_loss")]
    pub stop_loss_pct: f64,

    /// 최소 글로벌 스코어 (기본값: 60)
    #[serde(default = "default_min_global_score")]
    pub min_global_score: Decimal,
}

fn default_leverage_ticker() -> String {
    "122630".to_string()
}
fn default_inverse_ticker() -> String {
    "252670".to_string()
}
fn default_leverage_ratio() -> f64 {
    0.7
}
fn default_inverse_ratio() -> f64 {
    0.3
}
fn default_ma3() -> usize {
    3
}
fn default_ma6() -> usize {
    6
}
fn default_ma19() -> usize {
    19
}
fn default_ma60() -> usize {
    60
}
fn default_disparity_upper() -> f64 {
    106.0
}
fn default_disparity_lower() -> f64 {
    94.0
}
fn default_rsi_period() -> usize {
    14
}
fn default_rsi_oversold() -> f64 {
    30.0
}
fn default_rsi_overbought() -> f64 {
    70.0
}
fn default_stop_loss() -> f64 {
    5.0
}

fn default_min_global_score() -> Decimal {
    dec!(60)
}

impl Default for KospiBothSideConfig {
    fn default() -> Self {
        Self {
            leverage_ticker: "122630".to_string(),
            inverse_ticker: "252670".to_string(),
            leverage_ratio: 0.7,
            inverse_ratio: 0.3,
            ma3_period: 3,
            ma6_period: 6,
            ma19_period: 19,
            ma60_period: 60,
            disparity_upper: 106.0,
            disparity_lower: 94.0,
            rsi_period: 14,
            rsi_oversold: 30.0,
            rsi_overbought: 70.0,
            stop_loss_pct: 5.0,
            min_global_score: dec!(60),
        }
    }
}

/// ETF 포지션 상태.
#[derive(Debug, Clone)]
struct EtfPosition {
    ticker: String,
    holdings: Decimal,
    entry_price: Decimal,
    current_price: Decimal,
}

/// 기술적 지표 계산기.
#[derive(Debug, Clone)]
struct TechnicalIndicators {
    prices: VecDeque<Decimal>,
    gains: VecDeque<Decimal>,
    losses: VecDeque<Decimal>,
    max_len: usize,
}

impl TechnicalIndicators {
    fn new(max_len: usize) -> Self {
        Self {
            prices: VecDeque::new(),
            gains: VecDeque::new(),
            losses: VecDeque::new(),
            max_len,
        }
    }

    fn update(&mut self, price: Decimal) {
        if let Some(&prev) = self.prices.front() {
            let change = price - prev;
            if change > Decimal::ZERO {
                self.gains.push_front(change);
                self.losses.push_front(Decimal::ZERO);
            } else {
                self.gains.push_front(Decimal::ZERO);
                self.losses.push_front(change.abs());
            }

            while self.gains.len() > self.max_len {
                self.gains.pop_back();
            }
            while self.losses.len() > self.max_len {
                self.losses.pop_back();
            }
        }

        self.prices.push_front(price);
        while self.prices.len() > self.max_len {
            self.prices.pop_back();
        }
    }

    fn calculate_ma(&self, period: usize) -> Option<Decimal> {
        if self.prices.len() < period {
            return None;
        }

        let sum: Decimal = self.prices.iter().take(period).sum();
        Some(sum / Decimal::from(period))
    }

    fn calculate_rsi(&self, period: usize) -> Option<Decimal> {
        if self.gains.len() < period {
            return None;
        }

        let avg_gain: Decimal =
            self.gains.iter().take(period).sum::<Decimal>() / Decimal::from(period);
        let avg_loss: Decimal =
            self.losses.iter().take(period).sum::<Decimal>() / Decimal::from(period);

        if avg_loss == Decimal::ZERO {
            return Some(dec!(100));
        }

        let rs = avg_gain / avg_loss;
        Some(dec!(100) - (dec!(100) / (dec!(1) + rs)))
    }

    fn calculate_disparity(&self, period: usize) -> Option<Decimal> {
        let ma = self.calculate_ma(period)?;
        let current = self.prices.front()?;

        if ma == Decimal::ZERO {
            return None;
        }

        Some(*current / ma * dec!(100))
    }
}

/// 코스피 양방향 전략.
pub struct KospiBothSideStrategy {
    config: Option<KospiBothSideConfig>,
    leverage_symbol: Option<Symbol>,
    inverse_symbol: Option<Symbol>,
    context: Option<Arc<RwLock<StrategyContext>>>,

    /// 레버리지 포지션
    leverage_position: Option<EtfPosition>,

    /// 인버스 포지션
    inverse_position: Option<EtfPosition>,

    /// 레버리지 기술적 지표
    leverage_indicators: TechnicalIndicators,

    /// 인버스 기술적 지표
    inverse_indicators: TechnicalIndicators,

    /// 현재 날짜
    current_date: Option<chrono::NaiveDate>,

    /// 초기화 완료
    started: bool,

    /// 통계
    trades_count: u32,
    wins: u32,
    total_pnl: Decimal,

    initialized: bool,
}

impl KospiBothSideStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            leverage_symbol: None,
            inverse_symbol: None,
            context: None,
            leverage_position: None,
            inverse_position: None,
            leverage_indicators: TechnicalIndicators::new(70),
            inverse_indicators: TechnicalIndicators::new(70),
            current_date: None,
            started: false,
            trades_count: 0,
            wins: 0,
            total_pnl: Decimal::ZERO,
            initialized: false,
        }
    }

    /// 새로운 날인지 확인.
    fn is_new_day(&self, current_time: DateTime<Utc>) -> bool {
        match self.current_date {
            Some(date) => current_time.date_naive() != date,
            None => true,
        }
    }

    /// RouteState와 GlobalScore 기반 진입 조건 체크
    fn can_enter(&self) -> bool {
        let context = match &self.context {
            Some(ctx) => ctx,
            None => return true,
        };

        let config = match &self.config {
            Some(cfg) => cfg,
            None => return true,
        };

        let ctx = match context.try_read() {
            Ok(ctx) => ctx,
            Err(_) => return true,
        };

        // RouteState 체크 (레버리지 심볼 기준)
        if let Some(symbol) = &self.leverage_symbol {
            if let Some(route) = ctx.get_route_state(&symbol.base) {
                match route {
                    RouteState::Wait | RouteState::Overheat => {
                        debug!("[KospiBothSide] RouteState가 {:?}이므로 진입 불가", route);
                        return false;
                    }
                    _ => {}
                }
            }
        }

        // GlobalScore 체크 (레버리지 심볼 기준)
        if let Some(symbol) = &self.leverage_symbol {
            if let Some(score) = ctx.get_global_score(&symbol.base) {
                if score.overall_score < config.min_global_score {
                    debug!(
                        "[KospiBothSide] GlobalScore {} < {} 기준 미달",
                        score.overall_score, config.min_global_score
                    );
                    return false;
                }
            }
        }

        true
    }

    /// 레버리지 매수 조건 확인.
    fn should_buy_leverage(&self) -> bool {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return false,
        };

        // MA60 상향 돌파 체크
        let ma60 = match self.leverage_indicators.calculate_ma(config.ma60_period) {
            Some(v) => v,
            None => return false,
        };
        let ma60_prev = match self
            .leverage_indicators
            .calculate_ma(config.ma60_period + 1)
        {
            Some(v) => v,
            None => return false,
        };

        let current = match self.leverage_indicators.prices.front() {
            Some(&v) => v,
            None => return false,
        };
        let prev_close = match self.leverage_indicators.prices.get(1) {
            Some(&v) => v,
            None => return false,
        };

        // MA60 상향 돌파 (이전 MA60 < 이전 종가, 현재 MA60 <= 현재가)
        let ma60_breakout = ma60_prev > prev_close && ma60 <= current;

        // 이격도 체크 (11일)
        let disparity11 = match self.leverage_indicators.calculate_disparity(11) {
            Some(v) => v.to_f64().unwrap_or(100.0),
            None => return false,
        };

        // 이격도가 상한 미만일 때만 매수
        let disparity_ok = disparity11 < config.disparity_upper;

        // RSI 체크
        let rsi = match self.leverage_indicators.calculate_rsi(config.rsi_period) {
            Some(v) => v.to_f64().unwrap_or(50.0),
            None => return false,
        };
        let rsi_ok = rsi < config.rsi_overbought;

        debug!(
            ma60_breakout = ma60_breakout,
            disparity = %format!("{:.1}", disparity11),
            rsi = %format!("{:.1}", rsi),
            "레버리지 매수 조건 체크"
        );

        ma60_breakout && disparity_ok && rsi_ok
    }

    /// 레버리지 매도 조건 확인.
    fn should_sell_leverage(&self) -> bool {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return false,
        };

        // 포지션이 없으면 매도 불가
        let pos = match &self.leverage_position {
            Some(p) => p,
            None => return false,
        };

        // 손절 체크
        if pos.entry_price > Decimal::ZERO {
            let pnl_pct = ((pos.current_price - pos.entry_price) / pos.entry_price * dec!(100))
                .to_f64()
                .unwrap_or(0.0);

            if pnl_pct <= -config.stop_loss_pct {
                warn!(pnl = %format!("{:.1}%", pnl_pct), "레버리지 손절");
                return true;
            }
        }

        // MA 데드 크로스 체크 (MA3 < MA6 < MA19)
        let ma3 = match self.leverage_indicators.calculate_ma(config.ma3_period) {
            Some(v) => v,
            None => return false,
        };
        let ma6 = match self.leverage_indicators.calculate_ma(config.ma6_period) {
            Some(v) => v,
            None => return false,
        };
        let ma19 = match self.leverage_indicators.calculate_ma(config.ma19_period) {
            Some(v) => v,
            None => return false,
        };

        let dead_cross = ma3 < ma6 && ma6 < ma19;

        // 이격도 하한 체크
        let disparity20 = match self.leverage_indicators.calculate_disparity(20) {
            Some(v) => v.to_f64().unwrap_or(100.0),
            None => return false,
        };
        let disparity_sell = disparity20 < config.disparity_lower;

        debug!(
            dead_cross = dead_cross,
            disparity = %format!("{:.1}", disparity20),
            "레버리지 매도 조건 체크"
        );

        dead_cross || disparity_sell
    }

    /// 인버스 매수 조건 확인.
    fn should_buy_inverse(&self) -> bool {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return false,
        };

        // MA 데드 크로스 (MA3 < MA6 < MA19)
        let ma3 = match self.leverage_indicators.calculate_ma(config.ma3_period) {
            Some(v) => v,
            None => return false,
        };
        let ma6 = match self.leverage_indicators.calculate_ma(config.ma6_period) {
            Some(v) => v,
            None => return false,
        };
        let ma19 = match self.leverage_indicators.calculate_ma(config.ma19_period) {
            Some(v) => v,
            None => return false,
        };

        let dead_cross = ma3 < ma6 && ma6 < ma19;

        // RSI 과매도
        let rsi = match self.leverage_indicators.calculate_rsi(config.rsi_period) {
            Some(v) => v.to_f64().unwrap_or(50.0),
            None => return false,
        };

        // 레버리지 RSI가 과매수이거나 인버스 추세일 때
        let inverse_signal = rsi > config.rsi_overbought || dead_cross;

        debug!(
            dead_cross = dead_cross,
            rsi = %format!("{:.1}", rsi),
            "인버스 매수 조건 체크"
        );

        inverse_signal
    }

    /// 인버스 매도 조건 확인.
    fn should_sell_inverse(&self) -> bool {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return false,
        };

        // 포지션이 없으면 매도 불가
        let pos = match &self.inverse_position {
            Some(p) => p,
            None => return false,
        };

        // 손절 체크
        if pos.entry_price > Decimal::ZERO {
            let pnl_pct = ((pos.current_price - pos.entry_price) / pos.entry_price * dec!(100))
                .to_f64()
                .unwrap_or(0.0);

            if pnl_pct <= -config.stop_loss_pct {
                warn!(pnl = %format!("{:.1}%", pnl_pct), "인버스 손절");
                return true;
            }
        }

        // MA 골든 크로스 (MA3 > MA6 > MA19)
        let ma3 = match self.leverage_indicators.calculate_ma(config.ma3_period) {
            Some(v) => v,
            None => return false,
        };
        let ma6 = match self.leverage_indicators.calculate_ma(config.ma6_period) {
            Some(v) => v,
            None => return false,
        };
        let ma19 = match self.leverage_indicators.calculate_ma(config.ma19_period) {
            Some(v) => v,
            None => return false,
        };

        let golden_cross = ma3 > ma6 && ma6 > ma19;

        // RSI 과매도 회복
        let rsi = match self.leverage_indicators.calculate_rsi(config.rsi_period) {
            Some(v) => v.to_f64().unwrap_or(50.0),
            None => return false,
        };
        let rsi_recover = rsi < config.rsi_oversold;

        golden_cross || rsi_recover
    }

    /// 신호 생성.
    fn generate_signals(&mut self) -> Vec<Signal> {
        let config = match self.config.as_ref() {
            Some(c) => c.clone(),
            None => return Vec::new(),
        };

        let mut signals = Vec::new();

        // 레버리지 신호
        if let Some(sym) = &self.leverage_symbol {
            let price = self
                .leverage_indicators
                .prices
                .front()
                .copied()
                .unwrap_or(Decimal::ZERO);

            // 가격이 0인 경우 신호 생성 방지 (데이터 없음 - Division by zero 방지)
            if price == Decimal::ZERO {
                warn!("레버리지 ETF 가격 데이터 없음, 신호 생성 건너뜀");
            } else if self.leverage_position.is_none() && self.should_buy_leverage() {
                // can_enter() 체크: RouteState, GlobalScore 기반 진입 제한
                if !self.can_enter() {
                    debug!("[KospiBothSide] can_enter() 실패 - 레버리지 매수 신호 스킵");
                } else {
                    signals.push(
                        Signal::entry("kospi_bothside", sym.clone(), Side::Buy)
                            .with_strength(config.leverage_ratio)
                            .with_prices(Some(price), None, None)
                            .with_metadata("etf_type", json!("leverage"))
                            .with_metadata("action", json!("buy_leverage")),
                    );
                    info!(price = %price, "레버리지 매수 신호");
                }
            } else if self.leverage_position.is_some() && self.should_sell_leverage() {
                signals.push(
                    Signal::exit("kospi_bothside", sym.clone(), Side::Sell)
                        .with_strength(1.0)
                        .with_prices(Some(price), None, None)
                        .with_metadata("etf_type", json!("leverage"))
                        .with_metadata("action", json!("sell_leverage")),
                );
                info!(price = %price, "레버리지 매도 신호");
            }
        }

        // 인버스 신호
        if let Some(sym) = &self.inverse_symbol {
            let price = self
                .inverse_indicators
                .prices
                .front()
                .copied()
                .unwrap_or(Decimal::ZERO);

            // 가격이 0인 경우 신호 생성 방지 (데이터 없음 - Division by zero 방지)
            if price == Decimal::ZERO {
                warn!("인버스 ETF 가격 데이터 없음, 신호 생성 건너뜀");
            } else if self.inverse_position.is_none() && self.should_buy_inverse() {
                // can_enter() 체크: RouteState, GlobalScore 기반 진입 제한
                if !self.can_enter() {
                    debug!("[KospiBothSide] can_enter() 실패 - 인버스 매수 신호 스킵");
                } else {
                    signals.push(
                        Signal::entry("kospi_bothside", sym.clone(), Side::Buy)
                            .with_strength(config.inverse_ratio)
                            .with_prices(Some(price), None, None)
                            .with_metadata("etf_type", json!("inverse"))
                            .with_metadata("action", json!("buy_inverse")),
                    );
                    info!(price = %price, "인버스 매수 신호");
                }
            } else if self.inverse_position.is_some() && self.should_sell_inverse() {
                signals.push(
                    Signal::exit("kospi_bothside", sym.clone(), Side::Sell)
                        .with_strength(1.0)
                        .with_prices(Some(price), None, None)
                        .with_metadata("etf_type", json!("inverse"))
                        .with_metadata("action", json!("sell_inverse")),
                );
                info!(price = %price, "인버스 매도 신호");
            }
        }

        signals
    }
}

impl Default for KospiBothSideStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for KospiBothSideStrategy {
    fn name(&self) -> &str {
        "KOSPI BothSide"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "코스피 양방향 매매 전략. KODEX 레버리지(122630)와 인버스2X(252670)를 \
         조합하여 상승/하락장 모두 수익 추구. MA, RSI, 이격도 조합 신호."
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let kb_config: KospiBothSideConfig = serde_json::from_value(config)?;

        info!(
            leverage = %kb_config.leverage_ticker,
            inverse = %kb_config.inverse_ticker,
            leverage_ratio = %format!("{:.0}%", kb_config.leverage_ratio * 100.0),
            inverse_ratio = %format!("{:.0}%", kb_config.inverse_ratio * 100.0),
            "코스피 양방향 전략 초기화"
        );

        self.leverage_symbol = Some(Symbol::stock(&kb_config.leverage_ticker, "KRW"));
        self.inverse_symbol = Some(Symbol::stock(&kb_config.inverse_ticker, "KRW"));
        self.config = Some(kb_config);
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

        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return Ok(vec![]),
        };

        // base 심볼만 추출 (122630/KRW -> 122630)
        let symbol_str = data.symbol.base.clone();
        let is_leverage = symbol_str == config.leverage_ticker;
        let is_inverse = symbol_str == config.inverse_ticker;

        if !is_leverage && !is_inverse {
            return Ok(vec![]);
        }

        // kline에서 데이터 추출
        let (close, timestamp) = match &data.data {
            MarketDataType::Kline(kline) => (kline.close, kline.open_time),
            _ => return Ok(vec![]),
        };

        // 새 날짜 확인
        if self.is_new_day(timestamp) {
            self.current_date = Some(timestamp.date_naive());
        }

        // 지표 업데이트
        if is_leverage {
            self.leverage_indicators.update(close);
            if let Some(pos) = &mut self.leverage_position {
                pos.current_price = close;
            }
        } else {
            self.inverse_indicators.update(close);
            if let Some(pos) = &mut self.inverse_position {
                pos.current_price = close;
            }
        }

        // 충분한 데이터가 있는지 확인
        if self.leverage_indicators.prices.len() < 60 {
            return Ok(vec![]);
        }

        self.started = true;

        // 신호 생성
        let signals = self.generate_signals();

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return Ok(()),
        };

        // Symbol.base와 ticker 비교 (Symbol.to_string()은 "base/quote" 형식이므로 base만 비교)
        let ticker = order.symbol.base.clone();
        let price = order.price.unwrap_or(Decimal::ZERO);

        if ticker == config.leverage_ticker {
            match order.side {
                Side::Buy => {
                    self.leverage_position = Some(EtfPosition {
                        ticker: ticker.clone(),
                        holdings: order.quantity,
                        entry_price: price,
                        current_price: price,
                    });
                }
                Side::Sell => {
                    if let Some(pos) = &self.leverage_position {
                        let pnl = order.quantity * (price - pos.entry_price);
                        self.total_pnl += pnl;
                        if pnl > Decimal::ZERO {
                            self.wins += 1;
                        }
                        self.trades_count += 1;
                    }
                    self.leverage_position = None;
                }
            }
        } else if ticker == config.inverse_ticker {
            match order.side {
                Side::Buy => {
                    self.inverse_position = Some(EtfPosition {
                        ticker: ticker.clone(),
                        holdings: order.quantity,
                        entry_price: price,
                        current_price: price,
                    });
                }
                Side::Sell => {
                    if let Some(pos) = &self.inverse_position {
                        let pnl = order.quantity * (price - pos.entry_price);
                        self.total_pnl += pnl;
                        if pnl > Decimal::ZERO {
                            self.wins += 1;
                        }
                        self.trades_count += 1;
                    }
                    self.inverse_position = None;
                }
            }
        }

        debug!(
            ticker = %ticker,
            side = ?order.side,
            quantity = %order.quantity,
            price = %price,
            "주문 체결"
        );
        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return Ok(()),
        };

        // Symbol.base와 ticker 비교 (Symbol.to_string()은 "base/quote" 형식이므로 base만 비교)
        let ticker = position.symbol.base.clone();

        if ticker == config.leverage_ticker {
            if position.quantity > Decimal::ZERO {
                self.leverage_position = Some(EtfPosition {
                    ticker,
                    holdings: position.quantity,
                    entry_price: position.entry_price,
                    current_price: position.entry_price,
                });
            } else {
                self.leverage_position = None;
            }
        } else if ticker == config.inverse_ticker {
            if position.quantity > Decimal::ZERO {
                self.inverse_position = Some(EtfPosition {
                    ticker,
                    holdings: position.quantity,
                    entry_price: position.entry_price,
                    current_price: position.entry_price,
                });
            } else {
                self.inverse_position = None;
            }
        }

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
            win_rate = %format!("{:.1}%", win_rate),
            total_pnl = %self.total_pnl,
            "코스피 양방향 전략 종료"
        );

        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "initialized": self.initialized,
            "started": self.started,
            "leverage_position": self.leverage_position.as_ref().map(|p| json!({
                "holdings": p.holdings.to_string(),
                "entry_price": p.entry_price.to_string(),
                "current_price": p.current_price.to_string(),
            })),
            "inverse_position": self.inverse_position.as_ref().map(|p| json!({
                "holdings": p.holdings.to_string(),
                "entry_price": p.entry_price.to_string(),
                "current_price": p.current_price.to_string(),
            })),
            "trades_count": self.trades_count,
            "wins": self.wins,
            "total_pnl": self.total_pnl.to_string(),
        })
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("StrategyContext injected into KospiBothSide strategy");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_kospi_bothside_initialization() {
        let mut strategy = KospiBothSideStrategy::new();

        let config = json!({
            "leverage_ticker": "122630",
            "inverse_ticker": "252670",
            "leverage_ratio": 0.7,
            "inverse_ratio": 0.3
        });

        strategy.initialize(config).await.unwrap();
        assert!(strategy.initialized);
        assert!(strategy.leverage_symbol.is_some());
        assert!(strategy.inverse_symbol.is_some());
    }

    #[test]
    fn test_technical_indicators() {
        let mut indicators = TechnicalIndicators::new(20);

        // 데이터 추가
        for i in 1..=20 {
            indicators.update(Decimal::from(100 + i));
        }

        // MA 계산 확인
        let ma5 = indicators.calculate_ma(5);
        assert!(ma5.is_some());

        // RSI 계산 확인 (상승 추세이므로 높은 값)
        let rsi = indicators.calculate_rsi(14);
        assert!(rsi.is_some());
        assert!(rsi.unwrap() > dec!(50));
    }
}

// 전략 레지스트리에 자동 등록
use crate::register_strategy;

register_strategy! {
    id: "kospi_bothside",
    aliases: ["kospi_both"],
    name: "코스피 양방향",
    description: "코스피 지수 양방향 매매 전략입니다.",
    timeframe: "15m",
    symbols: ["122630", "252670"],
    category: Intraday,
    markets: [Stock],
    type: KospiBothSideStrategy
}
