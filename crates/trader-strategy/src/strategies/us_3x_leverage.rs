//! 미국 3배 레버리지/인버스 조합 전략 (US 3X Leverage)
//!
//! 3배 레버리지 ETF와 인버스 ETF를 조합하여 양방향 수익을 추구하는 전략.
//! 상승장에서는 레버리지 ETF, 하락장에서는 인버스 ETF로 헤지.
//!
//! # 전략 로직
//! - **기본 배분**: 레버리지 70% + 인버스 30% (조정 가능)
//! - **리밸런싱**: 월 1회 또는 비율 이탈 시
//! - **진입 조건**: MA 기반 추세 판단 후 비중 조절
//!
//! # 대상 ETF
//! - **레버리지**: TQQQ (나스닥 3배), SOXL (반도체 3배)
//! - **인버스**: SQQQ (나스닥 인버스 3배), SOXS (반도체 인버스 3배)
//!
//! # 기본 배분
//! - TQQQ: 35%
//! - SOXL: 35%
//! - SQQQ: 15%
//! - SOXS: 15%
//!
//! # 권장 타임프레임
//! - 일봉 (1D) - 장기 투자

use crate::Strategy;
use async_trait::async_trait;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use chrono::{DateTime, Utc};
use trader_core::{MarketData, MarketDataType, Order, Position, Side, Signal, Symbol};
use tracing::{debug, info, warn};

/// 개별 ETF 배분 설정.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EtfAllocation {
    /// ETF 티커
    pub ticker: String,
    /// 목표 비율 (0.0 ~ 1.0)
    pub target_ratio: f64,
    /// 레버리지/인버스 타입
    #[serde(default)]
    pub etf_type: EtfType,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum EtfType {
    #[serde(rename = "leverage")]
    Leverage,
    #[serde(rename = "inverse")]
    Inverse,
}

impl Default for EtfType {
    fn default() -> Self {
        EtfType::Leverage
    }
}

/// 미국 3배 레버리지 전략 설정.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Us3xLeverageConfig {
    /// ETF 배분 리스트
    #[serde(default = "default_allocations")]
    pub allocations: Vec<EtfAllocation>,

    /// 리밸런싱 임계값 (비율 이탈 %, 기본값: 5%)
    #[serde(default = "default_rebalance_threshold")]
    pub rebalance_threshold: f64,

    /// 리밸런싱 주기 (일, 기본값: 30일)
    #[serde(default = "default_rebalance_period_days")]
    pub rebalance_period_days: u32,

    /// MA 필터 사용 (기본값: true)
    #[serde(default = "default_use_ma_filter")]
    pub use_ma_filter: bool,

    /// 레버리지 MA 기간 (기본값: 20)
    #[serde(default = "default_ma_period")]
    pub ma_period: usize,

    /// 하락장 시 인버스 비중 증가 (기본값: true)
    #[serde(default = "default_dynamic_allocation")]
    pub dynamic_allocation: bool,

    /// 하락장 인버스 최대 비중 (기본값: 50%)
    #[serde(default = "default_max_inverse_ratio")]
    pub max_inverse_ratio: f64,

    /// 레버리지 최대 손실 시 전량 매도 (기본값: 30%)
    #[serde(default = "default_max_drawdown")]
    pub max_drawdown_pct: f64,
}

fn default_allocations() -> Vec<EtfAllocation> {
    vec![
        EtfAllocation { ticker: "TQQQ".to_string(), target_ratio: 0.35, etf_type: EtfType::Leverage },
        EtfAllocation { ticker: "SOXL".to_string(), target_ratio: 0.35, etf_type: EtfType::Leverage },
        EtfAllocation { ticker: "SQQQ".to_string(), target_ratio: 0.15, etf_type: EtfType::Inverse },
        EtfAllocation { ticker: "SOXS".to_string(), target_ratio: 0.15, etf_type: EtfType::Inverse },
    ]
}

fn default_rebalance_threshold() -> f64 { 5.0 }
fn default_rebalance_period_days() -> u32 { 30 }
fn default_use_ma_filter() -> bool { true }
fn default_ma_period() -> usize { 20 }
fn default_dynamic_allocation() -> bool { true }
fn default_max_inverse_ratio() -> f64 { 0.5 }
fn default_max_drawdown() -> f64 { 30.0 }

impl Default for Us3xLeverageConfig {
    fn default() -> Self {
        Self {
            allocations: default_allocations(),
            rebalance_threshold: 5.0,
            rebalance_period_days: 30,
            use_ma_filter: true,
            ma_period: 20,
            dynamic_allocation: true,
            max_inverse_ratio: 0.5,
            max_drawdown_pct: 30.0,
        }
    }
}

/// ETF 데이터.
#[derive(Debug, Clone)]
struct EtfData {
    ticker: String,
    etf_type: EtfType,
    target_ratio: f64,
    current_ratio: f64,
    current_price: Decimal,
    holdings: Decimal,
    entry_price: Decimal,
    prices: VecDeque<Decimal>,
    ma_period: usize,
}

impl EtfData {
    fn new(ticker: String, target_ratio: f64, etf_type: EtfType, ma_period: usize) -> Self {
        Self {
            ticker,
            etf_type,
            target_ratio,
            current_ratio: 0.0,
            current_price: Decimal::ZERO,
            holdings: Decimal::ZERO,
            entry_price: Decimal::ZERO,
            prices: VecDeque::new(),
            ma_period,
        }
    }

    fn update(&mut self, price: Decimal) {
        self.current_price = price;
        self.prices.push_front(price);
        while self.prices.len() > self.ma_period + 5 {
            self.prices.pop_back();
        }
    }

    fn calculate_ma(&self) -> Option<Decimal> {
        if self.prices.len() < self.ma_period {
            return None;
        }
        let sum: Decimal = self.prices.iter().take(self.ma_period).sum();
        Some(sum / Decimal::from(self.ma_period))
    }

    fn is_above_ma(&self) -> bool {
        match self.calculate_ma() {
            Some(ma) => self.current_price > ma,
            None => true, // 데이터 부족 시 기본값
        }
    }
}

/// 미국 3배 레버리지 전략.
pub struct Us3xLeverageStrategy {
    config: Option<Us3xLeverageConfig>,
    symbols: Vec<Symbol>,

    /// ETF별 데이터
    etf_data: HashMap<String, EtfData>,

    /// 총 포트폴리오 가치
    total_value: Decimal,

    /// 마지막 리밸런싱 날짜
    last_rebalance_date: Option<chrono::NaiveDate>,

    /// 현재 날짜
    current_date: Option<chrono::NaiveDate>,

    /// 포트폴리오 고점
    portfolio_high: Decimal,

    /// 시장 상태 (true = 상승장)
    is_bullish: bool,

    /// 초기화 완료
    started: bool,

    /// 통계
    rebalance_count: u32,
    total_pnl: Decimal,

    initialized: bool,
}

impl Us3xLeverageStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            symbols: Vec::new(),
            etf_data: HashMap::new(),
            total_value: Decimal::ZERO,
            last_rebalance_date: None,
            current_date: None,
            portfolio_high: Decimal::ZERO,
            is_bullish: true,
            started: false,
            rebalance_count: 0,
            total_pnl: Decimal::ZERO,
            initialized: false,
        }
    }

    /// 새로운 날인지 확인.
    fn is_new_day(&self, current_time: DateTime<Utc>) -> bool {
        match self.current_date {
            Some(date) => current_time.date_naive() != date,
            None => true,
        }
    }

    /// 리밸런싱 필요 여부 확인.
    fn needs_rebalancing(&self, current_time: DateTime<Utc>) -> bool {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return false,
        };

        // 주기적 리밸런싱
        if let Some(last_date) = self.last_rebalance_date {
            let days_since = (current_time.date_naive() - last_date).num_days();
            if days_since >= config.rebalance_period_days as i64 {
                return true;
            }
        }

        // 비율 이탈 체크
        for data in self.etf_data.values() {
            let diff = (data.current_ratio - data.target_ratio).abs() * 100.0;
            if diff >= config.rebalance_threshold {
                return true;
            }
        }

        false
    }

    /// 현재 비율 계산.
    fn calculate_current_ratios(&mut self) {
        if self.total_value <= Decimal::ZERO {
            return;
        }

        for data in self.etf_data.values_mut() {
            let value = data.holdings * data.current_price;
            data.current_ratio = (value / self.total_value).to_f64().unwrap_or(0.0);
        }
    }

    /// 시장 상태 판단 (MA 기반).
    fn update_market_state(&mut self) {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return,
        };

        if !config.use_ma_filter {
            self.is_bullish = true;
            return;
        }

        // 레버리지 ETF 중 하나라도 MA 위에 있으면 상승장
        let mut bullish_count = 0;
        let mut total_leverage = 0;

        for data in self.etf_data.values() {
            if data.etf_type == EtfType::Leverage {
                total_leverage += 1;
                if data.is_above_ma() {
                    bullish_count += 1;
                }
            }
        }

        // 과반수 기준
        self.is_bullish = bullish_count > total_leverage / 2;

        debug!(
            bullish_count = bullish_count,
            total = total_leverage,
            is_bullish = self.is_bullish,
            "시장 상태 업데이트"
        );
    }

    /// 목표 비율 조정 (동적 배분).
    fn adjust_target_ratios(&mut self) {
        let config = match self.config.as_ref() {
            Some(c) => c.clone(),
            None => return,
        };

        if !config.dynamic_allocation {
            return;
        }

        // 하락장이면 인버스 비중 증가
        if !self.is_bullish {
            let inverse_boost = config.max_inverse_ratio;

            for data in self.etf_data.values_mut() {
                match data.etf_type {
                    EtfType::Inverse => {
                        // 인버스 비중을 최대까지 증가
                        data.target_ratio = inverse_boost / 2.0; // 인버스 2개 균등
                    }
                    EtfType::Leverage => {
                        // 레버리지 비중 감소
                        data.target_ratio = (1.0 - inverse_boost) / 2.0; // 레버리지 2개 균등
                    }
                }
            }

            info!(
                inverse_ratio = inverse_boost,
                "하락장 감지 - 인버스 비중 증가"
            );
        } else {
            // 상승장이면 기본 비율로 복귀
            for alloc in &config.allocations {
                if let Some(data) = self.etf_data.get_mut(&alloc.ticker) {
                    data.target_ratio = alloc.target_ratio;
                }
            }
        }
    }

    /// 리밸런싱 신호 생성.
    fn generate_rebalance_signals(&mut self, timestamp: DateTime<Utc>) -> Vec<Signal> {
        let mut signals = Vec::new();

        self.calculate_current_ratios();
        self.update_market_state();
        self.adjust_target_ratios();

        for (ticker, data) in &self.etf_data {
            let target = data.target_ratio;
            let current = data.current_ratio;
            let diff = target - current;

            if diff.abs() < 0.01 {
                continue; // 1% 미만 차이는 무시
            }

            let symbol = match self.symbols.iter().find(|s| s.base == *ticker) {
                Some(s) => s.clone(),
                None => continue,
            };

            if diff > 0.0 {
                // 매수 필요
                signals.push(
                    Signal::entry("us_3x_leverage", symbol, Side::Buy)
                        .with_strength(diff.abs())
                        .with_prices(Some(data.current_price), None, None)
                        .with_metadata("action", json!("rebalance_buy"))
                        .with_metadata("target_ratio", json!(target))
                        .with_metadata("current_ratio", json!(current))
                );

                info!(
                    ticker = %ticker,
                    target = %format!("{:.1}%", target * 100.0),
                    current = %format!("{:.1}%", current * 100.0),
                    "리밸런싱 매수"
                );
            } else {
                // 매도 필요
                signals.push(
                    Signal::exit("us_3x_leverage", symbol, Side::Sell)
                        .with_strength(diff.abs())
                        .with_prices(Some(data.current_price), None, None)
                        .with_metadata("action", json!("rebalance_sell"))
                        .with_metadata("target_ratio", json!(target))
                        .with_metadata("current_ratio", json!(current))
                );

                info!(
                    ticker = %ticker,
                    target = %format!("{:.1}%", target * 100.0),
                    current = %format!("{:.1}%", current * 100.0),
                    "리밸런싱 매도"
                );
            }
        }

        if !signals.is_empty() {
            self.last_rebalance_date = Some(timestamp.date_naive());
            self.rebalance_count += 1;
        }

        signals
    }

    /// 초기 진입 신호 생성.
    fn generate_initial_signals(&mut self) -> Vec<Signal> {
        let mut signals = Vec::new();

        for (ticker, data) in &self.etf_data {
            if data.target_ratio <= 0.0 {
                continue;
            }

            let symbol = match self.symbols.iter().find(|s| s.base == *ticker) {
                Some(s) => s.clone(),
                None => continue,
            };

            signals.push(
                Signal::entry("us_3x_leverage", symbol, Side::Buy)
                    .with_strength(data.target_ratio)
                    .with_prices(Some(data.current_price), None, None)
                    .with_metadata("action", json!("initial_buy"))
                    .with_metadata("target_ratio", json!(data.target_ratio))
            );

            info!(
                ticker = %ticker,
                ratio = %format!("{:.1}%", data.target_ratio * 100.0),
                "초기 매수"
            );
        }

        self.started = true;
        signals
    }

    /// 드로다운 체크.
    fn check_drawdown(&mut self) -> Option<Vec<Signal>> {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return None,
        };

        if self.portfolio_high <= Decimal::ZERO {
            return None;
        }

        let drawdown = ((self.portfolio_high - self.total_value) / self.portfolio_high * dec!(100))
            .to_f64()
            .unwrap_or(0.0);

        if drawdown >= config.max_drawdown_pct {
            // 전량 청산
            let mut signals = Vec::new();

            for (ticker, data) in &self.etf_data {
                if data.holdings <= Decimal::ZERO {
                    continue;
                }

                if data.etf_type == EtfType::Leverage {
                    let symbol = match self.symbols.iter().find(|s| s.base == *ticker) {
                        Some(s) => s.clone(),
                        None => continue,
                    };

                    signals.push(
                        Signal::exit("us_3x_leverage", symbol, Side::Sell)
                            .with_strength(1.0)
                            .with_prices(Some(data.current_price), None, None)
                            .with_metadata("action", json!("drawdown_exit"))
                            .with_metadata("drawdown_pct", json!(drawdown))
                    );

                    warn!(
                        ticker = %ticker,
                        drawdown = %format!("{:.1}%", drawdown),
                        "최대 드로다운 도달 - 레버리지 청산"
                    );
                }
            }

            return Some(signals);
        }

        None
    }
}

impl Default for Us3xLeverageStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for Us3xLeverageStrategy {
    fn name(&self) -> &str {
        "US 3X Leverage"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "미국 3배 레버리지/인버스 조합 전략. TQQQ/SOXL (레버리지 70%) + \
         SQQQ/SOXS (인버스 30%) 조합. 월 1회 리밸런싱."
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let lev_config: Us3xLeverageConfig = serde_json::from_value(config)?;

        info!(
            allocations = ?lev_config.allocations.iter().map(|a| &a.ticker).collect::<Vec<_>>(),
            rebalance_days = lev_config.rebalance_period_days,
            dynamic = lev_config.dynamic_allocation,
            "미국 3배 레버리지 전략 초기화"
        );

        // ETF 데이터 초기화
        for alloc in &lev_config.allocations {
            let symbol = Symbol::stock(&alloc.ticker, "USD");
            self.symbols.push(symbol);
            self.etf_data.insert(
                alloc.ticker.clone(),
                EtfData::new(
                    alloc.ticker.clone(),
                    alloc.target_ratio,
                    alloc.etf_type.clone(),
                    lev_config.ma_period,
                ),
            );
        }

        self.config = Some(lev_config);
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

        // base 심볼만 추출 (TQQQ/USD -> TQQQ)
        let symbol_str = data.symbol.base.clone();

        // 등록된 ETF인지 확인
        if !self.etf_data.contains_key(&symbol_str) {
            return Ok(vec![]);
        }

        // kline에서 데이터 추출
        let (close, timestamp) = match &data.data {
            MarketDataType::Kline(kline) => (kline.close, kline.open_time),
            _ => return Ok(vec![]),
        };

        // 새 날짜 확인
        let new_day = self.is_new_day(timestamp);
        if new_day {
            self.current_date = Some(timestamp.date_naive());
        }

        // ETF 데이터 업데이트
        if let Some(etf) = self.etf_data.get_mut(&symbol_str) {
            etf.update(close);
        }

        // 총 가치 계산
        self.total_value = self.etf_data.values()
            .map(|d| d.holdings * d.current_price)
            .sum();

        // 포트폴리오 고점 업데이트
        if self.total_value > self.portfolio_high {
            self.portfolio_high = self.total_value;
        }

        // 아직 시작 안 했으면 초기 진입
        if !self.started {
            // 모든 ETF 가격이 있는지 확인
            let all_have_price = self.etf_data.values().all(|d| d.current_price > Decimal::ZERO);
            if all_have_price {
                return Ok(self.generate_initial_signals());
            }
            return Ok(vec![]);
        }

        // 드로다운 체크
        if let Some(signals) = self.check_drawdown() {
            return Ok(signals);
        }

        // 리밸런싱 체크 (새 날에만)
        if new_day && self.needs_rebalancing(timestamp) {
            return Ok(self.generate_rebalance_signals(timestamp));
        }

        Ok(vec![])
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ticker = order.symbol.to_string();

        if let Some(etf) = self.etf_data.get_mut(&ticker) {
            match order.side {
                Side::Buy => {
                    let old_value = etf.holdings * etf.entry_price;
                    let new_value = order.quantity * order.price.unwrap_or(etf.current_price);
                    let total_qty = etf.holdings + order.quantity;

                    if total_qty > Decimal::ZERO {
                        etf.entry_price = (old_value + new_value) / total_qty;
                    }
                    etf.holdings += order.quantity;
                }
                Side::Sell => {
                    let pnl = order.quantity * (order.price.unwrap_or(etf.current_price) - etf.entry_price);
                    self.total_pnl += pnl;
                    etf.holdings -= order.quantity;

                    if etf.holdings <= Decimal::ZERO {
                        etf.holdings = Decimal::ZERO;
                        etf.entry_price = Decimal::ZERO;
                    }
                }
            }
        }

        debug!(
            ticker = %ticker,
            side = ?order.side,
            quantity = %order.quantity,
            "주문 체결"
        );
        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ticker = position.symbol.to_string();

        if let Some(etf) = self.etf_data.get_mut(&ticker) {
            etf.holdings = position.quantity;
            if position.quantity > Decimal::ZERO {
                etf.entry_price = position.entry_price;
            }
        }

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            rebalance_count = self.rebalance_count,
            total_pnl = %self.total_pnl,
            final_value = %self.total_value,
            "미국 3배 레버리지 전략 종료"
        );

        Ok(())
    }

    fn get_state(&self) -> Value {
        let holdings: HashMap<_, _> = self.etf_data.iter()
            .map(|(k, v)| (k.clone(), json!({
                "holdings": v.holdings.to_string(),
                "current_ratio": format!("{:.1}%", v.current_ratio * 100.0),
                "target_ratio": format!("{:.1}%", v.target_ratio * 100.0),
            })))
            .collect();

        json!({
            "initialized": self.initialized,
            "started": self.started,
            "is_bullish": self.is_bullish,
            "total_value": self.total_value.to_string(),
            "portfolio_high": self.portfolio_high.to_string(),
            "rebalance_count": self.rebalance_count,
            "total_pnl": self.total_pnl.to_string(),
            "holdings": holdings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_us_3x_leverage_initialization() {
        let mut strategy = Us3xLeverageStrategy::new();

        let config = json!({
            "allocations": [
                { "ticker": "TQQQ", "target_ratio": 0.35, "etf_type": "leverage" },
                { "ticker": "SQQQ", "target_ratio": 0.15, "etf_type": "inverse" }
            ],
            "rebalance_period_days": 30
        });

        strategy.initialize(config).await.unwrap();
        assert!(strategy.initialized);
        assert_eq!(strategy.etf_data.len(), 2);
    }

    #[test]
    fn test_default_allocations() {
        let allocs = default_allocations();
        assert_eq!(allocs.len(), 4);

        let total_ratio: f64 = allocs.iter().map(|a| a.target_ratio).sum();
        assert!((total_ratio - 1.0).abs() < 0.01);
    }
}
