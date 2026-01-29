//! 포트폴리오 analytics endpoint.
//!
//! 포트폴리오 분석 데이터를 제공하는 REST API입니다.
//!
//! # 엔드포인트
//!
//! ## 포트폴리오 분석
//! - `GET /api/v1/analytics/performance` - 성과 요약
//! - `GET /api/v1/analytics/equity-curve` - 자산 곡선 데이터
//! - `GET /api/v1/analytics/charts/cagr` - CAGR 추이 차트
//! - `GET /api/v1/analytics/charts/mdd` - MDD 추이 차트
//! - `GET /api/v1/analytics/monthly-returns` - 월별 수익률
//!
//! ## 기술적 지표
//! - `GET /api/v1/analytics/indicators` - 사용 가능한 지표 목록
//! - `GET /api/v1/analytics/indicators/sma` - 단순 이동평균
//! - `GET /api/v1/analytics/indicators/ema` - 지수 이동평균
//! - `GET /api/v1/analytics/indicators/rsi` - RSI
//! - `GET /api/v1/analytics/indicators/macd` - MACD
//! - `GET /api/v1/analytics/indicators/bollinger` - 볼린저 밴드
//! - `GET /api/v1/analytics/indicators/stochastic` - 스토캐스틱
//! - `GET /api/v1/analytics/indicators/atr` - ATR
//! - `POST /api/v1/analytics/indicators/calculate` - 다중 지표 계산

use axum::{
    extract::{Query, State},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Datelike, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, warn};

use crate::routes::equity_history;
use crate::state::AppState;
use trader_analytics::portfolio::{
    ChartPoint, EquityCurve, EquityCurveBuilder, MonthlyReturnCell,
    PerformanceSummary, PeriodPerformance, PortfolioCharts,
};
use trader_analytics::{
    IndicatorEngine, SmaParams, EmaParams, MacdParams, RsiParams,
    StochasticParams, BollingerBandsParams, AtrParams,
};

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
}

fn default_period() -> String {
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

fn default_window() -> i64 {
    365
}

// ==================== 응답 타입 ====================

/// 성과 요약 응답.
#[derive(Debug, Serialize, Deserialize)]
pub struct PerformanceResponse {
    /// 현재 자산 가치
    pub current_equity: String,

    /// 초기 자본
    pub initial_capital: String,

    /// 총 수익/손실 금액
    pub total_pnl: String,

    /// 총 수익률 (%)
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

// ==================== 기술적 지표 타입 ====================

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

fn default_indicator_period() -> String {
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

fn default_sma_period() -> usize {
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

fn default_ema_period() -> usize {
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

fn default_rsi_period() -> usize {
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

fn default_macd_fast() -> usize {
    12
}

fn default_macd_slow() -> usize {
    26
}

fn default_macd_signal() -> usize {
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

fn default_bollinger_period() -> usize {
    20
}

fn default_bollinger_std() -> f64 {
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

fn default_stochastic_k() -> usize {
    14
}

fn default_stochastic_d() -> usize {
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

fn default_atr_period() -> usize {
    14
}

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

// ==================== Analytics 매니저 ====================

/// 분석 데이터 매니저.
///
/// 포트폴리오 자산 곡선을 관리하고 분석 데이터를 제공합니다.
pub struct AnalyticsManager {
    /// 자산 곡선 빌더
    builder: EquityCurveBuilder,

    /// 빌드된 자산 곡선 캐시
    curve_cache: Option<EquityCurve>,

    /// 캐시 유효 시간
    cache_valid_until: Option<DateTime<Utc>>,
}

impl AnalyticsManager {
    /// 새 매니저 생성.
    pub fn new(initial_capital: Decimal) -> Self {
        Self {
            builder: EquityCurveBuilder::new(initial_capital),
            curve_cache: None,
            cache_valid_until: None,
        }
    }

    /// 거래 결과 추가.
    pub fn add_trade_result(&mut self, timestamp: DateTime<Utc>, equity: Decimal) {
        self.builder.add_trade_result(timestamp, equity);
        self.invalidate_cache();
    }

    /// 캐시 무효화.
    fn invalidate_cache(&mut self) {
        self.curve_cache = None;
        self.cache_valid_until = None;
    }

    /// 자산 곡선 가져오기 (캐시 사용).
    pub fn get_curve(&mut self) -> &EquityCurve {
        let now = Utc::now();

        // 캐시가 유효하면 반환
        if let Some(valid_until) = self.cache_valid_until {
            if now < valid_until && self.curve_cache.is_some() {
                return self.curve_cache.as_ref().unwrap();
            }
        }

        // 캐시 재생성 (builder를 clone하여 소유권 문제 해결)
        self.curve_cache = Some(self.builder.clone().build());
        self.cache_valid_until = Some(now + Duration::minutes(5));

        self.curve_cache.as_ref().unwrap()
    }

    /// 성과 요약 가져오기.
    pub fn get_performance_summary(&mut self) -> PerformanceSummary {
        let curve = self.get_curve();
        PerformanceSummary::from_equity_curve(curve)
    }

    /// 기간별 성과 가져오기.
    pub fn get_period_performance(&mut self) -> Vec<PeriodPerformance> {
        let curve = self.get_curve();
        PeriodPerformance::calculate_periods(curve)
    }

    /// 차트 데이터 가져오기.
    pub fn get_charts(&mut self, window_days: i64) -> PortfolioCharts {
        let curve = self.get_curve();
        PortfolioCharts::from_equity_curve_with_params(curve, window_days, 0.05)
    }

    /// 자산 곡선 데이터 가져오기.
    pub fn get_equity_curve_data(&mut self) -> Vec<ChartPoint> {
        let curve = self.get_curve();
        curve
            .equity_series()
            .into_iter()
            .map(|(ts, equity)| ChartPoint::new(ts, equity))
            .collect()
    }

    /// 샘플 데이터 로드 (테스트용).
    pub fn load_sample_data(&mut self) {
        let base_time = Utc::now() - Duration::days(365);
        let mut equity = dec!(10_000_000);

        for i in 0..365 {
            // 변동성 있는 상승 곡선 시뮬레이션
            let daily_return = if i % 7 == 0 {
                dec!(-0.02) // 주간 조정
            } else if i % 3 == 0 {
                dec!(0.015) // 소폭 상승
            } else {
                dec!(0.003) // 일반 상승
            };

            equity = equity * (dec!(1.0) + daily_return);
            self.add_trade_result(base_time + Duration::days(i), equity);
        }
    }
}

impl Default for AnalyticsManager {
    fn default() -> Self {
        Self::new(dec!(10_000_000))
    }
}

// ==================== 기간 파싱 유틸리티 ====================

/// 기간 문자열을 Duration으로 변환.
fn parse_period_duration(period: &str) -> Duration {
    match period.to_lowercase().as_str() {
        "1w" => Duration::days(7),
        "1m" => Duration::days(30),
        "3m" => Duration::days(90),
        "6m" => Duration::days(180),
        "1y" | "12m" => Duration::days(365),
        "ytd" => {
            let now = Utc::now();
            let start_of_year: DateTime<Utc> = DateTime::from_naive_utc_and_offset(
                chrono::NaiveDate::from_ymd_opt(now.year(), 1, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap(),
                Utc,
            );
            now.signed_duration_since(start_of_year)
        }
        "all" | _ => Duration::days(3650), // 10년
    }
}

// ==================== 핸들러 ====================

/// 성과 요약 조회.
///
/// GET /api/v1/analytics/performance
pub async fn get_performance(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PeriodQuery>,
) -> impl IntoResponse {
    // DB에서 실제 데이터 조회 시도
    if let Some(db_pool) = &state.db_pool {
        let duration = parse_period_duration(&query.period);
        let start_time = Utc::now() - duration;
        let end_time = Utc::now();

        // 통합 자산 곡선 데이터 조회
        match equity_history::get_aggregated_equity_curve(db_pool, start_time, end_time).await {
            Ok(data) if !data.is_empty() => {
                debug!("DB에서 {} 개의 자산 곡선 포인트 로드됨", data.len());

                // 초기 자본 조회
                let initial_capital = equity_history::get_initial_capital(db_pool, None)
                    .await
                    .unwrap_or(dec!(10_000_000));

                // 최고점 조회
                let peak_equity = equity_history::get_peak_equity(db_pool, None)
                    .await
                    .unwrap_or(initial_capital);

                // 현재 자산 (마지막 데이터 포인트)
                let current_equity = data.last().map(|p| p.equity).unwrap_or(initial_capital);

                // 총 수익/손실
                let total_pnl = current_equity - initial_capital;
                let total_return_pct = if initial_capital > Decimal::ZERO {
                    (total_pnl / initial_capital) * dec!(100)
                } else {
                    Decimal::ZERO
                };

                // MDD 계산
                let max_drawdown_pct = data.iter()
                    .map(|p| p.drawdown_pct)
                    .max()
                    .unwrap_or(Decimal::ZERO);

                // 현재 Drawdown
                let current_drawdown_pct = if peak_equity > Decimal::ZERO {
                    ((peak_equity - current_equity) / peak_equity) * dec!(100)
                } else {
                    Decimal::ZERO
                };

                // CAGR 계산 (연환산 수익률)
                let days = data.len() as i64;
                let years = Decimal::from(days) / dec!(365);
                let cagr_pct = if years > Decimal::ZERO && initial_capital > Decimal::ZERO {
                    let growth_factor = current_equity / initial_capital;
                    // (growth_factor^(1/years) - 1) * 100
                    // 간단한 근사: ln(growth_factor) / years * 100
                    let ln_growth = (growth_factor.to_string().parse::<f64>().unwrap_or(1.0)).ln();
                    let cagr = (ln_growth / years.to_string().parse::<f64>().unwrap_or(1.0)).exp() - 1.0;
                    Decimal::from_f64_retain(cagr * 100.0).unwrap_or(Decimal::ZERO)
                } else {
                    Decimal::ZERO
                };

                return Json(PerformanceResponse {
                    current_equity: current_equity.to_string(),
                    initial_capital: initial_capital.to_string(),
                    total_pnl: total_pnl.to_string(),
                    total_return_pct: total_return_pct.to_string(),
                    cagr_pct: cagr_pct.to_string(),
                    max_drawdown_pct: max_drawdown_pct.to_string(),
                    current_drawdown_pct: current_drawdown_pct.to_string(),
                    peak_equity: peak_equity.to_string(),
                    period_days: days,
                    period_returns: Vec::new(), // TODO: 기간별 수익률 계산
                    last_updated: Utc::now().to_rfc3339(),
                });
            }
            Ok(_) => {
                debug!("DB에 자산 곡선 데이터 없음, 샘플 데이터 사용");
            }
            Err(e) => {
                warn!("자산 곡선 데이터 조회 실패: {}", e);
            }
        }
    }

    // Fallback: 샘플 데이터로 응답 생성
    let mut manager = AnalyticsManager::default();
    manager.load_sample_data();

    let summary = manager.get_performance_summary();
    let periods = manager.get_period_performance();

    let mut response = PerformanceResponse::from(&summary);
    response.period_returns = periods
        .iter()
        .map(|p| PeriodReturnResponse {
            period: p.period.clone(),
            return_pct: p.return_pct.to_string(),
        })
        .collect();

    Json(response)
}

/// 자산 곡선 데이터 조회.
///
/// GET /api/v1/analytics/equity-curve
pub async fn get_equity_curve(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PeriodQuery>,
) -> impl IntoResponse {
    let duration = parse_period_duration(&query.period);
    let start_time = Utc::now() - duration;
    let end_time = Utc::now();

    // DB에서 실제 데이터 조회 시도
    if let Some(db_pool) = &state.db_pool {
        match equity_history::get_aggregated_equity_curve(db_pool, start_time, end_time).await {
            Ok(data) if !data.is_empty() => {
                debug!("DB에서 {} 개의 자산 곡선 포인트 로드됨", data.len());

                let filtered: Vec<ChartPointResponse> = data
                    .iter()
                    .map(|p| ChartPointResponse {
                        x: p.timestamp.timestamp_millis(),
                        y: p.equity.to_string(),
                        label: None,
                    })
                    .collect();

                let (start_str, end_str) = if filtered.is_empty() {
                    (Utc::now().to_rfc3339(), Utc::now().to_rfc3339())
                } else {
                    let start = DateTime::from_timestamp_millis(filtered.first().unwrap().x)
                        .unwrap_or(Utc::now());
                    let end = DateTime::from_timestamp_millis(filtered.last().unwrap().x)
                        .unwrap_or(Utc::now());
                    (start.to_rfc3339(), end.to_rfc3339())
                };

                return Json(EquityCurveResponse {
                    count: filtered.len(),
                    data: filtered,
                    period: query.period,
                    start_time: start_str,
                    end_time: end_str,
                });
            }
            Ok(_) => {
                debug!("DB에 자산 곡선 데이터 없음, 샘플 데이터 사용");
            }
            Err(e) => {
                warn!("자산 곡선 데이터 조회 실패: {}", e);
            }
        }
    }

    // Fallback: 샘플 데이터
    let mut manager = AnalyticsManager::default();
    manager.load_sample_data();

    let data = manager.get_equity_curve_data();
    let cutoff = Utc::now() - duration;

    // 기간 필터링
    let filtered: Vec<ChartPointResponse> = data
        .iter()
        .filter(|p| {
            let ts = DateTime::from_timestamp_millis(p.x).unwrap_or(Utc::now());
            ts >= cutoff
        })
        .map(ChartPointResponse::from)
        .collect();

    let (start_str, end_str) = if filtered.is_empty() {
        (Utc::now().to_rfc3339(), Utc::now().to_rfc3339())
    } else {
        let start = DateTime::from_timestamp_millis(filtered.first().unwrap().x)
            .unwrap_or(Utc::now());
        let end = DateTime::from_timestamp_millis(filtered.last().unwrap().x)
            .unwrap_or(Utc::now());
        (start.to_rfc3339(), end.to_rfc3339())
    };

    Json(EquityCurveResponse {
        count: filtered.len(),
        data: filtered,
        period: query.period,
        start_time: start_str,
        end_time: end_str,
    })
}

/// CAGR 차트 데이터 조회.
///
/// GET /api/v1/analytics/charts/cagr
pub async fn get_cagr_chart(
    State(_state): State<Arc<AppState>>,
    Query(query): Query<ChartQuery>,
) -> impl IntoResponse {
    let mut manager = AnalyticsManager::default();
    manager.load_sample_data();

    let charts = manager.get_charts(query.window_days);
    let duration = parse_period_duration(&query.period);
    let cutoff = Utc::now() - duration;

    let filtered: Vec<ChartPointResponse> = charts
        .rolling_cagr
        .iter()
        .filter(|p| {
            let ts = DateTime::from_timestamp_millis(p.x).unwrap_or(Utc::now());
            ts >= cutoff
        })
        .map(ChartPointResponse::from)
        .collect();

    Json(ChartResponse {
        name: "Rolling CAGR".to_string(),
        count: filtered.len(),
        data: filtered,
        period: query.period,
    })
}

/// MDD 차트 데이터 조회.
///
/// GET /api/v1/analytics/charts/mdd
pub async fn get_mdd_chart(
    State(_state): State<Arc<AppState>>,
    Query(query): Query<ChartQuery>,
) -> impl IntoResponse {
    let mut manager = AnalyticsManager::default();
    manager.load_sample_data();

    let charts = manager.get_charts(query.window_days);
    let duration = parse_period_duration(&query.period);
    let cutoff = Utc::now() - duration;

    let filtered: Vec<ChartPointResponse> = charts
        .rolling_mdd
        .iter()
        .filter(|p| {
            let ts = DateTime::from_timestamp_millis(p.x).unwrap_or(Utc::now());
            ts >= cutoff
        })
        .map(ChartPointResponse::from)
        .collect();

    Json(ChartResponse {
        name: "Rolling MDD".to_string(),
        count: filtered.len(),
        data: filtered,
        period: query.period,
    })
}

/// Drawdown 차트 데이터 조회.
///
/// GET /api/v1/analytics/charts/drawdown
pub async fn get_drawdown_chart(
    State(_state): State<Arc<AppState>>,
    Query(query): Query<ChartQuery>,
) -> impl IntoResponse {
    let mut manager = AnalyticsManager::default();
    manager.load_sample_data();

    let charts = manager.get_charts(query.window_days);
    let duration = parse_period_duration(&query.period);
    let cutoff = Utc::now() - duration;

    let filtered: Vec<ChartPointResponse> = charts
        .drawdown_curve
        .iter()
        .filter(|p| {
            let ts = DateTime::from_timestamp_millis(p.x).unwrap_or(Utc::now());
            ts >= cutoff
        })
        .map(ChartPointResponse::from)
        .collect();

    Json(ChartResponse {
        name: "Drawdown".to_string(),
        count: filtered.len(),
        data: filtered,
        period: query.period,
    })
}

/// 월별 수익률 히트맵 데이터 조회.
///
/// GET /api/v1/analytics/monthly-returns
pub async fn get_monthly_returns(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // DB에서 실제 데이터 조회 시도
    if let Some(db_pool) = &state.db_pool {
        match equity_history::get_monthly_returns(db_pool, None, 3).await {
            Ok(monthly_data) if !monthly_data.is_empty() => {
                debug!("DB에서 {} 개의 월별 수익률 데이터 로드됨", monthly_data.len());

                // 강도(intensity) 계산을 위한 최대/최소값 찾기
                let max_return = monthly_data.iter()
                    .map(|m| m.return_pct.abs())
                    .max()
                    .unwrap_or(dec!(10));

                let data: Vec<MonthlyReturnCellResponse> = monthly_data
                    .iter()
                    .map(|m| {
                        let intensity = if max_return > Decimal::ZERO {
                            (m.return_pct / max_return).to_f64().unwrap_or(0.0)
                        } else {
                            0.0
                        };

                        MonthlyReturnCellResponse {
                            year: m.year,
                            month: m.month,
                            return_pct: m.return_pct.to_string(),
                            intensity,
                        }
                    })
                    .collect();

                let (min_year, max_year) = if data.is_empty() {
                    (Utc::now().year(), Utc::now().year())
                } else {
                    let min = data.iter().map(|c| c.year).min().unwrap();
                    let max = data.iter().map(|c| c.year).max().unwrap();
                    (min, max)
                };

                return Json(MonthlyReturnsResponse {
                    count: data.len(),
                    data,
                    year_range: (min_year, max_year),
                });
            }
            Ok(_) => {
                debug!("DB에 월별 수익률 데이터 없음, 샘플 데이터 사용");
            }
            Err(e) => {
                warn!("월별 수익률 데이터 조회 실패: {}", e);
            }
        }
    }

    // Fallback: 샘플 데이터
    let mut manager = AnalyticsManager::default();
    manager.load_sample_data();

    let charts = manager.get_charts(365);

    let data: Vec<MonthlyReturnCellResponse> = charts
        .monthly_returns
        .iter()
        .map(MonthlyReturnCellResponse::from)
        .collect();

    let (min_year, max_year) = if data.is_empty() {
        (Utc::now().year(), Utc::now().year())
    } else {
        let min = data.iter().map(|c| c.year).min().unwrap();
        let max = data.iter().map(|c| c.year).max().unwrap();
        (min, max)
    };

    Json(MonthlyReturnsResponse {
        count: data.len(),
        data,
        year_range: (min_year, max_year),
    })
}

// ==================== 자산 곡선 동기화 핸들러 ====================

/// 동기화 요청.
#[derive(Debug, Deserialize)]
pub struct SyncEquityCurveRequest {
    /// 자격증명 ID
    pub credential_id: String,
    /// 조회 시작일 (YYYYMMDD)
    pub start_date: String,
    /// 조회 종료일 (YYYYMMDD)
    pub end_date: String,
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

/// 거래소 체결 내역으로 자산 곡선 동기화.
///
/// POST /api/v1/analytics/sync-equity
///
/// KIS API에서 체결 내역을 가져와 자산 곡선 데이터를 재구성합니다.
pub async fn sync_equity_curve(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SyncEquityCurveRequest>,
) -> impl IntoResponse {
    use crate::routes::equity_history::{ExecutionForSync, sync_equity_from_executions};
    use uuid::Uuid;

    // 1. credential_id 파싱
    let credential_id = match Uuid::parse_str(&request.credential_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(SyncEquityCurveResponse {
                    success: false,
                    synced_count: 0,
                    execution_count: 0,
                    start_date: request.start_date,
                    end_date: request.end_date,
                    message: "Invalid credential_id format".to_string(),
                }),
            );
        }
    };

    // 2. KIS 클라이언트 가져오기
    let kis_clients = state.kis_clients_cache.read().await;
    let client_pair = match kis_clients.get(&credential_id) {
        Some(pair) => pair.clone(),
        None => {
            return (
                axum::http::StatusCode::NOT_FOUND,
                Json(SyncEquityCurveResponse {
                    success: false,
                    synced_count: 0,
                    execution_count: 0,
                    start_date: request.start_date.clone(),
                    end_date: request.end_date.clone(),
                    message: "KIS client not found. Please refresh portfolio first.".to_string(),
                }),
            );
        }
    };
    drop(kis_clients);

    // 3. 날짜 형식 변환 (ISO 8601 -> YYYYMMDD)
    // KIS API는 "20240101" 형식을 요구함, 입력은 "2024-01-01" 형식
    let start_date_yyyymmdd = request.start_date.replace('-', "");
    let end_date_yyyymmdd = request.end_date.replace('-', "");
    debug!("Date format converted: {} -> {}, {} -> {}",
        request.start_date, start_date_yyyymmdd,
        request.end_date, end_date_yyyymmdd);

    // 4. 체결 내역 조회 (연속 조회로 전체 가져오기)
    // KIS API는 초당 요청 수를 제한하므로 Rate Limiting 필요
    let mut all_executions: Vec<ExecutionForSync> = Vec::new();
    let mut ctx_fk = String::new();
    let mut ctx_nk = String::new();
    let mut prev_ctx_nk = String::new();  // Python 로직: 이전 키와 비교하여 무한 루프 방지
    let mut page_count = 0;
    const MAX_PAGES: usize = 50; // 무한 루프 방지

    // KIS API Rate Limit (2024.04.01 변경):
    // - 실계좌: 200ms (초당 5건)
    // - 모의계좌: 510ms (초당 2건) = 200ms + 310ms
    // Python 모듈의 검증된 값 사용
    use trader_exchange::connector::kis::KisEnvironment;
    let api_call_delay_ms: u64 = match client_pair.kr.oauth().config().environment {
        KisEnvironment::Real => 200,
        KisEnvironment::Paper => 520,  // 510ms + 안전 마진 10ms
    };
    debug!("Using API delay: {}ms (environment: {:?})",
        api_call_delay_ms, client_pair.kr.oauth().config().environment);

    loop {
        // Rate Limiting: 첫 번째 호출 이후에는 지연 적용
        if page_count > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(api_call_delay_ms)).await;
        }
        page_count += 1;

        // 무한 루프 방지
        if page_count > MAX_PAGES {
            warn!("Max pagination limit reached ({} pages), stopping", MAX_PAGES);
            break;
        }

        debug!("Fetching order history page {} (ctx_fk={}, ctx_nk={})",
            page_count, ctx_fk.len(), ctx_nk.len());

        let history = match client_pair.kr.get_order_history(
            &start_date_yyyymmdd,
            &end_date_yyyymmdd,
            "00",  // 전체 (매수+매도)
            &ctx_fk,
            &ctx_nk,
        ).await {
            Ok(h) => h,
            Err(e) => {
                // Rate Limit 에러인 경우 잠시 대기 후 재시도
                let error_msg = e.to_string();
                if error_msg.contains("초당") || error_msg.contains("건수") || error_msg.contains("exceeded") {
                    warn!("Rate limit hit, waiting 2 seconds before retry...");
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                    // 재시도
                    match client_pair.kr.get_order_history(
                        &start_date_yyyymmdd,
                        &end_date_yyyymmdd,
                        "00",
                        &ctx_fk,
                        &ctx_nk,
                    ).await {
                        Ok(h) => h,
                        Err(e2) => {
                            return (
                                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                                Json(SyncEquityCurveResponse {
                                    success: false,
                                    synced_count: 0,
                                    execution_count: all_executions.len(),
                                    start_date: request.start_date,
                                    end_date: request.end_date,
                                    message: format!("Failed to fetch order history after retry: {}", e2),
                                }),
                            );
                        }
                    }
                } else {
                    return (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        Json(SyncEquityCurveResponse {
                            success: false,
                            synced_count: 0,
                            execution_count: 0,
                            start_date: request.start_date,
                            end_date: request.end_date,
                            message: format!("Failed to fetch order history: {}", e),
                        }),
                    );
                }
            }
        };

        debug!("Received {} executions in page {}", history.executions.len(), page_count);

        // 체결 내역 변환
        for exec in history.executions {
            // 체결 시간 파싱 (order_date: YYYYMMDD, order_time: HHMMSS)
            let exec_date = format!("{}{}", exec.order_date, exec.order_time);
            let execution_time = chrono::NaiveDateTime::parse_from_str(&exec_date, "%Y%m%d%H%M%S")
                .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
                .unwrap_or_else(|_| Utc::now());

            let amount = exec.filled_amount;  // 총 체결 금액
            let is_buy = exec.side_code == "02";  // 02: 매수

            all_executions.push(ExecutionForSync {
                execution_time,
                amount,
                is_buy,
                symbol: exec.stock_code.clone(),
            });
        }

        // 연속 조회 확인 (Python 로직 참조)
        // 1. 데이터가 더 없으면 종료
        if !history.has_more {
            debug!("No more pages (has_more=false), total {} executions collected", all_executions.len());
            break;
        }

        // 2. 이전 키와 현재 키가 같으면 종료 (무한 루프 방지)
        if prev_ctx_nk == history.ctx_area_nk100 && !prev_ctx_nk.is_empty() {
            debug!("Same ctx_nk as previous, stopping (infinite loop prevention)");
            break;
        }

        // 3. NK 키가 비어있으면 종료
        if history.ctx_area_nk100.is_empty() {
            debug!("ctx_nk is empty, no more pages");
            break;
        }

        prev_ctx_nk = ctx_nk.clone();
        ctx_fk = history.ctx_area_fk100;
        ctx_nk = history.ctx_area_nk100;
    }

    let execution_count = all_executions.len();

    // 4. 현재 잔고 조회 (Rate Limit 방지를 위한 지연)
    tokio::time::sleep(std::time::Duration::from_millis(api_call_delay_ms)).await;

    let current_equity = match client_pair.kr.get_balance().await {
        Ok(balance) => {
            // summary에서 총 평가금액 추출
            balance.summary
                .map(|s| s.total_eval_amount)
                .unwrap_or_else(|| {
                    // summary가 없으면 holdings의 평가금액 합산 + 예수금
                    balance.holdings.iter()
                        .map(|h| h.eval_amount)
                        .sum()
                })
        },
        Err(e) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(SyncEquityCurveResponse {
                    success: false,
                    synced_count: 0,
                    execution_count,
                    start_date: request.start_date,
                    end_date: request.end_date,
                    message: format!("Failed to fetch balance: {}", e),
                }),
            );
        }
    };

    // 5. DB에 자산 곡선 저장
    if let Some(pool) = &state.db_pool {
        match sync_equity_from_executions(
            pool,
            credential_id,
            all_executions,
            current_equity,
            "KRW",
            "KR",
            Some("real"),
        ).await {
            Ok(synced_count) => {
                return (
                    axum::http::StatusCode::OK,
                    Json(SyncEquityCurveResponse {
                        success: true,
                        synced_count,
                        execution_count,
                        start_date: request.start_date,
                        end_date: request.end_date,
                        message: format!(
                            "Successfully synced {} equity points from {} executions",
                            synced_count, execution_count
                        ),
                    }),
                );
            }
            Err(e) => {
                return (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(SyncEquityCurveResponse {
                        success: false,
                        synced_count: 0,
                        execution_count,
                        start_date: request.start_date,
                        end_date: request.end_date,
                        message: format!("Failed to save equity curve: {}", e),
                    }),
                );
            }
        }
    }

    (
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        Json(SyncEquityCurveResponse {
            success: false,
            synced_count: 0,
            execution_count,
            start_date: request.start_date,
            end_date: request.end_date,
            message: "Database not available".to_string(),
        }),
    )
}

// ==================== 기술적 지표 핸들러 ====================

/// 사용 가능한 지표 목록 조회.
///
/// GET /api/v1/analytics/indicators
pub async fn get_available_indicators() -> impl IntoResponse {
    let indicators = vec![
        IndicatorInfo {
            id: "sma".to_string(),
            name: "단순 이동평균 (SMA)".to_string(),
            description: "지정된 기간 동안의 종가 평균을 계산합니다.".to_string(),
            category: "추세".to_string(),
            default_params: serde_json::json!({ "period": 20 }),
            overlay: true,
        },
        IndicatorInfo {
            id: "ema".to_string(),
            name: "지수 이동평균 (EMA)".to_string(),
            description: "최근 가격에 더 큰 가중치를 부여하는 이동평균입니다.".to_string(),
            category: "추세".to_string(),
            default_params: serde_json::json!({ "period": 12 }),
            overlay: true,
        },
        IndicatorInfo {
            id: "rsi".to_string(),
            name: "상대강도지수 (RSI)".to_string(),
            description: "과매수/과매도 상태를 측정합니다. 70 이상: 과매수, 30 이하: 과매도.".to_string(),
            category: "모멘텀".to_string(),
            default_params: serde_json::json!({ "period": 14 }),
            overlay: false,
        },
        IndicatorInfo {
            id: "macd".to_string(),
            name: "MACD".to_string(),
            description: "두 EMA의 차이로 추세의 강도와 방향을 분석합니다.".to_string(),
            category: "추세".to_string(),
            default_params: serde_json::json!({
                "fast_period": 12,
                "slow_period": 26,
                "signal_period": 9
            }),
            overlay: false,
        },
        IndicatorInfo {
            id: "bollinger".to_string(),
            name: "볼린저 밴드".to_string(),
            description: "이동평균을 중심으로 표준편차 밴드를 그려 변동성을 시각화합니다.".to_string(),
            category: "변동성".to_string(),
            default_params: serde_json::json!({ "period": 20, "std_dev": 2.0 }),
            overlay: true,
        },
        IndicatorInfo {
            id: "stochastic".to_string(),
            name: "스토캐스틱".to_string(),
            description: "현재 가격이 일정 기간 가격 범위 내에서 어디에 위치하는지 측정합니다.".to_string(),
            category: "모멘텀".to_string(),
            default_params: serde_json::json!({ "k_period": 14, "d_period": 3 }),
            overlay: false,
        },
        IndicatorInfo {
            id: "atr".to_string(),
            name: "평균 실제 범위 (ATR)".to_string(),
            description: "가격 변동성을 측정합니다. 값이 클수록 변동성이 높습니다.".to_string(),
            category: "변동성".to_string(),
            default_params: serde_json::json!({ "period": 14 }),
            overlay: false,
        },
    ];

    Json(AvailableIndicatorsResponse { indicators })
}

/// 샘플 OHLCV 데이터 생성 (테스트용).
fn generate_sample_ohlcv(days: i64) -> (Vec<i64>, Vec<Decimal>, Vec<Decimal>, Vec<Decimal>, Vec<Decimal>) {
    let base_time = Utc::now() - Duration::days(days);
    let mut timestamps = Vec::with_capacity(days as usize);
    let mut opens = Vec::with_capacity(days as usize);
    let mut highs = Vec::with_capacity(days as usize);
    let mut lows = Vec::with_capacity(days as usize);
    let mut closes = Vec::with_capacity(days as usize);

    let mut price = dec!(50000); // 시작 가격

    for i in 0..days {
        let ts = (base_time + Duration::days(i)).timestamp_millis();
        timestamps.push(ts);

        // 변동성 있는 가격 생성
        let change_pct = if i % 5 == 0 {
            dec!(-0.02)
        } else if i % 3 == 0 {
            dec!(0.015)
        } else {
            dec!(0.005)
        };

        let open = price;
        let close = price * (dec!(1.0) + change_pct);
        let high = if close > open {
            close * dec!(1.005)
        } else {
            open * dec!(1.005)
        };
        let low = if close < open {
            close * dec!(0.995)
        } else {
            open * dec!(0.995)
        };

        opens.push(open);
        highs.push(high);
        lows.push(low);
        closes.push(close);

        price = close;
    }

    (timestamps, opens, highs, lows, closes)
}

/// SMA 지표 데이터 조회.
///
/// GET /api/v1/analytics/indicators/sma
pub async fn get_sma_indicator(
    Query(query): Query<SmaQuery>,
) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, _, _, closes) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = SmaParams { period: query.sma_period };

    match engine.sma(&closes, params) {
        Ok(sma_values) => {
            let data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(sma_values.iter())
                .map(|(&ts, value)| IndicatorPoint {
                    x: ts,
                    y: value.map(|v| v.to_string()),
                })
                .collect();

            Json(IndicatorDataResponse {
                indicator: "sma".to_string(),
                name: format!("SMA({})", query.sma_period),
                symbol: query.symbol,
                params: serde_json::json!({ "period": query.sma_period }),
                series: vec![IndicatorSeries {
                    name: "sma".to_string(),
                    data,
                    color: Some("#2196F3".to_string()),
                    series_type: "line".to_string(),
                }],
            })
        }
        Err(e) => Json(IndicatorDataResponse {
            indicator: "sma".to_string(),
            name: format!("SMA({}) - 오류", query.sma_period),
            symbol: query.symbol,
            params: serde_json::json!({ "error": e.to_string() }),
            series: vec![],
        }),
    }
}

/// EMA 지표 데이터 조회.
///
/// GET /api/v1/analytics/indicators/ema
pub async fn get_ema_indicator(
    Query(query): Query<EmaQuery>,
) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, _, _, closes) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = EmaParams { period: query.ema_period };

    match engine.ema(&closes, params) {
        Ok(ema_values) => {
            let data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(ema_values.iter())
                .map(|(&ts, value)| IndicatorPoint {
                    x: ts,
                    y: value.map(|v| v.to_string()),
                })
                .collect();

            Json(IndicatorDataResponse {
                indicator: "ema".to_string(),
                name: format!("EMA({})", query.ema_period),
                symbol: query.symbol,
                params: serde_json::json!({ "period": query.ema_period }),
                series: vec![IndicatorSeries {
                    name: "ema".to_string(),
                    data,
                    color: Some("#FF9800".to_string()),
                    series_type: "line".to_string(),
                }],
            })
        }
        Err(e) => Json(IndicatorDataResponse {
            indicator: "ema".to_string(),
            name: format!("EMA({}) - 오류", query.ema_period),
            symbol: query.symbol,
            params: serde_json::json!({ "error": e.to_string() }),
            series: vec![],
        }),
    }
}

/// RSI 지표 데이터 조회.
///
/// GET /api/v1/analytics/indicators/rsi
pub async fn get_rsi_indicator(
    Query(query): Query<RsiQuery>,
) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, _, _, closes) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = RsiParams { period: query.rsi_period };

    match engine.rsi(&closes, params) {
        Ok(rsi_values) => {
            let data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(rsi_values.iter())
                .map(|(&ts, value)| IndicatorPoint {
                    x: ts,
                    y: value.map(|v| v.to_string()),
                })
                .collect();

            Json(IndicatorDataResponse {
                indicator: "rsi".to_string(),
                name: format!("RSI({})", query.rsi_period),
                symbol: query.symbol,
                params: serde_json::json!({ "period": query.rsi_period }),
                series: vec![IndicatorSeries {
                    name: "rsi".to_string(),
                    data,
                    color: Some("#9C27B0".to_string()),
                    series_type: "line".to_string(),
                }],
            })
        }
        Err(e) => Json(IndicatorDataResponse {
            indicator: "rsi".to_string(),
            name: format!("RSI({}) - 오류", query.rsi_period),
            symbol: query.symbol,
            params: serde_json::json!({ "error": e.to_string() }),
            series: vec![],
        }),
    }
}

/// MACD 지표 데이터 조회.
///
/// GET /api/v1/analytics/indicators/macd
pub async fn get_macd_indicator(
    Query(query): Query<MacdQuery>,
) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, _, _, closes) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = MacdParams {
        fast_period: query.fast_period,
        slow_period: query.slow_period,
        signal_period: query.signal_period,
    };

    match engine.macd(&closes, params) {
        Ok(macd_results) => {
            let macd_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(macd_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.macd.map(|v| v.to_string()),
                })
                .collect();

            let signal_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(macd_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.signal.map(|v| v.to_string()),
                })
                .collect();

            let histogram_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(macd_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.histogram.map(|v| v.to_string()),
                })
                .collect();

            Json(IndicatorDataResponse {
                indicator: "macd".to_string(),
                name: format!("MACD({},{},{})", query.fast_period, query.slow_period, query.signal_period),
                symbol: query.symbol,
                params: serde_json::json!({
                    "fast_period": query.fast_period,
                    "slow_period": query.slow_period,
                    "signal_period": query.signal_period
                }),
                series: vec![
                    IndicatorSeries {
                        name: "macd".to_string(),
                        data: macd_data,
                        color: Some("#2196F3".to_string()),
                        series_type: "line".to_string(),
                    },
                    IndicatorSeries {
                        name: "signal".to_string(),
                        data: signal_data,
                        color: Some("#FF5722".to_string()),
                        series_type: "line".to_string(),
                    },
                    IndicatorSeries {
                        name: "histogram".to_string(),
                        data: histogram_data,
                        color: Some("#4CAF50".to_string()),
                        series_type: "bar".to_string(),
                    },
                ],
            })
        }
        Err(e) => Json(IndicatorDataResponse {
            indicator: "macd".to_string(),
            name: "MACD - 오류".to_string(),
            symbol: query.symbol,
            params: serde_json::json!({ "error": e.to_string() }),
            series: vec![],
        }),
    }
}

/// 볼린저 밴드 지표 데이터 조회.
///
/// GET /api/v1/analytics/indicators/bollinger
pub async fn get_bollinger_indicator(
    Query(query): Query<BollingerQuery>,
) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, _, _, closes) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = BollingerBandsParams {
        period: query.bb_period,
        std_dev_multiplier: Decimal::from_f64_retain(query.std_dev).unwrap_or(dec!(2.0)),
    };

    match engine.bollinger_bands(&closes, params) {
        Ok(bb_results) => {
            let upper_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(bb_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.upper.map(|v| v.to_string()),
                })
                .collect();

            let middle_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(bb_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.middle.map(|v| v.to_string()),
                })
                .collect();

            let lower_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(bb_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.lower.map(|v| v.to_string()),
                })
                .collect();

            Json(IndicatorDataResponse {
                indicator: "bollinger".to_string(),
                name: format!("BB({}, {})", query.bb_period, query.std_dev),
                symbol: query.symbol,
                params: serde_json::json!({
                    "period": query.bb_period,
                    "std_dev": query.std_dev
                }),
                series: vec![
                    IndicatorSeries {
                        name: "upper".to_string(),
                        data: upper_data,
                        color: Some("#E91E63".to_string()),
                        series_type: "line".to_string(),
                    },
                    IndicatorSeries {
                        name: "middle".to_string(),
                        data: middle_data,
                        color: Some("#9C27B0".to_string()),
                        series_type: "line".to_string(),
                    },
                    IndicatorSeries {
                        name: "lower".to_string(),
                        data: lower_data,
                        color: Some("#2196F3".to_string()),
                        series_type: "line".to_string(),
                    },
                ],
            })
        }
        Err(e) => Json(IndicatorDataResponse {
            indicator: "bollinger".to_string(),
            name: "Bollinger Bands - 오류".to_string(),
            symbol: query.symbol,
            params: serde_json::json!({ "error": e.to_string() }),
            series: vec![],
        }),
    }
}

/// 스토캐스틱 지표 데이터 조회.
///
/// GET /api/v1/analytics/indicators/stochastic
pub async fn get_stochastic_indicator(
    Query(query): Query<StochasticQuery>,
) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, highs, lows, closes) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = StochasticParams {
        k_period: query.k_period,
        d_period: query.d_period,
    };

    match engine.stochastic(&highs, &lows, &closes, params) {
        Ok(stoch_results) => {
            let k_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(stoch_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.k.map(|v| v.to_string()),
                })
                .collect();

            let d_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(stoch_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.d.map(|v| v.to_string()),
                })
                .collect();

            Json(IndicatorDataResponse {
                indicator: "stochastic".to_string(),
                name: format!("Stochastic({}, {})", query.k_period, query.d_period),
                symbol: query.symbol,
                params: serde_json::json!({
                    "k_period": query.k_period,
                    "d_period": query.d_period
                }),
                series: vec![
                    IndicatorSeries {
                        name: "%K".to_string(),
                        data: k_data,
                        color: Some("#2196F3".to_string()),
                        series_type: "line".to_string(),
                    },
                    IndicatorSeries {
                        name: "%D".to_string(),
                        data: d_data,
                        color: Some("#FF9800".to_string()),
                        series_type: "line".to_string(),
                    },
                ],
            })
        }
        Err(e) => Json(IndicatorDataResponse {
            indicator: "stochastic".to_string(),
            name: "Stochastic - 오류".to_string(),
            symbol: query.symbol,
            params: serde_json::json!({ "error": e.to_string() }),
            series: vec![],
        }),
    }
}

/// ATR 지표 데이터 조회.
///
/// GET /api/v1/analytics/indicators/atr
pub async fn get_atr_indicator(
    Query(query): Query<AtrQuery>,
) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, highs, lows, closes) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = AtrParams { period: query.atr_period };

    match engine.atr(&highs, &lows, &closes, params) {
        Ok(atr_values) => {
            let data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(atr_values.iter())
                .map(|(&ts, value)| IndicatorPoint {
                    x: ts,
                    y: value.map(|v| v.to_string()),
                })
                .collect();

            Json(IndicatorDataResponse {
                indicator: "atr".to_string(),
                name: format!("ATR({})", query.atr_period),
                symbol: query.symbol,
                params: serde_json::json!({ "period": query.atr_period }),
                series: vec![IndicatorSeries {
                    name: "atr".to_string(),
                    data,
                    color: Some("#795548".to_string()),
                    series_type: "line".to_string(),
                }],
            })
        }
        Err(e) => Json(IndicatorDataResponse {
            indicator: "atr".to_string(),
            name: format!("ATR({}) - 오류", query.atr_period),
            symbol: query.symbol,
            params: serde_json::json!({ "error": e.to_string() }),
            series: vec![],
        }),
    }
}

/// 다중 지표 계산.
///
/// POST /api/v1/analytics/indicators/calculate
pub async fn calculate_indicators(
    Json(request): Json<CalculateIndicatorsRequest>,
) -> impl IntoResponse {
    let days = parse_period_to_days(&request.period);
    let (timestamps, _, highs, lows, closes) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let mut results = Vec::new();

    for config in &request.indicators {
        let indicator_result = match config.indicator_type.as_str() {
            "sma" => {
                let period = config.params.get("period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(20) as usize;

                if let Ok(values) = engine.sma(&closes, SmaParams { period }) {
                    let data: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(values.iter())
                        .map(|(&ts, v)| IndicatorPoint { x: ts, y: v.map(|d| d.to_string()) })
                        .collect();

                    Some(IndicatorDataResponse {
                        indicator: "sma".to_string(),
                        name: config.name.clone().unwrap_or_else(|| format!("SMA({})", period)),
                        symbol: request.symbol.clone(),
                        params: serde_json::json!({ "period": period }),
                        series: vec![IndicatorSeries {
                            name: "sma".to_string(),
                            data,
                            color: config.color.clone().or_else(|| Some("#2196F3".to_string())),
                            series_type: "line".to_string(),
                        }],
                    })
                } else {
                    None
                }
            }
            "ema" => {
                let period = config.params.get("period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(12) as usize;

                if let Ok(values) = engine.ema(&closes, EmaParams { period }) {
                    let data: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(values.iter())
                        .map(|(&ts, v)| IndicatorPoint { x: ts, y: v.map(|d| d.to_string()) })
                        .collect();

                    Some(IndicatorDataResponse {
                        indicator: "ema".to_string(),
                        name: config.name.clone().unwrap_or_else(|| format!("EMA({})", period)),
                        symbol: request.symbol.clone(),
                        params: serde_json::json!({ "period": period }),
                        series: vec![IndicatorSeries {
                            name: "ema".to_string(),
                            data,
                            color: config.color.clone().or_else(|| Some("#FF9800".to_string())),
                            series_type: "line".to_string(),
                        }],
                    })
                } else {
                    None
                }
            }
            "rsi" => {
                let period = config.params.get("period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(14) as usize;

                if let Ok(values) = engine.rsi(&closes, RsiParams { period }) {
                    let data: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(values.iter())
                        .map(|(&ts, v)| IndicatorPoint { x: ts, y: v.map(|d| d.to_string()) })
                        .collect();

                    Some(IndicatorDataResponse {
                        indicator: "rsi".to_string(),
                        name: config.name.clone().unwrap_or_else(|| format!("RSI({})", period)),
                        symbol: request.symbol.clone(),
                        params: serde_json::json!({ "period": period }),
                        series: vec![IndicatorSeries {
                            name: "rsi".to_string(),
                            data,
                            color: config.color.clone().or_else(|| Some("#9C27B0".to_string())),
                            series_type: "line".to_string(),
                        }],
                    })
                } else {
                    None
                }
            }
            "macd" => {
                let fast = config.params.get("fast_period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(12) as usize;
                let slow = config.params.get("slow_period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(26) as usize;
                let signal = config.params.get("signal_period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(9) as usize;

                if let Ok(macd_results) = engine.macd(&closes, MacdParams { fast_period: fast, slow_period: slow, signal_period: signal }) {
                    let macd_data: Vec<IndicatorPoint> = timestamps.iter().zip(macd_results.iter())
                        .map(|(&ts, r)| IndicatorPoint { x: ts, y: r.macd.map(|d| d.to_string()) }).collect();
                    let signal_data: Vec<IndicatorPoint> = timestamps.iter().zip(macd_results.iter())
                        .map(|(&ts, r)| IndicatorPoint { x: ts, y: r.signal.map(|d| d.to_string()) }).collect();
                    let hist_data: Vec<IndicatorPoint> = timestamps.iter().zip(macd_results.iter())
                        .map(|(&ts, r)| IndicatorPoint { x: ts, y: r.histogram.map(|d| d.to_string()) }).collect();

                    Some(IndicatorDataResponse {
                        indicator: "macd".to_string(),
                        name: config.name.clone().unwrap_or_else(|| format!("MACD({},{},{})", fast, slow, signal)),
                        symbol: request.symbol.clone(),
                        params: serde_json::json!({ "fast_period": fast, "slow_period": slow, "signal_period": signal }),
                        series: vec![
                            IndicatorSeries { name: "macd".to_string(), data: macd_data, color: Some("#2196F3".to_string()), series_type: "line".to_string() },
                            IndicatorSeries { name: "signal".to_string(), data: signal_data, color: Some("#FF5722".to_string()), series_type: "line".to_string() },
                            IndicatorSeries { name: "histogram".to_string(), data: hist_data, color: Some("#4CAF50".to_string()), series_type: "bar".to_string() },
                        ],
                    })
                } else {
                    None
                }
            }
            "bollinger" => {
                let period = config.params.get("period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(20) as usize;
                let std_dev = config.params.get("std_dev")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(2.0);

                if let Ok(bb_results) = engine.bollinger_bands(&closes, BollingerBandsParams {
                    period,
                    std_dev_multiplier: Decimal::from_f64_retain(std_dev).unwrap_or(dec!(2.0)),
                }) {
                    let upper: Vec<IndicatorPoint> = timestamps.iter().zip(bb_results.iter())
                        .map(|(&ts, r)| IndicatorPoint { x: ts, y: r.upper.map(|d| d.to_string()) }).collect();
                    let middle: Vec<IndicatorPoint> = timestamps.iter().zip(bb_results.iter())
                        .map(|(&ts, r)| IndicatorPoint { x: ts, y: r.middle.map(|d| d.to_string()) }).collect();
                    let lower: Vec<IndicatorPoint> = timestamps.iter().zip(bb_results.iter())
                        .map(|(&ts, r)| IndicatorPoint { x: ts, y: r.lower.map(|d| d.to_string()) }).collect();

                    Some(IndicatorDataResponse {
                        indicator: "bollinger".to_string(),
                        name: config.name.clone().unwrap_or_else(|| format!("BB({}, {})", period, std_dev)),
                        symbol: request.symbol.clone(),
                        params: serde_json::json!({ "period": period, "std_dev": std_dev }),
                        series: vec![
                            IndicatorSeries { name: "upper".to_string(), data: upper, color: Some("#E91E63".to_string()), series_type: "line".to_string() },
                            IndicatorSeries { name: "middle".to_string(), data: middle, color: Some("#9C27B0".to_string()), series_type: "line".to_string() },
                            IndicatorSeries { name: "lower".to_string(), data: lower, color: Some("#2196F3".to_string()), series_type: "line".to_string() },
                        ],
                    })
                } else {
                    None
                }
            }
            "stochastic" => {
                let k_period = config.params.get("k_period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(14) as usize;
                let d_period = config.params.get("d_period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(3) as usize;

                if let Ok(stoch_results) = engine.stochastic(&highs, &lows, &closes, StochasticParams { k_period, d_period }) {
                    let k_data: Vec<IndicatorPoint> = timestamps.iter().zip(stoch_results.iter())
                        .map(|(&ts, r)| IndicatorPoint { x: ts, y: r.k.map(|d| d.to_string()) }).collect();
                    let d_data: Vec<IndicatorPoint> = timestamps.iter().zip(stoch_results.iter())
                        .map(|(&ts, r)| IndicatorPoint { x: ts, y: r.d.map(|d| d.to_string()) }).collect();

                    Some(IndicatorDataResponse {
                        indicator: "stochastic".to_string(),
                        name: config.name.clone().unwrap_or_else(|| format!("Stochastic({}, {})", k_period, d_period)),
                        symbol: request.symbol.clone(),
                        params: serde_json::json!({ "k_period": k_period, "d_period": d_period }),
                        series: vec![
                            IndicatorSeries { name: "%K".to_string(), data: k_data, color: Some("#2196F3".to_string()), series_type: "line".to_string() },
                            IndicatorSeries { name: "%D".to_string(), data: d_data, color: Some("#FF9800".to_string()), series_type: "line".to_string() },
                        ],
                    })
                } else {
                    None
                }
            }
            "atr" => {
                let period = config.params.get("period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(14) as usize;

                if let Ok(values) = engine.atr(&highs, &lows, &closes, AtrParams { period }) {
                    let data: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(values.iter())
                        .map(|(&ts, v)| IndicatorPoint { x: ts, y: v.map(|d| d.to_string()) })
                        .collect();

                    Some(IndicatorDataResponse {
                        indicator: "atr".to_string(),
                        name: config.name.clone().unwrap_or_else(|| format!("ATR({})", period)),
                        symbol: request.symbol.clone(),
                        params: serde_json::json!({ "period": period }),
                        series: vec![IndicatorSeries {
                            name: "atr".to_string(),
                            data,
                            color: config.color.clone().or_else(|| Some("#795548".to_string())),
                            series_type: "line".to_string(),
                        }],
                    })
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(result) = indicator_result {
            results.push(result);
        }
    }

    Json(CalculateIndicatorsResponse {
        symbol: request.symbol,
        period: request.period,
        results,
    })
}

/// 기간 문자열을 일수로 변환.
fn parse_period_to_days(period: &str) -> i64 {
    match period.to_lowercase().as_str() {
        "1d" => 1,
        "1w" => 7,
        "1m" => 30,
        "3m" => 90,
        "6m" => 180,
        "1y" | "12m" => 365,
        "all" => 1000,
        _ => 90, // 기본값: 3개월
    }
}

// ==================== 라우터 ====================

/// 포트폴리오 분석 라우터 생성.
pub fn analytics_router() -> Router<Arc<AppState>> {
    Router::new()
        // 포트폴리오 분석 엔드포인트
        .route("/performance", get(get_performance))
        .route("/equity-curve", get(get_equity_curve))
        .route("/charts/cagr", get(get_cagr_chart))
        .route("/charts/mdd", get(get_mdd_chart))
        .route("/charts/drawdown", get(get_drawdown_chart))
        .route("/monthly-returns", get(get_monthly_returns))
        // 자산 곡선 동기화
        .route("/sync-equity", axum::routing::post(sync_equity_curve))
        // 기술적 지표 엔드포인트
        .route("/indicators", get(get_available_indicators))
        .route("/indicators/sma", get(get_sma_indicator))
        .route("/indicators/ema", get(get_ema_indicator))
        .route("/indicators/rsi", get(get_rsi_indicator))
        .route("/indicators/macd", get(get_macd_indicator))
        .route("/indicators/bollinger", get(get_bollinger_indicator))
        .route("/indicators/stochastic", get(get_stochastic_indicator))
        .route("/indicators/atr", get(get_atr_indicator))
        .route("/indicators/calculate", axum::routing::post(calculate_indicators))
}

// ==================== 테스트 ====================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[test]
    fn test_parse_period_duration() {
        assert_eq!(parse_period_duration("1w").num_days(), 7);
        assert_eq!(parse_period_duration("1m").num_days(), 30);
        assert_eq!(parse_period_duration("3m").num_days(), 90);
        assert_eq!(parse_period_duration("6m").num_days(), 180);
        assert_eq!(parse_period_duration("1y").num_days(), 365);
    }

    #[test]
    fn test_analytics_manager_creation() {
        let manager = AnalyticsManager::new(dec!(10_000_000));
        assert!(manager.curve_cache.is_none());
    }

    #[test]
    fn test_analytics_manager_add_trade() {
        let mut manager = AnalyticsManager::new(dec!(10_000_000));
        manager.add_trade_result(Utc::now(), dec!(10_100_000));

        let curve = manager.get_curve();
        assert!(!curve.is_empty());
    }

    #[test]
    fn test_analytics_manager_sample_data() {
        let mut manager = AnalyticsManager::default();
        manager.load_sample_data();

        let summary = manager.get_performance_summary();
        assert!(summary.current_equity > Decimal::ZERO);
        assert!(summary.period_days > 0);
    }

    #[test]
    fn test_analytics_manager_charts() {
        let mut manager = AnalyticsManager::default();
        manager.load_sample_data();

        let charts = manager.get_charts(365);
        assert!(!charts.equity_curve.is_empty());
        assert!(!charts.drawdown_curve.is_empty());
    }

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

    #[tokio::test]
    async fn test_get_performance_endpoint() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/performance", get(get_performance))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/performance")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let perf: PerformanceResponse = serde_json::from_slice(&body).unwrap();

        assert!(!perf.current_equity.is_empty());
        assert!(perf.period_days > 0);
    }

    #[tokio::test]
    async fn test_get_equity_curve_endpoint() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/equity-curve", get(get_equity_curve))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/equity-curve?period=3m")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let curve: EquityCurveResponse = serde_json::from_slice(&body).unwrap();

        assert!(!curve.data.is_empty());
        assert_eq!(curve.period, "3m");
    }

    #[tokio::test]
    async fn test_get_monthly_returns_endpoint() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/monthly-returns", get(get_monthly_returns))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/monthly-returns")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let monthly: MonthlyReturnsResponse = serde_json::from_slice(&body).unwrap();

        assert!(!monthly.data.is_empty());
    }
}
