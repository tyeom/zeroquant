//! US Market Cap TOP Strategy
//!
//! 미국 시총 상위 종목에 투자하는 전략입니다.
//!
//! ## 전략 개요
//!
//! 1. 시총 상위 N개 종목 선정
//! 2. 동일 비중 또는 시총 가중 비중으로 투자
//! 3. 정기적 리밸런싱 (월/분기)
//!
//! ## 대상 종목 예시 (기본 TOP 10)
//! - AAPL (Apple)
//! - MSFT (Microsoft)
//! - GOOGL (Alphabet)
//! - AMZN (Amazon)
//! - NVDA (NVIDIA)
//! - META (Meta)
//! - TSLA (Tesla)
//! - BRK.B (Berkshire)
//! - UNH (UnitedHealth)
//! - JNJ (Johnson & Johnson)

use crate::Strategy;
use async_trait::async_trait;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use trader_core::domain::{RouteState, StrategyContext};
use trader_core::{
    MarketData, MarketDataType, MarketType, Order, Position, Side, Signal, SignalType, Symbol,
};

/// 비중 할당 방식
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeightingMethod {
    /// 동일 비중
    Equal,
    /// 시총 가중
    MarketCapWeighted,
    /// 역변동성 가중
    InverseVolatility,
}

impl Default for WeightingMethod {
    fn default() -> Self {
        Self::Equal
    }
}

/// Market Cap TOP 전략 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketCapTopConfig {
    /// 상위 N개 종목
    #[serde(default = "default_top_n")]
    pub top_n: usize,

    /// 총 투자 금액
    #[serde(default = "default_total_amount")]
    pub total_amount: Decimal,

    /// 비중 할당 방식
    #[serde(default)]
    pub weighting_method: WeightingMethod,

    /// 리밸런싱 간격 (일)
    #[serde(default = "default_rebalance_days")]
    pub rebalance_days: u32,

    /// 리밸런싱 허용 오차 (%)
    #[serde(default = "default_rebalance_threshold")]
    pub rebalance_threshold: Decimal,

    /// 대상 종목 리스트 (빈 경우 기본 TOP 종목 사용)
    #[serde(default)]
    pub symbols: Vec<String>,

    /// 모멘텀 필터 사용 여부
    #[serde(default)]
    pub use_momentum_filter: bool,

    /// 모멘텀 기간 (일)
    #[serde(default = "default_momentum_period")]
    pub momentum_period: usize,

    /// 최소 글로벌 스코어 (기본값: 60)
    #[serde(default = "default_min_global_score")]
    pub min_global_score: Decimal,
}

fn default_top_n() -> usize {
    10
}
fn default_total_amount() -> Decimal {
    dec!(10000000)
}
fn default_rebalance_days() -> u32 {
    30
}
fn default_rebalance_threshold() -> Decimal {
    dec!(5)
}
fn default_momentum_period() -> usize {
    252
}
fn default_min_global_score() -> Decimal {
    dec!(60)
}

impl Default for MarketCapTopConfig {
    fn default() -> Self {
        Self {
            top_n: default_top_n(),
            total_amount: default_total_amount(),
            weighting_method: WeightingMethod::Equal,
            rebalance_days: default_rebalance_days(),
            rebalance_threshold: default_rebalance_threshold(),
            symbols: Vec::new(),
            use_momentum_filter: false,
            momentum_period: default_momentum_period(),
            min_global_score: default_min_global_score(),
        }
    }
}

/// 기본 시총 TOP 종목
fn default_top_symbols() -> Vec<&'static str> {
    vec![
        "AAPL",  // Apple
        "MSFT",  // Microsoft
        "GOOGL", // Alphabet
        "AMZN",  // Amazon
        "NVDA",  // NVIDIA
        "META",  // Meta
        "TSLA",  // Tesla
        "BRK.B", // Berkshire Hathaway
        "UNH",   // UnitedHealth
        "JNJ",   // Johnson & Johnson
        "V",     // Visa
        "XOM",   // Exxon Mobil
        "JPM",   // JPMorgan
        "PG",    // Procter & Gamble
        "MA",    // Mastercard
    ]
}

/// 종목별 포지션 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionInfo {
    pub symbol: String,
    pub target_weight: Decimal,
    pub current_weight: Decimal,
    pub quantity: Decimal,
    pub avg_price: Decimal,
    pub current_price: Decimal,
    pub pnl: Decimal,
}

/// Market Cap TOP 전략 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketCapTopState {
    /// 현재 포지션
    pub positions: HashMap<String, PositionInfo>,
    /// 마지막 리밸런싱 날짜
    pub last_rebalance_day: Option<u32>,
    /// 총 포트폴리오 가치
    pub portfolio_value: Decimal,
    /// 현금
    pub cash: Decimal,
}

impl Default for MarketCapTopState {
    fn default() -> Self {
        Self {
            positions: HashMap::new(),
            last_rebalance_day: None,
            portfolio_value: Decimal::ZERO,
            cash: Decimal::ZERO,
        }
    }
}

/// Market Cap TOP 전략
pub struct MarketCapTopStrategy {
    config: Option<MarketCapTopConfig>,
    state: MarketCapTopState,
    /// 심볼별 가격 히스토리
    price_history: HashMap<String, Vec<Decimal>>,
    /// 현재 날짜
    current_day: u32,
    /// 활성 심볼 리스트
    active_symbols: Vec<String>,
    initialized: bool,
    /// 전략 컨텍스트
    context: Option<Arc<RwLock<StrategyContext>>>,
}

impl MarketCapTopStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            state: MarketCapTopState::default(),
            price_history: HashMap::new(),
            current_day: 0,
            active_symbols: Vec::new(),
            initialized: false,
            context: None,
        }
    }

    /// RouteState와 GlobalScore 기반 진입 조건 체크
    fn can_enter(&self, symbol: &str) -> bool {
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

        // RouteState 체크
        if let Some(route) = ctx.get_route_state(symbol) {
            match route {
                RouteState::Wait | RouteState::Overheat => {
                    debug!("[MarketCapTop] RouteState가 {:?}이므로 진입 불가", route);
                    return false;
                }
                _ => {}
            }
        }

        // GlobalScore 체크
        if let Some(score) = ctx.get_global_score(symbol) {
            if score.overall_score < config.min_global_score {
                debug!(
                    "[MarketCapTop] {} GlobalScore {} < {} 기준 미달",
                    symbol, score.overall_score, config.min_global_score
                );
                return false;
            }
        }

        true
    }

    /// 목표 비중 계산
    fn calculate_target_weights(&self) -> HashMap<String, Decimal> {
        let config = match &self.config {
            Some(c) => c,
            None => return HashMap::new(),
        };

        let mut weights = HashMap::new();

        match config.weighting_method {
            WeightingMethod::Equal => {
                let weight = Decimal::ONE / Decimal::from(self.active_symbols.len());
                for symbol in &self.active_symbols {
                    weights.insert(symbol.clone(), weight);
                }
            }
            WeightingMethod::MarketCapWeighted => {
                // 실제로는 시총 데이터가 필요하지만, 여기서는 동일 비중 사용
                let weight = Decimal::ONE / Decimal::from(self.active_symbols.len());
                for symbol in &self.active_symbols {
                    weights.insert(symbol.clone(), weight);
                }
            }
            WeightingMethod::InverseVolatility => {
                // 변동성 역수 비중
                let mut volatilities = HashMap::new();
                let mut total_inv_vol = Decimal::ZERO;

                for symbol in &self.active_symbols {
                    if let Some(prices) = self.price_history.get(symbol) {
                        if prices.len() >= 20 {
                            let vol = self.calculate_volatility(prices, 20);
                            if vol > Decimal::ZERO {
                                let inv_vol = Decimal::ONE / vol;
                                volatilities.insert(symbol.clone(), inv_vol);
                                total_inv_vol += inv_vol;
                            }
                        }
                    }
                }

                if total_inv_vol > Decimal::ZERO {
                    for symbol in &self.active_symbols {
                        let weight =
                            volatilities.get(symbol).unwrap_or(&Decimal::ZERO) / total_inv_vol;
                        weights.insert(symbol.clone(), weight);
                    }
                } else {
                    // 데이터 부족 시 동일 비중
                    let weight = Decimal::ONE / Decimal::from(self.active_symbols.len());
                    for symbol in &self.active_symbols {
                        weights.insert(symbol.clone(), weight);
                    }
                }
            }
        }

        weights
    }

    /// 변동성 계산 (표준편차)
    fn calculate_volatility(&self, prices: &[Decimal], period: usize) -> Decimal {
        if prices.len() < period {
            return Decimal::ZERO;
        }

        let returns: Vec<Decimal> = prices
            .windows(2)
            .take(period)
            .map(|w| (w[0] - w[1]) / w[1])
            .collect();

        if returns.is_empty() {
            return Decimal::ZERO;
        }

        let mean: Decimal = returns.iter().sum::<Decimal>() / Decimal::from(returns.len());
        let variance: Decimal = returns
            .iter()
            .map(|r| {
                let diff = *r - mean;
                diff * diff
            })
            .sum::<Decimal>()
            / Decimal::from(returns.len());

        // 제곱근 근사 (뉴턴-랩슨)
        self.sqrt_approx(variance)
    }

    /// 제곱근 근사
    fn sqrt_approx(&self, x: Decimal) -> Decimal {
        if x <= Decimal::ZERO {
            return Decimal::ZERO;
        }

        let mut guess = x / dec!(2);
        for _ in 0..10 {
            guess = (guess + x / guess) / dec!(2);
        }
        guess
    }

    /// 모멘텀 스코어 계산
    fn calculate_momentum(&self, prices: &[Decimal]) -> Decimal {
        let config = match &self.config {
            Some(c) => c,
            None => return Decimal::ZERO,
        };

        if prices.len() < config.momentum_period {
            return Decimal::ZERO;
        }

        let current = prices[0];
        let past = prices[config.momentum_period - 1];

        if past > Decimal::ZERO {
            (current - past) / past * dec!(100)
        } else {
            Decimal::ZERO
        }
    }

    /// 모멘텀 필터 적용
    fn filter_by_momentum(&self) -> Vec<String> {
        let config = match &self.config {
            Some(c) => c,
            None => return self.active_symbols.clone(),
        };

        if !config.use_momentum_filter {
            return self.active_symbols.clone();
        }

        let mut momentum_scores: Vec<(String, Decimal)> = self
            .active_symbols
            .iter()
            .filter_map(|symbol| {
                self.price_history.get(symbol).map(|prices| {
                    let momentum = self.calculate_momentum(prices);
                    (symbol.clone(), momentum)
                })
            })
            .collect();

        // 모멘텀 순 정렬
        momentum_scores.sort_by(|a, b| b.1.cmp(&a.1));

        // 양의 모멘텀만 선택 (최소 1개)
        let filtered: Vec<String> = momentum_scores
            .into_iter()
            .filter(|(_, m)| *m > Decimal::ZERO)
            .map(|(s, _)| s)
            .collect();

        if filtered.is_empty() {
            // 모든 종목이 음의 모멘텀이면 상위 절반만
            self.active_symbols
                .iter()
                .take(self.active_symbols.len() / 2)
                .cloned()
                .collect()
        } else {
            filtered
        }
    }

    /// 리밸런싱 필요 여부 확인
    fn should_rebalance(&self) -> bool {
        let config = match &self.config {
            Some(c) => c,
            None => return false,
        };

        if let Some(last_day) = self.state.last_rebalance_day {
            let days_passed = if self.current_day >= last_day {
                self.current_day - last_day
            } else {
                365 - last_day + self.current_day
            };
            days_passed >= config.rebalance_days
        } else {
            true
        }
    }

    /// 현재 비중과 목표 비중의 차이가 임계값을 넘는지 확인
    fn needs_rebalancing(&self, target_weights: &HashMap<String, Decimal>) -> bool {
        let config = match &self.config {
            Some(c) => c,
            None => return false,
        };

        for (symbol, target) in target_weights {
            let current = self
                .state
                .positions
                .get(symbol)
                .map(|p| p.current_weight)
                .unwrap_or(Decimal::ZERO);

            let diff = (target - current).abs() * dec!(100);
            if diff > config.rebalance_threshold {
                return true;
            }
        }
        false
    }

    /// 신호 생성
    fn generate_signals(
        &mut self,
        symbol_data: &str,
        current_price: Decimal,
        timestamp: i64,
    ) -> Vec<Signal> {
        let config = match &self.config {
            Some(c) => c,
            None => return Vec::new(),
        };
        let mut signals = Vec::new();

        // 날짜 업데이트
        let day_of_year = ((timestamp / 86400) % 365) as u32;
        self.current_day = day_of_year;

        // 포지션 업데이트
        if let Some(pos) = self.state.positions.get_mut(symbol_data) {
            pos.current_price = current_price;
            pos.pnl = (current_price - pos.avg_price) * pos.quantity;
        }

        // 첫 번째 심볼에서만 리밸런싱 체크
        if symbol_data
            != self
                .active_symbols
                .first()
                .map(|s| s.as_str())
                .unwrap_or("")
        {
            return signals;
        }

        if !self.should_rebalance() {
            return signals;
        }

        // 목표 비중 계산
        let filtered_symbols = self.filter_by_momentum();
        let target_weights = self.calculate_target_weights();

        if !self.needs_rebalancing(&target_weights) {
            return signals;
        }

        // 리밸런싱 실행
        self.state.last_rebalance_day = Some(self.current_day);

        // 각 종목별 신호 생성
        for sym in &filtered_symbols {
            let target_weight = target_weights.get(sym).unwrap_or(&Decimal::ZERO);
            let target_value = config.total_amount * target_weight;

            if let Some(sym_current_price) = self.price_history.get(sym).and_then(|p| p.first()) {
                let target_quantity = target_value / sym_current_price;
                let current_quantity = self
                    .state
                    .positions
                    .get(sym)
                    .map(|p| p.quantity)
                    .unwrap_or(Decimal::ZERO);

                let diff = target_quantity - current_quantity;

                if diff.abs() * sym_current_price
                    > config.total_amount * config.rebalance_threshold / dec!(100)
                {
                    // 포지션 업데이트
                    self.state.positions.insert(
                        sym.clone(),
                        PositionInfo {
                            symbol: sym.clone(),
                            target_weight: *target_weight,
                            current_weight: *target_weight,
                            quantity: target_quantity,
                            avg_price: *sym_current_price,
                            current_price: *sym_current_price,
                            pnl: Decimal::ZERO,
                        },
                    );

                    // 심볼 생성
                    let symbol = Symbol::new(sym, "USD", MarketType::Stock);

                    let (side, signal_type) = if diff > Decimal::ZERO {
                        // BUY 신호 생성 전 can_enter 체크
                        if !self.can_enter(sym) {
                            continue;
                        }
                        (Side::Buy, SignalType::Entry)
                    } else {
                        (Side::Sell, SignalType::Exit)
                    };

                    let signal = Signal::new("market_cap_top", symbol, side, signal_type)
                        .with_strength(1.0)
                        .with_metadata("target_weight", json!(target_weight.to_string()))
                        .with_metadata("current_quantity", json!(current_quantity.to_string()))
                        .with_metadata("target_quantity", json!(target_quantity.to_string()))
                        .with_metadata(
                            "weighting_method",
                            json!(format!("{:?}", config.weighting_method)),
                        );

                    signals.push(signal);

                    info!(
                        "[MarketCapTOP] 리밸런싱: {} 목표 비중 {:.1}%",
                        sym,
                        target_weight * dec!(100)
                    );
                }
            }
        }

        signals
    }
}

impl Default for MarketCapTopStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for MarketCapTopStrategy {
    fn name(&self) -> &str {
        "US Market Cap TOP"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "미국 시총 상위 종목에 투자하는 패시브 전략"
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mct_config: MarketCapTopConfig = serde_json::from_value(config)?;

        // 활성 심볼 설정
        self.active_symbols = if mct_config.symbols.is_empty() {
            default_top_symbols()
                .into_iter()
                .take(mct_config.top_n)
                .map(|s| s.to_string())
                .collect()
        } else {
            mct_config
                .symbols
                .clone()
                .into_iter()
                .take(mct_config.top_n)
                .collect()
        };

        info!(
            top_n = %mct_config.top_n,
            weighting = ?mct_config.weighting_method,
            symbols = ?self.active_symbols,
            "Initializing Market Cap TOP strategy"
        );

        self.state = MarketCapTopState::default();
        self.state.cash = mct_config.total_amount;
        self.config = Some(mct_config);
        self.price_history.clear();
        self.current_day = 0;
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

        // base 심볼만 추출 (AAPL/USD -> AAPL)
        let symbol_str = data.symbol.base.clone();

        // 활성 심볼이 아니면 무시
        if !self.active_symbols.contains(&symbol_str) {
            return Ok(vec![]);
        }

        // 가격 및 시간 추출
        let (current_price, timestamp) = match &data.data {
            MarketDataType::Kline(kline) => (kline.close, kline.close_time.timestamp()),
            MarketDataType::Ticker(ticker) => (ticker.last, 0i64),
            MarketDataType::Trade(trade) => (trade.price, trade.timestamp.timestamp()),
            _ => return Ok(vec![]),
        };

        // 가격 히스토리 업데이트
        let prices = self.price_history.entry(symbol_str.clone()).or_default();
        prices.insert(0, current_price);
        if prices.len() > 260 {
            prices.pop();
        }

        // 신호 생성
        let signals = self.generate_signals(&symbol_str, current_price, timestamp);

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
            "[MarketCapTOP] 주문 체결: {} {} @ {}",
            order.symbol, order.quantity, fill_price
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
        info!("Market Cap TOP strategy shutdown");
        self.initialized = false;
        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "config": self.config,
            "state": self.state,
            "active_symbols": self.active_symbols,
            "positions_count": self.state.positions.len(),
            "initialized": self.initialized,
        })
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("StrategyContext injected into MarketCapTop strategy");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = MarketCapTopConfig::default();
        assert_eq!(config.top_n, 10);
        assert_eq!(config.weighting_method, WeightingMethod::Equal);
    }

    #[tokio::test]
    async fn test_initialization() {
        let mut strategy = MarketCapTopStrategy::new();

        let config = json!({
            "top_n": 5,
            "weighting_method": "Equal"
        });

        strategy.initialize(config).await.unwrap();
        assert!(strategy.initialized);
        assert_eq!(strategy.active_symbols.len(), 5);
    }

    #[test]
    fn test_strategy_creation() {
        let strategy = MarketCapTopStrategy::new();
        assert_eq!(strategy.name(), "US Market Cap TOP");
    }

    #[tokio::test]
    async fn test_equal_weights() {
        let mut strategy = MarketCapTopStrategy::new();

        let config = json!({
            "top_n": 5,
            "weighting_method": "Equal"
        });

        strategy.initialize(config).await.unwrap();
        let weights = strategy.calculate_target_weights();

        assert_eq!(weights.len(), 5);
        for weight in weights.values() {
            assert_eq!(*weight, dec!(0.2));
        }
    }

    #[tokio::test]
    async fn test_custom_symbols() {
        let mut strategy = MarketCapTopStrategy::new();

        let config = json!({
            "symbols": ["AAPL", "MSFT", "GOOGL"],
            "top_n": 10
        });

        strategy.initialize(config).await.unwrap();
        assert_eq!(strategy.active_symbols.len(), 3);
    }
}

// 전략 레지스트리에 자동 등록
use crate::register_strategy;

register_strategy! {
    id: "market_cap_top",
    aliases: [],
    name: "시총 TOP",
    description: "시가총액 상위 종목에 투자하는 전략입니다.",
    timeframe: "1d",
    symbols: ["AAPL", "MSFT", "GOOGL", "AMZN", "NVDA", "META", "TSLA", "BRK.B", "JPM", "V"],
    category: Daily,
    markets: [Stock, Stock],
    type: MarketCapTopStrategy
}
