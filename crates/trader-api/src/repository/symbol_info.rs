//! 심볼 정보 저장소.
//!
//! 티커-회사명 매핑 및 검색 기능을 제공합니다.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// 심볼 정보 레코드.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SymbolInfo {
    pub id: Uuid,
    pub ticker: String,
    pub name: String,
    pub name_en: Option<String>,
    pub market: String,
    pub exchange: Option<String>,
    pub sector: Option<String>,
    pub yahoo_symbol: Option<String>,
    pub is_active: Option<bool>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// 검색 결과용 간소화된 심볼 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolSearchResult {
    pub ticker: String,
    pub name: String,
    pub market: String,
    pub yahoo_symbol: Option<String>,
}

/// 새 심볼 정보 삽입용.
#[derive(Debug, Clone)]
pub struct NewSymbolInfo {
    pub ticker: String,
    pub name: String,
    pub name_en: Option<String>,
    pub market: String,
    pub exchange: Option<String>,
    pub sector: Option<String>,
    pub yahoo_symbol: Option<String>,
}

/// 심볼 정보 저장소.
pub struct SymbolInfoRepository;

impl SymbolInfoRepository {
    /// 심볼 검색 (티커 + 회사명).
    ///
    /// 검색어가 티커나 회사명에 포함되면 매칭됩니다.
    pub async fn search(
        pool: &PgPool,
        query: &str,
        limit: i64,
    ) -> Result<Vec<SymbolSearchResult>, sqlx::Error> {
        let query_upper = query.to_uppercase();
        let query_pattern = format!("%{}%", query_upper);

        let results = sqlx::query_as::<_, (String, String, String, Option<String>)>(
            r#"
            SELECT ticker, name, market, yahoo_symbol
            FROM symbol_info
            WHERE is_active = true
              AND (
                  UPPER(ticker) LIKE $1
                  OR UPPER(name) LIKE $1
                  OR UPPER(COALESCE(name_en, '')) LIKE $1
              )
            ORDER BY
                CASE WHEN UPPER(ticker) = $2 THEN 0
                     WHEN UPPER(ticker) LIKE $3 THEN 1
                     ELSE 2
                END,
                ticker
            LIMIT $4
            "#,
        )
        .bind(&query_pattern)
        .bind(&query_upper)
        .bind(format!("{}%", query_upper))
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(results
            .into_iter()
            .map(|(ticker, name, market, yahoo_symbol)| SymbolSearchResult {
                ticker,
                name,
                market,
                yahoo_symbol,
            })
            .collect())
    }

    /// 티커로 심볼 정보 조회.
    pub async fn get_by_ticker(
        pool: &PgPool,
        ticker: &str,
        market: Option<&str>,
    ) -> Result<Option<SymbolInfo>, sqlx::Error> {
        let mut query = String::from(
            "SELECT * FROM symbol_info WHERE UPPER(ticker) = UPPER($1) AND is_active = true",
        );

        if market.is_some() {
            query.push_str(" AND market = $2");
        }

        query.push_str(" LIMIT 1");

        if let Some(m) = market {
            sqlx::query_as::<_, SymbolInfo>(&query)
                .bind(ticker)
                .bind(m)
                .fetch_optional(pool)
                .await
        } else {
            sqlx::query_as::<_, SymbolInfo>(&query)
                .bind(ticker)
                .fetch_optional(pool)
                .await
        }
    }

    /// Yahoo 심볼로 조회.
    pub async fn get_by_yahoo_symbol(
        pool: &PgPool,
        yahoo_symbol: &str,
    ) -> Result<Option<SymbolInfo>, sqlx::Error> {
        sqlx::query_as::<_, SymbolInfo>(
            "SELECT * FROM symbol_info WHERE yahoo_symbol = $1 AND is_active = true LIMIT 1",
        )
        .bind(yahoo_symbol)
        .fetch_optional(pool)
        .await
    }

    /// 심볼 정보 일괄 삽입 (upsert).
    pub async fn upsert_batch(
        pool: &PgPool,
        symbols: &[NewSymbolInfo],
    ) -> Result<usize, sqlx::Error> {
        if symbols.is_empty() {
            return Ok(0);
        }

        let mut inserted = 0;

        for symbol in symbols {
            let result = sqlx::query(
                r#"
                INSERT INTO symbol_info (ticker, name, name_en, market, exchange, sector, yahoo_symbol)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (ticker, market) DO UPDATE SET
                    name = EXCLUDED.name,
                    name_en = EXCLUDED.name_en,
                    exchange = EXCLUDED.exchange,
                    sector = EXCLUDED.sector,
                    yahoo_symbol = EXCLUDED.yahoo_symbol,
                    updated_at = NOW()
                "#,
            )
            .bind(&symbol.ticker)
            .bind(&symbol.name)
            .bind(&symbol.name_en)
            .bind(&symbol.market)
            .bind(&symbol.exchange)
            .bind(&symbol.sector)
            .bind(&symbol.yahoo_symbol)
            .execute(pool)
            .await?;

            if result.rows_affected() > 0 {
                inserted += 1;
            }
        }

        Ok(inserted)
    }

    /// 시장별 심볼 수 조회.
    pub async fn count_by_market(pool: &PgPool) -> Result<Vec<(String, i64)>, sqlx::Error> {
        sqlx::query_as::<_, (String, i64)>(
            "SELECT market, COUNT(*) FROM symbol_info WHERE is_active = true GROUP BY market ORDER BY market",
        )
        .fetch_all(pool)
        .await
    }

    /// 전체 심볼 수 조회.
    pub async fn count_all(pool: &PgPool) -> Result<i64, sqlx::Error> {
        let row = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM symbol_info WHERE is_active = true",
        )
        .fetch_one(pool)
        .await?;

        Ok(row)
    }
}
