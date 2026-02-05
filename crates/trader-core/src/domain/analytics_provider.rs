//! 분석 결과 제공자 trait 및 관련 타입.
//!
//! 이 모듈은 전략에서 분석 결과를 조회하기 위한 추상화 계층을 제공합니다.
//! 실제 분석 로직(GlobalScorer, RouteStateAnalyzer 등)은 Phase 1에서 구현됩니다.

use crate::types::MarketType;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;

// Re-export RouteState from route_state module for convenience
pub use super::route_state::RouteState;
// Re-export MarketRegime, MacroEnvironment, MarketBreadth for convenience
pub use super::macro_environment::MacroEnvironment;
pub use super::market_breadth::MarketBreadth;
pub use super::market_regime::MarketRegime;

// ================================================================================================
// Error Types
// ================================================================================================

/// AnalyticsProvider 에러 타입.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnalyticsError {
    /// 데이터 조회 실패
    DataFetch(String),
    /// 계산 오류
    Calculation(String),
    /// 지원하지 않는 기능
    Unsupported(String),
    /// 기타 오류
    Other(String),
}

impl fmt::Display for AnalyticsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnalyticsError::DataFetch(msg) => write!(f, "Data fetch error: {}", msg),
            AnalyticsError::Calculation(msg) => write!(f, "Calculation error: {}", msg),
            AnalyticsError::Unsupported(msg) => write!(f, "Unsupported: {}", msg),
            AnalyticsError::Other(msg) => write!(f, "Analytics error: {}", msg),
        }
    }
}

impl StdError for AnalyticsError {}

// ================================================================================================
// Core Types
// ================================================================================================

/// Global Score 결과.
///
/// 시장 전체 또는 종목별 종합 점수를 나타냅니다.
/// 실제 계산 로직은 Phase 1에서 구현됩니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalScoreResult {
    /// 종목 티커 (종목별 점수인 경우)
    pub ticker: Option<String>,
    /// 시장 유형 (시장별 점수인 경우)
    pub market_type: Option<MarketType>,
    /// 종합 점수 (0.0 ~ 100.0)
    pub overall_score: Decimal,
    /// 컴포넌트별 점수 (예: "momentum": 75.0, "trend": 80.0)
    pub component_scores: HashMap<String, Decimal>,
    /// 추천 방향 (BUY/SELL/HOLD)
    pub recommendation: String,
    /// 신뢰도 (0.0 ~ 1.0)
    pub confidence: Decimal,
    /// 계산 시각
    pub timestamp: DateTime<Utc>,
}

/// 스크리닝 결과.
///
/// 특정 프리셋을 통과한 종목의 스크리닝 결과를 나타냅니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreeningResult {
    /// 종목 티커
    pub ticker: String,
    /// 프리셋 이름
    pub preset_name: String,
    /// 통과 여부
    pub passed: bool,
    /// 종합 점수 (0.0 ~ 100.0)
    pub overall_score: Decimal,
    /// 경로 상태
    pub route_state: RouteState,
    /// 조건별 결과 (조건명 -> 통과 여부)
    pub criteria_results: HashMap<String, bool>,
    /// 계산 시각
    pub timestamp: DateTime<Utc>,
    /// 섹터 상대강도 점수
    pub sector_rs: Option<Decimal>,
    /// 섹터 순위
    pub sector_rank: Option<i32>,
    /// 진입 트리거 점수 (0~100, 높을수록 강한 신호)
    pub trigger_score: Option<f64>,
    /// 진입 트리거 라벨 (예: "🚀스퀴즈 해제, 📊거래량 폭증")
    pub trigger_label: Option<String>,
}

/// 스크리닝 프리셋.
///
/// 스크리닝 조건 세트를 정의합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreeningPreset {
    /// 프리셋 이름
    pub name: String,
    /// 설명
    pub description: String,
    /// 시장 유형 필터
    pub market_types: Vec<MarketType>,
    /// 활성화된 조건 목록 (조건명)
    pub enabled_criteria: Vec<String>,
    /// 조건별 임계값 (조건명 -> 값)
    pub thresholds: HashMap<String, Decimal>,
    /// 최소 점수
    pub min_score: Decimal,
}

impl ScreeningPreset {
    /// 기본 프리셋 생성.
    pub fn default_preset() -> Self {
        Self {
            name: "default".to_string(),
            description: "Default screening preset".to_string(),
            market_types: vec![],
            enabled_criteria: vec![],
            thresholds: HashMap::new(),
            min_score: Decimal::ZERO,
        }
    }
}

/// 구조적 피처.
///
/// "살아있는 횡보"와 "죽은 횡보"를 구분하여 돌파 가능성을 예측합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralFeatures {
    /// 종목 티커
    pub ticker: String,
    /// Higher Low 강도 (-1.0 ~ 1.0, 양수=저점 상승)
    pub low_trend: Decimal,
    /// 매집/이탈 판별 (0 ~ 5, 2.0 이상=매집, -2.0 이하=이탈)
    pub vol_quality: Decimal,
    /// 박스권 위치 (0.0 ~ 1.0, 0=하단, 1=상단)
    pub range_pos: Decimal,
    /// MA20 이격도 (%, -20 ~ +20)
    pub dist_ma20: Decimal,
    /// 볼린저 밴드 폭 (%, 0 ~ 50)
    pub bb_width: Decimal,
    /// RSI 14일 (0 ~ 100)
    pub rsi: Decimal,
    /// 계산 시각
    pub timestamp: DateTime<Utc>,
}

// ================================================================================================
// AnalyticsProvider Trait
// ================================================================================================

/// 분석 결과 제공자.
///
/// 전략에서 분석 결과를 조회하기 위한 추상화 계층입니다.
/// 실제 구현체는 Phase 1에서 제공됩니다.
#[async_trait]
pub trait AnalyticsProvider: Send + Sync {
    /// Global Score 조회 (시장별).
    ///
    /// 특정 시장의 종합 점수를 조회합니다.
    ///
    /// # Arguments
    /// * `market_type` - 조회할 시장 유형
    ///
    /// # Returns
    /// GlobalScoreResult 리스트
    async fn fetch_global_scores(
        &self,
        market_type: MarketType,
    ) -> Result<Vec<GlobalScoreResult>, AnalyticsError>;

    /// RouteState 조회 (종목별).
    ///
    /// 특정 종목들의 경로 상태를 조회합니다.
    ///
    /// # Arguments
    /// * `tickers` - 조회할 종목 티커 목록
    ///
    /// # Returns
    /// ticker -> RouteState 매핑
    async fn fetch_route_states(
        &self,
        tickers: &[&str],
    ) -> Result<HashMap<String, RouteState>, AnalyticsError>;

    /// 스크리닝 결과 조회.
    ///
    /// 특정 프리셋으로 스크리닝한 결과를 조회합니다.
    ///
    /// # Arguments
    /// * `preset` - 스크리닝 프리셋
    ///
    /// # Returns
    /// ScreeningResult 리스트
    async fn fetch_screening(
        &self,
        preset: ScreeningPreset,
    ) -> Result<Vec<ScreeningResult>, AnalyticsError>;

    /// 구조적 피처 조회.
    ///
    /// 특정 종목들의 구조적 특징을 조회합니다.
    ///
    /// # Arguments
    /// * `tickers` - 조회할 종목 티커 목록
    ///
    /// # Returns
    /// ticker -> StructuralFeatures 매핑
    async fn fetch_features(
        &self,
        tickers: &[&str],
    ) -> Result<HashMap<String, StructuralFeatures>, AnalyticsError>;

    /// MarketRegime 조회 (종목별).
    ///
    /// 특정 종목들의 시장 레짐(추세 단계)을 조회합니다.
    ///
    /// # Arguments
    /// * `tickers` - 조회할 종목 티커 목록
    ///
    /// # Returns
    /// ticker -> MarketRegime 매핑
    async fn fetch_market_regimes(
        &self,
        tickers: &[&str],
    ) -> Result<HashMap<String, MarketRegime>, AnalyticsError>;

    /// MacroEnvironment 조회.
    ///
    /// 현재 매크로 환경(환율, 나스닥 등)을 조회합니다.
    ///
    /// # Returns
    /// 현재 MacroEnvironment
    async fn fetch_macro_environment(&self) -> Result<MacroEnvironment, AnalyticsError>;

    /// MarketBreadth 조회.
    ///
    /// 현재 시장 폭(20일선 상회 비율 등)을 조회합니다.
    ///
    /// # Returns
    /// 현재 MarketBreadth
    async fn fetch_market_breadth(&self) -> Result<MarketBreadth, AnalyticsError>;
}
