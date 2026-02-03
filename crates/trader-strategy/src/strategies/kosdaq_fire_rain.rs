//! 코스닥 피레인 전략 (KOSDAQ Fire Rain)
//!
//! 코스피/코스닥 레버리지와 인버스 ETF를 조합한 복합 양방향 전략.
//! OBV(On-Balance Volume)와 이동평균선을 활용한 추세 판단.
//!
//! # 전략 로직
//! - **대상 ETF**: 코스피 레버리지, 코스닥 레버리지, 코스피 인버스, 코스닥 인버스
//! - **진입 조건**:
//!   - 레버리지: OBV 상승 + MA 정배열 + RSI 조건
//!   - 인버스: OBV 하락 + MA 역배열 + RSI 조건
//! - **청산**: 반대 신호 발생 시 또는 손절/익절
//! - **포지션 분산**: 최대 4개 ETF 동시 보유
//!
//! # 대상 ETF
//! - **코스피 레버리지**: 122630 (KODEX 레버리지)
//! - **코스닥 레버리지**: 233740 (KODEX 코스닥150레버리지)
//! - **코스피 인버스**: 252670 (KODEX 200선물인버스2X)
//! - **코스닥 인버스**: 251340 (KODEX 코스닥150선물인버스)
//!
//! # 권장 타임프레임
//! - 일봉 (1D)

use crate::strategies::common::deserialize_symbols;
use crate::Strategy;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use trader_core::domain::{RouteState, StrategyContext};
use trader_core::{MarketData, MarketDataType, Order, Position, Side, Signal, Symbol};

/// 코스닥 피레인 전략 설정.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KosdaqFireRainConfig {
    /// 거래 대상 ETF 리스트
    #[serde(default = "default_etf_list", deserialize_with = "deserialize_symbols")]
    pub symbols: Vec<String>,

    /// 코스피 레버리지 티커
    #[serde(default = "default_kospi_leverage")]
    pub kospi_leverage: String,

    /// 코스닥 레버리지 티커
    #[serde(default = "default_kosdaq_leverage")]
    pub kosdaq_leverage: String,

    /// 코스피 인버스 티커
    #[serde(default = "default_kospi_inverse")]
    pub kospi_inverse: String,

    /// 코스닥 인버스 티커
    #[serde(default = "default_kosdaq_inverse")]
    pub kosdaq_inverse: String,

    /// 최대 동시 투자 종목 수 (기본값: 2)
    #[serde(default = "default_max_positions")]
    pub max_positions: usize,

    /// 종목당 투자 비율 (기본값: 0.5)
    #[serde(default = "default_position_ratio")]
    pub position_ratio: f64,

    /// OBV 기간 (기본값: 10)
    #[serde(default = "default_obv_period")]
    pub obv_period: usize,

    /// MA 단기 (기본값: 5)
    #[serde(default = "default_ma_short")]
    pub ma_short: usize,

    /// MA 중기 (기본값: 20)
    #[serde(default = "default_ma_medium")]
    pub ma_medium: usize,

    /// MA 장기 (기본값: 60)
    #[serde(default = "default_ma_long")]
    pub ma_long: usize,

    /// RSI 기간 (기본값: 14)
    #[serde(default = "default_rsi_period")]
    pub rsi_period: usize,

    /// 손절 비율 (기본값: 3%)
    #[serde(default = "default_stop_loss")]
    pub stop_loss_pct: f64,

    /// 익절 비율 (기본값: 10%)
    #[serde(default = "default_take_profit")]
    pub take_profit_pct: f64,

    /// 최소 글로벌 스코어 (기본값: 60)
    #[serde(default = "default_min_global_score")]
    pub min_global_score: Decimal,
}

fn default_etf_list() -> Vec<String> {
    vec![
        "122630".to_string(), // 코스피 레버리지
        "233740".to_string(), // 코스닥 레버리지
        "252670".to_string(), // 코스피 인버스
        "251340".to_string(), // 코스닥 인버스
    ]
}

fn default_kospi_leverage() -> String {
    "122630".to_string()
}
fn default_kosdaq_leverage() -> String {
    "233740".to_string()
}
fn default_kospi_inverse() -> String {
    "252670".to_string()
}
fn default_kosdaq_inverse() -> String {
    "251340".to_string()
}
fn default_max_positions() -> usize {
    2
}
fn default_position_ratio() -> f64 {
    0.5
}
fn default_obv_period() -> usize {
    10
}
fn default_ma_short() -> usize {
    5
}
fn default_ma_medium() -> usize {
    20
}
fn default_ma_long() -> usize {
    60
}
fn default_rsi_period() -> usize {
    14
}
fn default_stop_loss() -> f64 {
    3.0
}
fn default_take_profit() -> f64 {
    10.0
}

fn default_min_global_score() -> Decimal {
    dec!(60)
}

impl Default for KosdaqFireRainConfig {
    fn default() -> Self {
        Self {
            symbols: default_etf_list(),
            kospi_leverage: "122630".to_string(),
            kosdaq_leverage: "233740".to_string(),
            kospi_inverse: "252670".to_string(),
            kosdaq_inverse: "251340".to_string(),
            max_positions: 2,
            position_ratio: 0.5,
            obv_period: 10,
            ma_short: 5,
            ma_medium: 20,
            ma_long: 60,
            rsi_period: 14,
            stop_loss_pct: 3.0,
            take_profit_pct: 10.0,
            min_global_score: default_min_global_score(),
        }
    }
}

/// ETF 타입.
#[derive(Debug, Clone, PartialEq)]
enum EtfType {
    KospiLeverage,
    KosdaqLeverage,
    KospiInverse,
    KosdaqInverse,
}

/// ETF 데이터와 지표.
#[derive(Debug, Clone)]
struct EtfData {
    ticker: String,
    etf_type: EtfType,
    prices: VecDeque<Decimal>,
    volumes: VecDeque<Decimal>,
    obv: VecDeque<Decimal>,
    gains: VecDeque<Decimal>,
    losses: VecDeque<Decimal>,
    current_price: Decimal,
    holdings: Decimal,
    entry_price: Decimal,
}

impl EtfData {
    fn new(ticker: String, etf_type: EtfType) -> Self {
        Self {
            ticker,
            etf_type,
            prices: VecDeque::new(),
            volumes: VecDeque::new(),
            obv: VecDeque::new(),
            gains: VecDeque::new(),
            losses: VecDeque::new(),
            current_price: Decimal::ZERO,
            holdings: Decimal::ZERO,
            entry_price: Decimal::ZERO,
        }
    }

    fn update(&mut self, price: Decimal, volume: Decimal) {
        // RSI용 gain/loss 계산
        if let Some(&prev) = self.prices.front() {
            let change = price - prev;
            if change > Decimal::ZERO {
                self.gains.push_front(change);
                self.losses.push_front(Decimal::ZERO);
            } else {
                self.gains.push_front(Decimal::ZERO);
                self.losses.push_front(change.abs());
            }

            // OBV 계산
            let prev_obv = self.obv.front().copied().unwrap_or(Decimal::ZERO);
            let new_obv = if price > prev {
                prev_obv + volume
            } else if price < prev {
                prev_obv - volume
            } else {
                prev_obv
            };
            self.obv.push_front(new_obv);
        } else {
            self.obv.push_front(volume);
        }

        self.prices.push_front(price);
        self.volumes.push_front(volume);
        self.current_price = price;

        // 버퍼 크기 제한
        let max_len = 70;
        while self.prices.len() > max_len {
            self.prices.pop_back();
        }
        while self.volumes.len() > max_len {
            self.volumes.pop_back();
        }
        while self.obv.len() > max_len {
            self.obv.pop_back();
        }
        while self.gains.len() > max_len {
            self.gains.pop_back();
        }
        while self.losses.len() > max_len {
            self.losses.pop_back();
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

    fn obv_trend(&self, period: usize) -> Option<bool> {
        if self.obv.len() < period {
            return None;
        }

        let current = *self.obv.front()?;
        let past = *self.obv.get(period - 1)?;

        Some(current > past)
    }

    fn is_ma_aligned_bullish(&self, short: usize, medium: usize, long: usize) -> bool {
        let ma_s = self.calculate_ma(short);
        let ma_m = self.calculate_ma(medium);
        let ma_l = self.calculate_ma(long);

        match (ma_s, ma_m, ma_l) {
            (Some(s), Some(m), Some(l)) => s > m && m > l,
            _ => false,
        }
    }

    fn is_ma_aligned_bearish(&self, short: usize, medium: usize, long: usize) -> bool {
        let ma_s = self.calculate_ma(short);
        let ma_m = self.calculate_ma(medium);
        let ma_l = self.calculate_ma(long);

        match (ma_s, ma_m, ma_l) {
            (Some(s), Some(m), Some(l)) => s < m && m < l,
            _ => false,
        }
    }
}

/// 코스닥 피레인 전략.
pub struct KosdaqFireRainStrategy {
    config: Option<KosdaqFireRainConfig>,
    symbols: Vec<Symbol>,

    /// ETF별 데이터
    etf_data: HashMap<String, EtfData>,

    /// 현재 날짜
    current_date: Option<chrono::NaiveDate>,

    /// 초기화 완료
    started: bool,

    /// 통계
    trades_count: u32,
    wins: u32,
    total_pnl: Decimal,

    initialized: bool,

    /// 전략 컨텍스트
    context: Option<Arc<RwLock<StrategyContext>>>,
}

impl KosdaqFireRainStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            symbols: Vec::new(),
            etf_data: HashMap::new(),
            current_date: None,
            started: false,
            trades_count: 0,
            wins: 0,
            total_pnl: Decimal::ZERO,
            initialized: false,
            context: None,
        }
    }

    /// RouteState와 GlobalScore 기반 진입 조건 체크
    fn can_enter(&self) -> bool {
        let context = match &self.context {
            Some(ctx) => ctx,
            None => return true, // context 없으면 기본 허용
        };

        let config = match &self.config {
            Some(cfg) => cfg,
            None => return true,
        };

        let ctx = match context.try_read() {
            Ok(ctx) => ctx,
            Err(_) => return true,
        };

        // RouteState 체크 (첫 번째 심볼 기준)
        if let Some(symbol) = self.symbols.first() {
            if let Some(route) = ctx.get_route_state(&symbol.base) {
                match route {
                    RouteState::Wait | RouteState::Overheat => {
                        debug!("[KosdaqFireRain] RouteState가 {:?}이므로 진입 불가", route);
                        return false;
                    }
                    _ => {}
                }
            }
        }

        // GlobalScore 체크 (첫 번째 심볼 기준)
        if let Some(symbol) = self.symbols.first() {
            if let Some(score) = ctx.get_global_score(&symbol.base) {
                if score.overall_score < config.min_global_score {
                    debug!(
                        "[KosdaqFireRain] GlobalScore {} < {} 기준 미달",
                        score.overall_score, config.min_global_score
                    );
                    return false;
                }
            }
        }

        true
    }

    /// 새로운 날인지 확인.
    fn is_new_day(&self, current_time: DateTime<Utc>) -> bool {
        match self.current_date {
            Some(date) => current_time.date_naive() != date,
            None => true,
        }
    }

    /// 현재 포지션 수 계산.
    fn current_position_count(&self) -> usize {
        self.etf_data
            .values()
            .filter(|d| d.holdings > Decimal::ZERO)
            .count()
    }

    /// 레버리지 매수 조건 확인.
    fn should_buy_leverage(&self, data: &EtfData) -> bool {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return false,
        };

        // 이미 보유 중이면 매수 안 함
        if data.holdings > Decimal::ZERO {
            return false;
        }

        // 최대 포지션 수 확인
        if self.current_position_count() >= config.max_positions {
            return false;
        }

        // OBV 상승 추세
        let obv_up = match data.obv_trend(config.obv_period) {
            Some(v) => v,
            None => return false,
        };

        if !obv_up {
            return false;
        }

        // MA 정배열
        let ma_bullish =
            data.is_ma_aligned_bullish(config.ma_short, config.ma_medium, config.ma_long);

        if !ma_bullish {
            return false;
        }

        // RSI 조건 (과매수 아닐 때)
        let rsi = match data.calculate_rsi(config.rsi_period) {
            Some(v) => v.to_f64().unwrap_or(50.0),
            None => return false,
        };

        let rsi_ok = rsi < 70.0 && rsi > 30.0;

        debug!(
            ticker = %data.ticker,
            obv_up = obv_up,
            ma_bullish = ma_bullish,
            rsi = %format!("{:.1}", rsi),
            "레버리지 매수 조건 체크"
        );

        rsi_ok
    }

    /// 인버스 매수 조건 확인.
    fn should_buy_inverse(&self, data: &EtfData) -> bool {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return false,
        };

        // 이미 보유 중이면 매수 안 함
        if data.holdings > Decimal::ZERO {
            return false;
        }

        // 최대 포지션 수 확인
        if self.current_position_count() >= config.max_positions {
            return false;
        }

        // 해당 인버스의 페어 레버리지 데이터 확인
        let pair_ticker = match data.etf_type {
            EtfType::KospiInverse => &config.kospi_leverage,
            EtfType::KosdaqInverse => &config.kosdaq_leverage,
            _ => return false,
        };

        let pair_data = match self.etf_data.get(pair_ticker) {
            Some(d) => d,
            None => return false,
        };

        // 페어 레버리지의 OBV 하락 추세
        let obv_down = match pair_data.obv_trend(config.obv_period) {
            Some(v) => !v, // 반대
            None => return false,
        };

        if !obv_down {
            return false;
        }

        // 페어 레버리지의 MA 역배열
        let ma_bearish =
            pair_data.is_ma_aligned_bearish(config.ma_short, config.ma_medium, config.ma_long);

        if !ma_bearish {
            return false;
        }

        // RSI 조건 (과매도 회복 구간)
        let rsi = match pair_data.calculate_rsi(config.rsi_period) {
            Some(v) => v.to_f64().unwrap_or(50.0),
            None => return false,
        };

        let rsi_ok = rsi < 40.0; // 레버리지가 하락 중

        debug!(
            ticker = %data.ticker,
            pair = %pair_ticker,
            obv_down = obv_down,
            ma_bearish = ma_bearish,
            rsi = %format!("{:.1}", rsi),
            "인버스 매수 조건 체크"
        );

        rsi_ok
    }

    /// 매도 조건 확인.
    fn should_sell(&self, data: &EtfData) -> Option<String> {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return None,
        };

        // 보유 중이 아니면 매도 불가
        if data.holdings <= Decimal::ZERO {
            return None;
        }

        // 손절 체크
        if data.entry_price > Decimal::ZERO {
            let pnl_pct = ((data.current_price - data.entry_price) / data.entry_price * dec!(100))
                .to_f64()
                .unwrap_or(0.0);

            if pnl_pct <= -config.stop_loss_pct {
                return Some("stop_loss".to_string());
            }

            if pnl_pct >= config.take_profit_pct {
                return Some("take_profit".to_string());
            }
        }

        // 레버리지는 MA 역배열 시 매도
        if data.etf_type == EtfType::KospiLeverage || data.etf_type == EtfType::KosdaqLeverage {
            if data.is_ma_aligned_bearish(config.ma_short, config.ma_medium, config.ma_long) {
                return Some("ma_bearish".to_string());
            }

            // OBV 하락 전환
            if let Some(false) = data.obv_trend(config.obv_period) {
                return Some("obv_down".to_string());
            }
        }

        // 인버스는 MA 정배열 시 매도
        if data.etf_type == EtfType::KospiInverse || data.etf_type == EtfType::KosdaqInverse {
            let pair_ticker = match data.etf_type {
                EtfType::KospiInverse => &config.kospi_leverage,
                EtfType::KosdaqInverse => &config.kosdaq_leverage,
                _ => return None,
            };

            if let Some(pair_data) = self.etf_data.get(pair_ticker) {
                if pair_data.is_ma_aligned_bullish(
                    config.ma_short,
                    config.ma_medium,
                    config.ma_long,
                ) {
                    return Some("ma_bullish".to_string());
                }
            }
        }

        None
    }

    /// 신호 생성.
    fn generate_signals(&mut self) -> Vec<Signal> {
        let config = match self.config.as_ref() {
            Some(c) => c.clone(),
            None => return Vec::new(),
        };

        let mut signals = Vec::new();

        // 각 ETF에 대해 신호 확인
        let tickers: Vec<String> = self.etf_data.keys().cloned().collect();

        for ticker in tickers {
            let data = match self.etf_data.get(&ticker) {
                Some(d) => d.clone(),
                None => continue,
            };

            // Symbol.base와 ticker 비교 (Symbol.to_string()은 "base/quote" 형식이므로 base만 비교)
            let symbol = match self.symbols.iter().find(|s| s.base == ticker) {
                Some(s) => s.clone(),
                None => continue,
            };

            // 매도 신호 확인
            if let Some(reason) = self.should_sell(&data) {
                signals.push(
                    Signal::exit("kosdaq_fire_rain", symbol.clone(), Side::Sell)
                        .with_strength(1.0)
                        .with_prices(Some(data.current_price), None, None)
                        .with_metadata("exit_reason", json!(reason))
                        .with_metadata("etf_type", json!(format!("{:?}", data.etf_type))),
                );
                info!(
                    ticker = %ticker,
                    reason = %reason,
                    price = %data.current_price,
                    "매도 신호"
                );
                continue;
            }

            // 매수 신호 확인
            let should_buy = match data.etf_type {
                EtfType::KospiLeverage | EtfType::KosdaqLeverage => self.should_buy_leverage(&data),
                EtfType::KospiInverse | EtfType::KosdaqInverse => self.should_buy_inverse(&data),
            };

            if should_buy {
                // can_enter() 체크 - 진입 조건 미충족 시 스킵
                if !self.can_enter() {
                    debug!("[KosdaqFireRain] can_enter() 실패 - 매수 신호 스킵");
                    continue;
                }

                signals.push(
                    Signal::entry("kosdaq_fire_rain", symbol, Side::Buy)
                        .with_strength(config.position_ratio)
                        .with_prices(Some(data.current_price), None, None)
                        .with_metadata("etf_type", json!(format!("{:?}", data.etf_type)))
                        .with_metadata("action", json!("buy")),
                );
                info!(
                    ticker = %ticker,
                    etf_type = ?data.etf_type,
                    price = %data.current_price,
                    "매수 신호"
                );
            }
        }

        signals
    }
}

impl Default for KosdaqFireRainStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for KosdaqFireRainStrategy {
    fn name(&self) -> &str {
        "KOSDAQ Fire Rain"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "코스닥 피레인 전략. 코스피/코스닥 레버리지와 인버스 ETF를 조합한 \
         양방향 전략. OBV와 MA 조합으로 추세 판단."
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let fr_config: KosdaqFireRainConfig = serde_json::from_value(config)?;

        info!(
            symbols = ?fr_config.symbols,
            max_positions = fr_config.max_positions,
            position_ratio = %format!("{:.0}%", fr_config.position_ratio * 100.0),
            "코스닥 피레인 전략 초기화"
        );

        // ETF 데이터 초기화
        for ticker in &fr_config.symbols {
            let symbol = Symbol::stock(ticker, "KRW");
            self.symbols.push(symbol);

            let etf_type = if ticker == &fr_config.kospi_leverage {
                EtfType::KospiLeverage
            } else if ticker == &fr_config.kosdaq_leverage {
                EtfType::KosdaqLeverage
            } else if ticker == &fr_config.kospi_inverse {
                EtfType::KospiInverse
            } else if ticker == &fr_config.kosdaq_inverse {
                EtfType::KosdaqInverse
            } else {
                continue;
            };

            self.etf_data
                .insert(ticker.clone(), EtfData::new(ticker.clone(), etf_type));
        }

        self.config = Some(fr_config);
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

        // base 심볼만 추출 (229200/KRW -> 229200)
        let symbol_str = data.symbol.base.clone();

        // 등록된 ETF인지 확인
        if !self.etf_data.contains_key(&symbol_str) {
            return Ok(vec![]);
        }

        // kline에서 데이터 추출
        let (close, volume, timestamp) = match &data.data {
            MarketDataType::Kline(kline) => (kline.close, kline.volume, kline.open_time),
            _ => return Ok(vec![]),
        };

        // 새 날짜 확인
        if self.is_new_day(timestamp) {
            self.current_date = Some(timestamp.date_naive());
        }

        // ETF 데이터 업데이트
        if let Some(etf) = self.etf_data.get_mut(&symbol_str) {
            etf.update(close, volume);
        }

        // 충분한 데이터가 있는지 확인
        let all_have_data = self.etf_data.values().all(|d| d.prices.len() >= 60);

        if !all_have_data {
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
        let ticker = order.symbol.to_string();
        let price = order.price.unwrap_or(Decimal::ZERO);

        if let Some(etf) = self.etf_data.get_mut(&ticker) {
            match order.side {
                Side::Buy => {
                    let old_value = etf.holdings * etf.entry_price;
                    let new_value = order.quantity * price;
                    let total_qty = etf.holdings + order.quantity;

                    if total_qty > Decimal::ZERO {
                        etf.entry_price = (old_value + new_value) / total_qty;
                    }
                    etf.holdings += order.quantity;
                }
                Side::Sell => {
                    let pnl = order.quantity * (price - etf.entry_price);
                    self.total_pnl += pnl;
                    if pnl > Decimal::ZERO {
                        self.wins += 1;
                    }
                    self.trades_count += 1;

                    etf.holdings -= order.quantity;
                    if etf.holdings <= Decimal::ZERO {
                        etf.holdings = Decimal::ZERO;
                        etf.entry_price = Decimal::ZERO;
                    }
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
        let ticker = position.symbol.to_string();

        if let Some(etf) = self.etf_data.get_mut(&ticker) {
            etf.holdings = position.quantity;
            if position.quantity > Decimal::ZERO {
                etf.entry_price = position.entry_price;
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
            "코스닥 피레인 전략 종료"
        );

        Ok(())
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("StrategyContext injected into KosdaqFireRain strategy");
    }

    fn get_state(&self) -> Value {
        let holdings: HashMap<_, _> = self
            .etf_data
            .iter()
            .filter(|(_, v)| v.holdings > Decimal::ZERO)
            .map(|(k, v)| {
                (
                    k.clone(),
                    json!({
                        "holdings": v.holdings.to_string(),
                        "entry_price": v.entry_price.to_string(),
                        "current_price": v.current_price.to_string(),
                        "etf_type": format!("{:?}", v.etf_type),
                    }),
                )
            })
            .collect();

        json!({
            "initialized": self.initialized,
            "started": self.started,
            "position_count": self.current_position_count(),
            "holdings": holdings,
            "trades_count": self.trades_count,
            "wins": self.wins,
            "total_pnl": self.total_pnl.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_kosdaq_fire_rain_initialization() {
        let mut strategy = KosdaqFireRainStrategy::new();

        let config = json!({
            "symbols": ["122630", "233740", "252670", "251340"],
            "max_positions": 2
        });

        strategy.initialize(config).await.unwrap();
        assert!(strategy.initialized);
        assert_eq!(strategy.etf_data.len(), 4);
    }

    #[test]
    fn test_etf_data_update() {
        let mut data = EtfData::new("122630".to_string(), EtfType::KospiLeverage);

        // 데이터 추가
        for i in 1..=20 {
            data.update(Decimal::from(100 + i), Decimal::from(10000));
        }

        assert_eq!(data.prices.len(), 20);
        assert!(data.obv.front().unwrap() > &Decimal::ZERO);

        // MA 계산 확인
        let ma5 = data.calculate_ma(5);
        assert!(ma5.is_some());
    }
}

// 전략 레지스트리에 자동 등록
use crate::register_strategy;

register_strategy! {
    id: "kosdaq_fire_rain",
    aliases: ["kosdaq_surge"],
    name: "코스닥 급등주",
    description: "코스닥 급등 종목 포착 전략입니다.",
    timeframe: "15m",
    symbols: ["122630", "252670", "233740", "251340"],
    category: Intraday,
    markets: [Stock],
    type: KosdaqFireRainStrategy
}
