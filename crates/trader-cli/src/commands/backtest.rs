//! 백테스트 명령어.
//!
//! TimescaleDB에 저장된 과거 데이터로 전략을 백테스트합니다.
//!
//! # 사용 예시
//!
//! ```bash
//! # 삼성전자 데이터로 RSI 전략 백테스트
//! trader backtest -c config/backtest/rsi.toml -s 005930 -m KR
//!
//! # SPY 데이터로 Simple Power 전략 백테스트
//! trader backtest -c config/backtest/simple_power.toml -s SPY -m US
//!
//! # 특정 기간만 백테스트
//! trader backtest -c config/backtest/haa.toml -s SPY -m US -f 2024-01-01 -t 2024-12-31
//!
//! # 사용 가능한 전략 목록
//! trader backtest --list-strategies
//! ```

use anyhow::{anyhow, Result};
use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use std::path::Path;
use std::str::FromStr;
use tracing::{debug, info};

use trader_analytics::backtest::{BacktestConfig, BacktestEngine, BacktestReport};
use trader_core::{Kline, MarketType, Symbol, Timeframe};
use trader_data::{Database, DatabaseConfig, KlineRepository, SymbolRepository};
use trader_strategy::strategies::{
    BollingerStrategy, GridStrategy, HaaStrategy, MagicSplitStrategy, RsiStrategy,
    SimplePowerStrategy, StockRotationStrategy, VolatilityBreakoutStrategy, XaaStrategy,
};
use trader_strategy::Strategy;

use crate::commands::download::Market;

/// 백테스트 CLI 설정
#[derive(Debug, Clone)]
pub struct BacktestCliConfig {
    /// 전략 설정 파일 경로
    pub config_path: String,
    /// 시장 (KR/US)
    pub market: Market,
    /// 종목 코드
    pub symbol: String,
    /// 시작일 (옵션)
    pub start_date: Option<NaiveDate>,
    /// 종료일 (옵션)
    pub end_date: Option<NaiveDate>,
    /// 초기 자본금
    pub initial_capital: Decimal,
    /// 수수료율
    pub commission_rate: Decimal,
    /// 슬리피지율
    pub slippage_rate: Decimal,
    /// 데이터베이스 URL
    pub db_url: Option<String>,
    /// 결과 저장 경로 (옵션)
    pub output_path: Option<String>,
}

impl Default for BacktestCliConfig {
    fn default() -> Self {
        Self {
            config_path: String::new(),
            market: Market::KR,
            symbol: String::new(),
            start_date: None,
            end_date: None,
            initial_capital: Decimal::from(10_000_000), // 1천만원
            commission_rate: Decimal::from_str("0.00015").unwrap(), // 0.015% (한국 증권사 평균)
            slippage_rate: Decimal::from_str("0.0005").unwrap(), // 0.05%
            db_url: None,
            output_path: None,
        }
    }
}

/// 전략 설정 파일 형식
#[derive(Debug, Deserialize)]
pub struct StrategyConfigFile {
    /// 전략 이름
    pub name: String,
    /// 전략 타입
    pub strategy_type: String,
    /// 전략 매개변수
    #[serde(default)]
    pub parameters: serde_json::Value,
}

/// 지원하는 전략 타입
#[derive(Debug, Clone, Copy)]
pub enum StrategyType {
    Grid,
    Rsi,
    Bollinger,
    Volatility,
    MagicSplit,
    SimplePower,
    Haa,
    Xaa,
    StockRotation,
}

impl StrategyType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "grid" | "gridtrading" => Some(Self::Grid),
            "rsi" | "rsimeanreversion" => Some(Self::Rsi),
            "bollinger" | "bollingerbands" => Some(Self::Bollinger),
            "volatility" | "volatilitybreakout" => Some(Self::Volatility),
            "magic_split" | "magicsplit" => Some(Self::MagicSplit),
            "simple_power" | "simplepower" => Some(Self::SimplePower),
            "haa" => Some(Self::Haa),
            "xaa" => Some(Self::Xaa),
            "stock_rotation" | "stockrotation" => Some(Self::StockRotation),
            _ => None,
        }
    }
}

/// 백테스트 실행
pub async fn run_backtest(config: BacktestCliConfig) -> Result<BacktestReport> {
    info!(
        "Running backtest for {} {} with config: {}",
        match config.market {
            Market::KR => "KR",
            Market::US => "US",
        },
        config.symbol,
        config.config_path
    );

    // 1. 전략 설정 파일 로드
    let strategy_config = load_strategy_config(&config.config_path)?;
    info!("Loaded strategy config: {}", strategy_config.name);

    // 2. 데이터베이스 연결
    let db_url = config.db_url.clone().unwrap_or_else(|| {
        std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://trader:trader@localhost:5432/trader".to_string())
    });

    let db_config = DatabaseConfig {
        url: db_url,
        ..Default::default()
    };

    info!("Connecting to database...");
    let db = Database::connect(&db_config).await?;

    // 3. 심볼 조회
    let symbol_repo = SymbolRepository::new(db.clone());
    let kline_repo = KlineRepository::new(db.clone());

    let exchange = match config.market {
        Market::KR => "KIS_KR",
        Market::US => "KIS_US",
    };

    let symbol = create_symbol(&config);

    // 심볼 ID 조회 (get_or_create 사용)
    let symbol_id = symbol_repo.get_or_create(&symbol, exchange).await?;
    info!("Symbol ID: {}", symbol_id);

    // 4. 과거 데이터 로드
    let klines = load_klines_from_db(
        &kline_repo,
        symbol_id,
        &symbol,
        config.start_date,
        config.end_date,
    )
    .await?;

    if klines.is_empty() {
        return Err(anyhow!(
            "No historical data found for {}. Run import-db first.",
            config.symbol
        ));
    }

    info!("Loaded {} klines for backtest", klines.len());

    // 5. 전략 타입 파싱
    let strategy_type =
        StrategyType::from_str(&strategy_config.strategy_type).ok_or_else(|| {
            anyhow!(
                "Unknown strategy type: {}. Use --list-strategies to see available strategies.",
                strategy_config.strategy_type
            )
        })?;

    // 6. 백테스트 엔진 설정
    let backtest_config = BacktestConfig::new(config.initial_capital)
        .with_commission_rate(config.commission_rate)
        .with_slippage_rate(config.slippage_rate)
        .with_allow_short(false); // 주식은 기본적으로 숏 비허용

    // 7. 전략별 백테스트 실행
    let report = run_strategy_backtest(
        strategy_type,
        backtest_config,
        &klines,
        &strategy_config.parameters,
    )
    .await?;

    // 8. 결과 출력
    println!("\n{}", report.summary());

    // 9. 결과 저장 (옵션)
    if let Some(output_path) = &config.output_path {
        save_report(&report, output_path)?;
        info!("Report saved to: {}", output_path);
    }

    Ok(report)
}

/// 전략별 백테스트 실행 (제네릭 문제 해결을 위한 매크로 대신 개별 함수)
async fn run_strategy_backtest(
    strategy_type: StrategyType,
    backtest_config: BacktestConfig,
    klines: &[Kline],
    params: &serde_json::Value,
) -> Result<BacktestReport> {
    match strategy_type {
        StrategyType::Grid => {
            let mut strategy = GridStrategy::default();
            strategy
                .initialize(params.clone())
                .await
                .map_err(|e| anyhow!("Failed to initialize strategy: {}", e))?;
            let mut engine = BacktestEngine::new(backtest_config);
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| anyhow!("Backtest failed: {}", e))
        }
        StrategyType::Rsi => {
            let mut strategy = RsiStrategy::default();
            strategy
                .initialize(params.clone())
                .await
                .map_err(|e| anyhow!("Failed to initialize strategy: {}", e))?;
            let mut engine = BacktestEngine::new(backtest_config);
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| anyhow!("Backtest failed: {}", e))
        }
        StrategyType::Bollinger => {
            let mut strategy = BollingerStrategy::default();
            strategy
                .initialize(params.clone())
                .await
                .map_err(|e| anyhow!("Failed to initialize strategy: {}", e))?;
            let mut engine = BacktestEngine::new(backtest_config);
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| anyhow!("Backtest failed: {}", e))
        }
        StrategyType::Volatility => {
            let mut strategy = VolatilityBreakoutStrategy::default();
            strategy
                .initialize(params.clone())
                .await
                .map_err(|e| anyhow!("Failed to initialize strategy: {}", e))?;
            let mut engine = BacktestEngine::new(backtest_config);
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| anyhow!("Backtest failed: {}", e))
        }
        StrategyType::MagicSplit => {
            let mut strategy = MagicSplitStrategy::default();
            strategy
                .initialize(params.clone())
                .await
                .map_err(|e| anyhow!("Failed to initialize strategy: {}", e))?;
            let mut engine = BacktestEngine::new(backtest_config);
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| anyhow!("Backtest failed: {}", e))
        }
        StrategyType::SimplePower => {
            let mut strategy = SimplePowerStrategy::default();
            strategy
                .initialize(params.clone())
                .await
                .map_err(|e| anyhow!("Failed to initialize strategy: {}", e))?;
            let mut engine = BacktestEngine::new(backtest_config);
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| anyhow!("Backtest failed: {}", e))
        }
        StrategyType::Haa => {
            let mut strategy = HaaStrategy::default();
            strategy
                .initialize(params.clone())
                .await
                .map_err(|e| anyhow!("Failed to initialize strategy: {}", e))?;
            let mut engine = BacktestEngine::new(backtest_config);
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| anyhow!("Backtest failed: {}", e))
        }
        StrategyType::Xaa => {
            let mut strategy = XaaStrategy::default();
            strategy
                .initialize(params.clone())
                .await
                .map_err(|e| anyhow!("Failed to initialize strategy: {}", e))?;
            let mut engine = BacktestEngine::new(backtest_config);
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| anyhow!("Backtest failed: {}", e))
        }
        StrategyType::StockRotation => {
            let mut strategy = StockRotationStrategy::default();
            strategy
                .initialize(params.clone())
                .await
                .map_err(|e| anyhow!("Failed to initialize strategy: {}", e))?;
            let mut engine = BacktestEngine::new(backtest_config);
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| anyhow!("Backtest failed: {}", e))
        }
    }
}

/// 전략 설정 파일 로드
fn load_strategy_config(path: &str) -> Result<StrategyConfigFile> {
    let path = Path::new(path);

    if !path.exists() {
        return Err(anyhow!(
            "Strategy config file not found: {}",
            path.display()
        ));
    }

    let content = std::fs::read_to_string(path)?;

    if path.extension().map_or(false, |ext| ext == "toml") {
        Ok(toml::from_str(&content)?)
    } else if path.extension().map_or(false, |ext| ext == "json") {
        Ok(serde_json::from_str(&content)?)
    } else {
        Err(anyhow!(
            "Unsupported config format. Use .toml or .json: {}",
            path.display()
        ))
    }
}

/// 심볼 객체 생성
fn create_symbol(config: &BacktestCliConfig) -> Symbol {
    let market_type = MarketType::Stock;

    Symbol {
        base: config.symbol.to_uppercase(),
        quote: match config.market {
            Market::KR => "KRW".to_string(),
            Market::US => "USD".to_string(),
        },
        market_type,
        exchange_symbol: None,
    }
}

/// 데이터베이스에서 캔들 데이터 로드
async fn load_klines_from_db(
    kline_repo: &KlineRepository,
    symbol_id: uuid::Uuid,
    symbol: &Symbol,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
) -> Result<Vec<Kline>> {
    // 시작/종료 날짜가 없으면 기본값 사용 (최근 1년)
    let now = Utc::now();
    let start = start_date
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
        .unwrap_or_else(|| now - chrono::Duration::days(365));
    let end = end_date
        .map(|d| d.and_hms_opt(23, 59, 59).unwrap().and_utc())
        .unwrap_or(now);

    // get_range 메서드 사용 (limit: None = 기본 1000개)
    let rows = kline_repo
        .get_range(symbol_id, Timeframe::D1, start, end, None)
        .await?;

    debug!("Loaded {} rows from database", rows.len());

    // DB 행을 Kline으로 변환
    let klines: Vec<Kline> = rows
        .into_iter()
        .map(|row| row.to_kline(symbol.clone()))
        .collect();

    Ok(klines)
}

/// 백테스트 리포트를 파일로 저장
fn save_report(report: &BacktestReport, path: &str) -> Result<()> {
    let path = Path::new(path);

    // 디렉토리 생성
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = if path.extension().map_or(false, |ext| ext == "json") {
        serde_json::to_string_pretty(report)?
    } else {
        // 기본: 텍스트 요약
        report.summary()
    };

    std::fs::write(path, content)?;
    Ok(())
}

/// 사용 가능한 전략 목록 출력
pub fn print_available_strategies() {
    println!("\n📋 사용 가능한 전략 목록:");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("  전략 타입           | 설명");
    println!("  ─────────────────────────────────────────────────────────────");
    println!("  grid               | 그리드 트레이딩 (횡보장 적합)");
    println!("  rsi                | RSI 평균회귀 (과매수/과매도)");
    println!("  bollinger          | 볼린저 밴드 (동적 변동성)");
    println!("  volatility         | 변동성 돌파 (Larry Williams)");
    println!("  magic_split        | 매직 스플릿 (분할 매수)");
    println!("  simple_power       | 심플 파워 (TQQQ/SCHD 모멘텀)");
    println!("  haa                | HAA 계층적 자산배분 (카나리아)");
    println!("  xaa                | XAA 확장 자산배분");
    println!("  stock_rotation     | 종목 갈아타기 시스템");
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("예시 설정 파일 (config/backtest/rsi.toml):");
    println!("  name = \"RSI Strategy Backtest\"");
    println!("  strategy_type = \"rsi\"");
    println!("  ");
    println!("  [parameters]");
    println!("  period = 14");
    println!("  overbought = 70");
    println!("  oversold = 30");
}

// ==================== 테스트 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = BacktestCliConfig::default();
        assert_eq!(config.initial_capital, Decimal::from(10_000_000i64));
    }

    #[test]
    fn test_create_symbol_kr() {
        let config = BacktestCliConfig {
            market: Market::KR,
            symbol: "005930".to_string(),
            ..Default::default()
        };

        let symbol = create_symbol(&config);
        assert_eq!(symbol.base, "005930");
        assert_eq!(symbol.quote, "KRW");
    }

    #[test]
    fn test_create_symbol_us() {
        let config = BacktestCliConfig {
            market: Market::US,
            symbol: "spy".to_string(),
            ..Default::default()
        };

        let symbol = create_symbol(&config);
        assert_eq!(symbol.base, "SPY");
        assert_eq!(symbol.quote, "USD");
    }

    #[test]
    fn test_strategy_type_parsing() {
        assert!(matches!(
            StrategyType::from_str("grid"),
            Some(StrategyType::Grid)
        ));
        assert!(matches!(
            StrategyType::from_str("RSI"),
            Some(StrategyType::Rsi)
        ));
        assert!(matches!(
            StrategyType::from_str("simple_power"),
            Some(StrategyType::SimplePower)
        ));
        assert!(StrategyType::from_str("unknown").is_none());
    }
}
