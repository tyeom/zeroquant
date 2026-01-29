//! 기술적 지표 모듈.
//!
//! 이 모듈은 트레이딩 전략에서 사용되는 다양한 기술적 지표를 제공합니다.
//! ta-rs 라이브러리를 기반으로 구현되었으며, 추가적인 커스텀 지표도 포함합니다.
//!
//! # 지원 지표
//!
//! ## 추세 지표 (Trend Indicators)
//! - **SMA**: 단순 이동평균 (Simple Moving Average)
//! - **EMA**: 지수 이동평균 (Exponential Moving Average)
//! - **MACD**: 이동평균 수렴/확산 (Moving Average Convergence Divergence)
//!
//! ## 모멘텀 지표 (Momentum Indicators)
//! - **RSI**: 상대강도지수 (Relative Strength Index)
//! - **Stochastic**: 스토캐스틱 오실레이터
//! - **모멘텀 점수**: 다기간 평균 모멘텀
//!
//! ## 변동성 지표 (Volatility Indicators)
//! - **Bollinger Bands**: 볼린저 밴드
//! - **ATR**: 평균 실제 범위 (Average True Range)
//!
//! # 사용 예시
//!
//! ```ignore
//! use trader_analytics::indicators::{IndicatorEngine, SmaParams, RsiParams};
//!
//! let engine = IndicatorEngine::new();
//!
//! // SMA 계산
//! let sma = engine.sma(&prices, SmaParams { period: 20 })?;
//!
//! // RSI 계산
//! let rsi = engine.rsi(&prices, RsiParams { period: 14 })?;
//! ```

pub mod momentum;
pub mod trend;
pub mod volatility;

use rust_decimal::Decimal;
use thiserror::Error;

pub use momentum::{MomentumCalculator, RsiParams, StochasticParams, StochasticResult};
pub use trend::{EmaParams, MacdParams, MacdResult, SmaParams, TrendIndicators};
pub use volatility::{AtrParams, BollingerBandsParams, BollingerBandsResult, VolatilityIndicators};

/// 지표 계산 오류.
#[derive(Debug, Error)]
pub enum IndicatorError {
    /// 데이터 부족 오류
    #[error("데이터가 부족합니다: 필요 {required}개, 제공 {provided}개")]
    InsufficientData { required: usize, provided: usize },

    /// 잘못된 파라미터
    #[error("잘못된 파라미터: {0}")]
    InvalidParameter(String),

    /// 계산 오류
    #[error("계산 오류: {0}")]
    CalculationError(String),
}

/// 지표 계산 결과 타입.
pub type IndicatorResult<T> = Result<T, IndicatorError>;

/// 통합 지표 엔진.
///
/// 모든 기술적 지표 계산을 위한 통합 인터페이스를 제공합니다.
/// 내부적으로 ta-rs 라이브러리와 커스텀 구현을 사용합니다.
#[derive(Debug, Default)]
pub struct IndicatorEngine {
    trend: TrendIndicators,
    momentum: MomentumCalculator,
    volatility: VolatilityIndicators,
}

impl IndicatorEngine {
    /// 새로운 지표 엔진 생성.
    pub fn new() -> Self {
        Self::default()
    }

    // ==================== 추세 지표 ====================

    /// 단순 이동평균 (SMA) 계산.
    ///
    /// # 인자
    /// * `prices` - 가격 데이터 (종가)
    /// * `params` - SMA 파라미터 (기간)
    ///
    /// # 반환
    /// 계산된 SMA 값들의 벡터 (처음 period-1개는 None)
    pub fn sma(&self, prices: &[Decimal], params: SmaParams) -> IndicatorResult<Vec<Option<Decimal>>> {
        self.trend.sma(prices, params)
    }

    /// 지수 이동평균 (EMA) 계산.
    ///
    /// # 인자
    /// * `prices` - 가격 데이터 (종가)
    /// * `params` - EMA 파라미터 (기간)
    ///
    /// # 반환
    /// 계산된 EMA 값들의 벡터
    pub fn ema(&self, prices: &[Decimal], params: EmaParams) -> IndicatorResult<Vec<Option<Decimal>>> {
        self.trend.ema(prices, params)
    }

    /// MACD (Moving Average Convergence Divergence) 계산.
    ///
    /// # 인자
    /// * `prices` - 가격 데이터 (종가)
    /// * `params` - MACD 파라미터 (단기, 장기, 시그널 기간)
    ///
    /// # 반환
    /// MACD 라인, 시그널 라인, 히스토그램
    pub fn macd(&self, prices: &[Decimal], params: MacdParams) -> IndicatorResult<Vec<MacdResult>> {
        self.trend.macd(prices, params)
    }

    // ==================== 모멘텀 지표 ====================

    /// RSI (Relative Strength Index) 계산.
    ///
    /// # 인자
    /// * `prices` - 가격 데이터 (종가)
    /// * `params` - RSI 파라미터 (기간, 기본값 14)
    ///
    /// # 반환
    /// 0-100 사이의 RSI 값들
    pub fn rsi(&self, prices: &[Decimal], params: RsiParams) -> IndicatorResult<Vec<Option<Decimal>>> {
        self.momentum.rsi(prices, params)
    }

    /// 스토캐스틱 오실레이터 계산.
    ///
    /// # 인자
    /// * `high` - 고가 데이터
    /// * `low` - 저가 데이터
    /// * `close` - 종가 데이터
    /// * `params` - 스토캐스틱 파라미터 (%K, %D 기간)
    ///
    /// # 반환
    /// %K, %D 값들
    pub fn stochastic(
        &self,
        high: &[Decimal],
        low: &[Decimal],
        close: &[Decimal],
        params: StochasticParams,
    ) -> IndicatorResult<Vec<StochasticResult>> {
        self.momentum.stochastic(high, low, close, params)
    }

    /// 다기간 모멘텀 점수 계산.
    ///
    /// Python 전략 코드의 모멘텀 계산 방식을 따릅니다:
    /// 모멘텀 = (1개월 + 3개월 + 6개월 + 12개월) / 4
    ///
    /// # 인자
    /// * `prices` - 가격 데이터 (종가)
    /// * `lookback_periods` - 참조 기간들 (일 단위)
    ///
    /// # 반환
    /// 현재 가격 기준 모멘텀 점수
    pub fn momentum_score(
        &self,
        prices: &[Decimal],
        lookback_periods: &[usize],
    ) -> IndicatorResult<Decimal> {
        self.momentum.momentum_score(prices, lookback_periods)
    }

    // ==================== 변동성 지표 ====================

    /// 볼린저 밴드 계산.
    ///
    /// # 인자
    /// * `prices` - 가격 데이터 (종가)
    /// * `params` - 볼린저 밴드 파라미터 (기간, 표준편차 배수)
    ///
    /// # 반환
    /// 상단, 중간, 하단 밴드 값들
    pub fn bollinger_bands(
        &self,
        prices: &[Decimal],
        params: BollingerBandsParams,
    ) -> IndicatorResult<Vec<BollingerBandsResult>> {
        self.volatility.bollinger_bands(prices, params)
    }

    /// ATR (Average True Range) 계산.
    ///
    /// # 인자
    /// * `high` - 고가 데이터
    /// * `low` - 저가 데이터
    /// * `close` - 종가 데이터
    /// * `params` - ATR 파라미터 (기간, 기본값 14)
    ///
    /// # 반환
    /// ATR 값들
    pub fn atr(
        &self,
        high: &[Decimal],
        low: &[Decimal],
        close: &[Decimal],
        params: AtrParams,
    ) -> IndicatorResult<Vec<Option<Decimal>>> {
        self.volatility.atr(high, low, close, params)
    }

    // ==================== 유틸리티 ====================

    /// 골든 크로스 감지.
    ///
    /// 단기 이동평균이 장기 이동평균을 상향 돌파하는 시점을 감지합니다.
    ///
    /// # 인자
    /// * `short_ma` - 단기 이동평균 값들
    /// * `long_ma` - 장기 이동평균 값들
    ///
    /// # 반환
    /// 각 시점에서 골든 크로스 발생 여부
    pub fn detect_golden_cross(
        &self,
        short_ma: &[Option<Decimal>],
        long_ma: &[Option<Decimal>],
    ) -> Vec<bool> {
        self.trend.detect_golden_cross(short_ma, long_ma)
    }

    /// 데드 크로스 감지.
    ///
    /// 단기 이동평균이 장기 이동평균을 하향 돌파하는 시점을 감지합니다.
    ///
    /// # 인자
    /// * `short_ma` - 단기 이동평균 값들
    /// * `long_ma` - 장기 이동평균 값들
    ///
    /// # 반환
    /// 각 시점에서 데드 크로스 발생 여부
    pub fn detect_dead_cross(
        &self,
        short_ma: &[Option<Decimal>],
        long_ma: &[Option<Decimal>],
    ) -> Vec<bool> {
        self.trend.detect_dead_cross(short_ma, long_ma)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sample_prices() -> Vec<Decimal> {
        vec![
            dec!(100.0),
            dec!(102.0),
            dec!(101.0),
            dec!(103.0),
            dec!(105.0),
            dec!(104.0),
            dec!(106.0),
            dec!(108.0),
            dec!(107.0),
            dec!(109.0),
            dec!(111.0),
            dec!(110.0),
            dec!(112.0),
            dec!(114.0),
            dec!(113.0),
        ]
    }

    #[test]
    fn test_sma_calculation() {
        let engine = IndicatorEngine::new();
        let prices = sample_prices();

        let sma = engine.sma(&prices, SmaParams { period: 5 }).unwrap();

        // 처음 4개는 None (데이터 부족)
        assert!(sma[0].is_none());
        assert!(sma[3].is_none());

        // 5번째부터 값이 있어야 함
        assert!(sma[4].is_some());
    }

    #[test]
    fn test_rsi_calculation() {
        let engine = IndicatorEngine::new();
        let prices = sample_prices();

        let rsi = engine.rsi(&prices, RsiParams { period: 14 }).unwrap();

        // RSI 값이 0-100 범위인지 확인
        for value in rsi.iter().flatten() {
            assert!(*value >= Decimal::ZERO);
            assert!(*value <= dec!(100));
        }
    }

    #[test]
    fn test_insufficient_data_error() {
        let engine = IndicatorEngine::new();
        let prices = vec![dec!(100.0), dec!(101.0)];

        let result = engine.sma(&prices, SmaParams { period: 20 });
        assert!(result.is_err());
    }
}
