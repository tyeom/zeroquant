//! 분석 API 타입 정의.
//!
//! 이 모듈은 analytics 관련 요청/응답 타입을 정의합니다.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use trader_analytics::portfolio::{ChartPoint, MonthlyReturnCell, PerformanceSummary};

// ==================== 쿼리 파라미터 ====================

/// 기간 필터 쿼리 파라미터.
#[derive(Debug, Deserialize)]
pub struct PeriodQuery {
    /// 기간 (1w, 1m, 3m, 6m, 1y, ytd, all)
    #[serde(default = "default_period")]
    pub period: String,

    /// 시작 날짜 (ISO 8601, 선택적)
    pub start_date: Option<String>,

    /// 종료 날짜 (ISO 8601, 선택적)
    pub end_date: Option<String>,

    /// 자격증명 ID (선택적, 특정 계좌만 조회)
    pub credential_id: Option<String>,
}

pub(crate) fn default_period() -> String {
    "3m".to_string()
}

/// 차트 쿼리 파라미터.
#[derive(Debug, Deserialize)]
pub struct ChartQuery {
    /// 기간 (1w, 1m, 3m, 6m, 1y, ytd, all)
    #[serde(default = "default_period")]
    pub period: String,

    /// 롤링 윈도우 일수 (기본: 365)
    #[serde(default = "default_window")]
    pub window_days: i64,
}

pub(crate) fn default_window() -> i64 {
    365
}

// ==================== 응답 타입 ====================

/// 성과 요약 응답.
#[derive(Debug, Serialize, Deserialize)]
pub struct PerformanceResponse {
    /// 현재 자산 가치
    pub current_equity: String,

    /// 초기 자본 (기간 시작점)
    pub initial_capital: String,

    /// 총 수익/손실 금액 (기간 시작점 대비)
    pub total_pnl: String,

    /// 총 수익률 (%) - 기간 시작점 대비
    pub total_return_pct: String,

    /// CAGR (%)
    pub cagr_pct: String,

    /// 최대 낙폭 (%)
    pub max_drawdown_pct: String,

    /// 현재 낙폭 (%)
    pub current_drawdown_pct: String,

    /// 고점 자산 가치
    pub peak_equity: String,

    /// 데이터 기간 (일)
    pub period_days: i64,

    /// 기간별 수익률
    pub period_returns: Vec<PeriodReturnResponse>,

    /// 마지막 업데이트 시각
    pub last_updated: String,

    // === 포지션 기반 지표 (실제 투자 원금 대비) ===
    /// 총 투자 원금 (매입 총액)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_cost_basis: Option<String>,

    /// 포지션 손익 금액 (현재 평가액 - 매입 총액)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_pnl: Option<String>,

    /// 포지션 손익률 (%) - 실제 투자 원금 대비
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_pnl_pct: Option<String>,
}

/// 기간별 수익률 응답.
#[derive(Debug, Serialize, Deserialize)]
pub struct PeriodReturnResponse {
    /// 기간 이름 (1W, 1M, 3M, etc.)
    pub period: String,

    /// 수익률 (%)
    pub return_pct: String,
}

impl From<&PerformanceSummary> for PerformanceResponse {
    fn from(summary: &PerformanceSummary) -> Self {
        Self {
            current_equity: summary.current_equity.to_string(),
            initial_capital: summary.initial_capital.to_string(),
            total_pnl: summary.total_pnl.to_string(),
            total_return_pct: summary.total_return_pct.to_string(),
            cagr_pct: summary.cagr_pct.to_string(),
            max_drawdown_pct: summary.max_drawdown_pct.to_string(),
            current_drawdown_pct: summary.current_drawdown_pct.to_string(),
            peak_equity: summary.peak_equity.to_string(),
            period_days: summary.period_days,
            period_returns: Vec::new(),
            last_updated: summary.last_updated.to_rfc3339(),
            // 포지션 기반 지표 (샘플 데이터에서는 None)
            total_cost_basis: None,
            position_pnl: None,
            position_pnl_pct: None,
        }
    }
}

/// 자산 곡선 응답.
#[derive(Debug, Serialize, Deserialize)]
pub struct EquityCurveResponse {
    /// 차트 데이터 포인트
    pub data: Vec<ChartPointResponse>,

    /// 데이터 포인트 수
    pub count: usize,

    /// 기간
    pub period: String,

    /// 시작 시간
    pub start_time: String,

    /// 종료 시간
    pub end_time: String,
}

/// 차트 데이터 포인트 응답.
#[derive(Debug, Serialize, Deserialize)]
pub struct ChartPointResponse {
    /// 타임스탬프 (밀리초)
    pub x: i64,

    /// 값
    pub y: String,

    /// 레이블 (선택적)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl From<&ChartPoint> for ChartPointResponse {
    fn from(point: &ChartPoint) -> Self {
        Self {
            x: point.x,
            y: point.y.to_string(),
            label: point.label.clone(),
        }
    }
}

/// 차트 데이터 응답.
#[derive(Debug, Serialize, Deserialize)]
pub struct ChartResponse {
    /// 차트 이름
    pub name: String,

    /// 차트 데이터 포인트
    pub data: Vec<ChartPointResponse>,

    /// 데이터 포인트 수
    pub count: usize,

    /// 기간
    pub period: String,
}

/// 월별 수익률 응답.
#[derive(Debug, Serialize, Deserialize)]
pub struct MonthlyReturnsResponse {
    /// 월별 데이터
    pub data: Vec<MonthlyReturnCellResponse>,

    /// 총 월 수
    pub count: usize,

    /// 연도 범위
    pub year_range: (i32, i32),
}

/// 월별 수익률 셀 응답.
#[derive(Debug, Serialize, Deserialize)]
pub struct MonthlyReturnCellResponse {
    /// 연도
    pub year: i32,

    /// 월 (1-12)
    pub month: u32,

    /// 수익률 (%)
    pub return_pct: String,

    /// 색상 강도 (-1.0 ~ 1.0)
    pub intensity: f64,
}

impl From<&MonthlyReturnCell> for MonthlyReturnCellResponse {
    fn from(cell: &MonthlyReturnCell) -> Self {
        Self {
            year: cell.year,
            month: cell.month,
            return_pct: cell.return_pct.to_string(),
            intensity: cell.intensity,
        }
    }
}

// ==================== 기술적 지표 쿼리 타입 ====================

/// 지표 쿼리 파라미터.
#[derive(Debug, Deserialize)]
pub struct IndicatorQuery {
    /// 심볼 (예: 005930, AAPL)
    pub symbol: String,

    /// 기간 (1d, 1w, 1m, 3m, 6m, 1y)
    #[serde(default = "default_indicator_period")]
    pub period: String,

    /// 지표별 파라미터 (JSON 형식)
    pub params: Option<String>,
}

pub(crate) fn default_indicator_period() -> String {
    "3m".to_string()
}

/// SMA 쿼리 파라미터.
#[derive(Debug, Deserialize)]
pub struct SmaQuery {
    /// 심볼
    pub symbol: String,
    /// 기간
    #[serde(default = "default_indicator_period")]
    pub period: String,
    /// SMA 기간 (기본: 20)
    #[serde(default = "default_sma_period")]
    pub sma_period: usize,
}

pub(crate) fn default_sma_period() -> usize {
    20
}

/// EMA 쿼리 파라미터.
#[derive(Debug, Deserialize)]
pub struct EmaQuery {
    /// 심볼
    pub symbol: String,
    /// 기간
    #[serde(default = "default_indicator_period")]
    pub period: String,
    /// EMA 기간 (기본: 12)
    #[serde(default = "default_ema_period")]
    pub ema_period: usize,
}

pub(crate) fn default_ema_period() -> usize {
    12
}

/// RSI 쿼리 파라미터.
#[derive(Debug, Deserialize)]
pub struct RsiQuery {
    /// 심볼
    pub symbol: String,
    /// 기간
    #[serde(default = "default_indicator_period")]
    pub period: String,
    /// RSI 기간 (기본: 14)
    #[serde(default = "default_rsi_period")]
    pub rsi_period: usize,
}

pub(crate) fn default_rsi_period() -> usize {
    14
}

/// MACD 쿼리 파라미터.
#[derive(Debug, Deserialize)]
pub struct MacdQuery {
    /// 심볼
    pub symbol: String,
    /// 기간
    #[serde(default = "default_indicator_period")]
    pub period: String,
    /// 단기 EMA 기간 (기본: 12)
    #[serde(default = "default_macd_fast")]
    pub fast_period: usize,
    /// 장기 EMA 기간 (기본: 26)
    #[serde(default = "default_macd_slow")]
    pub slow_period: usize,
    /// 시그널 라인 기간 (기본: 9)
    #[serde(default = "default_macd_signal")]
    pub signal_period: usize,
}

pub(crate) fn default_macd_fast() -> usize {
    12
}

pub(crate) fn default_macd_slow() -> usize {
    26
}

pub(crate) fn default_macd_signal() -> usize {
    9
}

/// 볼린저 밴드 쿼리 파라미터.
#[derive(Debug, Deserialize)]
pub struct BollingerQuery {
    /// 심볼
    pub symbol: String,
    /// 기간
    #[serde(default = "default_indicator_period")]
    pub period: String,
    /// 이동평균 기간 (기본: 20)
    #[serde(default = "default_bollinger_period")]
    pub bb_period: usize,
    /// 표준편차 배수 (기본: 2.0)
    #[serde(default = "default_bollinger_std")]
    pub std_dev: f64,
}

pub(crate) fn default_bollinger_period() -> usize {
    20
}

pub(crate) fn default_bollinger_std() -> f64 {
    2.0
}

/// 스토캐스틱 쿼리 파라미터.
#[derive(Debug, Deserialize)]
pub struct StochasticQuery {
    /// 심볼
    pub symbol: String,
    /// 기간
    #[serde(default = "default_indicator_period")]
    pub period: String,
    /// %K 기간 (기본: 14)
    #[serde(default = "default_stochastic_k")]
    pub k_period: usize,
    /// %D 기간 (기본: 3)
    #[serde(default = "default_stochastic_d")]
    pub d_period: usize,
}

pub(crate) fn default_stochastic_k() -> usize {
    14
}

pub(crate) fn default_stochastic_d() -> usize {
    3
}

/// ATR 쿼리 파라미터.
#[derive(Debug, Deserialize)]
pub struct AtrQuery {
    /// 심볼
    pub symbol: String,
    /// 기간
    #[serde(default = "default_indicator_period")]
    pub period: String,
    /// ATR 기간 (기본: 14)
    #[serde(default = "default_atr_period")]
    pub atr_period: usize,
}

pub(crate) fn default_atr_period() -> usize {
    14
}

// ==================== 기술적 지표 응답 타입 ====================

/// 다중 지표 계산 요청.
#[derive(Debug, Deserialize)]
pub struct CalculateIndicatorsRequest {
    /// 심볼
    pub symbol: String,
    /// 기간
    #[serde(default = "default_indicator_period")]
    pub period: String,
    /// 계산할 지표 목록
    pub indicators: Vec<IndicatorConfig>,
}

/// 지표 설정.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IndicatorConfig {
    /// 지표 타입
    #[serde(rename = "type")]
    pub indicator_type: String,
    /// 지표 파라미터
    pub params: serde_json::Value,
    /// 차트에 표시할 색상 (선택적)
    pub color: Option<String>,
    /// 차트에 표시할 이름 (선택적)
    pub name: Option<String>,
}

/// 사용 가능한 지표 목록 응답.
#[derive(Debug, Serialize)]
pub struct AvailableIndicatorsResponse {
    /// 지표 목록
    pub indicators: Vec<IndicatorInfo>,
}

/// 지표 정보.
#[derive(Debug, Serialize)]
pub struct IndicatorInfo {
    /// 지표 ID
    pub id: String,
    /// 지표 이름
    pub name: String,
    /// 지표 설명
    pub description: String,
    /// 지표 카테고리
    pub category: String,
    /// 기본 파라미터
    pub default_params: serde_json::Value,
    /// 오버레이 여부 (가격 차트 위에 표시)
    pub overlay: bool,
}

/// 단일 지표 데이터 응답.
#[derive(Debug, Serialize)]
pub struct IndicatorDataResponse {
    /// 지표 ID
    pub indicator: String,
    /// 지표 이름
    pub name: String,
    /// 심볼
    pub symbol: String,
    /// 사용된 파라미터
    pub params: serde_json::Value,
    /// 데이터 시리즈
    pub series: Vec<IndicatorSeries>,
}

/// 지표 시리즈 데이터.
#[derive(Debug, Serialize)]
pub struct IndicatorSeries {
    /// 시리즈 이름 (예: "macd", "signal", "histogram")
    pub name: String,
    /// 데이터 포인트
    pub data: Vec<IndicatorPoint>,
    /// 색상 (선택적)
    pub color: Option<String>,
    /// 시리즈 타입 (line, bar, area)
    pub series_type: String,
}

/// 지표 데이터 포인트.
#[derive(Debug, Serialize)]
pub struct IndicatorPoint {
    /// 타임스탬프 (밀리초)
    pub x: i64,
    /// 값
    pub y: Option<String>,
}

/// 다중 지표 계산 응답.
#[derive(Debug, Serialize)]
pub struct CalculateIndicatorsResponse {
    /// 심볼
    pub symbol: String,
    /// 기간
    pub period: String,
    /// 지표별 결과
    pub results: Vec<IndicatorDataResponse>,
}

// ==================== 동기화 타입 ====================

/// 동기화 요청.
#[derive(Debug, Deserialize)]
pub struct SyncEquityCurveRequest {
    /// 자격증명 ID
    pub credential_id: String,
    /// 조회 시작일 (YYYYMMDD)
    pub start_date: String,
    /// 조회 종료일 (YYYYMMDD)
    pub end_date: String,
    /// 종가 기반 계산 사용 여부 (true: 정확한 계산, false: 현금 흐름 기반)
    #[serde(default)]
    pub use_market_prices: bool,
    /// 초기 자본금 (종가 기반 계산 시 필수)
    pub initial_capital: Option<Decimal>,
}

/// 동기화 응답.
#[derive(Debug, Serialize)]
pub struct SyncEquityCurveResponse {
    /// 성공 여부
    pub success: bool,
    /// 동기화된 스냅샷 개수
    pub synced_count: usize,
    /// 처리된 체결 내역 개수
    pub execution_count: usize,
    /// 시작 날짜
    pub start_date: String,
    /// 종료 날짜
    pub end_date: String,
    /// 메시지
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use trader_analytics::portfolio::ChartPoint;

    #[test]
    fn test_period_return_response() {
        let resp = PeriodReturnResponse {
            period: "1M".to_string(),
            return_pct: "5.25".to_string(),
        };
        assert_eq!(resp.period, "1M");
    }

    #[test]
    fn test_chart_point_response() {
        let point = ChartPoint::new(Utc::now(), dec!(100));
        let resp = ChartPointResponse::from(&point);
        assert_eq!(resp.y, "100");
    }
}
