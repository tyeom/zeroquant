//! 섹터 모멘텀 전략 (Sector Momentum)
//!
//! 섹터/업종 ETF의 모멘텀을 분석하여 상위 섹터에 투자하는 전략.
//! US/KR 시장 모두 지원합니다.
//!
//! Python 5번 전략 변환.
//!
//! # 전략 개요
//!
//! ## US 섹터 (11개)
//! - XLK (기술), XLF (금융), XLV (헬스케어), XLY (경기소비재)
//! - XLP (필수소비재), XLE (에너지), XLI (산업재), XLB (소재)
//! - XLU (유틸리티), XLRE (부동산), XLC (통신)
//!
//! ## KR 섹터 (10개)
//! - 091160 (반도체), 091180 (자동차), 091170 (은행)
//! - 102970 (철강), 102960 (기계장비), 117460 (건설)
//! - 305720 (2차전지), 091220 (TIGER 은행), 091230 (TIGER 반도체)
//! - 305540 (TIGER 2차전지)
//!
//! ## 모멘텀 계산
//! - 단기 모멘텀: 20일 수익률
//! - 중기 모멘텀: 60일 수익률
//! - 장기 모멘텀: 120일 수익률
//! - 종합 스코어: (단기 × 0.5) + (중기 × 0.3) + (장기 × 0.2)
//!
//! ## 자산 선택
//! 1. 모멘텀 스코어 상위 N개 섹터 선택 (기본 3개)
//! 2. 동일 비중 또는 모멘텀 비중으로 배분
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

/// 시장 타입.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SectorMomentumMarket {
    /// 미국 시장
    #[default]
    US,
    /// 한국 시장
    KR,
}

/// 비중 배분 방식.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WeightingMethod {
    /// 동일 비중
    #[default]
    Equal,
    /// 모멘텀 비례 비중
    Momentum,
}

/// 섹터 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorInfo {
    pub symbol: String,
    pub name: String,
}

impl SectorInfo {
    pub fn new(symbol: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            name: name.into(),
        }
    }
}

/// 섹터 모멘텀 전략 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorMomentumConfig {
    /// 시장 타입
    #[serde(default)]
    pub market: SectorMomentumMarket,

    /// 총 투자 금액
    #[serde(default = "default_total_amount")]
    pub total_amount: Decimal,

    /// 선택할 상위 섹터 수
    #[serde(default = "default_top_n")]
    pub top_n: usize,

    /// 비중 배분 방식
    #[serde(default)]
    pub weighting_method: WeightingMethod,

    /// 단기 모멘텀 기간 (일)
    #[serde(default = "default_short_period")]
    pub short_period: usize,

    /// 중기 모멘텀 기간 (일)
    #[serde(default = "default_medium_period")]
    pub medium_period: usize,

    /// 장기 모멘텀 기간 (일)
    #[serde(default = "default_long_period")]
    pub long_period: usize,

    /// 단기 모멘텀 가중치
    #[serde(default = "default_short_weight")]
    pub short_weight: f64,

    /// 중기 모멘텀 가중치
    #[serde(default = "default_medium_weight")]
    pub medium_weight: f64,

    /// 장기 모멘텀 가중치
    #[serde(default = "default_long_weight")]
    pub long_weight: f64,

    /// 리밸런싱 허용 오차 (%)
    #[serde(default = "default_rebalance_threshold")]
    pub rebalance_threshold: Decimal,

    /// 커스텀 섹터 목록
    pub custom_sectors: Option<Vec<SectorInfo>>,

    /// 최소 GlobalScore (기본값: 60)
    #[serde(default = "default_min_global_score")]
    pub min_global_score: Decimal,
}

fn default_total_amount() -> Decimal {
    dec!(10000000)
}
fn default_top_n() -> usize {
    3
}
fn default_short_period() -> usize {
    20
}
fn default_medium_period() -> usize {
    60
}
fn default_long_period() -> usize {
    120
}
fn default_short_weight() -> f64 {
    0.5
}
fn default_medium_weight() -> f64 {
    0.3
}
fn default_long_weight() -> f64 {
    0.2
}
fn default_rebalance_threshold() -> Decimal {
    dec!(5)
}
fn default_min_global_score() -> Decimal {
    dec!(60)
}

impl Default for SectorMomentumConfig {
    fn default() -> Self {
        Self {
            market: SectorMomentumMarket::US,
            total_amount: default_total_amount(),
            top_n: default_top_n(),
            weighting_method: WeightingMethod::Equal,
            short_period: default_short_period(),
            medium_period: default_medium_period(),
            long_period: default_long_period(),
            short_weight: default_short_weight(),
            medium_weight: default_medium_weight(),
            long_weight: default_long_weight(),
            rebalance_threshold: default_rebalance_threshold(),
            custom_sectors: None,
            min_global_score: default_min_global_score(),
        }
    }
}

impl SectorMomentumConfig {
    /// US 섹터 목록.
    pub fn us_sectors() -> Vec<SectorInfo> {
        vec![
            SectorInfo::new("XLK", "기술"),
            SectorInfo::new("XLF", "금융"),
            SectorInfo::new("XLV", "헬스케어"),
            SectorInfo::new("XLY", "경기소비재"),
            SectorInfo::new("XLP", "필수소비재"),
            SectorInfo::new("XLE", "에너지"),
            SectorInfo::new("XLI", "산업재"),
            SectorInfo::new("XLB", "소재"),
            SectorInfo::new("XLU", "유틸리티"),
            SectorInfo::new("XLRE", "부동산"),
            SectorInfo::new("XLC", "통신"),
        ]
    }

    /// KR 섹터 목록.
    pub fn kr_sectors() -> Vec<SectorInfo> {
        vec![
            SectorInfo::new("091160", "KODEX 반도체"),
            SectorInfo::new("091230", "TIGER 반도체"),
            SectorInfo::new("305720", "KODEX 2차전지산업"),
            SectorInfo::new("305540", "TIGER 2차전지테마"),
            SectorInfo::new("091170", "KODEX 은행"),
            SectorInfo::new("091220", "TIGER 은행"),
            SectorInfo::new("102970", "KODEX 철강"),
            SectorInfo::new("117460", "KODEX 건설"),
            SectorInfo::new("091180", "TIGER 자동차"),
            SectorInfo::new("102960", "KODEX 기계장비"),
        ]
    }

    /// 시장에 맞는 섹터 목록 반환.
    pub fn get_sectors(&self) -> Vec<SectorInfo> {
        if let Some(custom) = &self.custom_sectors {
            return custom.clone();
        }

        match self.market {
            SectorMomentumMarket::US => Self::us_sectors(),
            SectorMomentumMarket::KR => Self::kr_sectors(),
        }
    }

    /// Quote 통화 반환.
    pub fn get_quote_currency(&self) -> &str {
        match self.market {
            SectorMomentumMarket::US => "USD",
            SectorMomentumMarket::KR => "KRW",
        }
    }
}

/// 섹터별 모멘텀 데이터.
#[derive(Debug, Clone)]
struct SectorData {
    symbol: String,
    name: String,
    current_price: Decimal,
    prices: Vec<Decimal>,
    momentum_score: Decimal,
    current_holdings: Decimal,
}

impl SectorData {
    fn new(symbol: String, name: String) -> Self {
        Self {
            symbol,
            name,
            current_price: Decimal::ZERO,
            prices: Vec::new(),
            momentum_score: Decimal::ZERO,
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

    fn calculate_momentum(&mut self, config: &SectorMomentumConfig) {
        if self.prices.is_empty() {
            return;
        }

        let mut score = Decimal::ZERO;
        let len = self.prices.len();

        // 단기 모멘텀
        if len > config.short_period {
            let current = self.prices[len - 1];
            let past = self.prices[len - 1 - config.short_period];
            if past > Decimal::ZERO {
                let ret = (current - past) / past;
                score += ret * Decimal::from_f64_retain(config.short_weight).unwrap_or(dec!(0.5));
            }
        }

        // 중기 모멘텀
        if len > config.medium_period {
            let current = self.prices[len - 1];
            let past = self.prices[len - 1 - config.medium_period];
            if past > Decimal::ZERO {
                let ret = (current - past) / past;
                score += ret * Decimal::from_f64_retain(config.medium_weight).unwrap_or(dec!(0.3));
            }
        }

        // 장기 모멘텀
        if len > config.long_period {
            let current = self.prices[len - 1];
            let past = self.prices[len - 1 - config.long_period];
            if past > Decimal::ZERO {
                let ret = (current - past) / past;
                score += ret * Decimal::from_f64_retain(config.long_weight).unwrap_or(dec!(0.2));
            }
        }

        self.momentum_score = score;
    }
}

/// 섹터 모멘텀 전략.
pub struct SectorMomentumStrategy {
    config: Option<SectorMomentumConfig>,
    symbols: Vec<Symbol>,
    sector_data: HashMap<String, SectorData>,

    /// 마지막 리밸런싱 날짜
    last_rebalance_month: Option<u32>,

    /// 통계
    trades_count: u32,
    total_pnl: Decimal,

    initialized: bool,

    /// StrategyContext (RouteState, GlobalScore 접근용)
    context: Option<Arc<RwLock<StrategyContext>>>,
}

impl SectorMomentumStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            symbols: Vec::new(),
            sector_data: HashMap::new(),
            last_rebalance_month: None,
            trades_count: 0,
            total_pnl: Decimal::ZERO,
            initialized: false,
            context: None,
        }
    }

    /// StrategyContext 기반 진입 가능 여부 체크.
    ///
    /// RouteState와 GlobalScore를 확인하여 진입 가능 여부를 결정합니다.
    fn can_enter(&self, ticker: &str) -> bool {
        let Some(ctx) = self.context.as_ref() else {
            // Context 없으면 진입 허용 (기존 동작 유지)
            return true;
        };

        let Ok(ctx_lock) = ctx.try_read() else {
            return true;
        };

        // 1. RouteState 확인 - Attack/Armed만 진입 허용
        if let Some(state) = ctx_lock.get_route_state(ticker) {
            match state {
                RouteState::Attack | RouteState::Armed => {
                    // 진입 가능
                }
                RouteState::Overheat | RouteState::Wait | RouteState::Neutral => {
                    debug!(ticker = ticker, route_state = ?state, "RouteState not favorable for entry");
                    return false;
                }
            }
        }

        // 2. GlobalScore 확인
        if let Some(config) = self.config.as_ref() {
            if let Some(score) = ctx_lock.get_global_score(ticker) {
                if score.overall_score < config.min_global_score {
                    debug!(
                        ticker = ticker,
                        score = %score.overall_score,
                        min_required = %config.min_global_score,
                        "GlobalScore too low for entry"
                    );
                    return false;
                }
            }
        }

        true
    }

    /// 상위 N개 섹터 선택.
    fn select_top_sectors(&self, n: usize) -> Vec<(String, Decimal)> {
        let mut sectors: Vec<_> = self
            .sector_data
            .values()
            .filter(|s| s.momentum_score > Decimal::ZERO)
            .map(|s| (s.symbol.clone(), s.momentum_score))
            .collect();

        sectors.sort_by(|a, b| b.1.cmp(&a.1));
        sectors.truncate(n);

        sectors
    }

    /// 목표 배분 계산.
    fn calculate_target_allocations(&self) -> Vec<TargetAllocation> {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return Vec::new(),
        };

        let top_sectors = self.select_top_sectors(config.top_n);

        if top_sectors.is_empty() {
            return Vec::new();
        }

        let mut allocations = Vec::new();

        match config.weighting_method {
            WeightingMethod::Equal => {
                let weight = dec!(1) / Decimal::from(top_sectors.len());

                for (symbol, score) in top_sectors {
                    allocations.push(TargetAllocation::new(symbol.clone(), weight));
                    debug!(symbol = %symbol, score = %score, weight = %weight, "섹터 선택");
                }
            }
            WeightingMethod::Momentum => {
                let total_score: Decimal = top_sectors.iter().map(|(_, s)| *s).sum();

                for (symbol, score) in top_sectors {
                    let weight = if total_score > Decimal::ZERO {
                        score / total_score
                    } else {
                        dec!(1) / Decimal::from(config.top_n)
                    };

                    allocations.push(TargetAllocation::new(symbol.clone(), weight));
                    debug!(symbol = %symbol, score = %score, weight = %weight, "모멘텀 비중 섹터");
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

        // 모든 섹터 모멘텀 계산
        for data in self.sector_data.values_mut() {
            data.calculate_momentum(&config);
        }

        // 목표 배분 계산
        let target_allocations = self.calculate_target_allocations();

        if target_allocations.is_empty() {
            return Vec::new();
        }

        // 현재 포지션 구성
        let quote_currency = config.get_quote_currency();
        let mut current_positions: Vec<PortfolioPosition> = self
            .sector_data
            .values()
            .filter(|d| d.current_holdings > Decimal::ZERO)
            .map(|d| PortfolioPosition::new(&d.symbol, d.current_holdings, d.current_price))
            .collect();

        // 현금 포지션 추가
        let invested: Decimal = current_positions.iter().map(|p| p.market_value).sum();
        let cash_available = config.total_amount - invested;
        if cash_available > Decimal::ZERO {
            current_positions.push(PortfolioPosition::cash(cash_available, quote_currency));
        }

        // 리밸런싱 계산
        let rebalance_config = match config.market {
            SectorMomentumMarket::US => RebalanceConfig::us_market(),
            SectorMomentumMarket::KR => RebalanceConfig::korean_market(),
        };
        let calculator = RebalanceCalculator::new(rebalance_config);
        let result = calculator.calculate_orders(&current_positions, &target_allocations);

        // 시그널 생성
        let mut signals = Vec::new();

        for order in result.orders {
            let symbol = Symbol::stock(&order.symbol, quote_currency);

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

            // BUY 신호는 StrategyContext 조건 확인 (매도는 항상 허용)
            if order.side == RebalanceOrderSide::Buy && !self.can_enter(&order.symbol) {
                debug!(
                    symbol = %order.symbol,
                    "BUY 신호 스킵: StrategyContext 조건 미충족"
                );
                continue;
            }

            let signal = if order.side == RebalanceOrderSide::Buy {
                Signal::entry("sector_momentum", symbol, side)
                    .with_strength(0.5)
                    .with_prices(Some(price), None, None)
                    .with_metadata("reason", json!("rebalance"))
            } else {
                Signal::exit("sector_momentum", symbol, side)
                    .with_strength(0.5)
                    .with_prices(Some(price), None, None)
                    .with_metadata("reason", json!("rebalance"))
            };

            signals.push(signal);
        }

        self.last_rebalance_month = Some(timestamp.month());

        info!(
            top_n = config.top_n,
            signals = signals.len(),
            "섹터 모멘텀 리밸런싱"
        );

        signals
    }
}

impl Default for SectorMomentumStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for SectorMomentumStrategy {
    fn name(&self) -> &str {
        "Sector Momentum"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "섹터 모멘텀 전략. 섹터 ETF의 모멘텀을 분석하여 상위 N개 섹터에 투자합니다."
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let sm_config: SectorMomentumConfig = serde_json::from_value(config)?;

        info!(
            market = ?sm_config.market,
            top_n = sm_config.top_n,
            weighting = ?sm_config.weighting_method,
            "섹터 모멘텀 전략 초기화"
        );

        let quote = sm_config.get_quote_currency();

        for sector in sm_config.get_sectors() {
            let symbol = Symbol::stock(&sector.symbol, quote);
            self.symbols.push(symbol);

            self.sector_data.insert(
                sector.symbol.clone(),
                SectorData::new(sector.symbol, sector.name),
            );
        }

        self.config = Some(sm_config);
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

        // base 심볼만 추출 (XLK/USD -> XLK)
        let symbol_str = data.symbol.base.clone();

        if !self.sector_data.contains_key(&symbol_str) {
            return Ok(vec![]);
        }

        let (close, timestamp) = match &data.data {
            MarketDataType::Kline(kline) => (kline.close, kline.open_time),
            _ => return Ok(vec![]),
        };

        if let Some(sector) = self.sector_data.get_mut(&symbol_str) {
            sector.add_price(close);
        }

        if self.should_rebalance(timestamp) {
            let all_have_data = self.sector_data.values().all(|s| !s.prices.is_empty());

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
        // base 심볼만 추출 (XLK/USD -> XLK)
        let symbol_str = order.symbol.base.clone();

        if let Some(sector) = self.sector_data.get_mut(&symbol_str) {
            match order.side {
                Side::Buy => sector.current_holdings += order.quantity,
                Side::Sell => {
                    sector.current_holdings -= order.quantity;
                    if sector.current_holdings < Decimal::ZERO {
                        sector.current_holdings = Decimal::ZERO;
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
            "섹터 모멘텀 전략 종료"
        );
        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "initialized": self.initialized,
            "sector_count": self.sector_data.len(),
            "trades_count": self.trades_count,
            "last_rebalance_month": self.last_rebalance_month,
        })
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("StrategyContext injected into Sector Momentum strategy");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sector_momentum_initialization() {
        let mut strategy = SectorMomentumStrategy::new();

        let config = json!({
            "market": "US",
            "top_n": 3
        });

        strategy.initialize(config).await.unwrap();
        assert!(strategy.initialized);
        assert_eq!(strategy.sector_data.len(), 11); // US 섹터 11개
    }

    #[test]
    fn test_kr_sectors() {
        let sectors = SectorMomentumConfig::kr_sectors();
        assert_eq!(sectors.len(), 10);
    }

    #[test]
    fn test_us_sectors() {
        let sectors = SectorMomentumConfig::us_sectors();
        assert_eq!(sectors.len(), 11);
    }
}

// 전략 레지스트리에 자동 등록
use crate::register_strategy;

register_strategy! {
    id: "sector_momentum",
    aliases: [],
    name: "섹터 모멘텀",
    description: "섹터별 모멘텀 분석으로 상위 섹터에 투자합니다.",
    timeframe: "1d",
    symbols: ["XLK", "XLF", "XLV", "XLE", "XLI", "XLY", "XLP", "XLU", "XLB", "XLRE"],
    category: Daily,
    markets: [Stock, Stock],
    type: SectorMomentumStrategy
}
