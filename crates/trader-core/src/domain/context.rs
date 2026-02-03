//! 전략 실행 컨텍스트.
//!
//! 전략이 거래소 정보와 현재 포지션 상태를 실시간으로 조회하여
//! 의사결정에 활용할 수 있도록 합니다.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::analytics_provider::{
    GlobalScoreResult, MacroEnvironment, MarketBreadth, MarketRegime, RouteState,
    ScreeningResult, StructuralFeatures,
};
use super::market_data::Kline;
use super::order::{OrderStatusType, Side};
use crate::types::Symbol;
use crate::Timeframe;

// =============================================================================
// 다중 타임프레임 설정 (Phase 1.4.2)
// =============================================================================

/// 다중 타임프레임 설정.
///
/// 전략이 필요로 하는 타임프레임들과 각각의 캔들 개수를 명시합니다.
///
/// # 예시
///
/// ```rust,ignore
/// use trader_core::{Timeframe, domain::MultiTimeframeConfig};
///
/// let config = MultiTimeframeConfig::new()
///     .with_timeframe(Timeframe::D1, 60)   // 일봉 60개
///     .with_timeframe(Timeframe::H4, 120)  // 4시간봉 120개
///     .with_timeframe(Timeframe::H1, 240); // 1시간봉 240개
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MultiTimeframeConfig {
    /// 타임프레임별 캔들 개수 설정
    pub timeframes: HashMap<Timeframe, usize>,

    /// 기본 타임프레임 (분석의 기준이 되는 타임프레임)
    pub primary_timeframe: Option<Timeframe>,

    /// 자동 동기화 여부
    pub auto_sync: bool,
}

impl MultiTimeframeConfig {
    /// 빈 설정 생성.
    pub fn new() -> Self {
        Self {
            timeframes: HashMap::new(),
            primary_timeframe: None,
            auto_sync: true,
        }
    }

    /// 단일 타임프레임 설정 생성.
    pub fn single(timeframe: Timeframe, candle_count: usize) -> Self {
        let mut config = Self::new();
        config.timeframes.insert(timeframe, candle_count);
        config.primary_timeframe = Some(timeframe);
        config
    }

    /// 타임프레임 추가.
    pub fn with_timeframe(mut self, timeframe: Timeframe, candle_count: usize) -> Self {
        self.timeframes.insert(timeframe, candle_count);
        self
    }

    /// 기본 타임프레임 설정.
    pub fn with_primary(mut self, timeframe: Timeframe) -> Self {
        self.primary_timeframe = Some(timeframe);
        self
    }

    /// 자동 동기화 비활성화.
    pub fn without_auto_sync(mut self) -> Self {
        self.auto_sync = false;
        self
    }

    /// 설정된 타임프레임 목록 반환.
    pub fn get_timeframes(&self) -> Vec<Timeframe> {
        self.timeframes.keys().copied().collect()
    }

    /// 특정 타임프레임의 캔들 개수 반환.
    pub fn get_candle_count(&self, timeframe: Timeframe) -> usize {
        self.timeframes.get(&timeframe).copied().unwrap_or(60)
    }

    /// 기본 타임프레임 반환 (설정되지 않은 경우 첫 번째 타임프레임).
    pub fn get_primary_timeframe(&self) -> Option<Timeframe> {
        self.primary_timeframe.or_else(|| self.timeframes.keys().next().copied())
    }

    /// 설정이 비어있는지 확인.
    pub fn is_empty(&self) -> bool {
        self.timeframes.is_empty()
    }
}

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
    /// Global Score 결과 (ticker → 결과)
    pub global_scores: HashMap<String, GlobalScoreResult>,

    /// RouteState 결과 (ticker → 상태)
    pub route_states: HashMap<String, RouteState>,

    /// 스크리닝 결과 (프리셋명 → 결과 목록)
    pub screening_results: HashMap<String, Vec<ScreeningResult>>,

    /// 구조적 피처 (ticker → 피처)
    pub structural_features: HashMap<String, StructuralFeatures>,

    /// MarketRegime 결과 (ticker → 레짐)
    pub market_regime: HashMap<String, MarketRegime>,

    /// 매크로 환경 (환율, 나스닥 등)
    pub macro_environment: Option<MacroEnvironment>,

    /// 시장 폭 (20일선 상회 비율 등)
    pub market_breadth: Option<MarketBreadth>,

    // ===== 다중 타임프레임 데이터 (Phase 1.4.2) =====
    /// 타임프레임별 캔들 데이터 (ticker → (timeframe → klines))
    ///
    /// # 예시
    ///
    /// ```rust,ignore
    /// // 삼성전자의 일봉 데이터 조회
    /// let d1_klines = context.klines_by_timeframe
    ///     .get("005930")
    ///     .and_then(|tf_map| tf_map.get(&Timeframe::D1));
    ///
    /// // 또는 헬퍼 메서드 사용
    /// let d1_klines = context.get_klines("005930", Timeframe::D1);
    /// ```
    pub klines_by_timeframe: HashMap<String, HashMap<Timeframe, Vec<Kline>>>,

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
            market_regime: HashMap::new(),
            macro_environment: None,
            market_breadth: None,
            klines_by_timeframe: HashMap::new(),
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
    // 다중 타임프레임 메서드 (Phase 1.4.2)
    // =============================================================================

    /// 특정 심볼의 특정 타임프레임 캔들 데이터 조회.
    ///
    /// # 인자
    ///
    /// * `ticker` - 종목 심볼
    /// * `timeframe` - 타임프레임
    ///
    /// # 반환
    ///
    /// 캔들 데이터 슬라이스 (없으면 빈 슬라이스)
    pub fn get_klines(&self, ticker: &str, timeframe: Timeframe) -> &[Kline] {
        self.klines_by_timeframe
            .get(ticker)
            .and_then(|tf_map| tf_map.get(&timeframe))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// 여러 타임프레임의 캔들 데이터를 한 번에 조회.
    ///
    /// # 인자
    ///
    /// * `ticker` - 종목 심볼
    /// * `timeframes` - 조회할 타임프레임 목록
    ///
    /// # 반환
    ///
    /// (타임프레임, 캔들 데이터) 튜플의 벡터
    ///
    /// # 예시
    ///
    /// ```rust,ignore
    /// let data = context.get_multi_timeframe_klines(
    ///     "005930",
    ///     &[Timeframe::D1, Timeframe::H4, Timeframe::H1],
    /// );
    /// for (tf, klines) in data {
    ///     println!("{:?}: {} candles", tf, klines.len());
    /// }
    /// ```
    pub fn get_multi_timeframe_klines(
        &self,
        ticker: &str,
        timeframes: &[Timeframe],
    ) -> Vec<(Timeframe, &[Kline])> {
        timeframes
            .iter()
            .map(|&tf| (tf, self.get_klines(ticker, tf)))
            .collect()
    }

    /// 특정 심볼의 모든 타임프레임 데이터 조회.
    ///
    /// # 반환
    ///
    /// 사용 가능한 타임프레임 목록과 각각의 캔들 수
    pub fn get_available_timeframes(&self, ticker: &str) -> Vec<(Timeframe, usize)> {
        self.klines_by_timeframe
            .get(ticker)
            .map(|tf_map| {
                tf_map
                    .iter()
                    .map(|(&tf, klines)| (tf, klines.len()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 캔들 데이터 업데이트.
    ///
    /// # 인자
    ///
    /// * `ticker` - 종목 심볼
    /// * `timeframe` - 타임프레임
    /// * `klines` - 캔들 데이터
    pub fn update_klines(&mut self, ticker: &str, timeframe: Timeframe, klines: Vec<Kline>) {
        self.klines_by_timeframe
            .entry(ticker.to_string())
            .or_default()
            .insert(timeframe, klines);
    }

    /// 여러 타임프레임의 캔들 데이터 일괄 업데이트.
    ///
    /// # 인자
    ///
    /// * `ticker` - 종목 심볼
    /// * `data` - (타임프레임, 캔들 데이터) 튜플의 벡터
    pub fn update_multi_timeframe_klines(
        &mut self,
        ticker: &str,
        data: Vec<(Timeframe, Vec<Kline>)>,
    ) {
        let tf_map = self
            .klines_by_timeframe
            .entry(ticker.to_string())
            .or_default();

        for (timeframe, klines) in data {
            tf_map.insert(timeframe, klines);
        }
    }

    /// 특정 심볼의 캔들 데이터 모두 제거.
    pub fn clear_klines(&mut self, ticker: &str) {
        self.klines_by_timeframe.remove(ticker);
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
            if let Some(ticker) = score.ticker.clone() {
                self.global_scores.insert(ticker, score);
            }
        }
        self.last_analytics_sync = Utc::now();
    }

    /// RouteState 결과 업데이트.
    pub fn update_route_states(&mut self, states: HashMap<String, RouteState>) {
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
    pub fn update_features(&mut self, features: HashMap<String, StructuralFeatures>) {
        self.structural_features = features;
        self.last_analytics_sync = Utc::now();
    }

    /// MarketRegime 결과 업데이트.
    pub fn update_market_regime(&mut self, regimes: HashMap<String, MarketRegime>) {
        self.market_regime = regimes;
        self.last_analytics_sync = Utc::now();
    }

    /// 매크로 환경 업데이트.
    pub fn update_macro_environment(&mut self, env: MacroEnvironment) {
        self.macro_environment = Some(env);
        self.last_analytics_sync = Utc::now();
    }

    /// 시장 폭 업데이트.
    pub fn update_market_breadth(&mut self, breadth: MarketBreadth) {
        self.market_breadth = Some(breadth);
        self.last_analytics_sync = Utc::now();
    }

    // =============================================================================
    // 분석 결과 조회 헬퍼
    // =============================================================================

    /// 특정 종목의 RouteState 조회.
    pub fn get_route_state(&self, ticker: &str) -> Option<&RouteState> {
        self.route_states.get(ticker)
    }

    /// 특정 종목의 Global Score 조회.
    pub fn get_global_score(&self, ticker: &str) -> Option<&GlobalScoreResult> {
        self.global_scores.get(ticker)
    }

    /// 특정 종목의 구조적 피처 조회.
    pub fn get_features(&self, ticker: &str) -> Option<&StructuralFeatures> {
        self.structural_features.get(ticker)
    }

    /// 특정 종목의 MarketRegime 조회.
    pub fn get_market_regime(&self, ticker: &str) -> Option<&MarketRegime> {
        self.market_regime.get(ticker)
    }

    /// 매크로 환경 조회.
    pub fn get_macro_environment(&self) -> Option<&MacroEnvironment> {
        self.macro_environment.as_ref()
    }

    /// 시장 폭 조회.
    pub fn get_market_breadth(&self) -> Option<&MarketBreadth> {
        self.market_breadth.as_ref()
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
        let symbol = Symbol::new("AAPL", "USD", MarketType::Stock);
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
        let symbol = Symbol::new("AAPL", "USD", MarketType::Stock);
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
        let sym1 = Symbol::new("AAPL", "USD", MarketType::Stock);
        let mut pos1 = StrategyPositionInfo::new(sym1, Side::Buy, dec!(10), dec!(150));
        pos1.update_price(dec!(160)); // 1600

        let sym2 = Symbol::new("MSFT", "USD", MarketType::Stock);
        let mut pos2 = StrategyPositionInfo::new(sym2, Side::Buy, dec!(5), dec!(300));
        pos2.update_price(dec!(310)); // 1550

        ctx.positions.insert("AAPL".to_string(), pos1);
        ctx.positions.insert("MSFT".to_string(), pos2);

        // 총 가치: 1600 + 1550 = 3150
        assert_eq!(ctx.total_position_value(), dec!(3150));
    }
}
