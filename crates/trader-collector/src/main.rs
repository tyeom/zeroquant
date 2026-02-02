//! Standalone data collector CLI.

use clap::{Parser, Subcommand};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use trader_collector::{modules, CollectorConfig};

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
    },

    /// Fundamental 데이터 수집 (향후 구현)
    #[allow(dead_code)]
    CollectFundamental {
        /// 특정 심볼만 수집 (쉼표로 구분)
        #[arg(long)]
        symbols: Option<String>,
    },

    /// 전체 워크플로우 실행 (심볼 동기화 → OHLCV)
    RunAll,

    /// 데몬 모드: 주기적으로 전체 워크플로우 실행
    Daemon,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // 로깅 초기화
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("trader_collector={}", cli.log_level).into()),
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
        Commands::CollectOhlcv { symbols } => {
            let stats = modules::collect_ohlcv(&pool, &config, symbols).await?;
            stats.log_summary("OHLCV 수집");
        }
        Commands::CollectFundamental { .. } => {
            tracing::warn!("Fundamental 수집 기능은 아직 구현되지 않았습니다");
            return Err("Not implemented yet".into());
        }
        Commands::RunAll => {
            tracing::info!("=== 전체 워크플로우 시작 ===");

            // 1. 심볼 동기화
            tracing::info!("Step 1/2: 심볼 동기화");
            let sync_stats = modules::sync_symbols(&pool, &config).await?;
            sync_stats.log_summary("심볼 동기화");

            // 2. OHLCV 수집
            tracing::info!("Step 2/2: OHLCV 수집");
            let ohlcv_stats = modules::collect_ohlcv(&pool, &config, None).await?;
            ohlcv_stats.log_summary("OHLCV 수집");

            tracing::info!("=== 전체 워크플로우 완료 ===");
        }
        Commands::Daemon => {
            tracing::info!(
                "=== 데몬 모드 시작 (주기: {}분) ===",
                config.daemon.interval_minutes
            );

            let mut interval = tokio::time::interval(config.daemon.interval());
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        tracing::info!("종료 신호 수신, 데몬 종료 중...");
                        break;
                    }
                    _ = interval.tick() => {
                        tracing::info!("=== 워크플로우 실행 시작 ===");

                        // 1. 심볼 동기화
                        match modules::sync_symbols(&pool, &config).await {
                            Ok(stats) => {
                                stats.log_summary("심볼 동기화");
                            }
                            Err(e) => {
                                tracing::error!("심볼 동기화 실패: {}", e);
                            }
                        }

                        // 2. OHLCV 수집
                        match modules::collect_ohlcv(&pool, &config, None).await {
                            Ok(stats) => {
                                stats.log_summary("OHLCV 수집");
                            }
                            Err(e) => {
                                tracing::error!("OHLCV 수집 실패: {}", e);
                            }
                        }

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
