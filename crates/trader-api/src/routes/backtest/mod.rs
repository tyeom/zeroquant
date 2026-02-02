//! 백테스트 API 엔드포인트
//!
//! 과거 데이터로 트레이딩 전략을 시뮬레이션하고 성과를 분석하는 API를 제공합니다.
//!
//! # 엔드포인트
//!
//! - `GET /api/v1/backtest/strategies` - 백테스트 가능한 전략 목록
//! - `POST /api/v1/backtest/run` - 백테스트 실행
//! - `GET /api/v1/backtest/results/{id}` - 백테스트 결과 조회

mod engine;
mod loader;
mod types;
mod ui_schema;

// Re-export public types
pub use types::{
    BacktestApiError,
    BacktestConfigSummary,
    BacktestMetricsResponse,
    BacktestMultiRunRequest,
    BacktestMultiRunResponse,
    BacktestRunRequest,
    BacktestRunResponse,
    BacktestStrategiesResponse,
    BacktestableStrategy,
    BatchBacktestItem,
    // 배치 백테스트
    BatchBacktestRequest,
    BatchBacktestResponse,
    BatchBacktestResultItem,
    EquityCurvePoint,
    ExecutionSchedule,
    SymbolCategory,
    TradeHistoryItem,
    UiCondition,
    UiConditionOperator,
    UiField,
    UiFieldGroup,
    UiFieldType,
    UiLayout,
    UiSchema,
    UiSelectOption,
    UiValidation,
};

// Re-export UI schema functions
pub use ui_schema::get_ui_schema_for_strategy;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::state::AppState;
use trader_analytics::backtest::BacktestConfig;

use engine::{
    convert_multi_report_to_response, convert_report_to_response, generate_multi_sample_klines,
    run_multi_strategy_backtest, run_strategy_backtest,
};
use loader::{
    expand_strategy_symbols, generate_sample_klines, load_klines_from_db,
    load_multi_klines_from_db, merge_multi_klines,
};
// ui_schema 함수들은 get_ui_schema_for_strategy로 대체됨

// ==================== 핸들러 ====================

/// 백테스트 가능한 전략 목록 조회
///
/// GET /api/v1/backtest/strategies
///
/// 현재 등록된 모든 전략 중 백테스트가 가능한 전략 목록을 반환합니다.
pub async fn list_backtest_strategies(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // 최소 락 홀드: 데이터만 빠르게 복사하고 즉시 락 해제
    let all_statuses = {
        let engine = state.strategy_engine.read().await;
        engine.get_all_statuses().await
    }; // 락 해제됨

    // 락 없이 계산 수행
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
    let builtin_strategies = get_builtin_strategies();

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
    info!(
        "백테스트 실행 요청: strategy={}, symbol={}",
        request.strategy_id, request.symbol
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
        "sma_crossover",
        "rsi_mean_reversion",
        "grid_trading",
        "bollinger",
        "volatility_breakout",
        "magic_split",
        "simple_power",
        "haa",
        "xaa",
        "stock_rotation",
        // 신규 전략
        "all_weather",
        "snow",
        "market_cap_top",
        "candle_pattern",
        "infinity_bot",
        "market_interest_day",
        // 3차 전략
        "baa",
        "sector_momentum",
        "dual_momentum",
        "small_cap_quant",
        "pension_bot",
        // 2차 전략
        "sector_vb",
        "kospi_bothside",
        "kosdaq_fire_rain",
        "us_3x_leverage",
        "stock_gugan",
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

    // 전략별로 필요한 심볼을 동적으로 확장 (하드코딩 없이 expand_strategy_symbols에 위임)
    let user_symbols = vec![request.symbol.clone()];
    let expanded_symbols = expand_strategy_symbols(&request.strategy_id, &user_symbols);

    // 확장된 심볼이 1개 초과이면 다중 심볼 전략으로 처리
    let is_multi_symbol_strategy = expanded_symbols.len() > 1;

    if is_multi_symbol_strategy {
        info!(
            "다중 심볼 전략 {} 심볼 확장: {:?} -> {:?}",
            request.strategy_id, user_symbols, expanded_symbols
        );

        // 다중 심볼 데이터 로드
        let multi_klines = if let Some(pool) = &state.db_pool {
            match load_multi_klines_from_db(pool, &expanded_symbols, start_date, end_date).await {
                Ok(data) if !data.is_empty() => {
                    info!("DB에서 {} 심볼의 데이터 로드 완료", data.len());
                    for (sym, klines) in &data {
                        info!("  - {} 심볼: {} 개 캔들", sym, klines.len());
                    }
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

        // 모든 전략은 동일한 run_strategy_backtest 함수로 처리 (하드코딩 방지)
        // 병합된 캔들 데이터를 전달하여 전략이 필요한 심볼 데이터를 자체적으로 처리
        let report = run_strategy_backtest(
            &request.strategy_id,
            config,
            &merged_klines,
            &request.parameters,
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(BacktestApiError::new("BACKTEST_ERROR", e.to_string())),
            )
        })?;

        // BacktestReport를 API 응답으로 변환 (다중 심볼 표시)
        let symbols_str = expanded_symbols.join(",");
        let response = convert_report_to_response(
            &report,
            &request.strategy_id,
            &symbols_str,
            &request.start_date,
            &request.end_date,
        );

        info!(
            "다중 심볼 백테스트 완료: total_return={:.2}%",
            report.metrics.total_return_pct
        );

        return Ok(Json(response));
    }

    // 단일 심볼 전략 (기존 로직)
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

    info!(
        "백테스트 완료: total_return={:.2}%",
        report.metrics.total_return_pct
    );

    Ok(Json(response))
}

/// 백테스트 결과 조회
///
/// GET /api/v1/backtest/results/{id}
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
    let valid_multi_strategies = [
        "simple_power",
        "haa",
        "xaa",
        "stock_rotation",
        // 추가 다중 자산 전략들
        "all_weather",
        "all_weather_us",
        "all_weather_kr",
        "snow",
        "snow_us",
        "snow_kr",
        "baa",
        "sector_momentum",
        "dual_momentum",
        "pension_bot",
        "market_cap_top",
    ];
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
        // 배치 백테스트 (병렬 실행)
        .route("/run-batch", post(run_batch_backtest))
    // 백테스트 결과 조회는 backtest_results_router에서 처리
}

/// 배치 백테스트 실행 (병렬).
///
/// POST /api/v1/backtest/run-batch
///
/// 여러 전략을 병렬로 백테스트합니다.
/// 최대 10개 전략을 동시에 실행할 수 있습니다.
pub async fn run_batch_backtest(
    State(state): State<Arc<AppState>>,
    Json(request): Json<BatchBacktestRequest>,
) -> Result<Json<BatchBacktestResponse>, (StatusCode, Json<BacktestApiError>)> {
    use futures::stream::{self, StreamExt};
    use std::time::Instant;
    use validator::Validate;

    // 입력 유효성 검사
    if let Err(errors) = request.validate() {
        let message = errors
            .field_errors()
            .iter()
            .flat_map(|(field, errors)| {
                errors.iter().map(move |e| {
                    e.message
                        .as_ref()
                        .map(|m| m.to_string())
                        .unwrap_or_else(|| format!("{}: 유효하지 않은 값", field))
                })
            })
            .collect::<Vec<_>>()
            .join("; ");
        return Err((
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new("VALIDATION_ERROR", message)),
        ));
    }

    let start_time = Instant::now();
    let request_id = uuid::Uuid::new_v4().to_string();
    let parallelism = request.parallelism.unwrap_or(4).min(10);

    info!(
        request_id = %request_id,
        strategies = request.strategies.len(),
        parallelism = parallelism,
        "Starting batch backtest"
    );

    // 날짜 파싱
    let start_date = NaiveDate::parse_from_str(&request.start_date, "%Y-%m-%d").map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new(
                "INVALID_DATE",
                format!("시작 날짜 파싱 실패: {}", e),
            )),
        )
    })?;
    let end_date = NaiveDate::parse_from_str(&request.end_date, "%Y-%m-%d").map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new(
                "INVALID_DATE",
                format!("종료 날짜 파싱 실패: {}", e),
            )),
        )
    })?;

    // 수수료/슬리피지 기본값
    let commission_rate = request.commission_rate.unwrap_or(Decimal::new(1, 3));
    let slippage_rate = request.slippage_rate.unwrap_or(Decimal::new(5, 4));

    // 각 전략에 대한 백테스트 Future 생성
    let backtest_futures: Vec<_> = request
        .strategies
        .into_iter()
        .map(|item| {
            let state = Arc::clone(&state);
            let initial_capital = request.initial_capital;
            let strategy_id = item.strategy_id.clone();

            async move {
                let task_start = Instant::now();

                // 심볼에 따라 단일/다중 자산 백테스트 선택
                let result = if item.symbols.len() == 1 {
                    // 단일 심볼 백테스트
                    let symbol = item.symbols[0].clone();
                    run_single_strategy_internal(
                        &state,
                        &strategy_id,
                        &symbol,
                        start_date,
                        end_date,
                        initial_capital,
                        commission_rate,
                        slippage_rate,
                        &item.parameters,
                    )
                    .await
                } else {
                    // 다중 심볼 백테스트
                    run_multi_strategy_internal(
                        &state,
                        &strategy_id,
                        &item.symbols,
                        start_date,
                        end_date,
                        initial_capital,
                        commission_rate,
                        slippage_rate,
                        &item.parameters,
                    )
                    .await
                };

                let execution_time_ms = task_start.elapsed().as_millis() as u64;

                match result {
                    Ok(metrics) => BatchBacktestResultItem {
                        strategy_id,
                        success: true,
                        error: None,
                        metrics: Some(metrics),
                        execution_time_ms,
                    },
                    Err(e) => BatchBacktestResultItem {
                        strategy_id,
                        success: false,
                        error: Some(e),
                        metrics: None,
                        execution_time_ms,
                    },
                }
            }
        })
        .collect();

    // 병렬 실행 (parallelism 제한 적용: buffer_unordered 사용)
    let total_strategies = backtest_futures.len();
    let results: Vec<BatchBacktestResultItem> = stream::iter(backtest_futures)
        .buffer_unordered(parallelism)
        .collect()
        .await;

    let total_execution_time_ms = start_time.elapsed().as_millis() as u64;
    let successful = results.iter().filter(|r| r.success).count();
    let failed = results.iter().filter(|r| !r.success).count();

    info!(
        request_id = %request_id,
        successful = successful,
        failed = failed,
        total_time_ms = total_execution_time_ms,
        "Batch backtest completed"
    );

    Ok(Json(BatchBacktestResponse {
        request_id,
        total_strategies,
        successful,
        failed,
        total_execution_time_ms,
        results,
    }))
}

/// 단일 전략 내부 실행 (배치용).
async fn run_single_strategy_internal(
    state: &Arc<AppState>,
    strategy_id: &str,
    symbol: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
    initial_capital: Decimal,
    commission_rate: Decimal,
    slippage_rate: Decimal,
    params: &Option<serde_json::Value>,
) -> Result<BacktestMetricsResponse, String> {
    use trader_analytics::backtest::BacktestConfig;

    // 데이터 로드
    let klines = if let Some(pool) = &state.db_pool {
        match load_klines_from_db(pool, symbol, start_date, end_date).await {
            Ok(data) if !data.is_empty() => data,
            _ => generate_sample_klines(symbol, start_date, end_date),
        }
    } else {
        generate_sample_klines(symbol, start_date, end_date)
    };

    if klines.is_empty() {
        return Err("데이터 없음".to_string());
    }

    // 백테스트 설정
    let config = BacktestConfig::new(initial_capital)
        .with_commission_rate(commission_rate)
        .with_slippage_rate(slippage_rate);

    // 백테스트 실행
    let report = run_strategy_backtest(strategy_id, config, &klines, params)
        .await
        .map_err(|e| e.to_string())?;

    // 메트릭만 반환
    Ok(convert_report_to_metrics(&report))
}

/// 다중 자산 전략 내부 실행 (배치용).
async fn run_multi_strategy_internal(
    state: &Arc<AppState>,
    strategy_id: &str,
    symbols: &[String],
    start_date: NaiveDate,
    end_date: NaiveDate,
    initial_capital: Decimal,
    commission_rate: Decimal,
    slippage_rate: Decimal,
    params: &Option<serde_json::Value>,
) -> Result<BacktestMetricsResponse, String> {
    use trader_analytics::backtest::BacktestConfig;

    // 심볼 확장
    let expanded_symbols = expand_strategy_symbols(strategy_id, symbols);

    // 다중 심볼 데이터 로드
    let multi_klines = if let Some(pool) = &state.db_pool {
        match load_multi_klines_from_db(pool, &expanded_symbols, start_date, end_date).await {
            Ok(data) if !data.is_empty() => data,
            _ => generate_multi_sample_klines(&expanded_symbols, start_date, end_date),
        }
    } else {
        generate_multi_sample_klines(&expanded_symbols, start_date, end_date)
    };

    // 병합
    let merged_klines = merge_multi_klines(&multi_klines);

    if merged_klines.is_empty() {
        return Err("데이터 없음".to_string());
    }

    // 백테스트 설정
    let config = BacktestConfig::new(initial_capital)
        .with_commission_rate(commission_rate)
        .with_slippage_rate(slippage_rate);

    // 백테스트 실행
    let report =
        run_multi_strategy_backtest(strategy_id, config, &merged_klines, &multi_klines, params)
            .await
            .map_err(|e| e.to_string())?;

    // 메트릭만 반환
    Ok(convert_report_to_metrics(&report))
}

/// BacktestReport에서 메트릭만 추출.
fn convert_report_to_metrics(
    report: &trader_analytics::backtest::BacktestReport,
) -> BacktestMetricsResponse {
    let metrics = &report.metrics;
    BacktestMetricsResponse {
        total_return_pct: metrics.total_return_pct,
        annualized_return_pct: metrics.annualized_return_pct,
        net_profit: metrics.net_profit,
        total_trades: metrics.total_trades,
        win_rate_pct: metrics.win_rate_pct,
        profit_factor: metrics.profit_factor,
        sharpe_ratio: metrics.sharpe_ratio,
        sortino_ratio: metrics.sortino_ratio,
        max_drawdown_pct: metrics.max_drawdown_pct,
        calmar_ratio: metrics.calmar_ratio,
        avg_win: metrics.avg_win,
        avg_loss: metrics.avg_loss,
        largest_win: metrics.largest_win,
        largest_loss: metrics.largest_loss,
    }
}

// ==================== 내장 전략 목록 ====================

/// 구현된 모든 내장 전략 목록을 반환
fn get_builtin_strategies() -> Vec<BacktestableStrategy> {
    vec![
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
            id: "all_weather".to_string(),
            name: "올웨더".to_string(),
            description: "레이 달리오 올웨더 포트폴리오 (US/KR 선택)".to_string(),
            supported_symbols: vec![],  // 시장 선택에 따라 자동 결정
            default_params: serde_json::json!({
                "market": "US",
                "use_seasonality": true,
                "ma_periods": [50, 80, 120, 150],
                "rebalance_days": 30
            }),
            ui_schema: get_ui_schema_for_strategy("all_weather"),
            category: Some("자산배분".to_string()),
            tags: vec!["자산배분".to_string(), "올웨더".to_string(), "다중자산".to_string()],
            execution_schedule: Some(ExecutionSchedule::Monthly),
            schedule_detail: Some("매월 첫 거래일 리밸런싱".to_string()),
            how_it_works: Some("US: SPY, TLT, IEF, GLD, PDBC, IYK / KR: 360750, 294400, 148070 등 시장별 자산에 분산 투자. 5~10월 지옥기간 방어적 운용, MA 필터로 동적 조정.".to_string()),
        },
        BacktestableStrategy {
            id: "snow".to_string(),
            name: "스노우".to_string(),
            description: "TIP 기반 모멘텀 전략 (US/KR 선택)".to_string(),
            supported_symbols: vec![],  // 시장 선택에 따라 자동 결정
            default_params: serde_json::json!({
                "market": "US",
                "tip_ma_period": 200,
                "attack_ma_period": 5,
                "rebalance_days": 1
            }),
            ui_schema: get_ui_schema_for_strategy("snow"),
            category: Some("자산배분".to_string()),
            tags: vec!["모멘텀".to_string(), "자산배분".to_string()],
            execution_schedule: Some(ExecutionSchedule::Daily),
            schedule_detail: Some("장 마감 후 실행".to_string()),
            how_it_works: Some("TIP 10개월 MA 기준 모멘텀 판단. US: UPRO/TLT/BIL, KR: 레버리지(122630)/국고채(148070)에 투자.".to_string()),
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
            how_it_works: Some("최대 50라운드까지 물타기합니다. 1-5라운드: 무조건 매수, 6-20라운드: MA 확인, 21-30라운드: MA+양봉 확인. 트레일링 스톱 5%->10%로 익절 관리합니다.".to_string()),
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
        // 3차 전략들
        BacktestableStrategy {
            id: "baa".to_string(),
            name: "BAA".to_string(),
            description: "Bold Asset Allocation - 카나리아 자산 기반 듀얼 모멘텀 전략".to_string(),
            supported_symbols: vec![
                "SPY".to_string(), "VEA".to_string(), "VWO".to_string(), "BND".to_string(),
                "QQQ".to_string(), "IWM".to_string(),
                "TIP".to_string(), "DBC".to_string(), "BIL".to_string(), "IEF".to_string(), "TLT".to_string(),
            ],
            default_params: serde_json::json!({
                "version": "Bold",
                "total_amount": 10000000,
                "rebalance_threshold": 5
            }),
            ui_schema: get_ui_schema_for_strategy("baa"),
            category: Some("자산배분".to_string()),
            tags: vec!["듀얼모멘텀".to_string(), "카나리아".to_string(), "자산배분".to_string(), "월간리밸런싱".to_string()],
            execution_schedule: Some(ExecutionSchedule::Monthly),
            schedule_detail: Some("매월 초 리밸런싱".to_string()),
            how_it_works: Some("4개 카나리아 자산(SPY, VEA, VWO, BND)의 모멘텀으로 시장 상태를 판단합니다. 모두 양수면 공격 자산(QQQ, IWM 등) 중 최고 모멘텀 종목에 투자, 하나라도 음수면 방어 자산(TIP, TLT 등)으로 전환합니다.".to_string()),
        },
        BacktestableStrategy {
            id: "sector_momentum".to_string(),
            name: "섹터 모멘텀".to_string(),
            description: "섹터 ETF 모멘텀 순위 기반 투자 전략 (US/KR 지원)".to_string(),
            supported_symbols: vec![
                // US 섹터
                "XLK".to_string(), "XLF".to_string(), "XLV".to_string(), "XLY".to_string(),
                "XLP".to_string(), "XLE".to_string(), "XLI".to_string(), "XLB".to_string(),
                "XLU".to_string(), "XLRE".to_string(), "XLC".to_string(),
            ],
            default_params: serde_json::json!({
                "market": "US",
                "top_n": 3,
                "weighting_method": "Equal",
                "short_period": 20,
                "medium_period": 60,
                "long_period": 120
            }),
            ui_schema: get_ui_schema_for_strategy("sector_momentum"),
            category: Some("자산배분".to_string()),
            tags: vec!["섹터".to_string(), "모멘텀".to_string(), "ETF".to_string(), "월간리밸런싱".to_string()],
            execution_schedule: Some(ExecutionSchedule::Monthly),
            schedule_detail: Some("매월 초 리밸런싱".to_string()),
            how_it_works: Some("섹터 ETF의 단기(20일)/중기(60일)/장기(120일) 모멘텀을 가중 합산하여 순위를 매기고, 상위 N개 섹터에 동일 비중 또는 모멘텀 비례 비중으로 투자합니다.".to_string()),
        },
        BacktestableStrategy {
            id: "dual_momentum".to_string(),
            name: "듀얼 모멘텀".to_string(),
            description: "한국 주식 + 미국 채권 듀얼 모멘텀 전략".to_string(),
            supported_symbols: vec![
                // 한국 주식
                "069500".to_string(), "229200".to_string(),
                // 미국 채권
                "TLT".to_string(), "IEF".to_string(), "BIL".to_string(),
            ],
            default_params: serde_json::json!({
                "momentum_period": 63,
                "use_absolute_momentum": true,
                "total_amount": 10000000
            }),
            ui_schema: get_ui_schema_for_strategy("dual_momentum"),
            category: Some("자산배분".to_string()),
            tags: vec!["듀얼모멘텀".to_string(), "한국주식".to_string(), "미국채권".to_string(), "월간리밸런싱".to_string()],
            execution_schedule: Some(ExecutionSchedule::Monthly),
            schedule_detail: Some("매월 초 리밸런싱".to_string()),
            how_it_works: Some("한국 주식(KODEX 200, 코스닥150)과 미국 채권(TLT, IEF)의 상대 모멘텀을 비교하여 더 높은 쪽에 투자합니다. 선택된 자산의 절대 모멘텀이 음수면 안전 자산(BIL)으로 전환합니다.".to_string()),
        },
        BacktestableStrategy {
            id: "small_cap_quant".to_string(),
            name: "소형주 퀀트".to_string(),
            description: "코스닥 소형주 퀀트 전략. 20일 MA 필터 + 재무 필터".to_string(),
            supported_symbols: vec![
                "229200".to_string(),  // 코스닥150 ETF (기준 지수)
            ],
            default_params: serde_json::json!({
                "target_count": 20,
                "ma_period": 20,
                "min_market_cap": 50.0,
                "min_roe": 5.0,
                "min_pbr": 0.2,
                "min_per": 2.0,
                "total_amount": 10000000
            }),
            ui_schema: get_ui_schema_for_strategy("small_cap_quant"),
            category: Some("퀀트".to_string()),
            tags: vec!["소형주".to_string(), "퀀트".to_string(), "한국주식".to_string(), "월간리밸런싱".to_string()],
            execution_schedule: Some(ExecutionSchedule::Monthly),
            schedule_detail: Some("매월 초 리밸런싱".to_string()),
            how_it_works: Some("코스닥 소형지수가 20일 MA 위에 있을 때, 재무 필터(시총 50억+, ROE 5%+, PBR 0.2+, PER 2+)를 통과한 소형주 상위 20개에 동일 비중 투자합니다. MA 하회 시 전량 매도합니다.".to_string()),
        },
        BacktestableStrategy {
            id: "pension_bot".to_string(),
            name: "연금 자동화".to_string(),
            description: "개인연금 계좌용 정적+동적 자산배분 조합 전략".to_string(),
            supported_symbols: vec![
                // 주식 ETF
                "448290".to_string(), "379780".to_string(), "294400".to_string(),
                // 채권 ETF
                "305080".to_string(), "148070".to_string(),
                // 원자재 ETF
                "319640".to_string(),
                // 단기자금
                "130730".to_string(),
            ],
            default_params: serde_json::json!({
                "avg_momentum_period": 10,
                "top_bonus_count": 12,
                "cash_to_short_term_rate": 0.45,
                "cash_to_bonus_rate": 0.45,
                "total_amount": 10000000
            }),
            ui_schema: get_ui_schema_for_strategy("pension_bot"),
            category: Some("자산배분".to_string()),
            tags: vec!["연금".to_string(), "자산배분".to_string(), "모멘텀".to_string(), "월간리밸런싱".to_string()],
            execution_schedule: Some(ExecutionSchedule::Monthly),
            schedule_detail: Some("매월 초 리밸런싱".to_string()),
            how_it_works: Some("정적 자산배분(주식 44%, 채권 30%, 원자재 14%, 현금 12%)에 평균 모멘텀을 적용하여 동적 비중 조절. 남은 현금의 45%는 단기자금, 45%는 모멘텀 TOP 12에 차등 보너스로 분배합니다.".to_string()),
        },
        // 2차 전략들
        BacktestableStrategy {
            id: "sector_vb".to_string(),
            name: "섹터 변동성 돌파".to_string(),
            description: "한국 섹터 ETF 변동성 돌파 전략".to_string(),
            supported_symbols: vec![
                "139220".to_string(), "139230".to_string(), "139240".to_string(),
                "139250".to_string(), "139260".to_string(), "139270".to_string(),
            ],
            default_params: serde_json::json!({
                "k_factor": 0.5,
                "lookback_days": 20,
                "top_n": 3,
                "total_amount": 10000000
            }),
            ui_schema: None,
            category: Some("변동성".to_string()),
            tags: vec!["섹터".to_string(), "변동성돌파".to_string(), "한국주식".to_string(), "일간".to_string()],
            execution_schedule: Some(ExecutionSchedule::Daily),
            schedule_detail: Some("매일 장 시작 전 실행".to_string()),
            how_it_works: Some("한국 섹터 ETF 중 변동성 돌파 신호가 발생한 상위 3개 섹터에 투자합니다. 전일 고가-저가 범위의 K배 돌파 시 진입합니다.".to_string()),
        },
        BacktestableStrategy {
            id: "kospi_bothside".to_string(),
            name: "코스피 양방향".to_string(),
            description: "코스피 레버리지/인버스 양방향 매매 전략".to_string(),
            supported_symbols: vec![
                "122630".to_string(), // KODEX 레버리지
                "252670".to_string(), // KODEX 200선물인버스2X
            ],
            default_params: serde_json::json!({
                "rsi_period": 14,
                "rsi_oversold": 30,
                "rsi_overbought": 70,
                "ma_period": 20,
                "total_amount": 10000000
            }),
            ui_schema: None,
            category: Some("지수".to_string()),
            tags: vec!["코스피".to_string(), "레버리지".to_string(), "인버스".to_string(), "양방향".to_string()],
            execution_schedule: Some(ExecutionSchedule::Daily),
            schedule_detail: Some("매일 장중 실행".to_string()),
            how_it_works: Some("코스피 방향성에 따라 레버리지 또는 인버스 ETF에 투자합니다. RSI와 이동평균을 기반으로 방향을 판단합니다.".to_string()),
        },
        BacktestableStrategy {
            id: "kosdaq_fire_rain".to_string(),
            name: "코스닥 급등주".to_string(),
            description: "코스피/코스닥 복합 양방향 전략".to_string(),
            supported_symbols: vec![
                "229200".to_string(), // KODEX 코스닥150
                "251340".to_string(), // KODEX 코스닥150선물인버스
            ],
            default_params: serde_json::json!({
                "lookback_period": 20,
                "momentum_threshold": 0.05,
                "volume_threshold": 2.0,
                "total_amount": 10000000
            }),
            ui_schema: None,
            category: Some("지수".to_string()),
            tags: vec!["코스닥".to_string(), "급등주".to_string(), "모멘텀".to_string(), "양방향".to_string()],
            execution_schedule: Some(ExecutionSchedule::Daily),
            schedule_detail: Some("매일 장중 실행".to_string()),
            how_it_works: Some("코스닥 지수의 모멘텀과 거래량을 분석하여 레버리지 또는 인버스 ETF에 투자합니다.".to_string()),
        },
        BacktestableStrategy {
            id: "us_3x_leverage".to_string(),
            name: "미국 3배 레버리지".to_string(),
            description: "미국 3배 레버리지/인버스 ETF 조합 전략".to_string(),
            supported_symbols: vec![
                "TQQQ".to_string(), // ProShares UltraPro QQQ
                "SQQQ".to_string(), // ProShares UltraPro Short QQQ
                "UPRO".to_string(), // ProShares UltraPro S&P500
                "SPXU".to_string(), // ProShares UltraPro Short S&P500
            ],
            default_params: serde_json::json!({
                "ma_period": 20,
                "momentum_period": 10,
                "rebalance_threshold": 0.05,
                "total_amount": 10000000
            }),
            ui_schema: None,
            category: Some("레버리지".to_string()),
            tags: vec!["미국".to_string(), "3배레버리지".to_string(), "인버스".to_string(), "ETF".to_string()],
            execution_schedule: Some(ExecutionSchedule::Daily),
            schedule_detail: Some("매일 미국 장 마감 후 실행".to_string()),
            how_it_works: Some("미국 주요 지수(나스닥, S&P500)의 모멘텀에 따라 3배 레버리지 또는 인버스 ETF에 투자합니다.".to_string()),
        },
        BacktestableStrategy {
            id: "stock_gugan".to_string(),
            name: "주식 구간 매매".to_string(),
            description: "주식 구간 분할 매매 전략".to_string(),
            supported_symbols: vec![
                "005930".to_string(), // 삼성전자
                "000660".to_string(), // SK하이닉스
            ],
            default_params: serde_json::json!({
                "zone_count": 10,
                "zone_pct": 5.0,
                "invest_per_zone": 1000000,
                "total_amount": 10000000
            }),
            ui_schema: None,
            category: Some("분할매매".to_string()),
            tags: vec!["구간매매".to_string(), "분할매수".to_string(), "한국주식".to_string()],
            execution_schedule: Some(ExecutionSchedule::Daily),
            schedule_detail: Some("매일 장중 실행".to_string()),
            how_it_works: Some("주가를 10개 구간으로 나누어 하락 시 분할 매수, 상승 시 분할 매도합니다. 구간별 동일 금액을 투자합니다.".to_string()),
        },
    ]
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
            .route("/results/{id}", get(get_backtest_result))
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
