//! HAA (Hierarchical Asset Allocation) 전략 구현.
//!
//! 계층적 자산배분 전략으로 카나리아 자산을 통해 위험을 감지하고,
//! 상황에 따라 공격/방어 자산에 투자합니다.
//!
//! Python 22번 전략 변환.
//!
//! # 전략 개요
//!
//! ## 자산 분류
//! - **카나리아 (BIRD)**: TIP - 위험 감지용 (모멘텀 < 0이면 방어 모드)
//! - **공격 자산 (RISK)**: SPY, IWM, VEA, VWO, TLT, IEF, PDBC, VNQ 등
//! - **안전 자산 (SAFE)**: IEF, BIL (현금 대용)
//!
//! ## 모멘텀 계산
//! ```text
//! MomentumScore = (1M수익률) + (3M수익률) + (6M수익률) + (12M수익률)
//! ```
//!
//! ## 자산 선택 로직
//! 1. 카나리아(TIP) 모멘텀 < 0 → 방어 모드
//! 2. 공격 모드: Risk 자산 중 모멘텀 상위 N개 선택 (기본 4개)
//! 3. 방어 모드: Safe 자산 중 모멘텀 상위 1개 선택
//! 4. BIL이 선택되면 현금 보유 (투자 안 함)
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
use tracing::{debug, info, warn};

use crate::strategies::common::rebalance::{
    PortfolioPosition, RebalanceCalculator, RebalanceConfig, RebalanceOrderSide,
    TargetAllocation,
};
use crate::traits::Strategy;
use trader_core::{MarketData, MarketDataType, Order, Position, Side, Signal, SignalType, Symbol};

/// 자산 타입.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetType {
    /// 카나리아 자산 (위험 감지용)
    Canary,
    /// 공격 자산 (Risk)
    Offensive,
    /// 방어 자산 (Safe)
    Defensive,
    /// 현금 대용 (BIL 등)
    Cash,
}

/// 자산 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetInfo {
    /// 종목 코드
    pub symbol: String,
    /// 자산 타입
    pub asset_type: AssetType,
    /// 설명
    pub description: String,
}

impl AssetInfo {
    /// 새 자산 정보 생성.
    pub fn new(symbol: impl Into<String>, asset_type: AssetType, description: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            asset_type,
            description: description.into(),
        }
    }

    /// 카나리아 자산 생성.
    pub fn canary(symbol: impl Into<String>, description: impl Into<String>) -> Self {
        Self::new(symbol, AssetType::Canary, description)
    }

    /// 공격 자산 생성.
    pub fn offensive(symbol: impl Into<String>, description: impl Into<String>) -> Self {
        Self::new(symbol, AssetType::Offensive, description)
    }

    /// 방어 자산 생성.
    pub fn defensive(symbol: impl Into<String>, description: impl Into<String>) -> Self {
        Self::new(symbol, AssetType::Defensive, description)
    }

    /// 현금 대용 자산 생성.
    pub fn cash(symbol: impl Into<String>, description: impl Into<String>) -> Self {
        Self::new(symbol, AssetType::Cash, description)
    }
}

/// HAA 전략 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HaaConfig {
    /// 시장 타입 (US/KR)
    pub market: HaaMarketType,

    /// 카나리아 자산 목록
    pub canary_assets: Vec<AssetInfo>,

    /// 공격 자산 목록
    pub offensive_assets: Vec<AssetInfo>,

    /// 방어 자산 목록
    pub defensive_assets: Vec<AssetInfo>,

    /// 공격 자산 투자 개수 (기본: 4)
    pub offensive_top_n: usize,

    /// 방어 자산 투자 개수 (기본: 1)
    pub defensive_top_n: usize,

    /// 현금 대용 심볼 (BIL)
    pub cash_symbol: String,

    /// 투자 비율 (총 자산 대비)
    pub invest_rate: Decimal,

    /// 리밸런싱 임계값
    pub rebalance_threshold: Decimal,
}

/// 시장 타입.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HaaMarketType {
    /// 미국 시장
    US,
    /// 한국 시장
    KR,
}

impl Default for HaaConfig {
    fn default() -> Self {
        Self::us_default()
    }
}

impl HaaConfig {
    /// 미국 시장 기본 설정.
    pub fn us_default() -> Self {
        Self {
            market: HaaMarketType::US,
            canary_assets: vec![
                AssetInfo::canary("TIP", "iShares TIPS Bond ETF"),
            ],
            offensive_assets: vec![
                AssetInfo::offensive("SPY", "S&P 500 ETF"),
                AssetInfo::offensive("IWM", "Russell 2000 ETF"),
                AssetInfo::offensive("VEA", "Vanguard Developed Markets ETF"),
                AssetInfo::offensive("VWO", "Vanguard Emerging Markets ETF"),
                AssetInfo::offensive("TLT", "iShares 20+ Year Treasury ETF"),
                AssetInfo::offensive("IEF", "iShares 7-10 Year Treasury ETF"),
                AssetInfo::offensive("PDBC", "Invesco DB Commodity Index ETF"),
                AssetInfo::offensive("VNQ", "Vanguard Real Estate ETF"),
            ],
            defensive_assets: vec![
                AssetInfo::defensive("IEF", "iShares 7-10 Year Treasury ETF"),
                AssetInfo::cash("BIL", "SPDR 1-3 Month T-Bill ETF (Cash proxy)"),
            ],
            offensive_top_n: 4,
            defensive_top_n: 1,
            cash_symbol: "BIL".to_string(),
            invest_rate: dec!(1.0),
            rebalance_threshold: dec!(0.03),
        }
    }

    /// 한국 시장 설정.
    ///
    /// 한국 ETF로 대체:
    /// - TIP: 물가연동채 ETF (430500 KOSEF 물가채)
    /// - 공격 자산: 글로벌 ETF들
    /// - 방어 자산: 채권 ETF
    pub fn kr_default() -> Self {
        Self {
            market: HaaMarketType::KR,
            canary_assets: vec![
                // 한국에서 TIP 직접 사용 (모멘텀 계산용)
                AssetInfo::canary("TIP", "iShares TIPS (US, 모멘텀 계산용)"),
            ],
            offensive_assets: vec![
                AssetInfo::offensive("360750", "TIGER 미국S&P500"),
                AssetInfo::offensive("280930", "TIGER 미국나스닥100"),
                AssetInfo::offensive("251350", "KODEX 선진국MSCI World"),
                AssetInfo::offensive("195980", "ARIRANG 신흥국MSCI"),
                AssetInfo::offensive("304660", "KODEX 미국채울트라30년선물(H)"),
                AssetInfo::offensive("305080", "TIGER 미국채10년선물"),
                AssetInfo::offensive("276000", "KBSTAR 미국S&P원유생산기업"),
                AssetInfo::offensive("352560", "TIGER 미국필라델피아반도체나스닥"),
            ],
            defensive_assets: vec![
                AssetInfo::defensive("305080", "TIGER 미국채10년선물"),
                AssetInfo::cash("BIL", "현금 대용 (모멘텀 비교용)"),
            ],
            offensive_top_n: 4,
            defensive_top_n: 1,
            cash_symbol: "BIL".to_string(),
            invest_rate: dec!(1.0),
            rebalance_threshold: dec!(0.03),
        }
    }

    /// 모든 자산 심볼 가져오기.
    pub fn all_symbols(&self) -> Vec<String> {
        let mut symbols: Vec<String> = Vec::new();

        for asset in &self.canary_assets {
            if !symbols.contains(&asset.symbol) {
                symbols.push(asset.symbol.clone());
            }
        }

        for asset in &self.offensive_assets {
            if !symbols.contains(&asset.symbol) {
                symbols.push(asset.symbol.clone());
            }
        }

        for asset in &self.defensive_assets {
            if !symbols.contains(&asset.symbol) {
                symbols.push(asset.symbol.clone());
            }
        }

        symbols
    }
}

/// 자산 모멘텀 상태.
#[derive(Debug, Clone, Default)]
struct AssetMomentum {
    /// 종목 코드
    symbol: String,
    /// 모멘텀 스코어
    momentum_score: Decimal,
    /// 목표 비중 (%)
    target_weight: Decimal,
}

/// 포트폴리오 모드.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PortfolioMode {
    /// 공격 모드 (Risk 자산 투자)
    Offensive,
    /// 방어 모드 (Safe 자산 투자)
    Defensive,
}

/// HAA 전략.
pub struct HaaStrategy {
    config: Option<HaaConfig>,
    /// 자산별 가격 히스토리 (최신 가격이 앞에)
    price_history: HashMap<String, Vec<Decimal>>,
    /// 현재 포지션
    positions: HashMap<String, Decimal>,
    /// 마지막 리밸런싱 년월 (YYYY_MM)
    last_rebalance_ym: Option<String>,
    /// 리밸런싱 계산기
    rebalance_calculator: RebalanceCalculator,
    /// 현재 현금 잔고
    cash_balance: Decimal,
    /// 현재 포트폴리오 모드
    current_mode: PortfolioMode,
}

impl HaaStrategy {
    /// 새 전략 생성.
    pub fn new() -> Self {
        Self {
            config: None,
            price_history: HashMap::new(),
            positions: HashMap::new(),
            last_rebalance_ym: None,
            rebalance_calculator: RebalanceCalculator::new(RebalanceConfig::us_market()),
            cash_balance: Decimal::ZERO,
            current_mode: PortfolioMode::Offensive,
        }
    }

    /// 설정으로 전략 생성.
    pub fn with_config(config: HaaConfig) -> Self {
        let rebalance_config = match config.market {
            HaaMarketType::US => RebalanceConfig::us_market(),
            HaaMarketType::KR => RebalanceConfig::korean_market(),
        };

        Self {
            config: Some(config),
            price_history: HashMap::new(),
            positions: HashMap::new(),
            last_rebalance_ym: None,
            rebalance_calculator: RebalanceCalculator::new(rebalance_config),
            cash_balance: Decimal::ZERO,
            current_mode: PortfolioMode::Offensive,
        }
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
    /// 공식: (1M수익률) + (3M수익률) + (6M수익률) + (12M수익률)
    fn calculate_momentum_score(&self, symbol: &str) -> Option<Decimal> {
        let prices = self.price_history.get(symbol)?;

        // 최소 240일(12개월) 데이터 필요
        if prices.len() < 240 {
            debug!("[HAA] {} 데이터 부족: {}일", symbol, prices.len());
            return None;
        }

        let now_price = *prices.first()?;
        let one_month = *prices.get(20)?;   // 1개월 전 (20거래일)
        let three_month = *prices.get(60)?;  // 3개월 전 (60거래일)
        let six_month = *prices.get(120)?;   // 6개월 전 (120거래일)
        let twelve_month = *prices.get(239)?; // 12개월 전 (240거래일)

        // 0으로 나누기 방지
        if one_month.is_zero() || three_month.is_zero() || six_month.is_zero() || twelve_month.is_zero() {
            return None;
        }

        let ret_1m = (now_price - one_month) / one_month;
        let ret_3m = (now_price - three_month) / three_month;
        let ret_6m = (now_price - six_month) / six_month;
        let ret_12m = (now_price - twelve_month) / twelve_month;

        let score = ret_1m + ret_3m + ret_6m + ret_12m;

        debug!(
            "[HAA] {} 모멘텀 스코어: {:.4} (1M:{:.2}%, 3M:{:.2}%, 6M:{:.2}%, 12M:{:.2}%)",
            symbol,
            score,
            ret_1m * dec!(100),
            ret_3m * dec!(100),
            ret_6m * dec!(100),
            ret_12m * dec!(100)
        );

        Some(score)
    }

    /// 카나리아 자산 체크.
    ///
    /// 카나리아 자산의 모멘텀이 음수이면 방어 모드로 전환.
    fn check_canary_assets(&self, config: &HaaConfig) -> PortfolioMode {
        for asset in &config.canary_assets {
            if let Some(score) = self.calculate_momentum_score(&asset.symbol) {
                if score < Decimal::ZERO {
                    info!(
                        "[HAA] 카나리아 {} 모멘텀 음수 ({:.4}) → 방어 모드",
                        asset.symbol, score
                    );
                    return PortfolioMode::Defensive;
                }
            } else {
                // 데이터 부족 시 방어적으로 처리
                warn!(
                    "[HAA] 카나리아 {} 데이터 부족 → 방어 모드",
                    asset.symbol
                );
                return PortfolioMode::Defensive;
            }
        }

        info!("[HAA] 카나리아 모멘텀 양수 → 공격 모드");
        PortfolioMode::Offensive
    }

    /// 자산 순위 계산.
    fn rank_assets(&self, assets: &[AssetInfo]) -> Vec<AssetMomentum> {
        let mut ranked: Vec<AssetMomentum> = Vec::new();

        for asset in assets {
            if let Some(score) = self.calculate_momentum_score(&asset.symbol) {
                ranked.push(AssetMomentum {
                    symbol: asset.symbol.clone(),
                    momentum_score: score,
                    target_weight: Decimal::ZERO,
                });
            }
        }

        // 모멘텀 스코어 내림차순 정렬
        ranked.sort_by(|a, b| b.momentum_score.partial_cmp(&a.momentum_score).unwrap());

        ranked
    }

    /// 목표 비중 계산.
    fn calculate_target_weights(&mut self, config: &HaaConfig) -> Vec<TargetAllocation> {
        // 1. 카나리아 체크
        self.current_mode = self.check_canary_assets(config);

        let mut allocations: Vec<TargetAllocation> = Vec::new();

        match self.current_mode {
            PortfolioMode::Offensive => {
                // 공격 모드: Risk 자산 상위 N개 선택
                let ranked = self.rank_assets(&config.offensive_assets);
                let top_n = config.offensive_top_n.min(ranked.len());

                // 기본 비중 계산 (100% / N)
                let base_weight = if top_n > 0 {
                    dec!(1.0) / Decimal::from(top_n)
                } else {
                    Decimal::ZERO
                };

                let mut safe_overflow = Decimal::ZERO;

                // 상위 N개 자산 처리
                for (i, asset) in ranked.iter().take(top_n).enumerate() {
                    if asset.momentum_score > Decimal::ZERO {
                        // 모멘텀 양수 → 투자
                        allocations.push(TargetAllocation::new(
                            asset.symbol.clone(),
                            base_weight,
                        ));
                        info!(
                            "[HAA] 공격자산 #{}: {} (모멘텀: {:.4}, 비중: {:.1}%)",
                            i + 1,
                            asset.symbol,
                            asset.momentum_score,
                            base_weight * dec!(100)
                        );
                    } else {
                        // 모멘텀 음수 → 안전자산으로 비중 이전
                        safe_overflow += base_weight;
                        info!(
                            "[HAA] 공격자산 #{}: {} 모멘텀 음수 ({:.4}) → 비중 이전",
                            i + 1,
                            asset.symbol,
                            asset.momentum_score
                        );
                    }
                }

                // 안전자산으로 넘어간 비중 처리
                if safe_overflow > Decimal::ZERO {
                    self.add_defensive_allocation(config, &mut allocations, safe_overflow);
                }
            }
            PortfolioMode::Defensive => {
                // 방어 모드: Safe 자산 상위 1개 선택
                self.add_defensive_allocation(config, &mut allocations, dec!(1.0));
            }
        }

        allocations
    }

    /// 방어 자산 비중 추가.
    fn add_defensive_allocation(
        &self,
        config: &HaaConfig,
        allocations: &mut Vec<TargetAllocation>,
        weight: Decimal,
    ) {
        let ranked = self.rank_assets(&config.defensive_assets);

        if let Some(top_asset) = ranked.first() {
            if top_asset.symbol == config.cash_symbol {
                // BIL이 선택되면 현금 보유 (투자 안 함)
                info!(
                    "[HAA] 방어자산: {} (현금) → 투자 안 함, 비중 {:.1}% 현금 보유",
                    top_asset.symbol,
                    weight * dec!(100)
                );
            } else {
                // 기존 할당에 추가하거나 새로 생성
                let existing = allocations.iter_mut().find(|a| a.symbol == top_asset.symbol);

                if let Some(existing) = existing {
                    existing.weight += weight;
                    info!(
                        "[HAA] 방어자산: {} 비중 추가 → 총 {:.1}%",
                        top_asset.symbol,
                        existing.weight * dec!(100)
                    );
                } else {
                    allocations.push(TargetAllocation::new(
                        top_asset.symbol.clone(),
                        weight,
                    ));
                    info!(
                        "[HAA] 방어자산: {} (모멘텀: {:.4}, 비중: {:.1}%)",
                        top_asset.symbol,
                        top_asset.momentum_score,
                        weight * dec!(100)
                    );
                }
            }
        }
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
        config: &HaaConfig,
        current_time: DateTime<Utc>,
    ) -> Vec<Signal> {
        if !self.should_rebalance(current_time) {
            return Vec::new();
        }

        // 목표 비중 계산
        let target_allocations = self.calculate_target_weights(config);

        // 현재 포지션을 PortfolioPosition으로 변환
        let mut portfolio_positions: Vec<PortfolioPosition> = Vec::new();

        for (symbol, quantity) in &self.positions {
            if let Some(prices) = self.price_history.get(symbol) {
                if let Some(current_price) = prices.first() {
                    portfolio_positions.push(PortfolioPosition::new(symbol, *quantity, *current_price));
                }
            }
        }

        // 현금 포지션 추가
        let cash_symbol = match config.market {
            HaaMarketType::US => "USD",
            HaaMarketType::KR => "KRW",
        };
        portfolio_positions.push(PortfolioPosition::cash(self.cash_balance, cash_symbol));

        // 리밸런싱 계산
        let result = self.rebalance_calculator.calculate_orders_with_cash_constraint(
            &portfolio_positions,
            &target_allocations,
        );

        // 신호 변환
        let mut signals = Vec::new();

        for order in result.orders {
            let side = match order.side {
                RebalanceOrderSide::Buy => Side::Buy,
                RebalanceOrderSide::Sell => Side::Sell,
            };

            let quote_currency = match config.market {
                HaaMarketType::US => "USD",
                HaaMarketType::KR => "KRW",
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
            .with_metadata("mode", json!(format!("{:?}", self.current_mode)))
            .with_metadata("reason", json!("monthly_rebalance"));

            signals.push(signal);
        }

        // 리밸런싱 시간 기록
        if !signals.is_empty() {
            self.last_rebalance_ym = Some(format!("{}_{}", current_time.year(), current_time.month()));
            info!(
                "[HAA] 리밸런싱 완료: {} 주문 생성 (모드: {:?})",
                signals.len(),
                self.current_mode
            );
        }

        signals
    }
}

impl Default for HaaStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for HaaStrategy {
    fn name(&self) -> &str {
        "HAA"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "계층적 자산배분(HAA) 전략. 카나리아 자산으로 위험 감지, 모멘텀 기반 자산 선택, 월간 리밸런싱."
    }

    async fn initialize(&mut self, config: Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let parsed_config: HaaConfig = serde_json::from_value(config.clone())?;

        let rebalance_config = match parsed_config.market {
            HaaMarketType::US => RebalanceConfig::us_market(),
            HaaMarketType::KR => RebalanceConfig::korean_market(),
        };
        self.rebalance_calculator = RebalanceCalculator::new(rebalance_config);

        // initial_capital이 있으면 cash_balance로 설정
        if let Some(capital_str) = config.get("initial_capital") {
            if let Some(capital) = capital_str.as_str() {
                if let Ok(capital_dec) = capital.parse::<Decimal>() {
                    self.cash_balance = capital_dec;
                    info!("[HAA] 초기 자본금 설정: {}", capital_dec);
                }
            }
        }

        info!(
            "[HAA] 전략 초기화 - 시장: {:?}, 카나리아: {:?}, 공격자산: {}개, 방어자산: {}개, 초기자본: {}",
            parsed_config.market,
            parsed_config.canary_assets.iter().map(|a| &a.symbol).collect::<Vec<_>>(),
            parsed_config.offensive_assets.len(),
            parsed_config.defensive_assets.len(),
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
            debug!("[HAA] 가격 업데이트: {} = {}", symbol, price);
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
            "[HAA] 주문 체결: {:?} {} {} @ {:?}",
            order.side,
            order.quantity,
            order.symbol,
            order.average_fill_price
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
            "[HAA] 포지션 업데이트: {} = {} (PnL: {})",
            symbol,
            position.quantity,
            position.unrealized_pnl
        );
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("[HAA] 전략 종료");
        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "name": self.name(),
            "version": self.version(),
            "current_mode": format!("{:?}", self.current_mode),
            "last_rebalance_ym": self.last_rebalance_ym,
            "positions": self.positions.iter()
                .map(|(k, v)| (k.clone(), v.to_string()))
                .collect::<HashMap<_, _>>(),
            "cash_balance": self.cash_balance.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_us_default() {
        let config = HaaConfig::us_default();
        assert_eq!(config.market, HaaMarketType::US);
        assert_eq!(config.canary_assets.len(), 1);
        assert_eq!(config.canary_assets[0].symbol, "TIP");
        assert_eq!(config.offensive_assets.len(), 8);
        assert_eq!(config.defensive_assets.len(), 2);
        assert_eq!(config.offensive_top_n, 4);
        assert_eq!(config.defensive_top_n, 1);
        assert_eq!(config.cash_symbol, "BIL");
    }

    #[test]
    fn test_config_kr_default() {
        let config = HaaConfig::kr_default();
        assert_eq!(config.market, HaaMarketType::KR);
        assert_eq!(config.offensive_assets.len(), 8);
    }

    #[test]
    fn test_all_symbols() {
        let config = HaaConfig::us_default();
        let symbols = config.all_symbols();

        assert!(symbols.contains(&"TIP".to_string()));
        assert!(symbols.contains(&"SPY".to_string()));
        assert!(symbols.contains(&"IEF".to_string()));
        assert!(symbols.contains(&"BIL".to_string()));

        // 중복 제거 확인 (IEF는 공격/방어 모두에 있음)
        let ief_count = symbols.iter().filter(|s| *s == "IEF").count();
        assert_eq!(ief_count, 1);
    }

    #[test]
    fn test_asset_info_creation() {
        let canary = AssetInfo::canary("TIP", "TIPS ETF");
        assert_eq!(canary.asset_type, AssetType::Canary);

        let offensive = AssetInfo::offensive("SPY", "S&P 500");
        assert_eq!(offensive.asset_type, AssetType::Offensive);

        let defensive = AssetInfo::defensive("IEF", "Treasury");
        assert_eq!(defensive.asset_type, AssetType::Defensive);

        let cash = AssetInfo::cash("BIL", "Cash proxy");
        assert_eq!(cash.asset_type, AssetType::Cash);
    }

    #[test]
    fn test_strategy_creation() {
        let strategy = HaaStrategy::new();
        assert_eq!(strategy.name(), "HAA");
        assert_eq!(strategy.version(), "1.0.0");
        assert_eq!(strategy.current_mode, PortfolioMode::Offensive);
    }

    #[test]
    fn test_should_rebalance_first_time() {
        let strategy = HaaStrategy::new();
        let now = Utc::now();
        assert!(strategy.should_rebalance(now));
    }

    #[test]
    fn test_should_rebalance_same_month() {
        let mut strategy = HaaStrategy::new();
        let now = Utc::now();
        strategy.last_rebalance_ym = Some(format!("{}_{}", now.year(), now.month()));
        assert!(!strategy.should_rebalance(now));
    }

    #[test]
    fn test_update_price_history() {
        let mut strategy = HaaStrategy::new();
        strategy.update_price_history("SPY", dec!(400));
        strategy.update_price_history("SPY", dec!(401));
        strategy.update_price_history("SPY", dec!(402));

        let history = strategy.price_history.get("SPY").unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0], dec!(402)); // 최신 가격이 앞에
    }

    #[test]
    fn test_momentum_score_insufficient_data() {
        let strategy = HaaStrategy::new();
        // 데이터 없으면 None
        let score = strategy.calculate_momentum_score("SPY");
        assert!(score.is_none());
    }

    #[test]
    fn test_momentum_score_calculation() {
        let mut strategy = HaaStrategy::new();

        // 상승 추세 데이터 생성 (240일)
        let prices: Vec<Decimal> = (0..250)
            .rev()
            .map(|i| dec!(100) + Decimal::from(i) * dec!(0.1))
            .collect();
        strategy.price_history.insert("SPY".to_string(), prices);

        let score = strategy.calculate_momentum_score("SPY");
        assert!(score.is_some());
        // 상승 추세이므로 양수
        assert!(score.unwrap() > Decimal::ZERO);
    }

    #[test]
    fn test_rank_assets() {
        let mut strategy = HaaStrategy::new();

        // 테스트 데이터: SPY > VEA > IWM
        let spy_prices: Vec<Decimal> = (0..250).rev().map(|i| dec!(100) + Decimal::from(i) * dec!(0.2)).collect();
        let vea_prices: Vec<Decimal> = (0..250).rev().map(|i| dec!(100) + Decimal::from(i) * dec!(0.1)).collect();
        let iwm_prices: Vec<Decimal> = (0..250).rev().map(|i| dec!(100) + Decimal::from(i) * dec!(0.05)).collect();

        strategy.price_history.insert("SPY".to_string(), spy_prices);
        strategy.price_history.insert("VEA".to_string(), vea_prices);
        strategy.price_history.insert("IWM".to_string(), iwm_prices);

        let assets = vec![
            AssetInfo::offensive("SPY", "S&P 500"),
            AssetInfo::offensive("VEA", "Developed"),
            AssetInfo::offensive("IWM", "Russell 2000"),
        ];

        let ranked = strategy.rank_assets(&assets);

        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].symbol, "SPY"); // 가장 높은 모멘텀
        assert_eq!(ranked[1].symbol, "VEA");
        assert_eq!(ranked[2].symbol, "IWM");
    }

    #[test]
    fn test_check_canary_positive() {
        let mut strategy = HaaStrategy::new();
        let config = HaaConfig::us_default();

        // TIP 상승 추세 데이터
        let tip_prices: Vec<Decimal> = (0..250).rev().map(|i| dec!(100) + Decimal::from(i) * dec!(0.1)).collect();
        strategy.price_history.insert("TIP".to_string(), tip_prices);

        let mode = strategy.check_canary_assets(&config);
        assert_eq!(mode, PortfolioMode::Offensive);
    }

    #[test]
    fn test_check_canary_negative() {
        let mut strategy = HaaStrategy::new();
        let config = HaaConfig::us_default();

        // TIP 하락 추세 데이터
        let tip_prices: Vec<Decimal> = (0..250).map(|i| dec!(100) + Decimal::from(i) * dec!(0.1)).collect();
        strategy.price_history.insert("TIP".to_string(), tip_prices);

        let mode = strategy.check_canary_assets(&config);
        assert_eq!(mode, PortfolioMode::Defensive);
    }

    #[test]
    fn test_get_state() {
        let strategy = HaaStrategy::new();
        let state = strategy.get_state();

        assert_eq!(state["name"], "HAA");
        assert_eq!(state["version"], "1.0.0");
        assert_eq!(state["current_mode"], "Offensive");
    }
}
