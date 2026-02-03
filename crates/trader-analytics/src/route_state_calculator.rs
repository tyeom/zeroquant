//! RouteState 계산 로직.
//!
//! StructuralFeatures와 기술적 지표를 기반으로 종목의 매매 단계(RouteState)를 판정합니다.

use crate::indicators::{IndicatorEngine, IndicatorError, IndicatorResult, StructuralFeatures};
use rust_decimal::Decimal;
use trader_core::{Kline, RouteState};

/// RouteState 계산기.
///
/// StructuralFeatures와 추가 지표를 조합하여 종목의 현재 매매 단계를 판정합니다.
pub struct RouteStateCalculator {
    engine: IndicatorEngine,
}

impl RouteStateCalculator {
    /// 새로운 계산기 인스턴스 생성.
    pub fn new() -> Self {
        Self {
            engine: IndicatorEngine::new(),
        }
    }

    /// 캔들 데이터로부터 RouteState 계산.
    ///
    /// # 인자
    ///
    /// * `candles` - 최소 40개의 캔들 데이터
    ///
    /// # 반환
    ///
    /// 계산된 RouteState
    ///
    /// # 우선순위
    ///
    /// 1. Overheat (위험 신호 최우선)
    /// 2. Attack (진입 기회)
    /// 3. Armed (대기 준비)
    /// 4. Wait (관찰)
    /// 5. Neutral (기본값)
    pub fn calculate(&self, candles: &[Kline]) -> IndicatorResult<RouteState> {
        if candles.len() < 40 {
            return Err(IndicatorError::InsufficientData {
                required: 40,
                provided: candles.len(),
            });
        }

        // StructuralFeatures 계산
        let features = StructuralFeatures::from_candles(candles, &self.engine)?;

        // 5일 수익률 계산
        let return_5d = self.calculate_return_5d(candles)?;

        // 우선순위에 따라 상태 판정
        // 1. Overheat (가장 높은 우선순위)
        if self.is_overheat(&features, return_5d) {
            return Ok(RouteState::Overheat);
        }

        // 2. Attack
        // Note: TTM Squeeze는 Phase 1.2.x에서 구현 예정
        // 현재는 StructuralFeatures만으로 판정
        if self.is_attack(&features) {
            return Ok(RouteState::Attack);
        }

        // 3. Armed
        if self.is_armed(&features) {
            return Ok(RouteState::Armed);
        }

        // 4. Wait
        if self.is_wait(&features) {
            return Ok(RouteState::Wait);
        }

        // 5. Neutral (기본값)
        Ok(RouteState::Neutral)
    }

    /// Overheat 판정.
    ///
    /// **조건**:
    /// - 5일 수익률 > 20% 또는
    /// - RSI >= 75
    fn is_overheat(&self, features: &StructuralFeatures, return_5d: f64) -> bool {
        return_5d > 20.0 || features.rsi >= 75.0
    }

    /// Attack 판정.
    ///
    /// **조건**:
    /// - RSI 45~65 (건강한 범위)
    /// - Range_Pos >= 0.8 (박스권 상단)
    /// - Low_Trend > 0 (저가 상승)
    /// - Vol_Quality > 0 (매집 신호)
    ///
    /// Note: TTM Squeeze 조건은 Phase 1.2.x에서 추가 예정
    fn is_attack(&self, features: &StructuralFeatures) -> bool {
        features.rsi >= 45.0
            && features.rsi <= 65.0
            && features.range_pos >= 0.8
            && features.low_trend > 0.0
            && features.vol_quality > 0.0
    }

    /// Armed 판정.
    ///
    /// **조건**:
    /// - Vol_Quality >= 0.2 (강한 매집) 또는
    /// - Dist_MA20 > -2.0 (MA20 근처 또는 위)
    ///
    /// Note: TTM Squeeze 조건은 Phase 1.2.x에서 추가 예정
    fn is_armed(&self, features: &StructuralFeatures) -> bool {
        features.vol_quality >= 0.2 || features.dist_ma20 > -2.0
    }

    /// Wait 판정.
    ///
    /// **조건**:
    /// - Low_Trend > 0 (저가 상승 = 정배열 유지)
    /// - Dist_MA20 > -5.0 (MA 지지)
    fn is_wait(&self, features: &StructuralFeatures) -> bool {
        features.low_trend > 0.0 && features.dist_ma20 > -5.0
    }

    /// 5일 수익률 계산 (%).
    ///
    /// (최근 종가 / 5일 전 종가 - 1) * 100
    fn calculate_return_5d(&self, candles: &[Kline]) -> IndicatorResult<f64> {
        let len = candles.len();
        if len < 6 {
            return Err(IndicatorError::InsufficientData {
                required: 6,
                provided: len,
            });
        }

        let current = candles[len - 1].close;
        let past = candles[len - 6].close;

        if past == Decimal::ZERO {
            return Ok(0.0);
        }

        let return_pct = ((current / past - Decimal::ONE) * Decimal::from(100))
            .to_string()
            .parse::<f64>()
            .unwrap_or(0.0);

        Ok(return_pct)
    }
}

impl Default for RouteStateCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use trader_core::{types::Symbol, MarketType, Timeframe};

    fn create_test_candles(count: usize, trend: &str) -> Vec<Kline> {
        let symbol = Symbol::new("TEST", "USD", MarketType::KrStock);
        let mut candles = Vec::with_capacity(count);

        for i in 0..count {
            let base_price = dec!(100.0) + Decimal::from(i as i64);
            let (open, close) = match trend {
                "up" => (base_price, base_price + dec!(1.0)),
                "down" => (base_price, base_price - dec!(1.0)),
                _ => (base_price, base_price),
            };

            let now = Utc::now();
            candles.push(Kline {
                symbol: symbol.clone(),
                timeframe: Timeframe::D1,
                open_time: now,
                open,
                high: open.max(close) + dec!(0.5),
                low: open.min(close) - dec!(0.5),
                close,
                volume: dec!(1000000.0),
                close_time: now + chrono::Duration::days(1),
                quote_volume: None,
                num_trades: None,
            });
        }

        candles
    }

    #[test]
    fn test_insufficient_data() {
        let calculator = RouteStateCalculator::new();
        let candles = create_test_candles(30, "up");

        let result = calculator.calculate(&candles);
        assert!(result.is_err());
    }

    #[test]
    fn test_return_5d_calculation() {
        let calculator = RouteStateCalculator::new();
        let mut candles = create_test_candles(50, "neutral");

        // 5일 전: 100, 현재: 120 → 20% 상승
        candles[44].close = dec!(100.0);
        candles[49].close = dec!(120.0);

        let return_5d = calculator.calculate_return_5d(&candles).unwrap();
        assert!((return_5d - 20.0).abs() < 0.1);
    }

    #[test]
    fn test_overheat_by_return() {
        let calculator = RouteStateCalculator::new();
        let mut candles = create_test_candles(50, "up");

        // 5일 수익률 > 20%
        candles[44].close = dec!(100.0);
        candles[49].close = dec!(125.0);

        let state = calculator.calculate(&candles).unwrap();
        assert_eq!(state, RouteState::Overheat);
    }

    #[test]
    fn test_default() {
        let calc1 = RouteStateCalculator::new();
        let calc2 = RouteStateCalculator::default();

        let candles = create_test_candles(50, "up");
        let state1 = calc1.calculate(&candles).unwrap();
        let state2 = calc2.calculate(&candles).unwrap();

        assert_eq!(state1, state2);
    }
}
