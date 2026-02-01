//! Fundamental 데이터 백그라운드 수집기.
//!
//! 서버 실행 중 주기적으로 Yahoo Finance에서 펀더멘털 데이터를 수집합니다.
//! - 오래된 데이터(7일 이상)를 가진 심볼을 찾아 업데이트
//! - 새로 등록된 심볼(fundamental 데이터 없음)도 수집
//! - Rate limiting: 요청 간 딜레이로 API 제한 방지
//! - OHLCV 증분 업데이트: 같은 API 호출로 캔들 데이터도 함께 저장

use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use tokio::time::interval;
use tokio_util::sync::CancellationToken;
use trader_core::Timeframe;
use tracing::{debug, error, info, warn};

use trader_data::cache::{FundamentalData, FundamentalFetcher};
use trader_data::OhlcvCache;

use crate::repository::{NewSymbolFundamental, SymbolFundamentalRepository};
use super::symbol_sync::{sync_symbols, SymbolSyncConfig};

/// Decimal 값을 소수점 4자리로 반올림 (DB 저장 전 안전 처리).
fn round_dp4(value: Option<Decimal>) -> Option<Decimal> {
    value.map(|d| d.round_dp(4))
}

/// Decimal 값을 소수점 2자리로 반올림 (시가총액, 매출액 등).
fn round_dp2(value: Option<Decimal>) -> Option<Decimal> {
    value.map(|d| d.round_dp(2))
}

/// Fundamental 수집기 설정.
#[derive(Debug, Clone)]
pub struct FundamentalCollectorConfig {
    /// 수집 주기 (기본: 1시간)
    pub collect_interval: Duration,
    /// 데이터 갱신 기준 일수 (기본: 7일)
    pub stale_days: i64,
    /// 배치당 처리할 심볼 수 (기본: 50)
    pub batch_size: i64,
    /// API 요청 간 딜레이 (기본: 2초) - Rate limiting 방지
    pub request_delay: Duration,
    /// OHLCV 데이터 함께 수집 여부 (기본: true)
    pub update_ohlcv: bool,
    /// 심볼 자동 동기화 활성화 (기본: true)
    /// 수집 전에 KRX/Binance에서 종목 목록을 자동으로 가져옴
    pub auto_sync_symbols: bool,
    /// 심볼 동기화 설정
    pub symbol_sync_config: SymbolSyncConfig,
}

impl Default for FundamentalCollectorConfig {
    fn default() -> Self {
        Self {
            collect_interval: Duration::from_secs(60 * 60), // 1시간
            stale_days: 7,
            batch_size: 50,
            request_delay: Duration::from_secs(2),
            update_ohlcv: true,
            auto_sync_symbols: true,
            symbol_sync_config: SymbolSyncConfig::default(),
        }
    }
}

impl FundamentalCollectorConfig {
    /// 환경변수에서 설정 로드.
    pub fn from_env() -> Self {
        let collect_interval_secs: u64 = std::env::var("FUNDAMENTAL_COLLECT_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60 * 60);

        let stale_days: i64 = std::env::var("FUNDAMENTAL_STALE_DAYS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(7);

        let batch_size: i64 = std::env::var("FUNDAMENTAL_BATCH_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(50);

        let request_delay_ms: u64 = std::env::var("FUNDAMENTAL_REQUEST_DELAY_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2000);

        let update_ohlcv: bool = std::env::var("FUNDAMENTAL_UPDATE_OHLCV")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(true); // 기본값: 활성화

        let auto_sync_symbols: bool = std::env::var("FUNDAMENTAL_AUTO_SYNC_SYMBOLS")
            .map(|v| v != "false" && v != "0")
            .unwrap_or(true); // 기본값: 활성화

        Self {
            collect_interval: Duration::from_secs(collect_interval_secs),
            stale_days,
            batch_size,
            request_delay: Duration::from_millis(request_delay_ms),
            update_ohlcv,
            auto_sync_symbols,
            symbol_sync_config: SymbolSyncConfig::from_env(),
        }
    }
}

/// Fundamental 데이터 백그라운드 수집기 시작.
///
/// 서버 시작 시 호출하여 백그라운드에서 펀더멘털 데이터를 주기적으로 수집합니다.
///
/// # Arguments
/// * `pool` - PostgreSQL 연결 풀
/// * `config` - 수집기 설정
/// * `shutdown_token` - 종료 시그널 토큰
///
/// # 환경변수
/// * `FUNDAMENTAL_COLLECT_INTERVAL_SECS` - 수집 주기 (초, 기본: 3600)
/// * `FUNDAMENTAL_STALE_DAYS` - 데이터 갱신 기준 (일, 기본: 7)
/// * `FUNDAMENTAL_BATCH_SIZE` - 배치당 처리 심볼 수 (기본: 50)
/// * `FUNDAMENTAL_REQUEST_DELAY_MS` - API 요청 간 딜레이 (밀리초, 기본: 2000)
/// * `FUNDAMENTAL_UPDATE_OHLCV` - OHLCV 증분 업데이트 여부 (기본: true)
/// * `FUNDAMENTAL_AUTO_SYNC_SYMBOLS` - 심볼 자동 동기화 여부 (기본: true)
/// * `SYMBOL_SYNC_KRX` - KRX 종목 동기화 활성화 (기본: true)
/// * `SYMBOL_SYNC_BINANCE` - Binance 종목 동기화 활성화 (기본: false)
pub fn start_fundamental_collector(
    pool: PgPool,
    config: FundamentalCollectorConfig,
    shutdown_token: CancellationToken,
) {
    tokio::spawn(async move {
        info!(
            interval_secs = config.collect_interval.as_secs(),
            stale_days = config.stale_days,
            batch_size = config.batch_size,
            update_ohlcv = config.update_ohlcv,
            "Fundamental 데이터 수집기 시작"
        );

        // 시작 후 10초 대기 (서버 초기화 완료 후 시작)
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(10)) => {}
            _ = shutdown_token.cancelled() => {
                info!("Fundamental 수집기: 종료 시그널 수신 (초기화 중)");
                return;
            }
        }

        // 첫 번째 수집 즉시 실행
        info!("Fundamental 첫 번째 수집 배치 시작");
        if let Err(e) = run_collection_batch(&pool, &config).await {
            error!(error = %e, "Fundamental 데이터 첫 수집 배치 실패");
        }

        // 이후 주기적으로 수집
        let mut collect_interval = interval(config.collect_interval);
        collect_interval.tick().await; // 첫 tick 건너뛰기 (이미 위에서 실행함)

        loop {
            tokio::select! {
                _ = collect_interval.tick() => {
                    if let Err(e) = run_collection_batch(&pool, &config).await {
                        error!(error = %e, "Fundamental 데이터 수집 배치 실패");
                    }
                }
                _ = shutdown_token.cancelled() => {
                    info!("Fundamental 수집기: 종료 시그널 수신, 정리 중...");
                    break;
                }
            }
        }

        info!("Fundamental 데이터 수집기 종료됨");
    });
}

/// 수집 배치 실행.
async fn run_collection_batch(
    pool: &PgPool,
    config: &FundamentalCollectorConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 1. 심볼 자동 동기화 (활성화된 경우)
    if config.auto_sync_symbols {
        match sync_symbols(pool, &config.symbol_sync_config).await {
            Ok(synced) => {
                if synced > 0 {
                    info!(count = synced, "심볼 동기화 완료");
                }
            }
            Err(e) => {
                warn!(error = %e, "심볼 동기화 실패, 기존 심볼로 진행");
            }
        }
    }

    let stale_threshold = Utc::now() - ChronoDuration::days(config.stale_days);

    // 2. 오래된 데이터를 가진 심볼 조회
    let stale_symbols = SymbolFundamentalRepository::get_stale_symbols(
        pool,
        stale_threshold,
        config.batch_size,
    )
    .await?;

    if stale_symbols.is_empty() {
        debug!("업데이트가 필요한 심볼 없음");
        return Ok(());
    }

    info!(
        count = stale_symbols.len(),
        update_ohlcv = config.update_ohlcv,
        "Fundamental 데이터 수집 시작"
    );

    // Yahoo Finance fetcher 생성 (get_ticker_info는 &mut self 필요)
    let mut fetcher = match FundamentalFetcher::new() {
        Ok(f) => f,
        Err(e) => {
            error!(error = %e, "FundamentalFetcher 생성 실패");
            return Err(e.into());
        }
    };

    // OHLCV 캐시 (업데이트 활성화 시)
    let ohlcv_cache = if config.update_ohlcv {
        Some(OhlcvCache::new(pool.clone()))
    } else {
        None
    };

    let mut success_count = 0;
    let mut error_count = 0;
    let mut ohlcv_count = 0;

    for (symbol_info_id, ticker, market) in stale_symbols {
        // 참고: CRYPTO 심볼은 쿼리 단계에서 이미 제외됨 (get_stale_symbols)

        // Yahoo Finance 심볼 형식으로 변환
        let yahoo_symbol = FundamentalFetcher::to_yahoo_symbol(&ticker, &market);

        debug!(
            ticker = %ticker,
            market = %market,
            yahoo_symbol = %yahoo_symbol,
            "Fundamental 데이터 수집 중"
        );

        // OHLCV 업데이트가 활성화된 경우 통합 수집
        if let Some(ref cache) = ohlcv_cache {
            match fetcher.fetch_with_ohlcv(&yahoo_symbol, &ticker, &market).await {
                Ok(result) => {
                    // 종목명 업데이트 (Yahoo에서 가져온 종목명이 있는 경우)
                    if let Some(ref name) = result.name {
                        if let Err(e) = update_symbol_name(pool, symbol_info_id, name).await {
                            warn!(ticker = %ticker, error = %e, "종목명 업데이트 실패");
                        }
                    }

                    // Fundamental 데이터 저장
                    let new_fundamental = convert_to_new_fundamental(symbol_info_id, &result.fundamental);
                    match SymbolFundamentalRepository::upsert(pool, &new_fundamental).await {
                        Ok(_) => {
                            success_count += 1;
                            debug!(ticker = %ticker, "Fundamental 데이터 저장 완료");
                        }
                        Err(e) => {
                            error_count += 1;
                            // 디버깅용 상세 로그: 어떤 값이 문제인지 파악
                            warn!(
                                ticker = %ticker,
                                error = %e,
                                market_cap = ?new_fundamental.market_cap,
                                week_52_high = ?new_fundamental.week_52_high,
                                week_52_low = ?new_fundamental.week_52_low,
                                per = ?new_fundamental.per,
                                forward_per = ?new_fundamental.forward_per,
                                pbr = ?new_fundamental.pbr,
                                eps = ?new_fundamental.eps,
                                bps = ?new_fundamental.bps,
                                dividend_yield = ?new_fundamental.dividend_yield,
                                roe = ?new_fundamental.roe,
                                roa = ?new_fundamental.roa,
                                operating_margin = ?new_fundamental.operating_margin,
                                net_profit_margin = ?new_fundamental.net_profit_margin,
                                gross_margin = ?new_fundamental.gross_margin,
                                debt_ratio = ?new_fundamental.debt_ratio,
                                current_ratio = ?new_fundamental.current_ratio,
                                quick_ratio = ?new_fundamental.quick_ratio,
                                revenue_growth_yoy = ?new_fundamental.revenue_growth_yoy,
                                earnings_growth_yoy = ?new_fundamental.earnings_growth_yoy,
                                "Fundamental 데이터 저장 실패 - 값 상세"
                            );
                        }
                    }

                    // OHLCV 데이터 저장
                    if !result.klines.is_empty() {
                        match cache.save_klines(&ticker, Timeframe::D1, &result.klines).await {
                            Ok(saved) => {
                                ohlcv_count += saved;
                                debug!(
                                    ticker = %ticker,
                                    klines = result.klines.len(),
                                    saved = saved,
                                    "OHLCV 데이터 저장 완료"
                                );
                            }
                            Err(e) => {
                                warn!(ticker = %ticker, error = %e, "OHLCV 데이터 저장 실패");
                            }
                        }
                    }
                }
                Err(e) => {
                    error_count += 1;
                    warn!(
                        ticker = %ticker,
                        yahoo_symbol = %yahoo_symbol,
                        error = %e,
                        "Yahoo Finance 통합 수집 실패"
                    );
                }
            }
        } else {
            // OHLCV 비활성화: 기존 fetch 메서드 사용
            match fetcher.fetch(&yahoo_symbol).await {
                Ok(data) => {
                    let new_fundamental = convert_to_new_fundamental(symbol_info_id, &data);
                    match SymbolFundamentalRepository::upsert(pool, &new_fundamental).await {
                        Ok(_) => {
                            success_count += 1;
                            debug!(ticker = %ticker, "Fundamental 데이터 저장 완료");
                        }
                        Err(e) => {
                            error_count += 1;
                            warn!(ticker = %ticker, error = %e, "Fundamental 데이터 저장 실패");
                        }
                    }
                }
                Err(e) => {
                    error_count += 1;
                    warn!(
                        ticker = %ticker,
                        yahoo_symbol = %yahoo_symbol,
                        error = %e,
                        "Yahoo Finance 데이터 수집 실패"
                    );
                }
            }
        }

        // Rate limiting: API 요청 간 딜레이
        tokio::time::sleep(config.request_delay).await;
    }

    info!(
        success = success_count,
        errors = error_count,
        ohlcv_saved = ohlcv_count,
        "Fundamental 데이터 수집 배치 완료"
    );

    Ok(())
}

/// FundamentalData를 NewSymbolFundamental로 변환.
///
/// DB 저장 전 모든 Decimal 값을 적절한 소수점 자릿수로 반올림합니다:
/// - 시가총액, 매출액 등: 2자리 (DECIMAL(20, 2))
/// - 가격, 비율 등: 4자리 (DECIMAL(12, 4) 또는 DECIMAL(20, 4))
fn convert_to_new_fundamental(
    symbol_info_id: uuid::Uuid,
    data: &FundamentalData,
) -> NewSymbolFundamental {
    NewSymbolFundamental {
        symbol_info_id,
        // 시가총액 - DECIMAL(20, 2)
        market_cap: round_dp2(data.market_cap),
        shares_outstanding: data.shares_outstanding,
        float_shares: data.float_shares,
        // 가격 관련 - DECIMAL(20, 4)
        week_52_high: round_dp4(data.week_52_high),
        week_52_low: round_dp4(data.week_52_low),
        avg_volume_10d: data.avg_volume_10d,
        avg_volume_3m: data.avg_volume_3m,
        // 밸류에이션 - DECIMAL(12, 4)
        per: round_dp4(data.per),
        forward_per: round_dp4(data.forward_per),
        pbr: round_dp4(data.pbr),
        psr: round_dp4(data.psr),
        ev_ebitda: round_dp4(data.ev_ebitda),
        // 주당 지표 - DECIMAL(20, 4)
        eps: round_dp4(data.eps),
        bps: round_dp4(data.bps),
        dps: round_dp4(data.dps),
        // 배당 - DECIMAL(8, 4)
        dividend_yield: round_dp4(data.dividend_yield),
        dividend_payout_ratio: round_dp4(data.dividend_payout_ratio),
        ex_dividend_date: data.ex_dividend_date,
        // 수익성 - DECIMAL(8, 4)
        roe: round_dp4(data.roe),
        roa: round_dp4(data.roa),
        operating_margin: round_dp4(data.operating_margin),
        net_profit_margin: round_dp4(data.net_profit_margin),
        gross_margin: round_dp4(data.gross_margin),
        // 안정성 - DECIMAL(12, 4)
        debt_ratio: round_dp4(data.debt_ratio),
        current_ratio: round_dp4(data.current_ratio),
        quick_ratio: round_dp4(data.quick_ratio),
        // 성장성 - DECIMAL(8, 4)
        revenue_growth_yoy: round_dp4(data.revenue_growth_yoy),
        earnings_growth_yoy: round_dp4(data.earnings_growth_yoy),
        currency: data.currency.clone(),
        data_source: Some("yahoo_finance".to_string()),
        // 기본값 사용하는 필드들
        pcr: None,
        sps: None,
        interest_coverage: None,
        revenue: None,
        operating_income: None,
        net_income: None,
        total_assets: None,
        total_liabilities: None,
        total_equity: None,
        revenue_growth_3y: None,
        earnings_growth_3y: None,
        fiscal_year_end: None,
    }
}

/// 종목명 업데이트 (symbol_info 테이블).
///
/// 종목명이 비어있거나 티커와 동일한 경우에만 업데이트합니다.
async fn update_symbol_name(
    pool: &PgPool,
    symbol_info_id: uuid::Uuid,
    name: &str,
) -> Result<(), sqlx::Error> {
    // 현재 종목명이 티커와 같거나 비어있는 경우에만 업데이트
    sqlx::query(
        r#"
        UPDATE symbol_info
        SET name = $2, updated_at = NOW()
        WHERE id = $1
          AND (name = '' OR name = ticker OR name IS NULL)
        "#,
    )
    .bind(symbol_info_id)
    .bind(name)
    .execute(pool)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = FundamentalCollectorConfig::default();
        assert_eq!(config.collect_interval.as_secs(), 3600);
        assert_eq!(config.stale_days, 7);
        assert_eq!(config.batch_size, 50);
        assert_eq!(config.request_delay.as_millis(), 2000);
        assert!(config.update_ohlcv);
    }
}
