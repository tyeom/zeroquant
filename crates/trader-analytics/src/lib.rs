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
pub mod global_scorer;
pub mod indicators;
pub mod journal_integration;
pub mod liquidity_gate;
pub mod market_regime_calculator;
#[cfg(feature = "ml")]
pub mod ml;
pub mod performance;
pub mod portfolio;
pub mod route_state_calculator;
pub mod trigger_calculator;

// Performance 모듈 re-exports
pub use performance::metrics::{
    PerformanceMetrics, RollingMetrics, RoundTrip, DEFAULT_RISK_FREE_RATE, TRADING_DAYS_PER_YEAR,
};
pub use performance::tracker::{PerformanceEvent, PerformanceThresholds, PerformanceTracker};

// Portfolio 모듈 re-exports
pub use portfolio::charts::{
    ChartPoint, MonthlyReturnCell, PerformanceSummary, PeriodPerformance, PortfolioCharts,
};
pub use portfolio::equity_curve::{
    DrawdownPeriod, EquityCurve, EquityCurveBuilder, EquityPoint, TimeFrame,
};

// Journal Integration 모듈 re-exports
pub use journal_integration::{
    export_backtest_to_journal, export_backtest_trades, JournalTradeInput,
};

// Indicators 모듈 re-exports
pub use indicators::{
    AtrParams,
    // 변동성 지표
    BollingerBandsParams,
    BollingerBandsResult,
    EmaParams,
    IndicatorEngine,
    IndicatorError,
    IndicatorResult,
    MacdParams,
    MacdResult,
    MomentumCalculator,
    // 모멘텀 지표
    RsiParams,
    // 추세 지표
    SmaParams,
    StochasticParams,
    StochasticResult,
    // 구조적 피처
    StructuralFeatures,
    TrendIndicators,
    VolatilityIndicators,
};

// RouteState 계산기 re-export
pub use route_state_calculator::RouteStateCalculator;

// MarketRegime 계산기 re-export
pub use market_regime_calculator::{MarketRegimeCalculator, MarketRegimeResult};

// Trigger 계산기 re-export
pub use trigger_calculator::{TriggerCalculator, TriggerError};

// Global Scorer re-export
pub use global_scorer::{GlobalScorer, GlobalScorerError, GlobalScorerParams, GlobalScorerResult};

// Liquidity Gate re-export
pub use liquidity_gate::{LiquidityGate, LiquidityLevel};

// ML 모듈 re-exports (ml feature 필요)
#[cfg(feature = "ml")]
pub use ml::{
    // 패턴 인식
    CandlestickPattern,
    // 통합 서비스
    CandlestickPatternInfo,
    CandlestickPatternType,
    ChartPattern,
    ChartPatternInfo,
    ChartPatternType,
    // 예측
    ConfidenceLevel,
    // 피처 추출
    FeatureConfig,
    FeatureExtractor,
    FeatureSummary,
    FeatureVector,
    MlAnalysisResult,
    // 에러 타입
    MlError,
    MlResult,
    MlService,
    MlServiceConfig,
    MockPredictor,
    OnnxPredictor,
    PatternConfig,
    PatternDetectionResult,
    PatternRecognizer,
    Prediction,
    PredictionDirection,
    PredictionResult,
    PredictorConfig,
    PricePredictor,
};
