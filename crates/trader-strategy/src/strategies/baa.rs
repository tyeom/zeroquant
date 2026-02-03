//! BAA (Bold Asset Allocation) 전략 구현.
//!
//! 공격적인 듀얼 모멘텀 기반 자산배분 전략입니다.
//! 카나리아 자산의 절대 모멘텀으로 시장 상태를 판단하고,
//! 공격/방어 자산 중 모멘텀이 가장 높은 자산에 투자합니다.
//!
//! Python 7번 전략 변환.
//!
//! # 전략 개요
//!
//! ## 자산 분류
//! - **카나리아**: SPY, VEA, VWO, BND (4개 중 하나라도 모멘텀 < 0이면 방어 모드)
//! - **공격 자산**: QQQ, IWM, VWO, VEA (미국/해외 주식)
//! - **방어 자산**: TIP, DBC, BIL, IEF, TLT (채권/원자재)
//!
//! ## 13612W 모멘텀 계산
//! ```text
//! MomentumScore = 12×(1M수익률) + 4×(3M수익률) + 2×(6M수익률) + 1×(12M수익률)
//! ```
//!
//! ## 자산 선택 로직 (Bold 버전)
//! 1. 카나리아 4개 중 하나라도 모멘텀 < 0 → 방어 모드
//! 2. 공격 모드: 공격 자산 중 모멘텀 최상위 1개 선택
//! 3. 방어 모드: 방어 자산 중 모멘텀 최상위 1개 선택
//!
//! ## 자산 선택 로직 (Defensive 버전)
//! 1. 카나리아 4개 모두 모멘텀 >= 0 → 공격 모드
//! 2. 카나리아 1~3개 모멘텀 < 0 → 50% 공격 + 50% 방어
//! 3. 카나리아 4개 모두 모멘텀 < 0 → 방어 모드
//!
//! ## 리밸런싱
//! 월간 리밸런싱 (매월 초)

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

/// BAA 버전 타입.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BaaVersion {
    /// Bold 버전 - 카나리아 1개라도 음수면 방어
    #[default]
    Bold,
    /// Defensive 버전 - 카나리아 상태에 따라 비중 조절
    Defensive,
}

/// 자산 타입.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BaaAssetType {
    /// 카나리아 자산 (시장 상태 감지용)
    Canary,
    /// 공격 자산 (주식)
    Offensive,
    /// 방어 자산 (채권/원자재)
    Defensive,
}

/// 자산 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaaAsset {
    pub symbol: String,
    pub asset_type: BaaAssetType,
    pub description: String,
}

impl BaaAsset {
    pub fn new(
        symbol: impl Into<String>,
        asset_type: BaaAssetType,
        desc: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            asset_type,
            description: desc.into(),
        }
    }

    pub fn canary(symbol: impl Into<String>, desc: impl Into<String>) -> Self {
        Self::new(symbol, BaaAssetType::Canary, desc)
    }

    pub fn offensive(symbol: impl Into<String>, desc: impl Into<String>) -> Self {
        Self::new(symbol, BaaAssetType::Offensive, desc)
    }

    pub fn defensive(symbol: impl Into<String>, desc: impl Into<String>) -> Self {
        Self::new(symbol, BaaAssetType::Defensive, desc)
    }
}

/// BAA 전략 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaaConfig {
    /// BAA 버전 (Bold/Defensive)
    #[serde(default)]
    pub version: BaaVersion,

    /// 총 투자 금액
    #[serde(default = "default_total_amount")]
    pub total_amount: Decimal,

    /// 리밸런싱 허용 오차 (%)
    #[serde(default = "default_rebalance_threshold")]
    pub rebalance_threshold: Decimal,

    /// 모멘텀 기간 (개월)
    #[serde(default = "default_momentum_periods")]
    pub momentum_periods: Vec<usize>,

    /// 모멘텀 가중치 (13612W)
    #[serde(default = "default_momentum_weights")]
    pub momentum_weights: Vec<f64>,

    /// 커스텀 카나리아 자산
    pub canary_assets: Option<Vec<BaaAsset>>,

    /// 커스텀 공격 자산
    pub offensive_assets: Option<Vec<BaaAsset>>,

    /// 커스텀 방어 자산
    pub defensive_assets: Option<Vec<BaaAsset>>,

    /// 최소 GlobalScore (기본값: 60)
    #[serde(default = "default_min_global_score")]
    pub min_global_score: Decimal,
}

fn default_total_amount() -> Decimal {
    dec!(10000000)
}
fn default_rebalance_threshold() -> Decimal {
    dec!(5)
}
fn default_momentum_periods() -> Vec<usize> {
    vec![21, 63, 126, 252]
} // 1, 3, 6, 12개월
fn default_momentum_weights() -> Vec<f64> {
    vec![12.0, 4.0, 2.0, 1.0]
} // 13612W
fn default_min_global_score() -> Decimal {
    dec!(60)
}

impl Default for BaaConfig {
    fn default() -> Self {
        Self {
            version: BaaVersion::Bold,
            total_amount: default_total_amount(),
            rebalance_threshold: default_rebalance_threshold(),
            momentum_periods: default_momentum_periods(),
            momentum_weights: default_momentum_weights(),
            canary_assets: None,
            offensive_assets: None,
            defensive_assets: None,
            min_global_score: default_min_global_score(),
        }
    }
}

impl BaaConfig {
    /// 기본 카나리아 자산 목록.
    pub fn default_canary_assets() -> Vec<BaaAsset> {
        vec![
            BaaAsset::canary("SPY", "S&P 500"),
            BaaAsset::canary("VEA", "선진국 주식"),
            BaaAsset::canary("VWO", "신흥국 주식"),
            BaaAsset::canary("BND", "미국 채권"),
        ]
    }

    /// 기본 공격 자산 목록.
    pub fn default_offensive_assets() -> Vec<BaaAsset> {
        vec![
            BaaAsset::offensive("QQQ", "나스닥 100"),
            BaaAsset::offensive("IWM", "러셀 2000"),
            BaaAsset::offensive("VWO", "신흥국 주식"),
            BaaAsset::offensive("VEA", "선진국 주식"),
        ]
    }

    /// 기본 방어 자산 목록.
    pub fn default_defensive_assets() -> Vec<BaaAsset> {
        vec![
            BaaAsset::defensive("TIP", "TIPS"),
            BaaAsset::defensive("DBC", "원자재"),
            BaaAsset::defensive("BIL", "단기 국채"),
            BaaAsset::defensive("IEF", "중기 국채"),
            BaaAsset::defensive("TLT", "장기 국채"),
        ]
    }

    /// 카나리아 자산 반환.
    pub fn get_canary_assets(&self) -> Vec<BaaAsset> {
        self.canary_assets
            .clone()
            .unwrap_or_else(Self::default_canary_assets)
    }

    /// 공격 자산 반환.
    pub fn get_offensive_assets(&self) -> Vec<BaaAsset> {
        self.offensive_assets
            .clone()
            .unwrap_or_else(Self::default_offensive_assets)
    }

    /// 방어 자산 반환.
    pub fn get_defensive_assets(&self) -> Vec<BaaAsset> {
        self.defensive_assets
            .clone()
            .unwrap_or_else(Self::default_defensive_assets)
    }

    /// 모든 자산 반환.
    pub fn get_all_assets(&self) -> Vec<BaaAsset> {
        let mut all = self.get_canary_assets();
        all.extend(self.get_offensive_assets());
        all.extend(self.get_defensive_assets());
        all
    }
}

/// 자산별 모멘텀 데이터.
#[derive(Debug, Clone)]
struct AssetMomentum {
    symbol: String,
    asset_type: BaaAssetType,
    current_price: Decimal,
    prices: Vec<Decimal>,
    momentum_score: Decimal,
    current_holdings: Decimal,
}

impl AssetMomentum {
    fn new(symbol: String, asset_type: BaaAssetType) -> Self {
        Self {
            symbol,
            asset_type,
            current_price: Decimal::ZERO,
            prices: Vec::new(),
            momentum_score: Decimal::ZERO,
            current_holdings: Decimal::ZERO,
        }
    }

    /// 가격 추가.
    fn add_price(&mut self, price: Decimal) {
        self.current_price = price;
        self.prices.push(price);
        // 최대 300일치 보관 (12개월 + 여유)
        if self.prices.len() > 300 {
            self.prices.remove(0);
        }
    }

    /// 13612W 모멘텀 스코어 계산.
    fn calculate_momentum(&mut self, periods: &[usize], weights: &[f64]) {
        if self.prices.is_empty() {
            return;
        }

        let mut total_score = Decimal::ZERO;
        let mut total_weight = Decimal::ZERO;

        for (i, &period) in periods.iter().enumerate() {
            if self.prices.len() > period {
                let current = self.prices[self.prices.len() - 1];
                let past = self.prices[self.prices.len() - 1 - period];

                if past > Decimal::ZERO {
                    let returns = (current - past) / past;
                    let weight = Decimal::from_f64_retain(weights.get(i).copied().unwrap_or(1.0))
                        .unwrap_or(dec!(1));
                    total_score += returns * weight;
                    total_weight += weight;
                }
            }
        }

        if total_weight > Decimal::ZERO {
            self.momentum_score = total_score;
        }
    }

    /// 모멘텀이 양수인지 확인.
    fn is_positive_momentum(&self) -> bool {
        self.momentum_score > Decimal::ZERO
    }
}

/// BAA 전략.
pub struct BaaStrategy {
    config: Option<BaaConfig>,
    symbols: Vec<Symbol>,
    asset_data: HashMap<String, AssetMomentum>,

    /// 현재 모드 (공격/방어/혼합)
    mode: BaaMode,

    /// 마지막 리밸런싱 날짜
    last_rebalance_month: Option<u32>,

    /// 통계
    trades_count: u32,
    total_pnl: Decimal,

    /// StrategyContext (엔진에서 주입)
    context: Option<Arc<RwLock<StrategyContext>>>,

    initialized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BaaMode {
    /// 공격 모드 (100% 공격 자산)
    Offensive,
    /// 방어 모드 (100% 방어 자산)
    Defensive,
    /// 혼합 모드 (50% 공격 + 50% 방어, Defensive 버전용)
    Mixed,
}

impl BaaStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            symbols: Vec::new(),
            asset_data: HashMap::new(),
            mode: BaaMode::Defensive,
            last_rebalance_month: None,
            trades_count: 0,
            total_pnl: Decimal::ZERO,
            context: None,
            initialized: false,
        }
    }

    /// StrategyContext를 통해 진입 가능 여부 확인.
    ///
    /// RouteState와 GlobalScore를 확인하여 진입 적합성을 판단합니다.
    fn can_enter(&self, ticker: &str) -> bool {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return true, // 설정 없으면 기본 허용
        };

        let ctx = match self.context.as_ref() {
            Some(c) => c,
            None => return true, // 컨텍스트 없으면 기본 허용
        };

        let ctx_lock = match ctx.try_read() {
            Ok(lock) => lock,
            Err(_) => return true, // 락 실패 시 기본 허용
        };

        // 1. RouteState 확인 - Overheat/Neutral 시 진입 제한
        if let Some(state) = ctx_lock.get_route_state(ticker) {
            match state {
                RouteState::Overheat | RouteState::Neutral => {
                    debug!(
                        ticker = ticker,
                        route_state = ?state,
                        "RouteState not favorable for entry"
                    );
                    return false;
                }
                RouteState::Attack | RouteState::Armed | RouteState::Wait => {
                    // 진입 가능
                }
            }
        }

        // 2. GlobalScore 확인 - 저품질 종목 제외
        if let Some(score) = ctx_lock.get_global_score(ticker) {
            if score.overall_score < config.min_global_score {
                debug!(
                    ticker = ticker,
                    score = %score.overall_score,
                    min_required = %config.min_global_score,
                    "GlobalScore too low, skipping"
                );
                return false;
            }
        }

        true
    }

    /// 카나리아 자산 체크.
    fn check_canary(&self) -> (usize, usize) {
        let canary_assets: Vec<_> = self
            .asset_data
            .values()
            .filter(|a| a.asset_type == BaaAssetType::Canary)
            .collect();

        let total = canary_assets.len();
        let negative = canary_assets
            .iter()
            .filter(|a| !a.is_positive_momentum())
            .count();

        (negative, total)
    }

    /// 모드 결정.
    fn determine_mode(&mut self) {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return,
        };

        let (negative, total) = self.check_canary();

        self.mode = match config.version {
            BaaVersion::Bold => {
                // Bold: 하나라도 음수면 방어
                if negative > 0 {
                    BaaMode::Defensive
                } else {
                    BaaMode::Offensive
                }
            }
            BaaVersion::Defensive => {
                // Defensive: 상태에 따라 조절
                if negative == 0 {
                    BaaMode::Offensive
                } else if negative == total {
                    BaaMode::Defensive
                } else {
                    BaaMode::Mixed
                }
            }
        };

        debug!(
            negative_canary = negative,
            total_canary = total,
            mode = ?self.mode,
            "시장 상태 판단"
        );
    }

    /// 모멘텀 상위 자산 선택.
    fn select_top_asset(&self, asset_type: BaaAssetType) -> Option<String> {
        self.asset_data
            .values()
            .filter(|a| a.asset_type == asset_type && a.is_positive_momentum())
            .max_by(|a, b| a.momentum_score.cmp(&b.momentum_score))
            .map(|a| a.symbol.clone())
    }

    /// 목표 배분 계산.
    fn calculate_target_allocations(&self) -> Vec<TargetAllocation> {
        let _config = match self.config.as_ref() {
            Some(c) => c,
            None => return Vec::new(),
        };

        let mut allocations = Vec::new();

        match self.mode {
            BaaMode::Offensive => {
                // 공격 자산 중 최상위 1개에 100%
                if let Some(symbol) = self.select_top_asset(BaaAssetType::Offensive) {
                    allocations.push(TargetAllocation::new(symbol.clone(), dec!(1.0)));
                    info!(symbol = %symbol, "공격 모드: 공격 자산 선택");
                }
            }
            BaaMode::Defensive => {
                // 방어 자산 중 최상위 1개에 100%
                if let Some(symbol) = self.select_top_asset(BaaAssetType::Defensive) {
                    allocations.push(TargetAllocation::new(symbol.clone(), dec!(1.0)));
                    info!(symbol = %symbol, "방어 모드: 방어 자산 선택");
                }
            }
            BaaMode::Mixed => {
                // 50% 공격 + 50% 방어
                if let Some(off_symbol) = self.select_top_asset(BaaAssetType::Offensive) {
                    allocations.push(TargetAllocation::new(off_symbol.clone(), dec!(0.5)));
                    info!(symbol = %off_symbol, weight = 50, "혼합 모드: 공격 자산");
                }

                if let Some(def_symbol) = self.select_top_asset(BaaAssetType::Defensive) {
                    allocations.push(TargetAllocation::new(def_symbol.clone(), dec!(0.5)));
                    info!(symbol = %def_symbol, weight = 50, "혼합 모드: 방어 자산");
                }
            }
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
            data.calculate_momentum(&config.momentum_periods, &config.momentum_weights);
        }

        // 모드 결정
        self.determine_mode();

        // 목표 배분 계산
        let target_allocations = self.calculate_target_allocations();

        // 현재 포지션 구성
        let mut current_positions: Vec<PortfolioPosition> = self
            .asset_data
            .values()
            .filter(|d| d.current_holdings > Decimal::ZERO)
            .map(|d| PortfolioPosition::new(&d.symbol, d.current_holdings, d.current_price))
            .collect();

        // 현금 포지션 추가 (리밸런싱 시 현금 사용)
        let invested: Decimal = current_positions.iter().map(|p| p.market_value).sum();
        let cash_available = config.total_amount - invested;
        if cash_available > Decimal::ZERO {
            current_positions.push(PortfolioPosition::cash(cash_available, "USD"));
        }

        // 리밸런싱 계산
        let rebalance_config = RebalanceConfig::us_market();
        let calculator = RebalanceCalculator::new(rebalance_config);
        let result = calculator.calculate_orders(&current_positions, &target_allocations);

        // 시그널 생성
        let mut signals = Vec::new();
        let quote_currency = "USD";

        for order in result.orders {
            let symbol = Symbol::stock(&order.symbol, quote_currency);

            let side = match order.side {
                RebalanceOrderSide::Buy => Side::Buy,
                RebalanceOrderSide::Sell => Side::Sell,
            };

            // 가격 계산 (amount / quantity)
            let price = if order.quantity > Decimal::ZERO {
                order.amount / order.quantity
            } else {
                Decimal::ZERO
            };

            // BUY 신호 시 can_enter() 체크
            if order.side == RebalanceOrderSide::Buy && !self.can_enter(&order.symbol) {
                debug!(
                    symbol = %order.symbol,
                    "진입 조건 미충족, BUY 신호 스킵"
                );
                continue;
            }

            let signal = if order.side == RebalanceOrderSide::Buy {
                Signal::entry("baa", symbol, side)
                    .with_strength(0.5)
                    .with_prices(Some(price), None, None)
                    .with_metadata("reason", json!("rebalance"))
            } else {
                Signal::exit("baa", symbol, side)
                    .with_strength(0.5)
                    .with_prices(Some(price), None, None)
                    .with_metadata("reason", json!("rebalance"))
            };

            signals.push(signal);
        }

        self.last_rebalance_month = Some(timestamp.month());

        info!(
            mode = ?self.mode,
            signals = signals.len(),
            "리밸런싱 시그널 생성"
        );

        signals
    }
}

impl Default for BaaStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for BaaStrategy {
    fn name(&self) -> &str {
        "BAA"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "Bold Asset Allocation 전략. 카나리아 자산의 모멘텀으로 시장 상태를 판단하고, \
         13612W 가중 모멘텀으로 최상위 자산에 투자합니다."
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let baa_config: BaaConfig = serde_json::from_value(config)?;

        info!(
            version = ?baa_config.version,
            momentum_periods = ?baa_config.momentum_periods,
            "BAA 전략 초기화"
        );

        // 모든 자산에 대해 심볼 및 데이터 생성
        for asset in baa_config.get_all_assets() {
            let symbol = Symbol::stock(&asset.symbol, "USD");
            self.symbols.push(symbol);

            self.asset_data.insert(
                asset.symbol.clone(),
                AssetMomentum::new(asset.symbol, asset.asset_type),
            );
        }

        self.config = Some(baa_config);
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

        // 등록된 자산인지 확인
        let asset_exists = self.asset_data.contains_key(&symbol_str);
        if !asset_exists {
            return Ok(vec![]);
        }

        // Kline 데이터에서 종가 추출
        let (close, timestamp) = match &data.data {
            MarketDataType::Kline(kline) => (kline.close, kline.open_time),
            _ => return Ok(vec![]),
        };

        // 가격 업데이트
        if let Some(asset) = self.asset_data.get_mut(&symbol_str) {
            asset.add_price(close);
        }

        // 리밸런싱 체크
        if self.should_rebalance(timestamp) {
            // 모든 자산의 가격이 있는지 확인
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
        let symbol_str = order.symbol.to_string();

        if let Some(asset) = self.asset_data.get_mut(&symbol_str) {
            match order.side {
                Side::Buy => {
                    asset.current_holdings += order.quantity;
                }
                Side::Sell => {
                    asset.current_holdings -= order.quantity;
                    if asset.current_holdings < Decimal::ZERO {
                        asset.current_holdings = Decimal::ZERO;
                    }
                }
            }
            self.trades_count += 1;
        }

        debug!(
            symbol = %order.symbol,
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
        info!(
            trades = self.trades_count,
            total_pnl = %self.total_pnl,
            "BAA 전략 종료"
        );
        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "initialized": self.initialized,
            "mode": format!("{:?}", self.mode),
            "asset_count": self.asset_data.len(),
            "trades_count": self.trades_count,
            "last_rebalance_month": self.last_rebalance_month,
        })
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("StrategyContext injected into BAA strategy");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_baa_initialization() {
        let mut strategy = BaaStrategy::new();

        let config = json!({
            "version": "Bold",
            "total_amount": 10000000
        });

        strategy.initialize(config).await.unwrap();
        assert!(strategy.initialized);
        assert!(!strategy.asset_data.is_empty());
    }

    #[test]
    fn test_default_assets() {
        let config = BaaConfig::default();

        let canary = config.get_canary_assets();
        assert_eq!(canary.len(), 4);

        let offensive = config.get_offensive_assets();
        assert_eq!(offensive.len(), 4);

        let defensive = config.get_defensive_assets();
        assert_eq!(defensive.len(), 5);
    }

    #[test]
    fn test_momentum_calculation() {
        let mut asset = AssetMomentum::new("SPY".to_string(), BaaAssetType::Canary);

        // 252일치 가격 데이터 추가 (상승 추세)
        for i in 0..260 {
            asset.add_price(Decimal::from(100 + i));
        }

        let periods = vec![21, 63, 126, 252];
        let weights = vec![12.0, 4.0, 2.0, 1.0];

        asset.calculate_momentum(&periods, &weights);

        // 상승 추세이므로 모멘텀이 양수여야 함
        assert!(asset.is_positive_momentum());
    }
}

// 전략 레지스트리에 자동 등록
use crate::register_strategy;

register_strategy! {
    id: "baa",
    aliases: [],
    name: "BAA",
    description: "Bold Asset Allocation 공격적 자산배분 전략입니다.",
    timeframe: "1d",
    symbols: ["SPY", "VEA", "VWO", "BND", "QQQ", "IWM", "TIP", "DBC", "BIL", "IEF", "TLT"],
    category: Monthly,
    markets: [Stock],
    type: BaaStrategy
}
