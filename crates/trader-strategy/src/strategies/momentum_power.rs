//! Snow Strategy - 시장 안전도 기반 자산 전환 전략
//!
//! ## 핵심 아이디어
//!
//! TIP(TIPS ETF)의 이동평균을 시장 안전 지표로 사용하여
//! 공격 자산(레버리지)과 방어 자산(채권) 사이를 전환합니다.
//!
//! ## 진입 조건
//!
//! 1. **공격 모드 (Attack)**: TIP > TIP MA + 모멘텀 양호
//! 2. **안전 모드 (Safe)**: TIP > TIP MA + 모멘텀 부진
//! 3. **위기 모드 (Crisis)**: TIP <= TIP MA (시장 위험)
//!
//! ## 스크리닝 연동
//!
//! - `MacroEnvironment`: 매크로 위험도 확인 (Critical/High/Normal)
//! - `GlobalScore`: 최소 점수 필터
//! - `MarketRegime`: 추가 진입 조건

use crate::Strategy;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use trader_strategy_macro::StrategyConfig;
use crate::strategies::common::ExitConfig;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use trader_core::{
    domain::{MacroRisk, MarketRegime, StrategyContext},
    MarketData, MarketDataType, Order, Position, Side, Signal, SignalType,
};

// ============================================================================
// 설정 (Config)
// ============================================================================

/// 시장 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MomentumPowerMarket {
    /// 한국 시장
    KR,
    /// 미국 시장
    #[default]
    US,
}

/// Snow 전략 설정
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "momentum_power",
    name = "Momentum Power",
    description = "시장 안전도 기반 공격/방어 자산 전환 전략",
    category = "Monthly"
)]
pub struct MomentumPowerConfig {
    /// 시장 타입 (KR/US)
    #[serde(default)]
    #[schema(label = "시장 타입")]
    pub market: MomentumPowerMarket,

    /// TIP 이동평균 기간 (기본: 200일 = 약 10개월)
    #[serde(default = "default_tip_ma_period")]
    #[schema(label = "TIP MA 기간", min = 50, max = 300)]
    pub tip_ma_period: usize,

    /// 공격 자산 모멘텀 확인 기간 (기본: 5일)
    #[serde(default = "default_momentum_period")]
    #[schema(label = "모멘텀 확인 기간", min = 1, max = 30)]
    pub momentum_period: usize,

    /// 리밸런싱 간격 (일) - 기본: 30일 (월간)
    #[serde(default = "default_rebalance_days")]
    #[schema(label = "리밸런싱 간격 (일)", min = 1, max = 90)]
    pub rebalance_days: u32,

    /// 최소 GlobalScore
    #[serde(default = "default_min_global_score")]
    #[schema(label = "최소 GlobalScore", min = 0, max = 100)]
    pub min_global_score: Decimal,

    /// 청산 설정 (손절/익절/트레일링 스탑).
    #[serde(default)]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,
}

fn default_tip_ma_period() -> usize {
    200
}
fn default_momentum_period() -> usize {
    5
}
fn default_rebalance_days() -> u32 {
    30
}
fn default_min_global_score() -> Decimal {
    dec!(50)
}

impl Default for MomentumPowerConfig {
    fn default() -> Self {
        Self {
            market: MomentumPowerMarket::default(),
            tip_ma_period: default_tip_ma_period(),
            momentum_period: default_momentum_period(),
            rebalance_days: default_rebalance_days(),
            min_global_score: default_min_global_score(),
            exit_config: ExitConfig::default(),
        }
    }
}

// ============================================================================
// 자산 정의
// ============================================================================

/// 시장별 자산 매핑
struct Assets {
    /// 시장 안전 지표 (TIP)
    indicator: &'static str,
    /// 공격 자산 (레버리지)
    attack: &'static str,
    /// 안전 자산 (국채)
    safe: &'static str,
    /// 위기 자산 (단기채)
    crisis: &'static str,
}

impl Assets {
    fn for_market(market: MomentumPowerMarket) -> Self {
        match market {
            MomentumPowerMarket::KR => Self {
                indicator: "TIP",
                attack: "122630", // KODEX 레버리지
                safe: "148070",   // KOSEF 국고채10년
                crisis: "272580", // 미국채혼합레버리지
            },
            MomentumPowerMarket::US => Self {
                indicator: "TIP",
                attack: "UPRO", // 3x S&P 500
                safe: "TLT",    // 20년 국채
                crisis: "BIL",  // 단기 국채
            },
        }
    }

    fn quote(&self, market: MomentumPowerMarket) -> &'static str {
        match market {
            MomentumPowerMarket::KR => "KRW",
            MomentumPowerMarket::US => "USD",
        }
    }

    fn all(&self) -> Vec<&str> {
        vec![self.indicator, self.attack, self.safe, self.crisis]
    }
}

// ============================================================================
// 모드 정의
// ============================================================================

/// 전략 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MomentumPowerMode {
    /// 공격 모드: 시장 안전 + 모멘텀 양호 → 레버리지
    Attack,
    /// 안전 모드: 시장 안전 + 모멘텀 부진 → 국채
    Safe,
    /// 위기 모드: 시장 위험 → 단기채
    Crisis,
}

// ============================================================================
// 전략 상태
// ============================================================================

/// 전략 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MomentumPowerState {
    pub mode: MomentumPowerMode,
    pub tip_ma: Option<Decimal>,
    pub last_rebalance: Option<DateTime<Utc>>,
    pub current_asset: Option<String>,
}

impl Default for MomentumPowerState {
    fn default() -> Self {
        Self {
            mode: MomentumPowerMode::Safe,
            tip_ma: None,
            last_rebalance: None,
            current_asset: None,
        }
    }
}

// ============================================================================
// 전략 구현
// ============================================================================

/// Snow Strategy
pub struct MomentumPowerStrategy {
    config: Option<MomentumPowerConfig>,
    state: MomentumPowerState,
    context: Option<Arc<RwLock<StrategyContext>>>,
    /// TIP 가격 히스토리
    tip_prices: VecDeque<Decimal>,
    /// 공격 자산 가격 히스토리
    attack_prices: VecDeque<Decimal>,
    initialized: bool,
}

impl MomentumPowerStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            state: MomentumPowerState::default(),
            context: None,
            tip_prices: VecDeque::new(),
            attack_prices: VecDeque::new(),
            initialized: false,
        }
    }

    // ========================================================================
    // 스크리닝 연동 헬퍼
    // ========================================================================

    /// MacroEnvironment에서 위험도 확인
    fn get_macro_risk(&self) -> Option<MacroRisk> {
        let ctx = self.context.as_ref()?;
        let ctx_lock = ctx.try_read().ok()?;
        ctx_lock.get_macro_environment().map(|m| m.risk_level)
    }

    /// MarketRegime 확인
    fn get_regime(&self, ticker: &str) -> Option<MarketRegime> {
        let ctx = self.context.as_ref()?;
        let ctx_lock = ctx.try_read().ok()?;
        ctx_lock.get_market_regime(ticker).copied()
    }

    /// GlobalScore 확인
    fn check_global_score(&self, ticker: &str) -> bool {
        let config = match &self.config {
            Some(c) => c,
            None => return true,
        };

        let ctx = match self.context.as_ref() {
            Some(c) => c,
            None => return true, // Context 없으면 통과
        };

        let ctx_lock = match ctx.try_read() {
            Ok(l) => l,
            Err(_) => return true,
        };

        if let Some(score) = ctx_lock.get_global_score(ticker) {
            if score.overall_score < config.min_global_score {
                debug!(
                    ticker = %ticker,
                    score = %score.overall_score,
                    min = %config.min_global_score,
                    "GlobalScore 미달"
                );
                return false;
            }
        }
        true
    }

    // ========================================================================
    // 핵심 로직
    // ========================================================================

    /// 이동평균 계산
    fn calculate_ma(prices: &VecDeque<Decimal>, period: usize) -> Option<Decimal> {
        if prices.len() < period {
            return None;
        }
        let sum: Decimal = prices.iter().take(period).sum();
        Some(sum / Decimal::from(period))
    }

    /// 시장 안전 여부 (TIP > TIP MA)
    fn is_market_safe(&self) -> bool {
        let config = match &self.config {
            Some(c) => c,
            None => return false,
        };

        // MacroRisk 체크 (MacroEnvironment 활용)
        if let Some(risk) = self.get_macro_risk() {
            if risk == MacroRisk::Critical || risk == MacroRisk::High {
                debug!(risk = ?risk, "매크로 위험도 높음 - 위험");
                return false;
            }
        }

        // TIP MA 비교
        if let Some(ma) = Self::calculate_ma(&self.tip_prices, config.tip_ma_period) {
            if let Some(current) = self.tip_prices.front() {
                return *current > ma;
            }
        }

        false
    }

    /// 모멘텀 확인 (공격 자산 > 공격 자산 MA)
    fn has_momentum(&self) -> bool {
        let config = match &self.config {
            Some(c) => c,
            None => return false,
        };

        if let Some(ma) = Self::calculate_ma(&self.attack_prices, config.momentum_period) {
            if let Some(current) = self.attack_prices.front() {
                return *current > ma;
            }
        }

        false
    }

    /// 현재 모드 결정
    fn determine_mode(&self) -> MomentumPowerMode {
        let market_safe = self.is_market_safe();
        let has_momentum = self.has_momentum();

        // MarketRegime 추가 확인
        let config = self.config.as_ref();
        let assets = config.map(|c| Assets::for_market(c.market));

        if let Some(assets) = &assets {
            if let Some(regime) = self.get_regime(assets.attack) {
                // Downtrend면 Crisis로 전환
                if regime == MarketRegime::Downtrend {
                    debug!("MarketRegime::Downtrend 감지 - Crisis 모드");
                    return MomentumPowerMode::Crisis;
                }
            }
        }

        if market_safe && has_momentum {
            MomentumPowerMode::Attack
        } else if market_safe {
            MomentumPowerMode::Safe
        } else {
            MomentumPowerMode::Crisis
        }
    }

    /// 모드에 따른 목표 자산
    fn target_asset(&self, mode: MomentumPowerMode) -> Option<String> {
        let config = self.config.as_ref()?;
        let assets = Assets::for_market(config.market);
        let quote = assets.quote(config.market);

        let ticker = match mode {
            MomentumPowerMode::Attack => assets.attack,
            MomentumPowerMode::Safe => assets.safe,
            MomentumPowerMode::Crisis => assets.crisis,
        };

        Some(format!("{}/{}", ticker, quote))
    }

    /// 리밸런싱 필요 여부
    fn should_rebalance(&self, now: &DateTime<Utc>) -> bool {
        let config = match &self.config {
            Some(c) => c,
            None => return false,
        };

        match self.state.last_rebalance {
            Some(last) => {
                let days = now.signed_duration_since(last).num_days() as u32;
                days >= config.rebalance_days
            }
            None => true,
        }
    }
}

impl Default for MomentumPowerStrategy {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Strategy Trait 구현
// ============================================================================

#[async_trait]
impl Strategy for MomentumPowerStrategy {
    fn name(&self) -> &str {
        "Snow"
    }

    fn version(&self) -> &str {
        "2.0.0"
    }

    fn description(&self) -> &str {
        "시장 안전도 기반 공격/방어 자산 전환 전략"
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let cfg: MomentumPowerConfig = serde_json::from_value(config)?;

        info!(
            market = ?cfg.market,
            tip_ma = cfg.tip_ma_period,
            rebalance_days = cfg.rebalance_days,
            "Snow 전략 초기화"
        );

        self.config = Some(cfg);
        self.state = MomentumPowerState::default();
        self.tip_prices.clear();
        self.attack_prices.clear();
        self.initialized = false;

        Ok(())
    }

    async fn on_market_data(
        &mut self,
        data: &MarketData,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
        let config = match &self.config {
            Some(c) => c,
            None => return Ok(vec![]),
        };

        let assets = Assets::for_market(config.market);
        let ticker = &data.ticker;
        let now = data.timestamp;

        // 이 전략의 자산인지 확인
        if !assets.all().contains(&ticker.as_str()) {
            return Ok(vec![]);
        }

        // 가격 추출
        let price = match &data.data {
            MarketDataType::Kline(k) => k.close,
            MarketDataType::Ticker(t) => t.last,
            MarketDataType::Trade(t) => t.price,
            _ => return Ok(vec![]),
        };

        // 가격 히스토리 업데이트
        if ticker == assets.indicator {
            self.tip_prices.push_front(price);
            if self.tip_prices.len() > 250 {
                self.tip_prices.pop_back();
            }
            self.state.tip_ma = Self::calculate_ma(&self.tip_prices, config.tip_ma_period);
        }

        if ticker == assets.attack {
            self.attack_prices.push_front(price);
            if self.attack_prices.len() > 50 {
                self.attack_prices.pop_back();
            }
        }

        // 초기화 체크
        if !self.initialized {
            if self.tip_prices.len() >= config.tip_ma_period {
                self.initialized = true;
                info!("Snow 전략 초기화 완료");
            } else {
                return Ok(vec![]);
            }
        }

        // 공격 자산에서만 신호 생성
        if ticker != assets.attack {
            return Ok(vec![]);
        }

        // 리밸런싱 체크
        if !self.should_rebalance(&now) {
            return Ok(vec![]);
        }

        // 모드 결정
        let new_mode = self.determine_mode();
        let target = match self.target_asset(new_mode) {
            Some(t) => t,
            None => return Ok(vec![]),
        };

        // GlobalScore 체크
        let target_ticker = target.split('/').next().unwrap_or("");
        if !self.check_global_score(target_ticker) {
            return Ok(vec![]);
        }

        // 모드 변경 또는 자산 변경 시 신호 생성
        if new_mode != self.state.mode || self.state.current_asset.as_deref() != Some(&target) {
            self.state.mode = new_mode;
            self.state.last_rebalance = Some(now);
            self.state.current_asset = Some(target.clone());

            let signal = Signal::new("snow", target.clone(), Side::Buy, SignalType::Entry)
                .with_strength(1.0)
                .with_metadata("mode", json!(format!("{:?}", new_mode)))
                .with_metadata("tip_ma", json!(self.state.tip_ma.map(|d| d.to_string())))
                .with_metadata("market_safe", json!(self.is_market_safe()))
                .with_metadata("has_momentum", json!(self.has_momentum()));

            info!(
                mode = ?new_mode,
                target = %target,
                "Snow 모드 전환"
            );

            return Ok(vec![signal]);
        }

        Ok(vec![])
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            ticker = %order.ticker,
            side = ?order.side,
            qty = %order.quantity,
            "Snow 주문 체결"
        );
        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!(
            ticker = %position.ticker,
            qty = %position.quantity,
            "Snow 포지션 업데이트"
        );
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Snow 전략 종료");
        self.initialized = false;
        Ok(())
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        debug!("StrategyContext 주입 완료");
    }

    fn get_state(&self) -> Value {
        json!({
            "config": self.config,
            "state": self.state,
            "initialized": self.initialized,
            "tip_prices_count": self.tip_prices.len(),
            "attack_prices_count": self.attack_prices.len(),
        })
    }
}

// ============================================================================
// 레지스트리 등록
// ============================================================================

use crate::register_strategy;

register_strategy! {
    id: "snow",
    aliases: ["momentum_power", "snow_us", "snow_kr"],
    name: "Momentum Power",
    description: "시장 안전도 기반 공격/방어 자산 전환 전략",
    timeframe: "1d",
    tickers: ["TIP", "UPRO", "TLT", "BIL"],
    category: Monthly,
    markets: [Stock, Stock],
    type: MomentumPowerStrategy,
    config: MomentumPowerConfig
}

// ============================================================================
// 테스트
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = MomentumPowerConfig::default();
        assert_eq!(config.market, MomentumPowerMarket::US);
        assert_eq!(config.tip_ma_period, 200);
        assert_eq!(config.rebalance_days, 30);
        assert_eq!(config.min_global_score, dec!(50));
    }

    #[test]
    fn test_assets_us() {
        let assets = Assets::for_market(MomentumPowerMarket::US);
        assert_eq!(assets.attack, "UPRO");
        assert_eq!(assets.safe, "TLT");
        assert_eq!(assets.crisis, "BIL");
    }

    #[test]
    fn test_assets_kr() {
        let assets = Assets::for_market(MomentumPowerMarket::KR);
        assert_eq!(assets.attack, "122630");
        assert_eq!(assets.safe, "148070");
    }

    #[tokio::test]
    async fn test_initialization() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = json!({
            "market": "US",
            "tip_ma_period": 200,
            "rebalance_days": 30
        });

        strategy.initialize(config).await.unwrap();
        assert!(strategy.config.is_some());
        assert_eq!(strategy.state.mode, MomentumPowerMode::Safe);
    }
}
