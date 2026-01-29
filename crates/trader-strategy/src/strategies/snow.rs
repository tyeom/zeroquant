//! Snow Moving Average Momentum Strategy
//!
//! TIP(TIPS ETF) 기반 시장 상태 판단과 이동평균 모멘텀을 결합한 전략입니다.
//!
//! ## 전략 개요
//!
//! ### 한국형 (Snow KR)
//! - 공격 자산: KODEX 레버리지 (122630)
//! - 안전 자산: KOSEF 국고채10년 (148070)
//! - 위기 자산: 미국채혼합레버리지 (272580)
//!
//! ### 미국형 (Snow US)
//! - 공격 자산: UPRO (3x S&P 500)
//! - 안전 자산: TLT (20년 국채)
//! - 위기 자산: BIL (단기 국채)
//!
//! ## 진입 조건
//! 1. TIP > TIP의 10개월 이동평균 (시장 안전)
//! 2. 공격자산 > 공격자산의 5일 이동평균 (단기 모멘텀)
//! 3. 위 조건 만족 시 공격 자산 매수, 아니면 안전 자산 보유

use crate::Strategy;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use trader_core::{MarketData, MarketDataType, MarketType, Order, Position, Side, Signal, SignalType, Symbol};
use tracing::{debug, info};

/// Snow 전략 시장 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnowMarket {
    /// 한국 시장 (레버리지/국채)
    KR,
    /// 미국 시장 (UPRO/TLT)
    US,
}

impl Default for SnowMarket {
    fn default() -> Self {
        Self::US
    }
}

/// Snow 전략 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnowConfig {
    /// 시장 타입 (KR/US)
    #[serde(default)]
    pub market: SnowMarket,

    /// 총 투자 금액
    #[serde(default = "default_total_amount")]
    pub total_amount: Decimal,

    /// TIP 이동평균 기간 (기본: 200일 = 약 10개월)
    #[serde(default = "default_tip_ma_period")]
    pub tip_ma_period: usize,

    /// 공격 자산 이동평균 기간 (기본: 5일)
    #[serde(default = "default_attack_ma_period")]
    pub attack_ma_period: usize,

    /// 리밸런싱 간격 (일)
    #[serde(default = "default_rebalance_days")]
    pub rebalance_days: u32,

    /// 리밸런싱 허용 오차 (%)
    #[serde(default = "default_rebalance_threshold")]
    pub rebalance_threshold: Decimal,
}

fn default_total_amount() -> Decimal { dec!(10000000) }
fn default_tip_ma_period() -> usize { 200 }
fn default_attack_ma_period() -> usize { 5 }
fn default_rebalance_days() -> u32 { 1 }
fn default_rebalance_threshold() -> Decimal { dec!(5) }

impl Default for SnowConfig {
    fn default() -> Self {
        Self {
            market: SnowMarket::US,
            total_amount: default_total_amount(),
            tip_ma_period: default_tip_ma_period(),
            attack_ma_period: default_attack_ma_period(),
            rebalance_days: default_rebalance_days(),
            rebalance_threshold: default_rebalance_threshold(),
        }
    }
}

/// Snow 자산 정보
#[derive(Debug, Clone)]
struct SnowAssets {
    /// TIP (시장 상태 지표)
    tip: String,
    /// 공격 자산
    attack: String,
    /// 안전 자산
    safe: String,
    /// 위기 시 자산
    crisis: String,
}

impl SnowAssets {
    fn for_market(market: SnowMarket) -> Self {
        match market {
            SnowMarket::KR => Self {
                tip: "TIP".to_string(),       // 미국 TIP ETF (참조용)
                attack: "122630".to_string(), // KODEX 레버리지
                safe: "148070".to_string(),   // KOSEF 국고채10년
                crisis: "272580".to_string(), // 미국채혼합레버리지
            },
            SnowMarket::US => Self {
                tip: "TIP".to_string(),   // iShares TIPS Bond ETF
                attack: "UPRO".to_string(), // 3x S&P 500
                safe: "TLT".to_string(),    // 20년 국채
                crisis: "BIL".to_string(),  // 단기 국채
            },
        }
    }

    fn all_symbols(&self) -> Vec<&str> {
        vec![&self.tip, &self.attack, &self.safe, &self.crisis]
    }
}

/// 현재 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnowMode {
    /// 공격 모드 (레버리지/UPRO)
    Attack,
    /// 안전 모드 (국채)
    Safe,
    /// 위기 모드 (단기채권)
    Crisis,
}

/// Snow 전략 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnowState {
    /// 현재 모드
    pub current_mode: SnowMode,
    /// 현재 TIP 이동평균
    pub tip_ma: Option<Decimal>,
    /// 현재 공격 자산 이동평균
    pub attack_ma: Option<Decimal>,
    /// 마지막 리밸런싱 시간
    pub last_rebalance: Option<DateTime<Utc>>,
    /// 현재 보유 심볼
    pub current_holding: Option<String>,
    /// 현재 보유 수량
    pub current_quantity: Decimal,
}

impl Default for SnowState {
    fn default() -> Self {
        Self {
            current_mode: SnowMode::Safe,
            tip_ma: None,
            attack_ma: None,
            last_rebalance: None,
            current_holding: None,
            current_quantity: Decimal::ZERO,
        }
    }
}

/// Snow 이동평균 모멘텀 전략
pub struct SnowStrategy {
    config: Option<SnowConfig>,
    assets: Option<SnowAssets>,
    state: SnowState,
    /// 심볼별 가격 히스토리
    price_history: HashMap<String, VecDeque<Decimal>>,
    initialized: bool,
}

impl SnowStrategy {
    /// 새 전략 인스턴스 생성
    pub fn new() -> Self {
        Self {
            config: None,
            assets: None,
            state: SnowState::default(),
            price_history: HashMap::new(),
            initialized: false,
        }
    }

    /// 이동평균 계산 (순수 함수)
    fn calculate_ma(prices: &VecDeque<Decimal>, period: usize) -> Option<Decimal> {
        if prices.len() < period {
            return None;
        }

        let sum: Decimal = prices.iter().take(period).sum();
        Some(sum / Decimal::from(period))
    }

    /// 시장 상태 판단 (TIP 기반)
    fn is_market_safe(&self) -> bool {
        let assets = self.assets.as_ref().unwrap();
        let config = self.config.as_ref().unwrap();

        if let Some(prices) = self.price_history.get(&assets.tip) {
            if let Some(ma) = Self::calculate_ma(prices, config.tip_ma_period) {
                if let Some(current) = prices.front() {
                    return *current > ma;
                }
            }
        }
        // 데이터 부족 시 안전 모드
        false
    }

    /// 공격 자산 모멘텀 확인
    fn has_attack_momentum(&self) -> bool {
        let assets = self.assets.as_ref().unwrap();
        let config = self.config.as_ref().unwrap();

        if let Some(prices) = self.price_history.get(&assets.attack) {
            if let Some(ma) = Self::calculate_ma(prices, config.attack_ma_period) {
                if let Some(current) = prices.front() {
                    return *current > ma;
                }
            }
        }
        false
    }

    /// 현재 모드 결정
    fn determine_mode(&self) -> SnowMode {
        let market_safe = self.is_market_safe();
        let has_momentum = self.has_attack_momentum();

        if market_safe && has_momentum {
            SnowMode::Attack
        } else if market_safe {
            SnowMode::Safe
        } else {
            SnowMode::Crisis
        }
    }

    /// 모드에 따른 자산 반환
    fn get_asset_for_mode(&self, mode: SnowMode) -> &str {
        let assets = self.assets.as_ref().unwrap();
        match mode {
            SnowMode::Attack => &assets.attack,
            SnowMode::Safe => &assets.safe,
            SnowMode::Crisis => &assets.crisis,
        }
    }

    /// 리밸런싱 필요 여부 확인
    fn should_rebalance(&self, now: &DateTime<Utc>) -> bool {
        let config = self.config.as_ref().unwrap();

        if let Some(last) = self.state.last_rebalance {
            let days = now.signed_duration_since(last).num_days() as u32;
            days >= config.rebalance_days
        } else {
            true
        }
    }
}

impl Default for SnowStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for SnowStrategy {
    fn name(&self) -> &str {
        "Snow Moving Average Momentum"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "TIP 기반 시장 상태 판단과 이동평균 모멘텀을 결합한 자산배분 전략"
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let snow_config: SnowConfig = serde_json::from_value(config)?;

        info!(
            market = ?snow_config.market,
            tip_ma_period = snow_config.tip_ma_period,
            attack_ma_period = snow_config.attack_ma_period,
            "Initializing Snow strategy"
        );

        self.assets = Some(SnowAssets::for_market(snow_config.market));
        self.config = Some(snow_config);
        self.state = SnowState::default();
        self.price_history.clear();
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
        let assets = self.assets.as_ref().unwrap();

        let symbol = data.symbol.to_string();
        let now = data.timestamp;

        // 이 전략의 자산인지 확인
        if !assets.all_symbols().contains(&symbol.as_str()) {
            return Ok(vec![]);
        }

        // 가격 추출
        let close = match &data.data {
            MarketDataType::Kline(kline) => kline.close,
            MarketDataType::Ticker(ticker) => ticker.last,
            MarketDataType::Trade(trade) => trade.price,
            _ => return Ok(vec![]),
        };

        // 가격 히스토리 업데이트
        let prices = self.price_history.entry(symbol.clone()).or_default();
        prices.push_front(close);

        // 히스토리 길이 제한 (최대 250일)
        while prices.len() > 250 {
            prices.pop_back();
        }

        // TIP 데이터 업데이트
        if symbol == assets.tip {
            self.state.tip_ma = Self::calculate_ma(prices, config.tip_ma_period);
        }

        // 공격 자산 데이터 업데이트
        if symbol == assets.attack {
            self.state.attack_ma = Self::calculate_ma(prices, config.attack_ma_period);
        }

        // 초기화 확인 (TIP 데이터 충분성)
        if !self.initialized {
            let tip_prices = self.price_history.get(&assets.tip);
            if let Some(prices) = tip_prices {
                if prices.len() >= config.tip_ma_period {
                    self.initialized = true;
                    info!("[Snow] 전략 초기화 완료");
                }
            }
        }

        // 공격 자산에서만 신호 생성
        if symbol != assets.attack {
            return Ok(vec![]);
        }

        // 리밸런싱 체크
        if !self.initialized || !self.should_rebalance(&now) {
            return Ok(vec![]);
        }

        // 현재 모드 결정
        let new_mode = self.determine_mode();
        let target_asset = self.get_asset_for_mode(new_mode).to_string();

        // 모드 변경 시에만 신호 생성
        if new_mode != self.state.current_mode
            || self.state.current_holding.as_deref() != Some(&target_asset)
        {
            self.state.current_mode = new_mode;
            self.state.last_rebalance = Some(now);

            let (market_type, quote_currency) = match config.market {
                SnowMarket::KR => (MarketType::KrStock, "KRW"),
                SnowMarket::US => (MarketType::UsStock, "USD"),
            };

            let sym = Symbol::new(&target_asset, quote_currency, market_type);

            // 새 자산 매수 신호
            let signal = Signal::new(
                "snow",
                sym,
                Side::Buy,
                SignalType::Entry,
            )
            .with_strength(1.0)
            .with_metadata("mode", json!(format!("{:?}", new_mode)))
            .with_metadata("tip_ma", json!(self.state.tip_ma.map(|d| d.to_string())))
            .with_metadata("attack_ma", json!(self.state.attack_ma.map(|d| d.to_string())))
            .with_metadata("market_safe", json!(self.is_market_safe()))
            .with_metadata("has_momentum", json!(self.has_attack_momentum()));

            self.state.current_holding = Some(target_asset.to_string());

            info!(
                "[Snow] 모드 전환: {:?} → {} 매수",
                new_mode, target_asset
            );

            return Ok(vec![signal]);
        }

        Ok(vec![])
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let fill_price = order.average_fill_price
            .or(order.price)
            .unwrap_or(Decimal::ZERO);

        match order.side {
            Side::Buy => {
                self.state.current_quantity += order.quantity;
                info!(
                    "[Snow] 매수 체결: {} {} @ {}",
                    order.symbol, order.quantity, fill_price
                );
            }
            Side::Sell => {
                self.state.current_quantity -= order.quantity;
                if self.state.current_quantity <= dec!(0) {
                    self.state.current_holding = None;
                    self.state.current_quantity = Decimal::ZERO;
                }
                info!(
                    "[Snow] 매도 체결: {} {} @ {}",
                    order.symbol, order.quantity, fill_price
                );
            }
        }

        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!(
            symbol = %position.symbol,
            quantity = %position.quantity,
            "Position updated"
        );

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Snow strategy shutdown");
        self.initialized = false;
        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "config": self.config,
            "state": self.state,
            "initialized": self.initialized,
            "assets": self.assets.as_ref().map(|a| json!({
                "tip": a.tip,
                "attack": a.attack,
                "safe": a.safe,
                "crisis": a.crisis
            }))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snow_config_default() {
        let config = SnowConfig::default();
        assert_eq!(config.market, SnowMarket::US);
        assert_eq!(config.tip_ma_period, 200);
        assert_eq!(config.attack_ma_period, 5);
    }

    #[tokio::test]
    async fn test_snow_strategy_initialization() {
        let mut strategy = SnowStrategy::new();

        let config = json!({
            "market": "US",
            "tip_ma_period": 200,
            "attack_ma_period": 5
        });

        strategy.initialize(config).await.unwrap();
        assert!(strategy.assets.is_some());
        assert_eq!(strategy.state.current_mode, SnowMode::Safe);
    }

    #[test]
    fn test_snow_assets_kr() {
        let assets = SnowAssets::for_market(SnowMarket::KR);
        assert_eq!(assets.attack, "122630");
        assert_eq!(assets.safe, "148070");
    }

    #[test]
    fn test_snow_assets_us() {
        let assets = SnowAssets::for_market(SnowMarket::US);
        assert_eq!(assets.attack, "UPRO");
        assert_eq!(assets.safe, "TLT");
    }
}
