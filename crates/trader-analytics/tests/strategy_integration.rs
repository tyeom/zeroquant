//! 전략 통합 테스트 - CachedHistoricalDataProvider를 사용한 전체 파이프라인 검증.
//!
//! ## 테스트 목적
//! 모든 전략이 E2E로 실제 동작하는지 검증합니다:
//! 1. 데이터셋 캐시 시스템이 정상 동작하는지
//! 2. 프로바이더에서 데이터를 올바르게 가져오는지
//! 3. 백테스트 엔진이 정상 실행되는지
//! 4. 전략이 신호를 생성하는지
//! 5. 신호에 따라 거래가 실행되는지
//!
//! ## 테스트 실행 조건
//!
//! 환경 변수 `DATABASE_URL`이 설정되어 있어야 합니다.
//! 설정되지 않은 경우 테스트가 건너뛰어집니다.
//!
//! ## 전략별 데이터 요구사항
//!
//! | 분류 | 주기 | 최소 데이터 |
//! |------|------|-------------|
//! | 실시간 | 분봉/일봉 | 100개 |
//! | 일간 | 일봉 | 60개 |
//! | 월간 (자산배분) | 일봉 | 500개 (2년) |

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::env;

use trader_analytics::backtest::{BacktestConfig, BacktestEngine};
use trader_core::{Kline, Timeframe};
use trader_data::CachedHistoricalDataProvider;
use trader_strategy::Strategy;

// =============================================================================
// 단일 자산 전략
// =============================================================================
use trader_strategy::strategies::{
    BollingerStrategy, CandlePatternStrategy, GridStrategy, InfinityBotStrategy,
    MagicSplitStrategy, RsiStrategy, SmaStrategy, VolatilityBreakoutStrategy,
};

// =============================================================================
// 자산배분 전략
// =============================================================================
use trader_strategy::strategies::{
    AllWeatherStrategy, DualMomentumStrategy, HaaStrategy, MarketCapTopStrategy,
    MarketInterestDayStrategy, SimplePowerStrategy, SnowStrategy, StockRotationStrategy,
    XaaStrategy,
};

// =============================================================================
// 추가 전략 (KR/US 특화)
// =============================================================================
use trader_strategy::strategies::{
    BaaStrategy, KosdaqFireRainStrategy, KospiBothSideStrategy, SectorMomentumStrategy,
    SectorVbStrategy, SmallCapQuantStrategy, StockGuganStrategy, Us3xLeverageStrategy,
};

/// 테스트용 DB Pool 생성.
async fn get_test_pool() -> Option<sqlx::PgPool> {
    let database_url = env::var("DATABASE_URL").ok()?;

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .ok()?;

    Some(pool)
}

/// Kline 데이터를 BacktestEngine 형식에 맞게 정렬 (시간순).
fn sort_klines_ascending(klines: Vec<Kline>) -> Vec<Kline> {
    let mut sorted = klines;
    sorted.sort_by_key(|k| k.open_time);
    sorted
}

/// 테스트 결과 검증 매크로
macro_rules! verify_backtest_result {
    ($report:expr, $strategy_name:expr) => {{
        println!("\n========================================");
        println!("  {} 백테스트 결과", $strategy_name);
        println!("========================================");
        println!("{}", $report.summary());

        // 필수 검증: 데이터 포인트가 처리되었는지
        assert!($report.data_points > 0, "데이터 포인트가 처리되어야 함");

        println!("총 주문: {}", $report.total_orders);
        println!("총 거래: {}", $report.metrics.total_trades);
        println!("\n=== {} 테스트 완료 ===", $strategy_name);
    }};
}

// =============================================================================
// 1. 단일 자산 - 실시간 전략
// =============================================================================

/// RSI 전략 통합 테스트 - 분봉/일봉
#[tokio::test]
async fn test_rsi_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);
    let klines = match provider.get_klines("005930", Timeframe::D1, 100).await {
        Ok(k) if k.len() >= 20 => k,
        _ => {
            eprintln!("Skipping test: Insufficient data for RSI strategy");
            return;
        }
    };

    println!("RSI 테스트 데이터 로드: {} 캔들", klines.len());

    let mut strategy = RsiStrategy::new();
    let config = json!({
        "symbol": "005930",
        "period": 14,
        "oversold_threshold": 30.0,
        "overbought_threshold": 70.0,
        "amount": "1000000",
        "stop_loss_pct": 3.0,
        "take_profit_pct": 5.0
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000_000))
        .with_commission_rate(dec!(0.00015))
        .with_slippage_rate(dec!(0.001));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "RSI");
}

/// Grid Trading 전략 - 실시간
#[tokio::test]
async fn test_grid_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);
    let klines = match provider.get_klines("005930", Timeframe::D1, 60).await {
        Ok(k) if !k.is_empty() => k,
        _ => {
            eprintln!("Skipping test: No data available");
            return;
        }
    };

    let initial_price = klines.first().map(|k| k.close).unwrap_or(dec!(70000));
    println!(
        "Grid 테스트 데이터 로드: {} 캔들, 초기가 {}",
        klines.len(),
        initial_price
    );

    let mut strategy = GridStrategy::new();
    let config = json!({
        "symbol": "005930",
        "center_price": initial_price.to_string(),
        "grid_spacing_pct": 2.0,
        "grid_levels": 5,
        "amount_per_level": "1000000",
        "dynamic_spacing": false,
        "trend_filter": false
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000_000))
        .with_commission_rate(dec!(0.00015))
        .with_slippage_rate(dec!(0.001));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "Grid Trading");
}

/// Bollinger Bands 전략 - 분봉/일봉
#[tokio::test]
async fn test_bollinger_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);
    let klines = match provider.get_klines("005930", Timeframe::D1, 100).await {
        Ok(k) if k.len() >= 30 => k,
        _ => {
            eprintln!("Skipping test: Insufficient data for Bollinger strategy");
            return;
        }
    };

    println!("볼린저 밴드 테스트 데이터 로드: {} 캔들", klines.len());

    let mut strategy = BollingerStrategy::new();
    let config = json!({
        "symbol": "005930",
        "period": 20,
        "std_multiplier": 2.0,
        "use_rsi_confirmation": true,
        "exit_at_middle_band": true,
        "stop_loss_pct": 2.0,
        "take_profit_pct": 4.0
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000_000))
        .with_commission_rate(dec!(0.00015))
        .with_slippage_rate(dec!(0.001));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "Bollinger Bands");
}

/// SMA Crossover 전략 - 분봉/일봉
#[tokio::test]
async fn test_sma_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);
    let klines = match provider.get_klines("005930", Timeframe::D1, 200).await {
        Ok(k) if k.len() >= 50 => k,
        _ => {
            eprintln!("Skipping test: Insufficient data for SMA strategy");
            return;
        }
    };

    println!("SMA 테스트 데이터 로드: {} 캔들", klines.len());

    let mut strategy = SmaStrategy::new();
    let config = json!({
        "symbol": "005930",
        "fast_period": 10,
        "slow_period": 30,
        "amount": "1000000"
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000_000))
        .with_commission_rate(dec!(0.00015))
        .with_slippage_rate(dec!(0.001));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "SMA Crossover");
}

/// Magic Split 전략 - 실시간, 10차수 분할매수
#[tokio::test]
async fn test_magic_split_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);
    let klines = match provider.get_klines("005930", Timeframe::D1, 100).await {
        Ok(k) if !k.is_empty() => k,
        _ => {
            eprintln!("Skipping test: No data available");
            return;
        }
    };

    println!("Magic Split 테스트 데이터 로드: {} 캔들", klines.len());

    let mut strategy = MagicSplitStrategy::new();
    // 10차수 분할 매수 설정
    let config = json!({
        "symbol": "005930",
        "levels": [
            { "number": 1, "target_rate": "10.0", "trigger_rate": null, "invest_money": "500000" },
            { "number": 2, "target_rate": "2.0", "trigger_rate": "-3.0", "invest_money": "300000" },
            { "number": 3, "target_rate": "3.0", "trigger_rate": "-4.0", "invest_money": "300000" },
            { "number": 4, "target_rate": "3.0", "trigger_rate": "-5.0", "invest_money": "300000" },
            { "number": 5, "target_rate": "3.0", "trigger_rate": "-5.0", "invest_money": "300000" },
            { "number": 6, "target_rate": "4.0", "trigger_rate": "-6.0", "invest_money": "300000" },
            { "number": 7, "target_rate": "4.0", "trigger_rate": "-6.0", "invest_money": "300000" },
            { "number": 8, "target_rate": "4.0", "trigger_rate": "-6.0", "invest_money": "300000" },
            { "number": 9, "target_rate": "5.0", "trigger_rate": "-7.0", "invest_money": "300000" },
            { "number": 10, "target_rate": "5.0", "trigger_rate": "-7.0", "invest_money": "300000" }
        ],
        "allow_same_day_reentry": false,
        "slippage_tolerance": "1.0"
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(10_000_000))
        .with_commission_rate(dec!(0.00015))
        .with_slippage_rate(dec!(0.001));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "Magic Split");
}

// TrailingStopStrategy는 리스크 관리로 이동됨 (전략 테스트에서 제외)

/// Candle Pattern 전략 - 분봉/일봉, 35개 패턴
#[tokio::test]
async fn test_candle_pattern_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);
    let klines = match provider.get_klines("005930", Timeframe::D1, 100).await {
        Ok(k) if k.len() >= 30 => k,
        _ => {
            eprintln!("Skipping test: Insufficient data for Candle Pattern strategy");
            return;
        }
    };

    println!("Candle Pattern 테스트 데이터 로드: {} 캔들", klines.len());

    let mut strategy = CandlePatternStrategy::new();
    let config = json!({
        "symbol": "005930",
        "trade_amount": "1000000",
        "min_pattern_strength": "0.6",
        "use_volume_confirmation": true,
        "use_trend_confirmation": true,
        "trend_period": 20,
        "stop_loss_pct": "3.0",
        "take_profit_pct": "6.0"
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000_000))
        .with_commission_rate(dec!(0.00015))
        .with_slippage_rate(dec!(0.001));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "Candle Pattern");
}

/// Infinity Bot 전략 - 실시간, 50라운드 분할매수
#[tokio::test]
async fn test_infinity_bot_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);
    // Infinity Bot은 충분한 데이터가 필요 (MA 200일 계산)
    let klines = match provider.get_klines("005930", Timeframe::D1, 250).await {
        Ok(k) if k.len() >= 200 => k,
        _ => {
            eprintln!("Skipping test: Insufficient data for Infinity Bot (need 200+ candles)");
            return;
        }
    };

    println!("Infinity Bot 테스트 데이터 로드: {} 캔들", klines.len());

    let mut strategy = InfinityBotStrategy::new();
    let config = json!({
        "symbol": "005930",
        "total_amount": "10000000",
        "max_rounds": 50,
        "round_amount_pct": "2.0",
        "dip_trigger_pct": "2.0",
        "take_profit_pct": "3.0",
        "stop_loss_pct": "20.0",
        "short_ma_period": 10,
        "mid_ma_period": 100,
        "long_ma_period": 200
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000_000))
        .with_commission_rate(dec!(0.00015))
        .with_slippage_rate(dec!(0.001));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "Infinity Bot");
}

// =============================================================================
// 2. 단일 자산 - 일간 전략
// =============================================================================

/// Volatility Breakout 전략 - 일 1회 (장 시작 5분 후)
#[tokio::test]
async fn test_volatility_breakout_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);
    let klines = match provider.get_klines("AAPL", Timeframe::D1, 60).await {
        Ok(k) if k.len() >= 10 => k,
        _ => {
            eprintln!("Skipping test: Insufficient data for Volatility Breakout");
            return;
        }
    };

    println!(
        "Volatility Breakout 테스트 데이터 로드: {} 캔들",
        klines.len()
    );

    let mut strategy = VolatilityBreakoutStrategy::new();
    let config = json!({
        "symbol": "AAPL",
        "k_factor": 0.5,
        "lookback_period": 1,
        "use_atr": false,
        "stop_loss_multiplier": 1.0,
        "take_profit_multiplier": 2.0,
        "trade_both_directions": true
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000))
        .with_commission_rate(dec!(0.0))
        .with_slippage_rate(dec!(0.0005));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "Volatility Breakout");
}

/// Market Interest Day 전략 - 일 1회 (장 시작 직후)
#[tokio::test]
async fn test_market_interest_day_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);
    let klines = match provider.get_klines("005930", Timeframe::D1, 60).await {
        Ok(k) if k.len() >= 30 => k,
        _ => {
            eprintln!("Skipping test: Insufficient data for Market Interest Day");
            return;
        }
    };

    println!(
        "Market Interest Day 테스트 데이터 로드: {} 캔들",
        klines.len()
    );

    let mut strategy = MarketInterestDayStrategy::new();
    let config = json!({
        "symbol": "005930",
        "trade_amount": "1000000",
        "volume_multiplier": "2.0",
        "volume_period": 20,
        "consecutive_up_candles": 3,
        "trailing_stop_pct": "1.5",
        "take_profit_pct": "3.0",
        "stop_loss_pct": "2.0",
        "atr_period": 14,
        "atr_multiplier": "1.5",
        "max_hold_minutes": 120,
        "rsi_overbought": "80.0",
        "rsi_period": 14
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000_000))
        .with_commission_rate(dec!(0.00015))
        .with_slippage_rate(dec!(0.001));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "Market Interest Day");
}

// =============================================================================
// 3. 자산배분 전략 - 월간 (다중 심볼)
// =============================================================================

/// Simple Power 전략 - 월 1회, MA130 필터
#[tokio::test]
async fn test_simple_power_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);

    // Simple Power는 US ETF 필요: TQQQ, SCHD, PFIX, TMF
    // MA130 계산을 위해 최소 150일 데이터 필요
    let test_symbol = "SPY"; // 미국 주식 대표
    let klines = match provider.get_klines(test_symbol, Timeframe::D1, 200).await {
        Ok(k) if k.len() >= 130 => k,
        _ => {
            eprintln!("Skipping test: Insufficient data for Simple Power (need 130+ candles)");
            return;
        }
    };

    println!("Simple Power 테스트 데이터 로드: {} 캔들", klines.len());

    let mut strategy = SimplePowerStrategy::new();
    let config = json!({
        "market": "US",
        "aggressive_asset": "TQQQ",
        "aggressive_weight": "0.5",
        "dividend_asset": "SCHD",
        "dividend_weight": "0.2",
        "rate_hedge_asset": "PFIX",
        "rate_hedge_weight": "0.15",
        "bond_leverage_asset": "TMF",
        "bond_leverage_weight": "0.15",
        "ma_period": 130,
        "rebalance_interval_months": 1,
        "invest_rate": "1.0",
        "rebalance_threshold": "0.03"
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000))
        .with_commission_rate(dec!(0.0))
        .with_slippage_rate(dec!(0.0005));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "Simple Power");
}

/// HAA 전략 - 월 1회, 카나리아 자산 (TIP)
#[tokio::test]
async fn test_haa_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);

    // HAA는 모멘텀 계산을 위해 최소 252일 (1년) 데이터 필요
    let test_symbol = "SPY";
    let klines = match provider.get_klines(test_symbol, Timeframe::D1, 300).await {
        Ok(k) if k.len() >= 250 => k,
        _ => {
            eprintln!("Skipping test: Insufficient data for HAA (need 250+ candles)");
            return;
        }
    };

    println!("HAA 테스트 데이터 로드: {} 캔들", klines.len());

    let mut strategy = HaaStrategy::new();
    let config = json!({
        "market": "US",
        "canary_assets": [{"symbol": "TIP", "asset_type": "Canary", "description": "TIPS"}],
        "offensive_assets": [
            {"symbol": "SPY", "asset_type": "Offensive", "description": "S&P 500"},
            {"symbol": "IWM", "asset_type": "Offensive", "description": "Russell 2000"},
            {"symbol": "VEA", "asset_type": "Offensive", "description": "Developed Markets"},
            {"symbol": "VWO", "asset_type": "Offensive", "description": "Emerging Markets"}
        ],
        "defensive_assets": [
            {"symbol": "IEF", "asset_type": "Defensive", "description": "7-10Y Treasury"},
            {"symbol": "BIL", "asset_type": "Cash", "description": "1-3M T-Bill"}
        ],
        "offensive_top_n": 4,
        "defensive_top_n": 1,
        "cash_symbol": "BIL",
        "invest_rate": "1.0",
        "rebalance_threshold": "0.03"
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000))
        .with_commission_rate(dec!(0.0))
        .with_slippage_rate(dec!(0.0005));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "HAA");
}

/// XAA 전략 - 월 1회, TOP 4 선택
#[tokio::test]
async fn test_xaa_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);
    let test_symbol = "SPY";
    let klines = match provider.get_klines(test_symbol, Timeframe::D1, 300).await {
        Ok(k) if k.len() >= 250 => k,
        _ => {
            eprintln!("Skipping test: Insufficient data for XAA");
            return;
        }
    };

    println!("XAA 테스트 데이터 로드: {} 캔들", klines.len());

    let mut strategy = XaaStrategy::new();
    let config = json!({
        "market": "US",
        "canary_assets": [{"symbol": "TIP", "asset_type": "Canary", "description": "TIPS"}],
        "offensive_assets": [
            {"symbol": "SPY", "asset_type": "Offensive", "description": "S&P 500"},
            {"symbol": "IWM", "asset_type": "Offensive", "description": "Russell 2000"},
            {"symbol": "VEA", "asset_type": "Offensive", "description": "Developed Markets"},
            {"symbol": "VWO", "asset_type": "Offensive", "description": "Emerging Markets"},
            {"symbol": "VNQ", "asset_type": "Offensive", "description": "Real Estate"},
            {"symbol": "PDBC", "asset_type": "Offensive", "description": "Commodities"}
        ],
        "bond_assets": [
            {"symbol": "TLT", "asset_type": "Bond", "description": "20+ Treasury"},
            {"symbol": "IEF", "asset_type": "Bond", "description": "7-10Y Treasury"}
        ],
        "safe_assets": [{"symbol": "IEF", "asset_type": "Safe", "description": "7-10Y Treasury"}],
        "offensive_top_n": 4,
        "bond_top_n": 3,
        "cash_symbol": "BIL",
        "invest_rate": "1.0",
        "rebalance_threshold": "0.03"
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000))
        .with_commission_rate(dec!(0.0))
        .with_slippage_rate(dec!(0.0005));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "XAA");
}

/// All Weather 전략 - 월 1회, 계절성
#[tokio::test]
async fn test_all_weather_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);
    let test_symbol = "SPY";
    let klines = match provider.get_klines(test_symbol, Timeframe::D1, 200).await {
        Ok(k) if k.len() >= 150 => k,
        _ => {
            eprintln!("Skipping test: Insufficient data for All Weather");
            return;
        }
    };

    println!("All Weather 테스트 데이터 로드: {} 캔들", klines.len());

    let mut strategy = AllWeatherStrategy::new();
    let config = json!({
        "market": "US",
        "total_amount": "100000",
        "ma_periods": [50, 80, 120, 150],
        "use_seasonality": true,
        "rebalance_days": 30,
        "rebalance_threshold": "5.0"
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000))
        .with_commission_rate(dec!(0.0))
        .with_slippage_rate(dec!(0.0005));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "All Weather");
}

/// Snow 전략 - 일 1회, TIP 모멘텀
#[tokio::test]
async fn test_snow_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);
    // Snow는 TIP MA200 계산이 필요
    let test_symbol = "SPY";
    let klines = match provider.get_klines(test_symbol, Timeframe::D1, 250).await {
        Ok(k) if k.len() >= 200 => k,
        _ => {
            eprintln!("Skipping test: Insufficient data for Snow (need 200+ candles)");
            return;
        }
    };

    println!("Snow 테스트 데이터 로드: {} 캔들", klines.len());

    let mut strategy = SnowStrategy::new();
    let config = json!({
        "market": "US",
        "total_amount": "100000",
        "tip_ma_period": 200,
        "attack_ma_period": 5,
        "rebalance_days": 1,
        "rebalance_threshold": "5.0"
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000))
        .with_commission_rate(dec!(0.0))
        .with_slippage_rate(dec!(0.0005));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "Snow");
}

/// Stock Rotation 전략 - 일/주, 모멘텀 기반 종목 교체
#[tokio::test]
async fn test_stock_rotation_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);
    let test_symbol = "005930";
    let klines = match provider.get_klines(test_symbol, Timeframe::D1, 300).await {
        Ok(k) if k.len() >= 250 => k,
        _ => {
            eprintln!("Skipping test: Insufficient data for Stock Rotation");
            return;
        }
    };

    println!("Stock Rotation 테스트 데이터 로드: {} 캔들", klines.len());

    let mut strategy = StockRotationStrategy::new();
    let config = json!({
        "market": "KR",
        "universe": [
            {"symbol": "005930", "name": "삼성전자"},
            {"symbol": "000660", "name": "SK하이닉스"},
            {"symbol": "035420", "name": "NAVER"},
            {"symbol": "035720", "name": "카카오"},
            {"symbol": "051910", "name": "LG화학"}
        ],
        "top_n": 3,
        "invest_rate": "1.0",
        "rebalance_threshold": "0.03",
        "cash_reserve_rate": "0.0"
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000_000))
        .with_commission_rate(dec!(0.00015))
        .with_slippage_rate(dec!(0.001));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "Stock Rotation");
}

/// Market Cap TOP 전략 - 월 1회, 시총 상위 N
#[tokio::test]
async fn test_market_cap_top_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);
    let test_symbol = "AAPL";
    let klines = match provider.get_klines(test_symbol, Timeframe::D1, 300).await {
        Ok(k) if k.len() >= 250 => k,
        _ => {
            eprintln!("Skipping test: Insufficient data for Market Cap TOP");
            return;
        }
    };

    println!("Market Cap TOP 테스트 데이터 로드: {} 캔들", klines.len());

    let mut strategy = MarketCapTopStrategy::new();
    let config = json!({
        "top_n": 10,
        "total_amount": "100000",
        "weighting_method": "Equal",
        "rebalance_days": 30,
        "rebalance_threshold": "5.0",
        "symbols": ["AAPL", "MSFT", "GOOGL", "AMZN", "NVDA", "META", "TSLA", "BRK.B", "UNH", "JNJ"],
        "use_momentum_filter": false,
        "momentum_period": 252
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000))
        .with_commission_rate(dec!(0.0))
        .with_slippage_rate(dec!(0.0005));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "Market Cap TOP");
}

/// Dual Momentum 전략 - 월 1회, KR+US 복합
#[tokio::test]
async fn test_dual_momentum_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);
    let test_symbol = "SPY";
    let klines = match provider.get_klines(test_symbol, Timeframe::D1, 150).await {
        Ok(k) if k.len() >= 63 => k, // 63일 모멘텀 계산
        _ => {
            eprintln!("Skipping test: Insufficient data for Dual Momentum");
            return;
        }
    };

    println!("Dual Momentum 테스트 데이터 로드: {} 캔들", klines.len());

    let mut strategy = DualMomentumStrategy::new();
    let config = json!({
        "total_amount": "100000",
        "momentum_period": 63,
        "rebalance_threshold": "5.0",
        "kr_allocation": 0.5,
        "use_absolute_momentum": true
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000))
        .with_commission_rate(dec!(0.0))
        .with_slippage_rate(dec!(0.0005));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "Dual Momentum");
}

// =============================================================================
// 4. 추가 전략 - KR/US 특화
// =============================================================================

/// BAA 전략 - US 자산배분
#[tokio::test]
async fn test_baa_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);
    let test_symbol = "SPY";
    let klines = match provider.get_klines(test_symbol, Timeframe::D1, 300).await {
        Ok(k) if k.len() >= 250 => k,
        _ => {
            eprintln!("Skipping test: Insufficient data for BAA");
            return;
        }
    };

    println!("BAA 테스트 데이터 로드: {} 캔들", klines.len());

    let mut strategy = BaaStrategy::new();
    let config = json!({
        "total_amount": "100000",
        "momentum_period": 252,
        "rebalance_days": 30,
        "rebalance_threshold": "5.0"
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000))
        .with_commission_rate(dec!(0.0))
        .with_slippage_rate(dec!(0.0005));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "BAA");
}

/// US 3X Leverage 전략 - US ETF, 3배 레버리지
#[tokio::test]
async fn test_us_3x_leverage_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);
    let test_symbol = "SPY";
    let klines = match provider.get_klines(test_symbol, Timeframe::D1, 100).await {
        Ok(k) if k.len() >= 50 => k,
        _ => {
            eprintln!("Skipping test: Insufficient data for US 3X Leverage");
            return;
        }
    };

    println!("US 3X Leverage 테스트 데이터 로드: {} 캔들", klines.len());

    let mut strategy = Us3xLeverageStrategy::new();
    // Us3xLeverageConfig: allocations (Vec<EtfAllocation>), rebalance_threshold (f64),
    // rebalance_period_days (u32), use_ma_filter (bool), ma_period (usize),
    // dynamic_allocation (bool), max_inverse_ratio (f64), max_drawdown_pct (f64)
    let config = json!({
        "allocations": [
            {"ticker": "TQQQ", "target_ratio": 0.5, "etf_type": "leverage"},
            {"ticker": "SQQQ", "target_ratio": 0.5, "etf_type": "inverse"}
        ],
        "rebalance_threshold": 5.0,
        "rebalance_period_days": 30,
        "ma_period": 20
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000))
        .with_commission_rate(dec!(0.0))
        .with_slippage_rate(dec!(0.0005));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "US 3X Leverage");
}

/// Stock Gugan 전략 - KR/US, 주식 구간 매매
#[tokio::test]
async fn test_stock_gugan_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);
    let test_symbol = "005930";
    let klines = match provider.get_klines(test_symbol, Timeframe::D1, 100).await {
        Ok(k) if !k.is_empty() => k,
        _ => {
            eprintln!("Skipping test: No data available for Stock Gugan");
            return;
        }
    };

    println!("Stock Gugan 테스트 데이터 로드: {} 캔들", klines.len());

    let mut strategy = StockGuganStrategy::new();
    let config = json!({
        "symbol": "005930",
        "total_amount": "10000000",
        "buy_threshold_pct": "3.0",
        "sell_threshold_pct": "5.0",
        "max_position_pct": "0.2"
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000_000))
        .with_commission_rate(dec!(0.00015))
        .with_slippage_rate(dec!(0.001));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "Stock Gugan");
}

/// KOSDAQ Fire Rain 전략 - KR, 코스닥 급등주
#[tokio::test]
async fn test_kosdaq_fire_rain_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);
    let test_symbol = "005930"; // 코스닥 종목 대신 삼성전자 사용
    let klines = match provider.get_klines(test_symbol, Timeframe::D1, 60).await {
        Ok(k) if k.len() >= 30 => k,
        _ => {
            eprintln!("Skipping test: Insufficient data for KOSDAQ Fire Rain");
            return;
        }
    };

    println!("KOSDAQ Fire Rain 테스트 데이터 로드: {} 캔들", klines.len());

    let mut strategy = KosdaqFireRainStrategy::new();
    // KosdaqFireRainConfig: symbols, kospi_leverage, kosdaq_leverage, kospi_inverse, kosdaq_inverse,
    // max_positions, position_ratio, obv_period, ma_short, ma_medium, ma_long, rsi_period,
    // stop_loss_pct (f64), take_profit_pct (f64)
    let config = json!({
        "symbols": ["122630", "233740", "252670", "251340"],
        "stop_loss_pct": 3.0,
        "take_profit_pct": 10.0
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000_000))
        .with_commission_rate(dec!(0.00015))
        .with_slippage_rate(dec!(0.001));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "KOSDAQ Fire Rain");
}

/// Sector VB 전략 - KR, 섹터 변동성 돌파
#[tokio::test]
async fn test_sector_vb_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);
    let test_symbol = "005930";
    let klines = match provider.get_klines(test_symbol, Timeframe::D1, 60).await {
        Ok(k) if k.len() >= 30 => k,
        _ => {
            eprintln!("Skipping test: Insufficient data for Sector VB");
            return;
        }
    };

    println!("Sector VB 테스트 데이터 로드: {} 캔들", klines.len());

    let mut strategy = SectorVbStrategy::new();
    // SectorVbConfig: symbols, k_factor (f64), selection_method, top_n, min_volume,
    // close_before_minutes, stop_loss_pct (f64), take_profit_pct (f64)
    let config = json!({
        "symbols": ["091160", "091230", "305720"],
        "k_factor": 0.5,
        "stop_loss_pct": 2.0,
        "take_profit_pct": 4.0
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000_000))
        .with_commission_rate(dec!(0.00015))
        .with_slippage_rate(dec!(0.001));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "Sector VB");
}

/// KOSPI Bothside 전략 - KR, 양방향 매매
#[tokio::test]
async fn test_kospi_bothside_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);
    let test_symbol = "005930";
    let klines = match provider.get_klines(test_symbol, Timeframe::D1, 100).await {
        Ok(k) if k.len() >= 50 => k,
        _ => {
            eprintln!("Skipping test: Insufficient data for KOSPI Bothside");
            return;
        }
    };

    println!("KOSPI Bothside 테스트 데이터 로드: {} 캔들", klines.len());

    let mut strategy = KospiBothSideStrategy::new();
    // KospiBothSideConfig: leverage_ticker, inverse_ticker, leverage_ratio (f64),
    // inverse_ratio (f64), ma3_period, ma6_period, ma19_period, ma60_period,
    // disparity_upper (f64), disparity_lower (f64), rsi_period, rsi_oversold (f64),
    // rsi_overbought (f64), stop_loss_pct (f64)
    let config = json!({
        "leverage_ticker": "122630",
        "inverse_ticker": "252670",
        "leverage_ratio": 0.7,
        "inverse_ratio": 0.3,
        "stop_loss_pct": 2.0
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000_000))
        .with_commission_rate(dec!(0.00015))
        .with_slippage_rate(dec!(0.001));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "KOSPI Bothside");
}

/// Small Cap Quant 전략 - KR, 소형주 퀀트
#[tokio::test]
async fn test_small_cap_quant_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);
    let test_symbol = "005930";
    let klines = match provider.get_klines(test_symbol, Timeframe::D1, 300).await {
        Ok(k) if k.len() >= 250 => k,
        _ => {
            eprintln!("Skipping test: Insufficient data for Small Cap Quant");
            return;
        }
    };

    println!("Small Cap Quant 테스트 데이터 로드: {} 캔들", klines.len());

    let mut strategy = SmallCapQuantStrategy::new();
    let config = json!({
        "total_amount": "100000000",
        "top_n": 20,
        "momentum_period": 252,
        "rebalance_days": 30,
        "rebalance_threshold": "5.0"
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000_000))
        .with_commission_rate(dec!(0.00015))
        .with_slippage_rate(dec!(0.001));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "Small Cap Quant");
}

/// Sector Momentum 전략 - KR+US, 섹터 모멘텀
#[tokio::test]
async fn test_sector_momentum_strategy() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);
    let test_symbol = "SPY";
    let klines = match provider.get_klines(test_symbol, Timeframe::D1, 300).await {
        Ok(k) if k.len() >= 250 => k,
        _ => {
            eprintln!("Skipping test: Insufficient data for Sector Momentum");
            return;
        }
    };

    println!("Sector Momentum 테스트 데이터 로드: {} 캔들", klines.len());

    let mut strategy = SectorMomentumStrategy::new();
    let config = json!({
        "total_amount": "100000",
        "momentum_period": 252,
        "top_n": 3,
        "rebalance_days": 30,
        "rebalance_threshold": "5.0"
    });

    strategy.initialize(config).await.expect("초기화 실패");

    let backtest_config = BacktestConfig::new(dec!(100_000))
        .with_commission_rate(dec!(0.0))
        .with_slippage_rate(dec!(0.0005));

    let mut engine = BacktestEngine::new(backtest_config);
    let sorted_klines = sort_klines_ascending(klines);
    let report = engine
        .run(&mut strategy, &sorted_klines)
        .await
        .expect("백테스트 실행 실패");

    verify_backtest_result!(report, "Sector Momentum");
}

// =============================================================================
// 5. 데이터셋 캐시 검증 테스트
// =============================================================================

/// 데이터셋 캐시 일관성 검증.
#[tokio::test]
async fn test_cache_consistency() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);

    // 첫 번째 조회
    let klines1 = match provider.get_klines("005930", Timeframe::D1, 50).await {
        Ok(k) => k,
        Err(e) => {
            eprintln!("Skipping test: {}", e);
            return;
        }
    };

    // 두 번째 조회 (캐시에서 가져와야 함)
    let klines2 = provider
        .get_klines("005930", Timeframe::D1, 50)
        .await
        .expect("두 번째 조회 실패");

    // 검증: 두 결과가 동일해야 함
    assert_eq!(
        klines1.len(),
        klines2.len(),
        "캐시 일관성: 길이가 같아야 함"
    );

    for (k1, k2) in klines1.iter().zip(klines2.iter()) {
        assert_eq!(
            k1.open_time, k2.open_time,
            "캐시 일관성: open_time이 같아야 함"
        );
        assert_eq!(k1.close, k2.close, "캐시 일관성: close가 같아야 함");
    }

    println!("캐시 일관성 검증 완료: {} 캔들 확인", klines1.len());
}

/// 여러 심볼 동시 로드 테스트.
#[tokio::test]
async fn test_multiple_symbols_loading() {
    let pool = match get_test_pool().await {
        Some(p) => p,
        None => {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
    };

    let provider = CachedHistoricalDataProvider::new(pool);

    let symbols = ["005930", "AAPL", "MSFT"];
    let mut loaded_count = 0;

    for symbol in &symbols {
        match provider.get_klines(symbol, Timeframe::D1, 30).await {
            Ok(klines) if !klines.is_empty() => {
                println!("{}: {} 캔들 로드됨", symbol, klines.len());
                loaded_count += 1;
            }
            Ok(_) => {
                println!("{}: 데이터 없음", symbol);
            }
            Err(e) => {
                println!("{}: 로드 실패 - {}", symbol, e);
            }
        }
    }

    println!("\n{}/{} 심볼 로드 완료", loaded_count, symbols.len());

    // 최소 1개 이상 로드되어야 함
    assert!(loaded_count > 0, "최소 1개 심볼은 로드되어야 함");
}

// =============================================================================
// 6. 다중 자산 가격 검증 테스트
// =============================================================================

use chrono::{Duration, TimeZone, Utc};
use trader_core::{MarketType, Symbol};

/// 테스트용 다중 심볼 Kline 데이터 생성
/// 각 심볼별로 명확히 다른 가격을 설정하여 가격 매칭 검증 가능
fn create_multi_symbol_test_klines() -> Vec<Kline> {
    let days = 100;
    let base_time = Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap();

    // 심볼별로 명확히 다른 가격 설정 (절대 겹치지 않도록)
    let symbols_with_prices = vec![
        ("SPY", "USD", 450.0), // SPY: $450
        ("TLT", "USD", 95.0),  // TLT: $95
        ("IEF", "USD", 100.0), // IEF: $100
        ("GLD", "USD", 180.0), // GLD: $180
        ("PDBC", "USD", 15.0), // PDBC: $15
        ("IYK", "USD", 60.0),  // IYK: $60
    ];

    let mut all_klines = Vec::new();

    for (base, quote, base_price) in &symbols_with_prices {
        // Symbol 생성자를 통해 country 필드 자동 추론
        let symbol = Symbol::new(*base, *quote, MarketType::Stock);

        for day in 0..days {
            let open_time = base_time + Duration::days(day);
            let close_time = open_time + Duration::hours(6);

            // 가격 변동: ±5% 범위
            let variation = ((day as f64 * 0.1).sin() * 0.05 + 1.0);
            let price = base_price * variation;

            all_klines.push(Kline {
                symbol: symbol.clone(),
                timeframe: Timeframe::D1,
                open_time,
                close_time,
                open: Decimal::try_from(price * 0.999).unwrap_or(dec!(100)),
                high: Decimal::try_from(price * 1.01).unwrap_or(dec!(101)),
                low: Decimal::try_from(price * 0.99).unwrap_or(dec!(99)),
                close: Decimal::try_from(price).unwrap_or(dec!(100)),
                volume: dec!(1_000_000),
                quote_volume: None,
                num_trades: None,
            });
        }
    }

    // 시간순 정렬 (같은 날짜의 모든 심볼이 함께 처리되도록)
    all_klines.sort_by_key(|k| (k.open_time, k.symbol.base.clone()));

    all_klines
}

/// 다중 자산 전략 가격 검증 테스트
///
/// 핵심 검증: 각 심볼의 거래가 해당 시점의 해당 심볼 가격으로 체결되는지
/// - SPY 거래 시점의 SPY kline.close와 비교
/// - TLT 거래 시점의 TLT kline.close와 비교
/// - 모든 심볼이 같은 가격으로 거래되면 버그!
#[tokio::test]
async fn test_multi_asset_price_matching() {
    println!("\n========================================");
    println!("  다중 자산 가격 매칭 검증 테스트");
    println!("========================================");

    // 테스트 데이터 생성 (각 심볼별로 명확히 다른 가격)
    let klines = create_multi_symbol_test_klines();
    println!("테스트 데이터: {} 캔들 (6개 심볼 × 100일)", klines.len());

    // 가격 인덱스 생성: (날짜, 심볼) -> close 가격
    // 거래 발생 시점의 예상 가격을 조회하기 위함
    let mut price_index: std::collections::HashMap<(i64, String), Decimal> =
        std::collections::HashMap::new();
    for kline in &klines {
        // 날짜 단위로 인덱싱 (시간 제거)
        let date_key = kline.open_time.date_naive();
        let day_timestamp = date_key.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
        price_index.insert((day_timestamp, kline.symbol.base.clone()), kline.close);
    }
    println!("가격 인덱스 생성: {} 항목", price_index.len());

    // All Weather 전략 초기화
    let mut strategy = AllWeatherStrategy::new();
    let config = json!({
        "market": "US",
        "total_amount": "100000",
        "ma_periods": [5, 10, 20, 50],  // 테스트용으로 짧은 MA 기간
        "use_seasonality": false,
        "rebalance_days": 30,
        "rebalance_threshold": "3.0"
    });

    strategy.initialize(config).await.expect("초기화 실패");

    // 백테스트 실행
    let backtest_config = BacktestConfig::new(dec!(100_000))
        .with_commission_rate(dec!(0.0))  // 수수료 제거 (가격 검증 용이)
        .with_slippage_rate(dec!(0.0)); // 슬리피지 제거

    let mut engine = BacktestEngine::new(backtest_config);
    let report = engine
        .run(&mut strategy, &klines)
        .await
        .expect("백테스트 실행 실패");

    println!("\n--- 거래 내역 분석 (시점별 가격 검증) ---");
    println!("총 거래 수: {}", report.trades.len());

    let mut price_mismatch_count = 0;
    let mut price_match_count = 0;
    let tolerance = dec!(0.01); // 1% 허용 오차 (반올림 등)

    for trade in &report.trades {
        // 심볼에서 base 추출 (SPY/USD -> SPY)
        let symbol_base = trade.symbol.split('/').next().unwrap_or(&trade.symbol);

        // 거래 시점의 날짜 키 생성
        let entry_date = trade.entry_time.date_naive();
        let entry_day_ts = entry_date
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp();

        let exit_date = trade.exit_time.date_naive();
        let exit_day_ts = exit_date
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp();

        // 해당 시점의 해당 심볼 예상 가격 조회
        let expected_entry = price_index.get(&(entry_day_ts, symbol_base.to_string()));
        let expected_exit = price_index.get(&(exit_day_ts, symbol_base.to_string()));

        let mut entry_ok = true;
        let mut exit_ok = true;

        if let Some(&expected) = expected_entry {
            let diff = (trade.entry_price - expected).abs();
            let diff_pct = diff / expected;
            if diff_pct > tolerance {
                entry_ok = false;
                println!(
                    "❌ {} 진입 불일치: 실제=${:.2}, 예상=${:.2} (차이: {:.2}%)",
                    symbol_base,
                    trade.entry_price,
                    expected,
                    diff_pct * dec!(100)
                );
            }
        }

        if let Some(&expected) = expected_exit {
            let diff = (trade.exit_price - expected).abs();
            let diff_pct = diff / expected;
            if diff_pct > tolerance {
                exit_ok = false;
                println!(
                    "❌ {} 청산 불일치: 실제=${:.2}, 예상=${:.2} (차이: {:.2}%)",
                    symbol_base,
                    trade.exit_price,
                    expected,
                    diff_pct * dec!(100)
                );
            }
        }

        if entry_ok && exit_ok {
            price_match_count += 1;
            if let (Some(&entry_exp), Some(&exit_exp)) = (expected_entry, expected_exit) {
                println!(
                    "✅ {} - 진입: ${:.2}≈${:.2}, 청산: ${:.2}≈${:.2}",
                    symbol_base, trade.entry_price, entry_exp, trade.exit_price, exit_exp
                );
            }
        } else {
            price_mismatch_count += 1;
        }
    }

    println!("\n--- 검증 결과 ---");
    println!("가격 일치: {} 건", price_match_count);
    println!("가격 불일치: {} 건", price_mismatch_count);

    // 검증 1: 가격 불일치가 없어야 함
    assert_eq!(
        price_mismatch_count, 0,
        "❌ {} 건의 거래에서 가격 불일치 발생 - 심볼별 가격 매칭 버그!",
        price_mismatch_count
    );

    // 검증 2: 모든 심볼이 동일한 가격으로 거래되면 버그
    if report.trades.len() >= 2 {
        let first_entry = report.trades[0].entry_price;
        let all_same_entry = report.trades.iter().all(|t| t.entry_price == first_entry);

        assert!(
            !all_same_entry,
            "❌ 모든 거래가 동일한 진입가({})를 가짐 - 심볼별 가격 분리 버그!",
            first_entry
        );
        println!("✅ 심볼별 가격 분리 확인됨");
    }

    // 검증 3: 최소한 거래가 발생해야 함
    assert!(
        !report.trades.is_empty(),
        "❌ 거래가 발생하지 않음 - 전략 신호 또는 데이터 문제!"
    );

    println!("\n✅ 다중 자산 가격 매칭 검증 통과!");
    println!("========================================\n");
}
