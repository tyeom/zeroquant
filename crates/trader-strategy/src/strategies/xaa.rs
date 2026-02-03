//! XAA (Extended Asset Allocation) 전략 구현.
//!
//! HAA의 확장 버전으로 채권 자산에 대한 별도 모멘텀 계산을 수행합니다.
//!
//! Python 39/40번 전략 변환.
//!
//! # 전략 개요
//!
//! ## HAA와의 차이점
//! - **모멘텀 계산**: (1M + 3M + 6M + 12M) / 4 (평균)
//! - **Momentum6**: 6개월 모멘텀 별도 계산 (채권 자산용)
//! - **음수 모멘텀 처리**: 50% 방어 + 50% 1위 공격자산
//!
//! ## 자산 분류
//! - **카나리아 (BIRD)**: TIP - 위험 감지용
//! - **공격 자산 (RISK)**: SPY, IWM, VEA, VWO, VNQ, PDBC 등 (채권 제외)
//! - **채권 자산 (BOND)**: TLT, IEF (Momentum6으로 랭킹)
//! - **현금 (CASH)**: BIL (채권 비교 기준)
//!
//! ## 모멘텀 계산
//! ```text
//! Momentum = (1M + 3M + 6M + 12M) / 4
//! Momentum6 = 6개월 수익률 (채권 전용)
//! ```
//!
//! ## 자산 선택 로직
//! 1. 카나리아(TIP) 모멘텀 < 0 → 방어 모드
//! 2. 공격 모드:
//!    - 공격 자산 중 Momentum 상위 4개
//!    - 채권 자산 중 Momentum6 상위 3개 (BIL보다 높을 때만)
//!    - 음수 모멘텀 공격 자산: 50% 방어, 50% 1위 공격자산
//! 3. 방어 모드: 안전 자산 100%
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
use tracing::{debug, info, warn};
use trader_core::domain::{RouteState, StrategyContext};

use crate::strategies::common::rebalance::{
    PortfolioPosition, RebalanceCalculator, RebalanceConfig, RebalanceOrderSide, TargetAllocation,
};
use crate::traits::Strategy;
use trader_core::{MarketData, MarketDataType, Order, Position, Side, Signal, SignalType, Symbol};

/// XAA 자산 타입.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum XaaAssetType {
    /// 카나리아 자산 (위험 감지용)
    Canary,
    /// 공격 자산 (Risk, 채권 제외)
    Offensive,
    /// 채권 자산 (Bond, Momentum6 사용)
    Bond,
    /// 안전 자산 (방어 모드시 투자)
    Safe,
    /// 현금 대용 (BIL, 채권 비교 기준)
    Cash,
}

/// XAA 자산 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XaaAssetInfo {
    /// 종목 코드
    pub symbol: String,
    /// 자산 타입
    pub asset_type: XaaAssetType,
    /// 설명
    pub description: String,
}

impl XaaAssetInfo {
    /// 새 자산 정보 생성.
    pub fn new(
        symbol: impl Into<String>,
        asset_type: XaaAssetType,
        description: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            asset_type,
            description: description.into(),
        }
    }

    /// 카나리아 자산 생성.
    pub fn canary(symbol: impl Into<String>, description: impl Into<String>) -> Self {
        Self::new(symbol, XaaAssetType::Canary, description)
    }

    /// 공격 자산 생성 (채권 제외).
    pub fn offensive(symbol: impl Into<String>, description: impl Into<String>) -> Self {
        Self::new(symbol, XaaAssetType::Offensive, description)
    }

    /// 채권 자산 생성.
    pub fn bond(symbol: impl Into<String>, description: impl Into<String>) -> Self {
        Self::new(symbol, XaaAssetType::Bond, description)
    }

    /// 안전 자산 생성.
    pub fn safe(symbol: impl Into<String>, description: impl Into<String>) -> Self {
        Self::new(symbol, XaaAssetType::Safe, description)
    }

    /// 현금 대용 자산 생성.
    pub fn cash(symbol: impl Into<String>, description: impl Into<String>) -> Self {
        Self::new(symbol, XaaAssetType::Cash, description)
    }
}

/// XAA 전략 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XaaConfig {
    /// 시장 타입 (US/KR)
    pub market: XaaMarketType,

    /// 카나리아 자산 목록
    pub canary_assets: Vec<XaaAssetInfo>,

    /// 공격 자산 목록 (채권 제외)
    pub offensive_assets: Vec<XaaAssetInfo>,

    /// 채권 자산 목록
    pub bond_assets: Vec<XaaAssetInfo>,

    /// 안전 자산 목록
    pub safe_assets: Vec<XaaAssetInfo>,

    /// 공격 자산 투자 개수 (기본: 4)
    pub offensive_top_n: usize,

    /// 채권 자산 투자 개수 (기본: 3)
    pub bond_top_n: usize,

    /// 현금 대용 심볼 (BIL, 채권 비교 기준)
    pub cash_symbol: String,

    /// 투자 비율 (총 자산 대비)
    pub invest_rate: Decimal,

    /// 리밸런싱 임계값
    pub rebalance_threshold: Decimal,

    /// 최소 GlobalScore (기본값: 60)
    #[serde(default = "default_min_global_score")]
    pub min_global_score: Decimal,
}

/// 기본 최소 GlobalScore 값.
fn default_min_global_score() -> Decimal {
    dec!(60)
}

/// 시장 타입.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum XaaMarketType {
    /// 미국 시장
    US,
    /// 한국 시장
    KR,
}

impl Default for XaaConfig {
    fn default() -> Self {
        Self::us_default()
    }
}

impl XaaConfig {
    /// 미국 시장 기본 설정.
    ///
    /// Python XAA_Us_Bot.py 기준:
    /// - 공격자산: SPY, IWM, VEA, VWO, VNQ, PDBC
    /// - 채권자산: TLT, IEF
    /// - 안전자산: IEF, BIL
    /// - 카나리아: TIP
    pub fn us_default() -> Self {
        Self {
            market: XaaMarketType::US,
            canary_assets: vec![XaaAssetInfo::canary("TIP", "iShares TIPS Bond ETF")],
            offensive_assets: vec![
                XaaAssetInfo::offensive("SPY", "S&P 500 ETF"),
                XaaAssetInfo::offensive("IWM", "Russell 2000 ETF"),
                XaaAssetInfo::offensive("VEA", "Vanguard Developed Markets ETF"),
                XaaAssetInfo::offensive("VWO", "Vanguard Emerging Markets ETF"),
                XaaAssetInfo::offensive("VNQ", "Vanguard Real Estate ETF"),
                XaaAssetInfo::offensive("PDBC", "Invesco DB Commodity Index ETF"),
            ],
            bond_assets: vec![
                XaaAssetInfo::bond("TLT", "iShares 20+ Year Treasury ETF"),
                XaaAssetInfo::bond("IEF", "iShares 7-10 Year Treasury ETF"),
            ],
            safe_assets: vec![XaaAssetInfo::safe("IEF", "iShares 7-10 Year Treasury ETF")],
            offensive_top_n: 4,
            bond_top_n: 3,
            cash_symbol: "BIL".to_string(),
            invest_rate: dec!(1.0),
            rebalance_threshold: dec!(0.03),
            min_global_score: default_min_global_score(),
        }
    }

    /// 한국 시장 설정.
    ///
    /// Python XAA_Kr_Bot.py 기준으로 한국 ETF로 대체.
    pub fn kr_default() -> Self {
        Self {
            market: XaaMarketType::KR,
            canary_assets: vec![
                // 모멘텀 계산용 (실제로는 한국 ETF로 대체)
                XaaAssetInfo::canary("TIP", "iShares TIPS (모멘텀 계산용)"),
            ],
            offensive_assets: vec![
                XaaAssetInfo::offensive("360750", "TIGER 미국S&P500"),
                XaaAssetInfo::offensive("280930", "TIGER 미국나스닥100"),
                XaaAssetInfo::offensive("251350", "KODEX 선진국MSCI World"),
                XaaAssetInfo::offensive("195980", "ARIRANG 신흥국MSCI"),
                XaaAssetInfo::offensive("352560", "TIGER 미국필라델피아반도체나스닥"),
                XaaAssetInfo::offensive("276000", "KBSTAR 미국S&P원유생산기업"),
            ],
            bond_assets: vec![
                XaaAssetInfo::bond("304660", "KODEX 미국채울트라30년선물(H)"),
                XaaAssetInfo::bond("305080", "TIGER 미국채10년선물"),
            ],
            safe_assets: vec![XaaAssetInfo::safe("305080", "TIGER 미국채10년선물")],
            offensive_top_n: 4,
            bond_top_n: 3,
            cash_symbol: "BIL".to_string(),
            invest_rate: dec!(1.0),
            rebalance_threshold: dec!(0.03),
            min_global_score: default_min_global_score(),
        }
    }

    /// 모든 자산 심볼 가져오기.
    pub fn all_symbols(&self) -> Vec<String> {
        let mut symbols: Vec<String> = Vec::new();

        // 카나리아 자산
        for asset in &self.canary_assets {
            if !symbols.contains(&asset.symbol) {
                symbols.push(asset.symbol.clone());
            }
        }

        // 공격 자산
        for asset in &self.offensive_assets {
            if !symbols.contains(&asset.symbol) {
                symbols.push(asset.symbol.clone());
            }
        }

        // 채권 자산
        for asset in &self.bond_assets {
            if !symbols.contains(&asset.symbol) {
                symbols.push(asset.symbol.clone());
            }
        }

        // 안전 자산
        for asset in &self.safe_assets {
            if !symbols.contains(&asset.symbol) {
                symbols.push(asset.symbol.clone());
            }
        }

        // 현금 자산 (BIL)
        if !symbols.contains(&self.cash_symbol) {
            symbols.push(self.cash_symbol.clone());
        }

        symbols
    }
}

/// 자산 모멘텀 상태.
#[derive(Debug, Clone, Default)]
struct AssetMomentum {
    /// 종목 코드
    symbol: String,
    /// 모멘텀 스코어 (4기간 평균)
    momentum: Decimal,
    /// 6개월 모멘텀 (채권용)
    momentum6: Decimal,
    /// 목표 비중 (%)
    target_weight: Decimal,
}

/// 포트폴리오 모드.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PortfolioMode {
    /// 공격 모드 (Risk + Bond 자산 투자)
    Offensive,
    /// 방어 모드 (Safe 자산 투자)
    Defensive,
}

/// XAA 전략.
pub struct XaaStrategy {
    config: Option<XaaConfig>,
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
    /// 전략 컨텍스트 (RouteState, GlobalScore 조회용)
    context: Option<Arc<RwLock<StrategyContext>>>,
}

impl XaaStrategy {
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
            context: None,
        }
    }

    /// 설정으로 전략 생성.
    pub fn with_config(config: XaaConfig) -> Self {
        let rebalance_config = match config.market {
            XaaMarketType::US => RebalanceConfig::us_market(),
            XaaMarketType::KR => RebalanceConfig::korean_market(),
        };

        Self {
            config: Some(config),
            price_history: HashMap::new(),
            positions: HashMap::new(),
            last_rebalance_ym: None,
            rebalance_calculator: RebalanceCalculator::new(rebalance_config),
            cash_balance: Decimal::ZERO,
            current_mode: PortfolioMode::Offensive,
            context: None,
        }
    }

    /// 특정 종목의 진입 가능 여부 확인.
    ///
    /// RouteState와 GlobalScore를 기반으로 매수 신호 생성 가능 여부 판단.
    fn can_enter(&self, ticker: &str) -> bool {
        let Some(config) = self.config.as_ref() else {
            return true; // 설정 없으면 기본 허용
        };

        let Some(ctx) = self.context.as_ref() else {
            return true; // 컨텍스트 없으면 기본 허용
        };

        let Ok(ctx_lock) = ctx.try_read() else {
            return true; // 락 획득 실패 시 기본 허용
        };

        // RouteState 확인 - Attack/Armed만 진입 허용
        if let Some(state) = ctx_lock.get_route_state(ticker) {
            match state {
                RouteState::Attack | RouteState::Armed => {
                    // 진입 가능
                }
                RouteState::Overheat | RouteState::Wait | RouteState::Neutral => {
                    debug!(
                        "[XAA] {} RouteState {:?} - 진입 불가",
                        ticker, state
                    );
                    return false;
                }
            }
        }

        // GlobalScore 확인
        if let Some(score) = ctx_lock.get_global_score(ticker) {
            if score.overall_score < config.min_global_score {
                debug!(
                    "[XAA] {} GlobalScore {:.1} < {:.1} - 진입 불가",
                    ticker, score.overall_score, config.min_global_score
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

    /// 모멘텀 스코어 계산 (평균).
    ///
    /// 공식: (1M + 3M + 6M + 12M) / 4
    /// HAA와 다르게 4로 나눠 평균을 구함.
    fn calculate_momentum(&self, symbol: &str) -> Option<Decimal> {
        let prices = self.price_history.get(symbol)?;

        // 최소 240일(12개월) 데이터 필요
        if prices.len() < 240 {
            debug!("[XAA] {} 데이터 부족: {}일", symbol, prices.len());
            return None;
        }

        let now_price = *prices.first()?;
        let one_month = *prices.get(20)?; // 1개월 전 (20거래일)
        let three_month = *prices.get(60)?; // 3개월 전 (60거래일)
        let six_month = *prices.get(120)?; // 6개월 전 (120거래일)
        let twelve_month = *prices.get(239)?; // 12개월 전 (240거래일)

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

        // XAA는 4로 나눠 평균 계산
        let momentum = (ret_1m + ret_3m + ret_6m + ret_12m) / dec!(4);

        debug!(
            "[XAA] {} Momentum: {:.4} (1M:{:.2}%, 3M:{:.2}%, 6M:{:.2}%, 12M:{:.2}%)",
            symbol,
            momentum,
            ret_1m * dec!(100),
            ret_3m * dec!(100),
            ret_6m * dec!(100),
            ret_12m * dec!(100)
        );

        Some(momentum)
    }

    /// 6개월 모멘텀 계산 (채권 전용).
    ///
    /// 공식: 6개월 수익률
    fn calculate_momentum6(&self, symbol: &str) -> Option<Decimal> {
        let prices = self.price_history.get(symbol)?;

        // 최소 120일(6개월) 데이터 필요
        if prices.len() < 121 {
            debug!("[XAA] {} Momentum6 데이터 부족: {}일", symbol, prices.len());
            return None;
        }

        let now_price = *prices.first()?;
        let six_month = *prices.get(120)?;

        if six_month.is_zero() {
            return None;
        }

        let momentum6 = (now_price - six_month) / six_month;

        debug!(
            "[XAA] {} Momentum6: {:.4} ({:.2}%)",
            symbol,
            momentum6,
            momentum6 * dec!(100)
        );

        Some(momentum6)
    }

    /// 카나리아 자산 체크.
    ///
    /// 카나리아 자산의 모멘텀이 음수이면 방어 모드로 전환.
    fn check_canary_assets(&self, config: &XaaConfig) -> PortfolioMode {
        for asset in &config.canary_assets {
            if let Some(momentum) = self.calculate_momentum(&asset.symbol) {
                if momentum < Decimal::ZERO {
                    info!(
                        "[XAA] 카나리아 {} 모멘텀 음수 ({:.4}) → 방어 모드",
                        asset.symbol, momentum
                    );
                    return PortfolioMode::Defensive;
                }
            } else {
                // 데이터 부족 시 방어적으로 처리
                warn!("[XAA] 카나리아 {} 데이터 부족 → 방어 모드", asset.symbol);
                return PortfolioMode::Defensive;
            }
        }

        info!("[XAA] 카나리아 모멘텀 양수 → 공격 모드");
        PortfolioMode::Offensive
    }

    /// 공격 자산 순위 계산 (Momentum 기준).
    fn rank_offensive_assets(&self, assets: &[XaaAssetInfo]) -> Vec<AssetMomentum> {
        let mut ranked: Vec<AssetMomentum> = Vec::new();

        for asset in assets {
            if let Some(momentum) = self.calculate_momentum(&asset.symbol) {
                let momentum6 = self
                    .calculate_momentum6(&asset.symbol)
                    .unwrap_or(Decimal::ZERO);
                ranked.push(AssetMomentum {
                    symbol: asset.symbol.clone(),
                    momentum,
                    momentum6,
                    target_weight: Decimal::ZERO,
                });
            }
        }

        // Momentum 내림차순 정렬
        ranked.sort_by(|a, b| b.momentum.cmp(&a.momentum));

        ranked
    }

    /// 채권 자산 순위 계산 (Momentum6 기준).
    fn rank_bond_assets(&self, assets: &[XaaAssetInfo]) -> Vec<AssetMomentum> {
        let mut ranked: Vec<AssetMomentum> = Vec::new();

        for asset in assets {
            if let Some(momentum6) = self.calculate_momentum6(&asset.symbol) {
                let momentum = self
                    .calculate_momentum(&asset.symbol)
                    .unwrap_or(Decimal::ZERO);
                ranked.push(AssetMomentum {
                    symbol: asset.symbol.clone(),
                    momentum,
                    momentum6,
                    target_weight: Decimal::ZERO,
                });
            }
        }

        // Momentum6 내림차순 정렬
        ranked.sort_by(|a, b| b.momentum6.cmp(&a.momentum6));

        ranked
    }

    /// BIL 모멘텀6 가져오기.
    fn get_bil_momentum6(&self, config: &XaaConfig) -> Decimal {
        self.calculate_momentum6(&config.cash_symbol)
            .unwrap_or(Decimal::ZERO)
    }

    /// 목표 비중 계산.
    fn calculate_target_weights(&mut self, config: &XaaConfig) -> Vec<TargetAllocation> {
        // 1. 카나리아 체크
        self.current_mode = self.check_canary_assets(config);

        let mut allocations: Vec<TargetAllocation> = Vec::new();

        match self.current_mode {
            PortfolioMode::Offensive => {
                // 공격 모드
                self.calculate_offensive_weights(config, &mut allocations);
            }
            PortfolioMode::Defensive => {
                // 방어 모드: 안전 자산 100%
                self.calculate_defensive_weights(config, &mut allocations, dec!(1.0));
            }
        }

        allocations
    }

    /// 공격 모드 비중 계산.
    fn calculate_offensive_weights(
        &self,
        config: &XaaConfig,
        allocations: &mut Vec<TargetAllocation>,
    ) {
        // 공격 자산 순위 (Momentum 기준)
        let ranked_offensive = self.rank_offensive_assets(&config.offensive_assets);
        let top_n = config.offensive_top_n.min(ranked_offensive.len());

        // 채권 자산 순위 (Momentum6 기준)
        let ranked_bonds = self.rank_bond_assets(&config.bond_assets);
        let bil_momentum6 = self.get_bil_momentum6(config);

        // 전체 자산 개수 계산 (공격 + 채권)
        let total_offensive = top_n;
        let total_bonds = config.bond_top_n.min(ranked_bonds.len());
        let total_slots = total_offensive + total_bonds;

        if total_slots == 0 {
            return;
        }

        // 기본 비중 (100% / 전체 자산 수)
        let base_weight = dec!(1.0) / Decimal::from(total_slots);

        // 1위 공격 자산 (음수 모멘텀 재분배용)
        let top_offensive_symbol = ranked_offensive.first().map(|a| a.symbol.clone());

        // 방어로 이전할 비중
        let mut safe_overflow = Decimal::ZERO;
        // 1위 공격자산으로 이전할 비중
        let mut top_overflow = Decimal::ZERO;

        // 공격 자산 처리
        for (i, asset) in ranked_offensive.iter().take(top_n).enumerate() {
            if asset.momentum > Decimal::ZERO {
                // 모멘텀 양수 → 투자
                self.add_or_update_allocation(allocations, &asset.symbol, base_weight);
                info!(
                    "[XAA] 공격자산 #{}: {} (Momentum: {:.4}, 비중: {:.1}%)",
                    i + 1,
                    asset.symbol,
                    asset.momentum,
                    base_weight * dec!(100)
                );
            } else {
                // 모멘텀 음수 → 50% 방어, 50% 1위 공격자산
                let half_weight = base_weight / dec!(2);
                safe_overflow += half_weight;
                top_overflow += half_weight;
                info!(
                    "[XAA] 공격자산 #{}: {} 모멘텀 음수 ({:.4}) → 50% 방어, 50% 1위로 이전",
                    i + 1,
                    asset.symbol,
                    asset.momentum
                );
            }
        }

        // 1위 공격자산에 재분배된 비중 추가
        if top_overflow > Decimal::ZERO {
            if let Some(top_symbol) = &top_offensive_symbol {
                // 1위 자산이 이미 있으면 비중 추가
                if let Some(existing) = allocations.iter_mut().find(|a| &a.symbol == top_symbol) {
                    existing.weight += top_overflow;
                    info!(
                        "[XAA] 1위 공격자산 {} 비중 추가: {:.1}% → 총 {:.1}%",
                        top_symbol,
                        top_overflow * dec!(100),
                        existing.weight * dec!(100)
                    );
                }
            }
        }

        // 채권 자산 처리 (BIL 모멘텀6보다 높을 때만)
        for (i, asset) in ranked_bonds.iter().take(total_bonds).enumerate() {
            if asset.momentum6 > bil_momentum6 {
                // BIL보다 모멘텀6가 높으면 투자
                self.add_or_update_allocation(allocations, &asset.symbol, base_weight);
                info!(
                    "[XAA] 채권자산 #{}: {} (Momentum6: {:.4} > BIL {:.4}, 비중: {:.1}%)",
                    i + 1,
                    asset.symbol,
                    asset.momentum6,
                    bil_momentum6,
                    base_weight * dec!(100)
                );
            } else {
                // BIL보다 낮으면 현금 보유 (투자 안 함)
                info!(
                    "[XAA] 채권자산 #{}: {} (Momentum6: {:.4} <= BIL {:.4}) → 현금 보유",
                    i + 1,
                    asset.symbol,
                    asset.momentum6,
                    bil_momentum6
                );
            }
        }

        // 방어 자산으로 넘어간 비중 처리
        if safe_overflow > Decimal::ZERO {
            self.calculate_defensive_weights(config, allocations, safe_overflow);
        }
    }

    /// 방어 자산 비중 계산.
    fn calculate_defensive_weights(
        &self,
        config: &XaaConfig,
        allocations: &mut Vec<TargetAllocation>,
        weight: Decimal,
    ) {
        // 안전 자산 중 첫 번째 선택 (보통 IEF)
        if let Some(safe_asset) = config.safe_assets.first() {
            self.add_or_update_allocation(allocations, &safe_asset.symbol, weight);
            info!(
                "[XAA] 방어자산: {} (비중: {:.1}%)",
                safe_asset.symbol,
                weight * dec!(100)
            );
        }
    }

    /// 할당 추가 또는 업데이트.
    fn add_or_update_allocation(
        &self,
        allocations: &mut Vec<TargetAllocation>,
        symbol: &str,
        weight: Decimal,
    ) {
        if let Some(existing) = allocations.iter_mut().find(|a| a.symbol == symbol) {
            existing.weight += weight;
        } else {
            allocations.push(TargetAllocation::new(symbol.to_string(), weight));
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
        config: &XaaConfig,
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
            XaaMarketType::US => "USD",
            XaaMarketType::KR => "KRW",
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

            // 매수 신호인 경우 RouteState/GlobalScore 확인
            if side == Side::Buy && !self.can_enter(&order.symbol) {
                debug!(
                    "[XAA] {} 매수 신호 스킵 - RouteState/GlobalScore 조건 미충족",
                    order.symbol
                );
                continue;
            }

            let quote_currency = match config.market {
                XaaMarketType::US => "USD",
                XaaMarketType::KR => "KRW",
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
            self.last_rebalance_ym =
                Some(format!("{}_{}", current_time.year(), current_time.month()));
            info!(
                "[XAA] 리밸런싱 완료: {} 주문 생성 (모드: {:?})",
                signals.len(),
                self.current_mode
            );
        }

        signals
    }
}

impl Default for XaaStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for XaaStrategy {
    fn name(&self) -> &str {
        "XAA"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "확장 자산배분(XAA) 전략. HAA 확장 버전으로 채권 별도 모멘텀 계산, 월간 리밸런싱."
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let parsed_config: XaaConfig = serde_json::from_value(config.clone())?;

        let rebalance_config = match parsed_config.market {
            XaaMarketType::US => RebalanceConfig::us_market(),
            XaaMarketType::KR => RebalanceConfig::korean_market(),
        };
        self.rebalance_calculator = RebalanceCalculator::new(rebalance_config);

        // initial_capital이 있으면 cash_balance로 설정
        if let Some(capital_str) = config.get("initial_capital") {
            if let Some(capital) = capital_str.as_str() {
                if let Ok(capital_dec) = capital.parse::<Decimal>() {
                    self.cash_balance = capital_dec;
                    info!("[XAA] 초기 자본금 설정: {}", capital_dec);
                }
            }
        }

        info!(
            "[XAA] 전략 초기화 - 시장: {:?}, 카나리아: {:?}, 공격자산: {}개, 채권자산: {}개, 초기자본: {}",
            parsed_config.market,
            parsed_config.canary_assets.iter().map(|a| &a.symbol).collect::<Vec<_>>(),
            parsed_config.offensive_assets.len(),
            parsed_config.bond_assets.len(),
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
            debug!("[XAA] 가격 업데이트: {} = {}", symbol, price);
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
            "[XAA] 주문 체결: {:?} {} {} @ {:?}",
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
            "[XAA] 포지션 업데이트: {} = {} (PnL: {})",
            symbol, position.quantity, position.unrealized_pnl
        );
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("[XAA] 전략 종료");
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

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("[XAA] StrategyContext 주입 완료");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_us_default() {
        let config = XaaConfig::us_default();
        assert_eq!(config.market, XaaMarketType::US);
        assert_eq!(config.canary_assets.len(), 1);
        assert_eq!(config.canary_assets[0].symbol, "TIP");
        assert_eq!(config.offensive_assets.len(), 6);
        assert_eq!(config.bond_assets.len(), 2);
        assert_eq!(config.offensive_top_n, 4);
        assert_eq!(config.bond_top_n, 3);
        assert_eq!(config.cash_symbol, "BIL");
    }

    #[test]
    fn test_config_kr_default() {
        let config = XaaConfig::kr_default();
        assert_eq!(config.market, XaaMarketType::KR);
        assert_eq!(config.offensive_assets.len(), 6);
        assert_eq!(config.bond_assets.len(), 2);
    }

    #[test]
    fn test_all_symbols() {
        let config = XaaConfig::us_default();
        let symbols = config.all_symbols();

        assert!(symbols.contains(&"TIP".to_string()));
        assert!(symbols.contains(&"SPY".to_string()));
        assert!(symbols.contains(&"TLT".to_string()));
        assert!(symbols.contains(&"IEF".to_string()));
        assert!(symbols.contains(&"BIL".to_string()));
    }

    #[test]
    fn test_asset_info_creation() {
        let canary = XaaAssetInfo::canary("TIP", "TIPS ETF");
        assert_eq!(canary.asset_type, XaaAssetType::Canary);

        let offensive = XaaAssetInfo::offensive("SPY", "S&P 500");
        assert_eq!(offensive.asset_type, XaaAssetType::Offensive);

        let bond = XaaAssetInfo::bond("TLT", "Treasury 20+");
        assert_eq!(bond.asset_type, XaaAssetType::Bond);

        let safe = XaaAssetInfo::safe("IEF", "Treasury 7-10Y");
        assert_eq!(safe.asset_type, XaaAssetType::Safe);

        let cash = XaaAssetInfo::cash("BIL", "T-Bill");
        assert_eq!(cash.asset_type, XaaAssetType::Cash);
    }

    #[test]
    fn test_strategy_creation() {
        let strategy = XaaStrategy::new();
        assert_eq!(strategy.name(), "XAA");
        assert_eq!(strategy.version(), "1.0.0");
        assert_eq!(strategy.current_mode, PortfolioMode::Offensive);
    }

    #[test]
    fn test_should_rebalance_first_time() {
        let strategy = XaaStrategy::new();
        let now = Utc::now();
        assert!(strategy.should_rebalance(now));
    }

    #[test]
    fn test_should_rebalance_same_month() {
        let mut strategy = XaaStrategy::new();
        let now = Utc::now();
        strategy.last_rebalance_ym = Some(format!("{}_{}", now.year(), now.month()));
        assert!(!strategy.should_rebalance(now));
    }

    #[test]
    fn test_update_price_history() {
        let mut strategy = XaaStrategy::new();
        strategy.update_price_history("SPY", dec!(400));
        strategy.update_price_history("SPY", dec!(401));
        strategy.update_price_history("SPY", dec!(402));

        let history = strategy.price_history.get("SPY").unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0], dec!(402)); // 최신 가격이 앞에
    }

    #[test]
    fn test_momentum_insufficient_data() {
        let strategy = XaaStrategy::new();
        // 데이터 없으면 None
        let momentum = strategy.calculate_momentum("SPY");
        assert!(momentum.is_none());
    }

    #[test]
    fn test_momentum_calculation_averaged() {
        let mut strategy = XaaStrategy::new();

        // 상승 추세 데이터 생성 (240일)
        let prices: Vec<Decimal> = (0..250)
            .rev()
            .map(|i| dec!(100) + Decimal::from(i) * dec!(0.1))
            .collect();
        strategy.price_history.insert("SPY".to_string(), prices);

        let momentum = strategy.calculate_momentum("SPY");
        assert!(momentum.is_some());
        // 상승 추세이므로 양수
        assert!(momentum.unwrap() > Decimal::ZERO);
    }

    #[test]
    fn test_momentum6_calculation() {
        let mut strategy = XaaStrategy::new();

        // 데이터 생성 (121일)
        let prices: Vec<Decimal> = (0..130)
            .rev()
            .map(|i| dec!(100) + Decimal::from(i) * dec!(0.1))
            .collect();
        strategy.price_history.insert("TLT".to_string(), prices);

        let momentum6 = strategy.calculate_momentum6("TLT");
        assert!(momentum6.is_some());
        // 상승 추세이므로 양수
        assert!(momentum6.unwrap() > Decimal::ZERO);
    }

    #[test]
    fn test_rank_offensive_assets() {
        let mut strategy = XaaStrategy::new();

        // 테스트 데이터: SPY > VEA > IWM (모멘텀 기준)
        let spy_prices: Vec<Decimal> = (0..250)
            .rev()
            .map(|i| dec!(100) + Decimal::from(i) * dec!(0.2))
            .collect();
        let vea_prices: Vec<Decimal> = (0..250)
            .rev()
            .map(|i| dec!(100) + Decimal::from(i) * dec!(0.1))
            .collect();
        let iwm_prices: Vec<Decimal> = (0..250)
            .rev()
            .map(|i| dec!(100) + Decimal::from(i) * dec!(0.05))
            .collect();

        strategy.price_history.insert("SPY".to_string(), spy_prices);
        strategy.price_history.insert("VEA".to_string(), vea_prices);
        strategy.price_history.insert("IWM".to_string(), iwm_prices);

        let assets = vec![
            XaaAssetInfo::offensive("SPY", "S&P 500"),
            XaaAssetInfo::offensive("VEA", "Developed"),
            XaaAssetInfo::offensive("IWM", "Russell 2000"),
        ];

        let ranked = strategy.rank_offensive_assets(&assets);

        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].symbol, "SPY"); // 가장 높은 모멘텀
        assert_eq!(ranked[1].symbol, "VEA");
        assert_eq!(ranked[2].symbol, "IWM");
    }

    #[test]
    fn test_rank_bond_assets_by_momentum6() {
        let mut strategy = XaaStrategy::new();

        // TLT > IEF (Momentum6 기준)
        let tlt_prices: Vec<Decimal> = (0..130)
            .rev()
            .map(|i| dec!(100) + Decimal::from(i) * dec!(0.2))
            .collect();
        let ief_prices: Vec<Decimal> = (0..130)
            .rev()
            .map(|i| dec!(100) + Decimal::from(i) * dec!(0.1))
            .collect();

        strategy.price_history.insert("TLT".to_string(), tlt_prices);
        strategy.price_history.insert("IEF".to_string(), ief_prices);

        let assets = vec![
            XaaAssetInfo::bond("TLT", "20Y Treasury"),
            XaaAssetInfo::bond("IEF", "7-10Y Treasury"),
        ];

        let ranked = strategy.rank_bond_assets(&assets);

        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].symbol, "TLT"); // Momentum6 높은 순
        assert_eq!(ranked[1].symbol, "IEF");
    }

    #[test]
    fn test_check_canary_positive() {
        let mut strategy = XaaStrategy::new();
        let config = XaaConfig::us_default();

        // TIP 상승 추세 데이터
        let tip_prices: Vec<Decimal> = (0..250)
            .rev()
            .map(|i| dec!(100) + Decimal::from(i) * dec!(0.1))
            .collect();
        strategy.price_history.insert("TIP".to_string(), tip_prices);

        let mode = strategy.check_canary_assets(&config);
        assert_eq!(mode, PortfolioMode::Offensive);
    }

    #[test]
    fn test_check_canary_negative() {
        let mut strategy = XaaStrategy::new();
        let config = XaaConfig::us_default();

        // TIP 하락 추세 데이터
        let tip_prices: Vec<Decimal> = (0..250)
            .map(|i| dec!(100) + Decimal::from(i) * dec!(0.1))
            .collect();
        strategy.price_history.insert("TIP".to_string(), tip_prices);

        let mode = strategy.check_canary_assets(&config);
        assert_eq!(mode, PortfolioMode::Defensive);
    }

    #[test]
    fn test_get_state() {
        let strategy = XaaStrategy::new();
        let state = strategy.get_state();

        assert_eq!(state["name"], "XAA");
        assert_eq!(state["version"], "1.0.0");
        assert_eq!(state["current_mode"], "Offensive");
    }
}

// 전략 레지스트리에 자동 등록
use crate::register_strategy;

register_strategy! {
    id: "xaa",
    aliases: [],
    name: "XAA",
    description: "확장 자산배분 (eXtended Asset Allocation) 전략입니다.",
    timeframe: "1d",
    symbols: ["VWO", "BND", "SPY", "EFA", "EEM", "TLT", "IEF", "LQD", "BIL"],
    category: Monthly,
    markets: [Stock],
    type: XaaStrategy
}
