//! 포트폴리오 성과 핸들러.
//!
//! 포트폴리오 성과 요약 API를 제공합니다.

use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Datelike, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::sync::Arc;
use tracing::{debug, warn};

use crate::repository::EquityHistoryRepository;
use crate::state::AppState;

use super::manager::AnalyticsManager;
use super::types::{PerformanceResponse, PeriodQuery, PeriodReturnResponse};

// ==================== 기간 파싱 유틸리티 ====================

/// 기간 문자열을 Duration으로 변환.
pub(crate) fn parse_period_duration(period: &str) -> Duration {
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
        _ => Duration::days(3650), // 10년 (all 및 기타)
    }
}

// ==================== 포지션 지표 계산 ====================

/// 포지션 기반 지표 계산 (총 투자금, 포지션 손익)
/// credential_id가 주어지면 해당 계좌만, 없으면 가장 최근 체결 기록이 있는 자격증명 사용
/// 순 포지션(매수-매도) 기준으로 현재 보유 중인 포지션만 계산
pub(crate) async fn get_position_metrics(
    pool: &sqlx::PgPool,
    credential_id: Option<uuid::Uuid>,
) -> Result<(Option<String>, Option<String>, Option<String>), sqlx::Error> {
    // credential_id가 주어지면 해당 계좌 사용, 없으면 가장 최근 체결 기록 있는 계좌
    let cred_id = if let Some(id) = credential_id {
        // 해당 계좌에 체결 기록이 있는지 확인
        let has_executions = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM execution_cache WHERE credential_id = $1",
        )
        .bind(id)
        .fetch_one(pool)
        .await?;

        if has_executions > 0 {
            id
        } else {
            // 체결 기록이 없으면 포지션 지표 없음
            return Ok((None, None, None));
        }
    } else {
        // 가장 최근 체결 기록이 있는 자격증명 ID 조회 (순 포지션이 양수인 것만)
        let active_cred_id = sqlx::query_scalar::<_, uuid::Uuid>(
            r#"
            SELECT credential_id
            FROM execution_cache
            GROUP BY credential_id
            HAVING SUM(CASE WHEN side = 'buy' THEN quantity ELSE -quantity END) > 0
            ORDER BY MAX(executed_at) DESC
            LIMIT 1
            "#,
        )
        .fetch_optional(pool)
        .await?;

        match active_cred_id {
            Some(id) => id,
            None => return Ok((None, None, None)),
        }
    };

    // 해당 자격증명의 순 보유 포지션 총 투자금(평균단가 기준) 조회
    // CTE를 사용하여 종목별 순 포지션과 평균 매수단가를 계산한 후 합산
    // ROUND()로 소수점 6자리까지 제한 (rust_decimal 호환성)
    let cost_result = sqlx::query_as::<_, (rust_decimal::Decimal, rust_decimal::Decimal)>(
        r#"
        WITH net_positions AS (
            SELECT
                symbol,
                SUM(CASE WHEN side = 'buy' THEN quantity ELSE -quantity END) as net_qty,
                SUM(CASE WHEN side = 'buy' THEN quantity * price ELSE 0 END) as total_buy_cost,
                SUM(CASE WHEN side = 'buy' THEN quantity ELSE 0 END) as total_buy_qty
            FROM execution_cache
            WHERE credential_id = $1
            GROUP BY symbol
            HAVING SUM(CASE WHEN side = 'buy' THEN quantity ELSE -quantity END) > 0
        )
        SELECT
            COALESCE(ROUND(SUM(net_qty), 6), 0) as total_net_qty,
            COALESCE(ROUND(SUM(
                CASE WHEN total_buy_qty > 0
                THEN (total_buy_cost / total_buy_qty) * net_qty
                ELSE 0 END
            ), 2), 0) as total_cost_basis
        FROM net_positions
        "#,
    )
    .bind(cred_id)
    .fetch_optional(pool)
    .await?;

    let (total_qty, total_cost) = match cost_result {
        Some((qty, cost)) if qty > rust_decimal::Decimal::ZERO => (qty, cost),
        _ => return Ok((None, None, None)),
    };

    // 현재 평가액 조회 (해당 자격증명의 최신 자산곡선 데이터)
    let current_value = sqlx::query_scalar::<_, rust_decimal::Decimal>(
        r#"
        SELECT COALESCE(securities_value, 0)
        FROM portfolio_equity_history
        WHERE credential_id = $1
        ORDER BY snapshot_time DESC
        LIMIT 1
        "#,
    )
    .bind(cred_id)
    .fetch_optional(pool)
    .await?
    .unwrap_or(rust_decimal::Decimal::ZERO);

    if current_value == rust_decimal::Decimal::ZERO {
        return Ok((Some(total_cost.to_string()), None, None));
    }

    // 포지션 손익 계산
    let position_pnl = current_value - total_cost;
    let position_pnl_pct = if total_cost > rust_decimal::Decimal::ZERO {
        (position_pnl / total_cost) * rust_decimal::Decimal::from(100)
    } else {
        rust_decimal::Decimal::ZERO
    };

    // Suppress unused variable warning
    let _ = total_qty;

    Ok((
        Some(total_cost.to_string()),
        Some(position_pnl.to_string()),
        Some(position_pnl_pct.to_string()),
    ))
}

// ==================== 핸들러 ====================

/// 성과 요약 조회.
///
/// GET /api/v1/analytics/performance
///
/// # Query Parameters
/// - `period`: 기간 (1w, 1m, 3m, 6m, 1y, ytd, all)
/// - `credential_id`: 자격증명 ID (선택적, 특정 계좌만 조회)
pub async fn get_performance(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PeriodQuery>,
) -> impl IntoResponse {
    // DB에서 실제 데이터 조회 시도
    if let Some(db_pool) = &state.db_pool {
        let duration = parse_period_duration(&query.period);
        let start_time = Utc::now() - duration;
        let end_time = Utc::now();

        // credential_id 파싱
        let credential_id = query
            .credential_id
            .as_ref()
            .and_then(|id| uuid::Uuid::parse_str(id).ok());

        // credential_id가 있으면 특정 계좌만 조회, 없으면 전체 합산
        let data_result = if let Some(cred_id) = credential_id {
            debug!(credential_id = %cred_id, "특정 계좌 성과 조회");
            EquityHistoryRepository::get_equity_curve(db_pool, cred_id, start_time, end_time).await
        } else {
            debug!("전체 계좌 통합 성과 조회");
            EquityHistoryRepository::get_aggregated_equity_curve(db_pool, start_time, end_time)
                .await
        };

        match data_result {
            Ok(data) if !data.is_empty() => {
                debug!("DB에서 {} 개의 자산 곡선 포인트 로드됨", data.len());

                // 초기 자본: 선택한 기간의 첫 번째 데이터 포인트 사용
                let initial_capital = data.first().map(|p| p.equity).unwrap_or(dec!(10_000_000));

                // 최고점: 선택한 기간 내 최고점
                let peak_equity = data
                    .iter()
                    .map(|p| p.equity)
                    .max()
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
                let max_drawdown_pct = data
                    .iter()
                    .map(|p| p.drawdown_pct)
                    .max()
                    .unwrap_or(Decimal::ZERO);

                // 현재 Drawdown
                let current_drawdown_pct = if peak_equity > Decimal::ZERO {
                    ((peak_equity - current_equity) / peak_equity) * dec!(100)
                } else {
                    Decimal::ZERO
                };

                // CAGR 계산 (연환산 수익률) - 1년 이상 기간에만 유효
                let days = data.len() as i64;
                let years = Decimal::from(days) / dec!(365);
                // CAGR은 1년 이상 기간에만 의미가 있음 (1년 미만은 연환산 시 비현실적인 값 발생)
                let cagr_pct = if days >= 365 && initial_capital > Decimal::ZERO {
                    let growth_factor = current_equity / initial_capital;
                    // (growth_factor^(1/years) - 1) * 100
                    let ln_growth = (growth_factor.to_string().parse::<f64>().unwrap_or(1.0)).ln();
                    let cagr =
                        (ln_growth / years.to_string().parse::<f64>().unwrap_or(1.0)).exp() - 1.0;
                    Decimal::from_f64_retain(cagr * 100.0).unwrap_or(Decimal::ZERO)
                } else {
                    // 1년 미만 기간에서는 CAGR 대신 단순 수익률 표시 (total_return_pct와 동일)
                    total_return_pct
                };

                // 포지션 기반 지표 계산 (실제 투자 원금 대비)
                let (total_cost_basis, position_pnl, position_pnl_pct) =
                    match get_position_metrics(db_pool, credential_id).await {
                        Ok(metrics) => metrics,
                        Err(e) => {
                            warn!("포지션 지표 조회 실패: {}", e);
                            (None, None, None)
                        }
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
                    total_cost_basis,
                    position_pnl,
                    position_pnl_pct,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_period_duration() {
        assert_eq!(parse_period_duration("1w").num_days(), 7);
        assert_eq!(parse_period_duration("1m").num_days(), 30);
        assert_eq!(parse_period_duration("3m").num_days(), 90);
        assert_eq!(parse_period_duration("6m").num_days(), 180);
        assert_eq!(parse_period_duration("1y").num_days(), 365);
    }

    #[tokio::test]
    async fn test_get_performance_endpoint() {
        use crate::state::create_test_state;
        use axum::{body::Body, http::Request, routing::get, Router};
        use tower::ServiceExt;

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

        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let perf: PerformanceResponse = serde_json::from_slice(&body).unwrap();

        assert!(!perf.current_equity.is_empty());
        assert!(perf.period_days > 0);
    }
}
