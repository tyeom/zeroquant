//! Standalone data collector CLI.

use clap::{Parser, Subcommand};
use sqlx::PgPool;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use trader_collector::{modules, CollectorConfig};

/// 전체 워크플로우 실행 (에러 시 로깅 후 계속).
async fn run_workflow(pool: &PgPool, config: &CollectorConfig) {
    // 1. 심볼 동기화
    match modules::sync_symbols(pool, config).await {
        Ok(stats) => stats.log_summary("심볼 동기화"),
        Err(e) => tracing::error!("심볼 동기화 실패: {}", e),
    }

    // 2. Fundamental 동기화 (PER, PBR, 섹터 등)
    // 우선순위: KRX API > 네이버 금융
    if config.providers.krx_api_enabled {
        match modules::sync_krx_fundamentals(pool, &config.fundamental_collect).await {
            Ok(stats) => tracing::info!(
                processed = stats.processed,
                valuation = stats.valuation_updated,
                sector = stats.sector_updated,
                "KRX Fundamental 동기화 완료"
            ),
            Err(e) => tracing::error!("KRX Fundamental 동기화 실패: {}", e),
        }
    } else if config.providers.naver_enabled {
        // KRX API가 없으면 네이버 금융으로 fallback
        match modules::sync_naver_fundamentals(pool, config.providers.naver_request_delay_ms, None)
            .await
        {
            Ok(stats) => tracing::info!(
                processed = stats.processed,
                valuation = stats.valuation_updated,
                sector = stats.sector_updated,
                "네이버 Fundamental 동기화 완료"
            ),
            Err(e) => tracing::error!("네이버 Fundamental 동기화 실패: {}", e),
        }
    } else {
        tracing::info!("Fundamental 동기화 건너뜀 (KRX API, 네이버 모두 비활성화)");
    }

    // 3. OHLCV 수집 (지표도 함께 계산) - 데몬 모드에서는 24시간 증분 수집
    match modules::collect_ohlcv(pool, config, None, Some(24)).await {
        Ok(stats) => stats.log_summary("OHLCV 수집"),
        Err(e) => tracing::error!("OHLCV 수집 실패: {}", e),
    }

    // 4. 분석 지표 동기화 (누락된 지표 보완)
    match modules::sync_indicators(pool, config, None).await {
        Ok(stats) => stats.log_summary("지표 동기화"),
        Err(e) => tracing::error!("지표 동기화 실패: {}", e),
    }

    // 5. GlobalScore 동기화 (랭킹용)
    match modules::sync_global_scores(pool, config, None).await {
        Ok(stats) => stats.log_summary("GlobalScore 동기화"),
        Err(e) => tracing::error!("GlobalScore 동기화 실패: {}", e),
    }

    // 6. 스크리닝 Materialized View 갱신
    match modules::refresh_screening_view(pool).await {
        Ok(stats) => stats.log_summary("스크리닝 뷰 갱신"),
        Err(e) => tracing::error!("스크리닝 뷰 갱신 실패: {}", e),
    }
}

#[derive(Parser)]
#[command(name = "trader-collector")]
#[command(about = "ZeroQuant Standalone Data Collector", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// 로그 레벨 (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[derive(Subcommand)]
enum Commands {
    /// 심볼 정보 동기화 (KRX, Binance, Yahoo)
    SyncSymbols,

    /// OHLCV 데이터 수집 (일봉)
    CollectOhlcv {
        /// 특정 심볼만 수집 (쉼표로 구분, 예: "005930,000660")
        #[arg(long)]
        symbols: Option<String>,

        /// 증분 수집: 이 시간(hours) 이전에 업데이트된 심볼만 수집
        /// 예: --stale-hours 24 (24시간 이상 지난 심볼만)
        #[arg(long)]
        stale_hours: Option<u32>,

        /// 이전 중단점부터 재개
        #[arg(long)]
        resume: bool,
    },

    /// 체크포인트 상태 조회/관리
    Checkpoint {
        #[command(subcommand)]
        action: CheckpointAction,
    },

    /// 분석 지표 동기화 (RouteState, MarketRegime, TTM Squeeze)
    SyncIndicators {
        /// 특정 심볼만 처리 (쉼표로 구분, 예: "005930,000660")
        #[arg(long)]
        symbols: Option<String>,

        /// 이전 중단점부터 재개
        #[arg(long)]
        resume: bool,

        /// N시간 이내 업데이트된 심볼 스킵
        #[arg(long)]
        stale_hours: Option<u32>,
    },

    /// GlobalScore 동기화 (랭킹용 종합 점수)
    SyncGlobalScores {
        /// 특정 심볼만 처리 (쉼표로 구분, 예: "005930,000660")
        #[arg(long)]
        symbols: Option<String>,

        /// 이전 중단점부터 재개
        #[arg(long)]
        resume: bool,

        /// N시간 이내 업데이트된 심볼 스킵
        #[arg(long)]
        stale_hours: Option<u32>,
    },

    /// KRX Fundamental 데이터 동기화 (PER, PBR, 배당수익률, 섹터 등)
    SyncKrxFundamentals,

    /// 네이버 금융 Fundamental 데이터 동기화 (KR 시장)
    /// KRX API 없이 네이버 크롤링으로 PER, PBR, ROE, 섹터, 시장타입 등 수집
    SyncNaverFundamentals {
        /// 배치당 처리할 심볼 수 (기본: 전체)
        #[arg(long)]
        batch_size: Option<i64>,

        /// 특정 심볼 하나만 처리 (테스트용)
        #[arg(long)]
        ticker: Option<String>,

        /// 이전 중단점부터 재개
        #[arg(long)]
        resume: bool,

        /// N시간 이내 업데이트된 심볼 스킵
        #[arg(long)]
        stale_hours: Option<u32>,
    },

    /// 스크리닝 Materialized View 갱신
    /// symbol_info + fundamental + global_score 통합 뷰 갱신
    RefreshScreening,

    /// 전체 워크플로우 실행 (심볼 → Fundamental → OHLCV → 지표 → GlobalScore → 스크리닝)
    RunAll {
        /// 특정 심볼만 처리 (테스트용, 예: "005930")
        #[arg(long)]
        ticker: Option<String>,
    },

    /// 데몬 모드: 주기적으로 전체 워크플로우 실행
    Daemon,
}

/// 체크포인트 관리 액션
#[derive(Subcommand)]
enum CheckpointAction {
    /// 모든 체크포인트 상태 조회
    List,

    /// 특정 워크플로우의 체크포인트 삭제
    Clear {
        /// 워크플로우 이름 (naver_fundamental, indicator_sync, global_score_sync)
        workflow: String,
    },

    /// 실행 중인 워크플로우를 interrupted 상태로 마킹
    Interrupt {
        /// 워크플로우 이름
        workflow: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // 로깅 초기화 (trader_collector, trader_data 모두 포함)
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!(
                    "trader_collector={},trader_data={},trader_analytics={}",
                    cli.log_level, cli.log_level, cli.log_level
                )
                .into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("ZeroQuant Data Collector 시작");

    // 설정 로드
    let config = CollectorConfig::from_env()?;
    tracing::debug!(database_url = %config.database_url, "설정 로드 완료");

    // DB 연결
    let pool = sqlx::PgPool::connect(&config.database_url).await?;
    tracing::info!("데이터베이스 연결 성공");

    // 명령 실행
    match cli.command {
        Commands::SyncSymbols => {
            let stats = modules::sync_symbols(&pool, &config).await?;
            stats.log_summary("심볼 동기화");
        }
        Commands::CollectOhlcv {
            symbols,
            stale_hours,
            resume,
        } => {
            if resume {
                tracing::info!("OHLCV resume 모드는 현재 stale_hours 옵션으로 대체 가능합니다");
            }
            let stats = modules::collect_ohlcv(&pool, &config, symbols, stale_hours).await?;
            stats.log_summary("OHLCV 수집");
        }
        Commands::Checkpoint { action } => match action {
            CheckpointAction::List => {
                let checkpoints = modules::list_checkpoints(&pool).await?;
                if checkpoints.is_empty() {
                    println!("저장된 체크포인트가 없습니다.");
                } else {
                    println!("\n📋 체크포인트 상태:");
                    println!("{:-<80}", "");
                    for cp in checkpoints {
                        println!(
                            "  {:<25} | 상태: {:<12} | 처리: {:>5}개 | 마지막: {}",
                            cp.workflow_name,
                            cp.status,
                            cp.total_processed,
                            cp.last_ticker.unwrap_or_else(|| "-".to_string())
                        );
                    }
                    println!("{:-<80}", "");
                }
            }
            CheckpointAction::Clear { workflow } => {
                modules::clear_checkpoint(&pool, &workflow).await?;
                println!("✅ {} 체크포인트 삭제 완료", workflow);
            }
            CheckpointAction::Interrupt { workflow } => {
                modules::mark_interrupted(&pool, &workflow).await?;
                println!("✅ {} 워크플로우를 interrupted 상태로 마킹", workflow);
            }
        },
        Commands::SyncIndicators {
            symbols,
            resume,
            stale_hours,
        } => {
            let options = modules::IndicatorSyncOptions {
                resume,
                stale_hours,
            };
            let stats =
                modules::sync_indicators_with_options(&pool, &config, symbols, options).await?;
            stats.log_summary("지표 동기화");
        }
        Commands::SyncGlobalScores {
            symbols,
            resume,
            stale_hours,
        } => {
            let options = modules::GlobalScoreSyncOptions {
                resume,
                stale_hours,
            };
            let stats =
                modules::sync_global_scores_with_options(&pool, &config, symbols, options).await?;
            stats.log_summary("GlobalScore 동기화");
        }
        Commands::SyncKrxFundamentals => {
            if !config.providers.krx_api_enabled {
                tracing::warn!("KRX API가 비활성화되어 있습니다. PROVIDER_KRX_API_ENABLED=true로 활성화하세요.");
                return Ok(());
            }
            let stats = modules::sync_krx_fundamentals(&pool, &config.fundamental_collect).await?;
            tracing::info!(
                processed = stats.processed,
                valuation = stats.valuation_updated,
                market_cap = stats.market_cap_updated,
                sector = stats.sector_updated,
                "KRX Fundamental 동기화 완료"
            );
        }
        Commands::SyncNaverFundamentals {
            batch_size,
            ticker,
            resume,
            stale_hours,
        } => {
            if !config.providers.naver_enabled {
                tracing::warn!("네이버 금융이 비활성화되어 있습니다. NAVER_FUNDAMENTAL_ENABLED=true로 활성화하세요.");
                return Ok(());
            }

            // 단일 종목 테스트 모드
            if let Some(t) = ticker {
                tracing::info!("단일 종목 테스트: {}", t);
                match modules::fetch_and_save_naver_fundamental(&pool, &t).await {
                    Ok(data) => {
                        println!("\n✅ 네이버 데이터 수집 완료: {}", t);
                        println!("  종목명: {:?}", data.name);
                        println!("  시장: {}", data.market_type);
                        println!("  섹터: {:?}", data.sector);
                        println!("  시가총액: {:?}", data.market_cap);
                        println!("  PER: {:?}", data.per);
                        println!("  PBR: {:?}", data.pbr);
                        println!("  ROE: {:?}", data.roe);
                        println!("  52주 고가: {:?}", data.week_52_high);
                        println!("  52주 저가: {:?}", data.week_52_low);
                    }
                    Err(e) => {
                        tracing::error!("네이버 데이터 수집 실패: {}", e);
                        return Err(e.into());
                    }
                }
            } else {
                // 배치 모드 (옵션 포함)
                let options = modules::NaverSyncOptions {
                    request_delay_ms: config.providers.naver_request_delay_ms,
                    batch_size,
                    resume,
                    stale_hours,
                };
                let stats = modules::sync_naver_fundamentals_with_options(&pool, options).await?;
                tracing::info!(
                    processed = stats.processed,
                    valuation = stats.valuation_updated,
                    market_cap = stats.market_cap_updated,
                    sector = stats.sector_updated,
                    week_52 = stats.week_52_updated,
                    market_type = stats.market_type_updated,
                    failed = stats.failed,
                    "네이버 Fundamental 동기화 완료"
                );
            }
        }
        Commands::RefreshScreening => {
            let stats = modules::refresh_screening_view(&pool).await?;
            stats.log_summary("스크리닝 뷰 갱신");

            // 통계 출력
            if let Ok(view_stats) = modules::get_screening_view_stats(&pool).await {
                println!("\n📊 스크리닝 뷰 통계:");
                println!("  총 레코드: {}", view_stats.total_rows);
                println!("  Global Score 있음: {}", view_stats.with_score);
                println!("  Fundamental 있음: {}", view_stats.with_fundamental);
                println!("  시장별:");
                for (market, count) in &view_stats.by_market {
                    println!("    {}: {}", market, count);
                }
            }
        }
        Commands::RunAll { ticker } => {
            let is_single = ticker.is_some();
            let symbols_filter = ticker.clone();

            if is_single {
                tracing::info!(
                    "=== 단일 종목 워크플로우 시작: {} ===",
                    ticker.as_ref().unwrap()
                );
            } else {
                tracing::info!("=== 전체 워크플로우 시작 ===");
            }

            // 1. 심볼 동기화 (단일 종목 모드에서는 건너뜀)
            if !is_single {
                tracing::info!("Step 1/6: 심볼 동기화");
                let sync_stats = modules::sync_symbols(&pool, &config).await?;
                sync_stats.log_summary("심볼 동기화");
            } else {
                tracing::info!("Step 1/6: 심볼 동기화 (건너뜀 - 단일 종목 모드)");
            }

            // 2. Fundamental 동기화 (PER, PBR, 섹터 등)
            tracing::info!("Step 2/6: Fundamental 동기화");
            if let Some(ref t) = ticker {
                // 단일 종목: 네이버 금융으로 직접 수집
                if config.providers.naver_enabled {
                    match modules::fetch_and_save_naver_fundamental(&pool, t).await {
                        Ok(data) => {
                            println!("\n✅ 네이버 Fundamental 수집 완료: {}", t);
                            println!("  종목명: {:?}", data.name);
                            println!("  시장: {}", data.market_type);
                            println!("  섹터: {:?}", data.sector);
                            println!(
                                "  PER: {:?}, PBR: {:?}, ROE: {:?}",
                                data.per, data.pbr, data.roe
                            );
                        }
                        Err(e) => tracing::error!("네이버 Fundamental 수집 실패: {}", e),
                    }
                }
            } else if config.providers.krx_api_enabled {
                let krx_stats =
                    modules::sync_krx_fundamentals(&pool, &config.fundamental_collect).await?;
                tracing::info!(
                    processed = krx_stats.processed,
                    valuation = krx_stats.valuation_updated,
                    sector = krx_stats.sector_updated,
                    "KRX Fundamental 동기화 완료"
                );
            } else if config.providers.naver_enabled {
                // 24시간 이상 지난 데이터만 업데이트 (성장률 등 신규 필드 포함)
                let naver_options = modules::NaverSyncOptions {
                    request_delay_ms: config.providers.naver_request_delay_ms,
                    batch_size: None,
                    resume: false,
                    stale_hours: Some(24),
                };
                let naver_stats = modules::sync_naver_fundamentals_with_options(
                    &pool,
                    naver_options,
                )
                .await?;
                tracing::info!(
                    processed = naver_stats.processed,
                    valuation = naver_stats.valuation_updated,
                    sector = naver_stats.sector_updated,
                    "네이버 Fundamental 동기화 완료"
                );
            } else {
                tracing::info!("Fundamental 동기화 건너뜀 (KRX API, 네이버 모두 비활성화)");
            }

            // 3. OHLCV 수집 (지표도 함께 계산)
            tracing::info!("Step 3/6: OHLCV 수집");
            let ohlcv_stats =
                modules::collect_ohlcv(&pool, &config, symbols_filter.clone(), None).await?;
            ohlcv_stats.log_summary("OHLCV 수집");

            // 4. 분석 지표 동기화 (누락된 지표 보완)
            tracing::info!("Step 4/6: 분석 지표 동기화");
            let indicator_stats =
                modules::sync_indicators(&pool, &config, symbols_filter.clone()).await?;
            indicator_stats.log_summary("지표 동기화");

            // 5. GlobalScore 동기화 (랭킹용)
            tracing::info!("Step 5/6: GlobalScore 동기화");
            let global_score_stats =
                modules::sync_global_scores(&pool, &config, symbols_filter.clone()).await?;
            global_score_stats.log_summary("GlobalScore 동기화");

            // 6. 스크리닝 Materialized View 갱신
            tracing::info!("Step 6/6: 스크리닝 뷰 갱신");
            let screening_stats = modules::refresh_screening_view(&pool).await?;
            screening_stats.log_summary("스크리닝 뷰 갱신");

            if is_single {
                tracing::info!(
                    "=== 단일 종목 워크플로우 완료: {} ===",
                    ticker.as_ref().unwrap()
                );
            } else {
                tracing::info!("=== 전체 워크플로우 완료 ===");
            }
        }
        Commands::Daemon => {
            tracing::info!(
                "=== 데몬 모드 시작 (주기: {}분) ===",
                config.daemon.interval_minutes
            );

            // 데몬 시작 시 즉시 한 번 실행
            tracing::info!("=== 초기 워크플로우 실행 시작 ===");
            run_workflow(&pool, &config).await;
            tracing::info!(
                "=== 초기 워크플로우 완료, 다음 실행: {}분 후 ===",
                config.daemon.interval_minutes
            );

            let mut interval = tokio::time::interval(config.daemon.interval());
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            // 첫 tick은 즉시 발생하므로 건너뜀 (이미 위에서 실행함)
            interval.tick().await;

            loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        tracing::info!("종료 신호 수신, 데몬 종료 중...");
                        break;
                    }
                    _ = interval.tick() => {
                        tracing::info!("=== 워크플로우 실행 시작 ===");
                        run_workflow(&pool, &config).await;
                        tracing::info!(
                            "=== 워크플로우 완료, 다음 실행: {}분 후 ===",
                            config.daemon.interval_minutes
                        );
                    }
                }
            }
        }
    }

    pool.close().await;
    tracing::info!("ZeroQuant Data Collector 종료");

    Ok(())
}
