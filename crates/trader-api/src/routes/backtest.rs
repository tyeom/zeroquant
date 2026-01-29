//! 백테스트 API 엔드포인트
//!
//! 과거 데이터로 트레이딩 전략을 시뮬레이션하고 성과를 분석하는 API를 제공합니다.
//!
//! # 엔드포인트
//!
//! - `GET /api/v1/backtest/strategies` - 백테스트 가능한 전략 목록
//! - `POST /api/v1/backtest/run` - 백테스트 실행
//! - `GET /api/v1/backtest/results/:id` - 백테스트 결과 조회

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::state::AppState;
use trader_analytics::backtest::{BacktestConfig, BacktestEngine, BacktestReport};
use trader_core::{Kline, MarketType, Symbol, Timeframe};
use trader_data::{Database, KlineRepository, SymbolRepository};
use trader_strategy::Strategy;
use trader_strategy::strategies::{
    BollingerStrategy, GridStrategy, HaaStrategy, HaaConfig, MagicSplitStrategy, RsiStrategy,
    SimplePowerStrategy, SimplePowerConfig, SmaStrategy, StockRotationStrategy, StockRotationConfig,
    VolatilityBreakoutStrategy, XaaStrategy, XaaConfig,
};

// ==================== Yahoo Finance 응답 타입 ====================

#[derive(Debug, Deserialize)]
struct YahooResponse {
    chart: YahooChart,
}

#[derive(Debug, Deserialize)]
struct YahooChart {
    result: Option<Vec<YahooResult>>,
    error: Option<YahooError>,
}

#[derive(Debug, Deserialize)]
struct YahooError {
    code: String,
    description: String,
}

#[derive(Debug, Deserialize)]
struct YahooResult {
    timestamp: Option<Vec<i64>>,
    indicators: YahooIndicators,
}

#[derive(Debug, Deserialize)]
struct YahooIndicators {
    quote: Vec<YahooQuote>,
}

#[derive(Debug, Deserialize)]
struct YahooQuote {
    open: Vec<Option<f64>>,
    high: Vec<Option<f64>>,
    low: Vec<Option<f64>>,
    close: Vec<Option<f64>>,
    volume: Vec<Option<u64>>,
}

// ==================== 요청/응답 타입 ====================

// ==================== SDUI (Server Driven UI) 스키마 ====================

/// UI 필드 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiFieldType {
    /// 숫자 입력
    Number,
    /// 텍스트 입력
    Text,
    /// 드롭다운 선택
    Select,
    /// 체크박스
    Boolean,
    /// 심볼 선택 (멀티 선택 가능)
    SymbolPicker,
    /// 범위 슬라이더
    Range,
    /// 분할 레벨 배열 (Magic Split용)
    SplitLevels,
    /// 심볼 카테고리 그룹 (자산배분 전략용 - 카테고리별 심볼 선택)
    SymbolCategoryGroup,
    /// 날짜 선택
    Date,
    /// 시간대 선택
    Timeframe,
}

/// 유효성 검사 규칙
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiValidation {
    /// 필수 여부
    #[serde(default)]
    pub required: bool,
    /// 최소값 (Number, Range)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// 최대값 (Number, Range)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// 단계 (Number, Range)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    /// 최소 길이 (Text)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_length: Option<usize>,
    /// 최대 길이 (Text)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_length: Option<usize>,
    /// 정규식 패턴 (Text)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// 최소 선택 수 (SymbolPicker)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_items: Option<usize>,
    /// 최대 선택 수 (SymbolPicker)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_items: Option<usize>,
}

impl Default for UiValidation {
    fn default() -> Self {
        Self {
            required: false,
            min: None,
            max: None,
            step: None,
            min_length: None,
            max_length: None,
            pattern: None,
            min_items: None,
            max_items: None,
        }
    }
}

/// 선택 옵션 (Select 타입용)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiSelectOption {
    /// 표시 레이블
    pub label: String,
    /// 실제 값
    pub value: serde_json::Value,
    /// 설명 (툴팁)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// 심볼 카테고리 정의 (자산배분 전략용)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolCategory {
    /// 카테고리 키 (예: "canary_assets", "offensive_assets")
    pub key: String,
    /// 카테고리 표시 이름 (예: "카나리아 자산")
    pub label: String,
    /// 카테고리 설명
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 기본 심볼 목록
    #[serde(default)]
    pub default_symbols: Vec<String>,
    /// 추천 심볼 목록 (사용자에게 선택 가이드)
    #[serde(default)]
    pub suggested_symbols: Vec<String>,
    /// 최소 선택 수
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_items: Option<usize>,
    /// 최대 선택 수
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_items: Option<usize>,
    /// 표시 순서
    #[serde(default)]
    pub order: i32,
}

/// UI 필드 정의 (SDUI 핵심)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiField {
    /// 필드 키 (파라미터 이름)
    pub key: String,
    /// 표시 레이블
    pub label: String,
    /// 필드 타입
    pub field_type: UiFieldType,
    /// 기본값
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<serde_json::Value>,
    /// 플레이스홀더
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// 도움말/설명
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_text: Option<String>,
    /// 유효성 검사 규칙
    #[serde(default)]
    pub validation: UiValidation,
    /// 선택 옵션 (Select 타입용)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<UiSelectOption>>,
    /// 심볼 카테고리 목록 (SymbolCategoryGroup 타입용)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_categories: Option<Vec<SymbolCategory>>,
    /// 그룹 ID (필드 그룹핑용)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// 표시 순서
    #[serde(default)]
    pub order: i32,
    /// 조건부 표시 (다른 필드 값에 따라)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_when: Option<UiCondition>,
    /// 단위 (예: %, 원, USD)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

/// 조건부 표시 규칙
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiCondition {
    /// 참조 필드 키
    pub field: String,
    /// 연산자
    pub operator: UiConditionOperator,
    /// 비교 값
    pub value: serde_json::Value,
}

/// 조건 연산자
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiConditionOperator {
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    Contains,
}

/// 필드 그룹 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiFieldGroup {
    /// 그룹 ID
    pub id: String,
    /// 그룹 레이블
    pub label: String,
    /// 그룹 설명
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 표시 순서
    #[serde(default)]
    pub order: i32,
    /// 접힘 여부 (기본 접힘)
    #[serde(default)]
    pub collapsed: bool,
}

/// SDUI 스키마 (전략별 UI 정의)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiSchema {
    /// 필드 정의 목록
    pub fields: Vec<UiField>,
    /// 필드 그룹 정의
    #[serde(default)]
    pub groups: Vec<UiFieldGroup>,
    /// 레이아웃 힌트 (예: 2열 레이아웃)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<UiLayout>,
}

/// 레이아웃 힌트
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiLayout {
    /// 열 수 (기본 1)
    #[serde(default = "default_columns")]
    pub columns: usize,
}

fn default_columns() -> usize {
    1
}

/// 전략 실행 주기
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionSchedule {
    /// 실시간 (가격 변동 시마다)
    Realtime,
    /// 캔들 완성 시 (분봉/일봉)
    OnCandleClose,
    /// 일 1회
    Daily,
    /// 주 1회
    Weekly,
    /// 월 1회
    Monthly,
}

impl ExecutionSchedule {
    /// 한글 표시명
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Realtime => "실시간",
            Self::OnCandleClose => "캔들 완성 시",
            Self::Daily => "일 1회",
            Self::Weekly => "주 1회",
            Self::Monthly => "월 1회",
        }
    }

    /// 상세 설명
    pub fn description(&self) -> &'static str {
        match self {
            Self::Realtime => "가격 변동 시마다 전략을 실행합니다",
            Self::OnCandleClose => "분봉 또는 일봉 완성 시 전략을 실행합니다",
            Self::Daily => "매일 지정된 시간에 전략을 실행합니다",
            Self::Weekly => "매주 지정된 요일에 전략을 실행합니다",
            Self::Monthly => "매월 첫 거래일에 전략을 실행합니다",
        }
    }
}

/// 백테스트 가능한 전략 항목
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestableStrategy {
    /// 전략 ID
    pub id: String,
    /// 전략 이름
    pub name: String,
    /// 전략 설명
    pub description: String,
    /// 지원하는 심볼 목록
    pub supported_symbols: Vec<String>,
    /// 기본 설정 파라미터
    pub default_params: serde_json::Value,
    /// SDUI 스키마 (UI 메타데이터)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui_schema: Option<UiSchema>,
    /// 전략 카테고리
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// 전략 태그
    #[serde(default)]
    pub tags: Vec<String>,
    /// 실행 주기
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_schedule: Option<ExecutionSchedule>,
    /// 실행 주기 상세 설명 (예: "장 시작 5분 후", "매월 첫 거래일")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule_detail: Option<String>,
    /// 작동 방식 상세 설명
    #[serde(skip_serializing_if = "Option::is_none")]
    pub how_it_works: Option<String>,
}

/// 백테스트 가능한 전략 목록 응답
#[derive(Debug, Serialize, Deserialize)]
pub struct BacktestStrategiesResponse {
    /// 전략 목록
    pub strategies: Vec<BacktestableStrategy>,
    /// 전체 전략 수
    pub total: usize,
}

/// 백테스트 실행 요청
#[derive(Debug, Deserialize)]
pub struct BacktestRunRequest {
    /// 전략 ID
    pub strategy_id: String,
    /// 거래 심볼 (예: "BTC/USDT")
    pub symbol: String,
    /// 시작 날짜 (YYYY-MM-DD)
    pub start_date: String,
    /// 종료 날짜 (YYYY-MM-DD)
    pub end_date: String,
    /// 초기 자본금
    pub initial_capital: Decimal,
    /// 수수료율 (선택, 기본: 0.001 = 0.1%)
    #[serde(default)]
    pub commission_rate: Option<Decimal>,
    /// 슬리피지율 (선택, 기본: 0.0005 = 0.05%)
    #[serde(default)]
    pub slippage_rate: Option<Decimal>,
    /// 전략 파라미터 (선택)
    #[serde(default)]
    pub parameters: Option<serde_json::Value>,
}

/// 다중 자산 백테스트 실행 요청
#[derive(Debug, Deserialize)]
pub struct BacktestMultiRunRequest {
    /// 전략 ID
    pub strategy_id: String,
    /// 거래 심볼 목록 (예: ["TQQQ", "SCHD", "PFIX", "TMF"])
    pub symbols: Vec<String>,
    /// 시작 날짜 (YYYY-MM-DD)
    pub start_date: String,
    /// 종료 날짜 (YYYY-MM-DD)
    pub end_date: String,
    /// 초기 자본금
    pub initial_capital: Decimal,
    /// 수수료율 (선택, 기본: 0.001 = 0.1%)
    #[serde(default)]
    pub commission_rate: Option<Decimal>,
    /// 슬리피지율 (선택, 기본: 0.0005 = 0.05%)
    #[serde(default)]
    pub slippage_rate: Option<Decimal>,
    /// 전략 파라미터 (선택)
    #[serde(default)]
    pub parameters: Option<serde_json::Value>,
}

/// 다중 자산 백테스트 실행 응답
#[derive(Debug, Serialize, Deserialize)]
pub struct BacktestMultiRunResponse {
    /// 백테스트 결과 ID
    pub id: String,
    /// 성공 여부
    pub success: bool,
    /// 전략 ID
    pub strategy_id: String,
    /// 심볼 목록
    pub symbols: Vec<String>,
    /// 시작 날짜
    pub start_date: String,
    /// 종료 날짜
    pub end_date: String,
    /// 성과 지표
    pub metrics: BacktestMetricsResponse,
    /// 자산 곡선 (시간순)
    pub equity_curve: Vec<EquityCurvePoint>,
    /// 거래 내역
    pub trades: Vec<TradeHistoryItem>,
    /// 백테스트 설정 요약
    pub config_summary: BacktestConfigSummary,
    /// 심볼별 데이터 포인트 수
    pub data_points_by_symbol: std::collections::HashMap<String, usize>,
}

/// 백테스트 성과 지표 응답
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestMetricsResponse {
    /// 총 수익률 (%)
    pub total_return_pct: Decimal,
    /// 연율화 수익률 (%)
    pub annualized_return_pct: Decimal,
    /// 순수익
    pub net_profit: Decimal,
    /// 총 거래 수
    pub total_trades: usize,
    /// 승률 (%)
    pub win_rate_pct: Decimal,
    /// 프로핏 팩터
    pub profit_factor: Decimal,
    /// 샤프 비율
    pub sharpe_ratio: Decimal,
    /// 소르티노 비율
    pub sortino_ratio: Decimal,
    /// 최대 낙폭 (%)
    pub max_drawdown_pct: Decimal,
    /// 칼마 비율
    pub calmar_ratio: Decimal,
    /// 평균 수익 거래
    pub avg_win: Decimal,
    /// 평균 손실 거래
    pub avg_loss: Decimal,
    /// 최대 수익 거래
    pub largest_win: Decimal,
    /// 최대 손실 거래
    pub largest_loss: Decimal,
}

/// 자산 곡선 데이터 포인트
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityCurvePoint {
    /// 타임스탬프 (Unix timestamp)
    pub timestamp: i64,
    /// 자산 가치
    pub equity: Decimal,
    /// 낙폭 (%)
    pub drawdown_pct: Decimal,
}

/// 거래 내역 항목
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeHistoryItem {
    /// 심볼
    pub symbol: String,
    /// 진입 시간
    pub entry_time: DateTime<Utc>,
    /// 청산 시간
    pub exit_time: DateTime<Utc>,
    /// 진입가
    pub entry_price: Decimal,
    /// 청산가
    pub exit_price: Decimal,
    /// 수량
    pub quantity: Decimal,
    /// 방향 (Buy/Sell)
    pub side: String,
    /// 손익
    pub pnl: Decimal,
    /// 손익률 (%)
    pub return_pct: Decimal,
}

/// 백테스트 실행 응답
#[derive(Debug, Serialize, Deserialize)]
pub struct BacktestRunResponse {
    /// 백테스트 결과 ID
    pub id: String,
    /// 성공 여부
    pub success: bool,
    /// 전략 ID
    pub strategy_id: String,
    /// 심볼
    pub symbol: String,
    /// 시작 날짜
    pub start_date: String,
    /// 종료 날짜
    pub end_date: String,
    /// 성과 지표
    pub metrics: BacktestMetricsResponse,
    /// 자산 곡선 (시간순)
    pub equity_curve: Vec<EquityCurvePoint>,
    /// 거래 내역
    pub trades: Vec<TradeHistoryItem>,
    /// 백테스트 설정 요약
    pub config_summary: BacktestConfigSummary,
}

/// 백테스트 설정 요약
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfigSummary {
    /// 초기 자본금
    pub initial_capital: Decimal,
    /// 수수료율
    pub commission_rate: Decimal,
    /// 슬리피지율
    pub slippage_rate: Decimal,
    /// 총 수수료
    pub total_commission: Decimal,
    /// 총 슬리피지 비용
    pub total_slippage: Decimal,
    /// 데이터 포인트 수
    pub data_points: usize,
}

/// API 에러 응답
#[derive(Debug, Serialize, Deserialize)]
pub struct BacktestApiError {
    /// 에러 코드
    pub code: String,
    /// 에러 메시지
    pub message: String,
}

impl BacktestApiError {
    /// 새로운 에러 생성
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

// ==================== SDUI 스키마 빌더 ====================

/// RSI 평균회귀 전략 UI 스키마
fn build_rsi_ui_schema() -> UiSchema {
    UiSchema {
        fields: vec![
            UiField {
                key: "symbol".to_string(),
                label: "종목".to_string(),
                field_type: UiFieldType::SymbolPicker,
                default_value: Some(serde_json::json!(["005930"])),
                placeholder: Some("종목을 선택하세요".to_string()),
                help_text: Some("전략을 적용할 종목을 선택합니다".to_string()),
                validation: UiValidation {
                    required: true,
                    min_items: Some(1),
                    max_items: Some(1),
                    ..Default::default()
                },
                options: None,
                group: Some("basic".to_string()),
                order: 1,
                show_when: None,
                unit: None,
                symbol_categories: None,
            },
            UiField {
                key: "period".to_string(),
                label: "RSI 기간".to_string(),
                field_type: UiFieldType::Number,
                default_value: Some(serde_json::json!(14)),
                placeholder: None,
                help_text: Some("RSI 계산에 사용할 기간 (일반적으로 14)".to_string()),
                validation: UiValidation {
                    required: true,
                    min: Some(2.0),
                    max: Some(50.0),
                    step: Some(1.0),
                    ..Default::default()
                },
                options: None,
                group: Some("indicator".to_string()),
                order: 2,
                show_when: None,
                unit: Some("일".to_string()),
                symbol_categories: None,
            },
            UiField {
                key: "oversold_threshold".to_string(),
                label: "과매도 기준".to_string(),
                field_type: UiFieldType::Range,
                default_value: Some(serde_json::json!(30)),
                placeholder: None,
                help_text: Some("RSI가 이 값 아래로 떨어지면 매수 신호".to_string()),
                validation: UiValidation {
                    required: true,
                    min: Some(10.0),
                    max: Some(50.0),
                    step: Some(5.0),
                    ..Default::default()
                },
                options: None,
                group: Some("indicator".to_string()),
                order: 3,
                show_when: None,
                unit: None,
                symbol_categories: None,
            },
            UiField {
                key: "overbought_threshold".to_string(),
                label: "과매수 기준".to_string(),
                field_type: UiFieldType::Range,
                default_value: Some(serde_json::json!(70)),
                placeholder: None,
                help_text: Some("RSI가 이 값 위로 올라가면 매도 신호".to_string()),
                validation: UiValidation {
                    required: true,
                    min: Some(50.0),
                    max: Some(90.0),
                    step: Some(5.0),
                    ..Default::default()
                },
                options: None,
                group: Some("indicator".to_string()),
                order: 4,
                show_when: None,
                unit: None,
                symbol_categories: None,
            },
            UiField {
                key: "amount".to_string(),
                label: "주문 금액".to_string(),
                field_type: UiFieldType::Number,
                default_value: Some(serde_json::json!(100000)),
                placeholder: Some("100000".to_string()),
                help_text: Some("한 번 거래 시 투자할 금액".to_string()),
                validation: UiValidation {
                    required: true,
                    min: Some(10000.0),
                    step: Some(10000.0),
                    ..Default::default()
                },
                options: None,
                group: Some("risk".to_string()),
                order: 5,
                show_when: None,
                unit: Some("원".to_string()),
                symbol_categories: None,
            },
        ],
        groups: vec![
            UiFieldGroup {
                id: "basic".to_string(),
                label: "기본 설정".to_string(),
                description: None,
                order: 1,
                collapsed: false,
            },
            UiFieldGroup {
                id: "indicator".to_string(),
                label: "지표 설정".to_string(),
                description: Some("RSI 지표 관련 파라미터".to_string()),
                order: 2,
                collapsed: false,
            },
            UiFieldGroup {
                id: "risk".to_string(),
                label: "리스크 관리".to_string(),
                description: None,
                order: 3,
                collapsed: true,
            },
        ],
        layout: Some(UiLayout { columns: 2 }),
    }
}

/// 그리드 트레이딩 전략 UI 스키마
fn build_grid_ui_schema() -> UiSchema {
    UiSchema {
        fields: vec![
            UiField {
                key: "symbol".to_string(),
                label: "종목".to_string(),
                field_type: UiFieldType::SymbolPicker,
                default_value: Some(serde_json::json!(["005930"])),
                placeholder: Some("종목을 선택하세요".to_string()),
                help_text: None,
                validation: UiValidation {
                    required: true,
                    min_items: Some(1),
                    max_items: Some(1),
                    ..Default::default()
                },
                options: None,
                group: Some("basic".to_string()),
                order: 1,
                show_when: None,
                unit: None,
                symbol_categories: None,
            },
            UiField {
                key: "grid_spacing_pct".to_string(),
                label: "그리드 간격".to_string(),
                field_type: UiFieldType::Number,
                default_value: Some(serde_json::json!(1.0)),
                placeholder: None,
                help_text: Some("그리드 레벨 간 가격 간격 (%)".to_string()),
                validation: UiValidation {
                    required: true,
                    min: Some(0.5),
                    max: Some(10.0),
                    step: Some(0.5),
                    ..Default::default()
                },
                options: None,
                group: Some("grid".to_string()),
                order: 2,
                show_when: None,
                unit: Some("%".to_string()),
                symbol_categories: None,
            },
            UiField {
                key: "grid_levels".to_string(),
                label: "그리드 레벨 수".to_string(),
                field_type: UiFieldType::Number,
                default_value: Some(serde_json::json!(10)),
                placeholder: None,
                help_text: Some("위/아래 각각의 그리드 레벨 수".to_string()),
                validation: UiValidation {
                    required: true,
                    min: Some(3.0),
                    max: Some(50.0),
                    step: Some(1.0),
                    ..Default::default()
                },
                options: None,
                group: Some("grid".to_string()),
                order: 3,
                show_when: None,
                unit: Some("개".to_string()),
                symbol_categories: None,
            },
            UiField {
                key: "amount_per_level".to_string(),
                label: "레벨당 투자 금액".to_string(),
                field_type: UiFieldType::Number,
                default_value: Some(serde_json::json!(100000)),
                placeholder: None,
                help_text: Some("각 그리드 레벨에서 투자할 금액".to_string()),
                validation: UiValidation {
                    required: true,
                    min: Some(10000.0),
                    step: Some(10000.0),
                    ..Default::default()
                },
                options: None,
                group: Some("risk".to_string()),
                order: 4,
                show_when: None,
                unit: Some("원".to_string()),
                symbol_categories: None,
            },
        ],
        groups: vec![
            UiFieldGroup {
                id: "basic".to_string(),
                label: "기본 설정".to_string(),
                description: None,
                order: 1,
                collapsed: false,
            },
            UiFieldGroup {
                id: "grid".to_string(),
                label: "그리드 설정".to_string(),
                description: Some("그리드 간격 및 레벨 설정".to_string()),
                order: 2,
                collapsed: false,
            },
            UiFieldGroup {
                id: "risk".to_string(),
                label: "리스크 관리".to_string(),
                description: None,
                order: 3,
                collapsed: false,
            },
        ],
        layout: Some(UiLayout { columns: 2 }),
    }
}

/// Simple Power 전략 UI 스키마
fn build_simple_power_ui_schema() -> UiSchema {
    UiSchema {
        fields: vec![
            UiField {
                key: "symbols".to_string(),
                label: "투자 종목".to_string(),
                field_type: UiFieldType::SymbolPicker,
                default_value: Some(serde_json::json!(["TQQQ", "SCHD", "TMF", "PFIX"])),
                placeholder: Some("종목을 선택하세요".to_string()),
                help_text: Some("TQQQ/SCHD/TMF/PFIX 기본 구성".to_string()),
                validation: UiValidation {
                    required: true,
                    min_items: Some(2),
                    max_items: Some(10),
                    ..Default::default()
                },
                options: None,
                group: Some("basic".to_string()),
                order: 1,
                show_when: None,
                unit: None,
                symbol_categories: None,
            },
            UiField {
                key: "ma_period".to_string(),
                label: "이동평균 기간".to_string(),
                field_type: UiFieldType::Number,
                default_value: Some(serde_json::json!(130)),
                placeholder: None,
                help_text: Some("모멘텀 확인용 이동평균 기간".to_string()),
                validation: UiValidation {
                    required: true,
                    min: Some(20.0),
                    max: Some(200.0),
                    step: Some(10.0),
                    ..Default::default()
                },
                options: None,
                group: Some("momentum".to_string()),
                order: 2,
                show_when: None,
                unit: Some("일".to_string()),
                symbol_categories: None,
            },
            UiField {
                key: "rebalance_day".to_string(),
                label: "리밸런싱 요일".to_string(),
                field_type: UiFieldType::Select,
                default_value: Some(serde_json::json!("monday")),
                placeholder: None,
                help_text: Some("매주 리밸런싱을 실행할 요일".to_string()),
                validation: UiValidation { required: true, ..Default::default() },
                options: Some(vec![
                    UiSelectOption { label: "월요일".to_string(), value: serde_json::json!("monday"), description: None },
                    UiSelectOption { label: "수요일".to_string(), value: serde_json::json!("wednesday"), description: None },
                    UiSelectOption { label: "금요일".to_string(), value: serde_json::json!("friday"), description: None },
                ]),
                group: Some("schedule".to_string()),
                order: 3,
                show_when: None,
                unit: None,
                symbol_categories: None,
            },
        ],
        groups: vec![
            UiFieldGroup {
                id: "basic".to_string(),
                label: "기본 설정".to_string(),
                description: None,
                order: 1,
                collapsed: false,
            },
            UiFieldGroup {
                id: "momentum".to_string(),
                label: "모멘텀 설정".to_string(),
                description: None,
                order: 2,
                collapsed: false,
            },
            UiFieldGroup {
                id: "schedule".to_string(),
                label: "리밸런싱 스케줄".to_string(),
                description: None,
                order: 3,
                collapsed: true,
            },
        ],
        layout: Some(UiLayout { columns: 2 }),
    }
}

/// HAA 전략 UI 스키마
fn build_haa_ui_schema() -> UiSchema {
    UiSchema {
        fields: vec![
            UiField {
                key: "canary_assets".to_string(),
                label: "카나리아 자산".to_string(),
                field_type: UiFieldType::SymbolPicker,
                default_value: Some(serde_json::json!(["TIP", "BIL"])),
                placeholder: None,
                help_text: Some("위험 감지용 카나리아 자산".to_string()),
                validation: UiValidation {
                    required: true,
                    min_items: Some(1),
                    ..Default::default()
                },
                options: None,
                group: Some("assets".to_string()),
                order: 1,
                show_when: None,
                unit: None,
                symbol_categories: None,
            },
            UiField {
                key: "offensive_assets".to_string(),
                label: "공격적 자산".to_string(),
                field_type: UiFieldType::SymbolPicker,
                default_value: Some(serde_json::json!(["SPY", "VEA", "VWO"])),
                placeholder: None,
                help_text: Some("모멘텀 기반 투자 자산".to_string()),
                validation: UiValidation {
                    required: true,
                    min_items: Some(1),
                    ..Default::default()
                },
                options: None,
                group: Some("assets".to_string()),
                order: 2,
                show_when: None,
                unit: None,
                symbol_categories: None,
            },
            UiField {
                key: "defensive_assets".to_string(),
                label: "방어적 자산".to_string(),
                field_type: UiFieldType::SymbolPicker,
                default_value: Some(serde_json::json!(["IEF", "TLT"])),
                placeholder: None,
                help_text: Some("위험 시 투자할 방어 자산".to_string()),
                validation: UiValidation {
                    required: true,
                    min_items: Some(1),
                    ..Default::default()
                },
                options: None,
                group: Some("assets".to_string()),
                order: 3,
                show_when: None,
                unit: None,
                symbol_categories: None,
            },
            UiField {
                key: "momentum_period".to_string(),
                label: "모멘텀 기간".to_string(),
                field_type: UiFieldType::Number,
                default_value: Some(serde_json::json!(12)),
                placeholder: None,
                help_text: Some("모멘텀 계산 기간 (개월)".to_string()),
                validation: UiValidation {
                    required: true,
                    min: Some(1.0),
                    max: Some(24.0),
                    step: Some(1.0),
                    ..Default::default()
                },
                options: None,
                group: Some("momentum".to_string()),
                order: 4,
                show_when: None,
                unit: Some("개월".to_string()),
                symbol_categories: None,
            },
            UiField {
                key: "top_n".to_string(),
                label: "상위 N개 선택".to_string(),
                field_type: UiFieldType::Number,
                default_value: Some(serde_json::json!(2)),
                placeholder: None,
                help_text: Some("모멘텀 상위 N개 자산에 투자".to_string()),
                validation: UiValidation {
                    required: true,
                    min: Some(1.0),
                    max: Some(5.0),
                    step: Some(1.0),
                    ..Default::default()
                },
                options: None,
                group: Some("momentum".to_string()),
                order: 5,
                show_when: None,
                unit: Some("개".to_string()),
                symbol_categories: None,
            },
        ],
        groups: vec![
            UiFieldGroup {
                id: "assets".to_string(),
                label: "자산 구성".to_string(),
                description: Some("카나리아, 공격적, 방어적 자산 설정".to_string()),
                order: 1,
                collapsed: false,
            },
            UiFieldGroup {
                id: "momentum".to_string(),
                label: "모멘텀 설정".to_string(),
                description: None,
                order: 2,
                collapsed: false,
            },
        ],
        layout: Some(UiLayout { columns: 1 }),
    }
}

/// Magic Split 전략 UI 스키마
fn build_magic_split_ui_schema() -> UiSchema {
    UiSchema {
        fields: vec![
            UiField {
                key: "symbol".to_string(),
                label: "종목".to_string(),
                field_type: UiFieldType::SymbolPicker,
                default_value: Some(serde_json::json!(["305540"])),
                placeholder: Some("종목을 선택하세요".to_string()),
                help_text: Some("분할 매수를 적용할 종목".to_string()),
                validation: UiValidation {
                    required: true,
                    min_items: Some(1),
                    max_items: Some(1),
                    ..Default::default()
                },
                options: None,
                group: Some("basic".to_string()),
                order: 1,
                show_when: None,
                unit: None,
                symbol_categories: None,
            },
            UiField {
                key: "max_levels".to_string(),
                label: "최대 분할 차수".to_string(),
                field_type: UiFieldType::Number,
                default_value: Some(serde_json::json!(10)),
                placeholder: None,
                help_text: Some("최대 몇 차수까지 물타기 할 것인지".to_string()),
                validation: UiValidation {
                    required: true,
                    min: Some(2.0),
                    max: Some(20.0),
                    step: Some(1.0),
                    ..Default::default()
                },
                options: None,
                group: Some("split".to_string()),
                order: 2,
                show_when: None,
                unit: Some("차".to_string()),
                symbol_categories: None,
            },
            UiField {
                key: "level_spacing_pct".to_string(),
                label: "차수 간격".to_string(),
                field_type: UiFieldType::Number,
                default_value: Some(serde_json::json!(3.0)),
                placeholder: None,
                help_text: Some("다음 차수 진입을 위한 하락 %)".to_string()),
                validation: UiValidation {
                    required: true,
                    min: Some(1.0),
                    max: Some(10.0),
                    step: Some(0.5),
                    ..Default::default()
                },
                options: None,
                group: Some("split".to_string()),
                order: 3,
                show_when: None,
                unit: Some("%".to_string()),
                symbol_categories: None,
            },
            UiField {
                key: "first_level_amount".to_string(),
                label: "1차수 투자금".to_string(),
                field_type: UiFieldType::Number,
                default_value: Some(serde_json::json!(200000)),
                placeholder: None,
                help_text: Some("1차수 진입 시 투자 금액".to_string()),
                validation: UiValidation {
                    required: true,
                    min: Some(10000.0),
                    step: Some(10000.0),
                    ..Default::default()
                },
                options: None,
                group: Some("amount".to_string()),
                order: 4,
                show_when: None,
                unit: Some("원".to_string()),
                symbol_categories: None,
            },
            UiField {
                key: "additional_amount".to_string(),
                label: "추가 차수 투자금".to_string(),
                field_type: UiFieldType::Number,
                default_value: Some(serde_json::json!(100000)),
                placeholder: None,
                help_text: Some("2차수 이후 각 차수 투자 금액".to_string()),
                validation: UiValidation {
                    required: true,
                    min: Some(10000.0),
                    step: Some(10000.0),
                    ..Default::default()
                },
                options: None,
                group: Some("amount".to_string()),
                order: 5,
                show_when: None,
                unit: Some("원".to_string()),
                symbol_categories: None,
            },
            UiField {
                key: "target_profit_pct".to_string(),
                label: "목표 수익률".to_string(),
                field_type: UiFieldType::Number,
                default_value: Some(serde_json::json!(5.0)),
                placeholder: None,
                help_text: Some("이 수익률 달성 시 전량 매도".to_string()),
                validation: UiValidation {
                    required: true,
                    min: Some(1.0),
                    max: Some(30.0),
                    step: Some(0.5),
                    ..Default::default()
                },
                options: None,
                group: Some("target".to_string()),
                order: 6,
                show_when: None,
                unit: Some("%".to_string()),
                symbol_categories: None,
            },
        ],
        groups: vec![
            UiFieldGroup {
                id: "basic".to_string(),
                label: "기본 설정".to_string(),
                description: None,
                order: 1,
                collapsed: false,
            },
            UiFieldGroup {
                id: "split".to_string(),
                label: "분할 매수 설정".to_string(),
                description: Some("차수별 진입 조건".to_string()),
                order: 2,
                collapsed: false,
            },
            UiFieldGroup {
                id: "amount".to_string(),
                label: "투자 금액".to_string(),
                description: None,
                order: 3,
                collapsed: false,
            },
            UiFieldGroup {
                id: "target".to_string(),
                label: "익절 설정".to_string(),
                description: None,
                order: 4,
                collapsed: false,
            },
        ],
        layout: Some(UiLayout { columns: 2 }),
    }
}

/// 볼린저 밴드 전략 UI 스키마
fn build_bollinger_ui_schema() -> UiSchema {
    UiSchema {
        fields: vec![
            UiField {
                key: "symbol".to_string(),
                label: "종목".to_string(),
                field_type: UiFieldType::SymbolPicker,
                default_value: Some(serde_json::json!(["005930"])),
                placeholder: None,
                help_text: None,
                validation: UiValidation {
                    required: true,
                    min_items: Some(1),
                    max_items: Some(1),
                    ..Default::default()
                },
                options: None,
                group: Some("basic".to_string()),
                order: 1,
                show_when: None,
                unit: None,
                symbol_categories: None,
            },
            UiField {
                key: "period".to_string(),
                label: "기간".to_string(),
                field_type: UiFieldType::Number,
                default_value: Some(serde_json::json!(20)),
                placeholder: None,
                help_text: Some("볼린저 밴드 계산 기간".to_string()),
                validation: UiValidation {
                    required: true,
                    min: Some(10.0),
                    max: Some(50.0),
                    step: Some(1.0),
                    ..Default::default()
                },
                options: None,
                group: Some("indicator".to_string()),
                order: 2,
                show_when: None,
                unit: Some("일".to_string()),
                symbol_categories: None,
            },
            UiField {
                key: "std_multiplier".to_string(),
                label: "표준편차 배수".to_string(),
                field_type: UiFieldType::Number,
                default_value: Some(serde_json::json!(2.0)),
                placeholder: None,
                help_text: Some("밴드 폭 (일반적으로 2.0)".to_string()),
                validation: UiValidation {
                    required: true,
                    min: Some(1.0),
                    max: Some(3.0),
                    step: Some(0.1),
                    ..Default::default()
                },
                options: None,
                group: Some("indicator".to_string()),
                order: 3,
                show_when: None,
                unit: Some("σ".to_string()),
                symbol_categories: None,
            },
            UiField {
                key: "amount".to_string(),
                label: "주문 금액".to_string(),
                field_type: UiFieldType::Number,
                default_value: Some(serde_json::json!(100000)),
                placeholder: None,
                help_text: None,
                validation: UiValidation {
                    required: true,
                    min: Some(10000.0),
                    step: Some(10000.0),
                    ..Default::default()
                },
                options: None,
                group: Some("risk".to_string()),
                order: 4,
                show_when: None,
                unit: Some("원".to_string()),
                symbol_categories: None,
            },
        ],
        groups: vec![
            UiFieldGroup { id: "basic".to_string(), label: "기본 설정".to_string(), description: None, order: 1, collapsed: false },
            UiFieldGroup { id: "indicator".to_string(), label: "지표 설정".to_string(), description: None, order: 2, collapsed: false },
            UiFieldGroup { id: "risk".to_string(), label: "리스크 관리".to_string(), description: None, order: 3, collapsed: true },
        ],
        layout: Some(UiLayout { columns: 2 }),
    }
}

/// 변동성 돌파 전략 UI 스키마
fn build_volatility_breakout_ui_schema() -> UiSchema {
    UiSchema {
        fields: vec![
            UiField {
                key: "symbol".to_string(),
                label: "종목".to_string(),
                field_type: UiFieldType::SymbolPicker,
                default_value: Some(serde_json::json!(["005930"])),
                placeholder: None,
                help_text: None,
                validation: UiValidation { required: true, min_items: Some(1), max_items: Some(1), ..Default::default() },
                options: None,
                group: Some("basic".to_string()),
                order: 1,
                show_when: None,
                unit: None,
                symbol_categories: None,
            },
            UiField {
                key: "k_factor".to_string(),
                label: "K 팩터".to_string(),
                field_type: UiFieldType::Number,
                default_value: Some(serde_json::json!(0.5)),
                placeholder: None,
                help_text: Some("전일 변동폭 × K 만큼 상승 시 매수 (Larry Williams)".to_string()),
                validation: UiValidation {
                    required: true,
                    min: Some(0.1),
                    max: Some(1.0),
                    step: Some(0.1),
                    ..Default::default()
                },
                options: None,
                group: Some("breakout".to_string()),
                order: 2,
                show_when: None,
                unit: None,
                symbol_categories: None,
            },
            UiField {
                key: "amount".to_string(),
                label: "주문 금액".to_string(),
                field_type: UiFieldType::Number,
                default_value: Some(serde_json::json!(100000)),
                placeholder: None,
                help_text: None,
                validation: UiValidation { required: true, min: Some(10000.0), step: Some(10000.0), ..Default::default() },
                options: None,
                group: Some("risk".to_string()),
                order: 3,
                show_when: None,
                unit: Some("원".to_string()),
                symbol_categories: None,
            },
        ],
        groups: vec![
            UiFieldGroup { id: "basic".to_string(), label: "기본 설정".to_string(), description: None, order: 1, collapsed: false },
            UiFieldGroup { id: "breakout".to_string(), label: "돌파 설정".to_string(), description: None, order: 2, collapsed: false },
            UiFieldGroup { id: "risk".to_string(), label: "리스크 관리".to_string(), description: None, order: 3, collapsed: true },
        ],
        layout: Some(UiLayout { columns: 2 }),
    }
}

/// SMA 크로스오버 전략 UI 스키마
fn build_sma_crossover_ui_schema() -> UiSchema {
    UiSchema {
        fields: vec![
            UiField {
                key: "symbol".to_string(),
                label: "종목".to_string(),
                field_type: UiFieldType::SymbolPicker,
                default_value: Some(serde_json::json!(["005930"])),
                placeholder: None,
                help_text: None,
                validation: UiValidation { required: true, min_items: Some(1), max_items: Some(1), ..Default::default() },
                options: None,
                group: Some("basic".to_string()),
                order: 1,
                show_when: None,
                unit: None,
                symbol_categories: None,
            },
            UiField {
                key: "short_period".to_string(),
                label: "단기 이평선".to_string(),
                field_type: UiFieldType::Number,
                default_value: Some(serde_json::json!(10)),
                placeholder: None,
                help_text: Some("빠른 이동평균 기간".to_string()),
                validation: UiValidation { required: true, min: Some(5.0), max: Some(50.0), step: Some(1.0), ..Default::default() },
                options: None,
                group: Some("ma".to_string()),
                order: 2,
                show_when: None,
                unit: Some("일".to_string()),
                symbol_categories: None,
            },
            UiField {
                key: "long_period".to_string(),
                label: "장기 이평선".to_string(),
                field_type: UiFieldType::Number,
                default_value: Some(serde_json::json!(20)),
                placeholder: None,
                help_text: Some("느린 이동평균 기간".to_string()),
                validation: UiValidation { required: true, min: Some(10.0), max: Some(200.0), step: Some(5.0), ..Default::default() },
                options: None,
                group: Some("ma".to_string()),
                order: 3,
                show_when: None,
                unit: Some("일".to_string()),
                symbol_categories: None,
            },
            UiField {
                key: "amount".to_string(),
                label: "주문 금액".to_string(),
                field_type: UiFieldType::Number,
                default_value: Some(serde_json::json!(100000)),
                placeholder: None,
                help_text: None,
                validation: UiValidation { required: true, min: Some(10000.0), step: Some(10000.0), ..Default::default() },
                options: None,
                group: Some("risk".to_string()),
                order: 4,
                show_when: None,
                unit: Some("원".to_string()),
                symbol_categories: None,
            },
        ],
        groups: vec![
            UiFieldGroup { id: "basic".to_string(), label: "기본 설정".to_string(), description: None, order: 1, collapsed: false },
            UiFieldGroup { id: "ma".to_string(), label: "이동평균 설정".to_string(), description: None, order: 2, collapsed: false },
            UiFieldGroup { id: "risk".to_string(), label: "리스크 관리".to_string(), description: None, order: 3, collapsed: true },
        ],
        layout: Some(UiLayout { columns: 2 }),
    }
}

/// 전략 ID로 UI 스키마 조회
fn get_ui_schema_for_strategy(strategy_id: &str) -> Option<UiSchema> {
    match strategy_id {
        "rsi_mean_reversion" => Some(build_rsi_ui_schema()),
        "grid_trading" => Some(build_grid_ui_schema()),
        "simple_power" => Some(build_simple_power_ui_schema()),
        "haa" => Some(build_haa_ui_schema()),
        "magic_split" => Some(build_magic_split_ui_schema()),
        "bollinger" => Some(build_bollinger_ui_schema()),
        "volatility_breakout" => Some(build_volatility_breakout_ui_schema()),
        "sma_crossover" => Some(build_sma_crossover_ui_schema()),
        "xaa" => Some(build_haa_ui_schema()), // XAA는 HAA와 유사
        "stock_rotation" => Some(build_simple_power_ui_schema()), // Stock Rotation은 Simple Power와 유사
        _ => None,
    }
}

// ==================== 핸들러 ====================

/// 백테스트 가능한 전략 목록 조회
///
/// GET /api/v1/backtest/strategies
///
/// 현재 등록된 모든 전략 중 백테스트가 가능한 전략 목록을 반환합니다.
pub async fn list_backtest_strategies(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let engine = state.strategy_engine.read().await;
    let all_statuses = engine.get_all_statuses().await;

    // 등록된 전략을 백테스트 가능 전략으로 변환
    let strategies: Vec<BacktestableStrategy> = all_statuses
        .into_iter()
        .map(|(id, status)| {
            let ui_schema = get_ui_schema_for_strategy(&id);
            BacktestableStrategy {
                id: id.clone(),
                name: status.name,
                description: format!("버전 {}", status.version),
                supported_symbols: vec!["BTC/USDT".to_string(), "ETH/USDT".to_string()],
                default_params: serde_json::json!({
                    "period": 14,
                    "threshold": 30.0
                }),
                ui_schema,
                category: Some("사용자정의".to_string()),
                tags: vec!["플러그인".to_string()],
                execution_schedule: None,
                schedule_detail: None,
                how_it_works: None,
            }
        })
        .collect();

    // 기본 내장 전략 추가 (전략 엔진에 등록되지 않은 경우)
    let mut all_strategies = strategies;

    // 구현된 모든 전략 목록 (SDUI 스키마 포함)
    let builtin_strategies = vec![
        BacktestableStrategy {
            id: "rsi_mean_reversion".to_string(),
            name: "RSI 평균회귀".to_string(),
            description: "RSI 과매수/과매도 기반 평균회귀 전략".to_string(),
            supported_symbols: vec!["005930".to_string(), "SPY".to_string()],
            default_params: serde_json::json!({
                "period": 14,
                "oversold": 30,
                "overbought": 70
            }),
            ui_schema: get_ui_schema_for_strategy("rsi_mean_reversion"),
            category: Some("평균회귀".to_string()),
            tags: vec!["RSI".to_string(), "기술적지표".to_string(), "단일종목".to_string()],
            execution_schedule: Some(ExecutionSchedule::OnCandleClose),
            schedule_detail: Some("캔들 완성 시마다 실행".to_string()),
            how_it_works: Some("RSI가 과매도(30 이하) 구간에서 매수, 과매수(70 이상) 구간에서 매도합니다. Wilder's 스무딩을 사용하며, 쿨다운 기간 동안 추가 신호를 무시합니다.".to_string()),
        },
        BacktestableStrategy {
            id: "grid_trading".to_string(),
            name: "그리드 트레이딩".to_string(),
            description: "일정 간격 그리드 매수/매도 전략. 횡보장에 적합".to_string(),
            supported_symbols: vec!["005930".to_string(), "SPY".to_string()],
            default_params: serde_json::json!({
                "grid_spacing_pct": 1.0,
                "grid_levels": 10,
                "amount_per_level": 100000
            }),
            ui_schema: get_ui_schema_for_strategy("grid_trading"),
            category: Some("그리드".to_string()),
            tags: vec!["그리드".to_string(), "횡보장".to_string(), "단일종목".to_string()],
            execution_schedule: Some(ExecutionSchedule::Realtime),
            schedule_detail: Some("가격 변동 시마다 실행".to_string()),
            how_it_works: Some("현재가 기준으로 상하 그리드 레벨을 설정하고, 가격이 하락하면 매수, 상승하면 매도합니다. ATR 기반 동적 간격 및 추세 필터 옵션을 지원합니다.".to_string()),
        },
        BacktestableStrategy {
            id: "bollinger".to_string(),
            name: "볼린저 밴드".to_string(),
            description: "동적 변동성 밴드를 사용한 평균 회귀 전략".to_string(),
            supported_symbols: vec!["005930".to_string(), "SPY".to_string()],
            default_params: serde_json::json!({
                "period": 20,
                "std_dev": 2.0
            }),
            ui_schema: get_ui_schema_for_strategy("bollinger"),
            category: Some("평균회귀".to_string()),
            tags: vec!["볼린저".to_string(), "변동성".to_string(), "단일종목".to_string()],
            execution_schedule: Some(ExecutionSchedule::OnCandleClose),
            schedule_detail: Some("캔들 완성 시마다 실행".to_string()),
            how_it_works: Some("20일 이동평균과 표준편차로 상/하단 밴드를 계산합니다. 가격이 하단 밴드 터치 시 매수, 상단 밴드 터치 시 매도합니다. RSI 확인 옵션으로 거짓 신호를 필터링할 수 있습니다.".to_string()),
        },
        BacktestableStrategy {
            id: "volatility_breakout".to_string(),
            name: "변동성 돌파".to_string(),
            description: "Larry Williams 모멘텀 전략. 추세장에 적합".to_string(),
            supported_symbols: vec!["005930".to_string(), "SPY".to_string()],
            default_params: serde_json::json!({
                "k_factor": 0.5
            }),
            ui_schema: get_ui_schema_for_strategy("volatility_breakout"),
            category: Some("추세추종".to_string()),
            tags: vec!["돌파".to_string(), "모멘텀".to_string(), "단일종목".to_string()],
            execution_schedule: Some(ExecutionSchedule::Daily),
            schedule_detail: Some("장 시작 5분 후 실행".to_string()),
            how_it_works: Some("전일 변동폭(고가-저가)에 K값(0.5)을 곱한 값을 당일 시가에 더해 목표가를 설정합니다. 가격이 목표가를 돌파하면 매수하고, 장 마감 시 청산합니다.".to_string()),
        },
        BacktestableStrategy {
            id: "magic_split".to_string(),
            name: "Magic Split (분할 매수)".to_string(),
            description: "레벨 기반 수익 실현과 함께하는 체계적 물타기 전략".to_string(),
            supported_symbols: vec!["305540".to_string(), "QQQ".to_string()],
            default_params: serde_json::json!({
                "max_levels": 10,
                "level_spacing_pct": 3.0
            }),
            ui_schema: get_ui_schema_for_strategy("magic_split"),
            category: Some("분할매매".to_string()),
            tags: vec!["분할매수".to_string(), "물타기".to_string(), "단일종목".to_string()],
            execution_schedule: Some(ExecutionSchedule::Realtime),
            schedule_detail: Some("가격 변동 시마다 실행".to_string()),
            how_it_works: Some("10차수 분할매수 전략입니다. 1차: 무조건 진입(10% 익절), 2~10차: 하락 시 추가 매수. 각 차수별 익절가 도달 시 해당 차수만 매도하고, 모든 차수 청산 후 1차수부터 재시작합니다.".to_string()),
        },
        BacktestableStrategy {
            id: "simple_power".to_string(),
            name: "Simple Power".to_string(),
            description: "TQQQ/SCHD/BIL 모멘텀 자산 배분 전략".to_string(),
            supported_symbols: vec!["TQQQ".to_string(), "SCHD".to_string()],
            default_params: serde_json::json!({
                "ma_period": 130
            }),
            ui_schema: get_ui_schema_for_strategy("simple_power"),
            category: Some("자산배분".to_string()),
            tags: vec!["자산배분".to_string(), "모멘텀".to_string(), "다중자산".to_string(), "미국ETF".to_string()],
            execution_schedule: Some(ExecutionSchedule::Monthly),
            schedule_detail: Some("매월 첫 거래일 리밸런싱".to_string()),
            how_it_works: Some("TQQQ(50%), SCHD(20%), PFIX(15%), TMF(15%) 기본 비중으로 투자합니다. MA130 필터를 적용하여 가격이 이동평균 하회 시 비중을 50% 감소시킵니다.".to_string()),
        },
        BacktestableStrategy {
            id: "haa".to_string(),
            name: "HAA (계층적 자산 배분)".to_string(),
            description: "카나리아 자산 기반 위험 감지를 포함한 자산 배분".to_string(),
            supported_symbols: vec!["SPY".to_string(), "TLT".to_string(), "VEA".to_string()],
            default_params: serde_json::json!({
                "momentum_period": 12
            }),
            ui_schema: get_ui_schema_for_strategy("haa"),
            category: Some("자산배분".to_string()),
            tags: vec!["자산배분".to_string(), "카나리아".to_string(), "다중자산".to_string(), "미국ETF".to_string()],
            execution_schedule: Some(ExecutionSchedule::Monthly),
            schedule_detail: Some("매월 첫 거래일 리밸런싱".to_string()),
            how_it_works: Some("TIP(카나리아 자산) 모멘텀이 양수면 공격자산(SPY, IWM, VEA 등) TOP 4에 투자하고, 음수면 방어자산(IEF, BIL)으로 전환합니다. 1M/3M/6M/12M 모멘텀 평균을 사용합니다.".to_string()),
        },
        BacktestableStrategy {
            id: "xaa".to_string(),
            name: "XAA (확장 자산 배분)".to_string(),
            description: "HAA 확장 버전. 더 많은 자산군 지원".to_string(),
            supported_symbols: vec!["SPY".to_string(), "QQQ".to_string(), "TLT".to_string()],
            default_params: serde_json::json!({
                "top_n": 4
            }),
            ui_schema: get_ui_schema_for_strategy("xaa"),
            category: Some("자산배분".to_string()),
            tags: vec!["자산배분".to_string(), "확장".to_string(), "다중자산".to_string(), "미국ETF".to_string()],
            execution_schedule: Some(ExecutionSchedule::Monthly),
            schedule_detail: Some("매월 첫 거래일 리밸런싱".to_string()),
            how_it_works: Some("VWO, BND 카나리아 자산 기반으로 공격(SPY, EFA, EEM 등 TOP 4), 채권(TLT, IEF, LQD TOP 2), 안전(BIL TOP 1) 자산에 동적 배분합니다.".to_string()),
        },
        BacktestableStrategy {
            id: "stock_rotation".to_string(),
            name: "종목 갈아타기".to_string(),
            description: "모멘텀 기반 종목 순환 투자 전략".to_string(),
            supported_symbols: vec!["005930".to_string(), "000660".to_string()],
            default_params: serde_json::json!({
                "rotation_period": 20
            }),
            ui_schema: get_ui_schema_for_strategy("stock_rotation"),
            category: Some("모멘텀".to_string()),
            tags: vec!["모멘텀".to_string(), "순환".to_string(), "다중종목".to_string(), "한국주식".to_string()],
            execution_schedule: Some(ExecutionSchedule::Daily),
            schedule_detail: Some("매일 또는 매주 리밸런싱".to_string()),
            how_it_works: Some("후보 종목들의 모멘텀 스코어를 계산하여 상위 N개 종목에 투자합니다. 모멘텀이 음수인 종목은 제외하고, 현금 보유 비율을 조절할 수 있습니다.".to_string()),
        },
        BacktestableStrategy {
            id: "sma_crossover".to_string(),
            name: "이동평균 크로스오버".to_string(),
            description: "단기/장기 이동평균 교차 전략".to_string(),
            supported_symbols: vec!["005930".to_string(), "SPY".to_string()],
            default_params: serde_json::json!({
                "short_period": 10,
                "long_period": 20
            }),
            ui_schema: get_ui_schema_for_strategy("sma_crossover"),
            category: Some("추세추종".to_string()),
            tags: vec!["이동평균".to_string(), "크로스오버".to_string(), "단일종목".to_string()],
            execution_schedule: Some(ExecutionSchedule::OnCandleClose),
            schedule_detail: Some("캔들 완성 시마다 실행".to_string()),
            how_it_works: Some("단기 이동평균(10일)이 장기 이동평균(20일)을 상향 돌파하면 매수(골든크로스), 하향 돌파하면 매도(데드크로스)합니다.".to_string()),
        },
        // ===== 신규 전략들 =====
        BacktestableStrategy {
            id: "trailing_stop".to_string(),
            name: "트레일링 스톱".to_string(),
            description: "고점 대비 하락 시 자동 청산하는 트레일링 스톱 전략".to_string(),
            supported_symbols: vec!["005930".to_string(), "SPY".to_string(), "BTCUSDT".to_string()],
            default_params: serde_json::json!({
                "trailing_stop_pct": 5.0,
                "max_trailing_stop_pct": 10.0,
                "profit_rate_adjustment": 2.0
            }),
            ui_schema: get_ui_schema_for_strategy("trailing_stop"),
            category: Some("리스크관리".to_string()),
            tags: vec!["트레일링".to_string(), "스톱".to_string(), "리스크관리".to_string()],
            execution_schedule: Some(ExecutionSchedule::Realtime),
            schedule_detail: Some("가격 변동 시마다 실행".to_string()),
            how_it_works: Some("초기 트레일링 스톱 5%에서 시작하여 수익률 증가에 따라 최대 10%까지 조정됩니다. 고점 대비 설정된 비율 이상 하락 시 자동 청산합니다.".to_string()),
        },
        BacktestableStrategy {
            id: "all_weather_us".to_string(),
            name: "올웨더 US".to_string(),
            description: "레이 달리오 올웨더 포트폴리오 (미국 ETF)".to_string(),
            supported_symbols: vec!["SPY".to_string(), "TLT".to_string(), "IEF".to_string(), "GLD".to_string()],
            default_params: serde_json::json!({
                "market": "US",
                "use_seasonality": true,
                "ma_periods": [50, 80, 120, 150],
                "rebalance_days": 30
            }),
            ui_schema: get_ui_schema_for_strategy("all_weather"),
            category: Some("자산배분".to_string()),
            tags: vec!["자산배분".to_string(), "올웨더".to_string(), "다중자산".to_string(), "미국ETF".to_string()],
            execution_schedule: Some(ExecutionSchedule::Monthly),
            schedule_detail: Some("매월 첫 거래일 리밸런싱".to_string()),
            how_it_works: Some("SPY(20%), TLT(27%), IEF(15%), GLD(8%), PDBC(8%), IYK(22%) 기본 비중. 5~10월 지옥기간에는 주식 비중 감소, 채권 비중 증가. MA 필터로 추가 조정합니다.".to_string()),
        },
        BacktestableStrategy {
            id: "all_weather_kr".to_string(),
            name: "올웨더 KR".to_string(),
            description: "한국형 올웨더 포트폴리오 (국내 ETF)".to_string(),
            supported_symbols: vec!["360750".to_string(), "294400".to_string(), "148070".to_string()],
            default_params: serde_json::json!({
                "market": "KR",
                "use_seasonality": true,
                "ma_periods": [50, 80, 120, 150],
                "rebalance_days": 30
            }),
            ui_schema: get_ui_schema_for_strategy("all_weather"),
            category: Some("자산배분".to_string()),
            tags: vec!["자산배분".to_string(), "올웨더".to_string(), "다중자산".to_string(), "한국ETF".to_string()],
            execution_schedule: Some(ExecutionSchedule::Monthly),
            schedule_detail: Some("매월 첫 거래일 리밸런싱".to_string()),
            how_it_works: Some("한국 ETF를 활용한 올웨더 포트폴리오입니다. 계절성 조정과 MA 필터를 적용하여 주식/채권 비중을 동적으로 조절합니다.".to_string()),
        },
        BacktestableStrategy {
            id: "snow_us".to_string(),
            name: "스노우 US".to_string(),
            description: "TIP 기반 모멘텀 전략 (UPRO/TLT/BIL)".to_string(),
            supported_symbols: vec!["UPRO".to_string(), "TLT".to_string(), "TIP".to_string()],
            default_params: serde_json::json!({
                "market": "US",
                "tip_ma_period": 200,
                "attack_ma_period": 5,
                "rebalance_days": 1
            }),
            ui_schema: get_ui_schema_for_strategy("snow"),
            category: Some("자산배분".to_string()),
            tags: vec!["모멘텀".to_string(), "자산배분".to_string(), "미국ETF".to_string()],
            execution_schedule: Some(ExecutionSchedule::Daily),
            schedule_detail: Some("장 마감 후 실행".to_string()),
            how_it_works: Some("TIP 10개월 이동평균 기준으로 모멘텀 판단. TIP 모멘텀 양수면 UPRO(3x S&P 500), 음수면 TLT(20년 국채) 또는 BIL(단기 국채)에 투자합니다.".to_string()),
        },
        BacktestableStrategy {
            id: "snow_kr".to_string(),
            name: "스노우 KR".to_string(),
            description: "TIP 기반 모멘텀 전략 (레버리지/국채)".to_string(),
            supported_symbols: vec!["122630".to_string(), "148070".to_string()],
            default_params: serde_json::json!({
                "market": "KR",
                "tip_ma_period": 200,
                "attack_ma_period": 5,
                "rebalance_days": 1
            }),
            ui_schema: get_ui_schema_for_strategy("snow"),
            category: Some("자산배분".to_string()),
            tags: vec!["모멘텀".to_string(), "자산배분".to_string(), "한국ETF".to_string()],
            execution_schedule: Some(ExecutionSchedule::Daily),
            schedule_detail: Some("장 마감 후 실행".to_string()),
            how_it_works: Some("한국형 스노우 전략입니다. TIP 모멘텀 기준으로 KODEX 레버리지(122630) 또는 국고채 10년 ETF(148070)에 투자합니다.".to_string()),
        },
        BacktestableStrategy {
            id: "market_cap_top".to_string(),
            name: "시총 TOP".to_string(),
            description: "미국 시총 상위 종목 투자 전략".to_string(),
            supported_symbols: vec!["AAPL".to_string(), "MSFT".to_string(), "GOOGL".to_string()],
            default_params: serde_json::json!({
                "top_n": 10,
                "weighting_method": "Equal",
                "rebalance_days": 30,
                "use_momentum_filter": false
            }),
            ui_schema: get_ui_schema_for_strategy("market_cap_top"),
            category: Some("패시브".to_string()),
            tags: vec!["시총".to_string(), "패시브".to_string(), "다중종목".to_string(), "미국주식".to_string()],
            execution_schedule: Some(ExecutionSchedule::Monthly),
            schedule_detail: Some("매월 말 리밸런싱".to_string()),
            how_it_works: Some("미국 시총 상위 10개 종목에 동일 비중 또는 시총 비중으로 투자합니다. 모멘텀 필터 옵션으로 하락 종목을 제외할 수 있습니다.".to_string()),
        },
        BacktestableStrategy {
            id: "candle_pattern".to_string(),
            name: "캔들 패턴".to_string(),
            description: "35가지 캔들스틱 패턴 인식 전략".to_string(),
            supported_symbols: vec!["005930".to_string(), "BTCUSDT".to_string()],
            default_params: serde_json::json!({
                "min_pattern_strength": 0.6,
                "use_volume_confirmation": true,
                "use_trend_confirmation": true,
                "stop_loss_pct": 3.0,
                "take_profit_pct": 6.0
            }),
            ui_schema: get_ui_schema_for_strategy("candle_pattern"),
            category: Some("기술적분석".to_string()),
            tags: vec!["캔들".to_string(), "패턴인식".to_string(), "단일종목".to_string()],
            execution_schedule: Some(ExecutionSchedule::OnCandleClose),
            schedule_detail: Some("캔들 완성 시마다 실행".to_string()),
            how_it_works: Some("35가지 캔들스틱 패턴(해머, 도지, 인걸핑 등)을 인식합니다. 거래량 확인과 추세 확인 옵션으로 신호 정확도를 높일 수 있습니다.".to_string()),
        },
        BacktestableStrategy {
            id: "infinity_bot".to_string(),
            name: "무한매수봇".to_string(),
            description: "50라운드 피라미드 물타기 + 트레일링 스톱 전략".to_string(),
            supported_symbols: vec!["005930".to_string(), "BTCUSDT".to_string()],
            default_params: serde_json::json!({
                "max_rounds": 50,
                "round_amount_pct": 2.0,
                "dip_trigger_pct": 2.0,
                "take_profit_pct": 3.0,
                "short_ma_period": 10,
                "mid_ma_period": 100,
                "long_ma_period": 200
            }),
            ui_schema: get_ui_schema_for_strategy("infinity_bot"),
            category: Some("분할매매".to_string()),
            tags: vec!["물타기".to_string(), "분할매수".to_string(), "단일종목".to_string()],
            execution_schedule: Some(ExecutionSchedule::Realtime),
            schedule_detail: Some("가격 변동 시마다 실행".to_string()),
            how_it_works: Some("최대 50라운드까지 물타기합니다. 1-5라운드: 무조건 매수, 6-20라운드: MA 확인, 21-30라운드: MA+양봉 확인. 트레일링 스톱 5%→10%로 익절 관리합니다.".to_string()),
        },
        BacktestableStrategy {
            id: "market_interest_day".to_string(),
            name: "시장관심 단타".to_string(),
            description: "거래량 급증 종목 단기 모멘텀 트레이딩".to_string(),
            supported_symbols: vec!["005930".to_string(), "BTCUSDT".to_string()],
            default_params: serde_json::json!({
                "volume_multiplier": 2.0,
                "consecutive_up_candles": 3,
                "trailing_stop_pct": 1.5,
                "take_profit_pct": 3.0,
                "stop_loss_pct": 2.0,
                "max_hold_minutes": 120
            }),
            ui_schema: get_ui_schema_for_strategy("market_interest_day"),
            category: Some("단타".to_string()),
            tags: vec!["거래량".to_string(), "모멘텀".to_string(), "단타".to_string(), "단일종목".to_string()],
            execution_schedule: Some(ExecutionSchedule::Daily),
            schedule_detail: Some("장 시작 직후 실행".to_string()),
            how_it_works: Some("거래량이 평균 대비 2배 이상 급증하고, 연속 상승봉이 나타나면 진입합니다. 트레일링 스톱으로 수익 보호하고, 최대 120분 보유 후 청산합니다.".to_string()),
        },
    ];

    // 아직 추가되지 않은 전략만 추가
    for strategy in builtin_strategies {
        if !all_strategies.iter().any(|s| s.id == strategy.id) {
            all_strategies.push(strategy);
        }
    }

    let total = all_strategies.len();

    Json(BacktestStrategiesResponse {
        strategies: all_strategies,
        total,
    })
}

/// 백테스트 실행
///
/// POST /api/v1/backtest/run
///
/// 주어진 설정으로 백테스트를 실행하고 결과를 반환합니다.
pub async fn run_backtest(
    State(state): State<Arc<AppState>>,
    Json(request): Json<BacktestRunRequest>,
) -> Result<Json<BacktestRunResponse>, (StatusCode, Json<BacktestApiError>)> {
    info!("백테스트 실행 요청: strategy={}, symbol={}", request.strategy_id, request.symbol);

    // 날짜 파싱 검증
    let start_date = NaiveDate::parse_from_str(&request.start_date, "%Y-%m-%d")
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(BacktestApiError::new(
                    "INVALID_DATE",
                    format!("잘못된 시작 날짜 형식: {}", request.start_date),
                )),
            )
        })?;

    let end_date = NaiveDate::parse_from_str(&request.end_date, "%Y-%m-%d")
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(BacktestApiError::new(
                    "INVALID_DATE",
                    format!("잘못된 종료 날짜 형식: {}", request.end_date),
                )),
            )
        })?;

    // 날짜 유효성 검사
    if end_date <= start_date {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new(
                "INVALID_DATE_RANGE",
                "종료 날짜는 시작 날짜보다 이후여야 합니다",
            )),
        ));
    }

    // 초기 자본금 검증
    if request.initial_capital <= Decimal::ZERO {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new(
                "INVALID_CAPITAL",
                "초기 자본금은 0보다 커야 합니다",
            )),
        ));
    }

    // 유효한 전략 ID 목록
    let valid_strategies = [
        "sma_crossover", "rsi_mean_reversion", "grid_trading",
        "bollinger", "volatility_breakout", "magic_split",
        "simple_power", "haa", "xaa", "stock_rotation",
        // 신규 전략
        "trailing_stop", "all_weather_us", "all_weather_kr",
        "snow_us", "snow_kr", "market_cap_top",
        "candle_pattern", "infinity_bot", "market_interest_day",
    ];
    if !valid_strategies.contains(&request.strategy_id.as_str()) {
        return Err((
            StatusCode::NOT_FOUND,
            Json(BacktestApiError::new(
                "STRATEGY_NOT_FOUND",
                format!("전략을 찾을 수 없습니다: {}", request.strategy_id),
            )),
        ));
    }

    // 수수료/슬리피지 기본값 설정
    let commission_rate = request.commission_rate.unwrap_or(Decimal::new(1, 3)); // 0.1%
    let slippage_rate = request.slippage_rate.unwrap_or(Decimal::new(5, 4)); // 0.05%

    // 데이터베이스 연결 확인 및 Kline 로드 시도
    let klines = if let Some(pool) = &state.db_pool {
        match load_klines_from_db(pool, &request.symbol, start_date, end_date).await {
            Ok(data) if !data.is_empty() => {
                info!("DB에서 {} 개의 캔들 데이터 로드 완료", data.len());
                data
            }
            Ok(_) => {
                warn!("DB에 데이터가 없어 샘플 데이터로 백테스트 실행");
                generate_sample_klines(&request.symbol, start_date, end_date)
            }
            Err(e) => {
                warn!("DB 로드 실패, 샘플 데이터 사용: {}", e);
                generate_sample_klines(&request.symbol, start_date, end_date)
            }
        }
    } else {
        debug!("DB 연결 없음, 샘플 데이터로 백테스트 실행");
        generate_sample_klines(&request.symbol, start_date, end_date)
    };

    // 백테스트 설정
    let config = BacktestConfig::new(request.initial_capital)
        .with_commission_rate(commission_rate)
        .with_slippage_rate(slippage_rate);

    // 전략별 백테스트 실행
    let report = run_strategy_backtest(&request.strategy_id, config, &klines, &request.parameters)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(BacktestApiError::new("BACKTEST_ERROR", e.to_string())),
            )
        })?;

    // BacktestReport를 API 응답으로 변환
    let response = convert_report_to_response(
        &report,
        &request.strategy_id,
        &request.symbol,
        &request.start_date,
        &request.end_date,
    );

    info!("백테스트 완료: total_return={:.2}%", report.metrics.total_return_pct);

    Ok(Json(response))
}

/// 전략별 백테스트 실행
async fn run_strategy_backtest(
    strategy_id: &str,
    config: BacktestConfig,
    klines: &[Kline],
    params: &Option<serde_json::Value>,
) -> Result<BacktestReport, String> {
    let mut engine = BacktestEngine::new(config);

    // 심볼 추출 (klines에서)
    let symbol_str = if let Some(first_kline) = klines.first() {
        first_kline.symbol.to_string()
    } else {
        "BTC/USDT".to_string()
    };

    // 기본 전략 설정 생성
    let _default_config = |strategy_symbol: &str| -> serde_json::Value {
        serde_json::json!({
            "symbol": strategy_symbol,
            "amount": "100000"
        })
    };

    // 사용자 파라미터와 기본 설정 병합
    let merge_params = |default: serde_json::Value, user_params: &Option<serde_json::Value>| -> serde_json::Value {
        if let Some(user) = user_params {
            if let (Some(default_obj), Some(user_obj)) = (default.as_object(), user.as_object()) {
                let mut merged = default_obj.clone();
                for (key, value) in user_obj {
                    merged.insert(key.clone(), value.clone());
                }
                return serde_json::Value::Object(merged);
            }
        }
        default
    };

    match strategy_id {
        "rsi_mean_reversion" | "rsi" => {
            let mut strategy = RsiStrategy::new();
            let strategy_config = merge_params(
                serde_json::json!({
                    "symbol": symbol_str,
                    "period": 14,
                    "oversold_threshold": 30.0,
                    "overbought_threshold": 70.0,
                    "amount": "100000"
                }),
                params,
            );
            strategy.initialize(strategy_config).await.map_err(|e| e.to_string())?;
            engine.run(&mut strategy, klines).await.map_err(|e| e.to_string())
        }
        "grid_trading" | "grid" => {
            let mut strategy = GridStrategy::new();
            let strategy_config = merge_params(
                serde_json::json!({
                    "symbol": symbol_str,
                    "grid_spacing_pct": 1.0,
                    "grid_levels": 10,
                    "amount_per_level": "100000"
                }),
                params,
            );
            strategy.initialize(strategy_config).await.map_err(|e| e.to_string())?;
            engine.run(&mut strategy, klines).await.map_err(|e| e.to_string())
        }
        "bollinger" => {
            let mut strategy = BollingerStrategy::new();
            let strategy_config = merge_params(
                serde_json::json!({
                    "symbol": symbol_str,
                    "period": 20,
                    "std_multiplier": 1.5,
                    "use_rsi_confirmation": false,
                    "min_bandwidth_pct": 0.0,
                    "amount": "100000"
                }),
                params,
            );
            strategy.initialize(strategy_config).await.map_err(|e| e.to_string())?;
            engine.run(&mut strategy, klines).await.map_err(|e| e.to_string())
        }
        "volatility_breakout" => {
            let mut strategy = VolatilityBreakoutStrategy::new();
            let strategy_config = merge_params(
                serde_json::json!({
                    "symbol": symbol_str,
                    "k_factor": 0.3,
                    "lookback_period": 1,
                    "use_atr": true,
                    "atr_period": 5,
                    "min_range_pct": 0.1,
                    "amount": "100000"
                }),
                params,
            );
            strategy.initialize(strategy_config).await.map_err(|e| e.to_string())?;
            engine.run(&mut strategy, klines).await.map_err(|e| e.to_string())
        }
        "magic_split" => {
            let mut strategy = MagicSplitStrategy::new();
            let strategy_config = merge_params(
                serde_json::json!({
                    "symbol": symbol_str,
                    "levels": [
                        {"number": 1, "target_rate": "10.0", "trigger_rate": null, "invest_money": "200000"},
                        {"number": 2, "target_rate": "2.0", "trigger_rate": "-3.0", "invest_money": "100000"},
                        {"number": 3, "target_rate": "3.0", "trigger_rate": "-5.0", "invest_money": "100000"},
                        {"number": 4, "target_rate": "3.0", "trigger_rate": "-5.0", "invest_money": "100000"},
                        {"number": 5, "target_rate": "4.0", "trigger_rate": "-6.0", "invest_money": "100000"}
                    ],
                    "allow_same_day_reentry": false,
                    "slippage_tolerance": "1.0"
                }),
                params,
            );
            strategy.initialize(strategy_config).await.map_err(|e| e.to_string())?;
            engine.run(&mut strategy, klines).await.map_err(|e| e.to_string())
        }
        "simple_power" => {
            let mut strategy = SimplePowerStrategy::new();
            let default_cfg = serde_json::to_value(SimplePowerConfig::default())
                .map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy.initialize(strategy_config).await.map_err(|e| e.to_string())?;
            engine.run(&mut strategy, klines).await.map_err(|e| e.to_string())
        }
        "haa" => {
            let mut strategy = HaaStrategy::new();
            let default_cfg = serde_json::to_value(HaaConfig::default())
                .map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy.initialize(strategy_config).await.map_err(|e| e.to_string())?;
            engine.run(&mut strategy, klines).await.map_err(|e| e.to_string())
        }
        "xaa" => {
            let mut strategy = XaaStrategy::new();
            let default_cfg = serde_json::to_value(XaaConfig::default())
                .map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy.initialize(strategy_config).await.map_err(|e| e.to_string())?;
            engine.run(&mut strategy, klines).await.map_err(|e| e.to_string())
        }
        "stock_rotation" => {
            let mut strategy = StockRotationStrategy::new();
            let default_cfg = serde_json::to_value(StockRotationConfig::default())
                .map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy.initialize(strategy_config).await.map_err(|e| e.to_string())?;
            engine.run(&mut strategy, klines).await.map_err(|e| e.to_string())
        }
        "sma_crossover" => {
            // SMA 크로스오버 전략
            let mut strategy = SmaStrategy::new();
            let strategy_config = merge_params(
                serde_json::json!({
                    "symbol": symbol_str,
                    "short_period": 10,
                    "long_period": 20,
                    "amount": "100000"
                }),
                params,
            );
            strategy.initialize(strategy_config).await.map_err(|e| e.to_string())?;
            engine.run(&mut strategy, klines).await.map_err(|e| e.to_string())
        }
        _ => {
            return Err(format!("지원하지 않는 전략입니다: {}", strategy_id));
        }
    }
}

/// DB에서 Kline 데이터 로드
async fn load_klines_from_db(
    pool: &sqlx::PgPool,
    symbol_str: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Result<Vec<Kline>, String> {
    // 심볼 파싱 (예: "005930" -> base="005930", quote="KRW")
    let (base, quote) = if symbol_str.contains('/') {
        let parts: Vec<&str> = symbol_str.split('/').collect();
        (parts[0].to_string(), parts.get(1).map(|s| s.to_string()).unwrap_or("KRW".to_string()))
    } else if symbol_str.chars().all(|c| c.is_ascii_digit()) {
        // 한국 종목코드
        (symbol_str.to_string(), "KRW".to_string())
    } else {
        // 미국 심볼
        (symbol_str.to_string(), "USD".to_string())
    };

    let symbol = Symbol {
        base: base.clone(),
        quote: quote.clone(),
        market_type: MarketType::Stock,
        exchange_symbol: None,
    };

    let db = Database::from_pool(pool.clone());
    let symbol_repo = SymbolRepository::new(db.clone());
    let kline_repo = KlineRepository::new(db);

    // 심볼 조회 또는 생성
    let exchange = if quote == "KRW" { "KIS_KR" } else { "KIS_US" };
    let symbol_id = symbol_repo
        .get_or_create(&symbol, exchange)
        .await
        .map_err(|e| format!("심볼 조회 실패: {}", e))?;

    // 날짜를 DateTime으로 변환
    let start = Utc.from_utc_datetime(&start_date.and_hms_opt(0, 0, 0).unwrap());
    let end = Utc.from_utc_datetime(&end_date.and_hms_opt(23, 59, 59).unwrap());

    // Kline 조회
    let rows = kline_repo
        .get_range(symbol_id, Timeframe::D1, start, end, Some(10000))
        .await
        .map_err(|e| format!("Kline 조회 실패: {}", e))?;

    // DB 레코드를 Kline으로 변환
    let klines: Vec<Kline> = rows.into_iter().map(|r| r.to_kline(symbol.clone())).collect();

    // 데이터가 충분하지 않으면 Yahoo Finance에서 다운로드
    let expected_days = (end_date - start_date).num_days() as usize;
    let min_required = expected_days / 2; // 최소 절반의 데이터 필요

    if klines.len() < min_required {
        info!(
            "DB에 데이터가 부족합니다 ({} < {}). Yahoo Finance에서 다운로드합니다...",
            klines.len(),
            min_required
        );

        // Yahoo Finance에서 다운로드
        match download_from_yahoo(&base, &quote, start_date, end_date).await {
            Ok(downloaded) => {
                if !downloaded.is_empty() {
                    info!("Yahoo Finance에서 {} 캔들을 다운로드했습니다", downloaded.len());

                    // DB에 저장
                    if let Err(e) = kline_repo.insert_batch(symbol_id, &downloaded).await {
                        warn!("다운로드한 데이터 DB 저장 실패: {}", e);
                    } else {
                        info!("다운로드한 데이터를 DB에 저장했습니다");
                    }

                    return Ok(downloaded);
                }
            }
            Err(e) => {
                warn!("Yahoo Finance 다운로드 실패: {}", e);
            }
        }
    }

    Ok(klines)
}

/// Yahoo Finance에서 OHLCV 데이터 다운로드
async fn download_from_yahoo(
    base: &str,
    quote: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Result<Vec<Kline>, String> {
    // Yahoo Finance 심볼 변환
    let yahoo_symbol = if quote == "KRW" {
        // 한국 주식: 6자리 숫자 + .KS (코스피) 또는 .KQ (코스닥)
        // 기본적으로 .KS (코스피) 사용, 실패하면 .KQ 시도
        format!("{}.KS", base)
    } else {
        // 미국/암호화폐
        if quote == "USDT" {
            format!("{}-USD", base) // 암호화폐
        } else {
            base.to_uppercase() // 미국 주식
        }
    };

    // 타임스탬프 계산
    let start_ts = start_date
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();
    let end_ts = end_date
        .and_hms_opt(23, 59, 59)
        .unwrap()
        .and_utc()
        .timestamp();

    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{}?period1={}&period2={}&interval=1d",
        yahoo_symbol, start_ts, end_ts
    );

    debug!("Yahoo Finance에서 데이터 가져오기: {}", url);

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()
        .map_err(|e| format!("HTTP 클라이언트 생성 실패: {}", e))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Yahoo Finance 요청 실패: {}", e))?;

    if !response.status().is_success() {
        // 코스피에서 실패하면 코스닥으로 재시도
        if quote == "KRW" {
            let kosdaq_symbol = format!("{}.KQ", base);
            let kosdaq_url = format!(
                "https://query1.finance.yahoo.com/v8/finance/chart/{}?period1={}&period2={}&interval=1d",
                kosdaq_symbol, start_ts, end_ts
            );

            debug!("코스닥으로 재시도: {}", kosdaq_url);

            let response = client
                .get(&kosdaq_url)
                .send()
                .await
                .map_err(|e| format!("Yahoo Finance 요청 실패: {}", e))?;

            if !response.status().is_success() {
                return Err(format!("Yahoo Finance API 오류: {}", response.status()));
            }

            return parse_yahoo_response(response, base, quote).await;
        }

        return Err(format!("Yahoo Finance API 오류: {}", response.status()));
    }

    parse_yahoo_response(response, base, quote).await
}

/// Yahoo Finance 응답 파싱
async fn parse_yahoo_response(
    response: reqwest::Response,
    base: &str,
    quote: &str,
) -> Result<Vec<Kline>, String> {
    use rust_decimal::prelude::FromPrimitive;

    let data: YahooResponse = response
        .json()
        .await
        .map_err(|e| format!("Yahoo 응답 파싱 실패: {}", e))?;

    // 에러 체크
    if let Some(error) = data.chart.error {
        return Err(format!("Yahoo Finance 에러: {} - {}", error.code, error.description));
    }

    let results = data.chart.result.ok_or("결과 데이터 없음")?;
    let chart = results.first().ok_or("차트 데이터 없음")?;
    let timestamps = chart.timestamp.as_ref().ok_or("타임스탬프 없음")?;
    let quotes = chart.indicators.quote.first().ok_or("가격 데이터 없음")?;

    let symbol = Symbol {
        base: base.to_uppercase(),
        quote: quote.to_string(),
        market_type: MarketType::Stock,
        exchange_symbol: None,
    };

    let mut klines = Vec::with_capacity(timestamps.len());

    for i in 0..timestamps.len() {
        // null 값 스킵
        let open = match quotes.open.get(i).and_then(|v| *v) {
            Some(v) => Decimal::from_f64(v).unwrap_or(Decimal::ZERO),
            None => continue,
        };
        let high = match quotes.high.get(i).and_then(|v| *v) {
            Some(v) => Decimal::from_f64(v).unwrap_or(Decimal::ZERO),
            None => continue,
        };
        let low = match quotes.low.get(i).and_then(|v| *v) {
            Some(v) => Decimal::from_f64(v).unwrap_or(Decimal::ZERO),
            None => continue,
        };
        let close = match quotes.close.get(i).and_then(|v| *v) {
            Some(v) => Decimal::from_f64(v).unwrap_or(Decimal::ZERO),
            None => continue,
        };
        let volume = quotes
            .volume
            .get(i)
            .and_then(|v| *v)
            .map(|v| Decimal::from(v as i64))
            .unwrap_or(Decimal::ZERO);

        let open_time = Utc.timestamp_opt(timestamps[i], 0).unwrap();
        let close_time = open_time + chrono::Duration::days(1) - chrono::Duration::seconds(1);

        klines.push(Kline {
            symbol: symbol.clone(),
            timeframe: Timeframe::D1,
            open_time,
            close_time,
            open,
            high,
            low,
            close,
            volume,
            quote_volume: None,
            num_trades: None,
        });
    }

    Ok(klines)
}

/// 샘플 Kline 데이터 생성 (DB 데이터가 없을 경우 사용)
fn generate_sample_klines(symbol_str: &str, start_date: NaiveDate, end_date: NaiveDate) -> Vec<Kline> {
    use rust_decimal::prelude::FromPrimitive;

    let (base, quote) = if symbol_str.contains('/') {
        let parts: Vec<&str> = symbol_str.split('/').collect();
        (parts[0].to_string(), parts.get(1).map(|s| s.to_string()).unwrap_or("KRW".to_string()))
    } else if symbol_str.chars().all(|c| c.is_ascii_digit()) {
        (symbol_str.to_string(), "KRW".to_string())
    } else {
        (symbol_str.to_string(), "USD".to_string())
    };

    let symbol = Symbol {
        base,
        quote,
        market_type: MarketType::Stock,
        exchange_symbol: None,
    };

    let days = (end_date - start_date).num_days() as usize;
    let base_price = 50000.0_f64; // 기본 가격

    (0..=days)
        .map(|i| {
            let date = start_date + chrono::Duration::days(i as i64);
            let open_time = Utc.from_utc_datetime(&date.and_hms_opt(9, 0, 0).unwrap());
            let close_time = Utc.from_utc_datetime(&date.and_hms_opt(15, 30, 0).unwrap());

            // 랜덤한 가격 변동 시뮬레이션
            let noise = ((i as f64 * 0.7).sin() + (i as f64 * 1.3).cos()) * 0.02;
            let trend = i as f64 * 0.001;
            let price_mult = 1.0 + noise + trend;

            let open = base_price * price_mult;
            let high = open * 1.02;
            let low = open * 0.98;
            let close = open * (1.0 + noise * 0.5);
            let volume = 1000000.0 * (1.0 + noise.abs());

            Kline {
                symbol: symbol.clone(),
                timeframe: Timeframe::D1,
                open_time,
                close_time,
                open: Decimal::from_f64(open).unwrap_or(Decimal::from(50000)),
                high: Decimal::from_f64(high).unwrap_or(Decimal::from(51000)),
                low: Decimal::from_f64(low).unwrap_or(Decimal::from(49000)),
                close: Decimal::from_f64(close).unwrap_or(Decimal::from(50500)),
                volume: Decimal::from_f64(volume).unwrap_or(Decimal::from(1000000)),
                quote_volume: None,
                num_trades: None,
            }
        })
        .collect()
}

/// BacktestReport를 API 응답으로 변환
fn convert_report_to_response(
    report: &BacktestReport,
    strategy_id: &str,
    symbol: &str,
    start_date: &str,
    end_date: &str,
) -> BacktestRunResponse {
    let result_id = uuid::Uuid::new_v4().to_string();

    // 자산 곡선 변환
    let equity_curve: Vec<EquityCurvePoint> = report
        .equity_curve
        .iter()
        .map(|ep| EquityCurvePoint {
            timestamp: ep.timestamp.timestamp(),
            equity: ep.equity,
            drawdown_pct: ep.drawdown_pct,
        })
        .collect();

    // 거래 내역 변환
    let trades: Vec<TradeHistoryItem> = report
        .trades
        .iter()
        .map(|rt| TradeHistoryItem {
            symbol: rt.symbol.to_string(),
            entry_time: rt.entry_time,
            exit_time: rt.exit_time,
            entry_price: rt.entry_price,
            exit_price: rt.exit_price,
            quantity: rt.quantity,
            side: format!("{:?}", rt.side),
            pnl: rt.pnl,
            return_pct: rt.return_pct,
        })
        .collect();

    // 성과 지표 변환
    let metrics = BacktestMetricsResponse {
        total_return_pct: report.metrics.total_return_pct,
        annualized_return_pct: report.metrics.annualized_return_pct,
        net_profit: report.metrics.net_profit,
        total_trades: report.metrics.total_trades,
        win_rate_pct: report.metrics.win_rate_pct,
        profit_factor: report.metrics.profit_factor,
        sharpe_ratio: report.metrics.sharpe_ratio,
        sortino_ratio: report.metrics.sortino_ratio,
        max_drawdown_pct: report.metrics.max_drawdown_pct,
        calmar_ratio: report.metrics.calmar_ratio,
        avg_win: report.metrics.avg_win,
        avg_loss: report.metrics.avg_loss,
        largest_win: report.metrics.largest_win,
        largest_loss: report.metrics.largest_loss,
    };

    let config_summary = BacktestConfigSummary {
        initial_capital: report.config.initial_capital,
        commission_rate: report.config.commission_rate,
        slippage_rate: report.config.slippage_rate,
        total_commission: report.total_commission,
        total_slippage: report.total_slippage,
        data_points: report.data_points,
    };

    BacktestRunResponse {
        id: result_id,
        success: true,
        strategy_id: strategy_id.to_string(),
        symbol: symbol.to_string(),
        start_date: start_date.to_string(),
        end_date: end_date.to_string(),
        metrics,
        equity_curve,
        trades,
        config_summary,
    }
}

/// 백테스트 결과 조회
///
/// GET /api/v1/backtest/results/:id
///
/// 저장된 백테스트 결과를 조회합니다.
pub async fn get_backtest_result(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<BacktestApiError>)> {
    // 현재는 저장 기능이 없으므로 NOT_FOUND 반환
    // 추후 데이터베이스 연동 시 구현
    Err((
        StatusCode::NOT_FOUND,
        Json(BacktestApiError::new(
            "RESULT_NOT_FOUND",
            format!("백테스트 결과를 찾을 수 없습니다: {}", id),
        )),
    ))
}

/// 다중 자산 백테스트 실행
///
/// POST /api/v1/backtest/run-multi
///
/// 여러 심볼을 사용하는 자산배분 전략의 백테스트를 실행합니다.
/// 지원 전략: simple_power, haa, xaa, stock_rotation
pub async fn run_multi_backtest(
    State(state): State<Arc<AppState>>,
    Json(request): Json<BacktestMultiRunRequest>,
) -> Result<Json<BacktestMultiRunResponse>, (StatusCode, Json<BacktestApiError>)> {
    info!(
        "다중 자산 백테스트 실행 요청: strategy={}, symbols={:?}",
        request.strategy_id, request.symbols
    );

    // 날짜 파싱 검증
    let start_date = NaiveDate::parse_from_str(&request.start_date, "%Y-%m-%d").map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new(
                "INVALID_DATE",
                format!("잘못된 시작 날짜 형식: {}", request.start_date),
            )),
        )
    })?;

    let end_date = NaiveDate::parse_from_str(&request.end_date, "%Y-%m-%d").map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new(
                "INVALID_DATE",
                format!("잘못된 종료 날짜 형식: {}", request.end_date),
            )),
        )
    })?;

    // 날짜 유효성 검사
    if end_date <= start_date {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new(
                "INVALID_DATE_RANGE",
                "종료 날짜는 시작 날짜보다 이후여야 합니다",
            )),
        ));
    }

    // 심볼 목록 검증
    if request.symbols.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new(
                "INVALID_SYMBOLS",
                "최소 하나의 심볼이 필요합니다",
            )),
        ));
    }

    // 초기 자본금 검증
    if request.initial_capital <= Decimal::ZERO {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new(
                "INVALID_CAPITAL",
                "초기 자본금은 0보다 커야 합니다",
            )),
        ));
    }

    // 다중 자산 전략만 허용
    let valid_multi_strategies = ["simple_power", "haa", "xaa", "stock_rotation"];
    if !valid_multi_strategies.contains(&request.strategy_id.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new(
                "INVALID_STRATEGY",
                format!(
                    "다중 자산 백테스트는 다음 전략만 지원합니다: {:?}",
                    valid_multi_strategies
                ),
            )),
        ));
    }

    // 수수료/슬리피지 기본값 설정
    let commission_rate = request.commission_rate.unwrap_or(Decimal::new(1, 3));
    let slippage_rate = request.slippage_rate.unwrap_or(Decimal::new(5, 4));

    // 전략별로 필요한 모든 심볼 확장
    let expanded_symbols = expand_strategy_symbols(&request.strategy_id, &request.symbols);
    info!(
        "전략 {} 심볼 확장: {:?} -> {:?}",
        request.strategy_id, request.symbols, expanded_symbols
    );

    // 다중 심볼 데이터 로드
    let multi_klines = if let Some(pool) = &state.db_pool {
        match load_multi_klines_from_db(pool, &expanded_symbols, start_date, end_date).await {
            Ok(data) if !data.is_empty() => {
                info!("DB에서 {} 심볼의 데이터 로드 완료", data.len());
                data
            }
            Ok(_) => {
                warn!("DB에 데이터가 없어 샘플 데이터로 백테스트 실행");
                generate_multi_sample_klines(&expanded_symbols, start_date, end_date)
            }
            Err(e) => {
                warn!("DB 로드 실패, 샘플 데이터 사용: {}", e);
                generate_multi_sample_klines(&expanded_symbols, start_date, end_date)
            }
        }
    } else {
        debug!("DB 연결 없음, 샘플 데이터로 백테스트 실행");
        generate_multi_sample_klines(&expanded_symbols, start_date, end_date)
    };

    // 심볼별 데이터 포인트 수 계산
    let data_points_by_symbol: std::collections::HashMap<String, usize> = multi_klines
        .iter()
        .map(|(symbol, klines)| (symbol.clone(), klines.len()))
        .collect();

    // 모든 심볼의 캔들 데이터를 시간순으로 병합
    let merged_klines = merge_multi_klines(&multi_klines);

    if merged_klines.is_empty() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(BacktestApiError::new(
                "NO_DATA",
                "백테스트를 위한 데이터가 없습니다",
            )),
        ));
    }

    // 백테스트 설정
    let config = BacktestConfig::new(request.initial_capital)
        .with_commission_rate(commission_rate)
        .with_slippage_rate(slippage_rate);

    // 전략별 백테스트 실행 (다중 심볼 지원)
    let report = run_multi_strategy_backtest(
        &request.strategy_id,
        config,
        &merged_klines,
        &multi_klines,
        &request.parameters,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(BacktestApiError::new("BACKTEST_ERROR", e.to_string())),
        )
    })?;

    // BacktestReport를 API 응답으로 변환
    let response = convert_multi_report_to_response(
        &report,
        &request.strategy_id,
        &request.symbols,
        &request.start_date,
        &request.end_date,
        data_points_by_symbol,
    );

    info!(
        "다중 자산 백테스트 완료: total_return={:.2}%",
        report.metrics.total_return_pct
    );

    Ok(Json(response))
}

/// 다중 심볼의 Kline 데이터를 DB에서 로드 (Yahoo Finance fallback 포함)
async fn load_multi_klines_from_db(
    pool: &sqlx::PgPool,
    symbols: &[String],
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Result<std::collections::HashMap<String, Vec<Kline>>, String> {
    let mut result = std::collections::HashMap::new();

    for symbol_str in symbols {
        // 1. DB에서 로드 시도
        match load_klines_from_db(pool, symbol_str, start_date, end_date).await {
            Ok(klines) if !klines.is_empty() => {
                info!("심볼 {} DB에서 {} 개 캔들 로드", symbol_str, klines.len());
                result.insert(symbol_str.clone(), klines);
            }
            Ok(_) => {
                // 2. DB에 없으면 Yahoo Finance에서 다운로드
                info!("심볼 {} DB에 데이터 없음, Yahoo Finance에서 다운로드 시도", symbol_str);
                let (base, quote) = parse_symbol(symbol_str);

                match download_from_yahoo(&base, &quote, start_date, end_date).await {
                    Ok(downloaded) if !downloaded.is_empty() => {
                        info!(
                            "심볼 {} Yahoo Finance에서 {} 개 캔들 다운로드 완료",
                            symbol_str,
                            downloaded.len()
                        );

                        // 3. 다운로드한 데이터를 DB에 캐싱
                        if let Err(e) = save_klines_to_db(pool, &downloaded).await {
                            warn!("심볼 {} 캐싱 실패 (무시됨): {}", symbol_str, e);
                        } else {
                            debug!("심볼 {} DB에 캐싱 완료", symbol_str);
                        }

                        result.insert(symbol_str.clone(), downloaded);
                    }
                    Ok(_) => {
                        warn!("심볼 {} Yahoo Finance에서 데이터 없음", symbol_str);
                    }
                    Err(e) => {
                        warn!("심볼 {} Yahoo Finance 다운로드 실패: {}", symbol_str, e);
                    }
                }
            }
            Err(e) => {
                warn!("심볼 {} DB 로드 실패: {}", symbol_str, e);
            }
        }
    }

    Ok(result)
}

/// 다운로드한 Kline 데이터를 DB에 저장 (캐싱)
async fn save_klines_to_db(pool: &sqlx::PgPool, klines: &[Kline]) -> Result<(), String> {
    if klines.is_empty() {
        return Ok(());
    }

    // 첫 번째 캔들에서 심볼 정보 가져오기
    let first_kline = &klines[0];
    let base = &first_kline.symbol.base;
    let quote = &first_kline.symbol.quote;

    // 심볼 ID 조회 또는 생성
    let symbol_id: uuid::Uuid = match sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM symbols WHERE base = $1 AND quote = $2 LIMIT 1",
    )
    .bind(base)
    .bind(quote)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("심볼 조회 실패: {}", e))?
    {
        Some(id) => id,
        None => {
            // 심볼이 없으면 생성
            let market_type_str = match first_kline.symbol.market_type {
                MarketType::Crypto => "crypto",
                MarketType::Stock => "stock",
                MarketType::UsStock => "us_stock",
                MarketType::KrStock => "kr_stock",
                MarketType::Forex => "forex",
                MarketType::Futures => "futures",
            };

            sqlx::query_scalar::<_, uuid::Uuid>(
                r#"
                INSERT INTO symbols (base, quote, market_type, exchange, is_active)
                VALUES ($1, $2, $3::market_type, 'yahoo', true)
                ON CONFLICT (base, quote, market_type, exchange) DO UPDATE SET updated_at = NOW()
                RETURNING id
                "#,
            )
            .bind(base)
            .bind(quote)
            .bind(market_type_str)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("심볼 생성 실패: {}", e))?
        }
    };

    // klines 일괄 저장
    for kline in klines {
        sqlx::query(
            r#"
            INSERT INTO klines (time, symbol_id, timeframe, open, high, low, close, volume)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (symbol_id, timeframe, time) DO UPDATE
            SET open = EXCLUDED.open,
                high = EXCLUDED.high,
                low = EXCLUDED.low,
                close = EXCLUDED.close,
                volume = EXCLUDED.volume
            "#,
        )
        .bind(kline.open_time)
        .bind(symbol_id)
        .bind("D1")
        .bind(kline.open)
        .bind(kline.high)
        .bind(kline.low)
        .bind(kline.close)
        .bind(kline.volume)
        .execute(pool)
        .await
        .map_err(|e| format!("캔들 저장 실패: {}", e))?;
    }

    info!(
        "심볼 {}/{} 캔들 {} 개 DB에 저장 완료",
        base,
        quote,
        klines.len()
    );
    Ok(())
}

/// 전략별로 필요한 모든 심볼을 확장
///
/// 사용자가 입력한 심볼 외에 전략이 필요로 하는 추가 심볼을 자동으로 포함합니다.
fn expand_strategy_symbols(strategy_id: &str, user_symbols: &[String]) -> Vec<String> {
    let mut symbols: std::collections::HashSet<String> =
        user_symbols.iter().cloned().collect();

    // 전략별 필수 심볼 추가
    let required_symbols: &[&str] = match strategy_id {
        "simple_power" => &["TQQQ", "SCHD", "TMF", "PFIX"],
        "haa" => &["SPY", "TLT", "VEA", "VWO", "TIP", "BIL", "IEF"],
        "xaa" => &["SPY", "QQQ", "TLT", "IEF", "VEA", "VWO", "PDBC", "VNQ"],
        "stock_rotation" => &[], // 사용자 지정 심볼만 사용
        _ => &[],
    };

    for sym in required_symbols {
        symbols.insert(sym.to_string());
    }

    // 정렬된 벡터로 변환
    let mut result: Vec<String> = symbols.into_iter().collect();
    result.sort();
    result
}

/// 심볼 문자열을 base/quote로 파싱
fn parse_symbol(symbol_str: &str) -> (String, String) {
    if symbol_str.contains('/') {
        let parts: Vec<&str> = symbol_str.split('/').collect();
        (
            parts[0].to_string(),
            parts.get(1).map(|s| s.to_string()).unwrap_or("KRW".to_string()),
        )
    } else if symbol_str.chars().all(|c| c.is_ascii_digit()) {
        (symbol_str.to_string(), "KRW".to_string())
    } else {
        (symbol_str.to_string(), "USD".to_string())
    }
}

/// 다중 심볼 샘플 Kline 데이터 생성
fn generate_multi_sample_klines(
    symbols: &[String],
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> std::collections::HashMap<String, Vec<Kline>> {
    use rust_decimal::prelude::FromPrimitive;

    let mut result = std::collections::HashMap::new();
    let days = (end_date - start_date).num_days() as usize;

    // 심볼별 기본 가격 설정 (다양성을 위해)
    let base_prices: std::collections::HashMap<&str, f64> = [
        ("TQQQ", 45.0),
        ("SCHD", 75.0),
        ("PFIX", 25.0),
        ("TMF", 8.0),
        ("SPY", 450.0),
        ("QQQ", 380.0),
        ("TLT", 95.0),
        ("IEF", 100.0),
        ("VEA", 45.0),
        ("VWO", 40.0),
        ("TIP", 110.0),
        ("BIL", 91.5),
        ("PDBC", 15.0),
        ("VNQ", 85.0),
        ("EFA", 72.0),
        ("EEM", 38.0),
        ("LQD", 108.0),
        ("BND", 72.0),
    ]
    .iter()
    .cloned()
    .collect();

    for symbol_str in symbols {
        let (base, quote) = parse_symbol(symbol_str);
        let symbol = Symbol {
            base: base.clone(),
            quote: quote.clone(),
            market_type: MarketType::Stock,
            exchange_symbol: None,
        };

        let base_price = *base_prices.get(base.as_str()).unwrap_or(&50.0);

        let klines: Vec<Kline> = (0..=days)
            .map(|i| {
                let date = start_date + chrono::Duration::days(i as i64);
                let open_time = Utc.from_utc_datetime(&date.and_hms_opt(9, 0, 0).unwrap());
                let close_time = Utc.from_utc_datetime(&date.and_hms_opt(15, 30, 0).unwrap());

                // 심볼별로 다른 변동성 패턴
                let volatility = match base.as_str() {
                    "TQQQ" | "TMF" => 0.04, // 레버리지 ETF: 높은 변동성
                    "BIL" => 0.001,          // 단기 채권: 매우 낮은 변동성
                    "TLT" | "IEF" => 0.015,  // 채권 ETF: 중간 변동성
                    _ => 0.02,               // 일반 ETF
                };

                let noise = ((i as f64 * 0.7).sin() + (i as f64 * 1.3).cos()) * volatility;
                let trend = match base.as_str() {
                    "TQQQ" | "QQQ" | "SPY" => i as f64 * 0.0005, // 상승 추세
                    "TLT" | "TMF" => i as f64 * -0.0003,         // 하락 추세 (금리 상승)
                    _ => i as f64 * 0.0001,
                };
                let price_mult = 1.0 + noise + trend;

                let open = base_price * price_mult;
                let high = open * (1.0 + volatility * 0.5);
                let low = open * (1.0 - volatility * 0.5);
                let close = open * (1.0 + noise * 0.3);
                let volume = 1000000.0 * (1.0 + noise.abs());

                Kline {
                    symbol: symbol.clone(),
                    timeframe: Timeframe::D1,
                    open_time,
                    close_time,
                    open: Decimal::from_f64(open).unwrap_or(Decimal::from(50)),
                    high: Decimal::from_f64(high).unwrap_or(Decimal::from(51)),
                    low: Decimal::from_f64(low).unwrap_or(Decimal::from(49)),
                    close: Decimal::from_f64(close).unwrap_or(Decimal::from(50)),
                    volume: Decimal::from_f64(volume).unwrap_or(Decimal::from(1000000)),
                    quote_volume: None,
                    num_trades: None,
                }
            })
            .collect();

        result.insert(symbol_str.clone(), klines);
    }

    result
}

/// 다중 심볼 Kline 데이터를 시간순으로 병합
fn merge_multi_klines(
    multi_klines: &std::collections::HashMap<String, Vec<Kline>>,
) -> Vec<Kline> {
    let mut all_klines: Vec<Kline> = multi_klines
        .values()
        .flat_map(|klines| klines.iter().cloned())
        .collect();

    // 시간순 정렬
    all_klines.sort_by(|a, b| a.open_time.cmp(&b.open_time));

    all_klines
}

/// 다중 자산 전략 백테스트 실행
async fn run_multi_strategy_backtest(
    strategy_id: &str,
    config: BacktestConfig,
    merged_klines: &[Kline],
    multi_klines: &std::collections::HashMap<String, Vec<Kline>>,
    params: &Option<serde_json::Value>,
) -> Result<BacktestReport, String> {
    // 초기 자본금을 먼저 복사 (클로저에서 사용)
    let initial_capital = config.initial_capital;

    let mut engine = BacktestEngine::new(config);

    // 사용자 파라미터와 기본 설정 병합
    let merge_params =
        |default: serde_json::Value, user_params: &Option<serde_json::Value>| -> serde_json::Value {
            if let Some(user) = user_params {
                if let (Some(default_obj), Some(user_obj)) = (default.as_object(), user.as_object())
                {
                    let mut merged = default_obj.clone();
                    for (key, value) in user_obj {
                        merged.insert(key.clone(), value.clone());
                    }
                    return serde_json::Value::Object(merged);
                }
            }
            default
        };

    // 심볼 목록 추출
    let symbols: Vec<String> = multi_klines.keys().cloned().collect();

    // 초기 자본금을 전략 파라미터에 주입
    let inject_common_params = |mut cfg: serde_json::Value| -> serde_json::Value {
        if let Some(obj) = cfg.as_object_mut() {
            // 심볼 목록 주입
            obj.insert(
                "symbols".to_string(),
                serde_json::Value::Array(
                    symbols.iter().map(|s| serde_json::Value::String(s.clone())).collect(),
                ),
            );
            // 초기 자본금 주입 (cash_balance로 사용)
            obj.insert(
                "initial_capital".to_string(),
                serde_json::Value::String(initial_capital.to_string()),
            );
        }
        cfg
    };

    match strategy_id {
        "simple_power" => {
            let mut strategy = SimplePowerStrategy::new();
            let default_cfg =
                serde_json::to_value(SimplePowerConfig::default()).map_err(|e| e.to_string())?;
            let default_cfg = inject_common_params(default_cfg);
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, merged_klines)
                .await
                .map_err(|e| e.to_string())
        }
        "haa" => {
            let mut strategy = HaaStrategy::new();
            let default_cfg =
                serde_json::to_value(HaaConfig::default()).map_err(|e| e.to_string())?;
            let default_cfg = inject_common_params(default_cfg);
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, merged_klines)
                .await
                .map_err(|e| e.to_string())
        }
        "xaa" => {
            let mut strategy = XaaStrategy::new();
            let default_cfg =
                serde_json::to_value(XaaConfig::default()).map_err(|e| e.to_string())?;
            let default_cfg = inject_common_params(default_cfg);
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, merged_klines)
                .await
                .map_err(|e| e.to_string())
        }
        "stock_rotation" => {
            let mut strategy = StockRotationStrategy::new();
            let default_cfg =
                serde_json::to_value(StockRotationConfig::default()).map_err(|e| e.to_string())?;
            let default_cfg = inject_common_params(default_cfg);
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, merged_klines)
                .await
                .map_err(|e| e.to_string())
        }
        _ => Err(format!(
            "지원하지 않는 다중 자산 전략입니다: {}",
            strategy_id
        )),
    }
}

/// 다중 자산 BacktestReport를 API 응답으로 변환
fn convert_multi_report_to_response(
    report: &BacktestReport,
    strategy_id: &str,
    symbols: &[String],
    start_date: &str,
    end_date: &str,
    data_points_by_symbol: std::collections::HashMap<String, usize>,
) -> BacktestMultiRunResponse {
    let result_id = uuid::Uuid::new_v4().to_string();

    let equity_curve: Vec<EquityCurvePoint> = report
        .equity_curve
        .iter()
        .map(|ep| EquityCurvePoint {
            timestamp: ep.timestamp.timestamp(),
            equity: ep.equity,
            drawdown_pct: ep.drawdown_pct,
        })
        .collect();

    let trades: Vec<TradeHistoryItem> = report
        .trades
        .iter()
        .map(|rt| TradeHistoryItem {
            symbol: rt.symbol.to_string(),
            entry_time: rt.entry_time,
            exit_time: rt.exit_time,
            entry_price: rt.entry_price,
            exit_price: rt.exit_price,
            quantity: rt.quantity,
            side: format!("{:?}", rt.side),
            pnl: rt.pnl,
            return_pct: rt.return_pct,
        })
        .collect();

    let metrics = BacktestMetricsResponse {
        total_return_pct: report.metrics.total_return_pct,
        annualized_return_pct: report.metrics.annualized_return_pct,
        net_profit: report.metrics.net_profit,
        total_trades: report.metrics.total_trades,
        win_rate_pct: report.metrics.win_rate_pct,
        profit_factor: report.metrics.profit_factor,
        sharpe_ratio: report.metrics.sharpe_ratio,
        sortino_ratio: report.metrics.sortino_ratio,
        max_drawdown_pct: report.metrics.max_drawdown_pct,
        calmar_ratio: report.metrics.calmar_ratio,
        avg_win: report.metrics.avg_win,
        avg_loss: report.metrics.avg_loss,
        largest_win: report.metrics.largest_win,
        largest_loss: report.metrics.largest_loss,
    };

    let config_summary = BacktestConfigSummary {
        initial_capital: report.config.initial_capital,
        commission_rate: report.config.commission_rate,
        slippage_rate: report.config.slippage_rate,
        total_commission: report.total_commission,
        total_slippage: report.total_slippage,
        data_points: report.data_points,
    };

    BacktestMultiRunResponse {
        id: result_id,
        success: true,
        strategy_id: strategy_id.to_string(),
        symbols: symbols.to_vec(),
        start_date: start_date.to_string(),
        end_date: end_date.to_string(),
        metrics,
        equity_curve,
        trades,
        config_summary,
        data_points_by_symbol,
    }
}

// ==================== 라우터 ====================

/// 백테스트 라우터 생성
pub fn backtest_router() -> Router<Arc<AppState>> {
    Router::new()
        // 백테스트 가능한 전략 목록
        .route("/strategies", get(list_backtest_strategies))
        // 백테스트 실행 (단일 심볼)
        .route("/run", post(run_backtest))
        // 다중 자산 백테스트 실행
        .route("/run-multi", post(run_multi_backtest))
        // 백테스트 결과 조회
        .route("/results/:id", get(get_backtest_result))
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

    #[tokio::test]
    async fn test_list_backtest_strategies() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/strategies", get(list_backtest_strategies))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/strategies")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let list: BacktestStrategiesResponse = serde_json::from_slice(&body).unwrap();

        // 최소 3개의 기본 전략이 있어야 함
        assert!(list.total >= 3);
        assert!(list.strategies.iter().any(|s| s.id == "sma_crossover"));
        assert!(list.strategies.iter().any(|s| s.id == "rsi_mean_reversion"));
        assert!(list.strategies.iter().any(|s| s.id == "grid_trading"));
    }

    #[tokio::test]
    async fn test_run_backtest_success() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/run", post(run_backtest))
            .with_state(state);

        let request_body = serde_json::json!({
            "strategy_id": "sma_crossover",
            "symbol": "BTC/USDT",
            "start_date": "2024-01-01",
            "end_date": "2024-06-30",
            "initial_capital": 10000000
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/run")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let result: BacktestRunResponse = serde_json::from_slice(&body).unwrap();

        assert!(result.success);
        assert_eq!(result.strategy_id, "sma_crossover");
        assert_eq!(result.symbol, "BTC/USDT");
        assert!(!result.equity_curve.is_empty());
        // trades는 샘플 데이터에서 거래 신호가 발생하지 않을 수 있음
        // 실제 DB 데이터에서는 trades가 생성됨
    }

    #[tokio::test]
    async fn test_run_backtest_invalid_date() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/run", post(run_backtest))
            .with_state(state);

        let request_body = serde_json::json!({
            "strategy_id": "sma_crossover",
            "symbol": "BTC/USDT",
            "start_date": "invalid-date",
            "end_date": "2024-06-30",
            "initial_capital": 10000000
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/run")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let error: BacktestApiError = serde_json::from_slice(&body).unwrap();

        assert_eq!(error.code, "INVALID_DATE");
    }

    #[tokio::test]
    async fn test_run_backtest_invalid_date_range() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/run", post(run_backtest))
            .with_state(state);

        // 종료일이 시작일보다 이전
        let request_body = serde_json::json!({
            "strategy_id": "sma_crossover",
            "symbol": "BTC/USDT",
            "start_date": "2024-06-30",
            "end_date": "2024-01-01",
            "initial_capital": 10000000
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/run")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let error: BacktestApiError = serde_json::from_slice(&body).unwrap();

        assert_eq!(error.code, "INVALID_DATE_RANGE");
    }

    #[tokio::test]
    async fn test_run_backtest_strategy_not_found() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/run", post(run_backtest))
            .with_state(state);

        let request_body = serde_json::json!({
            "strategy_id": "nonexistent_strategy",
            "symbol": "BTC/USDT",
            "start_date": "2024-01-01",
            "end_date": "2024-06-30",
            "initial_capital": 10000000
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/run")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let error: BacktestApiError = serde_json::from_slice(&body).unwrap();

        assert_eq!(error.code, "STRATEGY_NOT_FOUND");
    }

    #[tokio::test]
    async fn test_get_backtest_result_not_found() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/results/:id", get(get_backtest_result))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/results/nonexistent-id")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_backtest_api_error_creation() {
        let error = BacktestApiError::new("TEST_ERROR", "테스트 메시지");
        assert_eq!(error.code, "TEST_ERROR");
        assert_eq!(error.message, "테스트 메시지");
    }
}
