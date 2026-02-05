//! KRX API 및 네이버 금융을 활용한 Fundamental 데이터 수집 모듈.
//!
//! ## 데이터 소스
//!
//! ### KRX OPEN API (인증 필요)
//! - 가치 지표: PER, PBR, 배당수익률, EPS, BPS
//! - 시가총액, 상장주식수
//! - 섹터 정보 업데이트
//!
//! ### 네이버 금융 크롤러 (인증 불필요)
//! - 가치 지표: PER, PBR, ROE, EPS, BPS, 배당수익률
//! - 시가총액, 52주 고저
//! - 섹터, 시장 구분 (KOSPI/KOSDAQ/ETF)
//! - 외국인 소진율

use chrono::Utc;
use sqlx::PgPool;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, info, warn};
use uuid::Uuid;

use trader_core::CredentialEncryptor;
use trader_data::provider::krx_api::{KrxApiClient, KrxDailyTrade, KrxValuation};
use trader_data::provider::naver::{NaverFinanceFetcher, NaverFundamentalData};

use super::checkpoint::{self, CheckpointStatus};
use crate::{config::FundamentalCollectConfig, error::CollectorError, Result};

/// Fundamental 동기화 통계.
#[derive(Debug, Default)]
pub struct FundamentalSyncStats {
    /// 처리된 종목 수
    pub processed: usize,
    /// PER/PBR 업데이트된 종목 수
    pub valuation_updated: usize,
    /// 시가총액 업데이트된 종목 수
    pub market_cap_updated: usize,
    /// 섹터 업데이트된 종목 수
    pub sector_updated: usize,
    /// 52주 고저 업데이트된 종목 수
    pub week_52_updated: usize,
    /// 시장 타입(KOSPI/KOSDAQ/ETF) 업데이트된 종목 수
    pub market_type_updated: usize,
    /// 실패 수
    pub failed: usize,
    /// 데이터 소스
    pub data_source: String,
}

/// KRX fundamental 데이터 동기화.
///
/// KOSPI/KOSDAQ 종목의 가치 지표, 시가총액, 섹터 정보를 KRX API에서 수집하여
/// symbol_fundamental 및 symbol_info 테이블에 저장합니다.
pub async fn sync_krx_fundamentals(
    pool: &PgPool,
    config: &FundamentalCollectConfig,
) -> Result<FundamentalSyncStats> {
    info!("KRX Fundamental 데이터 동기화 시작");

    // KRX API 클라이언트 생성
    let master_key = match std::env::var("ENCRYPTION_MASTER_KEY") {
        Ok(key) => key,
        Err(_) => {
            warn!("ENCRYPTION_MASTER_KEY 환경변수가 설정되지 않았습니다. 동기화를 건너뜁니다.");
            return Ok(FundamentalSyncStats::default());
        }
    };

    let encryptor = CredentialEncryptor::new(&master_key)
        .map_err(|e| CollectorError::DataSource(format!("암호화키 로드 실패: {}", e)))?;

    let client = match KrxApiClient::from_credential(pool, &encryptor).await {
        Ok(Some(client)) => client,
        Ok(None) => {
            warn!("KRX API credential이 등록되지 않았습니다. 동기화를 건너뜁니다.");
            return Ok(FundamentalSyncStats::default());
        }
        Err(e) => {
            return Err(CollectorError::DataSource(format!(
                "KRX API 클라이언트 생성 실패: {}",
                e
            )))
        }
    };

    let today = Utc::now().format("%Y%m%d").to_string();
    let mut stats = FundamentalSyncStats::default();

    // 1. 가치 지표 수집 (PER, PBR, 배당수익률, EPS, BPS)
    info!("가치 지표 수집 중 (PER, PBR, 배당수익률)...");
    let valuation_stats = sync_valuation(pool, &client, &today, config).await?;
    stats.valuation_updated = valuation_stats;

    // API 호출 간 딜레이
    tokio::time::sleep(config.request_delay()).await;

    // 2. 일별 매매정보에서 시가총액, 섹터 정보 수집
    info!("시가총액 및 섹터 정보 수집 중...");
    let (market_cap_stats, sector_stats) = sync_market_data(pool, &client, &today, config).await?;
    stats.market_cap_updated = market_cap_stats;
    stats.sector_updated = sector_stats;

    stats.processed = stats.valuation_updated + stats.market_cap_updated;

    info!(
        processed = stats.processed,
        valuation = stats.valuation_updated,
        market_cap = stats.market_cap_updated,
        sector = stats.sector_updated,
        failed = stats.failed,
        "KRX Fundamental 데이터 동기화 완료"
    );

    Ok(stats)
}

/// 가치 지표(PER, PBR, 배당수익률, EPS, BPS) 동기화.
async fn sync_valuation(
    pool: &PgPool,
    client: &KrxApiClient,
    base_date: &str,
    _config: &FundamentalCollectConfig,
) -> Result<usize> {
    // KOSPI 가치 지표 조회
    let kospi_valuation = match client.fetch_valuation(base_date, "STK").await {
        Ok(v) => v,
        Err(e) => {
            warn!(error = %e, "KOSPI 가치 지표 조회 실패");
            Vec::new()
        }
    };

    // API 호출 간 딜레이
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // KOSDAQ 가치 지표 조회
    let kosdaq_valuation = match client.fetch_valuation(base_date, "KSQ").await {
        Ok(v) => v,
        Err(e) => {
            warn!(error = %e, "KOSDAQ 가치 지표 조회 실패");
            Vec::new()
        }
    };

    let all_valuations: Vec<KrxValuation> = kospi_valuation
        .into_iter()
        .chain(kosdaq_valuation.into_iter())
        .collect();

    info!(count = all_valuations.len(), "가치 지표 데이터 조회 완료");

    // DB에 저장
    let mut updated = 0;
    for valuation in &all_valuations {
        if let Err(e) = upsert_valuation(pool, valuation).await {
            debug!(ticker = %valuation.ticker, error = %e, "가치 지표 저장 실패");
        } else {
            updated += 1;
        }
    }

    Ok(updated)
}

/// 가치 지표를 symbol_fundamental 테이블에 저장 (Upsert).
async fn upsert_valuation(pool: &PgPool, valuation: &KrxValuation) -> Result<()> {
    // symbol_info에서 ID 조회
    let symbol_info: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT id
        FROM symbol_info
        WHERE ticker = $1 AND market = 'KR' AND is_active = true
        LIMIT 1
        "#,
    )
    .bind(&valuation.ticker)
    .fetch_optional(pool)
    .await?;

    let symbol_info_id = match symbol_info {
        Some((id,)) => id,
        None => return Ok(()), // 심볼이 없으면 건너뜀
    };

    // symbol_fundamental에 Upsert
    sqlx::query(
        r#"
        INSERT INTO symbol_fundamental (
            symbol_info_id, per, pbr, dividend_yield, eps, bps,
            data_source, currency, fetched_at, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, 'KRX', 'KRW', NOW(), NOW())
        ON CONFLICT (symbol_info_id)
        DO UPDATE SET
            per = COALESCE(EXCLUDED.per, symbol_fundamental.per),
            pbr = COALESCE(EXCLUDED.pbr, symbol_fundamental.pbr),
            dividend_yield = COALESCE(EXCLUDED.dividend_yield, symbol_fundamental.dividend_yield),
            eps = COALESCE(EXCLUDED.eps, symbol_fundamental.eps),
            bps = COALESCE(EXCLUDED.bps, symbol_fundamental.bps),
            data_source = 'KRX',
            fetched_at = NOW(),
            updated_at = NOW()
        "#,
    )
    .bind(symbol_info_id)
    .bind(valuation.per)
    .bind(valuation.pbr)
    .bind(valuation.dividend_yield)
    .bind(valuation.eps)
    .bind(valuation.bps)
    .execute(pool)
    .await?;

    Ok(())
}

/// 시가총액 및 섹터 정보 동기화.
async fn sync_market_data(
    pool: &PgPool,
    client: &KrxApiClient,
    base_date: &str,
    _config: &FundamentalCollectConfig,
) -> Result<(usize, usize)> {
    // KOSPI 일별 매매정보 조회
    let kospi_trades = match client.fetch_kospi_daily_trades(base_date).await {
        Ok(t) => t,
        Err(e) => {
            warn!(error = %e, "KOSPI 일별 매매정보 조회 실패");
            Vec::new()
        }
    };

    // API 호출 간 딜레이
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // KOSDAQ 일별 매매정보 조회
    let kosdaq_trades = match client.fetch_kosdaq_daily_trades(base_date).await {
        Ok(t) => t,
        Err(e) => {
            warn!(error = %e, "KOSDAQ 일별 매매정보 조회 실패");
            Vec::new()
        }
    };

    let all_trades: Vec<KrxDailyTrade> = kospi_trades
        .into_iter()
        .chain(kosdaq_trades.into_iter())
        .collect();

    info!(count = all_trades.len(), "일별 매매정보 조회 완료");

    // DB에 저장
    let mut market_cap_updated = 0;
    let mut sector_updated = 0;

    for trade in &all_trades {
        // 시가총액 업데이트
        if let Err(e) = upsert_market_cap(pool, trade).await {
            debug!(ticker = %trade.code, error = %e, "시가총액 저장 실패");
        } else {
            market_cap_updated += 1;
        }

        // 섹터 정보 업데이트
        if let Some(sector) = &trade.sector {
            if !sector.is_empty() {
                if let Err(e) = update_sector(pool, &trade.code, sector).await {
                    debug!(ticker = %trade.code, error = %e, "섹터 업데이트 실패");
                } else {
                    sector_updated += 1;
                }
            }
        }
    }

    Ok((market_cap_updated, sector_updated))
}

/// 시가총액 및 상장주식수를 symbol_fundamental 테이블에 저장.
async fn upsert_market_cap(pool: &PgPool, trade: &KrxDailyTrade) -> Result<()> {
    // 종목코드에서 티커 추출 (KR7005930003 → 005930)
    let ticker = extract_ticker(&trade.code);

    // symbol_info에서 ID 조회
    let symbol_info: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT id
        FROM symbol_info
        WHERE ticker = $1 AND market = 'KR' AND is_active = true
        LIMIT 1
        "#,
    )
    .bind(&ticker)
    .fetch_optional(pool)
    .await?;

    let symbol_info_id = match symbol_info {
        Some((id,)) => id,
        None => return Ok(()), // 심볼이 없으면 건너뜀
    };

    // symbol_fundamental에 시가총액 Upsert
    sqlx::query(
        r#"
        INSERT INTO symbol_fundamental (
            symbol_info_id, market_cap, shares_outstanding,
            data_source, currency, fetched_at, updated_at
        )
        VALUES ($1, $2, $3, 'KRX', 'KRW', NOW(), NOW())
        ON CONFLICT (symbol_info_id)
        DO UPDATE SET
            market_cap = COALESCE(EXCLUDED.market_cap, symbol_fundamental.market_cap),
            shares_outstanding = COALESCE(EXCLUDED.shares_outstanding, symbol_fundamental.shares_outstanding),
            fetched_at = NOW(),
            updated_at = NOW()
        "#,
    )
    .bind(symbol_info_id)
    .bind(trade.market_cap)
    .bind(trade.shares_outstanding)
    .execute(pool)
    .await
    ?;

    Ok(())
}

/// 섹터 정보를 symbol_info 테이블에 업데이트.
async fn update_sector(pool: &PgPool, code: &str, sector: &str) -> Result<()> {
    let ticker = extract_ticker(code);

    sqlx::query(
        r#"
        UPDATE symbol_info
        SET sector = $2, updated_at = NOW()
        WHERE ticker = $1 AND market = 'KR'
        "#,
    )
    .bind(&ticker)
    .bind(sector)
    .execute(pool)
    .await?;

    Ok(())
}

/// KRX 종목코드에서 티커 추출.
///
/// KR7005930003 → 005930
/// 005930 → 005930 (이미 티커인 경우 그대로 반환)
fn extract_ticker(code: &str) -> String {
    // KR7XXXXXX003 형식에서 6자리 티커 추출
    if code.len() == 12 && code.starts_with("KR") {
        code[3..9].to_string()
    } else if code.len() == 6 {
        code.to_string()
    } else {
        // 기타 형식은 그대로 반환
        code.to_string()
    }
}

/// 섹터별 통계 조회.
pub async fn get_sector_statistics(pool: &PgPool) -> Result<HashMap<String, usize>> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT
            COALESCE(sector, '미분류') as sector,
            COUNT(*) as count
        FROM symbol_info
        WHERE market = 'KR' AND is_active = true
        GROUP BY sector
        ORDER BY count DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let stats: HashMap<String, usize> = rows
        .into_iter()
        .map(|(sector, count)| (sector, count as usize))
        .collect();

    Ok(stats)
}

// ==================== 네이버 금융 크롤러 ====================

/// 네이버 금융을 통한 KR 시장 fundamental 데이터 동기화.
///
/// KRX API 인증 없이 네이버 금융 크롤링을 통해 데이터를 수집합니다.
/// 수집 항목:
/// - PER, PBR, ROE, EPS, BPS, 배당수익률
/// - 시가총액, 52주 고저
/// - 섹터, 시장 타입 (KOSPI/KOSDAQ/ETF)
/// - 외국인 소진율
///
/// # Arguments
/// * `pool` - DB 연결 풀
/// * `request_delay_ms` - 요청 간 딜레이 (밀리초)
/// * `batch_size` - 배치당 처리할 심볼 수 (None이면 전체)
///
/// 네이버 Fundamental 동기화 옵션
#[derive(Debug, Default)]
pub struct NaverSyncOptions {
    /// 요청 간 딜레이 (ms)
    pub request_delay_ms: u64,
    /// 배치 크기 (None이면 전체)
    pub batch_size: Option<i64>,
    /// 중단점부터 재개
    pub resume: bool,
    /// N시간 이내 업데이트된 심볼 스킵
    pub stale_hours: Option<u32>,
}

pub async fn sync_naver_fundamentals(
    pool: &PgPool,
    request_delay_ms: u64,
    batch_size: Option<i64>,
) -> Result<FundamentalSyncStats> {
    // 기본 옵션으로 호출
    let options = NaverSyncOptions {
        request_delay_ms,
        batch_size,
        resume: false,
        stale_hours: None,
    };
    sync_naver_fundamentals_with_options(pool, options).await
}

/// 네이버 금융을 통한 KR 시장 fundamental 데이터 동기화 (옵션 포함).
///
/// - `resume`: true면 이전 중단점부터 재개
/// - `stale_hours`: 지정 시 해당 시간 이내 업데이트된 심볼 스킵
pub async fn sync_naver_fundamentals_with_options(
    pool: &PgPool,
    options: NaverSyncOptions,
) -> Result<FundamentalSyncStats> {
    info!("네이버 금융 Fundamental 데이터 동기화 시작");

    let mut stats = FundamentalSyncStats {
        data_source: "NAVER".to_string(),
        ..Default::default()
    };

    // 체크포인트 로드 (resume 모드)
    let resume_ticker = if options.resume {
        match checkpoint::load_checkpoint(pool, "naver_fundamental").await? {
            Some(t) => {
                info!(last_ticker = %t, "중단점부터 재개");
                Some(t)
            }
            None => {
                info!("이전 중단점 없음, 처음부터 시작");
                None
            }
        }
    } else {
        None
    };

    // KR 시장 활성 심볼 조회 (stale_hours 조건 포함)
    let limit = options.batch_size.unwrap_or(i64::MAX);
    let stale_condition = if let Some(hours) = options.stale_hours {
        format!(
            "AND (sf.updated_at IS NULL OR sf.updated_at < NOW() - INTERVAL '{} hours')",
            hours
        )
    } else {
        String::new()
    };

    let resume_condition = if let Some(ref t) = resume_ticker {
        format!("AND si.ticker > '{}'", t)
    } else {
        String::new()
    };

    let query = format!(
        r#"
        SELECT si.id, si.ticker
        FROM symbol_info si
        LEFT JOIN symbol_fundamental sf ON si.id = sf.symbol_info_id
        WHERE si.market = 'KR' AND si.is_active = true
          AND si.symbol_type IN ('STOCK', 'ETF')
          {} {}
        ORDER BY si.ticker
        LIMIT $1
        "#,
        stale_condition, resume_condition
    );

    let symbols: Vec<(Uuid, String)> = sqlx::query_as(&query).bind(limit).fetch_all(pool).await?;

    let total = symbols.len();

    if options.stale_hours.is_some() {
        info!(count = total, stale_hours = ?options.stale_hours, "업데이트 필요한 심볼 조회 완료");
    } else {
        info!(count = total, "KR 심볼 조회 완료");
    }

    if symbols.is_empty() {
        // 완료 상태로 저장
        checkpoint::save_checkpoint(
            pool,
            "naver_fundamental",
            "",
            0,
            CheckpointStatus::Completed,
        )
        .await?;
        return Ok(stats);
    }

    // 시작 상태 저장
    checkpoint::save_checkpoint(pool, "naver_fundamental", "", 0, CheckpointStatus::Running)
        .await?;

    // 네이버 금융 크롤러 초기화
    let fetcher = NaverFinanceFetcher::with_delay(Duration::from_millis(options.request_delay_ms));

    for (idx, (symbol_info_id, ticker)) in symbols.iter().enumerate() {
        stats.processed += 1;

        if (idx + 1) % 100 == 0 || idx + 1 == total {
            info!(
                progress = format!("{}/{}", idx + 1, total),
                "네이버 Fundamental 수집 진행 중"
            );
            // 체크포인트 저장 (100개마다)
            checkpoint::save_checkpoint(
                pool,
                "naver_fundamental",
                ticker,
                stats.processed as i32,
                CheckpointStatus::Running,
            )
            .await?;
        }

        // 네이버 금융에서 데이터 수집
        match fetcher.fetch_fundamental(ticker).await {
            Ok(data) => {
                // DB에 저장
                if let Err(e) = upsert_naver_fundamental(pool, *symbol_info_id, &data).await {
                    debug!(ticker = ticker, error = %e, "네이버 데이터 저장 실패");
                    stats.failed += 1;
                } else {
                    // 업데이트된 항목 카운트
                    if data.per.is_some() || data.pbr.is_some() {
                        stats.valuation_updated += 1;
                    }
                    if data.market_cap.is_some() {
                        stats.market_cap_updated += 1;
                    }
                    if data.sector.is_some() {
                        stats.sector_updated += 1;
                    }
                    if data.week_52_high.is_some() || data.week_52_low.is_some() {
                        stats.week_52_updated += 1;
                    }
                }

                // 시장 타입 업데이트 (KOSPI/KOSDAQ/ETF)
                if let Err(e) =
                    update_market_type(pool, *symbol_info_id, &data.market_type.to_string()).await
                {
                    debug!(ticker = ticker, error = %e, "시장 타입 업데이트 실패");
                } else {
                    stats.market_type_updated += 1;
                }
            }
            Err(e) => {
                // Rate limit 에러는 경고, 나머지는 debug
                if matches!(e, trader_data::provider::naver::NaverError::RateLimited) {
                    warn!(ticker = ticker, "네이버 Rate limit 초과 - 잠시 대기");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                } else {
                    debug!(ticker = ticker, error = %e, "네이버 데이터 수집 실패");
                }
                stats.failed += 1;
            }
        }

        // 요청 간 딜레이 (마지막 항목이 아닐 때만)
        if idx + 1 < total {
            tokio::time::sleep(Duration::from_millis(options.request_delay_ms)).await;
        }
    }

    // 완료 상태 저장
    checkpoint::save_checkpoint(
        pool,
        "naver_fundamental",
        "",
        stats.processed as i32,
        CheckpointStatus::Completed,
    )
    .await?;

    info!(
        processed = stats.processed,
        valuation = stats.valuation_updated,
        market_cap = stats.market_cap_updated,
        sector = stats.sector_updated,
        week_52 = stats.week_52_updated,
        market_type = stats.market_type_updated,
        failed = stats.failed,
        "네이버 금융 Fundamental 데이터 동기화 완료"
    );

    Ok(stats)
}

/// 네이버 금융 데이터를 symbol_fundamental 테이블에 저장 (Upsert).
async fn upsert_naver_fundamental(
    pool: &PgPool,
    symbol_info_id: Uuid,
    data: &NaverFundamentalData,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO symbol_fundamental (
            symbol_info_id, market_cap, per, pbr, psr, roe, eps, bps,
            dividend_yield, week_52_high, week_52_low, foreign_ratio,
            revenue, operating_income, net_income,
            revenue_growth_yoy, earnings_growth_yoy,
            roa, operating_margin, debt_ratio, current_ratio, quick_ratio,
            data_source, currency, fetched_at, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, 'NAVER', 'KRW', NOW(), NOW())
        ON CONFLICT (symbol_info_id)
        DO UPDATE SET
            market_cap = COALESCE(EXCLUDED.market_cap, symbol_fundamental.market_cap),
            per = COALESCE(EXCLUDED.per, symbol_fundamental.per),
            pbr = COALESCE(EXCLUDED.pbr, symbol_fundamental.pbr),
            psr = COALESCE(EXCLUDED.psr, symbol_fundamental.psr),
            roe = COALESCE(EXCLUDED.roe, symbol_fundamental.roe),
            eps = COALESCE(EXCLUDED.eps, symbol_fundamental.eps),
            bps = COALESCE(EXCLUDED.bps, symbol_fundamental.bps),
            dividend_yield = COALESCE(EXCLUDED.dividend_yield, symbol_fundamental.dividend_yield),
            week_52_high = COALESCE(EXCLUDED.week_52_high, symbol_fundamental.week_52_high),
            week_52_low = COALESCE(EXCLUDED.week_52_low, symbol_fundamental.week_52_low),
            foreign_ratio = COALESCE(EXCLUDED.foreign_ratio, symbol_fundamental.foreign_ratio),
            revenue = COALESCE(EXCLUDED.revenue, symbol_fundamental.revenue),
            operating_income = COALESCE(EXCLUDED.operating_income, symbol_fundamental.operating_income),
            net_income = COALESCE(EXCLUDED.net_income, symbol_fundamental.net_income),
            revenue_growth_yoy = COALESCE(EXCLUDED.revenue_growth_yoy, symbol_fundamental.revenue_growth_yoy),
            earnings_growth_yoy = COALESCE(EXCLUDED.earnings_growth_yoy, symbol_fundamental.earnings_growth_yoy),
            roa = COALESCE(EXCLUDED.roa, symbol_fundamental.roa),
            operating_margin = COALESCE(EXCLUDED.operating_margin, symbol_fundamental.operating_margin),
            debt_ratio = COALESCE(EXCLUDED.debt_ratio, symbol_fundamental.debt_ratio),
            current_ratio = COALESCE(EXCLUDED.current_ratio, symbol_fundamental.current_ratio),
            quick_ratio = COALESCE(EXCLUDED.quick_ratio, symbol_fundamental.quick_ratio),
            data_source = 'NAVER',
            fetched_at = NOW(),
            updated_at = NOW()
        "#,
    )
    .bind(symbol_info_id)
    .bind(data.market_cap)
    .bind(data.per)
    .bind(data.pbr)
    .bind(data.psr)
    .bind(data.roe)
    .bind(data.eps)
    .bind(data.bps)
    .bind(data.dividend_yield)
    .bind(data.week_52_high)
    .bind(data.week_52_low)
    .bind(data.foreign_ratio)
    .bind(data.revenue)
    .bind(data.operating_income)
    .bind(data.net_income)
    .bind(data.revenue_growth_yoy)
    .bind(data.net_income_growth_yoy) // earnings_growth_yoy로 매핑
    .bind(data.roa)
    .bind(data.operating_margin)
    .bind(data.debt_ratio)
    .bind(data.current_ratio)
    .bind(data.quick_ratio)
    .execute(pool)
    .await?;

    // 섹터 정보도 symbol_info에 업데이트
    if let Some(sector) = &data.sector {
        if !sector.is_empty() {
            sqlx::query(
                r#"
                UPDATE symbol_info
                SET sector = $2, updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(symbol_info_id)
            .bind(sector)
            .execute(pool)
            .await?;
        }
    }

    Ok(())
}

/// 시장 타입(KOSPI/KOSDAQ/ETF)을 symbol_info의 exchange 필드에 업데이트.
async fn update_market_type(pool: &PgPool, symbol_info_id: Uuid, market_type: &str) -> Result<()> {
    // UNKNOWN이 아닌 경우에만 업데이트
    if market_type != "UNKNOWN" {
        sqlx::query(
            r#"
            UPDATE symbol_info
            SET exchange = $2, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(symbol_info_id)
        .bind(market_type)
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// 특정 심볼의 네이버 fundamental 데이터 조회 및 저장 (단건).
///
/// 테스트나 개별 심볼 업데이트에 유용합니다.
pub async fn fetch_and_save_naver_fundamental(
    pool: &PgPool,
    ticker: &str,
) -> Result<NaverFundamentalData> {
    // symbol_info에서 ID 조회
    let symbol_info: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT id
        FROM symbol_info
        WHERE ticker = $1 AND market = 'KR' AND is_active = true
        LIMIT 1
        "#,
    )
    .bind(ticker)
    .fetch_optional(pool)
    .await?;

    let symbol_info_id = match symbol_info {
        Some((id,)) => id,
        None => {
            return Err(CollectorError::DataSource(format!(
                "심볼을 찾을 수 없습니다: {}",
                ticker
            )))
        }
    };

    // 네이버 금융에서 데이터 수집
    let fetcher = NaverFinanceFetcher::new();
    let data = fetcher
        .fetch_fundamental(ticker)
        .await
        .map_err(|e| CollectorError::DataSource(format!("네이버 데이터 수집 실패: {}", e)))?;

    // DB에 저장
    upsert_naver_fundamental(pool, symbol_info_id, &data).await?;

    // 시장 타입 업데이트
    update_market_type(pool, symbol_info_id, &data.market_type.to_string()).await?;

    info!(
        ticker = ticker,
        name = ?data.name,
        market_type = %data.market_type,
        sector = ?data.sector,
        per = ?data.per,
        "네이버 Fundamental 데이터 저장 완료"
    );

    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_ticker() {
        assert_eq!(extract_ticker("KR7005930003"), "005930");
        assert_eq!(extract_ticker("005930"), "005930");
        assert_eq!(extract_ticker("KR7000660001"), "000660");
    }
}
