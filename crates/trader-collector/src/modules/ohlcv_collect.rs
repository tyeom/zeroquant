//! OHLCV 데이터 수집 모듈.

use crate::{CollectionStats, CollectorConfig, Result};
use chrono::{NaiveDate, Utc};
use sqlx::PgPool;
use std::time::Instant;
use trader_core::Timeframe;
use trader_data::cache::historical::CachedHistoricalDataProvider;

/// OHLCV 데이터 수집
pub async fn collect_ohlcv(
    pool: &PgPool,
    config: &CollectorConfig,
    symbols: Option<String>,
) -> Result<CollectionStats> {
    let start = Instant::now();
    let mut stats = CollectionStats::new();

    tracing::info!("OHLCV 수집 시작");

    // 수집할 심볼 목록 결정
    let target_symbols = match symbols {
        Some(ref s) => {
            // 쉼표로 구분된 심볼 파싱
            let syms: Vec<String> = s.split(',').map(|s| s.trim().to_string()).collect();
            tracing::info!(count = syms.len(), "특정 심볼 수집");
            syms
        }
        None => {
            // DB에서 활성화된 KR 심볼 조회 (STOCK, ETF만, 전체)
            let symbols: Vec<(String,)> = sqlx::query_as(
                "SELECT ticker FROM symbol_info
                 WHERE is_active = true
                   AND market = 'KR'
                   AND symbol_type IN ('STOCK', 'ETF')
                 ORDER BY ticker",
            )
            .fetch_all(pool)
            .await?;

            let syms: Vec<String> = symbols.into_iter().map(|(s,)| s).collect();
            tracing::info!(count = syms.len(), "활성 심볼 조회 완료 (STOCK/ETF 전체)");
            syms
        }
    };

    if target_symbols.is_empty() {
        tracing::warn!("수집할 심볼이 없습니다");
        stats.elapsed = start.elapsed();
        return Ok(stats);
    }

    // 날짜 범위 계산
    let (start_date_str, end_date_str) = determine_date_range(config);
    let start_date = NaiveDate::parse_from_str(&start_date_str, "%Y%m%d")
        .map_err(|e| crate::error::CollectorError::Other(Box::new(e)))?;
    let end_date = NaiveDate::parse_from_str(&end_date_str, "%Y%m%d")
        .map_err(|e| crate::error::CollectorError::Other(Box::new(e)))?;

    tracing::info!(
        symbols = target_symbols.len(),
        start_date = ?start_date,
        end_date = ?end_date,
        "수집 범위 설정 완료"
    );

    // Yahoo Finance 기반 데이터 제공자 초기화 (KRX fallback 내장)
    let provider = CachedHistoricalDataProvider::new(pool.clone());

    // 심볼별 수집
    for (idx, symbol) in target_symbols.iter().enumerate() {
        stats.total += 1;

        tracing::debug!(
            symbol = symbol,
            progress = format!("{}/{}", idx + 1, target_symbols.len()),
            "수집 시작"
        );

        // Yahoo Finance API 호출 (KRX 실패 시 자동 fallback)
        match provider
            .get_klines_range(symbol, Timeframe::D1, start_date, end_date)
            .await
        {
            Ok(klines) if !klines.is_empty() => {
                stats.success += 1;
                stats.total_klines += klines.len();
                tracing::info!(symbol = symbol, klines = klines.len(), "수집 및 저장 완료");
            }
            Ok(_) => {
                // 데이터 없음
                stats.empty += 1;
                tracing::debug!(symbol = symbol, "데이터 없음");
            }
            Err(e) => {
                stats.errors += 1;
                tracing::error!(
                    symbol = symbol,
                    error = %e,
                    "조회 실패"
                );
            }
        }

        // Rate limiting
        tokio::time::sleep(config.ohlcv_collect.request_delay()).await;
    }

    stats.elapsed = start.elapsed();
    Ok(stats)
}

/// 날짜 범위 결정
fn determine_date_range(config: &CollectorConfig) -> (String, String) {
    let end_date = match &config.ohlcv_collect.end_date {
        Some(date) => date.clone(),
        None => Utc::now().format("%Y%m%d").to_string(),
    };

    let start_date = match &config.ohlcv_collect.start_date {
        Some(date) => date.clone(),
        None => {
            // 기본: 1년 전부터
            let one_year_ago = Utc::now() - chrono::Duration::days(365);
            one_year_ago.format("%Y%m%d").to_string()
        }
    };

    (start_date, end_date)
}
