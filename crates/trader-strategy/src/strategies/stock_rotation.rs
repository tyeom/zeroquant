//! Stock Rotation (종목 갈아타기) 전략 구현.
//!
//! 모멘텀 기반으로 상위 N개 종목을 선택하고, 순위 변동에 따라
//! 종목을 교체하는 전략입니다.
//!
//! Python 66번 전략 (미국주식시총TOP전략) 패턴 기반 구현.
//!
//! # 전략 개요
//!
//! ## 핵심 로직
//! 1. 종목 유니버스에서 모멘텀 스코어 계산
//! 2. 모멘텀 상위 N개 종목 선택
//! 3. 순위에서 밀려난 종목은 매도
//! 4. 새로 진입한 종목은 매수
//!
//! ## 모멘텀 계산
//! ```text
//! MomentumScore = (1M + 3M + 6M + 12M) / 4
//! ```
//!
//! ## 리밸런싱
//! 월간 리밸런싱 (매월 초)
//!
//! ## 종목 교체 조건
//! - 순위 밖으로 밀려남 → 전량 매도
//! - 새로 순위 진입 → 균등 비중 매수

use async_trait::async_trait;
use chrono::{DateTime, Datelike, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use trader_core::domain::{RouteState, StrategyContext};

use crate::strategies::common::rebalance::{
    PortfolioPosition, RebalanceCalculator, RebalanceConfig, RebalanceOrderSide, TargetAllocation,
};
use crate::traits::Strategy;
use trader_core::{MarketData, MarketDataType, Order, Position, Side, Signal, SignalType, Symbol};

/// 종목 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockInfo {
    /// 종목 코드
    pub symbol: String,
    /// 종목명
    pub name: String,
    /// 섹터 (선택)
    pub sector: Option<String>,
}

impl StockInfo {
    /// 새 종목 정보 생성.
    pub fn new(symbol: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            name: name.into(),
            sector: None,
        }
    }

    /// 섹터와 함께 생성.
    pub fn with_sector(mut self, sector: impl Into<String>) -> Self {
        self.sector = Some(sector.into());
        self
    }
}

/// Stock Rotation 전략 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockRotationConfig {
    /// 시장 타입 (US/KR)
    pub market: RotationMarketType,

    /// 종목 유니버스
    pub universe: Vec<StockInfo>,

    /// 보유할 종목 수 (Top N)
    pub top_n: usize,

    /// 투자 비율 (총 자산 대비)
    pub invest_rate: Decimal,

    /// 리밸런싱 임계값 (비중 차이 %)
    pub rebalance_threshold: Decimal,

    /// 최소 모멘텀 (이 이하면 투자 안 함)
    pub min_momentum: Option<Decimal>,

    /// 현금 보유 비율 (0.0 ~ 1.0)
    pub cash_reserve_rate: Decimal,

    /// 최소 GlobalScore (기본값: 60)
    #[serde(default = "default_min_global_score")]
    pub min_global_score: Decimal,
}

fn default_min_global_score() -> Decimal {
    dec!(60)
}

/// 시장 타입.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RotationMarketType {
    /// 미국 시장
    US,
    /// 한국 시장
    KR,
}

impl Default for StockRotationConfig {
    fn default() -> Self {
        Self::us_default()
    }
}

impl StockRotationConfig {
    /// 미국 시장 기본 설정 (시총 상위 종목).
    pub fn us_default() -> Self {
        Self {
            market: RotationMarketType::US,
            universe: vec![
                StockInfo::new("AAPL", "Apple Inc.").with_sector("Technology"),
                StockInfo::new("MSFT", "Microsoft Corporation").with_sector("Technology"),
                StockInfo::new("GOOGL", "Alphabet Inc.").with_sector("Technology"),
                StockInfo::new("AMZN", "Amazon.com Inc.").with_sector("Consumer"),
                StockInfo::new("NVDA", "NVIDIA Corporation").with_sector("Technology"),
                StockInfo::new("META", "Meta Platforms Inc.").with_sector("Technology"),
                StockInfo::new("TSLA", "Tesla Inc.").with_sector("Consumer"),
                StockInfo::new("BRK.B", "Berkshire Hathaway").with_sector("Financials"),
                StockInfo::new("JPM", "JPMorgan Chase & Co.").with_sector("Financials"),
                StockInfo::new("V", "Visa Inc.").with_sector("Financials"),
            ],
            top_n: 5,
            invest_rate: dec!(1.0),
            rebalance_threshold: dec!(0.03),
            min_momentum: None,
            cash_reserve_rate: dec!(0.0),
            min_global_score: default_min_global_score(),
        }
    }

    /// 한국 시장 설정 (KOSPI 대형주).
    pub fn kr_default() -> Self {
        Self {
            market: RotationMarketType::KR,
            universe: vec![
                StockInfo::new("005930", "삼성전자").with_sector("IT"),
                StockInfo::new("000660", "SK하이닉스").with_sector("IT"),
                StockInfo::new("373220", "LG에너지솔루션").with_sector("IT"),
                StockInfo::new("207940", "삼성바이오로직스").with_sector("Healthcare"),
                StockInfo::new("005380", "현대차").with_sector("Consumer"),
                StockInfo::new("006400", "삼성SDI").with_sector("IT"),
                StockInfo::new("035420", "NAVER").with_sector("IT"),
                StockInfo::new("000270", "기아").with_sector("Consumer"),
                StockInfo::new("035720", "카카오").with_sector("IT"),
                StockInfo::new("105560", "KB금융").with_sector("Financials"),
            ],
            top_n: 5,
            invest_rate: dec!(1.0),
            rebalance_threshold: dec!(0.03),
            min_momentum: None,
            cash_reserve_rate: dec!(0.0),
            min_global_score: default_min_global_score(),
        }
    }

    /// 유니버스의 모든 심볼 가져오기.
    pub fn all_symbols(&self) -> Vec<String> {
        self.universe.iter().map(|s| s.symbol.clone()).collect()
    }
}

/// 종목 모멘텀 정보.
#[derive(Debug, Clone)]
struct StockMomentum {
    /// 종목 코드
    symbol: String,
    /// 모멘텀 스코어
    momentum: Decimal,
    /// 순위 (1이 가장 높음)
    rank: usize,
}

/// Stock Rotation 전략.
pub struct StockRotationStrategy {
    config: Option<StockRotationConfig>,
    /// StrategyContext (RouteState, GlobalScore 조회용)
    context: Option<Arc<RwLock<StrategyContext>>>,
    /// 자산별 가격 히스토리 (최신 가격이 앞에)
    price_history: HashMap<String, Vec<Decimal>>,
    /// 현재 포지션
    positions: HashMap<String, Decimal>,
    /// 현재 보유 종목 리스트
    current_holdings: HashSet<String>,
    /// 마지막 리밸런싱 년월 (YYYY_MM)
    last_rebalance_ym: Option<String>,
    /// 리밸런싱 계산기
    rebalance_calculator: RebalanceCalculator,
    /// 현재 현금 잔고
    cash_balance: Decimal,
}

impl StockRotationStrategy {
    /// 새 전략 생성.
    pub fn new() -> Self {
        Self {
            config: None,
            context: None,
            price_history: HashMap::new(),
            positions: HashMap::new(),
            current_holdings: HashSet::new(),
            last_rebalance_ym: None,
            rebalance_calculator: RebalanceCalculator::new(RebalanceConfig::us_market()),
            cash_balance: Decimal::ZERO,
        }
    }

    /// 설정으로 전략 생성.
    pub fn with_config(config: StockRotationConfig) -> Self {
        let rebalance_config = match config.market {
            RotationMarketType::US => RebalanceConfig::us_market(),
            RotationMarketType::KR => RebalanceConfig::korean_market(),
        };

        Self {
            config: Some(config),
            context: None,
            price_history: HashMap::new(),
            positions: HashMap::new(),
            current_holdings: HashSet::new(),
            last_rebalance_ym: None,
            rebalance_calculator: RebalanceCalculator::new(rebalance_config),
            cash_balance: Decimal::ZERO,
        }
    }

    /// 진입 가능 여부 확인 (RouteState, GlobalScore 기반).
    fn can_enter(&self, ticker: &str) -> bool {
        let Some(config) = self.config.as_ref() else {
            return false;
        };

        let Some(ctx) = self.context.as_ref() else {
            // Context 없으면 진입 허용 (하위 호환성)
            return true;
        };

        let Ok(ctx_lock) = ctx.try_read() else {
            return true;
        };

        // RouteState 체크
        if let Some(route_state) = ctx_lock.get_route_state(ticker) {
            match route_state {
                RouteState::Overheat | RouteState::Wait | RouteState::Neutral => {
                    debug!(
                        ticker = %ticker,
                        route_state = ?route_state,
                        "[StockRotation] RouteState 진입 제한"
                    );
                    return false;
                }
                RouteState::Armed | RouteState::Attack => {
                    // 진입 가능
                }
            }
        }

        // GlobalScore 체크
        if let Some(score) = ctx_lock.get_global_score(ticker) {
            if score.overall_score < config.min_global_score {
                debug!(
                    ticker = %ticker,
                    score = %score.overall_score,
                    min_score = %config.min_global_score,
                    "[StockRotation] GlobalScore 미달로 진입 제한"
                );
                return false;
            }
        }

        true
    }

    /// 가격 히스토리 업데이트.
    fn update_price_history(&mut self, symbol: &str, price: Decimal) {
        let history = self.price_history.entry(symbol.to_string()).or_default();
        history.insert(0, price);

        // 최대 300일 보관
        if history.len() > 300 {
            history.truncate(300);
        }
    }

    /// 모멘텀 스코어 계산.
    ///
    /// 공식: (1M + 3M + 6M + 12M) / 4
    fn calculate_momentum(&self, symbol: &str) -> Option<Decimal> {
        let prices = self.price_history.get(symbol)?;

        // 최소 240일(12개월) 데이터 필요
        if prices.len() < 240 {
            debug!("[StockRotation] {} 데이터 부족: {}일", symbol, prices.len());
            return None;
        }

        let now_price = *prices.first()?;
        let one_month = *prices.get(20)?;
        let three_month = *prices.get(60)?;
        let six_month = *prices.get(120)?;
        let twelve_month = *prices.get(239)?;

        // 0으로 나누기 방지
        if one_month.is_zero()
            || three_month.is_zero()
            || six_month.is_zero()
            || twelve_month.is_zero()
        {
            return None;
        }

        let ret_1m = (now_price - one_month) / one_month;
        let ret_3m = (now_price - three_month) / three_month;
        let ret_6m = (now_price - six_month) / six_month;
        let ret_12m = (now_price - twelve_month) / twelve_month;

        let momentum = (ret_1m + ret_3m + ret_6m + ret_12m) / dec!(4);

        debug!(
            "[StockRotation] {} Momentum: {:.4} (1M:{:.2}%, 3M:{:.2}%, 6M:{:.2}%, 12M:{:.2}%)",
            symbol,
            momentum,
            ret_1m * dec!(100),
            ret_3m * dec!(100),
            ret_6m * dec!(100),
            ret_12m * dec!(100)
        );

        Some(momentum)
    }

    /// 모든 유니버스 종목의 모멘텀 순위 계산.
    fn rank_all_stocks(&self, config: &StockRotationConfig) -> Vec<StockMomentum> {
        let mut stocks: Vec<StockMomentum> = Vec::new();

        for stock_info in &config.universe {
            if let Some(momentum) = self.calculate_momentum(&stock_info.symbol) {
                // 최소 모멘텀 필터
                if let Some(min_mom) = config.min_momentum {
                    if momentum < min_mom {
                        debug!(
                            "[StockRotation] {} 모멘텀 {:.4} < 최소값 {:.4} → 제외",
                            stock_info.symbol, momentum, min_mom
                        );
                        continue;
                    }
                }

                stocks.push(StockMomentum {
                    symbol: stock_info.symbol.clone(),
                    momentum,
                    rank: 0,
                });
            }
        }

        // 모멘텀 내림차순 정렬
        stocks.sort_by(|a, b| b.momentum.cmp(&a.momentum));

        // 순위 부여
        for (i, stock) in stocks.iter_mut().enumerate() {
            stock.rank = i + 1;
        }

        stocks
    }

    /// 교체 대상 종목 계산.
    ///
    /// Returns: (매도할 종목, 매수할 종목)
    fn calculate_rotation(
        &self,
        config: &StockRotationConfig,
        ranked_stocks: &[StockMomentum],
    ) -> (Vec<String>, Vec<String>) {
        // 상위 N개 종목
        let new_top_n: HashSet<String> = ranked_stocks
            .iter()
            .take(config.top_n)
            .map(|s| s.symbol.clone())
            .collect();

        // 밀려난 종목 (현재 보유 중인데 상위 N에서 빠진 종목)
        let stocks_to_sell: Vec<String> = self
            .current_holdings
            .iter()
            .filter(|s| !new_top_n.contains(*s))
            .cloned()
            .collect();

        // 새로 진입한 종목 (상위 N에 있는데 현재 미보유)
        let stocks_to_buy: Vec<String> = new_top_n
            .iter()
            .filter(|s| !self.current_holdings.contains(*s))
            .cloned()
            .collect();

        (stocks_to_sell, stocks_to_buy)
    }

    /// 목표 비중 계산.
    fn calculate_target_weights(
        &self,
        config: &StockRotationConfig,
        ranked_stocks: &[StockMomentum],
    ) -> Vec<TargetAllocation> {
        let mut allocations: Vec<TargetAllocation> = Vec::new();

        // 투자 가능 비율 (현금 보유분 제외)
        let investable_rate = dec!(1.0) - config.cash_reserve_rate;

        // 상위 N개 선택
        let top_n_count = config.top_n.min(ranked_stocks.len());

        if top_n_count == 0 {
            return allocations;
        }

        // 균등 비중
        let weight_per_stock = investable_rate / Decimal::from(top_n_count);

        for stock in ranked_stocks.iter().take(top_n_count) {
            allocations.push(TargetAllocation::new(
                stock.symbol.clone(),
                weight_per_stock,
            ));
            info!(
                "[StockRotation] #{} {} (Momentum: {:.4}, 비중: {:.1}%)",
                stock.rank,
                stock.symbol,
                stock.momentum,
                weight_per_stock * dec!(100)
            );
        }

        allocations
    }

    /// 리밸런싱 필요 여부 확인.
    fn should_rebalance(&self, current_time: DateTime<Utc>) -> bool {
        let current_ym = format!("{}_{}", current_time.year(), current_time.month());

        match &self.last_rebalance_ym {
            None => true,
            Some(last_ym) => last_ym != &current_ym,
        }
    }

    /// 리밸런싱 신호 생성.
    fn generate_rebalance_signals(
        &mut self,
        config: &StockRotationConfig,
        current_time: DateTime<Utc>,
    ) -> Vec<Signal> {
        if !self.should_rebalance(current_time) {
            return Vec::new();
        }

        // 모멘텀 순위 계산
        let ranked_stocks = self.rank_all_stocks(config);

        if ranked_stocks.is_empty() {
            warn!("[StockRotation] 모멘텀 계산 가능한 종목 없음");
            return Vec::new();
        }

        // 교체 대상 계산
        let (stocks_to_sell, stocks_to_buy) = self.calculate_rotation(config, &ranked_stocks);

        // 로깅
        if !stocks_to_sell.is_empty() {
            info!(
                "[StockRotation] 매도 예정 (순위 이탈): {:?}",
                stocks_to_sell
            );
        }
        if !stocks_to_buy.is_empty() {
            info!("[StockRotation] 매수 예정 (순위 진입): {:?}", stocks_to_buy);
        }

        // 목표 비중 계산
        let target_allocations = self.calculate_target_weights(config, &ranked_stocks);

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
            RotationMarketType::US => "USD",
            RotationMarketType::KR => "KRW",
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

            // 매수 신호의 경우 can_enter() 체크
            if side == Side::Buy && !self.can_enter(&order.symbol) {
                debug!(
                    symbol = %order.symbol,
                    "[StockRotation] RouteState/GlobalScore 조건 미충족, 매수 신호 스킵"
                );
                continue;
            }

            let quote_currency = match config.market {
                RotationMarketType::US => "USD",
                RotationMarketType::KR => "KRW",
            };

            // 교체 사유 결정
            let rotation_type = if stocks_to_sell.contains(&order.symbol) {
                "rank_exit"
            } else if stocks_to_buy.contains(&order.symbol) {
                "rank_enter"
            } else {
                "rebalance"
            };

            let signal = Signal::new(
                self.name(),
                Symbol::stock(&order.symbol, quote_currency),
                side,
                SignalType::Scale,
            )
            .with_metadata("current_weight", json!(order.current_weight.to_string()))
            .with_metadata("target_weight", json!(order.target_weight.to_string()))
            .with_metadata("amount", json!(order.amount.to_string()))
            .with_metadata("quantity", json!(order.quantity.to_string()))
            .with_metadata("rotation_type", json!(rotation_type))
            .with_metadata("reason", json!("monthly_rotation"));

            signals.push(signal);
        }

        // 현재 보유 종목 업데이트
        if !signals.is_empty() {
            // 매도 종목 제거
            for symbol in &stocks_to_sell {
                self.current_holdings.remove(symbol);
            }
            // 매수 종목 추가
            for symbol in &stocks_to_buy {
                self.current_holdings.insert(symbol.clone());
            }

            self.last_rebalance_ym =
                Some(format!("{}_{}", current_time.year(), current_time.month()));
            info!(
                "[StockRotation] 리밸런싱 완료: {} 주문 생성 (매도: {}, 매수: {})",
                signals.len(),
                stocks_to_sell.len(),
                stocks_to_buy.len()
            );
        }

        signals
    }
}

impl Default for StockRotationStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for StockRotationStrategy {
    fn name(&self) -> &str {
        "StockRotation"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "종목 갈아타기 전략. 모멘텀 순위 기반으로 상위 N개 종목을 보유하고, 순위 변동 시 교체."
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let parsed_config: StockRotationConfig = serde_json::from_value(config.clone())?;

        let rebalance_config = match parsed_config.market {
            RotationMarketType::US => RebalanceConfig::us_market(),
            RotationMarketType::KR => RebalanceConfig::korean_market(),
        };
        self.rebalance_calculator = RebalanceCalculator::new(rebalance_config);

        // initial_capital이 있으면 cash_balance로 설정
        if let Some(capital_str) = config.get("initial_capital") {
            if let Some(capital) = capital_str.as_str() {
                if let Ok(capital_dec) = capital.parse::<Decimal>() {
                    self.cash_balance = capital_dec;
                    info!("[StockRotation] 초기 자본금 설정: {}", capital_dec);
                }
            }
        }

        info!(
            "[StockRotation] 전략 초기화 - 시장: {:?}, 유니버스: {}개, Top N: {}, 초기자본: {}",
            parsed_config.market,
            parsed_config.universe.len(),
            parsed_config.top_n,
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

        // 유니버스에 없는 종목이면 무시
        if !config.all_symbols().contains(&symbol) {
            return Ok(Vec::new());
        }

        // 가격 추출
        let price = match &data.data {
            MarketDataType::Kline(kline) => Some(kline.close),
            MarketDataType::Ticker(ticker) => Some(ticker.last),
            MarketDataType::Trade(trade) => Some(trade.price),
            MarketDataType::OrderBook(_) => None,
        };

        // 가격 업데이트
        if let Some(price) = price {
            self.update_price_history(&symbol, price);
            debug!("[StockRotation] 가격 업데이트: {} = {}", symbol, price);
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
            "[StockRotation] 주문 체결: {:?} {} {} @ {:?}",
            order.side, order.quantity, order.symbol, order.average_fill_price
        );
        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let symbol = position.symbol.base.clone();

        if position.quantity > Decimal::ZERO {
            self.positions.insert(symbol.clone(), position.quantity);
            self.current_holdings.insert(symbol.clone());
        } else {
            self.positions.remove(&symbol);
            self.current_holdings.remove(&symbol);
        }

        info!(
            "[StockRotation] 포지션 업데이트: {} = {} (PnL: {})",
            symbol, position.quantity, position.unrealized_pnl
        );
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("[StockRotation] 전략 종료");
        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "name": self.name(),
            "version": self.version(),
            "last_rebalance_ym": self.last_rebalance_ym,
            "current_holdings": self.current_holdings.iter().collect::<Vec<_>>(),
            "holdings_count": self.current_holdings.len(),
            "positions": self.positions.iter()
                .map(|(k, v)| (k.clone(), v.to_string()))
                .collect::<HashMap<_, _>>(),
            "cash_balance": self.cash_balance.to_string(),
        })
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("StrategyContext injected into StockRotation strategy");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_us_default() {
        let config = StockRotationConfig::us_default();
        assert_eq!(config.market, RotationMarketType::US);
        assert_eq!(config.universe.len(), 10);
        assert_eq!(config.top_n, 5);
        assert_eq!(config.invest_rate, dec!(1.0));
    }

    #[test]
    fn test_config_kr_default() {
        let config = StockRotationConfig::kr_default();
        assert_eq!(config.market, RotationMarketType::KR);
        assert_eq!(config.universe.len(), 10);
        assert_eq!(config.top_n, 5);
    }

    #[test]
    fn test_all_symbols() {
        let config = StockRotationConfig::us_default();
        let symbols = config.all_symbols();

        assert!(symbols.contains(&"AAPL".to_string()));
        assert!(symbols.contains(&"MSFT".to_string()));
        assert_eq!(symbols.len(), 10);
    }

    #[test]
    fn test_stock_info_creation() {
        let stock = StockInfo::new("AAPL", "Apple Inc.");
        assert_eq!(stock.symbol, "AAPL");
        assert_eq!(stock.name, "Apple Inc.");
        assert!(stock.sector.is_none());

        let stock_with_sector = stock.with_sector("Technology");
        assert_eq!(stock_with_sector.sector, Some("Technology".to_string()));
    }

    #[test]
    fn test_strategy_creation() {
        let strategy = StockRotationStrategy::new();
        assert_eq!(strategy.name(), "StockRotation");
        assert_eq!(strategy.version(), "1.0.0");
        assert!(strategy.current_holdings.is_empty());
    }

    #[test]
    fn test_should_rebalance_first_time() {
        let strategy = StockRotationStrategy::new();
        let now = Utc::now();
        assert!(strategy.should_rebalance(now));
    }

    #[test]
    fn test_should_rebalance_same_month() {
        let mut strategy = StockRotationStrategy::new();
        let now = Utc::now();
        strategy.last_rebalance_ym = Some(format!("{}_{}", now.year(), now.month()));
        assert!(!strategy.should_rebalance(now));
    }

    #[test]
    fn test_update_price_history() {
        let mut strategy = StockRotationStrategy::new();
        strategy.update_price_history("AAPL", dec!(150));
        strategy.update_price_history("AAPL", dec!(151));
        strategy.update_price_history("AAPL", dec!(152));

        let history = strategy.price_history.get("AAPL").unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0], dec!(152)); // 최신 가격이 앞에
    }

    #[test]
    fn test_momentum_insufficient_data() {
        let strategy = StockRotationStrategy::new();
        let momentum = strategy.calculate_momentum("AAPL");
        assert!(momentum.is_none());
    }

    #[test]
    fn test_momentum_calculation() {
        let mut strategy = StockRotationStrategy::new();

        // 상승 추세 데이터 생성 (240일)
        let prices: Vec<Decimal> = (0..250)
            .rev()
            .map(|i| dec!(100) + Decimal::from(i) * dec!(0.1))
            .collect();
        strategy.price_history.insert("AAPL".to_string(), prices);

        let momentum = strategy.calculate_momentum("AAPL");
        assert!(momentum.is_some());
        assert!(momentum.unwrap() > Decimal::ZERO);
    }

    #[test]
    fn test_rank_all_stocks() {
        let mut strategy = StockRotationStrategy::with_config(StockRotationConfig {
            market: RotationMarketType::US,
            universe: vec![
                StockInfo::new("AAPL", "Apple"),
                StockInfo::new("MSFT", "Microsoft"),
                StockInfo::new("GOOGL", "Alphabet"),
            ],
            top_n: 2,
            invest_rate: dec!(1.0),
            rebalance_threshold: dec!(0.03),
            min_momentum: None,
            cash_reserve_rate: dec!(0.0),
            min_global_score: dec!(60),
        });

        // AAPL: 높은 모멘텀, MSFT: 중간, GOOGL: 낮음
        let aapl_prices: Vec<Decimal> = (0..250)
            .rev()
            .map(|i| dec!(100) + Decimal::from(i) * dec!(0.3))
            .collect();
        let msft_prices: Vec<Decimal> = (0..250)
            .rev()
            .map(|i| dec!(100) + Decimal::from(i) * dec!(0.2))
            .collect();
        let googl_prices: Vec<Decimal> = (0..250)
            .rev()
            .map(|i| dec!(100) + Decimal::from(i) * dec!(0.1))
            .collect();

        strategy
            .price_history
            .insert("AAPL".to_string(), aapl_prices);
        strategy
            .price_history
            .insert("MSFT".to_string(), msft_prices);
        strategy
            .price_history
            .insert("GOOGL".to_string(), googl_prices);

        let config = strategy.config.as_ref().unwrap();
        let ranked = strategy.rank_all_stocks(config);

        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].symbol, "AAPL");
        assert_eq!(ranked[0].rank, 1);
        assert_eq!(ranked[1].symbol, "MSFT");
        assert_eq!(ranked[1].rank, 2);
        assert_eq!(ranked[2].symbol, "GOOGL");
        assert_eq!(ranked[2].rank, 3);
    }

    #[test]
    fn test_calculate_rotation() {
        let mut strategy = StockRotationStrategy::new();

        // 현재 보유: AAPL, MSFT
        strategy.current_holdings.insert("AAPL".to_string());
        strategy.current_holdings.insert("MSFT".to_string());

        let config = StockRotationConfig {
            market: RotationMarketType::US,
            universe: vec![
                StockInfo::new("AAPL", "Apple"),
                StockInfo::new("MSFT", "Microsoft"),
                StockInfo::new("GOOGL", "Alphabet"),
            ],
            top_n: 2,
            invest_rate: dec!(1.0),
            rebalance_threshold: dec!(0.03),
            min_momentum: None,
            cash_reserve_rate: dec!(0.0),
            min_global_score: dec!(60),
        };

        // 새 순위: AAPL, GOOGL (MSFT 탈락)
        let ranked = vec![
            StockMomentum {
                symbol: "AAPL".to_string(),
                momentum: dec!(0.3),
                rank: 1,
            },
            StockMomentum {
                symbol: "GOOGL".to_string(),
                momentum: dec!(0.2),
                rank: 2,
            },
            StockMomentum {
                symbol: "MSFT".to_string(),
                momentum: dec!(0.1),
                rank: 3,
            },
        ];

        let (to_sell, to_buy) = strategy.calculate_rotation(&config, &ranked);

        assert_eq!(to_sell, vec!["MSFT".to_string()]);
        assert_eq!(to_buy, vec!["GOOGL".to_string()]);
    }

    #[test]
    fn test_min_momentum_filter() {
        let mut strategy = StockRotationStrategy::with_config(StockRotationConfig {
            market: RotationMarketType::US,
            universe: vec![
                StockInfo::new("AAPL", "Apple"),
                StockInfo::new("MSFT", "Microsoft"),
            ],
            top_n: 2,
            invest_rate: dec!(1.0),
            rebalance_threshold: dec!(0.03),
            min_momentum: Some(dec!(0.05)), // 최소 5% 모멘텀
            cash_reserve_rate: dec!(0.0),
            min_global_score: dec!(60),
        });

        // AAPL: 높은 모멘텀 (통과), MSFT: 낮은 모멘텀 (필터)
        let aapl_prices: Vec<Decimal> = (0..250)
            .rev()
            .map(|i| dec!(100) + Decimal::from(i) * dec!(0.3))
            .collect();
        let msft_prices: Vec<Decimal> = (0..250)
            .rev()
            .map(|i| dec!(100) + Decimal::from(i) * dec!(0.01))
            .collect();

        strategy
            .price_history
            .insert("AAPL".to_string(), aapl_prices);
        strategy
            .price_history
            .insert("MSFT".to_string(), msft_prices);

        let config = strategy.config.as_ref().unwrap();
        let ranked = strategy.rank_all_stocks(config);

        // MSFT는 최소 모멘텀 미달로 제외됨
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].symbol, "AAPL");
    }

    #[test]
    fn test_get_state() {
        let mut strategy = StockRotationStrategy::new();
        strategy.current_holdings.insert("AAPL".to_string());
        strategy.current_holdings.insert("MSFT".to_string());

        let state = strategy.get_state();

        assert_eq!(state["name"], "StockRotation");
        assert_eq!(state["version"], "1.0.0");
        assert_eq!(state["holdings_count"], 2);
    }
}

// 전략 레지스트리에 자동 등록
use crate::register_strategy;

register_strategy! {
    id: "stock_rotation",
    aliases: ["rotation"],
    name: "종목 갈아타기",
    description: "모멘텀 기반 종목 회전 전략입니다.",
    timeframe: "1d",
    symbols: ["005930", "000660", "035420", "051910", "006400"],
    category: Daily,
    markets: [Stock, Stock],
    type: StockRotationStrategy
}
