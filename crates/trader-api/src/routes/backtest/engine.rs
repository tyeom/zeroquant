//! 백테스트 실행 함수들
//!
//! 전략별 백테스트를 실행하고 결과를 변환하는 함수를 제공합니다.
//!
//! # CPU-intensive 작업 처리
//!
//! 백테스트는 대량의 캔들 데이터를 처리하는 CPU-intensive 작업입니다.
//! Tokio async runtime의 worker thread를 블로킹하지 않도록
//! `tokio::task::spawn_blocking`을 사용하여 별도의 blocking thread pool에서 실행합니다.

use chrono::{NaiveDate, TimeZone, Utc};
use rust_decimal::Decimal;
use std::collections::{BTreeMap, HashMap};

use super::loader::parse_symbol;
use super::types::{
    BacktestConfigSummary, BacktestMetricsResponse, BacktestMultiRunResponse, BacktestRunResponse,
    EquityCurvePoint, TradeHistoryItem,
};

use trader_analytics::backtest::{BacktestConfig, BacktestEngine, BacktestReport};
use trader_core::{Kline, MarketType, Symbol, Timeframe};
use trader_strategy::strategies::{
    AllWeatherConfig, AllWeatherStrategy, BaaConfig, BaaStrategy, BollingerStrategy,
    CandlePatternConfig, CandlePatternStrategy, DualMomentumConfig, DualMomentumStrategy,
    GridStrategy, HaaConfig, HaaStrategy, InfinityBotConfig, InfinityBotStrategy,
    KosdaqFireRainConfig, KosdaqFireRainStrategy, KospiBothSideConfig, KospiBothSideStrategy,
    MagicSplitStrategy, MarketCapTopConfig, MarketCapTopStrategy, MarketInterestDayConfig,
    MarketInterestDayStrategy, PensionBotConfig, PensionBotStrategy, RsiStrategy,
    SectorMomentumConfig, SectorMomentumStrategy, SectorVbConfig, SectorVbStrategy,
    SimplePowerConfig, SimplePowerStrategy, SmaStrategy, SmallCapQuantConfig,
    SmallCapQuantStrategy, SnowConfig, SnowStrategy, StockGuganConfig, StockGuganStrategy,
    StockRotationConfig, StockRotationStrategy, Us3xLeverageConfig, Us3xLeverageStrategy,
    VolatilityBreakoutStrategy, XaaConfig, XaaStrategy,
};
use trader_strategy::Strategy;

/// 전략별 백테스트 실행
///
/// CPU-intensive 백테스트 계산을 `spawn_blocking`으로 별도 thread pool에서 실행하여
/// Tokio async runtime의 worker thread를 블로킹하지 않습니다.
pub async fn run_strategy_backtest(
    strategy_id: &str,
    config: BacktestConfig,
    klines: &[Kline],
    params: &Option<serde_json::Value>,
) -> Result<BacktestReport, String> {
    // 데이터를 owned 타입으로 변환하여 spawn_blocking으로 이동
    let strategy_id = strategy_id.to_string();
    let klines = klines.to_vec();
    let params = params.clone();

    // CPU-intensive 작업을 blocking thread pool에서 실행
    let report = tokio::task::spawn_blocking(move || {
        // blocking 컨텍스트에서 async 코드를 실행하기 위해 새 runtime 생성
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("Runtime 생성 실패: {}", e))?;

        rt.block_on(run_strategy_backtest_inner(
            &strategy_id,
            config,
            &klines,
            &params,
        ))
    })
    .await
    .map_err(|e| format!("백테스트 태스크 실행 실패: {}", e))??;

    Ok(report)
}

/// 내부 백테스트 실행 함수 (sync 컨텍스트에서 호출됨)
async fn run_strategy_backtest_inner(
    strategy_id: &str,
    config: BacktestConfig,
    klines: &[Kline],
    params: &Option<serde_json::Value>,
) -> Result<BacktestReport, String> {
    let mut engine = BacktestEngine::new(config);

    // 심볼 추출 (klines에서)
    let symbol_str = if let Some(first_kline) = klines.first() {
        first_kline.symbol.to_string()
    } else {
        "BTC/USDT".to_string()
    };

    // 기본 전략 설정 생성
    let _default_config = |strategy_symbol: &str| -> serde_json::Value {
        serde_json::json!({
            "symbol": strategy_symbol,
            "amount": "100000"
        })
    };

    // 사용자 파라미터와 기본 설정 병합
    let merge_params = |default: serde_json::Value,
                        user_params: &Option<serde_json::Value>|
     -> serde_json::Value {
        if let Some(user) = user_params {
            if let (Some(default_obj), Some(user_obj)) = (default.as_object(), user.as_object()) {
                let mut merged = default_obj.clone();
                for (key, value) in user_obj {
                    merged.insert(key.clone(), value.clone());
                }
                return serde_json::Value::Object(merged);
            }
        }
        default
    };

    match strategy_id {
        "rsi_mean_reversion" | "rsi" => {
            let mut strategy = RsiStrategy::new();
            let strategy_config = merge_params(
                serde_json::json!({
                    "symbol": symbol_str,
                    "period": 14,
                    "oversold_threshold": 30.0,
                    "overbought_threshold": 70.0,
                    "amount": "100000"
                }),
                params,
            );
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        "grid_trading" | "grid" => {
            let mut strategy = GridStrategy::new();
            let strategy_config = merge_params(
                serde_json::json!({
                    "symbol": symbol_str,
                    "grid_spacing_pct": 1.0,
                    "grid_levels": 10,
                    "amount_per_level": "100000"
                }),
                params,
            );
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        "bollinger" => {
            let mut strategy = BollingerStrategy::new();
            let strategy_config = merge_params(
                serde_json::json!({
                    "symbol": symbol_str,
                    "period": 20,
                    "std_multiplier": 1.5,
                    "use_rsi_confirmation": false,
                    "min_bandwidth_pct": 0.0,
                    "amount": "100000"
                }),
                params,
            );
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        "volatility_breakout" => {
            let mut strategy = VolatilityBreakoutStrategy::new();
            let strategy_config = merge_params(
                serde_json::json!({
                    "symbol": symbol_str,
                    "k_factor": 0.3,
                    "lookback_period": 1,
                    "use_atr": true,
                    "atr_period": 5,
                    "min_range_pct": 0.1,
                    "amount": "100000"
                }),
                params,
            );
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        "magic_split" => {
            let mut strategy = MagicSplitStrategy::new();
            let strategy_config = merge_params(
                serde_json::json!({
                    "symbol": symbol_str,
                    "levels": [
                        {"number": 1, "target_rate": "10.0", "trigger_rate": null, "invest_money": "200000"},
                        {"number": 2, "target_rate": "2.0", "trigger_rate": "-3.0", "invest_money": "100000"},
                        {"number": 3, "target_rate": "3.0", "trigger_rate": "-5.0", "invest_money": "100000"},
                        {"number": 4, "target_rate": "3.0", "trigger_rate": "-5.0", "invest_money": "100000"},
                        {"number": 5, "target_rate": "4.0", "trigger_rate": "-6.0", "invest_money": "100000"}
                    ],
                    "allow_same_day_reentry": false,
                    "slippage_tolerance": "1.0"
                }),
                params,
            );
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        "simple_power" => {
            let mut strategy = SimplePowerStrategy::new();
            let default_cfg =
                serde_json::to_value(SimplePowerConfig::default()).map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        "haa" => {
            let mut strategy = HaaStrategy::new();
            let default_cfg =
                serde_json::to_value(HaaConfig::default()).map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        "xaa" => {
            let mut strategy = XaaStrategy::new();
            let default_cfg =
                serde_json::to_value(XaaConfig::default()).map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        "stock_rotation" => {
            let mut strategy = StockRotationStrategy::new();
            let default_cfg =
                serde_json::to_value(StockRotationConfig::default()).map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        "sma_crossover" => {
            // SMA 크로스오버 전략
            let mut strategy = SmaStrategy::new();
            let strategy_config = merge_params(
                serde_json::json!({
                    "symbol": symbol_str,
                    "short_period": 10,
                    "long_period": 20,
                    "amount": "100000"
                }),
                params,
            );
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        "all_weather" | "all_weather_us" | "all_weather_kr" => {
            let mut strategy = AllWeatherStrategy::new();
            // market 필드가 params에 있으면 그대로 사용, 없으면 기본값 US
            let default_cfg =
                serde_json::to_value(AllWeatherConfig::default()).map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        "snow" | "snow_us" | "snow_kr" => {
            let mut strategy = SnowStrategy::new();
            // market 필드가 params에 있으면 그대로 사용, 없으면 기본값 US
            let default_cfg =
                serde_json::to_value(SnowConfig::default()).map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        "market_cap_top" => {
            let mut strategy = MarketCapTopStrategy::new();
            let default_cfg =
                serde_json::to_value(MarketCapTopConfig::default()).map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        "candle_pattern" => {
            let mut strategy = CandlePatternStrategy::new();
            let default_cfg =
                serde_json::to_value(CandlePatternConfig::default()).map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        "infinity_bot" => {
            let mut strategy = InfinityBotStrategy::new();
            let default_cfg =
                serde_json::to_value(InfinityBotConfig::default()).map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        "market_interest_day" => {
            let mut strategy = MarketInterestDayStrategy::new();
            let default_cfg = serde_json::to_value(MarketInterestDayConfig::default())
                .map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        // 3차 전략들
        "baa" => {
            let mut strategy = BaaStrategy::new();
            let default_cfg =
                serde_json::to_value(BaaConfig::default()).map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        "sector_momentum" => {
            let mut strategy = SectorMomentumStrategy::new();
            let default_cfg =
                serde_json::to_value(SectorMomentumConfig::default()).map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        "dual_momentum" => {
            let mut strategy = DualMomentumStrategy::new();
            let default_cfg =
                serde_json::to_value(DualMomentumConfig::default()).map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        "small_cap_quant" => {
            let mut strategy = SmallCapQuantStrategy::new();
            let default_cfg =
                serde_json::to_value(SmallCapQuantConfig::default()).map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        "pension_bot" => {
            let mut strategy = PensionBotStrategy::new();
            let default_cfg =
                serde_json::to_value(PensionBotConfig::default()).map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        // 2차 전략들
        "sector_vb" => {
            let mut strategy = SectorVbStrategy::new();
            let default_cfg =
                serde_json::to_value(SectorVbConfig::default()).map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        "kospi_bothside" => {
            let mut strategy = KospiBothSideStrategy::new();
            let default_cfg =
                serde_json::to_value(KospiBothSideConfig::default()).map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        "kosdaq_fire_rain" => {
            let mut strategy = KosdaqFireRainStrategy::new();
            let default_cfg =
                serde_json::to_value(KosdaqFireRainConfig::default()).map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        "us_3x_leverage" => {
            let mut strategy = Us3xLeverageStrategy::new();
            let default_cfg =
                serde_json::to_value(Us3xLeverageConfig::default()).map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        "stock_gugan" => {
            let mut strategy = StockGuganStrategy::new();
            let default_cfg =
                serde_json::to_value(StockGuganConfig::default()).map_err(|e| e.to_string())?;
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, klines)
                .await
                .map_err(|e| e.to_string())
        }
        _ => {
            return Err(format!("지원하지 않는 전략입니다: {}", strategy_id));
        }
    }
}

/// 다중 자산 전략 백테스트 실행
///
/// CPU-intensive 백테스트 계산을 `spawn_blocking`으로 별도 thread pool에서 실행하여
/// Tokio async runtime의 worker thread를 블로킹하지 않습니다.
pub async fn run_multi_strategy_backtest(
    strategy_id: &str,
    config: BacktestConfig,
    merged_klines: &[Kline],
    multi_klines: &HashMap<String, Vec<Kline>>,
    params: &Option<serde_json::Value>,
) -> Result<BacktestReport, String> {
    // 데이터를 owned 타입으로 변환하여 spawn_blocking으로 이동
    let strategy_id = strategy_id.to_string();
    let merged_klines = merged_klines.to_vec();
    let multi_klines = multi_klines.clone();
    let params = params.clone();

    // CPU-intensive 작업을 blocking thread pool에서 실행
    let report = tokio::task::spawn_blocking(move || {
        // blocking 컨텍스트에서 async 코드를 실행하기 위해 새 runtime 생성
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("Runtime 생성 실패: {}", e))?;

        rt.block_on(run_multi_strategy_backtest_inner(
            &strategy_id,
            config,
            &merged_klines,
            &multi_klines,
            &params,
        ))
    })
    .await
    .map_err(|e| format!("다중 자산 백테스트 태스크 실행 실패: {}", e))??;

    Ok(report)
}

/// 내부 다중 자산 백테스트 실행 함수 (sync 컨텍스트에서 호출됨)
async fn run_multi_strategy_backtest_inner(
    strategy_id: &str,
    config: BacktestConfig,
    merged_klines: &[Kline],
    multi_klines: &HashMap<String, Vec<Kline>>,
    params: &Option<serde_json::Value>,
) -> Result<BacktestReport, String> {
    // 초기 자본금을 먼저 복사 (클로저에서 사용)
    let initial_capital = config.initial_capital;

    let mut engine = BacktestEngine::new(config);

    // 사용자 파라미터와 기본 설정 병합
    let merge_params = |default: serde_json::Value,
                        user_params: &Option<serde_json::Value>|
     -> serde_json::Value {
        if let Some(user) = user_params {
            if let (Some(default_obj), Some(user_obj)) = (default.as_object(), user.as_object()) {
                let mut merged = default_obj.clone();
                for (key, value) in user_obj {
                    merged.insert(key.clone(), value.clone());
                }
                return serde_json::Value::Object(merged);
            }
        }
        default
    };

    // 심볼 목록 추출
    let symbols: Vec<String> = multi_klines.keys().cloned().collect();

    // 초기 자본금을 전략 파라미터에 주입
    let inject_common_params = |mut cfg: serde_json::Value| -> serde_json::Value {
        if let Some(obj) = cfg.as_object_mut() {
            // 심볼 목록 주입
            obj.insert(
                "symbols".to_string(),
                serde_json::Value::Array(
                    symbols
                        .iter()
                        .map(|s| serde_json::Value::String(s.clone()))
                        .collect(),
                ),
            );
            // 초기 자본금 주입 (cash_balance로 사용)
            obj.insert(
                "initial_capital".to_string(),
                serde_json::Value::String(initial_capital.to_string()),
            );
        }
        cfg
    };

    match strategy_id {
        "simple_power" => {
            let mut strategy = SimplePowerStrategy::new();
            let default_cfg =
                serde_json::to_value(SimplePowerConfig::default()).map_err(|e| e.to_string())?;
            let default_cfg = inject_common_params(default_cfg);
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, merged_klines)
                .await
                .map_err(|e| e.to_string())
        }
        "haa" => {
            let mut strategy = HaaStrategy::new();
            let default_cfg =
                serde_json::to_value(HaaConfig::default()).map_err(|e| e.to_string())?;
            let default_cfg = inject_common_params(default_cfg);
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, merged_klines)
                .await
                .map_err(|e| e.to_string())
        }
        "xaa" => {
            let mut strategy = XaaStrategy::new();
            let default_cfg =
                serde_json::to_value(XaaConfig::default()).map_err(|e| e.to_string())?;
            let default_cfg = inject_common_params(default_cfg);
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, merged_klines)
                .await
                .map_err(|e| e.to_string())
        }
        "stock_rotation" => {
            let mut strategy = StockRotationStrategy::new();
            let default_cfg =
                serde_json::to_value(StockRotationConfig::default()).map_err(|e| e.to_string())?;
            let default_cfg = inject_common_params(default_cfg);
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, merged_klines)
                .await
                .map_err(|e| e.to_string())
        }
        // 추가 다중 자산 전략들
        "all_weather" | "all_weather_us" | "all_weather_kr" => {
            let mut strategy = AllWeatherStrategy::new();
            let default_cfg =
                serde_json::to_value(AllWeatherConfig::default()).map_err(|e| e.to_string())?;
            let default_cfg = inject_common_params(default_cfg);
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, merged_klines)
                .await
                .map_err(|e| e.to_string())
        }
        "snow" | "snow_us" | "snow_kr" => {
            let mut strategy = SnowStrategy::new();
            let default_cfg =
                serde_json::to_value(SnowConfig::default()).map_err(|e| e.to_string())?;
            let default_cfg = inject_common_params(default_cfg);
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, merged_klines)
                .await
                .map_err(|e| e.to_string())
        }
        "baa" => {
            let mut strategy = BaaStrategy::new();
            let default_cfg =
                serde_json::to_value(BaaConfig::default()).map_err(|e| e.to_string())?;
            let default_cfg = inject_common_params(default_cfg);
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, merged_klines)
                .await
                .map_err(|e| e.to_string())
        }
        "sector_momentum" => {
            let mut strategy = SectorMomentumStrategy::new();
            let default_cfg =
                serde_json::to_value(SectorMomentumConfig::default()).map_err(|e| e.to_string())?;
            let default_cfg = inject_common_params(default_cfg);
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, merged_klines)
                .await
                .map_err(|e| e.to_string())
        }
        "dual_momentum" => {
            let mut strategy = DualMomentumStrategy::new();
            let default_cfg =
                serde_json::to_value(DualMomentumConfig::default()).map_err(|e| e.to_string())?;
            let default_cfg = inject_common_params(default_cfg);
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, merged_klines)
                .await
                .map_err(|e| e.to_string())
        }
        "pension_bot" => {
            let mut strategy = PensionBotStrategy::new();
            let default_cfg =
                serde_json::to_value(PensionBotConfig::default()).map_err(|e| e.to_string())?;
            let default_cfg = inject_common_params(default_cfg);
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, merged_klines)
                .await
                .map_err(|e| e.to_string())
        }
        "market_cap_top" => {
            let mut strategy = MarketCapTopStrategy::new();
            let default_cfg =
                serde_json::to_value(MarketCapTopConfig::default()).map_err(|e| e.to_string())?;
            let default_cfg = inject_common_params(default_cfg);
            let strategy_config = merge_params(default_cfg, params);
            strategy
                .initialize(strategy_config)
                .await
                .map_err(|e| e.to_string())?;
            engine
                .run(&mut strategy, merged_klines)
                .await
                .map_err(|e| e.to_string())
        }
        _ => Err(format!(
            "지원하지 않는 다중 자산 전략입니다: {}",
            strategy_id
        )),
    }
}

/// BacktestReport를 API 응답으로 변환
pub fn convert_report_to_response(
    report: &BacktestReport,
    strategy_id: &str,
    symbol: &str,
    start_date: &str,
    end_date: &str,
) -> BacktestRunResponse {
    let result_id = uuid::Uuid::new_v4().to_string();

    // 자산 곡선 변환
    let equity_curve: Vec<EquityCurvePoint> = report
        .equity_curve
        .iter()
        .map(|ep| EquityCurvePoint {
            timestamp: ep.timestamp.timestamp(),
            equity: ep.equity,
            drawdown_pct: ep.drawdown_pct,
        })
        .collect();

    // 거래 내역 변환
    let trades: Vec<TradeHistoryItem> = report
        .trades
        .iter()
        .map(|rt| TradeHistoryItem {
            symbol: rt.symbol.to_string(),
            entry_time: rt.entry_time,
            exit_time: rt.exit_time,
            entry_price: rt.entry_price,
            exit_price: rt.exit_price,
            quantity: rt.quantity,
            side: rt.side,
            pnl: rt.pnl,
            return_pct: rt.return_pct,
        })
        .collect();

    // 성과 지표 변환
    let metrics = BacktestMetricsResponse {
        total_return_pct: report.metrics.total_return_pct,
        annualized_return_pct: report.metrics.annualized_return_pct,
        net_profit: report.metrics.net_profit,
        total_trades: report.metrics.total_trades,
        win_rate_pct: report.metrics.win_rate_pct,
        profit_factor: report.metrics.profit_factor,
        sharpe_ratio: report.metrics.sharpe_ratio,
        sortino_ratio: report.metrics.sortino_ratio,
        max_drawdown_pct: report.metrics.max_drawdown_pct,
        calmar_ratio: report.metrics.calmar_ratio,
        avg_win: report.metrics.avg_win,
        avg_loss: report.metrics.avg_loss,
        largest_win: report.metrics.largest_win,
        largest_loss: report.metrics.largest_loss,
    };

    let config_summary = BacktestConfigSummary {
        initial_capital: report.config.initial_capital,
        commission_rate: report.config.commission_rate,
        slippage_rate: report.config.slippage_rate,
        total_commission: report.total_commission,
        total_slippage: report.total_slippage,
        data_points: report.data_points,
    };

    BacktestRunResponse {
        id: result_id,
        success: true,
        strategy_id: strategy_id.to_string(),
        symbol: symbol.to_string(),
        start_date: start_date.to_string(),
        end_date: end_date.to_string(),
        metrics,
        equity_curve,
        trades,
        config_summary,
    }
}

/// 다중 자산 BacktestReport를 API 응답으로 변환
pub fn convert_multi_report_to_response(
    report: &BacktestReport,
    strategy_id: &str,
    symbols: &[String],
    start_date: &str,
    end_date: &str,
    data_points_by_symbol: HashMap<String, usize>,
) -> BacktestMultiRunResponse {
    let result_id = uuid::Uuid::new_v4().to_string();

    // 다중 자산 전략에서 같은 timestamp에 여러 equity 값이 기록될 수 있음
    // 같은 timestamp의 마지막 값만 유지하여 데이터 왜곡 방지
    let mut equity_map: BTreeMap<i64, EquityCurvePoint> = BTreeMap::new();
    for ep in &report.equity_curve {
        let ts = ep.timestamp.timestamp();
        // 같은 timestamp면 마지막 값(전체 포트폴리오 equity)으로 덮어씀
        equity_map.insert(
            ts,
            EquityCurvePoint {
                timestamp: ts,
                equity: ep.equity,
                drawdown_pct: ep.drawdown_pct,
            },
        );
    }
    let equity_curve: Vec<EquityCurvePoint> = equity_map.into_values().collect();

    let trades: Vec<TradeHistoryItem> = report
        .trades
        .iter()
        .map(|rt| TradeHistoryItem {
            symbol: rt.symbol.to_string(),
            entry_time: rt.entry_time,
            exit_time: rt.exit_time,
            entry_price: rt.entry_price,
            exit_price: rt.exit_price,
            quantity: rt.quantity,
            side: rt.side,
            pnl: rt.pnl,
            return_pct: rt.return_pct,
        })
        .collect();

    let metrics = BacktestMetricsResponse {
        total_return_pct: report.metrics.total_return_pct,
        annualized_return_pct: report.metrics.annualized_return_pct,
        net_profit: report.metrics.net_profit,
        total_trades: report.metrics.total_trades,
        win_rate_pct: report.metrics.win_rate_pct,
        profit_factor: report.metrics.profit_factor,
        sharpe_ratio: report.metrics.sharpe_ratio,
        sortino_ratio: report.metrics.sortino_ratio,
        max_drawdown_pct: report.metrics.max_drawdown_pct,
        calmar_ratio: report.metrics.calmar_ratio,
        avg_win: report.metrics.avg_win,
        avg_loss: report.metrics.avg_loss,
        largest_win: report.metrics.largest_win,
        largest_loss: report.metrics.largest_loss,
    };

    let config_summary = BacktestConfigSummary {
        initial_capital: report.config.initial_capital,
        commission_rate: report.config.commission_rate,
        slippage_rate: report.config.slippage_rate,
        total_commission: report.total_commission,
        total_slippage: report.total_slippage,
        data_points: report.data_points,
    };

    BacktestMultiRunResponse {
        id: result_id,
        success: true,
        strategy_id: strategy_id.to_string(),
        symbols: symbols.to_vec(),
        start_date: start_date.to_string(),
        end_date: end_date.to_string(),
        metrics,
        equity_curve,
        trades,
        config_summary,
        data_points_by_symbol,
    }
}

/// 다중 심볼 샘플 Kline 데이터 생성
pub fn generate_multi_sample_klines(
    symbols: &[String],
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> HashMap<String, Vec<Kline>> {
    use rust_decimal::prelude::FromPrimitive;

    let mut result = HashMap::new();
    let days = (end_date - start_date).num_days() as usize;

    // 심볼별 기본 가격 설정 (다양성을 위해)
    let base_prices: HashMap<&str, f64> = [
        ("TQQQ", 45.0),
        ("SCHD", 75.0),
        ("PFIX", 25.0),
        ("TMF", 8.0),
        ("SPY", 450.0),
        ("QQQ", 380.0),
        ("TLT", 95.0),
        ("IEF", 100.0),
        ("VEA", 45.0),
        ("VWO", 40.0),
        ("TIP", 110.0),
        ("BIL", 91.5),
        ("PDBC", 15.0),
        ("VNQ", 85.0),
        ("EFA", 72.0),
        ("EEM", 38.0),
        ("LQD", 108.0),
        ("BND", 72.0),
    ]
    .iter()
    .cloned()
    .collect();

    for symbol_str in symbols {
        let (base, quote) = parse_symbol(symbol_str);
        // Symbol 생성자를 통해 country 필드 자동 추론
        let symbol = Symbol::new(&base, &quote, MarketType::Stock);

        let base_price = *base_prices.get(base.as_str()).unwrap_or(&50.0);

        let klines: Vec<Kline> = (0..=days)
            .map(|i| {
                let date = start_date + chrono::Duration::days(i as i64);
                let open_time = Utc.from_utc_datetime(&date.and_hms_opt(9, 0, 0).unwrap());
                let close_time = Utc.from_utc_datetime(&date.and_hms_opt(15, 30, 0).unwrap());

                // 심볼별로 다른 변동성 패턴
                let volatility = match base.as_str() {
                    "TQQQ" | "TMF" => 0.04, // 레버리지 ETF: 높은 변동성
                    "BIL" => 0.001,         // 단기 채권: 매우 낮은 변동성
                    "TLT" | "IEF" => 0.015, // 채권 ETF: 중간 변동성
                    _ => 0.02,              // 일반 ETF
                };

                let noise = ((i as f64 * 0.7).sin() + (i as f64 * 1.3).cos()) * volatility;
                let trend = match base.as_str() {
                    "TQQQ" | "QQQ" | "SPY" => i as f64 * 0.0005, // 상승 추세
                    "TLT" | "TMF" => i as f64 * -0.0003,         // 하락 추세 (금리 상승)
                    _ => i as f64 * 0.0001,
                };
                let price_mult = 1.0 + noise + trend;

                let open = base_price * price_mult;
                let high = open * (1.0 + volatility * 0.5);
                let low = open * (1.0 - volatility * 0.5);
                let close = open * (1.0 + noise * 0.3);
                let volume = 1000000.0 * (1.0 + noise.abs());

                Kline {
                    symbol: symbol.clone(),
                    timeframe: Timeframe::D1,
                    open_time,
                    close_time,
                    open: Decimal::from_f64(open).unwrap_or(Decimal::from(50)),
                    high: Decimal::from_f64(high).unwrap_or(Decimal::from(51)),
                    low: Decimal::from_f64(low).unwrap_or(Decimal::from(49)),
                    close: Decimal::from_f64(close).unwrap_or(Decimal::from(50)),
                    volume: Decimal::from_f64(volume).unwrap_or(Decimal::from(1000000)),
                    quote_volume: None,
                    num_trades: None,
                }
            })
            .collect();

        result.insert(symbol_str.clone(), klines);
    }

    result
}
