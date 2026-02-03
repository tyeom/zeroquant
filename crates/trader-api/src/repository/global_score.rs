//! GlobalScore Repository
//!
//! GlobalScore 계산 및 랭킹 조회를 담당합니다.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use tracing::{debug, info, warn};
use uuid::Uuid;
use utoipa::ToSchema;

use trader_analytics::{GlobalScorer, GlobalScorerParams};
use trader_core::types::{MarketType, Symbol, Timeframe};
use trader_data::cache::CachedHistoricalDataProvider;

// ================================================================================================
// Types
// ================================================================================================

/// GlobalScore 계산 결과 레코드 (DB)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GlobalScoreRecord {
    pub id: Uuid,
    pub symbol_info_id: Uuid,
    pub overall_score: Decimal,
    pub grade: String,
    pub confidence: Option<String>,
    pub component_scores: JsonValue,
    pub penalties: Option<JsonValue>,
    pub market: String,
    pub ticker: String,
    pub calculated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// GlobalScore 랭킹 응답용 (JOIN with symbol_info)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct RankedSymbol {
    pub ticker: String,
    pub name: String,
    pub market: String,
    pub exchange: Option<String>,
    pub overall_score: Decimal,
    pub grade: String,
    pub confidence: Option<String>,
    pub component_scores: JsonValue,
    pub penalties: Option<JsonValue>,
    pub calculated_at: DateTime<Utc>,
}

/// 랭킹 필터
#[derive(Debug, Clone, Default)]
pub struct RankingFilter {
    pub market: Option<String>,
    pub grade: Option<String>,
    pub min_score: Option<Decimal>,
    pub limit: Option<i64>,
}

// ================================================================================================
// Repository
// ================================================================================================

/// GlobalScore Repository
pub struct GlobalScoreRepository;

impl GlobalScoreRepository {
    /// 모든 활성 심볼에 대해 GlobalScore 계산.
    ///
    /// # 인자
    ///
    /// * `pool` - PostgreSQL 연결 풀
    ///
    /// # 반환
    ///
    /// 처리된 종목 수
    ///
    /// # 에러
    ///
    /// DB 조회/삽입 실패 시
    pub async fn calculate_all(pool: &PgPool) -> Result<i32, sqlx::Error> {
        // 1. 활성 심볼 목록 조회
        let symbols = sqlx::query!(
            r#"
            SELECT id, ticker, name, market, exchange
            FROM symbol_info
            WHERE is_active = true
            ORDER BY ticker
            "#
        )
        .fetch_all(pool)
        .await?;

        info!("GlobalScore 계산 시작: {} 종목", symbols.len());

        let scorer = GlobalScorer::new();
        let mut processed = 0;

        // OHLCV 데이터 제공자 초기화
        let data_provider = CachedHistoricalDataProvider::new(pool.clone());

        // 2. 각 심볼에 대해 GlobalScore 계산
        for sym in symbols.iter() {
            match Self::calculate_single(
                pool,
                &scorer,
                &data_provider,
                sym.id,
                &sym.ticker,
                &sym.market,
            )
            .await
            {
                Ok(_) => {
                    processed += 1;
                    if processed % 100 == 0 {
                        debug!("진행률: {}/{}", processed, symbols.len());
                    }
                }
                Err(e) => {
                    warn!("GlobalScore 계산 실패 ({}): {}", sym.ticker, e);
                }
            }
        }

        info!("GlobalScore 계산 완료: {}/{} 종목", processed, symbols.len());

        Ok(processed)
    }

    /// 단일 심볼에 대해 GlobalScore 계산 및 저장.
    async fn calculate_single(
        pool: &PgPool,
        scorer: &GlobalScorer,
        data_provider: &CachedHistoricalDataProvider,
        symbol_info_id: Uuid,
        ticker: &str,
        market: &str,
    ) -> Result<(), sqlx::Error> {
        // 1. Market 문자열을 MarketType으로 변환
        let market_type = match market {
            "KR" => MarketType::Stock,
            "US" => MarketType::Stock,
            "CRYPTO" => MarketType::Crypto,
            "FOREX" => MarketType::Forex,
            "FUTURES" => MarketType::Futures,
            _ => MarketType::Stock,
        };

        let symbol = Symbol::new(ticker, "", market_type);

        // 2. OHLCV 데이터 조회 (60일치) - SymbolResolver가 자동으로 원천 심볼 변환
        let candles = data_provider
            .get_klines(ticker, Timeframe::D1, 60)
            .await
            .map_err(|e| sqlx::Error::Protocol(format!("OHLCV 조회 실패: {}", e)))?;

        if candles.len() < 30 {
            warn!("{}: 데이터 부족 ({}개)", ticker, candles.len());
            return Ok(()); // 스킵
        }

        // 3. GlobalScore 계산
        let params = GlobalScorerParams {
            symbol: Some(symbol.clone()),
            market_type: Some(market_type),
            ..Default::default()
        };

        let result = scorer
            .calculate(&candles, params)
            .map_err(|e| sqlx::Error::Protocol(format!("GlobalScore 계산 실패: {}", e)))?;

        // 4. DB 저장 (UPSERT)
        // component_scores를 복사하여 penalties 추출
        let mut component_scores_map = result.component_scores.clone();
        let penalties_value = component_scores_map.remove("penalties").unwrap_or(Decimal::ZERO);

        let component_scores = serde_json::to_value(&component_scores_map)
            .map_err(|e| sqlx::Error::Protocol(format!("JSON 변환 실패: {}", e)))?;

        // penalties를 JSONB로 변환 (단일 값을 객체로 감싸기)
        let penalties = serde_json::json!({ "total": penalties_value.to_string() });

        // grade는 recommendation 필드 사용
        let grade = &result.recommendation;

        // confidence는 Decimal이므로 HIGH/MEDIUM/LOW 문자열로 변환
        let confidence_str = if result.confidence >= dec!(0.8) {
            Some("HIGH".to_string())
        } else if result.confidence >= dec!(0.6) {
            Some("MEDIUM".to_string())
        } else {
            Some("LOW".to_string())
        };

        sqlx::query(
            r#"
            SELECT upsert_global_score($1, $2, $3, $4, $5, $6, $7, $8)
            "#
        )
        .bind(symbol_info_id)
        .bind(result.overall_score)
        .bind(grade)
        .bind(confidence_str)
        .bind(component_scores)
        .bind(penalties)
        .bind(market)
        .bind(ticker)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// 상위 랭킹 조회.
    ///
    /// # 인자
    ///
    /// * `pool` - PostgreSQL 연결 풀
    /// * `filter` - 필터 조건
    ///
    /// # 반환
    ///
    /// 상위 N개 종목 (overall_score DESC)
    pub async fn get_top_ranked(
        pool: &PgPool,
        filter: RankingFilter,
    ) -> Result<Vec<RankedSymbol>, sqlx::Error> {
        let limit = filter.limit.unwrap_or(50).min(500); // 기본 50, 최대 500

        // 동적 쿼리 빌더
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"
            SELECT
                si.ticker,
                si.name,
                sgs.market,
                si.exchange,
                sgs.overall_score,
                sgs.grade,
                sgs.confidence,
                sgs.component_scores,
                sgs.penalties,
                sgs.calculated_at
            FROM symbol_global_score sgs
            INNER JOIN symbol_info si ON sgs.symbol_info_id = si.id
            WHERE 1=1
            "#,
        );

        // 필터 적용
        if let Some(ref market) = filter.market {
            query_builder.push(" AND sgs.market = ");
            query_builder.push_bind(market);
        }

        if let Some(ref grade) = filter.grade {
            query_builder.push(" AND sgs.grade = ");
            query_builder.push_bind(grade);
        }

        if let Some(min_score) = filter.min_score {
            query_builder.push(" AND sgs.overall_score >= ");
            query_builder.push_bind(min_score);
        }

        // 정렬 및 제한
        query_builder.push(" ORDER BY sgs.overall_score DESC, si.ticker ASC");
        query_builder.push(" LIMIT ");
        query_builder.push_bind(limit);

        let results = query_builder
            .build_query_as::<RankedSymbol>()
            .fetch_all(pool)
            .await?;

        Ok(results)
    }

    /// 특정 심볼의 GlobalScore 조회.
    ///
    /// # 인자
    ///
    /// * `pool` - PostgreSQL 연결 풀
    /// * `ticker` - 티커 코드
    /// * `market` - 시장 (KR, US 등)
    ///
    /// # 반환
    ///
    /// GlobalScore 레코드 (없으면 None)
    pub async fn get_by_ticker(
        pool: &PgPool,
        ticker: &str,
        market: &str,
    ) -> Result<Option<RankedSymbol>, sqlx::Error> {
        let result = sqlx::query_as!(
            RankedSymbol,
            r#"
            SELECT
                si.ticker,
                si.name,
                sgs.market,
                si.exchange,
                sgs.overall_score,
                sgs.grade,
                sgs.confidence,
                sgs.component_scores,
                sgs.penalties,
                sgs.calculated_at
            FROM symbol_global_score sgs
            INNER JOIN symbol_info si ON sgs.symbol_info_id = si.id
            WHERE si.ticker = $1 AND sgs.market = $2
            "#,
            ticker,
            market
        )
        .fetch_optional(pool)
        .await?;

        Ok(result)
    }
}
