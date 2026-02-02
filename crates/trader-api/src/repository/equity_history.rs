//! 포트폴리오 자산 히스토리 Repository.
//!
//! 자산 곡선(Equity Curve) 데이터의 저장 및 조회를 담당합니다.
//! PostgreSQL의 window functions를 활용한 효율적인 분석 쿼리를 제공합니다.

use chrono::{DateTime, Duration, NaiveDate, TimeZone, Timelike, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::collections::{BTreeMap, HashMap};
use tracing::{info, warn};
use uuid::Uuid;

// ==================== 타입 정의 ====================

/// 포트폴리오 스냅샷 데이터.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSnapshot {
    pub credential_id: Uuid,
    pub snapshot_time: DateTime<Utc>,
    pub total_equity: Decimal,
    pub cash_balance: Decimal,
    pub securities_value: Decimal,
    pub total_pnl: Decimal,
    pub daily_pnl: Decimal,
    pub currency: String,
    pub market: String,
    pub account_type: Option<String>,
}

/// 자산 곡선 데이터 포인트.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityPoint {
    pub timestamp: DateTime<Utc>,
    pub equity: Decimal,
    pub drawdown_pct: Decimal,
    pub return_pct: Decimal,
}

/// 월별 수익률 데이터.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlyReturn {
    pub year: i32,
    pub month: u32,
    pub opening_equity: Decimal,
    pub closing_equity: Decimal,
    pub return_pct: Decimal,
}

/// 동기화용 체결 정보.
#[derive(Debug, Clone)]
pub struct ExecutionForSync {
    /// 체결 시간
    pub execution_time: DateTime<Utc>,
    /// 체결 금액 (수량 × 가격)
    pub amount: Decimal,
    /// 매수 여부 (true: 매수, false: 매도)
    pub is_buy: bool,
    /// 종목 코드
    pub symbol: String,
}

/// 동기화 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    /// 동기화된 스냅샷 개수
    pub synced_count: usize,
    /// 처리된 체결 내역 개수
    pub execution_count: usize,
    /// 시작 날짜
    pub start_date: String,
    /// 종료 날짜
    pub end_date: String,
    /// 동기화 시간
    pub synced_at: DateTime<Utc>,
}

// ==================== Repository ====================

/// 포트폴리오 자산 히스토리 Repository.
///
/// `portfolio_equity_history` 테이블에 대한 CRUD 및 분석 쿼리를 제공합니다.
pub struct EquityHistoryRepository;

impl EquityHistoryRepository {
    // ==================== 저장 ====================

    /// 포트폴리오 스냅샷 저장.
    ///
    /// 동일 시간대(분 단위)에 이미 스냅샷이 있으면 업데이트합니다 (UPSERT).
    pub async fn save_snapshot(
        pool: &PgPool,
        snapshot: &PortfolioSnapshot,
    ) -> Result<Uuid, sqlx::Error> {
        // 분 단위로 truncate하여 중복 방지
        let truncated_time = snapshot
            .snapshot_time
            .with_nanosecond(0)
            .unwrap()
            .with_second(0)
            .unwrap();

        let row = sqlx::query(
            r#"
            INSERT INTO portfolio_equity_history (
                credential_id, snapshot_time, total_equity, cash_balance,
                securities_value, total_pnl, daily_pnl, currency, market, account_type
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (credential_id, snapshot_time)
            DO UPDATE SET
                total_equity = EXCLUDED.total_equity,
                cash_balance = EXCLUDED.cash_balance,
                securities_value = EXCLUDED.securities_value,
                total_pnl = EXCLUDED.total_pnl,
                daily_pnl = EXCLUDED.daily_pnl
            RETURNING id
            "#,
        )
        .bind(snapshot.credential_id)
        .bind(truncated_time)
        .bind(snapshot.total_equity)
        .bind(snapshot.cash_balance)
        .bind(snapshot.securities_value)
        .bind(snapshot.total_pnl)
        .bind(snapshot.daily_pnl)
        .bind(&snapshot.currency)
        .bind(&snapshot.market)
        .bind(&snapshot.account_type)
        .fetch_one(pool)
        .await?;

        Ok(row.get("id"))
    }

    /// 여러 포트폴리오 스냅샷 일괄 저장.
    ///
    /// UNNEST 패턴을 사용하여 한 번의 쿼리로 모든 스냅샷을 저장합니다.
    /// N+1 쿼리 문제를 해결합니다.
    pub async fn save_snapshots_batch(
        pool: &PgPool,
        snapshots: &[PortfolioSnapshot],
    ) -> Result<usize, sqlx::Error> {
        if snapshots.is_empty() {
            return Ok(0);
        }

        // 각 컬럼에 대한 배열 생성
        let credential_ids: Vec<Uuid> = snapshots.iter().map(|s| s.credential_id).collect();
        let snapshot_times: Vec<DateTime<Utc>> = snapshots
            .iter()
            .map(|s| {
                s.snapshot_time
                    .with_nanosecond(0)
                    .unwrap()
                    .with_second(0)
                    .unwrap()
            })
            .collect();
        let total_equities: Vec<Decimal> = snapshots.iter().map(|s| s.total_equity).collect();
        let cash_balances: Vec<Decimal> = snapshots.iter().map(|s| s.cash_balance).collect();
        let securities_values: Vec<Decimal> =
            snapshots.iter().map(|s| s.securities_value).collect();
        let total_pnls: Vec<Decimal> = snapshots.iter().map(|s| s.total_pnl).collect();
        let daily_pnls: Vec<Decimal> = snapshots.iter().map(|s| s.daily_pnl).collect();
        let currencies: Vec<String> = snapshots.iter().map(|s| s.currency.clone()).collect();
        let markets: Vec<String> = snapshots.iter().map(|s| s.market.clone()).collect();
        let account_types: Vec<Option<String>> =
            snapshots.iter().map(|s| s.account_type.clone()).collect();

        let result = sqlx::query(
            r#"
            INSERT INTO portfolio_equity_history (
                credential_id, snapshot_time, total_equity, cash_balance,
                securities_value, total_pnl, daily_pnl, currency, market, account_type
            )
            SELECT * FROM UNNEST(
                $1::uuid[], $2::timestamptz[], $3::numeric[], $4::numeric[],
                $5::numeric[], $6::numeric[], $7::numeric[], $8::text[], $9::text[], $10::text[]
            )
            ON CONFLICT (credential_id, snapshot_time)
            DO UPDATE SET
                total_equity = EXCLUDED.total_equity,
                cash_balance = EXCLUDED.cash_balance,
                securities_value = EXCLUDED.securities_value,
                total_pnl = EXCLUDED.total_pnl,
                daily_pnl = EXCLUDED.daily_pnl
            "#,
        )
        .bind(&credential_ids)
        .bind(&snapshot_times)
        .bind(&total_equities)
        .bind(&cash_balances)
        .bind(&securities_values)
        .bind(&total_pnls)
        .bind(&daily_pnls)
        .bind(&currencies)
        .bind(&markets)
        .bind(&account_types)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() as usize)
    }

    // ==================== 조회 ====================

    /// 특정 기간의 자산 곡선 데이터 조회 (특정 credential).
    ///
    /// Window functions를 사용하여 drawdown과 return을 효율적으로 계산합니다.
    pub async fn get_equity_curve(
        pool: &PgPool,
        credential_id: Uuid,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<EquityPoint>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            WITH ordered_data AS (
                SELECT
                    snapshot_time,
                    total_equity,
                    FIRST_VALUE(total_equity) OVER (ORDER BY snapshot_time) as initial_equity,
                    MAX(total_equity) OVER (ORDER BY snapshot_time ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) as peak_equity
                FROM portfolio_equity_history
                WHERE credential_id = $1
                  AND snapshot_time >= $2
                  AND snapshot_time <= $3
                ORDER BY snapshot_time
            )
            SELECT
                snapshot_time,
                total_equity,
                CASE
                    WHEN peak_equity > 0 THEN ((peak_equity - total_equity) / peak_equity * 100)
                    ELSE 0
                END as drawdown_pct,
                CASE
                    WHEN initial_equity > 0 THEN ((total_equity - initial_equity) / initial_equity * 100)
                    ELSE 0
                END as return_pct
            FROM ordered_data
            "#,
        )
        .bind(credential_id)
        .bind(start_time)
        .bind(end_time)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| EquityPoint {
                timestamp: r.get("snapshot_time"),
                equity: r.get("total_equity"),
                drawdown_pct: r.get("drawdown_pct"),
                return_pct: r.get("return_pct"),
            })
            .collect())
    }

    /// 모든 자격증명의 통합 자산 곡선 데이터 조회.
    ///
    /// 일별로 각 credential의 마지막 스냅샷을 합산하여 전체 포트폴리오를 계산합니다.
    pub async fn get_aggregated_equity_curve(
        pool: &PgPool,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<EquityPoint>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            WITH daily_totals AS (
                SELECT
                    date_trunc('day', snapshot_time) as day,
                    SUM(total_equity) as total_equity
                FROM (
                    SELECT DISTINCT ON (credential_id, date_trunc('day', snapshot_time))
                        credential_id,
                        snapshot_time,
                        total_equity
                    FROM portfolio_equity_history
                    WHERE snapshot_time >= $1
                      AND snapshot_time <= $2
                    ORDER BY credential_id, date_trunc('day', snapshot_time), snapshot_time DESC
                ) sub
                GROUP BY date_trunc('day', snapshot_time)
            ),
            ordered_data AS (
                SELECT
                    day as snapshot_time,
                    total_equity,
                    FIRST_VALUE(total_equity) OVER (ORDER BY day) as initial_equity,
                    MAX(total_equity) OVER (ORDER BY day ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) as peak_equity
                FROM daily_totals
                ORDER BY day
            )
            SELECT
                snapshot_time,
                total_equity,
                CASE
                    WHEN peak_equity > 0 THEN ((peak_equity - total_equity) / peak_equity * 100)
                    ELSE 0
                END as drawdown_pct,
                CASE
                    WHEN initial_equity > 0 THEN ((total_equity - initial_equity) / initial_equity * 100)
                    ELSE 0
                END as return_pct
            FROM ordered_data
            "#,
        )
        .bind(start_time)
        .bind(end_time)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| EquityPoint {
                timestamp: r.get("snapshot_time"),
                equity: r.get("total_equity"),
                drawdown_pct: r.get("drawdown_pct"),
                return_pct: r.get("return_pct"),
            })
            .collect())
    }

    /// 월별 수익률 조회.
    ///
    /// 각 월의 시가/종가를 기반으로 월별 수익률을 계산합니다.
    pub async fn get_monthly_returns(
        pool: &PgPool,
        credential_id: Option<Uuid>,
        years: i32,
    ) -> Result<Vec<MonthlyReturn>, sqlx::Error> {
        let start_time = Utc::now() - Duration::days(years as i64 * 365);

        let rows = if let Some(cred_id) = credential_id {
            sqlx::query(
                r#"
                WITH monthly_data AS (
                    SELECT
                        EXTRACT(YEAR FROM snapshot_time)::int as year,
                        EXTRACT(MONTH FROM snapshot_time)::int as month,
                        (array_agg(total_equity ORDER BY snapshot_time ASC))[1] as opening_equity,
                        (array_agg(total_equity ORDER BY snapshot_time DESC))[1] as closing_equity
                    FROM portfolio_equity_history
                    WHERE credential_id = $1
                      AND snapshot_time >= $2
                    GROUP BY EXTRACT(YEAR FROM snapshot_time), EXTRACT(MONTH FROM snapshot_time)
                )
                SELECT
                    year,
                    month,
                    opening_equity,
                    closing_equity,
                    CASE
                        WHEN opening_equity > 0 THEN
                            ((closing_equity - opening_equity) / opening_equity * 100)
                        ELSE 0
                    END as return_pct
                FROM monthly_data
                ORDER BY year, month
                "#,
            )
            .bind(cred_id)
            .bind(start_time)
            .fetch_all(pool)
            .await?
        } else {
            // 모든 credential 합산
            sqlx::query(
                r#"
                WITH daily_totals AS (
                    SELECT
                        date_trunc('day', snapshot_time) as day,
                        SUM(total_equity) as total_equity
                    FROM (
                        SELECT DISTINCT ON (credential_id, date_trunc('day', snapshot_time))
                            credential_id,
                            snapshot_time,
                            total_equity
                        FROM portfolio_equity_history
                        WHERE snapshot_time >= $1
                        ORDER BY credential_id, date_trunc('day', snapshot_time), snapshot_time DESC
                    ) sub
                    GROUP BY date_trunc('day', snapshot_time)
                ),
                monthly_data AS (
                    SELECT
                        EXTRACT(YEAR FROM day)::int as year,
                        EXTRACT(MONTH FROM day)::int as month,
                        (array_agg(total_equity ORDER BY day ASC))[1] as opening_equity,
                        (array_agg(total_equity ORDER BY day DESC))[1] as closing_equity
                    FROM daily_totals
                    GROUP BY EXTRACT(YEAR FROM day), EXTRACT(MONTH FROM day)
                )
                SELECT
                    year,
                    month,
                    opening_equity,
                    closing_equity,
                    CASE
                        WHEN opening_equity > 0 THEN
                            ((closing_equity - opening_equity) / opening_equity * 100)
                        ELSE 0
                    END as return_pct
                FROM monthly_data
                ORDER BY year, month
                "#,
            )
            .bind(start_time)
            .fetch_all(pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|r| MonthlyReturn {
                year: r.get("year"),
                month: r.get::<i32, _>("month") as u32,
                opening_equity: r.get("opening_equity"),
                closing_equity: r.get("closing_equity"),
                return_pct: r.get("return_pct"),
            })
            .collect())
    }

    /// 최신 스냅샷 조회.
    pub async fn get_latest_snapshot(
        pool: &PgPool,
        credential_id: Uuid,
    ) -> Result<Option<PortfolioSnapshot>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT
                credential_id,
                snapshot_time,
                total_equity,
                cash_balance,
                securities_value,
                total_pnl,
                daily_pnl,
                currency,
                market,
                account_type
            FROM portfolio_equity_history
            WHERE credential_id = $1
            ORDER BY snapshot_time DESC
            LIMIT 1
            "#,
        )
        .bind(credential_id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| PortfolioSnapshot {
            credential_id: r.get("credential_id"),
            snapshot_time: r.get("snapshot_time"),
            total_equity: r.get("total_equity"),
            cash_balance: r.get("cash_balance"),
            securities_value: r.get("securities_value"),
            total_pnl: r.get::<Option<Decimal>, _>("total_pnl").unwrap_or_default(),
            daily_pnl: r.get::<Option<Decimal>, _>("daily_pnl").unwrap_or_default(),
            currency: r
                .get::<Option<String>, _>("currency")
                .unwrap_or_else(|| "KRW".to_string()),
            market: r
                .get::<Option<String>, _>("market")
                .unwrap_or_else(|| "KR".to_string()),
            account_type: r.get("account_type"),
        }))
    }

    /// 초기 자본(첫 번째 스냅샷) 조회.
    pub async fn get_initial_capital(
        pool: &PgPool,
        credential_id: Option<Uuid>,
    ) -> Result<Decimal, sqlx::Error> {
        let result: Option<Decimal> = if let Some(cred_id) = credential_id {
            sqlx::query_scalar(
                r#"
                SELECT total_equity
                FROM portfolio_equity_history
                WHERE credential_id = $1
                ORDER BY snapshot_time ASC
                LIMIT 1
                "#,
            )
            .bind(cred_id)
            .fetch_optional(pool)
            .await?
        } else {
            sqlx::query_scalar(
                r#"
                SELECT SUM(total_equity) as total_equity
                FROM (
                    SELECT DISTINCT ON (credential_id)
                        credential_id,
                        total_equity
                    FROM portfolio_equity_history
                    ORDER BY credential_id, snapshot_time ASC
                ) sub
                "#,
            )
            .fetch_optional(pool)
            .await?
        };

        Ok(result.unwrap_or_default())
    }

    /// 최고점 자산 조회 (MDD 계산용).
    pub async fn get_peak_equity(
        pool: &PgPool,
        credential_id: Option<Uuid>,
    ) -> Result<Decimal, sqlx::Error> {
        let result: Option<Decimal> = if let Some(cred_id) = credential_id {
            sqlx::query_scalar(
                r#"
                SELECT MAX(total_equity)
                FROM portfolio_equity_history
                WHERE credential_id = $1
                "#,
            )
            .bind(cred_id)
            .fetch_optional(pool)
            .await?
        } else {
            sqlx::query_scalar(
                r#"
                SELECT MAX(daily_total)
                FROM (
                    SELECT
                        date_trunc('day', snapshot_time) as day,
                        SUM(total_equity) as daily_total
                    FROM (
                        SELECT DISTINCT ON (credential_id, date_trunc('day', snapshot_time))
                            credential_id,
                            snapshot_time,
                            total_equity
                        FROM portfolio_equity_history
                        ORDER BY credential_id, date_trunc('day', snapshot_time), snapshot_time DESC
                    ) sub
                    GROUP BY date_trunc('day', snapshot_time)
                ) totals
                "#,
            )
            .fetch_optional(pool)
            .await?
        };

        Ok(result.unwrap_or_default())
    }

    /// 스냅샷 개수 조회.
    pub async fn get_snapshot_count(
        pool: &PgPool,
        credential_id: Option<Uuid>,
    ) -> Result<i64, sqlx::Error> {
        let count: Option<i64> = if let Some(cred_id) = credential_id {
            sqlx::query_scalar(
                r#"SELECT COUNT(*) FROM portfolio_equity_history WHERE credential_id = $1"#,
            )
            .bind(cred_id)
            .fetch_one(pool)
            .await?
        } else {
            sqlx::query_scalar(r#"SELECT COUNT(*) FROM portfolio_equity_history"#)
                .fetch_one(pool)
                .await?
        };

        Ok(count.unwrap_or(0))
    }

    // ==================== 거래소 데이터 동기화 ====================

    /// 거래소 체결 내역으로 자산 곡선 복원.
    ///
    /// 체결 내역을 기반으로 일별 자산 변동을 계산하여 자산 곡선을 생성합니다.
    /// 현재 자산에서 역순으로 거래 금액을 적용하여 과거 자산을 추정합니다.
    pub async fn sync_from_executions(
        pool: &PgPool,
        credential_id: Uuid,
        executions: Vec<ExecutionForSync>,
        current_equity: Decimal,
        currency: &str,
        market: &str,
        account_type: Option<&str>,
    ) -> Result<usize, sqlx::Error> {
        if executions.is_empty() {
            return Ok(0);
        }

        // 1. 일별 순손익 집계
        let mut daily_pnl: BTreeMap<NaiveDate, Decimal> = BTreeMap::new();

        for exec in &executions {
            let date = exec.execution_time.date_naive();
            let pnl = if exec.is_buy {
                -exec.amount // 매수: 현금 감소
            } else {
                exec.amount // 매도: 현금 증가 (실현 손익 포함)
            };

            *daily_pnl.entry(date).or_insert(Decimal::ZERO) += pnl;
        }

        // 2. 역순으로 일별 자산 계산 (오늘부터 과거로)
        let mut daily_equity: Vec<(NaiveDate, Decimal)> = Vec::new();
        let today = Utc::now().date_naive();
        let mut equity = current_equity;

        // 오늘 자산 추가
        daily_equity.push((today, equity));

        // 과거 날짜들의 자산 계산 (역순)
        let dates: Vec<_> = daily_pnl.keys().cloned().collect();
        for date in dates.into_iter().rev() {
            if date >= today {
                continue;
            }

            // 해당 날짜의 순손익을 역으로 적용
            if let Some(&pnl) = daily_pnl.get(&date) {
                equity -= pnl; // 역산
            }

            daily_equity.push((date, equity));
        }

        // 시간순으로 정렬
        daily_equity.sort_by_key(|(d, _)| *d);

        // 3. 스냅샷 배열 생성 및 일괄 저장
        let snapshots: Vec<PortfolioSnapshot> = daily_equity
            .iter()
            .map(|(date, eq)| {
                let snapshot_time = Utc.from_utc_datetime(&date.and_hms_opt(12, 0, 0).unwrap());

                PortfolioSnapshot {
                    credential_id,
                    snapshot_time,
                    total_equity: *eq,
                    cash_balance: *eq,
                    securities_value: Decimal::ZERO,
                    total_pnl: *eq - current_equity,
                    daily_pnl: daily_pnl.get(date).cloned().unwrap_or(Decimal::ZERO),
                    currency: currency.to_string(),
                    market: market.to_string(),
                    account_type: account_type.map(|s| s.to_string()),
                }
            })
            .collect();

        let saved_count = match Self::save_snapshots_batch(pool, &snapshots).await {
            Ok(count) => count,
            Err(e) => {
                warn!("스냅샷 배치 저장 실패: {}", e);
                0
            }
        };

        info!(
            "{} 체결 내역에서 {} 일별 자산 포인트 동기화 완료 (credential: {})",
            executions.len(),
            saved_count,
            credential_id
        );

        Ok(saved_count)
    }

    /// 체결 기록과 일별 종가를 기반으로 정확한 자산 곡선 계산.
    ///
    /// 1. 체결 기록에서 일별 보유 수량 추적 (누적)
    /// 2. ohlcv 테이블에서 일별 종가 조회
    /// 3. 일별 자산 = Σ(보유 수량 × 종가) + 현금 잔고
    pub async fn sync_with_market_prices(
        pool: &PgPool,
        credential_id: Uuid,
        _current_cash: Decimal, // 주식 가치만 추적
        currency: &str,
        market: &str,
        account_type: Option<&str>,
    ) -> Result<usize, sqlx::Error> {
        // 1. 체결 기록 조회 (시간순 정렬)
        let executions = sqlx::query(
            r#"
            SELECT executed_at, symbol, side, quantity, price, amount
            FROM execution_cache
            WHERE credential_id = $1
            ORDER BY executed_at ASC
            "#,
        )
        .bind(credential_id)
        .fetch_all(pool)
        .await?;

        if executions.is_empty() {
            info!("체결 기록 없음 (credential: {})", credential_id);
            return Ok(0);
        }

        // 2. 모든 체결 기록을 처리하여 최종 보유 수량 계산
        let mut all_holdings: HashMap<String, Decimal> = HashMap::new();
        for row in &executions {
            let symbol: String = row.get("symbol");
            let side: String = row.get("side");
            let quantity: Decimal = row.get("quantity");

            let current_qty = all_holdings.entry(symbol).or_insert(Decimal::ZERO);
            if side == "buy" {
                *current_qty += quantity;
            } else {
                *current_qty -= quantity;
            }
        }

        // 3. 현재 보유 중인 심볼만 필터링 (수량 > 0)
        let active_symbols: Vec<String> = all_holdings
            .iter()
            .filter(|(_, qty)| **qty > Decimal::ZERO)
            .map(|(symbol, _)| symbol.clone())
            .collect();

        if active_symbols.is_empty() {
            info!("활성 포지션 없음 (credential: {})", credential_id);
            return Ok(0);
        }

        info!(
            "{} 활성 포지션 추적 중: {:?}",
            active_symbols.len(),
            active_symbols
        );

        // 4. 현재 보유 심볼의 체결 기록만 필터링하여 일별 보유 수량 추적
        let mut holdings: HashMap<String, Decimal> = HashMap::new();
        let mut daily_holdings: BTreeMap<NaiveDate, HashMap<String, Decimal>> = BTreeMap::new();
        let mut first_active_date: Option<NaiveDate> = None;

        for row in &executions {
            let executed_at: DateTime<Utc> = row.get("executed_at");
            let symbol: String = row.get("symbol");
            let side: String = row.get("side");
            let quantity: Decimal = row.get("quantity");
            let date = executed_at.date_naive();

            // 현재 보유 중인 심볼만 처리
            if !active_symbols.contains(&symbol) {
                continue;
            }

            // 보유 수량 업데이트
            let current_qty = holdings.entry(symbol).or_insert(Decimal::ZERO);
            if side == "buy" {
                *current_qty += quantity;
            } else {
                *current_qty -= quantity;
            }

            daily_holdings.insert(date, holdings.clone());

            if first_active_date.is_none() {
                first_active_date = Some(date);
            }
        }

        let start_date = first_active_date.unwrap_or_else(|| Utc::now().date_naive());
        let end_date = Utc::now().date_naive();

        info!(
            "자산 곡선 계산 기간: {} ~ {} (활성 포지션만)",
            start_date, end_date
        );

        // 5. 현재 보유 심볼의 일별 종가 조회 (배치 쿼리)
        let mut price_cache: HashMap<(String, NaiveDate), Decimal> = HashMap::new();

        // Yahoo Finance 형식으로 심볼 변환 맵 생성
        let symbol_to_yahoo: HashMap<String, String> = active_symbols
            .iter()
            .map(|s| {
                let yahoo = if s.len() == 6 && s.chars().all(|c| c.is_ascii_digit()) {
                    format!("{}.KS", s)
                } else {
                    s.clone()
                };
                (s.clone(), yahoo)
            })
            .collect();

        let yahoo_to_symbol: HashMap<String, String> = symbol_to_yahoo
            .iter()
            .map(|(k, v)| (v.clone(), k.clone()))
            .collect();

        let yahoo_symbols: Vec<String> = symbol_to_yahoo.values().cloned().collect();

        // 모든 심볼을 한 번의 쿼리로 조회 (ANY 패턴 사용)
        let prices = sqlx::query(
            r#"
            SELECT symbol, DATE(open_time) as date, close
            FROM ohlcv
            WHERE symbol = ANY($1::text[]) AND timeframe = '1d'
            ORDER BY symbol, open_time
            "#,
        )
        .bind(&yahoo_symbols)
        .fetch_all(pool)
        .await?;

        for row in prices {
            let yahoo_symbol: String = row.get("symbol");
            let date: NaiveDate = row.get("date");
            let close: Decimal = row.get("close");

            if let Some(original_symbol) = yahoo_to_symbol.get(&yahoo_symbol) {
                price_cache.insert((original_symbol.clone(), date), close);
            }
        }

        info!(
            "{} 심볼에 대해 {} 가격 포인트 로드 완료 (단일 배치 쿼리)",
            active_symbols.len(),
            price_cache.len()
        );

        // 6. 일별 자산 계산 및 저장 (주식 가치만)
        let mut active_holdings: HashMap<String, Decimal> = HashMap::new();
        let mut saved_count = 0;
        let mut prev_equity = Decimal::ZERO;
        let mut initial_equity = Decimal::ZERO;
        let mut is_first = true;

        let mut current_date = start_date;
        while current_date <= end_date {
            if let Some(holdings_snapshot) = daily_holdings.get(&current_date) {
                active_holdings = holdings_snapshot.clone();
            }

            let mut securities_value = Decimal::ZERO;
            let mut has_all_prices = true;

            for (symbol, qty) in &active_holdings {
                if *qty > Decimal::ZERO {
                    let mut price = None;
                    let mut lookup_date = current_date;
                    for _ in 0..7 {
                        // 최대 7일 전까지 탐색 (주말/공휴일)
                        if let Some(p) = price_cache.get(&(symbol.clone(), lookup_date)) {
                            price = Some(*p);
                            break;
                        }
                        lookup_date = lookup_date.pred_opt().unwrap_or(lookup_date);
                    }

                    if let Some(p) = price {
                        securities_value += *qty * p;
                    } else {
                        has_all_prices = false;
                        warn!("{} 종가 없음: {} (보유: {})", current_date, symbol, qty);
                    }
                }
            }

            // 모든 종가가 있고 보유 종목이 있는 날만 저장
            if has_all_prices && securities_value > Decimal::ZERO {
                if is_first {
                    initial_equity = securities_value;
                    prev_equity = securities_value;
                    is_first = false;
                }

                let total_equity = securities_value;
                let daily_pnl = total_equity - prev_equity;

                let snapshot_time =
                    Utc.from_utc_datetime(&current_date.and_hms_opt(12, 0, 0).unwrap());

                let snapshot = PortfolioSnapshot {
                    credential_id,
                    snapshot_time,
                    total_equity,
                    cash_balance: Decimal::ZERO,
                    securities_value,
                    total_pnl: total_equity - initial_equity,
                    daily_pnl,
                    currency: currency.to_string(),
                    market: market.to_string(),
                    account_type: account_type.map(|s| s.to_string()),
                };

                match Self::save_snapshot(pool, &snapshot).await {
                    Ok(_) => {
                        saved_count += 1;
                        prev_equity = total_equity;
                    }
                    Err(e) => {
                        warn!("{} 스냅샷 저장 실패: {}", current_date, e);
                    }
                }
            }

            current_date = current_date.succ_opt().unwrap_or(current_date);
        }

        info!(
            "{} 일별 자산 포인트 동기화 완료 (credential: {}, initial_equity={}, active_symbols={:?})",
            saved_count, credential_id, initial_equity, active_symbols
        );

        Ok(saved_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_portfolio_snapshot() {
        let snapshot = PortfolioSnapshot {
            credential_id: Uuid::new_v4(),
            snapshot_time: Utc::now(),
            total_equity: dec!(10_000_000),
            cash_balance: dec!(5_000_000),
            securities_value: dec!(5_000_000),
            total_pnl: dec!(500_000),
            daily_pnl: dec!(10_000),
            currency: "KRW".to_string(),
            market: "KR".to_string(),
            account_type: Some("real".to_string()),
        };

        assert_eq!(snapshot.total_equity, dec!(10_000_000));
    }

    #[test]
    fn test_equity_point() {
        let point = EquityPoint {
            timestamp: Utc::now(),
            equity: dec!(10_000_000),
            drawdown_pct: dec!(5.5),
            return_pct: dec!(10.0),
        };

        assert_eq!(point.return_pct, dec!(10.0));
    }
}
