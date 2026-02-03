//! Simple Power 전략 구현.
//!
//! TQQQ/SCHD/PFIX/TMF 기반 모멘텀 자산배분 전략.
//! Python 44/45번 전략 변환.
//!
//! # 전략 개요
//!
//! - **공격 자산**: TQQQ (나스닥 3배 레버리지) - 50%
//! - **배당 자산**: SCHD (배당 성장 ETF) - 20%
//! - **금리 헤지**: PFIX (금리 상승 헤지) - 15%
//! - **채권 레버리지**: TMF (장기채 3배) - 15%
//!
//! # 모멘텀 필터
//!
//! 각 자산에 대해 MA130 기반 모멘텀 필터 적용:
//! 1. 전일 종가 < MA130 → 비중 50% 감소
//! 2. MA130 하락 추세 → 비중 추가 50% 감소
//! 3. 두 조건 모두 충족 시 PFIX/TMF는 완전 청산
//!
//! # 대체 로직
//!
//! PFIX/TMF 중 하나만 청산되면 다른 자산에 2배 배분
//!
//! # 리밸런싱
//!
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
use tracing::{debug, info, warn};
use trader_core::domain::{RouteState, StrategyContext};

use crate::strategies::common::rebalance::{
    PortfolioPosition, RebalanceCalculator, RebalanceConfig, RebalanceOrderSide, TargetAllocation,
};
use crate::traits::Strategy;
use trader_core::{MarketData, MarketDataType, Order, Position, Side, Signal, SignalType, Symbol};

/// Simple Power 전략 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimplePowerConfig {
    /// 시장 타입 (US/KR)
    pub market: MarketType,

    /// 공격 자산 (기본: TQQQ)
    pub aggressive_asset: String,
    /// 공격 자산 기본 비중
    pub aggressive_weight: Decimal,

    /// 배당 자산 (기본: SCHD)
    pub dividend_asset: String,
    /// 배당 자산 비중
    pub dividend_weight: Decimal,

    /// 금리 헤지 자산 (기본: PFIX)
    pub rate_hedge_asset: String,
    /// 금리 헤지 비중
    pub rate_hedge_weight: Decimal,

    /// 채권 레버리지 자산 (기본: TMF)
    pub bond_leverage_asset: String,
    /// 채권 레버리지 비중
    pub bond_leverage_weight: Decimal,

    /// MA 기간 (기본: 130일)
    pub ma_period: usize,

    /// 리밸런싱 주기 (월 단위)
    pub rebalance_interval_months: u32,

    /// 투자 비율 (총 자산 대비)
    pub invest_rate: Decimal,

    /// 리밸런싱 임계값 (비중 편차)
    pub rebalance_threshold: Decimal,

    /// 최소 GlobalScore (기본값: 60)
    #[serde(default = "default_min_global_score")]
    pub min_global_score: Decimal,
}

/// 기본 최소 GlobalScore.
fn default_min_global_score() -> Decimal {
    dec!(60)
}

/// 시장 타입.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketType {
    /// 미국 시장
    US,
    /// 한국 시장
    KR,
}

impl Default for SimplePowerConfig {
    fn default() -> Self {
        Self::us_default()
    }
}

impl SimplePowerConfig {
    /// 미국 시장 기본 설정 (V2).
    pub fn us_default() -> Self {
        Self {
            market: MarketType::US,
            aggressive_asset: "TQQQ".to_string(),
            aggressive_weight: dec!(0.5),
            dividend_asset: "SCHD".to_string(),
            dividend_weight: dec!(0.2),
            rate_hedge_asset: "PFIX".to_string(),
            rate_hedge_weight: dec!(0.15),
            bond_leverage_asset: "TMF".to_string(),
            bond_leverage_weight: dec!(0.15),
            ma_period: 130,
            rebalance_interval_months: 1,
            invest_rate: dec!(1.0),
            rebalance_threshold: dec!(0.03),
            min_global_score: dec!(60),
        }
    }

    /// 한국 시장 설정 (ETF 기반).
    ///
    /// 한국에서 유사한 ETF로 대체:
    /// - TQQQ 대체: KODEX 미국나스닥100레버리지 (409820)
    /// - SCHD 대체: KODEX 미국배당프리미엄액티브 (441640) 또는 TIGER 미국S&P500배당귀족
    /// - PFIX 대체: 단기채권 ETF
    /// - TMF 대체: KODEX 미국채울트라30년선물(H) (304660)
    pub fn kr_default() -> Self {
        Self {
            market: MarketType::KR,
            aggressive_asset: "409820".to_string(), // KODEX 미국나스닥100레버리지
            aggressive_weight: dec!(0.5),
            dividend_asset: "441640".to_string(), // KODEX 미국배당프리미엄액티브
            dividend_weight: dec!(0.2),
            rate_hedge_asset: "453850".to_string(), // KODEX CD금리액티브(합성)
            rate_hedge_weight: dec!(0.15),
            bond_leverage_asset: "304660".to_string(), // KODEX 미국채울트라30년선물(H)
            bond_leverage_weight: dec!(0.15),
            ma_period: 130,
            rebalance_interval_months: 1,
            invest_rate: dec!(1.0),
            rebalance_threshold: dec!(0.03),
            min_global_score: dec!(60),
        }
    }

    /// 모든 자산 심볼 가져오기.
    pub fn all_assets(&self) -> Vec<String> {
        vec![
            self.aggressive_asset.clone(),
            self.dividend_asset.clone(),
            self.rate_hedge_asset.clone(),
            self.bond_leverage_asset.clone(),
        ]
    }

    /// 기본 비중 맵 가져오기.
    pub fn base_weights(&self) -> HashMap<String, Decimal> {
        let mut weights = HashMap::new();
        weights.insert(self.aggressive_asset.clone(), self.aggressive_weight);
        weights.insert(self.dividend_asset.clone(), self.dividend_weight);
        weights.insert(self.rate_hedge_asset.clone(), self.rate_hedge_weight);
        weights.insert(self.bond_leverage_asset.clone(), self.bond_leverage_weight);
        weights
    }
}

/// 자산별 모멘텀 상태.
#[derive(Debug, Clone)]
struct AssetMomentumState {
    /// 현재 MA130
    ma_current: Option<Decimal>,
    /// 전일 MA130
    ma_previous: Option<Decimal>,
    /// 전일 종가
    prev_close: Option<Decimal>,
    /// 모멘텀 컷 카운트 (0, 1, 2)
    cut_count: u8,
    /// 조정된 비중 배율 (1.0, 0.5, 0.25, 0.0)
    rate_multiplier: Decimal,
    /// 아웃 상태 (완전 청산)
    is_out: bool,
}

impl Default for AssetMomentumState {
    fn default() -> Self {
        Self {
            ma_current: None,
            ma_previous: None,
            prev_close: None,
            cut_count: 0,
            rate_multiplier: dec!(1.0), // 기본값은 100% 투자
            is_out: false,
        }
    }
}

/// Simple Power 전략.
pub struct SimplePowerStrategy {
    config: Option<SimplePowerConfig>,
    /// StrategyContext (RouteState, GlobalScore 조회용)
    context: Option<Arc<RwLock<StrategyContext>>>,
    /// 자산별 가격 히스토리 (최신 가격이 앞에)
    price_history: HashMap<String, Vec<Decimal>>,
    /// 자산별 모멘텀 상태
    momentum_states: HashMap<String, AssetMomentumState>,
    /// 현재 포지션
    positions: HashMap<String, Decimal>,
    /// 마지막 리밸런싱 년월 (YYYY_MM)
    last_rebalance_ym: Option<String>,
    /// 리밸런싱 계산기
    rebalance_calculator: RebalanceCalculator,
    /// 현재 현금 잔고
    cash_balance: Decimal,
}

impl SimplePowerStrategy {
    /// 새 전략 생성.
    pub fn new() -> Self {
        Self {
            config: None,
            context: None,
            price_history: HashMap::new(),
            momentum_states: HashMap::new(),
            positions: HashMap::new(),
            last_rebalance_ym: None,
            rebalance_calculator: RebalanceCalculator::new(RebalanceConfig::us_market()),
            cash_balance: Decimal::ZERO,
        }
    }

    /// 설정으로 전략 생성.
    pub fn with_config(config: SimplePowerConfig) -> Self {
        let rebalance_config = match config.market {
            MarketType::US => RebalanceConfig::us_market(),
            MarketType::KR => RebalanceConfig::korean_market(),
        };

        Self {
            config: Some(config),
            context: None,
            price_history: HashMap::new(),
            momentum_states: HashMap::new(),
            positions: HashMap::new(),
            last_rebalance_ym: None,
            rebalance_calculator: RebalanceCalculator::new(rebalance_config),
            cash_balance: Decimal::ZERO,
        }
    }

    /// 가격 히스토리 업데이트.
    fn update_price_history(&mut self, symbol: &str, price: Decimal) {
        let history = self.price_history.entry(symbol.to_string()).or_default();
        history.insert(0, price); // 최신 가격을 앞에

        // 최대 300일 보관
        if history.len() > 300 {
            history.truncate(300);
        }
    }

    /// 이동평균 계산.
    fn calculate_ma(&self, prices: &[Decimal], period: usize, offset: usize) -> Option<Decimal> {
        if prices.len() < period + offset {
            return None;
        }

        let start = offset;
        let end = start + period;
        if end > prices.len() {
            return None;
        }

        let sum: Decimal = prices[start..end].iter().sum();
        Some(sum / Decimal::from(period))
    }

    /// 모멘텀 상태 계산.
    fn calculate_momentum_state(
        &self,
        symbol: &str,
        config: &SimplePowerConfig,
    ) -> AssetMomentumState {
        let prices = match self.price_history.get(symbol) {
            Some(p) if p.len() >= config.ma_period + 3 => p,
            _ => return AssetMomentumState::default(),
        };

        // 전일 종가 (index 1, 오늘은 index 0)
        let prev_close = prices.get(1).copied();

        // 현재 MA130 (전일 기준, index 1에서 시작)
        let ma_current = self.calculate_ma(prices, config.ma_period, 1);

        // 전일 MA130 (2일 전 기준, index 2에서 시작)
        let ma_previous = self.calculate_ma(prices, config.ma_period, 2);

        let mut cut_count: u8 = 0;
        let mut rate = dec!(1.0);

        if let (Some(ma), Some(close)) = (ma_current, prev_close) {
            // 조건 1: 전일 종가 < MA130
            if ma > close {
                rate *= dec!(0.5);
                cut_count += 1;
            }
        }

        if let (Some(ma_curr), Some(ma_prev)) = (ma_current, ma_previous) {
            // 조건 2: MA130 하락 추세
            if ma_prev > ma_curr {
                rate *= dec!(0.5);
                cut_count += 1;
            }
        }

        // PFIX/TMF는 두 조건 모두 충족 시 완전 청산
        let is_hedge_asset =
            symbol == config.rate_hedge_asset || symbol == config.bond_leverage_asset;
        let is_out = is_hedge_asset && cut_count == 2;

        if is_out {
            rate = Decimal::ZERO;
        }

        AssetMomentumState {
            ma_current,
            ma_previous,
            prev_close,
            cut_count,
            rate_multiplier: rate,
            is_out,
        }
    }

    /// 조정된 목표 비중 계산.
    fn calculate_adjusted_weights(&mut self, config: &SimplePowerConfig) -> Vec<TargetAllocation> {
        // 각 자산의 모멘텀 상태 계산
        for asset in config.all_assets() {
            let state = self.calculate_momentum_state(&asset, config);
            self.momentum_states.insert(asset.clone(), state);
        }

        let base_weights = config.base_weights();
        let mut adjusted_weights: HashMap<String, Decimal> = HashMap::new();

        // 기본 비중에 모멘텀 필터 적용
        for (asset, base_weight) in &base_weights {
            let state = self.momentum_states.get(asset).cloned().unwrap_or_default();
            adjusted_weights.insert(asset.clone(), *base_weight * state.rate_multiplier);
        }

        // PFIX/TMF 대체 로직
        let pfix_state = self
            .momentum_states
            .get(&config.rate_hedge_asset)
            .cloned()
            .unwrap_or_default();
        let tmf_state = self
            .momentum_states
            .get(&config.bond_leverage_asset)
            .cloned()
            .unwrap_or_default();

        if pfix_state.is_out && !tmf_state.is_out {
            // PFIX 아웃 → TMF에 2배 배분
            if let Some(weight) = adjusted_weights.get_mut(&config.bond_leverage_asset) {
                *weight *= dec!(2.0);
                info!(
                    "PFIX 청산 → {} 비중 2배: {:.1}%",
                    config.bond_leverage_asset,
                    (*weight * dec!(100))
                );
            }
        } else if tmf_state.is_out && !pfix_state.is_out {
            // TMF 아웃 → PFIX에 2배 배분
            if let Some(weight) = adjusted_weights.get_mut(&config.rate_hedge_asset) {
                *weight *= dec!(2.0);
                info!(
                    "TMF 청산 → {} 비중 2배: {:.1}%",
                    config.rate_hedge_asset,
                    (*weight * dec!(100))
                );
            }
        }

        // 로그 출력
        for (asset, weight) in &adjusted_weights {
            let state = self.momentum_states.get(asset).cloned().unwrap_or_default();
            info!(
                "{} → 투자 비중: {:.1}% (cut_count: {}, rate: {:.2})",
                asset,
                (*weight * dec!(100)),
                state.cut_count,
                state.rate_multiplier
            );
        }

        // TargetAllocation으로 변환
        adjusted_weights
            .into_iter()
            .map(|(symbol, weight)| TargetAllocation::new(symbol, weight))
            .collect()
    }

    /// RouteState와 GlobalScore를 체크하여 특정 자산에 대한 진입 가능 여부 반환.
    ///
    /// # 진입 조건
    ///
    /// - RouteState::Attack: 적극 진입 가능
    /// - RouteState::Armed: 조건부 허용
    /// - RouteState::Overheat/Wait/Neutral: 진입 금지
    /// - GlobalScore >= min_global_score: 진입 허용
    fn can_enter(&self, ticker: &str) -> bool {
        let Some(config) = self.config.as_ref() else {
            return false;
        };

        let Some(ctx) = self.context.as_ref() else {
            // Context가 없으면 진입 허용 (하위 호환성)
            debug!("StrategyContext not available - allowing entry by default");
            return true;
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

    /// 리밸런싱 필요 여부 확인.
    fn should_rebalance(&self, current_time: DateTime<Utc>) -> bool {
        let current_ym = format!("{}_{}", current_time.year(), current_time.month());

        match &self.last_rebalance_ym {
            None => true,                            // 첫 리밸런싱
            Some(last_ym) => last_ym != &current_ym, // 달이 바뀌었으면 리밸런싱
        }
    }

    /// 리밸런싱 신호 생성.
    fn generate_rebalance_signals(
        &mut self,
        config: &SimplePowerConfig,
        current_time: DateTime<Utc>,
    ) -> Vec<Signal> {
        if !self.should_rebalance(current_time) {
            return Vec::new();
        }

        // 조정된 목표 비중 계산
        let target_allocations = self.calculate_adjusted_weights(config);

        // 현재 포지션을 PortfolioPosition으로 변환
        let mut portfolio_positions: Vec<PortfolioPosition> = Vec::new();

        for (symbol, quantity) in &self.positions {
            if let Some(prices) = self.price_history.get(symbol) {
                if let Some(current_price) = prices.first() {
                    portfolio_positions.push(PortfolioPosition::new(
                        symbol,
                        *quantity,
                        *current_price,
                    ));
                }
            }
        }

        // 현금 포지션 추가
        let cash_symbol = match config.market {
            MarketType::US => "USD",
            MarketType::KR => "KRW",
        };
        portfolio_positions.push(PortfolioPosition::cash(self.cash_balance, cash_symbol));

        // 리밸런싱 계산
        let result = self
            .rebalance_calculator
            .calculate_orders_with_cash_constraint(&portfolio_positions, &target_allocations);

        // 신호 변환
        let mut signals = Vec::new();

        for order in result.orders {
            let side = match order.side {
                RebalanceOrderSide::Buy => Side::Buy,
                RebalanceOrderSide::Sell => Side::Sell,
            };

            // BUY 신호의 경우 can_enter() 체크
            if side == Side::Buy && !self.can_enter(&order.symbol) {
                debug!(
                    symbol = %order.symbol,
                    "Skipping BUY signal due to RouteState/GlobalScore filter"
                );
                continue;
            }

            // USD를 quote로 사용 (미국 시장)
            let quote_currency = match config.market {
                MarketType::US => "USD",
                MarketType::KR => "KRW",
            };

            // Signal 빌더 패턴으로 생성
            let signal = Signal::new(
                self.name(),
                Symbol::stock(&order.symbol, quote_currency),
                side,
                SignalType::Scale, // 리밸런싱은 Scale 타입 사용
            )
            .with_metadata("current_weight", json!(order.current_weight.to_string()))
            .with_metadata("target_weight", json!(order.target_weight.to_string()))
            .with_metadata("amount", json!(order.amount.to_string()))
            .with_metadata("quantity", json!(order.quantity.to_string()))
            .with_metadata("reason", json!("monthly_rebalance"));

            signals.push(signal);
        }

        // 리밸런싱 시간 기록
        if !signals.is_empty() {
            self.last_rebalance_ym =
                Some(format!("{}_{}", current_time.year(), current_time.month()));
            info!("Simple Power 리밸런싱 완료: {} 주문 생성", signals.len());
        }

        signals
    }
}

impl Default for SimplePowerStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for SimplePowerStrategy {
    fn name(&self) -> &str {
        "Simple Power"
    }

    fn version(&self) -> &str {
        "2.0.0"
    }

    fn description(&self) -> &str {
        "TQQQ/SCHD/PFIX/TMF 기반 모멘텀 자산배분 전략. MA130 필터로 비중 조정, 월간 리밸런싱."
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let parsed_config: SimplePowerConfig = serde_json::from_value(config.clone())?;

        // 시장에 맞는 리밸런싱 설정
        let rebalance_config = match parsed_config.market {
            MarketType::US => RebalanceConfig::us_market(),
            MarketType::KR => RebalanceConfig::korean_market(),
        };
        self.rebalance_calculator = RebalanceCalculator::new(rebalance_config);

        // initial_capital이 있으면 cash_balance로 설정
        if let Some(capital_str) = config.get("initial_capital") {
            if let Some(capital) = capital_str.as_str() {
                if let Ok(capital_dec) = capital.parse::<Decimal>() {
                    self.cash_balance = capital_dec;
                    info!("[Simple Power] 초기 자본금 설정: {}", capital_dec);
                }
            }
        }

        info!(
            "[Simple Power] 전략 초기화 - 시장: {:?}, 자산: {:?}, 초기자본: {}",
            parsed_config.market,
            parsed_config.all_assets(),
            self.cash_balance
        );

        self.config = Some(parsed_config);
        Ok(())
    }

    async fn on_market_data(
        &mut self,
        data: &MarketData,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
        let config = match &self.config {
            Some(c) => c.clone(),
            None => return Ok(Vec::new()),
        };

        let symbol = data.symbol.base.clone();

        // 관심 자산이 아니면 무시
        if !config.all_assets().contains(&symbol) {
            return Ok(Vec::new());
        }

        // MarketDataType에서 가격 추출
        let price = match &data.data {
            MarketDataType::Kline(kline) => Some(kline.close),
            MarketDataType::Ticker(ticker) => Some(ticker.last),
            MarketDataType::Trade(trade) => Some(trade.price),
            MarketDataType::OrderBook(_) => None, // 호가창에서는 가격 추출 안함
        };

        // 가격 업데이트
        if let Some(price) = price {
            self.update_price_history(&symbol, price);
            debug!("[Simple Power] 가격 업데이트: {} = {}", symbol, price);
        }

        // 리밸런싱 신호 생성
        let signals = self.generate_rebalance_signals(&config, data.timestamp);

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "[Simple Power] 주문 체결: {:?} {} {} @ {:?}",
            order.side, order.quantity, order.symbol, order.average_fill_price
        );
        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let symbol = position.symbol.base.clone();
        self.positions.insert(symbol.clone(), position.quantity);
        info!(
            "[Simple Power] 포지션 업데이트: {} = {} (PnL: {})",
            symbol, position.quantity, position.unrealized_pnl
        );
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("[Simple Power] 전략 종료");
        Ok(())
    }

    fn get_state(&self) -> Value {
        let momentum_info: HashMap<String, Value> = self
            .momentum_states
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    json!({
                        "cut_count": v.cut_count,
                        "rate_multiplier": v.rate_multiplier.to_string(),
                        "is_out": v.is_out,
                        "ma_current": v.ma_current.map(|d| d.to_string()),
                        "prev_close": v.prev_close.map(|d| d.to_string()),
                    }),
                )
            })
            .collect();

        json!({
            "name": self.name(),
            "version": self.version(),
            "last_rebalance_ym": self.last_rebalance_ym,
            "momentum_states": momentum_info,
            "positions": self.positions.iter()
                .map(|(k, v)| (k.clone(), v.to_string()))
                .collect::<HashMap<_, _>>(),
            "cash_balance": self.cash_balance.to_string(),
        })
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("StrategyContext injected into Simple Power strategy");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_us_default() {
        let config = SimplePowerConfig::us_default();
        assert_eq!(config.market, MarketType::US);
        assert_eq!(config.aggressive_asset, "TQQQ");
        assert_eq!(config.dividend_asset, "SCHD");
        assert_eq!(config.rate_hedge_asset, "PFIX");
        assert_eq!(config.bond_leverage_asset, "TMF");
        assert_eq!(config.aggressive_weight, dec!(0.5));
        assert_eq!(config.ma_period, 130);
    }

    #[test]
    fn test_config_kr_default() {
        let config = SimplePowerConfig::kr_default();
        assert_eq!(config.market, MarketType::KR);
        assert_eq!(config.aggressive_asset, "409820");
        assert_eq!(config.ma_period, 130);
    }

    #[test]
    fn test_config_all_assets() {
        let config = SimplePowerConfig::us_default();
        let assets = config.all_assets();
        assert_eq!(assets.len(), 4);
        assert!(assets.contains(&"TQQQ".to_string()));
        assert!(assets.contains(&"SCHD".to_string()));
        assert!(assets.contains(&"PFIX".to_string()));
        assert!(assets.contains(&"TMF".to_string()));
    }

    #[test]
    fn test_base_weights_sum() {
        let config = SimplePowerConfig::us_default();
        let weights = config.base_weights();
        let sum: Decimal = weights.values().sum();
        assert_eq!(sum, dec!(1.0));
    }

    #[test]
    fn test_strategy_creation() {
        let strategy = SimplePowerStrategy::new();
        assert_eq!(strategy.name(), "Simple Power");
        assert_eq!(strategy.version(), "2.0.0");
    }

    #[test]
    fn test_calculate_ma() {
        let strategy = SimplePowerStrategy::new();
        let prices: Vec<Decimal> = (0..150).map(|i| dec!(100) + Decimal::from(i)).collect();

        // MA5 at offset 0
        let ma = strategy.calculate_ma(&prices, 5, 0);
        assert!(ma.is_some());
        // MA of [100, 101, 102, 103, 104] = 102
        assert_eq!(ma.unwrap(), dec!(102));
    }

    #[test]
    fn test_should_rebalance_first_time() {
        let strategy = SimplePowerStrategy::new();
        let now = Utc::now();
        assert!(strategy.should_rebalance(now));
    }

    #[test]
    fn test_should_rebalance_same_month() {
        let mut strategy = SimplePowerStrategy::new();
        let now = Utc::now();
        strategy.last_rebalance_ym = Some(format!("{}_{}", now.year(), now.month()));
        assert!(!strategy.should_rebalance(now));
    }

    #[test]
    fn test_update_price_history() {
        let mut strategy = SimplePowerStrategy::new();
        strategy.update_price_history("TQQQ", dec!(100));
        strategy.update_price_history("TQQQ", dec!(101));
        strategy.update_price_history("TQQQ", dec!(102));

        let history = strategy.price_history.get("TQQQ").unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0], dec!(102)); // 최신 가격이 앞에
        assert_eq!(history[1], dec!(101));
        assert_eq!(history[2], dec!(100));
    }

    #[test]
    fn test_momentum_state_no_data() {
        let strategy = SimplePowerStrategy::new();
        let config = SimplePowerConfig::us_default();
        let state = strategy.calculate_momentum_state("TQQQ", &config);

        // 데이터 없으면 기본값
        assert_eq!(state.cut_count, 0);
        assert_eq!(state.rate_multiplier, dec!(1.0));
        assert!(!state.is_out);
    }

    #[test]
    fn test_momentum_state_with_data() {
        let mut strategy = SimplePowerStrategy::new();
        let config = SimplePowerConfig::us_default();

        // 상승 추세 데이터 생성 (최신이 가장 높음)
        let prices: Vec<Decimal> = (0..200)
            .rev()
            .map(|i| dec!(100) + Decimal::from(i) * dec!(0.1))
            .collect();
        strategy.price_history.insert("TQQQ".to_string(), prices);

        let state = strategy.calculate_momentum_state("TQQQ", &config);

        // 상승 추세이므로 컷 없음
        assert_eq!(state.cut_count, 0);
        assert_eq!(state.rate_multiplier, dec!(1.0));
    }

    #[test]
    fn test_get_state() {
        let strategy = SimplePowerStrategy::new();
        let state = strategy.get_state();

        assert_eq!(state["name"], "Simple Power");
        assert_eq!(state["version"], "2.0.0");
    }
}

// 전략 레지스트리에 자동 등록
use crate::register_strategy;

register_strategy! {
    id: "simple_power",
    aliases: [],
    name: "Simple Power",
    description: "심플 파워 모멘텀 자산배분 전략입니다.",
    timeframe: "1d",
    symbols: ["TQQQ", "SCHD", "PFIX", "TMF"],
    category: Monthly,
    markets: [Stock],
    type: SimplePowerStrategy
}
