//! Magic Split Strategy - 분할 매수/매도 전략
//!
//! 물타기(averaging down) 전략의 체계적 구현. 미리 정의된 차수별 설정에 따라
//! 단계적으로 매수하고, 각 차수별 목표 수익률 달성 시 해당 차수만 매도합니다.
//!
//! # 전략 개요
//!
//! 1. **1차수 진입**: 시작 시 무조건 매수
//! 2. **N차수 진입**: 이전 차수 진입가 대비 `trigger_rate` 이상 하락 시 추가 매수
//! 3. **차수별 익절**: 해당 차수 진입가 대비 `target_rate` 이상 상승 시 매도
//! 4. **전량 익절 시 초기화**: 모든 차수 매도 후 1차수부터 다시 시작
//!
//! # 예시 설정
//!
//! ```text
//! 1차수: 20만원 매수, 목표 10%, 트리거 없음 (무조건 진입)
//! 2차수: 10만원 매수, 목표 2%, 트리거 -3%
//! 3차수: 10만원 매수, 목표 3%, 트리거 -4%
//! ...
//! 10차수: 10만원 매수, 목표 5%, 트리거 -7%
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, info};
use trader_core::{
    cost_basis, CostMethod, MarketData, MarketDataType, MarketType, Order, Position, Side, Signal,
    Symbol, TradeEntry,
};

use crate::strategies::common::deserialize_symbol;
use crate::traits::Strategy;

/// 분할 매수 레벨 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitLevel {
    /// 차수 번호 (1부터 시작)
    pub number: usize,
    /// 목표 수익률 (%) - 이 수익률 달성 시 매도
    pub target_rate: Decimal,
    /// 매수 트리거 손실률 (%) - 이전 차수 대비 이 손실률 도달 시 매수
    /// 1차수는 None (무조건 진입)
    pub trigger_rate: Option<Decimal>,
    /// 투자 금액
    pub invest_money: Decimal,
}

impl SplitLevel {
    /// 새 분할 레벨 생성
    pub fn new(
        number: usize,
        target_rate: Decimal,
        trigger_rate: Option<Decimal>,
        invest_money: Decimal,
    ) -> Self {
        Self {
            number,
            target_rate,
            trigger_rate,
            invest_money,
        }
    }

    /// 1차수 레벨 생성 (트리거 없음)
    pub fn first(target_rate: Decimal, invest_money: Decimal) -> Self {
        Self {
            number: 1,
            target_rate,
            trigger_rate: None,
            invest_money,
        }
    }
}

/// 차수별 진입 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitLevelState {
    /// 차수 번호
    pub number: usize,
    /// 매수 완료 여부
    pub is_bought: bool,
    /// 진입 가격
    pub entry_price: Decimal,
    /// 진입 수량
    pub entry_quantity: Decimal,
    /// 진입 시각
    pub entry_time: Option<DateTime<Utc>>,
}

impl SplitLevelState {
    /// 새 상태 생성 (미진입 상태)
    pub fn new(number: usize) -> Self {
        Self {
            number,
            is_bought: false,
            entry_price: Decimal::ZERO,
            entry_quantity: Decimal::ZERO,
            entry_time: None,
        }
    }

    /// 매수 상태로 전환
    pub fn buy(&mut self, price: Decimal, quantity: Decimal) {
        self.is_bought = true;
        self.entry_price = price;
        self.entry_quantity = quantity;
        self.entry_time = Some(Utc::now());
    }

    /// 매도 상태로 전환 (초기화)
    pub fn sell(&mut self) {
        self.is_bought = false;
        self.entry_price = Decimal::ZERO;
        self.entry_quantity = Decimal::ZERO;
        self.entry_time = None;
    }

    /// 현재 수익률 계산 (%)
    pub fn current_return_rate(&self, current_price: Decimal) -> Option<Decimal> {
        if !self.is_bought || self.entry_price.is_zero() {
            return None;
        }
        Some((current_price - self.entry_price) / self.entry_price * dec!(100))
    }
}

/// Magic Split 전략 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagicSplitConfig {
    /// 거래 심볼
    #[serde(deserialize_with = "deserialize_symbol")]
    pub symbol: String,
    /// 분할 레벨 목록
    pub levels: Vec<SplitLevel>,
    /// 당일 재진입 허용 여부
    pub allow_same_day_reentry: bool,
    /// 슬리피지 허용치 (%)
    pub slippage_tolerance: Decimal,
}

impl Default for MagicSplitConfig {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            levels: Self::default_levels(),
            allow_same_day_reentry: false,
            slippage_tolerance: dec!(1.0),
        }
    }
}

impl MagicSplitConfig {
    /// 기본 10차수 레벨 생성
    pub fn default_levels() -> Vec<SplitLevel> {
        vec![
            SplitLevel::new(1, dec!(10.0), None, dec!(200000)),
            SplitLevel::new(2, dec!(2.0), Some(dec!(-3.0)), dec!(100000)),
            SplitLevel::new(3, dec!(3.0), Some(dec!(-4.0)), dec!(100000)),
            SplitLevel::new(4, dec!(3.0), Some(dec!(-5.0)), dec!(100000)),
            SplitLevel::new(5, dec!(3.0), Some(dec!(-5.0)), dec!(100000)),
            SplitLevel::new(6, dec!(4.0), Some(dec!(-6.0)), dec!(100000)),
            SplitLevel::new(7, dec!(4.0), Some(dec!(-6.0)), dec!(100000)),
            SplitLevel::new(8, dec!(4.0), Some(dec!(-6.0)), dec!(100000)),
            SplitLevel::new(9, dec!(5.0), Some(dec!(-7.0)), dec!(100000)),
            SplitLevel::new(10, dec!(5.0), Some(dec!(-7.0)), dec!(100000)),
        ]
    }

    /// 레벨 수 반환
    pub fn num_levels(&self) -> usize {
        self.levels.len()
    }

    /// 특정 차수의 레벨 설정 반환
    pub fn get_level(&self, number: usize) -> Option<&SplitLevel> {
        self.levels.iter().find(|l| l.number == number)
    }
}

/// Magic Split 전략 통계
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MagicSplitStats {
    /// 누적 실현 손익
    pub realized_pnl: Decimal,
    /// 총 매수 횟수
    pub total_buys: usize,
    /// 총 매도 횟수
    pub total_sells: usize,
    /// 최대 도달 차수
    pub max_level_reached: usize,
    /// 전체 사이클 완료 횟수 (1차수~익절까지)
    pub completed_cycles: usize,
}

/// Magic Split 전략
pub struct MagicSplitStrategy {
    config: Option<MagicSplitConfig>,
    /// 차수별 상태
    level_states: Vec<SplitLevelState>,
    /// 당일 1차수 매수 가능 여부
    is_ready: bool,
    /// 마지막 가격
    last_price: Option<Decimal>,
    /// 통계
    stats: MagicSplitStats,
    /// 심볼
    symbol: Option<Symbol>,
}

impl MagicSplitStrategy {
    /// 새 전략 인스턴스 생성
    pub fn new() -> Self {
        Self {
            config: None,
            level_states: Vec::new(),
            is_ready: true,
            last_price: None,
            stats: MagicSplitStats::default(),
            symbol: None,
        }
    }

    /// 설정으로 초기화
    fn init_from_config(&mut self, config: MagicSplitConfig) {
        // 레벨 상태 초기화
        self.level_states = config
            .levels
            .iter()
            .map(|l| SplitLevelState::new(l.number))
            .collect();

        // 심볼 파싱 (기본값: 주식 시장)
        self.symbol = Symbol::from_string(&config.symbol, MarketType::Stock);
        self.config = Some(config);
        self.is_ready = true;
        self.stats = MagicSplitStats::default();
    }

    /// 특정 차수의 상태 반환
    fn get_level_state(&self, number: usize) -> Option<&SplitLevelState> {
        self.level_states.iter().find(|s| s.number == number)
    }

    /// 매수 신호 생성
    fn create_buy_signal(
        &self,
        symbol: &Symbol,
        level: &SplitLevel,
        current_price: Decimal,
    ) -> Signal {
        // 투자금액 / 현재가 = 매수 수량
        let quantity = (level.invest_money / current_price).floor();
        let quantity = if quantity < Decimal::ONE {
            Decimal::ONE
        } else {
            quantity
        };

        let slippage = self
            .config
            .as_ref()
            .map(|c| c.slippage_tolerance)
            .unwrap_or(dec!(1.0));
        let suggested_price = current_price * (Decimal::ONE + slippage / dec!(100));

        Signal::entry(self.name(), symbol.clone(), Side::Buy)
            .with_prices(Some(suggested_price), None, None)
            .with_strength(1.0)
            .with_metadata("level", serde_json::json!(level.number))
            .with_metadata("target_rate", serde_json::json!(level.target_rate))
            .with_metadata("invest_money", serde_json::json!(level.invest_money))
            .with_metadata("quantity", serde_json::json!(quantity))
            .with_metadata(
                "reason",
                serde_json::json!(if level.number == 1 {
                    "initial_entry"
                } else {
                    "averaging_down"
                }),
            )
    }

    /// 매도 신호 생성
    fn create_sell_signal(
        &self,
        symbol: &Symbol,
        level: &SplitLevel,
        state: &SplitLevelState,
        current_price: Decimal,
        current_rate: Decimal,
    ) -> Signal {
        let slippage = self
            .config
            .as_ref()
            .map(|c| c.slippage_tolerance)
            .unwrap_or(dec!(1.0));
        let suggested_price = current_price * (Decimal::ONE - slippage / dec!(100));

        Signal::exit(self.name(), symbol.clone(), Side::Sell)
            .with_prices(Some(suggested_price), None, None)
            .with_strength(1.0)
            .with_metadata("level", serde_json::json!(level.number))
            .with_metadata("entry_price", serde_json::json!(state.entry_price))
            .with_metadata("quantity", serde_json::json!(state.entry_quantity))
            .with_metadata("current_rate", serde_json::json!(current_rate))
            .with_metadata("target_rate", serde_json::json!(level.target_rate))
            .with_metadata("reason", serde_json::json!("target_reached"))
    }

    /// 레벨 상태의 인덱스 찾기
    fn find_level_index(&self, number: usize) -> Option<usize> {
        self.level_states.iter().position(|s| s.number == number)
    }

    /// 전략 로직 처리
    fn process_price(&mut self, current_price: Decimal) -> Vec<Signal> {
        let mut signals = Vec::new();

        let config = match &self.config {
            Some(c) => c.clone(),
            None => return signals,
        };

        let symbol = match &self.symbol {
            Some(s) => s.clone(),
            None => return signals,
        };

        self.last_price = Some(current_price);

        // 1. 1차수 처리 (아직 매수 안됨)
        if let Some(level) = config.get_level(1) {
            let should_buy = self
                .get_level_state(1)
                .map(|s| !s.is_bought && self.is_ready)
                .unwrap_or(false);

            if should_buy {
                // 1차수 매수 신호 생성
                let signal = self.create_buy_signal(&symbol, level, current_price);
                signals.push(signal);

                // 상태 업데이트 (스코프 분리)
                let quantity = (level.invest_money / current_price).floor();
                let quantity = if quantity < Decimal::ONE {
                    Decimal::ONE
                } else {
                    quantity
                };

                if let Some(idx) = self.find_level_index(1) {
                    self.level_states[idx].buy(current_price, quantity);
                }
                self.stats.total_buys += 1;
                self.stats.max_level_reached = 1;

                info!("[MagicSplit] 1차수 진입: {} @ {}", symbol, current_price);
            }
        }

        // 2. 매수된 차수들의 익절 체크
        for level in &config.levels {
            let state = match self.get_level_state(level.number) {
                Some(s) if s.is_bought => s.clone(),
                _ => continue,
            };

            if let Some(current_rate) = state.current_return_rate(current_price) {
                debug!(
                    "[MagicSplit] {}차수 수익률: {:.2}% (목표: {}%)",
                    level.number, current_rate, level.target_rate
                );

                // 목표 수익률 달성
                if current_rate >= level.target_rate {
                    let signal = self.create_sell_signal(
                        &symbol,
                        level,
                        &state,
                        current_price,
                        current_rate,
                    );
                    signals.push(signal);

                    // 상태 업데이트 (스코프 분리로 borrow 충돌 방지)
                    let pnl = state.entry_quantity * (current_price - state.entry_price);

                    if let Some(idx) = self.find_level_index(level.number) {
                        self.level_states[idx].sell();
                    }
                    self.stats.realized_pnl += pnl;
                    self.stats.total_sells += 1;

                    // 1차수 매도 시 당일 재진입 방지
                    if level.number == 1 && !config.allow_same_day_reentry {
                        self.is_ready = false;
                    }

                    info!(
                        "[MagicSplit] {}차수 익절: {:.2}% 달성 (목표: {}%)",
                        level.number, current_rate, level.target_rate
                    );
                }
            }
        }

        // 3. 미매수 차수들의 추가 매수 체크 (2차수부터)
        for level in config.levels.iter().skip(1) {
            // 현재 차수가 미매수인지 확인
            let is_not_bought = self
                .get_level_state(level.number)
                .map(|s| !s.is_bought)
                .unwrap_or(false);

            if !is_not_bought {
                continue;
            }

            // 이전 차수가 매수된 상태인지 확인
            let prev_state = match self.get_level_state(level.number - 1) {
                Some(s) if s.is_bought => s.clone(),
                _ => continue,
            };

            let trigger_rate = match level.trigger_rate {
                Some(r) => r,
                None => continue,
            };

            // 이전 차수 대비 손실률 계산
            if let Some(prev_rate) = prev_state.current_return_rate(current_price) {
                debug!(
                    "[MagicSplit] {}차수 진입 체크: 이전 차수 수익률 {:.2}% (트리거: {}%)",
                    level.number, prev_rate, trigger_rate
                );

                // 트리거 조건 충족 (손실률이 트리거 이하)
                if prev_rate <= trigger_rate {
                    let signal = self.create_buy_signal(&symbol, level, current_price);
                    signals.push(signal);

                    // 상태 업데이트 (스코프 분리)
                    let quantity = (level.invest_money / current_price).floor();
                    let quantity = if quantity < Decimal::ONE {
                        Decimal::ONE
                    } else {
                        quantity
                    };

                    if let Some(idx) = self.find_level_index(level.number) {
                        self.level_states[idx].buy(current_price, quantity);
                    }
                    self.stats.total_buys += 1;
                    if level.number > self.stats.max_level_reached {
                        self.stats.max_level_reached = level.number;
                    }

                    info!(
                        "[MagicSplit] {}차수 추가 매수: 이전 차수 손실률 {:.2}% <= {}%",
                        level.number, prev_rate, trigger_rate
                    );
                }
            }
        }

        // 4. 모든 차수가 매도된 경우 사이클 완료
        let all_sold = self.level_states.iter().all(|s| !s.is_bought);
        if all_sold && self.stats.total_sells > 0 {
            self.stats.completed_cycles += 1;
            info!(
                "[MagicSplit] 사이클 완료! 총 {}회",
                self.stats.completed_cycles
            );
        }

        signals
    }

    /// 일일 리셋 (장 마감 후 호출)
    pub fn daily_reset(&mut self) {
        self.is_ready = true;
        info!("[MagicSplit] 일일 리셋: 1차수 매수 가능");
    }

    /// 통계 반환
    pub fn stats(&self) -> &MagicSplitStats {
        &self.stats
    }

    /// 현재 진입된 총 금액
    pub fn total_invested(&self) -> Decimal {
        self.level_states
            .iter()
            .filter(|s| s.is_bought)
            .map(|s| s.entry_price * s.entry_quantity)
            .sum()
    }

    /// 현재 평균 단가
    pub fn average_entry_price(&self) -> Option<Decimal> {
        let entries: Vec<TradeEntry> = self
            .level_states
            .iter()
            .filter(|s| s.is_bought)
            .map(|s| {
                TradeEntry::new(
                    s.entry_price,
                    s.entry_quantity,
                    s.entry_time.unwrap_or_else(|| Utc::now()),
                )
            })
            .collect();

        if entries.is_empty() {
            return None;
        }

        Some(cost_basis(&entries, CostMethod::WeightedAverage))
    }

    /// 현재 총 수익률 (평균단가 기준)
    pub fn total_return_rate(&self, current_price: Decimal) -> Option<Decimal> {
        let avg_price = self.average_entry_price()?;
        Some((current_price - avg_price) / avg_price * dec!(100))
    }
}

impl Default for MagicSplitStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for MagicSplitStrategy {
    fn name(&self) -> &str {
        "magic_split"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "분할 매수/매도 전략. 단계적 물타기와 차수별 익절."
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let config: MagicSplitConfig = serde_json::from_value(config)?;
        self.init_from_config(config);
        info!("[MagicSplit] 전략 초기화 완료");
        Ok(())
    }

    async fn on_market_data(
        &mut self,
        data: &MarketData,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
        let config = match &self.config {
            Some(c) => c,
            None => return Ok(Vec::new()),
        };

        // 심볼 체크
        if data.symbol.to_string() != config.symbol {
            return Ok(Vec::new());
        }

        // 현재가 추출
        let current_price = match &data.data {
            MarketDataType::Ticker(t) => t.last,
            MarketDataType::Kline(k) => k.close,
            _ => return Ok(Vec::new()),
        };

        Ok(self.process_price(current_price))
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "[MagicSplit] 주문 체결: {:?} {} @ {:?}",
            order.side, order.quantity, order.average_fill_price
        );
        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!(
            "[MagicSplit] 포지션 업데이트: {} (PnL: {})",
            position.quantity, position.unrealized_pnl
        );
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "[MagicSplit] 전략 종료. 누적 실현손익: {}, 완료 사이클: {}",
            self.stats.realized_pnl, self.stats.completed_cycles
        );
        Ok(())
    }

    fn get_state(&self) -> Value {
        serde_json::json!({
            "is_ready": self.is_ready,
            "last_price": self.last_price,
            "level_states": self.level_states,
            "stats": self.stats,
            "total_invested": self.total_invested(),
            "average_entry_price": self.average_entry_price(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> MagicSplitConfig {
        MagicSplitConfig {
            symbol: "TEST/KRW".to_string(),
            levels: vec![
                SplitLevel::new(1, dec!(10.0), None, dec!(200000)),
                SplitLevel::new(2, dec!(2.0), Some(dec!(-3.0)), dec!(100000)),
                SplitLevel::new(3, dec!(3.0), Some(dec!(-5.0)), dec!(100000)),
            ],
            allow_same_day_reentry: false,
            slippage_tolerance: dec!(1.0),
        }
    }

    #[test]
    fn test_split_level_creation() {
        let level = SplitLevel::first(dec!(10.0), dec!(200000));
        assert_eq!(level.number, 1);
        assert_eq!(level.target_rate, dec!(10.0));
        assert!(level.trigger_rate.is_none());
    }

    #[test]
    fn test_level_state_buy_sell() {
        let mut state = SplitLevelState::new(1);
        assert!(!state.is_bought);

        state.buy(dec!(10000), dec!(10));
        assert!(state.is_bought);
        assert_eq!(state.entry_price, dec!(10000));
        assert_eq!(state.entry_quantity, dec!(10));

        state.sell();
        assert!(!state.is_bought);
        assert_eq!(state.entry_price, Decimal::ZERO);
    }

    #[test]
    fn test_current_return_rate() {
        let mut state = SplitLevelState::new(1);
        state.buy(dec!(10000), dec!(10));

        // 10% 상승
        let rate = state.current_return_rate(dec!(11000)).unwrap();
        assert_eq!(rate, dec!(10));

        // 5% 하락
        let rate = state.current_return_rate(dec!(9500)).unwrap();
        assert_eq!(rate, dec!(-5));
    }

    #[test]
    fn test_strategy_initialization() {
        let mut strategy = MagicSplitStrategy::new();
        let config = create_test_config();
        strategy.init_from_config(config);

        assert_eq!(strategy.level_states.len(), 3);
        assert!(strategy.is_ready);
    }

    #[test]
    fn test_first_level_entry() {
        let mut strategy = MagicSplitStrategy::new();
        let config = create_test_config();
        strategy.init_from_config(config);

        let signals = strategy.process_price(dec!(10000));

        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].side, Side::Buy);

        // 1차수가 매수됨
        let state = strategy.get_level_state(1).unwrap();
        assert!(state.is_bought);
        assert_eq!(state.entry_price, dec!(10000));
    }

    #[test]
    fn test_second_level_trigger() {
        let mut strategy = MagicSplitStrategy::new();
        let config = create_test_config();
        strategy.init_from_config(config);

        // 1차수 진입
        strategy.process_price(dec!(10000));

        // 가격 3% 하락 -> 2차수 트리거
        let signals = strategy.process_price(dec!(9700));

        // 2차수 매수 신호가 있어야 함
        assert!(signals.iter().any(|s| s.side == Side::Buy));

        let state = strategy.get_level_state(2).unwrap();
        assert!(state.is_bought);
    }

    #[test]
    fn test_take_profit() {
        let mut strategy = MagicSplitStrategy::new();
        let config = create_test_config();
        strategy.init_from_config(config);

        // 1차수 진입
        strategy.process_price(dec!(10000));

        // 가격 10% 상승 -> 1차수 익절
        let signals = strategy.process_price(dec!(11000));

        // 매도 신호가 있어야 함
        assert!(signals.iter().any(|s| s.side == Side::Sell));

        // 1차수가 매도됨
        let state = strategy.get_level_state(1).unwrap();
        assert!(!state.is_bought);

        // 당일 재진입 방지
        assert!(!strategy.is_ready);
    }

    #[test]
    fn test_average_entry_price() {
        let mut strategy = MagicSplitStrategy::new();
        let config = create_test_config();
        strategy.init_from_config(config);

        // 1차수 진입 @ 10000 (20주)
        strategy.process_price(dec!(10000));

        // 2차수 트리거
        strategy.process_price(dec!(9700)); // 2차수 진입 @ 9700 (10주)

        // 평균단가: (10000*20 + 9700*10) / 30 = 9900
        let avg = strategy.average_entry_price().unwrap();
        assert_eq!(avg, dec!(9900));
    }

    #[test]
    fn test_daily_reset() {
        let mut strategy = MagicSplitStrategy::new();
        let config = create_test_config();
        strategy.init_from_config(config);

        strategy.is_ready = false;
        strategy.daily_reset();
        assert!(strategy.is_ready);
    }

    #[test]
    fn test_stats_tracking() {
        let mut strategy = MagicSplitStrategy::new();
        let config = create_test_config();
        strategy.init_from_config(config);

        // 1차수 진입
        strategy.process_price(dec!(10000));
        assert_eq!(strategy.stats.total_buys, 1);
        assert_eq!(strategy.stats.max_level_reached, 1);

        // 2차수 트리거
        strategy.process_price(dec!(9700));
        assert_eq!(strategy.stats.total_buys, 2);
        assert_eq!(strategy.stats.max_level_reached, 2);

        // 1차수 익절 (2차수 기준 10%+)
        strategy.process_price(dec!(11000)); // 1차수: +10%, 2차수: +13.4%
        assert_eq!(strategy.stats.total_sells, 2); // 둘 다 익절
    }
}

// 전략 레지스트리에 자동 등록
use crate::register_strategy;

register_strategy! {
    id: "magic_split",
    aliases: ["split"],
    name: "Magic Split",
    description: "가격대별 분할 매수/매도 전략입니다.",
    timeframe: "1m",
    symbols: [],
    category: Realtime,
    markets: [Crypto, KrStock, UsStock],
    type: MagicSplitStrategy
}
