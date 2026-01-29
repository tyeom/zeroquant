//! 분석 및 백테스팅 엔진.
//!
//! 이 크레이트는 다음을 제공합니다:
//! - 성과 지표 계산
//! - 포트폴리오 분석 (자산 곡선, 차트)
//! - 백테스팅 엔진
//! - ML/AI 모델 추론 (ONNX) - `ml` feature 필요
//! - 기술적 지표
//!
//! # Re-exports
//!
//! - [`performance`]: 성과 지표 계산 (PerformanceMetrics, RollingMetrics 등)
//! - [`portfolio`]: 포트폴리오 분석 (EquityCurve, PortfolioCharts 등)
//! - [`ml`]: ML/AI 기능 (패턴 인식, 피처 추출, 예측) - `ml` feature 필요

pub mod backtest;
pub mod indicators;
#[cfg(feature = "ml")]
pub mod ml;
pub mod performance;
pub mod portfolio;

// Performance 모듈 re-exports
pub use performance::metrics::{
    PerformanceMetrics, RollingMetrics, RoundTrip, DEFAULT_RISK_FREE_RATE, TRADING_DAYS_PER_YEAR,
};
pub use performance::tracker::{PerformanceEvent, PerformanceThresholds, PerformanceTracker};

// Portfolio 모듈 re-exports
pub use portfolio::charts::{ChartPoint, MonthlyReturnCell, PeriodPerformance, PerformanceSummary, PortfolioCharts};
pub use portfolio::equity_curve::{DrawdownPeriod, EquityCurve, EquityCurveBuilder, EquityPoint, TimeFrame};

// Indicators 모듈 re-exports
pub use indicators::{
    IndicatorEngine, IndicatorError, IndicatorResult,
    // 추세 지표
    SmaParams, EmaParams, MacdParams, MacdResult, TrendIndicators,
    // 모멘텀 지표
    RsiParams, StochasticParams, StochasticResult, MomentumCalculator,
    // 변동성 지표
    BollingerBandsParams, BollingerBandsResult, AtrParams, VolatilityIndicators,
};

// ML 모듈 re-exports (ml feature 필요)
#[cfg(feature = "ml")]
pub use ml::{
    // 에러 타입
    MlError, MlResult,
    // 피처 추출
    FeatureConfig, FeatureExtractor, FeatureVector,
    // 패턴 인식
    CandlestickPattern, CandlestickPatternType, ChartPattern, ChartPatternType,
    PatternConfig, PatternRecognizer,
    // 예측
    ConfidenceLevel, MockPredictor, OnnxPredictor, Prediction, PredictionDirection,
    PredictionResult, PredictorConfig, PricePredictor,
    // 통합 서비스
    CandlestickPatternInfo, ChartPatternInfo, FeatureSummary, MlAnalysisResult,
    MlService, MlServiceConfig, PatternDetectionResult,
};
