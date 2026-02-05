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
use ts_rs::TS;
use utoipa::ToSchema;
use uuid::Uuid;

use trader_analytics::{
    GlobalScorer, GlobalScorerParams, IndicatorEngine, RouteStateCalculator, SevenFactorCalculator,
    SevenFactorInput, SevenFactorScores,
};
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
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema, TS)]
#[ts(export, export_to = "ranking/")]
pub struct RankedSymbol {
    pub ticker: String,
    pub name: String,
    pub market: String,
    pub exchange: Option<String>,
    /// 종합 점수 (0-100) - JSON에서 숫자로 직렬화
    #[serde(with = "rust_decimal::serde::float")]
    #[ts(type = "number")]
    pub overall_score: Decimal,
    pub grade: String,
    pub confidence: Option<String>,
    #[ts(type = "Record<string, number>")]
    pub component_scores: JsonValue,
    #[ts(type = "Record<string, number> | null")]
    pub penalties: Option<JsonValue>,
    #[ts(type = "string")]
    pub calculated_at: DateTime<Utc>,
    /// RouteState (실시간 계산됨, DB 조회 시 None)
    #[sqlx(skip)]
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub route_state: Option<String>,
}

/// 7Factor 응답용 타입
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "ranking/")]
pub struct SevenFactorResponse {
    pub ticker: String,
    pub name: String,
    pub market: String,
    /// 7개 정규화 팩터 (0-100)
    pub factors: SevenFactorData,
    /// 종합 점수 (0-100) - JSON에서 숫자로 직렬화
    #[serde(with = "rust_decimal::serde::float")]
    #[ts(type = "number")]
    pub composite_score: Decimal,
    /// 기존 GlobalScore 정보 - JSON에서 숫자로 직렬화
    #[serde(default, with = "rust_decimal::serde::float_option")]
    #[ts(type = "number | null")]
    pub global_score: Option<Decimal>,
    pub grade: Option<String>,
    /// 계산 시각
    #[ts(type = "string")]
    pub calculated_at: chrono::DateTime<Utc>,
}

/// 7Factor 데이터
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "ranking/")]
pub struct SevenFactorData {
    /// 모멘텀 (0-100)
    #[ts(type = "number")]
    pub norm_momentum: Decimal,
    /// 가치 (0-100)
    #[ts(type = "number")]
    pub norm_value: Decimal,
    /// 품질 (0-100)
    #[ts(type = "number")]
    pub norm_quality: Decimal,
    /// 변동성 (0-100, 낮은 변동성 = 높은 점수)
    #[ts(type = "number")]
    pub norm_volatility: Decimal,
    /// 유동성 (0-100)
    #[ts(type = "number")]
    pub norm_liquidity: Decimal,
    /// 성장성 (0-100)
    #[ts(type = "number")]
    pub norm_growth: Decimal,
    /// 시장 심리 (0-100)
    #[ts(type = "number")]
    pub norm_sentiment: Decimal,
}

impl From<SevenFactorScores> for SevenFactorData {
    fn from(scores: SevenFactorScores) -> Self {
        Self {
            norm_momentum: scores.norm_momentum,
            norm_value: scores.norm_value,
            norm_quality: scores.norm_quality,
            norm_volatility: scores.norm_volatility,
            norm_liquidity: scores.norm_liquidity,
            norm_growth: scores.norm_growth,
            norm_sentiment: scores.norm_sentiment,
        }
    }
}

/// 랭킹 필터
#[derive(Debug, Clone, Default)]
pub struct RankingFilter {
    pub market: Option<String>,
    pub grade: Option<String>,
    pub min_score: Option<Decimal>,
    pub limit: Option<i64>,
    /// RouteState 필터 (ATTACK, ARMED, WATCH, REST)
    pub route_state: Option<String>,
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

        info!(
            "GlobalScore 계산 완료: {}/{} 종목",
            processed,
            symbols.len()
        );

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
            symbol: Some(symbol.to_string()),
            market_type: Some(market_type),
            ..Default::default()
        };

        let result = scorer
            .calculate(&candles, params)
            .map_err(|e| sqlx::Error::Protocol(format!("GlobalScore 계산 실패: {}", e)))?;

        // 4. DB 저장 (UPSERT)
        // component_scores를 복사하여 penalties 추출
        let mut component_scores_map = result.component_scores.clone();
        let penalties_value = component_scores_map
            .remove("penalties")
            .unwrap_or(Decimal::ZERO);

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
            "#,
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
    ///
    /// # 참고
    ///
    /// route_state 필터가 있을 경우 실시간으로 계산됩니다 (성능 주의).
    pub async fn get_top_ranked(
        pool: &PgPool,
        filter: RankingFilter,
    ) -> Result<Vec<RankedSymbol>, sqlx::Error> {
        // RouteState 필터가 있으면 더 많이 조회 후 필터링 (계산 비용 고려)
        let has_route_state_filter = filter.route_state.is_some();
        let db_limit = if has_route_state_filter {
            // RouteState 필터 시 최대 2000개 조회 후 필터링
            filter.limit.unwrap_or(50).min(500) * 4
        } else {
            filter.limit.unwrap_or(50).min(500)
        };

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

        // 필터 적용 (KR-KOSPI 형식 지원)
        if let Some(ref market) = filter.market {
            // "KR-KOSPI", "KR-KOSDAQ" 등 하이픈 구분 형식 파싱
            if let Some((market_code, exchange_code)) = market.split_once('-') {
                query_builder.push(" AND sgs.market = ");
                query_builder.push_bind(market_code.to_string());
                query_builder.push(" AND si.exchange = ");
                query_builder.push_bind(exchange_code.to_string());
            } else {
                query_builder.push(" AND sgs.market = ");
                query_builder.push_bind(market.clone());
            }
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
        query_builder.push_bind(db_limit);

        let results = query_builder
            .build_query_as::<RankedSymbol>()
            .fetch_all(pool)
            .await?;

        // RouteState 필터가 있으면 실시간 계산 및 필터링
        if let Some(ref target_state) = filter.route_state {
            let calculator = RouteStateCalculator::new();
            let data_provider = CachedHistoricalDataProvider::new(pool.clone());
            let final_limit = filter.limit.unwrap_or(50).min(500) as usize;

            let mut filtered_results = Vec::with_capacity(final_limit);

            for mut symbol in results.into_iter() {
                if filtered_results.len() >= final_limit {
                    break;
                }

                // 캔들 데이터 조회 (ticker를 symbol로 사용)
                match data_provider
                    .get_klines(&symbol.ticker, Timeframe::D1, 100)
                    .await
                {
                    Ok(candles) if candles.len() >= 40 => {
                        // RouteState 계산
                        match calculator.calculate(&candles) {
                            Ok(state) => {
                                let state_str = state.to_string();

                                // 필터 매칭 (대소문자 무시)
                                if state_str.eq_ignore_ascii_case(target_state) {
                                    symbol.route_state = Some(state_str);
                                    filtered_results.push(symbol);
                                }
                            }
                            Err(e) => {
                                debug!("RouteState 계산 실패 ({}): {}", symbol.ticker, e);
                            }
                        }
                    }
                    _ => {
                        // 캔들 데이터 부족 또는 조회 실패
                    }
                }
            }

            return Ok(filtered_results);
        }

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
        let result = sqlx::query_as::<_, RankedSymbol>(
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
        )
        .bind(ticker)
        .bind(market)
        .fetch_optional(pool)
        .await?;

        Ok(result)
    }

    /// 특정 심볼의 7Factor 데이터 조회.
    ///
    /// GlobalScore + Fundamental 데이터를 조합하여 7Factor 점수를 계산합니다.
    ///
    /// # 인자
    ///
    /// * `pool` - PostgreSQL 연결 풀
    /// * `ticker` - 티커 코드
    /// * `market` - 시장 (KR, US 등)
    ///
    /// # 반환
    ///
    /// 7Factor 응답 (데이터 부족 시 None)
    #[allow(clippy::field_reassign_with_default)]
    pub async fn get_seven_factor(
        pool: &PgPool,
        ticker: &str,
        market: &str,
    ) -> Result<Option<SevenFactorResponse>, sqlx::Error> {
        // 1. GlobalScore + Fundamental 데이터 조회
        let row = sqlx::query!(
            r#"
            SELECT
                si.id as symbol_info_id,
                si.ticker,
                si.name,
                sgs.market,
                sgs.overall_score,
                sgs.grade,
                sgs.component_scores,
                sgs.calculated_at,
                -- Fundamental 데이터
                sf.per,
                sf.pbr,
                sf.psr,
                sf.roe,
                sf.roa,
                sf.operating_margin,
                sf.net_profit_margin,
                sf.revenue_growth_yoy,
                sf.earnings_growth_yoy,
                sf.week_52_high,
                sf.week_52_low,
                sf.avg_volume_10d
            FROM symbol_info si
            LEFT JOIN symbol_global_score sgs ON sgs.symbol_info_id = si.id
            LEFT JOIN symbol_fundamental sf ON sf.symbol_info_id = si.id
            WHERE si.ticker = $1 AND (sgs.market = $2 OR sgs.market IS NULL)
            "#,
            ticker,
            market
        )
        .fetch_optional(pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        // 2. OHLCV 데이터에서 기술 지표 계산
        let data_provider = CachedHistoricalDataProvider::new(pool.clone());
        let candles = data_provider
            .get_klines(ticker, Timeframe::D1, 60)
            .await
            .map_err(|e| sqlx::Error::Protocol(format!("OHLCV 조회 실패: {}", e)))?;

        // 3. 7Factor 입력 데이터 구성
        let mut input = SevenFactorInput::default();

        // Fundamental 데이터 매핑
        input.per = row.per;
        input.pbr = row.pbr;
        input.psr = row.psr;
        input.roe = row.roe;
        input.roa = row.roa;
        input.operating_margin = row.operating_margin;
        input.net_profit_margin = row.net_profit_margin;
        input.revenue_growth_yoy = row.revenue_growth_yoy;
        input.earnings_growth_yoy = row.earnings_growth_yoy;
        input.week_52_high = row.week_52_high;
        input.week_52_low = row.week_52_low;

        // 거래량 데이터
        if let Some(vol) = row.avg_volume_10d {
            input.avg_volume_amount = Some(Decimal::from(vol));
        }

        // 기술 지표 계산 (캔들 데이터 있는 경우)
        if candles.len() >= 20 {
            let indicator = IndicatorEngine::new();

            // 종가, 고가, 저가 배열 추출
            let closes: Vec<Decimal> = candles.iter().map(|c| c.close).collect();
            let highs: Vec<Decimal> = candles.iter().map(|c| c.high).collect();
            let lows: Vec<Decimal> = candles.iter().map(|c| c.low).collect();

            // RSI - 가장 최근 값 사용
            use trader_analytics::{AtrParams, RsiParams};
            if let Ok(rsi_values) = indicator.rsi(&closes, RsiParams::default()) {
                if let Some(Some(last_rsi)) = rsi_values.last() {
                    input.rsi = Some(*last_rsi);
                }
            }

            // ATR% - 가장 최근 값 사용
            if let Ok(atr_values) = indicator.atr(&highs, &lows, &closes, AtrParams::default()) {
                if let Some(Some(last_atr)) = atr_values.last() {
                    let current_price = candles.last().map(|c| c.close).unwrap_or(Decimal::ONE);
                    if current_price > Decimal::ZERO {
                        input.atr_pct = Some(*last_atr / current_price * dec!(100));
                    }
                }
            }

            // 현재가
            if let Some(last) = candles.last() {
                input.current_price = Some(last.close);

                // 5일/20일 수익률
                if candles.len() >= 5 {
                    let price_5d_ago = candles[candles.len() - 5].close;
                    if price_5d_ago > Decimal::ZERO {
                        input.return_5d =
                            Some((last.close - price_5d_ago) / price_5d_ago * dec!(100));
                    }
                }
                if candles.len() >= 20 {
                    let price_20d_ago = candles[candles.len() - 20].close;
                    if price_20d_ago > Decimal::ZERO {
                        input.return_20d =
                            Some((last.close - price_20d_ago) / price_20d_ago * dec!(100));
                    }
                }
            }
        }

        // 4. 7Factor 계산
        let scores = SevenFactorCalculator::calculate(&input);
        let composite = scores.composite_score();

        // SevenFactorResponse 생성
        // sqlx::query! 타입 (스키마 NOT NULL 기반, non-Option):
        // - row.market: String, row.overall_score: Decimal, row.grade: String
        // - row.calculated_at: DateTime<Utc>
        // SevenFactorResponse의 global_score와 grade는 Option이므로 Some()으로 감싸기
        Ok(Some(SevenFactorResponse {
            ticker: row.ticker,
            name: row.name,
            market: row.market.clone(),
            factors: scores.into(),
            composite_score: composite,
            global_score: Some(row.overall_score),
            grade: Some(row.grade.clone()),
            calculated_at: row.calculated_at,
        }))
    }

    /// 여러 심볼의 7Factor 데이터 일괄 조회.
    pub async fn get_seven_factor_batch(
        pool: &PgPool,
        tickers: &[String],
        market: &str,
    ) -> Result<Vec<SevenFactorResponse>, sqlx::Error> {
        let mut results = Vec::with_capacity(tickers.len());

        for ticker in tickers {
            if let Some(factor) = Self::get_seven_factor(pool, ticker, market).await? {
                results.push(factor);
            }
        }

        Ok(results)
    }
}
