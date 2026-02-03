//! 듀얼 모멘텀 전략 (한국 주식 + 미국 채권)
//!
//! 한국 주식과 미국 채권의 듀얼 모멘텀을 활용한 자산배분 전략.
//! 두 시장의 상대 모멘텀과 절대 모멘텀을 결합합니다.
//!
//! Python 1번 전략 변환.
//!
//! # 전략 개요
//!
//! ## 자산
//! - **한국 주식**: KODEX 200 (069500), KODEX 코스닥150 (229200)
//! - **미국 채권**: TLT (20년 국채), IEF (중기 국채), BIL (단기 국채)
//! - **안전 자산**: 달러 MMF 또는 BIL
//!
//! ## 모멘텀 계산
//! - 상대 모멘텀: 한국 주식 vs 미국 채권 수익률 비교
//! - 절대 모멘텀: 선택된 자산의 수익률이 양수인지 확인
//!
//! ## 자산 선택 로직
//! 1. 한국 주식 모멘텀 > 미국 채권 모멘텀 → 한국 주식 선택
//! 2. 선택된 자산의 절대 모멘텀 < 0 → 안전 자산(BIL)
//! 3. 월간 리밸런싱

use async_trait::async_trait;
use chrono::{DateTime, Datelike, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use trader_core::domain::{RouteState, StrategyContext};

use crate::strategies::common::rebalance::{
    PortfolioPosition, RebalanceCalculator, RebalanceConfig, RebalanceOrderSide, TargetAllocation,
};
use crate::traits::Strategy;
use trader_core::{MarketData, MarketDataType, Order, Position, Side, Signal, Symbol};

/// 자산 클래스.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DualAssetClass {
    /// 한국 주식
    Stock,
    /// 미국 채권
    UsBond,
    /// 안전 자산 (현금 대용)
    Safe,
}

/// 자산 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualAsset {
    pub symbol: String,
    pub name: String,
    pub asset_class: DualAssetClass,
    pub market: String, // "KR" or "US"
}

impl DualAsset {
    pub fn kr_stock(symbol: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            name: name.into(),
            asset_class: DualAssetClass::Stock,
            market: "KR".to_string(),
        }
    }

    pub fn us_bond(symbol: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            name: name.into(),
            asset_class: DualAssetClass::UsBond,
            market: "US".to_string(),
        }
    }

    pub fn safe(symbol: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            name: name.into(),
            asset_class: DualAssetClass::Safe,
            market: "US".to_string(),
        }
    }
}

/// 듀얼 모멘텀 전략 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualMomentumConfig {
    /// 총 투자 금액
    #[serde(default = "default_total_amount")]
    pub total_amount: Decimal,

    /// 모멘텀 기간 (일)
    #[serde(default = "default_momentum_period")]
    pub momentum_period: usize,

    /// 리밸런싱 허용 오차 (%)
    #[serde(default = "default_rebalance_threshold")]
    pub rebalance_threshold: Decimal,

    /// 한국 주식 배분 비율 (0~1, 나머지는 미국 채권)
    #[serde(default = "default_kr_allocation")]
    pub kr_allocation: f64,

    /// 절대 모멘텀 사용 여부
    #[serde(default = "default_use_absolute")]
    pub use_absolute_momentum: bool,

    /// 커스텀 한국 주식
    pub kr_stocks: Option<Vec<DualAsset>>,

    /// 커스텀 미국 채권
    pub us_bonds: Option<Vec<DualAsset>>,

    /// 최소 글로벌 스코어 (기본값: 60)
    #[serde(default = "default_min_global_score")]
    pub min_global_score: Decimal,
}

fn default_total_amount() -> Decimal {
    dec!(10000000)
}
fn default_momentum_period() -> usize {
    63
} // 3개월
fn default_rebalance_threshold() -> Decimal {
    dec!(5)
}
fn default_kr_allocation() -> f64 {
    0.5
}
fn default_use_absolute() -> bool {
    true
}
fn default_min_global_score() -> Decimal {
    dec!(60)
}

impl Default for DualMomentumConfig {
    fn default() -> Self {
        Self {
            total_amount: default_total_amount(),
            momentum_period: default_momentum_period(),
            rebalance_threshold: default_rebalance_threshold(),
            kr_allocation: default_kr_allocation(),
            use_absolute_momentum: true,
            kr_stocks: None,
            us_bonds: None,
            min_global_score: default_min_global_score(),
        }
    }
}

impl DualMomentumConfig {
    /// 기본 한국 주식 목록.
    pub fn default_kr_stocks() -> Vec<DualAsset> {
        vec![
            DualAsset::kr_stock("069500", "KODEX 200"),
            DualAsset::kr_stock("229200", "KODEX 코스닥150"),
        ]
    }

    /// 기본 미국 채권 목록.
    pub fn default_us_bonds() -> Vec<DualAsset> {
        vec![
            DualAsset::us_bond("TLT", "장기 국채 20년"),
            DualAsset::us_bond("IEF", "중기 국채 7-10년"),
            DualAsset::safe("BIL", "단기 국채 (안전)"),
        ]
    }

    pub fn get_kr_stocks(&self) -> Vec<DualAsset> {
        self.kr_stocks
            .clone()
            .unwrap_or_else(Self::default_kr_stocks)
    }

    pub fn get_us_bonds(&self) -> Vec<DualAsset> {
        self.us_bonds.clone().unwrap_or_else(Self::default_us_bonds)
    }

    pub fn get_all_assets(&self) -> Vec<DualAsset> {
        let mut all = self.get_kr_stocks();
        all.extend(self.get_us_bonds());
        all
    }
}

/// 자산별 데이터.
#[derive(Debug, Clone)]
struct AssetData {
    symbol: String,
    asset_class: DualAssetClass,
    market: String,
    current_price: Decimal,
    prices: Vec<Decimal>,
    momentum: Decimal,
    current_holdings: Decimal,
}

impl AssetData {
    fn new(symbol: String, asset_class: DualAssetClass, market: String) -> Self {
        Self {
            symbol,
            asset_class,
            market,
            current_price: Decimal::ZERO,
            prices: Vec::new(),
            momentum: Decimal::ZERO,
            current_holdings: Decimal::ZERO,
        }
    }

    fn add_price(&mut self, price: Decimal) {
        self.current_price = price;
        self.prices.push(price);
        if self.prices.len() > 150 {
            self.prices.remove(0);
        }
    }

    fn calculate_momentum(&mut self, period: usize) {
        if self.prices.len() > period {
            let current = self.prices[self.prices.len() - 1];
            let past = self.prices[self.prices.len() - 1 - period];

            if past > Decimal::ZERO {
                self.momentum = (current - past) / past;
            }
        }
    }
}

/// 듀얼 모멘텀 전략.
pub struct DualMomentumStrategy {
    config: Option<DualMomentumConfig>,
    symbols: Vec<Symbol>,
    asset_data: HashMap<String, AssetData>,

    /// 현재 선택된 자산 클래스
    selected_class: Option<DualAssetClass>,

    /// 마지막 리밸런싱 월
    last_rebalance_month: Option<u32>,

    /// 통계
    trades_count: u32,
    total_pnl: Decimal,

    /// StrategyContext 참조
    context: Option<Arc<RwLock<StrategyContext>>>,

    initialized: bool,
}

impl DualMomentumStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            symbols: Vec::new(),
            asset_data: HashMap::new(),
            selected_class: None,
            last_rebalance_month: None,
            trades_count: 0,
            total_pnl: Decimal::ZERO,
            context: None,
            initialized: false,
        }
    }

    /// StrategyContext를 기반으로 진입 가능 여부를 확인합니다.
    /// RouteState가 Wait 또는 Overheat인 경우 진입을 제한합니다.
    fn can_enter(&self, ticker: &str) -> bool {
        let Some(ctx) = self.context.as_ref() else {
            // Context가 없으면 기본적으로 진입 허용
            return true;
        };

        let Ok(ctx_lock) = ctx.try_read() else {
            return true;
        };

        // RouteState 확인 - Overheat/Wait 시 진입 제한
        if let Some(state) = ctx_lock.get_route_state(ticker) {
            match state {
                RouteState::Overheat | RouteState::Wait => {
                    debug!(route_state = ?state, ticker = ticker, "RouteState not favorable for entry");
                    return false;
                }
                RouteState::Attack | RouteState::Armed | RouteState::Neutral => {
                    // 진입 가능
                }
            }
        }

        // GlobalScore 확인
        if let Some(config) = self.config.as_ref() {
            if let Some(score) = ctx_lock.get_global_score(ticker) {
                if score.overall_score < config.min_global_score {
                    debug!(
                        score = %score.overall_score,
                        min = %config.min_global_score,
                        ticker = ticker,
                        "GlobalScore too low, skipping"
                    );
                    return false;
                }
            }
        }

        true
    }

    /// 클래스별 평균 모멘텀 계산.
    fn get_class_momentum(&self, class: DualAssetClass) -> Decimal {
        let assets: Vec<_> = self
            .asset_data
            .values()
            .filter(|a| a.asset_class == class)
            .collect();

        if assets.is_empty() {
            return Decimal::ZERO;
        }

        let sum: Decimal = assets.iter().map(|a| a.momentum).sum();
        sum / Decimal::from(assets.len())
    }

    /// 클래스 내 최고 모멘텀 자산 선택.
    fn select_best_in_class(&self, class: DualAssetClass) -> Option<String> {
        self.asset_data
            .values()
            .filter(|a| a.asset_class == class)
            .max_by(|a, b| a.momentum.cmp(&b.momentum))
            .map(|a| a.symbol.clone())
    }

    /// 상대 모멘텀으로 자산 클래스 선택.
    fn determine_asset_class(&mut self) {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return,
        };

        let kr_momentum = self.get_class_momentum(DualAssetClass::Stock);
        let us_momentum = self.get_class_momentum(DualAssetClass::UsBond);

        debug!(
            kr_momentum = %kr_momentum,
            us_momentum = %us_momentum,
            "모멘텀 비교"
        );

        // 상대 모멘텀: 높은 쪽 선택
        let selected = if kr_momentum > us_momentum {
            DualAssetClass::Stock
        } else {
            DualAssetClass::UsBond
        };

        // 절대 모멘텀 체크
        if config.use_absolute_momentum {
            let selected_momentum = match selected {
                DualAssetClass::Stock => kr_momentum,
                DualAssetClass::UsBond => us_momentum,
                DualAssetClass::Safe => Decimal::ZERO,
            };

            if selected_momentum < Decimal::ZERO {
                self.selected_class = Some(DualAssetClass::Safe);
                info!("절대 모멘텀 음수 → 안전 자산 선택");
                return;
            }
        }

        self.selected_class = Some(selected);
        info!(selected = ?selected, "자산 클래스 선택");
    }

    /// 목표 배분 계산.
    fn calculate_target_allocations(&self) -> Vec<TargetAllocation> {
        let _config = match self.config.as_ref() {
            Some(c) => c,
            None => return Vec::new(),
        };

        let selected_class = match self.selected_class {
            Some(c) => c,
            None => return Vec::new(),
        };

        let mut allocations = Vec::new();

        // 선택된 클래스 내 최고 모멘텀 자산 선택
        if let Some(symbol) = self.select_best_in_class(selected_class) {
            allocations.push(TargetAllocation::new(symbol, dec!(1.0)));
        }

        allocations
    }

    /// 리밸런싱 필요 여부.
    fn should_rebalance(&self, timestamp: DateTime<Utc>) -> bool {
        let current_month = timestamp.month();

        match self.last_rebalance_month {
            Some(last) => current_month != last,
            None => true,
        }
    }

    /// 리밸런싱 시그널 생성.
    fn generate_rebalance_signals(&mut self, timestamp: DateTime<Utc>) -> Vec<Signal> {
        let config = match self.config.as_ref() {
            Some(c) => c.clone(),
            None => return Vec::new(),
        };

        // 모든 자산 모멘텀 계산
        for data in self.asset_data.values_mut() {
            data.calculate_momentum(config.momentum_period);
        }

        // 자산 클래스 결정
        self.determine_asset_class();

        // 목표 배분 계산
        let target_allocations = self.calculate_target_allocations();

        if target_allocations.is_empty() {
            return Vec::new();
        }

        // 현재 포지션
        let mut current_positions: Vec<PortfolioPosition> = self
            .asset_data
            .values()
            .filter(|d| d.current_holdings > Decimal::ZERO)
            .map(|d| PortfolioPosition::new(&d.symbol, d.current_holdings, d.current_price))
            .collect();

        // 현금 포지션 추가 (기본적으로 USD 사용)
        let invested: Decimal = current_positions.iter().map(|p| p.market_value).sum();
        let cash_available = config.total_amount - invested;
        if cash_available > Decimal::ZERO {
            current_positions.push(PortfolioPosition::cash(cash_available, "USD"));
        }

        // 리밸런싱 계산 (주로 US 자산이므로 US 설정 사용)
        let rebalance_config = RebalanceConfig::us_market();
        let calculator = RebalanceCalculator::new(rebalance_config);
        let result = calculator.calculate_orders(&current_positions, &target_allocations);

        // 시그널 생성
        let mut signals = Vec::new();

        for order in result.orders {
            // 자산에 맞는 통화 결정
            let quote = self
                .asset_data
                .get(&order.symbol)
                .map(|a| if a.market == "KR" { "KRW" } else { "USD" })
                .unwrap_or("USD");

            let symbol = Symbol::stock(&order.symbol, quote);

            let side = match order.side {
                RebalanceOrderSide::Buy => Side::Buy,
                RebalanceOrderSide::Sell => Side::Sell,
            };

            // 가격 계산
            let price = if order.quantity > Decimal::ZERO {
                order.amount / order.quantity
            } else {
                Decimal::ZERO
            };

            let signal = if order.side == RebalanceOrderSide::Buy {
                // BUY 신호 전에 RouteState/GlobalScore 확인
                if !self.can_enter(&order.symbol) {
                    debug!(symbol = %order.symbol, "Skipping BUY due to RouteState/GlobalScore");
                    continue;
                }
                Signal::entry("dual_momentum", symbol, side)
                    .with_strength(0.5)
                    .with_prices(Some(price), None, None)
                    .with_metadata("reason", json!("rebalance"))
            } else {
                Signal::exit("dual_momentum", symbol, side)
                    .with_strength(0.5)
                    .with_prices(Some(price), None, None)
                    .with_metadata("reason", json!("rebalance"))
            };

            signals.push(signal);
        }

        self.last_rebalance_month = Some(timestamp.month());

        info!(
            selected = ?self.selected_class,
            signals = signals.len(),
            "듀얼 모멘텀 리밸런싱"
        );

        signals
    }
}

impl Default for DualMomentumStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for DualMomentumStrategy {
    fn name(&self) -> &str {
        "Dual Momentum"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "한국 주식 + 미국 채권 듀얼 모멘텀 전략. 상대 모멘텀으로 자산 클래스를 선택하고, \
         절대 모멘텀으로 안전 자산 전환 여부를 결정합니다."
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let dm_config: DualMomentumConfig = serde_json::from_value(config)?;

        info!(
            momentum_period = dm_config.momentum_period,
            kr_allocation = %dm_config.kr_allocation,
            use_absolute = dm_config.use_absolute_momentum,
            "듀얼 모멘텀 전략 초기화"
        );

        for asset in dm_config.get_all_assets() {
            let quote = if asset.market == "KR" { "KRW" } else { "USD" };
            let symbol = Symbol::stock(&asset.symbol, quote);
            self.symbols.push(symbol);

            self.asset_data.insert(
                asset.symbol.clone(),
                AssetData::new(asset.symbol, asset.asset_class, asset.market.clone()),
            );
        }

        self.config = Some(dm_config);
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

        // base 심볼만 추출 (SPY/USD -> SPY)
        let symbol_str = data.symbol.base.clone();

        if !self.asset_data.contains_key(&symbol_str) {
            return Ok(vec![]);
        }

        let (close, timestamp) = match &data.data {
            MarketDataType::Kline(kline) => (kline.close, kline.open_time),
            _ => return Ok(vec![]),
        };

        if let Some(asset) = self.asset_data.get_mut(&symbol_str) {
            asset.add_price(close);
        }

        if self.should_rebalance(timestamp) {
            let all_have_data = self.asset_data.values().all(|a| !a.prices.is_empty());

            if all_have_data {
                return Ok(self.generate_rebalance_signals(timestamp));
            }
        }

        Ok(vec![])
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // base 심볼만 추출 (SPY/USD -> SPY)
        let symbol_str = order.symbol.base.clone();

        if let Some(asset) = self.asset_data.get_mut(&symbol_str) {
            match order.side {
                Side::Buy => asset.current_holdings += order.quantity,
                Side::Sell => {
                    asset.current_holdings -= order.quantity;
                    if asset.current_holdings < Decimal::ZERO {
                        asset.current_holdings = Decimal::ZERO;
                    }
                }
            }
            self.trades_count += 1;
        }

        Ok(())
    }

    async fn on_position_update(
        &mut self,
        _position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            trades = self.trades_count,
            total_pnl = %self.total_pnl,
            "듀얼 모멘텀 전략 종료"
        );
        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "initialized": self.initialized,
            "selected_class": format!("{:?}", self.selected_class),
            "asset_count": self.asset_data.len(),
            "trades_count": self.trades_count,
            "last_rebalance_month": self.last_rebalance_month,
        })
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("StrategyContext injected into Dual Momentum strategy");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dual_momentum_initialization() {
        let mut strategy = DualMomentumStrategy::new();

        let config = json!({
            "momentum_period": 63,
            "use_absolute_momentum": true
        });

        strategy.initialize(config).await.unwrap();
        assert!(strategy.initialized);
        assert!(!strategy.asset_data.is_empty());
    }

    #[test]
    fn test_default_assets() {
        let config = DualMomentumConfig::default();

        let kr = config.get_kr_stocks();
        assert_eq!(kr.len(), 2);

        let us = config.get_us_bonds();
        assert_eq!(us.len(), 3);
    }
}

// 전략 레지스트리에 자동 등록
use crate::register_strategy;

register_strategy! {
    id: "dual_momentum",
    aliases: [],
    name: "듀얼 모멘텀",
    description: "절대 모멘텀과 상대 모멘텀을 결합한 전략입니다.",
    timeframe: "1d",
    symbols: ["069500", "122630", "IEF", "TLT"],
    category: Daily,
    markets: [Stock, Stock],
    type: DualMomentumStrategy
}
