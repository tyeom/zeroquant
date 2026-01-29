//! 올웨더 포트폴리오 전략
//!
//! Ray Dalio의 All Weather 포트폴리오를 기반으로 한 자산배분 전략입니다.
//! 계절성과 이동평균을 활용하여 동적으로 비중을 조정합니다.
//!
//! # US 버전 (34번)
//! - SPY: 20%, TLT: 27%, IEF: 15%, GLD: 8%, PDBC: 8%, IYK: 22%
//! - 계절성: 5~10월 "지옥기간" 방어적 운용
//!
//! # KR 버전 (35번)
//! - 주식(360750, 294400): 각 20%
//! - 채권(148070, 305080): 각 15%
//! - 금(319640): 15%, 현금(261240): 15%

use crate::strategies::common::deserialize_symbol;
use crate::Strategy;
use async_trait::async_trait;
use chrono::{DateTime, Datelike, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use trader_core::{MarketData, MarketDataType, MarketType, Order, Position, Side, Signal, SignalType, Symbol};
use tracing::{debug, info};

/// 올웨더 시장 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AllWeatherMarket {
    /// 미국 ETF
    US,
    /// 한국 ETF
    KR,
}

impl Default for AllWeatherMarket {
    fn default() -> Self {
        Self::US
    }
}

/// 자산 클래스
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetClass {
    Stock,
    Bond,
    Gold,
    Commodity,
    Cash,
}

/// 자산 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetInfo {
    pub symbol: String,
    pub name: String,
    pub asset_class: AssetClass,
    pub base_weight: Decimal,
}

/// 올웨더 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllWeatherConfig {
    /// 시장 타입
    #[serde(default)]
    pub market: AllWeatherMarket,

    /// 총 투자 금액
    #[serde(default = "default_total_amount")]
    pub total_amount: Decimal,

    /// MA 기간 (일)
    #[serde(default = "default_ma_periods")]
    pub ma_periods: Vec<usize>,

    /// 계절성 사용 여부
    #[serde(default = "default_use_seasonality")]
    pub use_seasonality: bool,

    /// 리밸런싱 주기 (일)
    #[serde(default = "default_rebalance_days")]
    pub rebalance_days: u32,

    /// 리밸런싱 임계값 (%)
    #[serde(default = "default_rebalance_threshold")]
    pub rebalance_threshold: Decimal,

    /// 커스텀 자산 배분 (선택)
    pub custom_assets: Option<Vec<AssetInfo>>,
}

fn default_total_amount() -> Decimal { dec!(10000000) }
fn default_ma_periods() -> Vec<usize> { vec![50, 80, 120, 150] }
fn default_use_seasonality() -> bool { true }
fn default_rebalance_days() -> u32 { 30 }
fn default_rebalance_threshold() -> Decimal { dec!(5) }

impl Default for AllWeatherConfig {
    fn default() -> Self {
        Self {
            market: AllWeatherMarket::US,
            total_amount: default_total_amount(),
            ma_periods: default_ma_periods(),
            use_seasonality: true,
            rebalance_days: 30,
            rebalance_threshold: default_rebalance_threshold(),
            custom_assets: None,
        }
    }
}

impl AllWeatherConfig {
    /// US 올웨더 포트폴리오 생성
    pub fn us_default() -> Self {
        Self {
            market: AllWeatherMarket::US,
            ..Default::default()
        }
    }

    /// KR 올웨더 포트폴리오 생성
    pub fn kr_default() -> Self {
        Self {
            market: AllWeatherMarket::KR,
            ..Default::default()
        }
    }

    /// 기본 자산 목록 반환
    pub fn get_assets(&self) -> Vec<AssetInfo> {
        if let Some(custom) = &self.custom_assets {
            return custom.clone();
        }

        match self.market {
            AllWeatherMarket::US => vec![
                AssetInfo {
                    symbol: "SPY".to_string(),
                    name: "S&P 500".to_string(),
                    asset_class: AssetClass::Stock,
                    base_weight: dec!(20),
                },
                AssetInfo {
                    symbol: "TLT".to_string(),
                    name: "20+ Year Treasury".to_string(),
                    asset_class: AssetClass::Bond,
                    base_weight: dec!(27),
                },
                AssetInfo {
                    symbol: "IEF".to_string(),
                    name: "7-10 Year Treasury".to_string(),
                    asset_class: AssetClass::Bond,
                    base_weight: dec!(15),
                },
                AssetInfo {
                    symbol: "GLD".to_string(),
                    name: "Gold".to_string(),
                    asset_class: AssetClass::Gold,
                    base_weight: dec!(8),
                },
                AssetInfo {
                    symbol: "PDBC".to_string(),
                    name: "Commodities".to_string(),
                    asset_class: AssetClass::Commodity,
                    base_weight: dec!(8),
                },
                AssetInfo {
                    symbol: "IYK".to_string(),
                    name: "Consumer Staples".to_string(),
                    asset_class: AssetClass::Stock,
                    base_weight: dec!(22),
                },
            ],
            AllWeatherMarket::KR => vec![
                AssetInfo {
                    symbol: "360750".to_string(),
                    name: "TIGER 미국S&P500".to_string(),
                    asset_class: AssetClass::Stock,
                    base_weight: dec!(20),
                },
                AssetInfo {
                    symbol: "294400".to_string(),
                    name: "KOSEF 200TR".to_string(),
                    asset_class: AssetClass::Stock,
                    base_weight: dec!(20),
                },
                AssetInfo {
                    symbol: "148070".to_string(),
                    name: "KOSEF 국고채10년".to_string(),
                    asset_class: AssetClass::Bond,
                    base_weight: dec!(15),
                },
                AssetInfo {
                    symbol: "305080".to_string(),
                    name: "TIGER 미국채10년선물".to_string(),
                    asset_class: AssetClass::Bond,
                    base_weight: dec!(15),
                },
                AssetInfo {
                    symbol: "319640".to_string(),
                    name: "TIGER 골드선물(H)".to_string(),
                    asset_class: AssetClass::Gold,
                    base_weight: dec!(15),
                },
                AssetInfo {
                    symbol: "261240".to_string(),
                    name: "TIGER 미국달러단기채권".to_string(),
                    asset_class: AssetClass::Cash,
                    base_weight: dec!(15),
                },
            ],
        }
    }
}

/// 자산별 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AssetState {
    symbol: String,
    current_weight: Decimal,
    target_weight: Decimal,
    quantity: Decimal,
    last_price: Decimal,
    ma_values: HashMap<usize, Decimal>,
}

/// 올웨더 전략
pub struct AllWeatherStrategy {
    config: Option<AllWeatherConfig>,
    assets: Vec<AssetInfo>,
    asset_states: HashMap<String, AssetState>,
    price_history: HashMap<String, Vec<Decimal>>,
    last_rebalance: Option<DateTime<Utc>>,
    is_initialized: bool,
}

impl AllWeatherStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            assets: Vec::new(),
            asset_states: HashMap::new(),
            price_history: HashMap::new(),
            last_rebalance: None,
            is_initialized: false,
        }
    }

    /// 계절성 체크 (5~10월 = 지옥기간)
    fn is_hell_period(&self, date: &DateTime<Utc>) -> bool {
        let month = date.month();
        month >= 5 && month <= 10
    }

    /// MA 기반 비중 조정 계수 계산
    fn calculate_ma_adjustment(&self, symbol: &str, current_price: Decimal) -> Decimal {
        let Some(state) = self.asset_states.get(symbol) else {
            return dec!(1);
        };

        let mut adjustment = dec!(1);

        // MA 150 하단: 비중 50% 축소
        if let Some(&ma150) = state.ma_values.get(&150) {
            if current_price < ma150 {
                adjustment *= dec!(0.5);
            }
        }

        // MA 50 하단: 추가 25% 축소
        if let Some(&ma50) = state.ma_values.get(&50) {
            if current_price < ma50 {
                adjustment *= dec!(0.75);
            }
        }

        adjustment
    }

    /// 목표 비중 계산
    fn calculate_target_weights(&self, now: &DateTime<Utc>) -> HashMap<String, Decimal> {
        let config = self.config.as_ref().unwrap();
        let mut weights = HashMap::new();
        let is_hell = config.use_seasonality && self.is_hell_period(now);

        for asset in &self.assets {
            let mut target = asset.base_weight;

            // 계절성 조정
            if is_hell {
                match asset.asset_class {
                    AssetClass::Stock => target *= dec!(0.7),  // 주식 30% 축소
                    AssetClass::Bond => target *= dec!(1.2),   // 채권 20% 확대
                    _ => {}
                }
            }

            // MA 조정
            if let Some(state) = self.asset_states.get(&asset.symbol) {
                let ma_adj = self.calculate_ma_adjustment(&asset.symbol, state.last_price);
                target *= ma_adj;
            }

            weights.insert(asset.symbol.clone(), target);
        }

        // 비중 정규화 (합 = 100%)
        let total: Decimal = weights.values().sum();
        if total > dec!(0) {
            for weight in weights.values_mut() {
                *weight = (*weight / total) * dec!(100);
            }
        }

        weights
    }

    /// MA 계산
    fn calculate_ma(&self, symbol: &str, period: usize) -> Option<Decimal> {
        let history = self.price_history.get(symbol)?;
        if history.len() < period {
            return None;
        }

        let sum: Decimal = history.iter().rev().take(period).sum();
        Some(sum / Decimal::from(period))
    }

    /// 리밸런싱 필요 여부 확인
    fn needs_rebalance(&self, now: &DateTime<Utc>) -> bool {
        let config = self.config.as_ref().unwrap();

        if let Some(last) = self.last_rebalance {
            let days = (now.signed_duration_since(last).num_days()) as u32;
            if days < config.rebalance_days {
                return false;
            }
        }

        // 비중 편차 확인
        let target_weights = self.calculate_target_weights(now);
        for (symbol, state) in &self.asset_states {
            if let Some(&target) = target_weights.get(symbol) {
                let diff = (state.current_weight - target).abs();
                if diff >= config.rebalance_threshold {
                    return true;
                }
            }
        }

        // 첫 리밸런싱
        self.last_rebalance.is_none()
    }

    /// 리밸런싱 신호 생성
    fn generate_rebalance_signals(&self, now: &DateTime<Utc>) -> Vec<Signal> {
        let config = self.config.as_ref().unwrap();
        let target_weights = self.calculate_target_weights(now);
        let mut signals = Vec::new();

        for (symbol, &target_weight) in &target_weights {
            let current_weight = self.asset_states
                .get(symbol)
                .map(|s| s.current_weight)
                .unwrap_or(dec!(0));

            let diff = target_weight - current_weight;
            let diff_abs = diff.abs();

            if diff_abs < dec!(1) {
                continue; // 1% 미만 차이는 무시
            }

            let (market_type, quote_currency) = match config.market {
                AllWeatherMarket::US => (MarketType::UsStock, "USD"),
                AllWeatherMarket::KR => (MarketType::KrStock, "KRW"),
            };

            let sym = Symbol::new(symbol, quote_currency, market_type);

            if diff > dec!(0) {
                // 매수
                let signal = Signal::new(
                    "all_weather",
                    sym,
                    Side::Buy,
                    SignalType::AddToPosition,
                )
                .with_strength((diff_abs / dec!(10)).min(dec!(1)).to_string().parse().unwrap_or(1.0))
                .with_metadata("reason", json!("rebalance_buy"))
                .with_metadata("target_weight", json!(target_weight.to_string()))
                .with_metadata("current_weight", json!(current_weight.to_string()));

                signals.push(signal);
            } else {
                // 매도
                let signal = Signal::new(
                    "all_weather",
                    sym,
                    Side::Sell,
                    SignalType::ReducePosition,
                )
                .with_strength((diff_abs / dec!(10)).min(dec!(1)).to_string().parse().unwrap_or(1.0))
                .with_metadata("reason", json!("rebalance_sell"))
                .with_metadata("target_weight", json!(target_weight.to_string()))
                .with_metadata("current_weight", json!(current_weight.to_string()));

                signals.push(signal);
            }
        }

        signals
    }
}

impl Default for AllWeatherStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for AllWeatherStrategy {
    fn name(&self) -> &str {
        "All Weather Portfolio"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "Ray Dalio 올웨더 포트폴리오. 계절성과 MA 기반 동적 비중 조정."
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let aw_config: AllWeatherConfig = serde_json::from_value(config)?;

        info!(
            market = ?aw_config.market,
            use_seasonality = aw_config.use_seasonality,
            rebalance_days = aw_config.rebalance_days,
            "Initializing All Weather strategy"
        );

        self.assets = aw_config.get_assets();
        self.config = Some(aw_config);
        self.asset_states.clear();
        self.price_history.clear();
        self.last_rebalance = None;
        self.is_initialized = false;

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

        let symbol = data.symbol.to_string();
        let now = data.timestamp;

        // 이 전략의 자산인지 확인
        if !self.assets.iter().any(|a| a.symbol == symbol) {
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
        self.price_history
            .entry(symbol.clone())
            .or_insert_with(Vec::new)
            .push(close);

        // 최대 250개 유지
        if let Some(history) = self.price_history.get_mut(&symbol) {
            if history.len() > 250 {
                history.remove(0);
            }
        }

        // MA 업데이트
        let mut ma_values = HashMap::new();
        for &period in &config.ma_periods {
            if let Some(ma) = self.calculate_ma(&symbol, period) {
                ma_values.insert(period, ma);
            }
        }

        // 자산 상태 업데이트
        let state = self.asset_states.entry(symbol.clone()).or_insert(AssetState {
            symbol: symbol.clone(),
            current_weight: dec!(0),
            target_weight: dec!(0),
            quantity: dec!(0),
            last_price: close,
            ma_values: HashMap::new(),
        });
        state.last_price = close;
        state.ma_values = ma_values;

        // 초기화 확인 (모든 자산의 충분한 데이터)
        if !self.is_initialized {
            let all_initialized = self.assets.iter().all(|a| {
                self.price_history
                    .get(&a.symbol)
                    .map(|h| h.len() >= 50)
                    .unwrap_or(false)
            });

            if all_initialized {
                self.is_initialized = true;
                info!("[AllWeather] 전략 초기화 완료");
            }
        }

        // 리밸런싱 확인
        if self.is_initialized && self.needs_rebalance(&now) {
            let signals = self.generate_rebalance_signals(&now);
            if !signals.is_empty() {
                self.last_rebalance = Some(now);
                info!(
                    "[AllWeather] 리밸런싱 실행: {} 개 신호, 계절성: {}",
                    signals.len(),
                    if self.is_hell_period(&now) { "지옥기간" } else { "천국기간" }
                );
                return Ok(signals);
            }
        }

        Ok(vec![])
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let symbol = order.symbol.to_string();

        if let Some(state) = self.asset_states.get_mut(&symbol) {
            match order.side {
                Side::Buy => state.quantity += order.quantity,
                Side::Sell => state.quantity -= order.quantity,
            }

            // 비중 재계산
            let total_value: Decimal = self.asset_states
                .values()
                .map(|s| s.quantity * s.last_price)
                .sum();

            if total_value > dec!(0) {
                for s in self.asset_states.values_mut() {
                    s.current_weight = (s.quantity * s.last_price / total_value) * dec!(100);
                }
            }

            info!(
                "[AllWeather] 주문 체결: {} {:?} {} @ {:?}",
                symbol, order.side, order.quantity, order.average_fill_price
            );
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
        info!("All Weather strategy shutdown");
        self.is_initialized = false;
        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "config": self.config,
            "asset_states": self.asset_states,
            "last_rebalance": self.last_rebalance,
            "is_initialized": self.is_initialized,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_us_assets() {
        let config = AllWeatherConfig::us_default();
        let assets = config.get_assets();

        assert_eq!(assets.len(), 6);
        let total_weight: Decimal = assets.iter().map(|a| a.base_weight).sum();
        assert_eq!(total_weight, dec!(100));
    }

    #[test]
    fn test_kr_assets() {
        let config = AllWeatherConfig::kr_default();
        let assets = config.get_assets();

        assert_eq!(assets.len(), 6);
        let total_weight: Decimal = assets.iter().map(|a| a.base_weight).sum();
        assert_eq!(total_weight, dec!(100));
    }

    #[test]
    fn test_hell_period() {
        let mut strategy = AllWeatherStrategy::new();
        strategy.config = Some(AllWeatherConfig::us_default());

        // 5월 = 지옥기간
        let may = chrono::DateTime::parse_from_rfc3339("2024-05-15T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(strategy.is_hell_period(&may));

        // 1월 = 천국기간
        let jan = chrono::DateTime::parse_from_rfc3339("2024-01-15T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(!strategy.is_hell_period(&jan));
    }

    #[tokio::test]
    async fn test_initialization() {
        let mut strategy = AllWeatherStrategy::new();

        let config = json!({
            "market": "US",
            "use_seasonality": true,
            "total_amount": "10000000"
        });

        strategy.initialize(config).await.unwrap();
        assert_eq!(strategy.assets.len(), 6);
    }
}
