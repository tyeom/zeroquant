//! 종목 스크리닝 Repository
//!
//! Fundamental 데이터와 OHLCV 데이터를 조합하여
//! 다양한 조건으로 종목을 필터링합니다.

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, QueryBuilder};
use tracing::debug;
use utoipa::ToSchema;
use uuid::Uuid;

/// 스크리닝 결과 레코드
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ScreeningResult {
    // 심볼 기본 정보
    pub id: Uuid,
    pub ticker: String,
    pub name: String,
    pub market: String,
    pub exchange: Option<String>,
    pub sector: Option<String>,
    pub yahoo_symbol: Option<String>,

    // Fundamental 지표
    pub market_cap: Option<Decimal>,
    pub per: Option<Decimal>,
    pub pbr: Option<Decimal>,
    pub roe: Option<Decimal>,
    pub roa: Option<Decimal>,
    pub eps: Option<Decimal>,
    pub bps: Option<Decimal>,
    pub dividend_yield: Option<Decimal>,
    pub operating_margin: Option<Decimal>,
    pub debt_ratio: Option<Decimal>,
    pub revenue_growth_yoy: Option<Decimal>,
    pub earnings_growth_yoy: Option<Decimal>,

    // 가격 정보 (OHLCV 기반)
    pub current_price: Option<Decimal>,
    pub price_change_1d: Option<Decimal>,
    pub price_change_5d: Option<Decimal>,
    pub price_change_20d: Option<Decimal>,
    pub volume_ratio: Option<Decimal>,
    pub week_52_high: Option<Decimal>,
    pub week_52_low: Option<Decimal>,
    pub distance_from_52w_high: Option<Decimal>,
    pub distance_from_52w_low: Option<Decimal>,
}

/// 스크리닝 필터 조건
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScreeningFilter {
    // 시장/거래소 필터
    pub market: Option<String>,
    pub exchange: Option<String>,
    pub sector: Option<String>,

    // 시가총액 필터
    pub min_market_cap: Option<Decimal>,
    pub max_market_cap: Option<Decimal>,

    // 밸류에이션 필터
    pub min_per: Option<Decimal>,
    pub max_per: Option<Decimal>,
    pub min_pbr: Option<Decimal>,
    pub max_pbr: Option<Decimal>,
    pub min_psr: Option<Decimal>,
    pub max_psr: Option<Decimal>,

    // 수익성 필터
    pub min_roe: Option<Decimal>,
    pub max_roe: Option<Decimal>,
    pub min_roa: Option<Decimal>,
    pub max_roa: Option<Decimal>,
    pub min_operating_margin: Option<Decimal>,
    pub max_operating_margin: Option<Decimal>,

    // 배당 필터
    pub min_dividend_yield: Option<Decimal>,
    pub max_dividend_yield: Option<Decimal>,

    // 안정성 필터
    pub max_debt_ratio: Option<Decimal>,
    pub min_current_ratio: Option<Decimal>,

    // 성장성 필터
    pub min_revenue_growth: Option<Decimal>,
    pub min_earnings_growth: Option<Decimal>,

    // 가격/기술적 필터
    pub min_price_change_1d: Option<Decimal>,
    pub max_price_change_1d: Option<Decimal>,
    pub min_price_change_5d: Option<Decimal>,
    pub max_price_change_5d: Option<Decimal>,
    pub min_price_change_20d: Option<Decimal>,
    pub max_price_change_20d: Option<Decimal>,

    // 거래량 필터
    pub min_volume_ratio: Option<Decimal>, // 평균 대비 거래량 배율 (예: 2.0 = 평균의 2배)
    pub min_avg_volume: Option<i64>,       // 최소 평균 거래량

    // 52주 고/저가 대비
    pub max_distance_from_52w_high: Option<Decimal>, // 52주 고가 대비 하락률 (예: 10 = 10% 이내)
    pub min_distance_from_52w_low: Option<Decimal>,  // 52주 저가 대비 상승률

    // 정렬 및 제한
    pub sort_by: Option<String>, // market_cap, per, pbr, roe, price_change_1d, volume_ratio
    pub sort_order: Option<String>, // asc, desc
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

/// 스크리닝 Repository
pub struct ScreeningRepository;

impl ScreeningRepository {
    /// 종합 스크리닝 실행
    ///
    /// Fundamental 데이터와 최근 OHLCV 데이터를 조합하여 스크리닝합니다.
    pub async fn screen(
        pool: &PgPool,
        filter: &ScreeningFilter,
    ) -> Result<Vec<ScreeningResult>, sqlx::Error> {
        // 기본 쿼리: Fundamental 뷰 + OHLCV 최신 가격 정보
        // 복잡한 price_changes CTE 대신 단순화된 쿼리 사용
        let mut builder: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
            r#"
            WITH price_data AS (
                SELECT DISTINCT ON (symbol)
                    symbol,
                    close as current_price
                FROM ohlcv
                WHERE timeframe = '1d'
                ORDER BY symbol, open_time DESC
            )
            SELECT
                sf.id,
                sf.ticker,
                sf.name,
                sf.market,
                sf.exchange,
                sf.sector,
                sf.yahoo_symbol,
                sf.market_cap,
                sf.per,
                sf.pbr,
                sf.roe,
                sf.roa,
                sf.eps,
                sf.bps,
                sf.dividend_yield,
                sf.operating_margin,
                sf.debt_ratio,
                sf.revenue_growth_yoy,
                sf.earnings_growth_yoy,
                pd.current_price,
                NULL::decimal as price_change_1d,
                NULL::decimal as price_change_5d,
                NULL::decimal as price_change_20d,
                NULL::decimal as volume_ratio,
                sf.week_52_high,
                sf.week_52_low,
                CASE WHEN sf.week_52_high > 0 AND pd.current_price IS NOT NULL
                    THEN ((sf.week_52_high - pd.current_price) / sf.week_52_high) * 100
                    ELSE NULL END as distance_from_52w_high,
                CASE WHEN sf.week_52_low > 0 AND pd.current_price IS NOT NULL
                    THEN ((pd.current_price - sf.week_52_low) / sf.week_52_low) * 100
                    ELSE NULL END as distance_from_52w_low
            FROM v_symbol_with_fundamental sf
            LEFT JOIN price_data pd ON pd.symbol = sf.yahoo_symbol OR pd.symbol = sf.ticker
            WHERE sf.is_active = true
            "#,
        );

        // 동적 WHERE 조건 추가
        Self::add_filter_conditions(&mut builder, filter);

        // 정렬
        let sort_by = filter.sort_by.as_deref().unwrap_or("market_cap");
        let sort_order = filter.sort_order.as_deref().unwrap_or("desc");

        builder.push(" ORDER BY ");
        match sort_by {
            "per" => builder.push("sf.per"),
            "pbr" => builder.push("sf.pbr"),
            "roe" => builder.push("sf.roe"),
            "dividend_yield" => builder.push("sf.dividend_yield"),
            "price_change_1d" => builder.push("pd.current_price"), // TODO: 실제 변동률로 변경
            _ => builder.push("sf.market_cap"),
        };

        if sort_order.to_lowercase() == "asc" {
            builder.push(" ASC NULLS LAST");
        } else {
            builder.push(" DESC NULLS LAST");
        }

        // LIMIT/OFFSET
        let limit = filter.limit.unwrap_or(50).min(500);
        let offset = filter.offset.unwrap_or(0);
        builder.push(" LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let query = builder.build_query_as::<ScreeningResult>();
        let results = query.fetch_all(pool).await?;

        debug!("스크리닝 완료: {} 종목 반환", results.len());
        Ok(results)
    }

    /// 동적 WHERE 조건 추가
    fn add_filter_conditions(builder: &mut QueryBuilder<sqlx::Postgres>, filter: &ScreeningFilter) {
        // 시장 필터
        if let Some(ref market) = filter.market {
            builder.push(" AND sf.market = ");
            builder.push_bind(market.clone());
        }
        if let Some(ref exchange) = filter.exchange {
            builder.push(" AND sf.exchange = ");
            builder.push_bind(exchange.clone());
        }
        if let Some(ref sector) = filter.sector {
            builder.push(" AND sf.sector ILIKE ");
            builder.push_bind(format!("%{}%", sector));
        }

        // 시가총액 필터
        if let Some(v) = filter.min_market_cap {
            builder.push(" AND sf.market_cap >= ");
            builder.push_bind(v);
        }
        if let Some(v) = filter.max_market_cap {
            builder.push(" AND sf.market_cap <= ");
            builder.push_bind(v);
        }

        // PER 필터
        if let Some(v) = filter.min_per {
            builder.push(" AND sf.per >= ");
            builder.push_bind(v);
        }
        if let Some(v) = filter.max_per {
            builder.push(" AND sf.per <= ");
            builder.push_bind(v);
        }

        // PBR 필터
        if let Some(v) = filter.min_pbr {
            builder.push(" AND sf.pbr >= ");
            builder.push_bind(v);
        }
        if let Some(v) = filter.max_pbr {
            builder.push(" AND sf.pbr <= ");
            builder.push_bind(v);
        }

        // ROE 필터
        if let Some(v) = filter.min_roe {
            builder.push(" AND sf.roe >= ");
            builder.push_bind(v);
        }
        if let Some(v) = filter.max_roe {
            builder.push(" AND sf.roe <= ");
            builder.push_bind(v);
        }

        // ROA 필터
        if let Some(v) = filter.min_roa {
            builder.push(" AND sf.roa >= ");
            builder.push_bind(v);
        }
        if let Some(v) = filter.max_roa {
            builder.push(" AND sf.roa <= ");
            builder.push_bind(v);
        }

        // 배당수익률 필터
        if let Some(v) = filter.min_dividend_yield {
            builder.push(" AND sf.dividend_yield >= ");
            builder.push_bind(v);
        }
        if let Some(v) = filter.max_dividend_yield {
            builder.push(" AND sf.dividend_yield <= ");
            builder.push_bind(v);
        }

        // Operating Margin 필터
        if let Some(v) = filter.min_operating_margin {
            builder.push(" AND sf.operating_margin >= ");
            builder.push_bind(v);
        }
        if let Some(v) = filter.max_operating_margin {
            builder.push(" AND sf.operating_margin <= ");
            builder.push_bind(v);
        }

        // 부채비율 필터
        if let Some(v) = filter.max_debt_ratio {
            builder.push(" AND sf.debt_ratio <= ");
            builder.push_bind(v);
        }

        // 성장성 필터
        if let Some(v) = filter.min_revenue_growth {
            builder.push(" AND sf.revenue_growth_yoy >= ");
            builder.push_bind(v);
        }
        if let Some(v) = filter.min_earnings_growth {
            builder.push(" AND sf.earnings_growth_yoy >= ");
            builder.push_bind(v);
        }

        // 52주 고저가 필터
        if let Some(v) = filter.max_distance_from_52w_high {
            builder.push(
                " AND CASE WHEN sf.week_52_high > 0 AND pd.current_price IS NOT NULL
                  THEN ((sf.week_52_high - pd.current_price) / sf.week_52_high) * 100
                  ELSE NULL END <= ",
            );
            builder.push_bind(v);
        }
        if let Some(v) = filter.min_distance_from_52w_low {
            builder.push(
                " AND CASE WHEN sf.week_52_low > 0 AND pd.current_price IS NOT NULL
                  THEN ((pd.current_price - sf.week_52_low) / sf.week_52_low) * 100
                  ELSE NULL END >= ",
            );
            builder.push_bind(v);
        }
    }

    /// 사전 정의된 스크리닝 프리셋 실행
    pub async fn screen_preset(
        pool: &PgPool,
        preset: &str,
        market: Option<&str>,
    ) -> Result<Vec<ScreeningResult>, sqlx::Error> {
        let filter = match preset {
            // 가치주: 저PER + 저PBR + 적정 ROE
            "value" => ScreeningFilter {
                market: market.map(String::from),
                max_per: Some(Decimal::from(15)),
                max_pbr: Some(Decimal::from(1)),
                min_roe: Some(Decimal::from(5)),
                limit: Some(50),
                sort_by: Some("pbr".to_string()),
                sort_order: Some("asc".to_string()),
                ..Default::default()
            },
            // 고배당주: 배당수익률 높은 종목
            "dividend" => ScreeningFilter {
                market: market.map(String::from),
                min_dividend_yield: Some(Decimal::from(3)),
                min_roe: Some(Decimal::from(5)),
                max_debt_ratio: Some(Decimal::from(100)),
                limit: Some(50),
                sort_by: Some("dividend_yield".to_string()),
                sort_order: Some("desc".to_string()),
                ..Default::default()
            },
            // 성장주: 높은 매출/이익 성장률
            "growth" => ScreeningFilter {
                market: market.map(String::from),
                min_revenue_growth: Some(Decimal::from(20)),
                min_earnings_growth: Some(Decimal::from(15)),
                min_roe: Some(Decimal::from(10)),
                limit: Some(50),
                sort_by: Some("revenue_growth_yoy".to_string()),
                sort_order: Some("desc".to_string()),
                ..Default::default()
            },
            // 스노우볼: 저PBR + 고배당 + 안정성
            "snowball" => ScreeningFilter {
                market: market.map(String::from),
                max_pbr: Some(Decimal::from(1)),
                min_dividend_yield: Some(Decimal::from(3)),
                max_debt_ratio: Some(Decimal::from(80)),
                min_roe: Some(Decimal::from(8)),
                limit: Some(30),
                sort_by: Some("dividend_yield".to_string()),
                sort_order: Some("desc".to_string()),
                ..Default::default()
            },
            // 대형주: 시가총액 상위
            "large_cap" => ScreeningFilter {
                market: market.map(String::from),
                min_market_cap: Some(Decimal::from(10_000_000_000_000i64)), // 10조 이상
                limit: Some(50),
                sort_by: Some("market_cap".to_string()),
                sort_order: Some("desc".to_string()),
                ..Default::default()
            },
            // 52주 신저가 근접 (바닥 매수 전략)
            "near_52w_low" => ScreeningFilter {
                market: market.map(String::from),
                min_distance_from_52w_low: Some(Decimal::from(0)),
                max_distance_from_52w_high: Some(Decimal::from(50)), // 고가 대비 50% 이상 하락
                min_roe: Some(Decimal::from(5)),                     // 기본 수익성 보장
                limit: Some(50),
                sort_by: Some("pbr".to_string()),
                sort_order: Some("asc".to_string()),
                ..Default::default()
            },
            // 기본: 전체 목록
            _ => ScreeningFilter {
                market: market.map(String::from),
                limit: Some(100),
                ..Default::default()
            },
        };

        Self::screen(pool, &filter).await
    }

    /// 가격 변동률 기반 모멘텀 스크리닝
    ///
    /// OHLCV 데이터를 직접 분석하여 급등주, 급락주 등을 찾습니다.
    pub async fn screen_momentum(
        pool: &PgPool,
        market: Option<&str>,
        days: i32,
        min_change_pct: Decimal,
        min_volume_ratio: Option<Decimal>,
        limit: i32,
    ) -> Result<Vec<MomentumScreenResult>, sqlx::Error> {
        let lookback_date = Utc::now() - Duration::days(days.into());

        let market_filter = if let Some(m) = market {
            format!("AND si.market = '{}'", m)
        } else {
            String::new()
        };

        let volume_filter = if let Some(vr) = min_volume_ratio {
            format!("AND m.volume_ratio >= {}", vr)
        } else {
            String::new()
        };

        // 윈도우 함수와 집계 함수를 분리하여 처리
        let query = format!(
            r#"
            WITH start_prices AS (
                -- 기간 시작 시점의 가격 (DISTINCT ON으로 심볼별 첫 번째 레코드)
                SELECT DISTINCT ON (symbol)
                    symbol,
                    close as start_price
                FROM ohlcv
                WHERE timeframe = '1d'
                  AND open_time >= $1
                ORDER BY symbol, open_time ASC
            ),
            end_prices AS (
                -- 기간 종료 시점의 가격 (심볼별 마지막 레코드)
                SELECT DISTINCT ON (symbol)
                    symbol,
                    close as end_price,
                    volume as current_volume
                FROM ohlcv
                WHERE timeframe = '1d'
                  AND open_time >= $1
                ORDER BY symbol, open_time DESC
            ),
            avg_volumes AS (
                -- 기간 내 평균 거래량
                SELECT
                    symbol,
                    AVG(volume) as avg_volume
                FROM ohlcv
                WHERE timeframe = '1d'
                  AND open_time >= $1
                GROUP BY symbol
            ),
            momentum AS (
                SELECT
                    sp.symbol,
                    sp.start_price,
                    ep.end_price,
                    CASE WHEN sp.start_price > 0
                        THEN ((ep.end_price - sp.start_price) / sp.start_price) * 100
                        ELSE 0 END as change_pct,
                    COALESCE(av.avg_volume, 0) as avg_volume,
                    ep.current_volume,
                    CASE WHEN av.avg_volume > 0
                        THEN ep.current_volume / av.avg_volume
                        ELSE 0 END as volume_ratio
                FROM start_prices sp
                JOIN end_prices ep ON ep.symbol = sp.symbol
                LEFT JOIN avg_volumes av ON av.symbol = sp.symbol
            )
            SELECT
                m.symbol,
                COALESCE(si.name, m.symbol) as name,
                COALESCE(si.market, 'UNKNOWN') as market,
                si.exchange,
                m.start_price,
                m.end_price,
                m.change_pct,
                m.avg_volume,
                m.current_volume,
                m.volume_ratio
            FROM momentum m
            LEFT JOIN symbol_info si ON (si.yahoo_symbol = m.symbol OR si.ticker = m.symbol)
            WHERE (si.is_active = true OR si.id IS NULL)
              AND m.change_pct >= $2
              {}
              {}
            ORDER BY m.change_pct DESC
            LIMIT $3
            "#,
            market_filter, volume_filter
        );

        let results = sqlx::query_as::<_, MomentumScreenResult>(&query)
            .bind(lookback_date)
            .bind(min_change_pct)
            .bind(limit)
            .fetch_all(pool)
            .await?;

        debug!(
            "모멘텀 스크리닝 완료: {}일간 {}% 이상 상승, {} 종목",
            days,
            min_change_pct,
            results.len()
        );
        Ok(results)
    }

    /// 사용 가능한 프리셋 목록 반환
    pub fn available_presets() -> Vec<ScreeningPreset> {
        vec![
            ScreeningPreset {
                id: "value".to_string(),
                name: "가치주".to_string(),
                description: "저PER, 저PBR, 적정 ROE를 가진 저평가 종목".to_string(),
            },
            ScreeningPreset {
                id: "dividend".to_string(),
                name: "고배당주".to_string(),
                description: "배당수익률 3% 이상, 안정적인 수익성".to_string(),
            },
            ScreeningPreset {
                id: "growth".to_string(),
                name: "성장주".to_string(),
                description: "매출/이익 20% 이상 성장, 높은 ROE".to_string(),
            },
            ScreeningPreset {
                id: "snowball".to_string(),
                name: "스노우볼".to_string(),
                description: "저PBR + 고배당 + 낮은 부채비율의 안정 성장주".to_string(),
            },
            ScreeningPreset {
                id: "large_cap".to_string(),
                name: "대형주".to_string(),
                description: "시가총액 10조원 이상 우량 대형주".to_string(),
            },
            ScreeningPreset {
                id: "near_52w_low".to_string(),
                name: "52주 신저가 근접".to_string(),
                description: "52주 저가 근처에서 거래되는 수익성 있는 종목".to_string(),
            },
        ]
    }
}

/// 모멘텀 스크리닝 결과
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MomentumScreenResult {
    pub symbol: String,
    pub name: String,
    pub market: String,
    pub exchange: Option<String>,
    pub start_price: Decimal,
    pub end_price: Decimal,
    pub change_pct: Decimal,
    pub avg_volume: Decimal,
    pub current_volume: Decimal,
    pub volume_ratio: Decimal,
}

/// 스크리닝 프리셋 정보
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ScreeningPreset {
    pub id: String,
    pub name: String,
    pub description: String,
}
