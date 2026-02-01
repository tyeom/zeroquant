//! Dataset 관리 endpoint.
//!
//! 과거 데이터 캐시 조회, 다운로드 요청, 삭제 등을 관리합니다.
//!
//! # 엔드포인트
//!
//! - `GET /api/v1/dataset` - 캐시된 데이터셋 목록 조회
//! - `POST /api/v1/dataset/fetch` - 새 데이터셋 다운로드 요청
//! - `GET /api/v1/dataset/:symbol` - 특정 심볼의 캔들 데이터 조회
//! - `DELETE /api/v1/dataset/:symbol` - 특정 심볼 캐시 삭제

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

use trader_core::Timeframe;
use trader_data::cache::CachedHistoricalDataProvider;

use crate::repository::{SymbolInfoRepository, SymbolSearchResult};
use crate::routes::strategies::ApiError;
use crate::state::AppState;
use crate::tasks::eod_csv_sync::{sync_eod_all, sync_eod_exchange, EodSyncResult};
use crate::tasks::krx_csv_sync::{sync_krx_from_csv, sync_krx_full, update_sectors_from_csv, CsvSyncResult};

// ==================== 응답 타입 ====================

/// 캐시된 데이터셋 요약.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetSummary {
    /// 심볼
    pub symbol: String,
    /// 표시 이름 (예: "005930(삼성전자)")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// 타임프레임
    pub timeframe: String,
    /// 첫 번째 캔들 시간
    pub first_time: Option<DateTime<Utc>>,
    /// 마지막 캔들 시간
    pub last_time: Option<DateTime<Utc>>,
    /// 총 캔들 수
    pub candle_count: i64,
    /// 마지막 업데이트 시간
    pub last_updated: Option<DateTime<Utc>>,
}

/// 데이터셋 목록 응답.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetListResponse {
    pub datasets: Vec<DatasetSummary>,
    pub total_count: usize,
}

/// 캔들 데이터 응답.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandleDataResponse {
    pub symbol: String,
    pub timeframe: String,
    pub candles: Vec<CandleItem>,
    pub total_count: usize,
}

/// 단일 캔들.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandleItem {
    pub time: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
}

/// 데이터셋 다운로드 요청.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchDatasetRequest {
    /// 심볼 (예: 005930, AAPL)
    pub symbol: String,
    /// 타임프레임 (1d, 1h, 5m 등)
    #[serde(default = "default_timeframe")]
    pub timeframe: String,
    /// 다운로드할 캔들 수 (start_date/end_date가 없을 때 사용)
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// 시작 날짜 (ISO 8601, 예: 2024-01-01)
    #[serde(default)]
    pub start_date: Option<String>,
    /// 종료 날짜 (ISO 8601, 예: 2024-12-31)
    #[serde(default)]
    pub end_date: Option<String>,
}

fn default_timeframe() -> String {
    "1d".to_string()
}

fn default_limit() -> usize {
    500
}

/// 데이터셋 다운로드 응답.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchDatasetResponse {
    pub symbol: String,
    pub timeframe: String,
    pub fetched_count: usize,
    pub message: String,
}

/// 정렬 컬럼.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SortColumn {
    Time,
    Open,
    High,
    Low,
    Close,
    Volume,
}

impl Default for SortColumn {
    fn default() -> Self {
        Self::Time
    }
}

impl SortColumn {
    /// DB 컬럼명으로 변환.
    fn to_db_column(&self) -> &'static str {
        match self {
            Self::Time => "open_time",
            Self::Open => "open",
            Self::High => "high",
            Self::Low => "low",
            Self::Close => "close",
            Self::Volume => "volume",
        }
    }
}

/// 정렬 방향.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    Asc,
    Desc,
}

impl Default for SortOrder {
    fn default() -> Self {
        Self::Desc
    }
}

impl SortOrder {
    fn to_sql(&self) -> &'static str {
        match self {
            Self::Asc => "ASC",
            Self::Desc => "DESC",
        }
    }
}

/// 캔들 조회 쿼리.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandleQuery {
    /// 타임프레임 (기본: 1d)
    #[serde(default = "default_timeframe")]
    pub timeframe: String,
    /// 조회할 캔들 수 (기본: 100)
    #[serde(default = "default_candle_limit")]
    pub limit: usize,
    /// 페이지 (기본: 0)
    #[serde(default)]
    pub page: usize,
    /// 정렬 컬럼 (기본: time)
    #[serde(default)]
    pub sort_by: SortColumn,
    /// 정렬 방향 (기본: desc)
    #[serde(default)]
    pub sort_order: SortOrder,
}

fn default_candle_limit() -> usize {
    100
}

/// 삭제 쿼리.
#[derive(Debug, Deserialize)]
pub struct DeleteQuery {
    /// 특정 타임프레임만 삭제 (선택사항)
    pub timeframe: Option<String>,
}

// ==================== Handler ====================

/// 캐시된 데이터셋 목록 조회.
///
/// GET /api/v1/dataset
pub async fn list_datasets(
    State(state): State<Arc<AppState>>,
) -> Result<Json<DatasetListResponse>, (StatusCode, Json<ApiError>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new("DB_NOT_AVAILABLE", "데이터베이스 연결이 없습니다")),
        )
    })?;

    let provider = CachedHistoricalDataProvider::new(pool.clone());

    match provider.get_cache_stats().await {
        Ok(stats) => {
            // 심볼 목록 추출 (Yahoo 형식에서 canonical로 변환)
            let symbols: Vec<String> = stats
                .iter()
                .map(|s| {
                    // .KS, .KQ 등 Yahoo 접미사 제거
                    if s.symbol.ends_with(".KS") || s.symbol.ends_with(".KQ") {
                        s.symbol[..s.symbol.len() - 3].to_string()
                    } else {
                        s.symbol.clone()
                    }
                })
                .collect();

            // display_name 배치 조회
            let display_names = state.get_display_names(&symbols, false).await;

            let datasets: Vec<DatasetSummary> = stats
                .into_iter()
                .map(|s| {
                    // Yahoo 형식에서 canonical 심볼로 변환
                    let canonical = if s.symbol.ends_with(".KS") || s.symbol.ends_with(".KQ") {
                        s.symbol[..s.symbol.len() - 3].to_string()
                    } else {
                        s.symbol.clone()
                    };
                    let display_name = display_names.get(&canonical).cloned();
                    DatasetSummary {
                        symbol: canonical,
                        display_name,
                        timeframe: s.timeframe,
                        first_time: s.first_time,
                        last_time: s.last_time,
                        candle_count: s.candle_count,
                        last_updated: s.last_updated,
                    }
                })
                .collect();

            let total_count = datasets.len();

            info!(count = total_count, "데이터셋 목록 조회 성공");

            Ok(Json(DatasetListResponse {
                datasets,
                total_count,
            }))
        }
        Err(e) => {
            error!(error = %e, "데이터셋 목록 조회 실패");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("CACHE_STATS_ERROR", &format!("캐시 통계 조회 실패: {}", e))),
            ))
        }
    }
}

/// 새 데이터셋 다운로드 요청.
///
/// POST /api/v1/dataset/fetch
///
/// # 요청 본문
/// - `symbol`: 심볼 (예: 005930, AAPL)
/// - `timeframe`: 타임프레임 (기본값: 1d)
/// - `limit`: 캔들 수 (start_date/end_date가 없을 때 사용, 기본값: 500)
/// - `start_date`: 시작 날짜 (선택, 예: 2024-01-01)
/// - `end_date`: 종료 날짜 (선택, 예: 2024-12-31)
pub async fn fetch_dataset(
    State(state): State<Arc<AppState>>,
    Json(req): Json<FetchDatasetRequest>,
) -> Result<Json<FetchDatasetResponse>, (StatusCode, Json<ApiError>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new("DB_NOT_AVAILABLE", "데이터베이스 연결이 없습니다")),
        )
    })?;

    let timeframe = parse_timeframe(&req.timeframe);
    let provider = CachedHistoricalDataProvider::new(pool.clone());

    info!(
        symbol = %req.symbol,
        timeframe = %req.timeframe,
        limit = req.limit,
        start_date = ?req.start_date,
        end_date = ?req.end_date,
        "데이터셋 다운로드 요청"
    );

    // 날짜 범위가 지정된 경우 날짜 범위 API 사용
    let result = if req.start_date.is_some() && req.end_date.is_some() {
        let (start_date, end_date) = parse_date_range(&req)?;
        provider.get_klines_range(&req.symbol, timeframe, start_date, end_date).await
    } else if req.start_date.is_some() {
        // 시작 날짜만 있으면: 시작일 ~ 오늘
        let start_date = parse_start_date(&req)?;
        let end_date = Utc::now().date_naive();
        provider.get_klines_range(&req.symbol, timeframe, start_date, end_date).await
    } else {
        // 날짜 범위 없으면 기존 방식 (최근 N개)
        provider.get_klines(&req.symbol, timeframe, req.limit).await
    };

    match result {
        Ok(klines) => {
            let fetched_count = klines.len();
            let message = if req.start_date.is_some() || req.end_date.is_some() {
                format!(
                    "{}개 캔들 데이터가 캐시되었습니다 (기간: {} ~ {})",
                    fetched_count,
                    req.start_date.as_deref().unwrap_or("처음"),
                    req.end_date.as_deref().unwrap_or("오늘")
                )
            } else {
                format!("{}개 캔들 데이터가 캐시되었습니다", fetched_count)
            };

            info!(
                symbol = %req.symbol,
                count = fetched_count,
                "데이터셋 다운로드 완료"
            );

            Ok(Json(FetchDatasetResponse {
                symbol: req.symbol,
                timeframe: req.timeframe,
                fetched_count,
                message,
            }))
        }
        Err(e) => {
            error!(
                symbol = %req.symbol,
                error = %e,
                "데이터셋 다운로드 실패"
            );
            Err((
                StatusCode::BAD_GATEWAY,
                Json(ApiError::new("FETCH_ERROR", &format!("데이터 다운로드 실패: {}", e))),
            ))
        }
    }
}

/// 날짜 범위 파싱.
fn parse_date_range(
    req: &FetchDatasetRequest,
) -> Result<(NaiveDate, NaiveDate), (StatusCode, Json<ApiError>)> {
    let start = req.start_date.as_ref().unwrap();
    let end = req.end_date.as_ref().unwrap();

    let start_date = NaiveDate::parse_from_str(start, "%Y-%m-%d").map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "INVALID_START_DATE",
                &format!("시작 날짜 형식 오류 (YYYY-MM-DD): {}", e),
            )),
        )
    })?;

    let end_date = NaiveDate::parse_from_str(end, "%Y-%m-%d").map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "INVALID_END_DATE",
                &format!("종료 날짜 형식 오류 (YYYY-MM-DD): {}", e),
            )),
        )
    })?;

    if start_date > end_date {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "INVALID_DATE_RANGE",
                "시작 날짜가 종료 날짜보다 늦습니다",
            )),
        ));
    }

    Ok((start_date, end_date))
}

/// 시작 날짜 파싱.
fn parse_start_date(
    req: &FetchDatasetRequest,
) -> Result<NaiveDate, (StatusCode, Json<ApiError>)> {
    let start = req.start_date.as_ref().unwrap();

    NaiveDate::parse_from_str(start, "%Y-%m-%d").map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "INVALID_START_DATE",
                &format!("시작 날짜 형식 오류 (YYYY-MM-DD): {}", e),
            )),
        )
    })
}

#[allow(dead_code)]
/// 날짜 범위에서 유효한 limit 계산 (사용하지 않음).
fn calculate_effective_limit(
    req: &FetchDatasetRequest,
) -> Result<usize, (StatusCode, Json<ApiError>)> {
    match (&req.start_date, &req.end_date) {
        (Some(start), Some(end)) => {
            let start_date = NaiveDate::parse_from_str(start, "%Y-%m-%d").map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ApiError::new(
                        "INVALID_START_DATE",
                        &format!("시작 날짜 형식 오류 (YYYY-MM-DD): {}", e),
                    )),
                )
            })?;
            let end_date = NaiveDate::parse_from_str(end, "%Y-%m-%d").map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ApiError::new(
                        "INVALID_END_DATE",
                        &format!("종료 날짜 형식 오류 (YYYY-MM-DD): {}", e),
                    )),
                )
            })?;

            if start_date > end_date {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ApiError::new(
                        "INVALID_DATE_RANGE",
                        "시작 날짜가 종료 날짜보다 늦습니다",
                    )),
                ));
            }

            let days = (end_date - start_date).num_days() as usize;
            // 최소 1일, 여유분 포함
            Ok(days.max(1) + 30)
        }
        (Some(start), None) => {
            let start_date = NaiveDate::parse_from_str(start, "%Y-%m-%d").map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ApiError::new(
                        "INVALID_START_DATE",
                        &format!("시작 날짜 형식 오류 (YYYY-MM-DD): {}", e),
                    )),
                )
            })?;
            let today = Utc::now().date_naive();
            let days = (today - start_date).num_days() as usize;
            Ok(days.max(1) + 30)
        }
        (None, Some(end)) => {
            // 종료 날짜만 지정된 경우, 기본 limit 사용하되 최대 범위 제한
            let end_date = NaiveDate::parse_from_str(end, "%Y-%m-%d").map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ApiError::new(
                        "INVALID_END_DATE",
                        &format!("종료 날짜 형식 오류 (YYYY-MM-DD): {}", e),
                    )),
                )
            })?;
            let _ = end_date; // 유효성 검사만 수행
            Ok(req.limit)
        }
        (None, None) => Ok(req.limit),
    }
}

/// 특정 심볼의 캔들 데이터 조회 (정렬 지원).
///
/// GET /api/v1/dataset/:symbol
///
/// # 쿼리 파라미터
/// - `timeframe`: 타임프레임 (1d, 1h, 5m 등)
/// - `limit`: 조회할 캔들 수
/// - `page`: 페이지 번호
/// - `sortBy`: 정렬 컬럼 (time, open, high, low, close, volume)
/// - `sortOrder`: 정렬 방향 (asc, desc)
pub async fn get_candles(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
    Query(query): Query<CandleQuery>,
) -> Result<Json<CandleDataResponse>, (StatusCode, Json<ApiError>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new("DB_NOT_AVAILABLE", "데이터베이스 연결이 없습니다")),
        )
    })?;

    // 심볼 변환 (6자리 숫자는 .KS 추가)
    let yahoo_symbol = to_yahoo_symbol(&symbol);
    let tf_str = timeframe_to_db_string(&query.timeframe);

    // 정렬 조건이 기본값(time desc)이 아닌 경우 DB 직접 쿼리
    // 기본값인 경우 캐시 제공자 사용 (캐시 업데이트 로직 활용)
    if query.sort_by == SortColumn::Time && query.sort_order == SortOrder::Desc {
        // 기본 정렬: 캐시 제공자 사용
        let timeframe = parse_timeframe(&query.timeframe);
        let provider = CachedHistoricalDataProvider::new(pool.clone());
        let total_limit = (query.page + 1) * query.limit;

        // 전체 개수를 DB에서 조회 (캐시 제공자는 limit만큼만 반환하므로)
        let total_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ohlcv WHERE symbol = $1 AND timeframe = $2"
        )
        .bind(&yahoo_symbol)
        .bind(&tf_str)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        match provider.get_klines(&symbol, timeframe, total_limit).await {
            Ok(klines) => {
                let start = query.page * query.limit;

                // 최신순 정렬 (desc)
                let mut sorted_klines = klines;
                sorted_klines.sort_by(|a, b| b.open_time.cmp(&a.open_time));

                let candles: Vec<CandleItem> = sorted_klines
                    .into_iter()
                    .skip(start)
                    .take(query.limit)
                    .map(|k| CandleItem {
                        time: k.open_time.format("%Y-%m-%d %H:%M:%S").to_string(),
                        open: k.open.to_string(),
                        high: k.high.to_string(),
                        low: k.low.to_string(),
                        close: k.close.to_string(),
                        volume: k.volume.to_string(),
                    })
                    .collect();

                info!(
                    symbol = %symbol,
                    timeframe = %query.timeframe,
                    sort_by = "time",
                    sort_order = "desc",
                    page = query.page,
                    returned = candles.len(),
                    total = total_count,
                    "캔들 데이터 조회 성공 (캐시)"
                );

                return Ok(Json(CandleDataResponse {
                    symbol,
                    timeframe: query.timeframe,
                    candles,
                    total_count: total_count as usize,
                }));
            }
            Err(e) => {
                error!(symbol = %symbol, error = %e, "캔들 데이터 조회 실패");
                return Err((
                    StatusCode::BAD_GATEWAY,
                    Json(ApiError::new("FETCH_ERROR", &format!("캔들 데이터 조회 실패: {}", e))),
                ));
            }
        }
    }

    // 커스텀 정렬: DB 직접 쿼리
    let sort_column = query.sort_by.to_db_column();
    let sort_order = query.sort_order.to_sql();
    let offset = query.page * query.limit;

    // 전체 개수 조회
    let count_query = r#"
        SELECT COUNT(*) as count
        FROM ohlcv
        WHERE symbol = $1 AND timeframe = $2
    "#;

    let total_count: i64 = sqlx::query_scalar(count_query)
        .bind(&yahoo_symbol)
        .bind(&tf_str)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            error!(error = %e, "캔들 개수 조회 실패");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", &format!("데이터 조회 실패: {}", e))),
            )
        })?;

    // 동적 ORDER BY로 데이터 조회
    // 보안: sort_column은 enum에서 변환되므로 SQL 인젝션 위험 없음
    let data_query = format!(
        r#"
        SELECT open_time, open, high, low, close, volume
        FROM ohlcv
        WHERE symbol = $1 AND timeframe = $2
        ORDER BY {} {}
        LIMIT $3 OFFSET $4
        "#,
        sort_column, sort_order
    );

    #[derive(sqlx::FromRow)]
    struct OhlcvRow {
        open_time: DateTime<Utc>,
        open: rust_decimal::Decimal,
        high: rust_decimal::Decimal,
        low: rust_decimal::Decimal,
        close: rust_decimal::Decimal,
        volume: rust_decimal::Decimal,
    }

    let rows: Vec<OhlcvRow> = sqlx::query_as(&data_query)
        .bind(&yahoo_symbol)
        .bind(&tf_str)
        .bind(query.limit as i64)
        .bind(offset as i64)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!(error = %e, "캔들 데이터 조회 실패");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", &format!("데이터 조회 실패: {}", e))),
            )
        })?;

    let candles: Vec<CandleItem> = rows
        .into_iter()
        .map(|r| CandleItem {
            time: r.open_time.format("%Y-%m-%d %H:%M:%S").to_string(),
            open: r.open.to_string(),
            high: r.high.to_string(),
            low: r.low.to_string(),
            close: r.close.to_string(),
            volume: r.volume.to_string(),
        })
        .collect();

    info!(
        symbol = %symbol,
        timeframe = %query.timeframe,
        sort_by = %sort_column,
        sort_order = %sort_order,
        page = query.page,
        returned = candles.len(),
        total = total_count,
        "캔들 데이터 조회 성공 (DB 직접)"
    );

    Ok(Json(CandleDataResponse {
        symbol,
        timeframe: query.timeframe,
        candles,
        total_count: total_count as usize,
    }))
}

/// 특정 심볼 캐시 삭제.
///
/// DELETE /api/v1/dataset/:symbol
pub async fn delete_dataset(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
    Query(query): Query<DeleteQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new("DB_NOT_AVAILABLE", "데이터베이스 연결이 없습니다")),
        )
    })?;

    // 심볼 변환 (6자리 숫자는 .KS 추가)
    let yahoo_symbol = to_yahoo_symbol(&symbol);

    info!(
        symbol = %yahoo_symbol,
        timeframe = ?query.timeframe,
        "데이터셋 삭제 요청"
    );

    let deleted = if let Some(ref tf) = query.timeframe {
        // 특정 타임프레임만 삭제
        sqlx::query(
            "DELETE FROM ohlcv WHERE symbol = $1 AND timeframe = $2"
        )
        .bind(&yahoo_symbol)
        .bind(tf)
        .execute(pool)
        .await
        .map_err(|e| {
            error!(error = %e, "캐시 삭제 실패");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DELETE_ERROR", &format!("삭제 실패: {}", e))),
            )
        })?
        .rows_affected()
    } else {
        // 모든 타임프레임 삭제
        sqlx::query("DELETE FROM ohlcv WHERE symbol = $1")
            .bind(&yahoo_symbol)
            .execute(pool)
            .await
            .map_err(|e| {
                error!(error = %e, "캐시 삭제 실패");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError::new("DELETE_ERROR", &format!("삭제 실패: {}", e))),
                )
            })?
            .rows_affected()
    };

    // 메타데이터도 삭제
    if let Some(ref tf) = query.timeframe {
        let _ = sqlx::query(
            "DELETE FROM ohlcv_metadata WHERE symbol = $1 AND timeframe = $2"
        )
        .bind(&yahoo_symbol)
        .bind(tf)
        .execute(pool)
        .await;
    } else {
        let _ = sqlx::query("DELETE FROM ohlcv_metadata WHERE symbol = $1")
            .bind(&yahoo_symbol)
            .execute(pool)
            .await;
    }

    info!(
        symbol = %yahoo_symbol,
        deleted = deleted,
        "데이터셋 삭제 완료"
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "symbol": symbol,
        "deleted_count": deleted,
        "message": format!("{}개 캔들 데이터가 삭제되었습니다", deleted)
    })))
}

// ==================== 헬퍼 함수 ====================

/// 타임프레임 문자열을 Timeframe enum으로 변환.
fn parse_timeframe(tf: &str) -> Timeframe {
    match tf.to_lowercase().as_str() {
        "1m" => Timeframe::M1,
        "3m" => Timeframe::M3,
        "5m" => Timeframe::M5,
        "15m" => Timeframe::M15,
        "30m" => Timeframe::M30,
        "1h" => Timeframe::H1,
        "2h" => Timeframe::H2,
        "4h" => Timeframe::H4,
        "6h" => Timeframe::H6,
        "8h" => Timeframe::H8,
        "12h" => Timeframe::H12,
        "1d" | "d" => Timeframe::D1,
        "3d" => Timeframe::D3,
        "1w" | "w" => Timeframe::W1,
        "1M" | "M" | "1mn" | "mn" => Timeframe::MN1,
        _ => Timeframe::D1, // 기본값: 일봉
    }
}

/// 심볼을 Yahoo Finance 형식으로 변환.
fn to_yahoo_symbol(symbol: &str) -> String {
    if symbol.len() == 6 && symbol.chars().all(|c| c.is_ascii_digit()) {
        format!("{}.KS", symbol)
    } else {
        symbol.to_string()
    }
}

/// 타임프레임 문자열을 DB 저장 형식으로 변환.
fn timeframe_to_db_string(tf: &str) -> String {
    match tf.to_lowercase().as_str() {
        "1m" => "1m",
        "3m" => "3m",
        "5m" => "5m",
        "15m" => "15m",
        "30m" => "30m",
        "1h" => "1h",
        "2h" => "2h",
        "4h" => "4h",
        "6h" => "6h",
        "8h" => "8h",
        "12h" => "12h",
        "1d" | "d" => "1d",
        "3d" => "3d",
        "1w" | "w" => "1wk",
        "1M" | "M" | "1mn" | "mn" => "1mo",
        _ => "1d",
    }
    .to_string()
}

// ==================== 심볼 검색 ====================

/// 심볼 검색 요청 파라미터.
#[derive(Debug, Deserialize)]
pub struct SymbolSearchQuery {
    /// 검색어 (티커 또는 회사명)
    pub q: String,
    /// 최대 결과 수 (기본: 10)
    #[serde(default = "default_search_limit")]
    pub limit: i64,
}

fn default_search_limit() -> i64 {
    10
}

/// 심볼 검색 응답.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolSearchResponse {
    pub results: Vec<SymbolSearchResult>,
    pub total: usize,
}

/// 심볼 검색 API.
///
/// 티커 코드와 회사명으로 검색합니다.
/// DB에 없는 종목은 KRX/Yahoo Finance에서 자동으로 조회하여 저장합니다.
///
/// # Query Parameters
///
/// - `q`: 검색어 (필수)
/// - `limit`: 최대 결과 수 (선택, 기본 10)
///
/// # 예시
///
/// ```text
/// GET /api/v1/dataset/search?q=삼성&limit=5
/// GET /api/v1/dataset/search?q=AAPL
/// ```
pub async fn search_symbols(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SymbolSearchQuery>,
) -> Result<Json<SymbolSearchResponse>, (StatusCode, Json<ApiError>)> {
    if params.q.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                code: "INVALID_QUERY".to_string(),
                message: "검색어가 필요합니다".to_string(),
            }),
        ));
    }

    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError {
                code: "DB_UNAVAILABLE".to_string(),
                message: "데이터베이스를 사용할 수 없습니다".to_string(),
            }),
        )
    })?;

    // DB에서 먼저 검색
    let mut results = SymbolInfoRepository::search(pool, &params.q, params.limit)
        .await
        .map_err(|e| {
            error!("심볼 검색 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError {
                    code: "SEARCH_ERROR".to_string(),
                    message: format!("검색 실패: {}", e),
                }),
            )
        })?;

    // DB에서 결과가 없고, 검색어가 티커 형식인 경우 외부 API에서 조회
    if results.is_empty() {
        let query_trimmed = params.q.trim();
        let looks_like_ticker = query_trimmed.len() <= 10
            && query_trimmed.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-');

        if looks_like_ticker {
            info!(ticker = query_trimmed, "DB에 없는 티커, 외부 API에서 조회 시도");

            match SymbolInfoRepository::get_or_fetch(pool, query_trimmed).await {
                Ok(Some(result)) => {
                    results.push(result);
                }
                Ok(None) => {
                    // 외부 API에서도 찾지 못함
                }
                Err(e) => {
                    // 외부 조회 실패 로그만 남기고 빈 결과 반환
                    error!("외부 API 조회 실패: {}", e);
                }
            }
        }
    }

    let total = results.len();

    Ok(Json(SymbolSearchResponse { results, total }))
}

// ==================== KRX CSV 동기화 ====================

/// KRX CSV 동기화 응답.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KrxSyncResponse {
    /// 성공 여부
    pub success: bool,
    /// 심볼 동기화 결과
    pub symbols: Option<SyncResultDto>,
    /// 섹터 업데이트 결과
    pub sectors: Option<SyncResultDto>,
    /// 메시지
    pub message: String,
}

/// 동기화 결과 DTO.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResultDto {
    /// 처리된 총 레코드 수
    pub total_processed: usize,
    /// 성공적으로 upsert된 수
    pub upserted: usize,
    /// 실패한 수
    pub failed: usize,
    /// 스킵된 수
    pub skipped: usize,
}

impl From<CsvSyncResult> for SyncResultDto {
    fn from(r: CsvSyncResult) -> Self {
        Self {
            total_processed: r.total_processed,
            upserted: r.upserted,
            failed: r.failed,
            skipped: r.skipped,
        }
    }
}

/// KRX CSV에서 전체 심볼 및 섹터 동기화.
///
/// POST /api/v1/dataset/sync/krx
///
/// `data/krx_codes.csv`와 `data/krx_sector_map.csv` 파일을 읽어
/// symbol_info 테이블을 업데이트합니다.
pub async fn sync_krx_csv(
    State(state): State<Arc<AppState>>,
) -> Result<Json<KrxSyncResponse>, (StatusCode, Json<ApiError>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new("DB_NOT_AVAILABLE", "데이터베이스 연결이 없습니다")),
        )
    })?;

    info!("KRX CSV 전체 동기화 시작");

    // CSV 파일 경로
    let codes_csv = "data/krx_codes.csv";
    let sector_csv = "data/krx_sector_map.csv";

    // 전체 동기화 실행
    match sync_krx_full(pool, codes_csv, sector_csv).await {
        Ok((symbol_result, sector_result)) => {
            let message = format!(
                "심볼 {}개 동기화, 섹터 {}개 업데이트 완료",
                symbol_result.upserted, sector_result.upserted
            );

            info!(%message, "KRX CSV 동기화 완료");

            Ok(Json(KrxSyncResponse {
                success: true,
                symbols: Some(symbol_result.into()),
                sectors: Some(sector_result.into()),
                message,
            }))
        }
        Err(e) => {
            error!(error = %e, "KRX CSV 동기화 실패");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("SYNC_ERROR", &format!("동기화 실패: {}", e))),
            ))
        }
    }
}

/// KRX 섹터 정보만 업데이트.
///
/// POST /api/v1/dataset/sync/sectors
///
/// `data/krx_sector_map.csv` 파일을 읽어 섹터 정보만 업데이트합니다.
pub async fn sync_sectors_csv(
    State(state): State<Arc<AppState>>,
) -> Result<Json<KrxSyncResponse>, (StatusCode, Json<ApiError>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new("DB_NOT_AVAILABLE", "데이터베이스 연결이 없습니다")),
        )
    })?;

    info!("KRX 섹터 CSV 동기화 시작");

    let sector_csv = "data/krx_sector_map.csv";

    match update_sectors_from_csv(pool, sector_csv).await {
        Ok(result) => {
            let message = format!("섹터 {}개 업데이트 완료", result.upserted);

            info!(%message, "섹터 동기화 완료");

            Ok(Json(KrxSyncResponse {
                success: true,
                symbols: None,
                sectors: Some(result.into()),
                message,
            }))
        }
        Err(e) => {
            error!(error = %e, "섹터 동기화 실패");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("SYNC_ERROR", &format!("섹터 동기화 실패: {}", e))),
            ))
        }
    }
}

/// KRX 심볼 목록만 동기화.
///
/// POST /api/v1/dataset/sync/symbols
///
/// `data/krx_codes.csv` 파일을 읽어 심볼 목록을 동기화합니다.
pub async fn sync_symbols_csv(
    State(state): State<Arc<AppState>>,
) -> Result<Json<KrxSyncResponse>, (StatusCode, Json<ApiError>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new("DB_NOT_AVAILABLE", "데이터베이스 연결이 없습니다")),
        )
    })?;

    info!("KRX 심볼 CSV 동기화 시작");

    let codes_csv = "data/krx_codes.csv";

    match sync_krx_from_csv(pool, codes_csv).await {
        Ok(result) => {
            let message = format!("심볼 {}개 동기화 완료", result.upserted);

            info!(%message, "심볼 동기화 완료");

            Ok(Json(KrxSyncResponse {
                success: true,
                symbols: Some(result.into()),
                sectors: None,
                message,
            }))
        }
        Err(e) => {
            error!(error = %e, "심볼 동기화 실패");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("SYNC_ERROR", &format!("심볼 동기화 실패: {}", e))),
            ))
        }
    }
}

// ==================== EODData CSV 동기화 ====================

/// EODData 동기화 요청.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EodSyncRequest {
    /// 거래소 코드 (예: NYSE, NASDAQ). 없으면 전체 동기화
    pub exchange: Option<String>,
}

/// EODData 동기화 응답.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EodSyncResponse {
    /// 성공 여부
    pub success: bool,
    /// 거래소별 동기화 결과
    pub exchanges: Vec<EodExchangeResultDto>,
    /// 메시지
    pub message: String,
}

/// 거래소별 동기화 결과 DTO.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EodExchangeResultDto {
    /// 거래소 코드
    pub exchange: String,
    /// 처리된 총 레코드 수
    pub total_processed: usize,
    /// 성공적으로 upsert된 수
    pub upserted: usize,
    /// 스킵된 수
    pub skipped: usize,
}

impl From<EodSyncResult> for EodExchangeResultDto {
    fn from(r: EodSyncResult) -> Self {
        Self {
            exchange: r.exchange,
            total_processed: r.total_processed,
            upserted: r.upserted,
            skipped: r.skipped,
        }
    }
}

/// EODData CSV에서 해외 거래소 심볼 동기화.
///
/// POST /api/v1/dataset/sync/eod
///
/// `data/eod_*.csv` 파일들을 읽어 해외 거래소 심볼을 업데이트합니다.
/// exchange 파라미터가 있으면 해당 거래소만, 없으면 전체 동기화.
pub async fn sync_eod_csv(
    State(state): State<Arc<AppState>>,
    Query(req): Query<EodSyncRequest>,
) -> Result<Json<EodSyncResponse>, (StatusCode, Json<ApiError>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new("DB_NOT_AVAILABLE", "데이터베이스 연결이 없습니다")),
        )
    })?;

    let data_dir = "data";

    match &req.exchange {
        Some(exchange_code) => {
            // 특정 거래소만 동기화
            let csv_path = format!("{}/eod_{}.csv", data_dir, exchange_code.to_lowercase());
            info!(exchange = %exchange_code, path = %csv_path, "EODData 거래소 동기화 시작");

            match sync_eod_exchange(pool, exchange_code, &csv_path).await {
                Ok(result) => {
                    let message = format!(
                        "[{}] 심볼 {}개 동기화 완료",
                        exchange_code, result.upserted
                    );
                    info!(%message);

                    Ok(Json(EodSyncResponse {
                        success: true,
                        exchanges: vec![result.into()],
                        message,
                    }))
                }
                Err(e) => {
                    error!(exchange = %exchange_code, error = %e, "EODData 동기화 실패");
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ApiError::new("SYNC_ERROR", &format!("동기화 실패: {}", e))),
                    ))
                }
            }
        }
        None => {
            // 전체 거래소 동기화
            info!("EODData 전체 동기화 시작");

            match sync_eod_all(pool, data_dir).await {
                Ok(results) => {
                    let total_upserted: usize = results.values().map(|r| r.upserted).sum();
                    let exchanges: Vec<EodExchangeResultDto> = results
                        .into_values()
                        .map(|r| r.into())
                        .collect();

                    let message = format!(
                        "{}개 거래소에서 총 {}개 심볼 동기화 완료",
                        exchanges.len(),
                        total_upserted
                    );
                    info!(%message);

                    Ok(Json(EodSyncResponse {
                        success: true,
                        exchanges,
                        message,
                    }))
                }
                Err(e) => {
                    error!(error = %e, "EODData 전체 동기화 실패");
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ApiError::new("SYNC_ERROR", &format!("동기화 실패: {}", e))),
                    ))
                }
            }
        }
    }
}

// ==================== 라우터 ====================

/// Dataset 라우터 생성.
pub fn dataset_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_datasets))
        .route("/fetch", post(fetch_dataset))
        .route("/search", get(search_symbols))
        // KRX CSV 동기화
        .route("/sync/krx", post(sync_krx_csv))
        .route("/sync/symbols", post(sync_symbols_csv))
        .route("/sync/sectors", post(sync_sectors_csv))
        // EODData CSV 동기화 (해외 거래소)
        .route("/sync/eod", post(sync_eod_csv))
        // 심볼별 조회/삭제
        .route("/{symbol}", get(get_candles))
        .route("/{symbol}", delete(delete_dataset))
}
