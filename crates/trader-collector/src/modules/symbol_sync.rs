//! 심볼 동기화 모듈.

use crate::{CollectionStats, CollectorConfig, Result};
use sqlx::PgPool;
use std::time::Instant;
use trader_data::provider::symbol_info::{KrxSymbolProvider, SymbolInfoProvider};

/// 심볼 정보 동기화
pub async fn sync_symbols(pool: &PgPool, config: &CollectorConfig) -> Result<CollectionStats> {
    let start = Instant::now();
    let mut stats = CollectionStats::new();

    tracing::info!("심볼 동기화 시작");

    // 1. 현재 심볼 수 확인
    let current_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM symbol_info")
        .fetch_one(pool)
        .await?;

    tracing::info!(
        current_count,
        min = config.symbol_sync.min_symbol_count,
        "심볼 수 확인"
    );

    if current_count >= config.symbol_sync.min_symbol_count {
        tracing::info!("심볼 수 충분, 동기화 건너뛰기");
        stats.skipped = 1;
        stats.elapsed = start.elapsed();
        return Ok(stats);
    }

    // 2. KRX 동기화
    if config.symbol_sync.enable_krx {
        tracing::info!("KRX 심볼 동기화 시작");
        match sync_krx_symbols(pool).await {
            Ok(count) => {
                stats.success += 1;
                stats.total += count;
                tracing::info!(count, "KRX 심볼 동기화 완료");
            }
            Err(e) => {
                stats.errors += 1;
                tracing::error!(error = %e, "KRX 동기화 실패");
            }
        }
    }

    // TODO: Binance, Yahoo 동기화 구현

    stats.elapsed = start.elapsed();
    Ok(stats)
}

/// KRX 심볼 동기화
async fn sync_krx_symbols(pool: &PgPool) -> Result<usize> {
    let provider = KrxSymbolProvider::new();

    // KRX에서 종목 목록 조회
    let symbols = provider
        .fetch_all()
        .await
        .map_err(|e| crate::error::CollectorError::DataSource(e.to_string()))?;

    tracing::info!(count = symbols.len(), "KRX 종목 조회 완료");

    let mut inserted_count = 0;

    // DB에 저장 (배치로 처리하는 것이 더 효율적이지만, 지금은 개별 upsert)
    for symbol_meta in symbols {
        // 심볼 정규화 (005930.KS -> 005930)
        let ticker = symbol_meta.ticker.trim_end_matches(".KS").to_string();

        // symbol_info 테이블에 upsert
        let result = sqlx::query(
            r#"
            INSERT INTO symbol_info (
                id, ticker, name, name_en, market, exchange, sector, yahoo_symbol, is_active, created_at, updated_at
            )
            VALUES (
                gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, true, NOW(), NOW()
            )
            ON CONFLICT (ticker, market) DO UPDATE SET
                name = EXCLUDED.name,
                name_en = EXCLUDED.name_en,
                exchange = EXCLUDED.exchange,
                sector = EXCLUDED.sector,
                yahoo_symbol = EXCLUDED.yahoo_symbol,
                is_active = true,
                updated_at = NOW()
            "#
        )
        .bind(&ticker)
        .bind(&symbol_meta.name)
        .bind(symbol_meta.name_en.as_deref())
        .bind(&symbol_meta.market)
        .bind(symbol_meta.exchange.as_deref())
        .bind(symbol_meta.sector.as_deref())
        .bind(symbol_meta.yahoo_symbol.as_deref())
        .execute(pool)
        .await;

        match result {
            Ok(_) => {
                inserted_count += 1;
            }
            Err(e) => {
                tracing::warn!(
                    ticker = &ticker,
                    error = %e,
                    "심볼 저장 실패"
                );
            }
        }
    }

    Ok(inserted_count)
}
