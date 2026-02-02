//! 트레이딩 봇 CLI.
//!
//! # 사용 예시
//!
//! ```bash
//! # 삼성전자 일봉 다운로드 (한국 시장)
//! trader download -m KR -s 005930 -f 2024-01-01 -t 2024-12-31
//!
//! # 코스닥 종목 다운로드
//! trader download -m KR -s 035720 --kosdaq -f 2024-01-01 -t 2024-12-31
//!
//! # SPY ETF 다운로드 (미국 시장)
//! trader download -m US -s SPY -f 2024-01-01 -t 2024-12-31
//!
//! # 인기 종목 목록 보기
//! trader list -m KR
//! trader list -m US
//! ```

use clap::{Parser, Subcommand};
use tracing::{error, info};

mod commands;

use commands::download::{
    download_data, parse_date, print_available_symbols, DownloadConfig, Interval, Market,
};
use commands::import::{import_to_db, ImportDbConfig};

#[derive(Parser)]
#[command(name = "trader")]
#[command(about = "Trading bot CLI - 한국투자증권 기반 자동 거래 시스템", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 과거 OHLCV 데이터 다운로드 (Yahoo Finance → KIS API fallback)
    Download {
        /// 시장 유형 (KR: 한국, US: 미국)
        #[arg(short, long)]
        market: String,

        /// 종목 코드/심볼 (예: 005930, SPY)
        #[arg(short, long)]
        symbol: String,

        /// 타임프레임 간격 (1d: 일봉, 1w: 주봉, 1m: 월봉)
        #[arg(short, long, default_value = "1d")]
        interval: String,

        /// 시작 날짜 (YYYY-MM-DD)
        #[arg(short = 'f', long)]
        from: String,

        /// 종료 날짜 (YYYY-MM-DD)
        #[arg(short, long)]
        to: String,

        /// 출력 파일 경로 (자동 생성됨)
        #[arg(short, long)]
        output: Option<String>,

        /// 코스닥 종목 여부 (한국 시장 전용)
        #[arg(long, default_value = "false")]
        kosdaq: bool,
    },

    /// 인기 종목 목록 보기
    List {
        /// 시장 유형 (KR: 한국, US: 미국)
        #[arg(short, long)]
        market: String,
    },

    /// 과거 데이터 가져오기 (download의 별칭, 일봉 기본)
    Import {
        /// 시장 유형 (KR: 한국, US: 미국)
        #[arg(short, long)]
        market: String,

        /// 종목 코드/심볼
        #[arg(short, long)]
        symbol: String,

        /// 시작 날짜 (YYYY-MM-DD)
        #[arg(short = 'f', long)]
        from: String,

        /// 종료 날짜 (YYYY-MM-DD)
        #[arg(short, long)]
        to: String,

        /// 코스닥 종목 여부
        #[arg(long, default_value = "false")]
        kosdaq: bool,
    },

    /// 데이터를 TimescaleDB에 저장 (Yahoo Finance → DB)
    ImportDb {
        /// 시장 유형 (KR: 한국, US: 미국)
        #[arg(short, long)]
        market: String,

        /// 종목 코드/심볼 (예: 005930, SPY)
        #[arg(short, long)]
        symbol: String,

        /// 타임프레임 간격 (1d: 일봉, 1w: 주봉, 1m: 월봉)
        #[arg(short, long, default_value = "1d")]
        interval: String,

        /// 시작 날짜 (YYYY-MM-DD)
        #[arg(short = 'f', long)]
        from: String,

        /// 종료 날짜 (YYYY-MM-DD)
        #[arg(short, long)]
        to: String,

        /// 코스닥 종목 여부 (한국 시장 전용)
        #[arg(long, default_value = "false")]
        kosdaq: bool,

        /// 데이터베이스 URL (기본: DATABASE_URL 환경변수)
        #[arg(long)]
        db_url: Option<String>,
    },

    /// DB에서 종목 목록 조회
    ListSymbols {
        /// 시장 필터 (KR, US, CRYPTO, ALL 등)
        #[arg(short, long, default_value = "ALL")]
        market: String,

        /// 활성화된 종목만 조회
        #[arg(long, default_value = "true")]
        active_only: bool,

        /// 출력 형식 (table, csv, json)
        #[arg(short, long, default_value = "table")]
        format: String,

        /// 출력 파일 경로 (지정하지 않으면 stdout)
        #[arg(short, long)]
        output: Option<String>,

        /// 검색 키워드 (종목명 또는 티커)
        #[arg(short, long)]
        search: Option<String>,

        /// 최대 결과 수 (0 = 무제한)
        #[arg(long, default_value = "0")]
        limit: usize,

        /// 데이터베이스 URL (기본: DATABASE_URL 환경변수)
        #[arg(long)]
        db_url: Option<String>,
    },

    /// 온라인 소스에서 종목 정보 자동 수집 및 DB 동기화
    FetchSymbols {
        /// 시장 유형 (KR: 한국, US: 미국, CRYPTO: 암호화폐, ALL: 전체)
        #[arg(short, long, default_value = "ALL")]
        market: String,

        /// CSV 파일로도 저장 (선택적)
        #[arg(long)]
        save_csv: bool,

        /// CSV 출력 디렉토리 (기본: data)
        #[arg(long, default_value = "data")]
        csv_dir: String,

        /// 데이터베이스 URL (기본: DATABASE_URL 환경변수)
        #[arg(long)]
        db_url: Option<String>,

        /// 드라이런 모드 (DB에 저장하지 않음)
        #[arg(long, default_value = "false")]
        dry_run: bool,
    },

    /// 백테스트 실행
    Backtest {
        /// 전략 설정 파일 (TOML 또는 JSON)
        #[arg(short, long)]
        config: String,

        /// 시장 유형 (KR: 한국, US: 미국)
        #[arg(short, long)]
        market: String,

        /// 종목 코드/심볼 (예: 005930, SPY)
        #[arg(short, long)]
        symbol: String,

        /// 시작 날짜 (YYYY-MM-DD)
        #[arg(short = 'f', long)]
        from: Option<String>,

        /// 종료 날짜 (YYYY-MM-DD)
        #[arg(short, long)]
        to: Option<String>,

        /// 초기 자본금 (기본: 10,000,000원)
        #[arg(long, default_value = "10000000")]
        capital: String,

        /// 결과 저장 경로
        #[arg(short, long)]
        output: Option<String>,

        /// 사용 가능한 전략 목록 보기
        #[arg(long)]
        list_strategies: bool,
    },

    /// 시스템 상태 확인
    Health,

    /// 트레이딩 봇 시작
    Start {
        /// 설정 파일
        #[arg(short, long, default_value = "config/default.toml")]
        config: String,

        /// 드라이런 모드 (실제 주문 미실행)
        #[arg(long, default_value = "false")]
        dry_run: bool,
    },

    /// ML 모델 훈련 (Yahoo Finance 데이터 → ONNX)
    Train {
        /// 종목 심볼 (예: SPY, QQQ)
        #[arg(short, long)]
        symbol: Option<String>,

        /// 여러 심볼 (쉼표로 구분, 예: SPY,QQQ,IWM)
        #[arg(long)]
        symbols: Option<String>,

        /// 모델 유형 (xgboost, lightgbm, random_forest, gradient_boosting)
        #[arg(short, long, default_value = "xgboost")]
        model: String,

        /// 데이터 기간 (1y, 2y, 5y, 10y, max)
        #[arg(short, long, default_value = "5y")]
        period: String,

        /// 예측 기간 (일)
        #[arg(long, default_value = "5")]
        horizon: u32,

        /// 모델 이름 (기본: 자동 생성)
        #[arg(short, long)]
        name: Option<String>,

        /// 출력 디렉토리
        #[arg(short, long, default_value = "models")]
        output_dir: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 트레이싱 초기화
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Download {
            market,
            symbol,
            interval,
            from,
            to,
            output,
            kosdaq,
        } => {
            let market = Market::from_str(&market)
                .ok_or_else(|| format!("Invalid market: {}. Supported: KR, US", market))?;

            let interval = Interval::from_str(&interval).ok_or_else(|| {
                format!(
                    "Invalid interval: {}. Supported: 1d (daily), 1w (weekly), 1m (monthly)",
                    interval
                )
            })?;

            let start_date = parse_date(&from)?;
            let end_date = parse_date(&to)?;

            if start_date > end_date {
                return Err("Start date must be before end date".into());
            }

            // 출력 경로 자동 생성
            let output_path = output.unwrap_or_else(|| {
                let market_str = match market {
                    Market::KR => "kr",
                    Market::US => "us",
                };
                let interval_str = match interval {
                    Interval::D1 => "daily",
                    Interval::W1 => "weekly",
                    Interval::M1 => "monthly",
                };
                format!(
                    "data/{}/{}_{}_{}_to_{}.csv",
                    market_str,
                    symbol.to_uppercase(),
                    interval_str,
                    start_date.format("%Y%m%d"),
                    end_date.format("%Y%m%d")
                )
            });

            let config = DownloadConfig {
                market,
                symbol,
                interval,
                start_date,
                end_date,
                output_path: output_path.clone(),
                is_kosdaq: kosdaq,
            };

            info!("Output will be saved to: {}", output_path);

            match download_data(config).await {
                Ok(count) => {
                    info!("✅ Successfully downloaded {} candles", count);
                    println!("\n데이터 다운로드 완료: {} 캔들", count);
                    println!("저장 위치: {}", output_path);
                }
                Err(e) => {
                    error!("Download failed: {}", e);
                    return Err(e.into());
                }
            }
        }

        Commands::List { market } => {
            let market = Market::from_str(&market)
                .ok_or_else(|| format!("Invalid market: {}. Supported: KR, US", market))?;

            print_available_symbols(market);
        }

        Commands::Import {
            market,
            symbol,
            from,
            to,
            kosdaq,
        } => {
            let market = Market::from_str(&market)
                .ok_or_else(|| format!("Invalid market: {}. Supported: KR, US", market))?;

            let interval = Interval::D1; // Import는 일봉 기본
            let start_date = parse_date(&from)?;
            let end_date = parse_date(&to)?;

            let market_str = match market {
                Market::KR => "kr",
                Market::US => "us",
            };

            let output_path = format!(
                "data/{}/{}_daily_{}_to_{}.csv",
                market_str,
                symbol.to_uppercase(),
                start_date.format("%Y%m%d"),
                end_date.format("%Y%m%d")
            );

            let config = DownloadConfig {
                market,
                symbol,
                interval,
                start_date,
                end_date,
                output_path: output_path.clone(),
                is_kosdaq: kosdaq,
            };

            match download_data(config).await {
                Ok(count) => {
                    info!("✅ Successfully imported {} candles", count);
                    println!("\n데이터 가져오기 완료: {} 캔들", count);
                    println!("저장 위치: {}", output_path);
                }
                Err(e) => {
                    error!("Import failed: {}", e);
                    return Err(e.into());
                }
            }
        }

        Commands::ImportDb {
            market,
            symbol,
            interval,
            from,
            to,
            kosdaq,
            db_url,
        } => {
            let market = Market::from_str(&market)
                .ok_or_else(|| format!("Invalid market: {}. Supported: KR, US", market))?;

            let interval = Interval::from_str(&interval).ok_or_else(|| {
                format!(
                    "Invalid interval: {}. Supported: 1d (daily), 1w (weekly), 1m (monthly)",
                    interval
                )
            })?;

            let start_date = parse_date(&from)?;
            let end_date = parse_date(&to)?;

            if start_date > end_date {
                return Err("Start date must be before end date".into());
            }

            let config = ImportDbConfig {
                market,
                symbol: symbol.clone(),
                interval,
                start_date,
                end_date,
                is_kosdaq: kosdaq,
                db_url,
            };

            println!("\n📥 데이터를 TimescaleDB에 저장합니다...");
            let market_str = match market {
                Market::KR => "KR",
                Market::US => "US",
            };
            println!("시장: {}", market_str);
            println!("종목: {}", symbol.to_uppercase());
            println!("기간: {} ~ {}", start_date, end_date);

            match import_to_db(config).await {
                Ok(count) => {
                    info!("✅ Successfully imported {} candles to database", count);
                    println!("\n✅ 데이터베이스 저장 완료: {} 캔들", count);
                }
                Err(e) => {
                    error!("Import to database failed: {}", e);
                    return Err(e.into());
                }
            }
        }

        Commands::ListSymbols {
            market,
            active_only,
            format,
            output,
            search,
            limit,
            db_url,
        } => {
            use commands::list_symbols::{list_symbols, ListSymbolsConfig, OutputFormat};

            let output_format = OutputFormat::from_str(&format)?;

            let config = ListSymbolsConfig {
                market: market.clone(),
                active_only,
                format: output_format,
                output: output.clone(),
                search: search.clone(),
                limit,
                db_url: db_url.clone(),
            };

            match list_symbols(config).await {
                Ok(count) => {
                    info!("✅ Listed {} symbols", count);
                }
                Err(e) => {
                    error!("List symbols failed: {}", e);
                    return Err(e.into());
                }
            }
        }

        Commands::FetchSymbols {
            market,
            save_csv,
            csv_dir,
            db_url,
            dry_run,
        } => {
            use commands::fetch_symbols::{fetch_symbols, FetchSymbolsConfig};

            let config = FetchSymbolsConfig {
                market: market.clone(),
                save_csv,
                csv_dir: csv_dir.clone(),
                db_url: db_url.clone(),
                dry_run,
            };

            match fetch_symbols(config).await {
                Ok(result) => {
                    info!(
                        "✅ Fetched symbols: KR={}, US={}, CRYPTO={}, Total={}",
                        result.kr_count, result.us_count, result.crypto_count, result.total
                    );
                }
                Err(e) => {
                    error!("Fetch symbols failed: {}", e);
                    return Err(e.into());
                }
            }
        }

        Commands::Backtest {
            config,
            market,
            symbol,
            from,
            to,
            capital,
            output,
            list_strategies,
        } => {
            // 전략 목록 출력
            if list_strategies {
                commands::backtest::print_available_strategies();
                return Ok(());
            }

            let market = Market::from_str(&market)
                .ok_or_else(|| format!("Invalid market: {}. Supported: KR, US", market))?;

            let start_date = from.as_ref().map(|d| parse_date(d)).transpose()?;
            let end_date = to.as_ref().map(|d| parse_date(d)).transpose()?;

            let initial_capital = capital
                .parse::<rust_decimal::Decimal>()
                .map_err(|_| format!("Invalid capital: {}", capital))?;

            let backtest_config = commands::backtest::BacktestCliConfig {
                config_path: config.clone(),
                market,
                symbol: symbol.clone(),
                start_date,
                end_date,
                initial_capital,
                output_path: output.clone(),
                ..Default::default()
            };

            println!("\n📊 백테스트 실행 중...");
            println!("전략 설정: {}", config);
            let market_str = match market {
                Market::KR => "KR",
                Market::US => "US",
            };
            println!("시장: {}", market_str);
            println!("종목: {}", symbol.to_uppercase());
            if let (Some(s), Some(e)) = (&start_date, &end_date) {
                println!("기간: {} ~ {}", s, e);
            }
            println!("초기 자본: {}", initial_capital);

            match commands::backtest::run_backtest(backtest_config).await {
                Ok(report) => {
                    info!("✅ Backtest completed successfully");
                    if let Some(out) = output {
                        println!("\n📁 결과 저장됨: {}", out);
                    }
                }
                Err(e) => {
                    error!("Backtest failed: {}", e);
                    return Err(e.into());
                }
            }
        }

        Commands::Health => {
            info!("Checking system health...");
            println!("\n시스템 상태 확인 중...");

            // TODO: 실제 상태 확인 구현
            println!("✅ CLI 도구: 정상");
            println!("⚠️  KIS API 연결: 미확인 (설정 필요)");
            println!("⚠️  데이터베이스: 미확인 (설정 필요)");
        }

        Commands::Start { config, dry_run } => {
            info!("Starting trading bot with config: {}", config);

            if dry_run {
                println!("\n🔒 드라이런 모드: 실제 주문이 실행되지 않습니다.");
            }

            println!("\n⚠️  트레이딩 봇 시작 기능은 추후 구현 예정입니다.");
            println!("설정 파일: {}", config);
        }

        Commands::Train {
            symbol,
            symbols,
            model,
            period,
            horizon,
            name,
            output_dir,
        } => {
            info!("Starting ML model training...");
            println!("\n🤖 ML 모델 훈련 시작...");

            // Python 스크립트 경로
            let script_path = "tools/ml/train_model.py";

            // 인자 구성
            let mut args = vec![
                script_path.to_string(),
                "--model".to_string(),
                model.clone(),
                "--period".to_string(),
                period.clone(),
                "--horizon".to_string(),
                horizon.to_string(),
                "--output-dir".to_string(),
                output_dir.clone(),
            ];

            // 심볼 처리
            if let Some(s) = symbol {
                args.push("--symbol".to_string());
                args.push(s.clone());
                println!("심볼: {}", s);
            } else if let Some(syms) = symbols {
                args.push("--symbols".to_string());
                args.push(syms.clone());
                println!("심볼: {}", syms);
            } else {
                args.push("--symbol".to_string());
                args.push("SPY".to_string());
                println!("심볼: SPY (기본값)");
            }

            if let Some(n) = name {
                args.push("--name".to_string());
                args.push(n);
            }

            println!("모델: {}", model);
            println!("기간: {}", period);
            println!("예측 horizon: {}일", horizon);
            println!("출력 디렉토리: {}", output_dir);
            println!();

            // Python 실행
            let output = std::process::Command::new("python")
                .args(&args)
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit())
                .status();

            match output {
                Ok(status) => {
                    if status.success() {
                        info!("✅ ML model training completed successfully");
                        println!("\n✅ 모델 훈련 완료!");
                        println!("ONNX 모델이 {} 디렉토리에 저장되었습니다.", output_dir);
                        println!("\nRust에서 사용하려면:");
                        println!(
                            "  cp {}/[모델이름].onnx crates/trader-analytics/models/",
                            output_dir
                        );
                    } else {
                        error!("ML training failed with exit code: {:?}", status.code());
                        return Err("ML training failed".into());
                    }
                }
                Err(e) => {
                    error!("Failed to execute Python: {}", e);
                    println!("\n❌ Python 실행 실패: {}", e);
                    println!("\n필수 사항:");
                    println!("1. Python 3.9+ 설치");
                    println!("2. cd tools/ml && pip install -r requirements.txt");
                    return Err(e.into());
                }
            }
        }
    }

    Ok(())
}
