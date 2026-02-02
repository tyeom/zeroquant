//! 전략 실행 컨텍스트.
//!
//! 전략이 거래소 정보와 현재 포지션 상태를 실시간으로 조회하여
//! 의사결정에 활용할 수 있도록 합니다.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::analytics_provider::{
    GlobalScoreResult, RouteState, ScreeningResult, StructuralFeatures,
};
use super::order::{OrderStatusType, Side};
use crate::types::Symbol;

// =============================================================================
// 계좌 정보
// =============================================================================

/// 전략용 실시간 계좌 정보 (집계된 정보).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyAccountInfo {
    /// 총 자산 (현금 + 포지션 평가액)
    pub total_balance: Decimal,
    /// 매수 가능 금액 (사용 가능한 현금)
    pub available_balance: Decimal,
    /// 사용 중인 증거금 (레버리지 거래 시)
    pub margin_used: Decimal,
    /// 미실현 손익 합계
    pub unrealized_pnl: Decimal,
    /// 계좌 통화 (KRW, USD 등)
    pub currency: String,
}

impl Default for StrategyAccountInfo {
    fn default() -> Self {
        Self {
            total_balance: Decimal::ZERO,
            available_balance: Decimal::ZERO,
            margin_used: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
            currency: "KRW".to_string(),
        }
    }
}

// =============================================================================
// 포지션 정보
// =============================================================================

/// 전략용 포지션 상세 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyPositionInfo {
    /// 심볼
    pub symbol: Symbol,
    /// 방향 (매수/매도)
    pub side: Side,
    /// 보유 수량
    pub quantity: Decimal,
    /// 평균 진입가
    pub avg_entry_price: Decimal,
    /// 현재가 (실시간 시세)
    pub current_price: Decimal,
    /// 미실현 손익
    pub unrealized_pnl: Decimal,
    /// 미실현 손익률 (%)
    pub unrealized_pnl_pct: Decimal,
    /// 청산가 (레버리지 거래 시)
    pub liquidation_price: Option<Decimal>,
    /// 포지션 생성 시각
    pub created_at: DateTime<Utc>,
    /// 마지막 업데이트 시각
    pub updated_at: DateTime<Utc>,
}

impl StrategyPositionInfo {
    /// 새 포지션 정보 생성.
    pub fn new(symbol: Symbol, side: Side, quantity: Decimal, avg_entry_price: Decimal) -> Self {
        let now = Utc::now();
        Self {
            symbol,
            side,
            quantity,
            avg_entry_price,
            current_price: avg_entry_price,
            unrealized_pnl: Decimal::ZERO,
            unrealized_pnl_pct: Decimal::ZERO,
            liquidation_price: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// 현재가 업데이트 및 미실현 손익 재계산.
    pub fn update_price(&mut self, current_price: Decimal) {
        self.current_price = current_price;
        self.updated_at = Utc::now();

        // 미실현 손익 계산
        let price_diff = match self.side {
            Side::Buy => current_price - self.avg_entry_price,
            Side::Sell => self.avg_entry_price - current_price,
        };
        self.unrealized_pnl = price_diff * self.quantity;

        // 수익률 계산
        if self.avg_entry_price > Decimal::ZERO {
            self.unrealized_pnl_pct =
                (self.unrealized_pnl / (self.avg_entry_price * self.quantity)) * Decimal::from(100);
        }
    }
}

// =============================================================================
// 미체결 주문
// =============================================================================

/// 미체결 주문 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingOrder {
    /// 주문 ID
    pub order_id: String,
    /// 심볼
    pub symbol: Symbol,
    /// 방향
    pub side: Side,
    /// 주문 가격
    pub price: Decimal,
    /// 주문 수량
    pub quantity: Decimal,
    /// 체결 수량
    pub filled_quantity: Decimal,
    /// 상태
    pub status: OrderStatusType,
    /// 주문 시각
    pub created_at: DateTime<Utc>,
}

// =============================================================================
// 거래소 제약 조건
// =============================================================================

/// 거래 시간대.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingHours {
    /// 개장 시각 (UTC)
    pub open: DateTime<Utc>,
    /// 폐장 시각 (UTC)
    pub close: DateTime<Utc>,
    /// 점심 시간 시작 (선택적)
    pub lunch_start: Option<DateTime<Utc>>,
    /// 점심 시간 종료 (선택적)
    pub lunch_end: Option<DateTime<Utc>>,
}

/// 거래소 제약 조건.
#[derive(Debug, Clone)]
pub struct ExchangeConstraints {
    /// 최소 주문 수량
    pub min_order_qty: Decimal,
    /// 최대 레버리지 (선택적)
    pub max_leverage: Option<Decimal>,
    /// 거래 시간 (선택적, 24/7 거래소는 None)
    pub trading_hours: Option<TradingHours>,
    /// 거래 수수료율 (Taker)
    pub taker_fee_rate: Decimal,
    /// 거래 수수료율 (Maker)
    pub maker_fee_rate: Decimal,
}

impl Default for ExchangeConstraints {
    fn default() -> Self {
        Self {
            min_order_qty: Decimal::ONE,
            max_leverage: None,
            trading_hours: None,
            taker_fee_rate: Decimal::ZERO,
            maker_fee_rate: Decimal::ZERO,
        }
    }
}

// =============================================================================
// 전략 컨텍스트
// =============================================================================

/// 전략 실행 컨텍스트.
///
/// 전략이 실시간으로 참조할 수 있는 거래소 정보와 분석 결과를 담고 있습니다.
#[derive(Debug, Clone)]
pub struct StrategyContext {
    // ===== 거래소 실시간 정보 =====
    /// 계좌 정보 (거래소에서 실시간 조회)
    pub account: StrategyAccountInfo,

    /// 현재 보유 포지션 (전략 간 공유)
    pub positions: HashMap<String, StrategyPositionInfo>,

    /// 미체결 주문 목록
    pub pending_orders: Vec<PendingOrder>,

    /// 거래소 제약 조건
    pub exchange_constraints: ExchangeConstraints,

    // ===== 분석 결과 (1~10분 갱신) =====
    /// Global Score 결과 (종목별)
    pub global_scores: HashMap<Symbol, GlobalScoreResult>,

    /// RouteState 결과 (종목별)
    pub route_states: HashMap<Symbol, RouteState>,

    /// 스크리닝 결과 (프리셋명 → 결과 목록)
    pub screening_results: HashMap<String, Vec<ScreeningResult>>,

    /// 구조적 피처 (종목별)
    pub structural_features: HashMap<Symbol, StructuralFeatures>,

    // ===== 메타 정보 =====
    /// 마지막 거래소 동기화 시간
    pub last_exchange_sync: DateTime<Utc>,

    /// 마지막 분석 결과 동기화 시간
    pub last_analytics_sync: DateTime<Utc>,

    /// 컨텍스트 생성 시각
    pub created_at: DateTime<Utc>,
}

impl Default for StrategyContext {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            account: StrategyAccountInfo::default(),
            positions: HashMap::new(),
            pending_orders: Vec::new(),
            exchange_constraints: ExchangeConstraints::default(),
            global_scores: HashMap::new(),
            route_states: HashMap::new(),
            screening_results: HashMap::new(),
            structural_features: HashMap::new(),
            last_exchange_sync: now,
            last_analytics_sync: now,
            created_at: now,
        }
    }
}

impl StrategyContext {
    /// 새 컨텍스트 생성.
    pub fn new() -> Self {
        Self::default()
    }

    /// 특정 심볼의 포지션 조회.
    pub fn get_position(&self, symbol: &str) -> Option<&StrategyPositionInfo> {
        self.positions.get(symbol)
    }

    /// 포지션 보유 여부 확인.
    pub fn has_position(&self, symbol: &str) -> bool {
        self.positions.contains_key(symbol)
    }

    /// 특정 심볼의 미체결 주문 조회.
    pub fn get_pending_orders(&self, symbol: &str) -> Vec<&PendingOrder> {
        self.pending_orders
            .iter()
            .filter(|o| o.symbol.base == symbol)
            .collect()
    }

    /// 미체결 주문 존재 여부 확인.
    pub fn has_pending_order(&self, symbol: &str) -> bool {
        self.pending_orders.iter().any(|o| o.symbol.base == symbol)
    }

    /// 총 포지션 가치 계산.
    pub fn total_position_value(&self) -> Decimal {
        self.positions
            .values()
            .map(|p| p.current_price * p.quantity)
            .sum()
    }

    /// 거래소 동기화 만료 여부 확인.
    ///
    /// # Arguments
    ///
    /// * `max_age_secs` - 최대 허용 시간 (초)
    pub fn is_exchange_sync_stale(&self, max_age_secs: i64) -> bool {
        let now = Utc::now();
        let age = now.signed_duration_since(self.last_exchange_sync);
        age.num_seconds() > max_age_secs
    }

    // =============================================================================
    // 거래소 정보 업데이트 메서드
    // =============================================================================

    /// 계좌 정보 업데이트.
    pub fn update_account(&mut self, account: StrategyAccountInfo) {
        self.account = account;
        self.last_exchange_sync = Utc::now();
    }

    /// 포지션 정보 업데이트.
    ///
    /// 기존 포지션을 모두 지우고 새 포지션으로 교체합니다.
    pub fn update_positions(&mut self, positions: Vec<StrategyPositionInfo>) {
        self.positions.clear();
        for pos in positions {
            self.positions.insert(pos.symbol.to_standard_string(), pos);
        }
        self.last_exchange_sync = Utc::now();
    }

    /// 미체결 주문 업데이트.
    pub fn update_pending_orders(&mut self, orders: Vec<PendingOrder>) {
        self.pending_orders = orders;
        self.last_exchange_sync = Utc::now();
    }

    // =============================================================================
    // 분석 결과 업데이트 메서드
    // =============================================================================

    /// Global Score 결과 업데이트.
    ///
    /// 기존 스코어를 모두 지우고 새 스코어로 교체합니다.
    pub fn update_global_scores(&mut self, scores: Vec<GlobalScoreResult>) {
        self.global_scores.clear();
        for score in scores {
            if let Some(symbol) = score.symbol.clone() {
                self.global_scores.insert(symbol, score);
            }
        }
        self.last_analytics_sync = Utc::now();
    }

    /// RouteState 결과 업데이트.
    pub fn update_route_states(&mut self, states: HashMap<Symbol, RouteState>) {
        self.route_states = states;
        self.last_analytics_sync = Utc::now();
    }

    /// 스크리닝 결과 업데이트.
    ///
    /// 특정 프리셋의 스크리닝 결과를 업데이트합니다.
    pub fn update_screening(&mut self, preset_name: String, results: Vec<ScreeningResult>) {
        self.screening_results.insert(preset_name, results);
        self.last_analytics_sync = Utc::now();
    }

    /// 구조적 피처 업데이트.
    pub fn update_features(&mut self, features: HashMap<Symbol, StructuralFeatures>) {
        self.structural_features = features;
        self.last_analytics_sync = Utc::now();
    }

    // =============================================================================
    // 분석 결과 조회 헬퍼
    // =============================================================================

    /// 특정 심볼의 RouteState 조회.
    pub fn get_route_state(&self, symbol: &Symbol) -> Option<&RouteState> {
        self.route_states.get(symbol)
    }

    /// 특정 심볼의 Global Score 조회.
    pub fn get_global_score(&self, symbol: &Symbol) -> Option<&GlobalScoreResult> {
        self.global_scores.get(symbol)
    }

    /// 특정 심볼의 구조적 피처 조회.
    pub fn get_features(&self, symbol: &Symbol) -> Option<&StructuralFeatures> {
        self.structural_features.get(symbol)
    }

    /// 분석 결과 동기화 만료 여부 확인.
    ///
    /// # Arguments
    ///
    /// * `max_age_secs` - 최대 허용 시간 (초)
    pub fn is_analytics_sync_stale(&self, max_age_secs: i64) -> bool {
        (Utc::now() - self.last_analytics_sync).num_seconds() > max_age_secs
    }
}

// =============================================================================
// 테스트
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MarketType;
    use rust_decimal_macros::dec;

    #[test]
    fn test_account_info_default() {
        let account = StrategyAccountInfo::default();
        assert_eq!(account.total_balance, Decimal::ZERO);
        assert_eq!(account.currency, "KRW");
    }

    #[test]
    fn test_position_info_update_price() {
        let symbol = Symbol::new("AAPL", "USD", MarketType::UsStock);
        let mut pos = StrategyPositionInfo::new(symbol, Side::Buy, dec!(10), dec!(150));

        // 가격 상승 → 수익
        pos.update_price(dec!(160));
        assert_eq!(pos.unrealized_pnl, dec!(100)); // (160-150) * 10
        assert!(pos.unrealized_pnl_pct > Decimal::ZERO);

        // 가격 하락 → 손실
        pos.update_price(dec!(140));
        assert_eq!(pos.unrealized_pnl, dec!(-100)); // (140-150) * 10
        assert!(pos.unrealized_pnl_pct < Decimal::ZERO);
    }

    #[test]
    fn test_strategy_context_position_query() {
        let mut ctx = StrategyContext::new();

        // 포지션 추가
        let symbol = Symbol::new("AAPL", "USD", MarketType::UsStock);
        let pos = StrategyPositionInfo::new(symbol, Side::Buy, dec!(10), dec!(150));
        ctx.positions.insert("AAPL".to_string(), pos);

        // 조회 테스트
        assert!(ctx.has_position("AAPL"));
        assert!(!ctx.has_position("MSFT"));
        assert!(ctx.get_position("AAPL").is_some());
        assert!(ctx.get_position("MSFT").is_none());
    }

    #[test]
    fn test_total_position_value() {
        let mut ctx = StrategyContext::new();

        // 포지션 2개 추가
        let sym1 = Symbol::new("AAPL", "USD", MarketType::UsStock);
        let mut pos1 = StrategyPositionInfo::new(sym1, Side::Buy, dec!(10), dec!(150));
        pos1.update_price(dec!(160)); // 1600

        let sym2 = Symbol::new("MSFT", "USD", MarketType::UsStock);
        let mut pos2 = StrategyPositionInfo::new(sym2, Side::Buy, dec!(5), dec!(300));
        pos2.update_price(dec!(310)); // 1550

        ctx.positions.insert("AAPL".to_string(), pos1);
        ctx.positions.insert("MSFT".to_string(), pos2);

        // 총 가치: 1600 + 1550 = 3150
        assert_eq!(ctx.total_position_value(), dec!(3150));
    }
}
