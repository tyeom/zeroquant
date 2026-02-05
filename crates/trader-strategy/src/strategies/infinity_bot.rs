//! Infinity Bot (무한매수봇) Strategy v2.0
//!
//! ## 핵심 아이디어
//!
//! 피라미드 구조로 하락 시 분할 매수하고,
//! 평균 단가 대비 목표 수익률 달성 시 익절하는 전략입니다.
//!
//! ## 진입 조건 (MarketRegime 기반)
//!
//! 1. **적극 진입**: StrongUptrend, BottomBounce
//! 2. **조건부 진입**: Correction (MA 상회 시)
//! 3. **보수적 진입**: Sideways (양봉 + 모멘텀 양수)
//! 4. **진입 금지**: Downtrend
//!
//! ## 스크리닝 연동
//!
//! - `GlobalScore`: 최소 점수 필터
//! - `MarketRegime`: 진입 조건 결정
//! - `RouteState`: Attack/Armed 시 적극 진입

use crate::Strategy;
use async_trait::async_trait;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use trader_core::{
    domain::{MarketRegime, RouteState, StrategyContext},
    MarketData, MarketDataType, Order, Position, Side, Signal, SignalType,
};
use trader_strategy_macro::StrategyConfig;

use crate::strategies::common::ExitConfig;

// ============================================================================
// 설정 (Config)
// ============================================================================

/// Infinity Bot 설정
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "infinity_bot",
    name = "무한매수봇",
    description = "피라미드 구조로 하락 시 분할 매수하고 평균 단가 대비 목표 수익률 달성 시 익절",
    category = "Daily"
)]
pub struct InfinityBotConfig {
    /// 대상 티커
    #[serde(default = "default_ticker")]
    #[schema(label = "대상 티커", field_type = "symbol", default = "005930")]
    pub ticker: String,

    /// 총 투자 금액
    #[serde(default = "default_total_amount")]
    #[schema(label = "총 투자 금액", min = 100000, max = 1000000000, default = 10000000)]
    pub total_amount: Decimal,

    /// 최대 라운드 수
    #[serde(default = "default_max_rounds")]
    #[schema(label = "최대 라운드 수", min = 1, max = 100, default = 50)]
    pub max_rounds: usize,

    /// 라운드당 투자 비율 (%)
    #[serde(default = "default_round_pct")]
    #[schema(label = "라운드당 투자 비율 (%)", min = 0.5, max = 20, default = 2)]
    pub round_pct: Decimal,

    /// 추가 매수 트리거 하락률 (%)
    #[serde(default = "default_dip_trigger")]
    #[schema(label = "추가 매수 트리거 하락률 (%)", min = 0.5, max = 20, default = 2)]
    pub dip_trigger_pct: Decimal,

    /// 익절 목표 수익률 (%)
    #[serde(default = "default_take_profit")]
    #[schema(label = "익절 목표 수익률 (%)", min = 0.5, max = 50, default = 3)]
    pub take_profit_pct: Decimal,

    /// 이동평균 기간
    #[serde(default = "default_ma_period")]
    #[schema(label = "이동평균 기간", min = 5, max = 200, default = 20)]
    pub ma_period: usize,

    /// 최소 GlobalScore
    #[serde(default = "default_min_global_score")]
    #[schema(label = "최소 GlobalScore", min = 0, max = 100, default = 50)]
    pub min_global_score: Decimal,

    /// 청산 설정 (손절/익절/트레일링 스탑).
    #[serde(default)]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,
}

fn default_ticker() -> String {
    "005930".to_string()
}
fn default_total_amount() -> Decimal {
    dec!(10000000)
}
fn default_max_rounds() -> usize {
    50
}
fn default_round_pct() -> Decimal {
    dec!(2)
}
fn default_dip_trigger() -> Decimal {
    dec!(2)
}
fn default_take_profit() -> Decimal {
    dec!(3)
}
fn default_ma_period() -> usize {
    20
}
fn default_min_global_score() -> Decimal {
    dec!(50)
}

impl Default for InfinityBotConfig {
    fn default() -> Self {
        Self {
            ticker: "005930".to_string(),
            total_amount: default_total_amount(),
            max_rounds: default_max_rounds(),
            round_pct: default_round_pct(),
            dip_trigger_pct: default_dip_trigger(),
            take_profit_pct: default_take_profit(),
            ma_period: default_ma_period(),
            min_global_score: default_min_global_score(),
            exit_config: ExitConfig::default(),
        }
    }
}

// ============================================================================
// 라운드 정보
// ============================================================================

/// 개별 라운드 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundInfo {
    pub round: usize,
    pub entry_price: Decimal,
    pub quantity: Decimal,
    pub timestamp: i64,
}

// ============================================================================
// 전략 상태
// ============================================================================

/// 전략 상태
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InfinityBotState {
    /// 현재 라운드
    pub current_round: usize,
    /// 라운드 히스토리
    pub rounds: Vec<RoundInfo>,
    /// 평균 진입가
    pub avg_price: Option<Decimal>,
    /// 총 수량
    pub total_quantity: Decimal,
    /// 총 투자 금액
    pub invested_amount: Decimal,
}

impl InfinityBotState {
    /// 평균 단가 계산
    fn calculate_avg_price(&self) -> Option<Decimal> {
        if self.total_quantity.is_zero() {
            return None;
        }
        Some(self.invested_amount / self.total_quantity)
    }

    /// 현재 수익률 계산
    fn current_return(&self, current_price: Decimal) -> Option<Decimal> {
        let avg = self.avg_price?;
        if avg.is_zero() {
            return None;
        }
        Some((current_price - avg) / avg * dec!(100))
    }
}

// ============================================================================
// 전략 구현
// ============================================================================

/// Infinity Bot Strategy
pub struct InfinityBotStrategy {
    config: Option<InfinityBotConfig>,
    state: InfinityBotState,
    context: Option<Arc<RwLock<StrategyContext>>>,
    /// 가격 히스토리
    prices: VecDeque<Decimal>,
    /// 마지막 진입 가격 (물타기용)
    last_entry_price: Option<Decimal>,
    initialized: bool,
}

impl InfinityBotStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            state: InfinityBotState::default(),
            context: None,
            prices: VecDeque::new(),
            last_entry_price: None,
            initialized: false,
        }
    }

    // ========================================================================
    // 스크리닝 연동 헬퍼
    // ========================================================================

    /// GlobalScore 확인
    fn check_global_score(&self) -> bool {
        let config = match &self.config {
            Some(c) => c,
            None => return true,
        };

        let ctx = match self.context.as_ref() {
            Some(c) => c,
            None => return true,
        };

        let ctx_lock = match ctx.try_read() {
            Ok(l) => l,
            Err(_) => return true,
        };

        if let Some(score) = ctx_lock.get_global_score(&config.ticker) {
            if score.overall_score < config.min_global_score {
                debug!(
                    ticker = %config.ticker,
                    score = %score.overall_score,
                    min = %config.min_global_score,
                    "GlobalScore 미달"
                );
                return false;
            }
        }
        true
    }

    /// MarketRegime 확인
    fn get_regime(&self) -> Option<MarketRegime> {
        let config = self.config.as_ref()?;
        let ctx = self.context.as_ref()?;
        let ctx_lock = ctx.try_read().ok()?;
        ctx_lock.get_market_regime(&config.ticker).copied()
    }

    /// RouteState 확인
    fn get_route_state(&self) -> Option<RouteState> {
        let config = self.config.as_ref()?;
        let ctx = self.context.as_ref()?;
        let ctx_lock = ctx.try_read().ok()?;
        ctx_lock.get_route_state(&config.ticker).copied()
    }

    // ========================================================================
    // 핵심 로직
    // ========================================================================

    /// 이동평균 계산
    fn calculate_ma(&self) -> Option<Decimal> {
        let config = self.config.as_ref()?;
        let period = config.ma_period;

        if self.prices.len() < period {
            return None;
        }

        let sum: Decimal = self.prices.iter().take(period).sum();
        Some(sum / Decimal::from(period))
    }

    /// 현재 가격이 MA 위인지
    fn is_above_ma(&self, price: Decimal) -> bool {
        self.calculate_ma().map(|ma| price > ma).unwrap_or(false)
    }

    /// 모멘텀 확인 (최근 5일 수익률)
    fn has_positive_momentum(&self) -> bool {
        if self.prices.len() < 6 {
            return false;
        }

        let current = self.prices.front().copied().unwrap_or(Decimal::ZERO);
        let past = self.prices.get(5).copied().unwrap_or(Decimal::ZERO);

        if past.is_zero() {
            return false;
        }

        current > past
    }

    /// MarketRegime 기반 진입 가능 여부
    fn can_enter(&self, price: Decimal) -> bool {
        let regime = self.get_regime();

        // RouteState가 Attack/Armed면 적극 진입
        if let Some(route) = self.get_route_state() {
            if route == RouteState::Attack || route == RouteState::Armed {
                debug!(route = ?route, "RouteState 적극 모드 - 진입 허용");
                return true;
            }
        }

        match regime {
            Some(MarketRegime::StrongUptrend) | Some(MarketRegime::BottomBounce) => {
                debug!(regime = ?regime, "적극 진입 구간");
                true
            }
            Some(MarketRegime::Correction) => {
                // MA 상회 시 진입
                let above_ma = self.is_above_ma(price);
                debug!(regime = ?regime, above_ma, "조건부 진입 구간");
                above_ma
            }
            Some(MarketRegime::Sideways) => {
                // 양봉 + 모멘텀 양수
                let positive = self.has_positive_momentum();
                debug!(regime = ?regime, positive_momentum = positive, "보수적 진입 구간");
                positive
            }
            Some(MarketRegime::Downtrend) => {
                debug!(regime = ?regime, "Downtrend - 진입 금지");
                false
            }
            None => {
                // Context 없으면 MA 조건만 확인
                self.is_above_ma(price)
            }
        }
    }

    /// 물타기 가능 여부 (하락률 체크)
    fn can_add_position(&self, current_price: Decimal) -> bool {
        let config = match &self.config {
            Some(c) => c,
            None => return false,
        };

        // 최대 라운드 체크
        if self.state.current_round >= config.max_rounds {
            return false;
        }

        // 마지막 진입가 대비 하락률
        if let Some(last_price) = self.last_entry_price {
            if last_price.is_zero() {
                return false;
            }
            let drop_pct = (last_price - current_price) / last_price * dec!(100);
            drop_pct >= config.dip_trigger_pct
        } else {
            // 첫 진입
            true
        }
    }

    /// 익절 조건 확인
    fn should_take_profit(&self, current_price: Decimal) -> bool {
        let config = match &self.config {
            Some(c) => c,
            None => return false,
        };

        if let Some(return_pct) = self.state.current_return(current_price) {
            return_pct >= config.take_profit_pct
        } else {
            false
        }
    }

    /// 라운드당 투자 금액
    fn round_amount(&self) -> Decimal {
        let config = match &self.config {
            Some(c) => c,
            None => return Decimal::ZERO,
        };

        config.total_amount * config.round_pct / dec!(100)
    }
}

impl Default for InfinityBotStrategy {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Strategy Trait 구현
// ============================================================================

#[async_trait]
impl Strategy for InfinityBotStrategy {
    fn name(&self) -> &str {
        "InfinityBot"
    }

    fn version(&self) -> &str {
        "2.0.0"
    }

    fn description(&self) -> &str {
        "피라미드 물타기 + MarketRegime 기반 진입 전략"
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let cfg: InfinityBotConfig = serde_json::from_value(config)?;

        info!(
            ticker = %cfg.ticker,
            max_rounds = cfg.max_rounds,
            take_profit = %cfg.take_profit_pct,
            "InfinityBot 전략 초기화"
        );

        self.config = Some(cfg);
        self.state = InfinityBotState::default();
        self.prices.clear();
        self.last_entry_price = None;
        self.initialized = false;

        Ok(())
    }

    async fn on_market_data(
        &mut self,
        data: &MarketData,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
        let config = match &self.config {
            Some(c) => c.clone(),
            None => return Ok(vec![]),
        };

        // 해당 티커인지 확인
        if !data.ticker.starts_with(&config.ticker) {
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
        self.prices.push_front(price);
        if self.prices.len() > 250 {
            self.prices.pop_back();
        }

        // 초기화 체크
        if !self.initialized && self.prices.len() >= config.ma_period {
            self.initialized = true;
            info!("InfinityBot 초기화 완료");
        }

        if !self.initialized {
            return Ok(vec![]);
        }

        let mut signals = vec![];
        let timestamp = data.timestamp.timestamp();

        // 1. 익절 조건 확인 (보유 중일 때)
        if !self.state.total_quantity.is_zero() && self.should_take_profit(price) {
            let return_pct = self.state.current_return(price).unwrap_or(Decimal::ZERO);

            info!(
                ticker = %config.ticker,
                return_pct = %return_pct,
                rounds = self.state.current_round,
                "익절 조건 충족"
            );

            let signal = Signal::new(
                "infinity_bot",
                config.ticker.clone(),
                Side::Sell,
                SignalType::Exit,
            )
            .with_strength(1.0)
            .with_metadata("action", json!("take_profit"))
            .with_metadata("return_pct", json!(return_pct.to_string()))
            .with_metadata("rounds", json!(self.state.current_round));

            // 상태 초기화
            self.state = InfinityBotState::default();
            self.last_entry_price = None;

            return Ok(vec![signal]);
        }

        // 2. GlobalScore 필터
        if !self.check_global_score() {
            return Ok(vec![]);
        }

        // 3. 진입/물타기 조건 확인
        if self.can_add_position(price) && self.can_enter(price) {
            let round = self.state.current_round + 1;
            let amount = self.round_amount();
            let quantity = if price.is_zero() {
                Decimal::ZERO
            } else {
                amount / price
            };

            // 상태 업데이트
            self.state.current_round = round;
            self.state.rounds.push(RoundInfo {
                round,
                entry_price: price,
                quantity,
                timestamp,
            });
            self.state.total_quantity += quantity;
            self.state.invested_amount += amount;
            self.state.avg_price = self.state.calculate_avg_price();
            self.last_entry_price = Some(price);

            info!(
                ticker = %config.ticker,
                round,
                price = %price,
                quantity = %quantity,
                avg_price = ?self.state.avg_price,
                "라운드 진입"
            );

            let signal = Signal::new(
                "infinity_bot",
                config.ticker.clone(),
                Side::Buy,
                SignalType::Entry,
            )
            .with_strength(1.0)
            .with_metadata("action", json!("round_entry"))
            .with_metadata("round", json!(round))
            .with_metadata("quantity", json!(quantity.to_string()))
            .with_metadata(
                "avg_price",
                json!(self.state.avg_price.map(|d| d.to_string())),
            );

            signals.push(signal);
        }

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            ticker = %order.ticker,
            side = ?order.side,
            qty = %order.quantity,
            "InfinityBot 주문 체결"
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
            "InfinityBot 포지션 업데이트"
        );
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            rounds = self.state.current_round,
            total_qty = %self.state.total_quantity,
            "InfinityBot 종료"
        );
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
            "prices_count": self.prices.len(),
            "last_entry_price": self.last_entry_price.map(|d| d.to_string()),
        })
    }
}

// ============================================================================
// 레지스트리 등록
// ============================================================================

use crate::register_strategy;

register_strategy! {
    id: "infinity_bot",
    aliases: ["무한매수봇", "infinity"],
    name: "인피니티봇",
    description: "피라미드 물타기 + MarketRegime 기반 진입 전략",
    timeframe: "1d",
    tickers: ["005930"],
    category: Realtime,
    markets: [Stock],
    type: InfinityBotStrategy,
    config: InfinityBotConfig
}

// ============================================================================
// 테스트
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = InfinityBotConfig::default();
        assert_eq!(config.max_rounds, 50);
        assert_eq!(config.take_profit_pct, dec!(3));
        assert_eq!(config.dip_trigger_pct, dec!(2));
        assert_eq!(config.min_global_score, dec!(50));
    }

    #[test]
    fn test_state_avg_price() {
        let mut state = InfinityBotState::default();

        // 첫 진입: 1000원에 10주
        state.invested_amount = dec!(10000);
        state.total_quantity = dec!(10);
        state.avg_price = state.calculate_avg_price();
        assert_eq!(state.avg_price, Some(dec!(1000)));

        // 물타기: 800원에 10주 추가
        state.invested_amount += dec!(8000);
        state.total_quantity += dec!(10);
        state.avg_price = state.calculate_avg_price();
        assert_eq!(state.avg_price, Some(dec!(900)));
    }

    #[test]
    fn test_current_return() {
        let mut state = InfinityBotState::default();
        state.invested_amount = dec!(10000);
        state.total_quantity = dec!(10);
        state.avg_price = Some(dec!(1000));

        // 10% 수익
        let ret = state.current_return(dec!(1100));
        assert_eq!(ret, Some(dec!(10)));

        // 5% 손실
        let ret = state.current_return(dec!(950));
        assert_eq!(ret, Some(dec!(-5)));
    }

    #[tokio::test]
    async fn test_initialization() {
        let mut strategy = InfinityBotStrategy::new();
        let config = json!({
            "ticker": "005930",
            "max_rounds": 30,
            "take_profit_pct": 5
        });

        strategy.initialize(config).await.unwrap();
        assert!(strategy.config.is_some());
        assert_eq!(strategy.state.current_round, 0);
    }
}
