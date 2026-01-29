//! End-to-end integration test for the backtesting system.
//!
//! This test demonstrates the complete pipeline:
//! 1. Load historical data from CSV (downloaded from Binance)
//! 2. Create a simple SMA crossover strategy
//! 3. Run the backtest engine
//! 4. Verify the results

use async_trait::async_trait;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::path::Path;

use trader_analytics::backtest::{BacktestConfig, BacktestEngine};
use trader_core::{MarketData, MarketDataType, Order, Position, Side, Signal, Symbol, Timeframe};
use trader_exchange::simulated::{DataFeed, DataFeedConfig};
use trader_strategy::Strategy;

/// Simple Moving Average (SMA) Crossover Strategy for testing.
///
/// Generates buy signals when fast SMA crosses above slow SMA,
/// and sell signals when fast SMA crosses below slow SMA.
struct SmaCrossoverStrategy {
    /// Strategy name
    name: String,
    /// Target symbol
    symbol: Symbol,
    /// Fast SMA period
    fast_period: usize,
    /// Slow SMA period
    slow_period: usize,
    /// Price history for SMA calculation
    prices: VecDeque<Decimal>,
    /// Previous fast SMA value
    prev_fast_sma: Option<Decimal>,
    /// Previous slow SMA value
    prev_slow_sma: Option<Decimal>,
    /// Current position state
    in_position: bool,
    /// Signal count for debugging
    signal_count: usize,
}

impl SmaCrossoverStrategy {
    fn new(symbol: Symbol, fast_period: usize, slow_period: usize) -> Self {
        Self {
            name: "SMA Crossover".to_string(),
            symbol,
            fast_period,
            slow_period,
            prices: VecDeque::new(),
            prev_fast_sma: None,
            prev_slow_sma: None,
            in_position: false,
            signal_count: 0,
        }
    }

    /// Calculate Simple Moving Average
    fn calculate_sma(&self, period: usize) -> Option<Decimal> {
        if self.prices.len() < period {
            return None;
        }

        let sum: Decimal = self.prices.iter().take(period).copied().sum();
        Some(sum / Decimal::from(period))
    }
}

#[async_trait]
impl Strategy for SmaCrossoverStrategy {
    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "SMA Crossover strategy for backtesting"
    }

    async fn initialize(&mut self, _config: Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.prices.clear();
        self.prev_fast_sma = None;
        self.prev_slow_sma = None;
        self.in_position = false;
        self.signal_count = 0;
        Ok(())
    }

    async fn on_market_data(
        &mut self,
        data: &MarketData,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
        // Only process kline data for our symbol
        let kline = match &data.data {
            MarketDataType::Kline(k) if k.symbol == self.symbol => k,
            _ => return Ok(Vec::new()),
        };

        // Update price history (most recent first)
        self.prices.push_front(kline.close);
        if self.prices.len() > self.slow_period + 1 {
            self.prices.pop_back();
        }

        // Calculate SMAs
        let fast_sma = match self.calculate_sma(self.fast_period) {
            Some(v) => v,
            None => return Ok(Vec::new()),
        };

        let slow_sma = match self.calculate_sma(self.slow_period) {
            Some(v) => v,
            None => return Ok(Vec::new()),
        };

        let mut signals = Vec::new();

        // Check for crossover
        if let (Some(prev_fast), Some(prev_slow)) = (self.prev_fast_sma, self.prev_slow_sma) {
            // Bullish crossover: fast crosses above slow
            if prev_fast <= prev_slow && fast_sma > slow_sma && !self.in_position {
                let signal = Signal::entry(&self.name, self.symbol.clone(), Side::Buy)
                    .with_strength(0.5) // Use 50% of available capital
                    .with_prices(Some(kline.close), None, None);
                signals.push(signal);
                self.in_position = true;
                self.signal_count += 1;
            }

            // Bearish crossover: fast crosses below slow
            if prev_fast >= prev_slow && fast_sma < slow_sma && self.in_position {
                let signal = Signal::exit(&self.name, self.symbol.clone(), Side::Sell)
                    .with_strength(1.0) // Close full position
                    .with_prices(Some(kline.close), None, None);
                signals.push(signal);
                self.in_position = false;
                self.signal_count += 1;
            }
        }

        // Update previous SMA values
        self.prev_fast_sma = Some(fast_sma);
        self.prev_slow_sma = Some(slow_sma);

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        _order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn on_position_update(
        &mut self,
        _position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "in_position": self.in_position,
            "signal_count": self.signal_count,
            "prices_count": self.prices.len(),
        })
    }
}

/// Test loading data from CSV and running backtest.
#[tokio::test]
async fn test_full_backtest_pipeline() {
    // Skip if test data doesn't exist
    let csv_path = Path::new("../../data/btcusdt_1m_jan2024.csv");
    if !csv_path.exists() {
        eprintln!("Skipping test: CSV file not found at {:?}", csv_path);
        eprintln!("Run: cargo run -p trader-cli -- download -s BTCUSDT -i 1m -f 2024-01-01 -t 2024-01-31 -o data/btcusdt_1m_jan2024.csv");
        return;
    }

    // 1. Load data from CSV
    let symbol = Symbol::crypto("BTC", "USDT");
    let mut feed = DataFeed::new(DataFeedConfig::default());
    let count = feed
        .load_from_csv(symbol.clone(), Timeframe::M1, csv_path)
        .expect("Failed to load CSV");

    println!("Loaded {} candles from CSV", count);
    assert!(count > 10000, "Should have enough data for meaningful backtest");

    // 2. Extract klines for backtest engine
    let mut klines = Vec::new();
    while let Some(kline) = feed.next_kline(&symbol, Timeframe::M1) {
        klines.push(kline);
    }
    println!("Extracted {} klines for backtest", klines.len());

    // 3. Create strategy
    let mut strategy = SmaCrossoverStrategy::new(
        symbol.clone(),
        10,  // Fast SMA period
        30,  // Slow SMA period
    );
    strategy.initialize(json!({})).await.unwrap();

    // 4. Configure and run backtest
    let config = BacktestConfig::new(dec!(10_000_000)) // 1000만원
        .with_commission_rate(dec!(0.001))  // 0.1% 수수료
        .with_slippage_rate(dec!(0.0005))   // 0.05% 슬리피지
        .with_max_position_size_pct(dec!(0.2)); // 20% 최대 포지션

    let mut engine = BacktestEngine::new(config);
    let report = engine.run(&mut strategy, &klines).await
        .expect("Backtest should complete successfully");

    // 5. Verify results
    println!("\n{}", report.summary());

    // Basic sanity checks
    assert!(report.data_points > 0, "Should process data points");
    assert!(report.start_time < report.end_time, "Time range should be valid");

    // Check that some trades were made
    println!("Total trades: {}", report.metrics.total_trades);
    println!("Total orders: {}", report.total_orders);
    println!("Signals generated: {}", strategy.signal_count);

    // The strategy should generate some signals on this dataset
    assert!(strategy.signal_count > 0, "Strategy should generate at least some signals");

    // Verify metrics are calculated (Decimal type - no NaN possible)
    println!("Sharpe ratio: {}", report.metrics.sharpe_ratio);

    // Commission and slippage should be tracked
    if report.total_orders > 0 {
        assert!(report.total_commission > Decimal::ZERO, "Commission should be charged");
    }

    println!("\n=== Backtest completed successfully ===");
}

/// Test with smaller dataset for quick verification.
#[tokio::test]
async fn test_backtest_with_hourly_data() {
    let csv_path = Path::new("../../data/test_btcusdt_1h.csv");
    if !csv_path.exists() {
        eprintln!("Skipping test: CSV file not found");
        return;
    }

    let symbol = Symbol::crypto("BTC", "USDT");
    let mut feed = DataFeed::new(DataFeedConfig::default());
    let count = feed
        .load_from_csv(symbol.clone(), Timeframe::H1, csv_path)
        .expect("Failed to load CSV");

    println!("Loaded {} hourly candles", count);

    // Extract klines
    let mut klines = Vec::new();
    while let Some(kline) = feed.next_kline(&symbol, Timeframe::H1) {
        klines.push(kline);
    }

    // Use shorter SMA periods for hourly data
    let mut strategy = SmaCrossoverStrategy::new(symbol.clone(), 5, 12);
    strategy.initialize(json!({})).await.unwrap();

    let config = BacktestConfig::new(dec!(10_000_000))
        .with_commission_rate(dec!(0.001))
        .with_slippage_rate(dec!(0.0005));

    let mut engine = BacktestEngine::new(config);
    let report = engine.run(&mut strategy, &klines).await
        .expect("Backtest should complete");

    println!("\n{}", report.summary());
    println!("Signals generated: {}", strategy.signal_count);

    // With only 168 candles (7 days of hourly data), might have few or no crossovers
    // This is expected - the test verifies the pipeline works, not profitability
    assert!(report.data_points == count, "All data points should be processed");
}

/// Test Grid Trading strategy with BTC data.
///
/// Grid trading places buy/sell orders at regular intervals around a center price,
/// profiting from market oscillations rather than directional moves.
#[tokio::test]
async fn test_grid_trading_strategy() {
    use trader_strategy::strategies::GridStrategy;

    // Skip if test data doesn't exist
    let csv_path = Path::new("../../data/btcusdt_1m_jan2024.csv");
    if !csv_path.exists() {
        eprintln!("Skipping test: CSV file not found at {:?}", csv_path);
        eprintln!("Run: cargo run -p trader-cli -- download -s BTCUSDT -i 1m -f 2024-01-01 -t 2024-01-31 -o data/btcusdt_1m_jan2024.csv");
        return;
    }

    // 1. Load data from CSV
    let symbol = Symbol::crypto("BTC", "USDT");
    let mut feed = DataFeed::new(DataFeedConfig::default());
    let count = feed
        .load_from_csv(symbol.clone(), Timeframe::M1, csv_path)
        .expect("Failed to load CSV");

    println!("Loaded {} candles from CSV", count);

    // Get initial price for grid center calculation
    let mut klines = Vec::new();
    while let Some(kline) = feed.next_kline(&symbol, Timeframe::M1) {
        klines.push(kline);
    }

    // Calculate initial price from first candle
    let initial_price = klines.first()
        .map(|k| k.close)
        .unwrap_or(dec!(42000));

    println!("Initial BTC price: {} USDT", initial_price);

    // 2. Create Grid Trading strategy
    let mut strategy = GridStrategy::new();

    // Configure grid with 0.5% spacing, 8 levels each side
    // Note: max_position_size is removed to allow sell signals without tracking internal position
    let grid_config = json!({
        "symbol": "BTC/USDT",
        "center_price": initial_price.to_string(),
        "grid_spacing_pct": 0.5,        // 0.5% spacing for 1-minute data
        "grid_levels": 8,               // 8 levels above and below
        "amount_per_level": "50000",    // 5만원 per level
        "dynamic_spacing": false,       // Use fixed spacing
        "trend_filter": false,          // Trade both directions
        "reset_threshold_pct": 3.0      // Reset grid if center drifts 3%
        // max_position_size: removed to bypass internal position tracking
    });

    strategy.initialize(grid_config).await
        .expect("Failed to initialize grid strategy");

    // Send first data point to initialize grid
    let first_data = trader_core::MarketData::from_kline("backtest", klines[0].clone());
    let init_signals = strategy.on_market_data(&first_data).await
        .expect("First data should succeed");

    // Debug: print grid state after first data
    let state_after_init = strategy.get_state();
    println!("\n그리드 초기화 후 상태:");
    println!("  - stats: {:?}", state_after_init.get("stats"));

    // 3. Configure and run backtest
    // Using lower commission rate (0.05%) to simulate maker orders
    let config = BacktestConfig::new(dec!(10_000_000)) // 1000만원
        .with_commission_rate(dec!(0.0005))  // 0.05% 수수료 (Binance VIP or maker)
        .with_slippage_rate(dec!(0.0001))    // 0.01% 슬리피지 (지정가 주문)
        .with_max_position_size_pct(dec!(0.05)); // 5% per trade

    let mut engine = BacktestEngine::new(config);

    println!("\n========================================");
    println!("  그리드 트레이딩 백테스트 시작");
    println!("========================================");
    println!("그리드 설정:");
    println!("  - 중심가: {} USDT", initial_price);
    println!("  - 간격: 0.5%");
    println!("  - 레벨 수: 8 (위/아래 각각)");
    println!("  - 레벨당 금액: 50,000 USDT");
    println!("========================================\n");

    let report = engine.run(&mut strategy, &klines).await
        .expect("Backtest should complete successfully");

    // 4. Print detailed results
    println!("\n{}", report.summary());

    // Get strategy state for additional info
    let state = strategy.get_state();
    println!("\n그리드 전략 상태:");
    println!("  - 초기화됨: {}", state["initialized"]);
    println!("  - 총 거래 수: {}", state["trades_count"]);
    println!("  - 포지션 크기: {}", state["position_size"]);
    println!("  - 총 손익: {}", state["total_pnl"]);

    // 5. Verify results
    assert!(report.data_points > 0, "Should process data points");
    assert!(report.start_time < report.end_time, "Time range should be valid");

    // Grid trading should generate more trades than trend following
    println!("\n총 거래: {}", report.metrics.total_trades);
    println!("총 주문: {}", report.total_orders);

    // Compare with expected behavior
    // Grid trading in a volatile market should:
    // - Generate more trades than trend following
    // - Have higher win rate (many small profits)
    // - Have lower maximum drawdown (positions are spread)

    if report.metrics.total_trades > 0 {
        println!("\n승률: {:.1}%", report.metrics.win_rate_pct);
        println!("Profit Factor: {}", report.metrics.profit_factor);
        println!("최대 낙폭: {:.2}%", report.metrics.max_drawdown_pct);
    }

    println!("\n=== 그리드 전략 백테스트 완료 ===");
}

/// Test Grid strategy with hourly data (less noise, clearer patterns).
#[tokio::test]
async fn test_grid_trading_hourly() {
    use trader_strategy::strategies::GridStrategy;

    let csv_path = Path::new("../../data/test_btcusdt_1h.csv");
    if !csv_path.exists() {
        eprintln!("Skipping test: hourly CSV file not found");
        return;
    }

    let symbol = Symbol::crypto("BTC", "USDT");
    let mut feed = DataFeed::new(DataFeedConfig::default());
    let count = feed
        .load_from_csv(symbol.clone(), Timeframe::H1, csv_path)
        .expect("Failed to load CSV");

    println!("Loaded {} hourly candles", count);

    let mut klines = Vec::new();
    while let Some(kline) = feed.next_kline(&symbol, Timeframe::H1) {
        klines.push(kline);
    }

    let initial_price = klines.first()
        .map(|k| k.close)
        .unwrap_or(dec!(42000));

    // Grid with wider spacing for hourly data
    let mut strategy = GridStrategy::new();
    let grid_config = json!({
        "symbol": "BTC/USDT",
        "center_price": initial_price.to_string(),
        "grid_spacing_pct": 1.5,        // 1.5% for hourly
        "grid_levels": 5,
        "amount_per_level": "100000",
        "dynamic_spacing": true,        // Use ATR-based spacing
        "atr_period": 14,
        "atr_multiplier": 1.0,
        "trend_filter": false
    });

    strategy.initialize(grid_config).await.unwrap();

    let config = BacktestConfig::new(dec!(10_000_000))
        .with_commission_rate(dec!(0.001))
        .with_slippage_rate(dec!(0.0001));

    let mut engine = BacktestEngine::new(config);
    let report = engine.run(&mut strategy, &klines).await
        .expect("Backtest should complete");

    println!("\n{}", report.summary());

    // With limited hourly data, grid may have few trades
    // This is expected - validates the pipeline works
    assert!(report.data_points == count);
}

/// Test DataFeed integration with SimulatedExchange.
#[tokio::test]
async fn test_datafeed_with_simulated_exchange() {
    use trader_exchange::simulated::{SimulatedConfig, SimulatedExchange};
    use trader_exchange::traits::Exchange;

    let csv_path = Path::new("../../data/test_btcusdt_1h.csv");
    if !csv_path.exists() {
        eprintln!("Skipping test: CSV file not found");
        return;
    }

    // Create simulated exchange with initial balance
    let config = SimulatedConfig::default()
        .with_initial_balance("USDT", dec!(100000))
        .with_initial_balance("BTC", dec!(1));

    let exchange = SimulatedExchange::new(config);
    let symbol = Symbol::crypto("BTC", "USDT");

    // Load historical data
    exchange.load_from_csv(symbol.clone(), Timeframe::H1, csv_path).await
        .expect("Failed to load CSV into exchange");

    // Get initial balance
    let usdt_balance = exchange.get_balance("USDT").await.unwrap();
    println!("Initial USDT balance: {}", usdt_balance.free);
    assert_eq!(usdt_balance.free, dec!(100000));

    // Step through some data
    for i in 0..10 {
        if let Some(kline) = exchange.step(&symbol, Timeframe::H1).await {
            println!("Step {}: close = {}, volume = {}", i, kline.close, kline.volume);
        } else {
            println!("Step {}: No more data", i);
            break;
        }
    }

    // Verify exchange can provide ticker
    if let Ok(ticker) = exchange.get_ticker(&symbol).await {
        println!("Current ticker: bid={}, ask={}, last={}", ticker.bid, ticker.ask, ticker.last);
    }

    // Verify exchange state
    let account = exchange.get_account().await.expect("Should get account");
    println!("Account balances: {} assets", account.balances.len());
}

/// Test Bollinger Bands Mean Reversion strategy.
///
/// Bollinger Bands adapts to volatility dynamically, making it ideal for crypto markets.
#[tokio::test]
async fn test_bollinger_bands_strategy() {
    use trader_strategy::strategies::BollingerStrategy;

    let csv_path = Path::new("../../data/btcusdt_1m_jan2024.csv");
    if !csv_path.exists() {
        eprintln!("Skipping test: CSV file not found");
        return;
    }

    // Load data
    let symbol = Symbol::crypto("BTC", "USDT");
    let mut feed = DataFeed::new(DataFeedConfig::default());
    let count = feed
        .load_from_csv(symbol.clone(), Timeframe::M1, csv_path)
        .expect("Failed to load CSV");

    println!("Loaded {} candles for Bollinger Bands test", count);

    let mut klines = Vec::new();
    while let Some(kline) = feed.next_kline(&symbol, Timeframe::M1) {
        klines.push(kline);
    }

    // Create Bollinger Bands strategy
    let mut strategy = BollingerStrategy::new();

    let bb_config = json!({
        "symbol": "BTC/USDT",
        "period": 20,                    // 20-period SMA
        "std_multiplier": 2.0,           // 2 standard deviations
        "rsi_period": 14,
        "rsi_oversold": 30.0,
        "rsi_overbought": 70.0,
        "use_rsi_confirmation": true,    // Require RSI confirmation
        "exit_at_middle_band": true,     // Exit at middle band
        "stop_loss_pct": 1.5,            // 1.5% stop loss
        "take_profit_pct": 3.0,          // 3% take profit
        "min_bandwidth_pct": 0.5         // Minimum 0.5% band width
    });

    strategy.initialize(bb_config).await
        .expect("Failed to initialize Bollinger strategy");

    // Run backtest with low fees
    let config = BacktestConfig::new(dec!(10_000_000))
        .with_commission_rate(dec!(0.0005))
        .with_slippage_rate(dec!(0.0001))
        .with_max_position_size_pct(dec!(0.1));

    let mut engine = BacktestEngine::new(config);

    println!("\n========================================");
    println!("  볼린저 밴드 평균회귀 백테스트 시작");
    println!("========================================\n");

    let report = engine.run(&mut strategy, &klines).await
        .expect("Backtest should complete");

    println!("\n{}", report.summary());

    let state = strategy.get_state();
    println!("\n볼린저 밴드 전략 상태:");
    println!("  - 현재 RSI: {:?}", state.get("rsi"));
    println!("  - 밴드 폭: {:?}", state.get("bandwidth_pct"));
    println!("  - 거래 수: {}", state.get("trades_count").unwrap_or(&json!(0)));

    assert!(report.data_points > 0);
    println!("\n=== 볼린저 밴드 전략 백테스트 완료 ===");
}

/// Test Volatility Breakout (Larry Williams) strategy.
///
/// This strategy captures strong momentum moves when price breaks out of a volatility range.
#[tokio::test]
async fn test_volatility_breakout_strategy() {
    use trader_strategy::strategies::VolatilityBreakoutStrategy;

    // Use hourly data for volatility breakout (better suited for longer timeframes)
    let csv_path = Path::new("../../data/test_btcusdt_1h.csv");
    if !csv_path.exists() {
        eprintln!("Skipping test: hourly CSV file not found");
        return;
    }

    // Load data
    let symbol = Symbol::crypto("BTC", "USDT");
    let mut feed = DataFeed::new(DataFeedConfig::default());
    let count = feed
        .load_from_csv(symbol.clone(), Timeframe::H1, csv_path)
        .expect("Failed to load CSV");

    println!("Loaded {} hourly candles for Volatility Breakout test", count);

    let mut klines = Vec::new();
    while let Some(kline) = feed.next_kline(&symbol, Timeframe::H1) {
        klines.push(kline);
    }

    // Create Volatility Breakout strategy
    let mut strategy = VolatilityBreakoutStrategy::new();

    let vb_config = json!({
        "symbol": "BTC/USDT",
        "k_factor": 0.6,                 // Breakout threshold (60% of range)
        "lookback_period": 1,            // Use previous period's range
        "use_atr": false,                // Use simple range
        "stop_loss_multiplier": 1.0,     // Stop at 1x range
        "take_profit_multiplier": 2.0,   // Target 2x range
        "trade_both_directions": true,   // Trade long and short
        "min_range_pct": 0.3,            // Minimum 0.3% range
        "max_range_pct": 15.0            // Maximum 15% range
    });

    strategy.initialize(vb_config).await
        .expect("Failed to initialize Volatility Breakout strategy");

    // Run backtest
    let config = BacktestConfig::new(dec!(10_000_000))
        .with_commission_rate(dec!(0.0005))
        .with_slippage_rate(dec!(0.0001))
        .with_max_position_size_pct(dec!(0.1));

    let mut engine = BacktestEngine::new(config);

    println!("\n========================================");
    println!("  변동성 돌파 전략 백테스트 시작");
    println!("========================================\n");

    let report = engine.run(&mut strategy, &klines).await
        .expect("Backtest should complete");

    println!("\n{}", report.summary());

    let state = strategy.get_state();
    println!("\n변동성 돌파 전략 상태:");
    println!("  - 현재 레인지: {:?}", state.get("current_range"));
    println!("  - 상단 돌파 레벨: {:?}", state.get("upper_breakout"));
    println!("  - 하단 돌파 레벨: {:?}", state.get("lower_breakout"));
    println!("  - 거래 수: {}", state.get("trades_count").unwrap_or(&json!(0)));

    assert!(report.data_points > 0);
    println!("\n=== 변동성 돌파 전략 백테스트 완료 ===");
}

/// Compare all strategies on the same dataset.
#[tokio::test]
async fn test_strategy_comparison() {
    use trader_strategy::strategies::{GridStrategy, BollingerStrategy};

    let csv_path = Path::new("../../data/btcusdt_1m_jan2024.csv");
    if !csv_path.exists() {
        eprintln!("Skipping comparison test: CSV file not found");
        return;
    }

    let symbol = Symbol::crypto("BTC", "USDT");
    let mut feed = DataFeed::new(DataFeedConfig::default());
    feed.load_from_csv(symbol.clone(), Timeframe::M1, csv_path)
        .expect("Failed to load CSV");

    let mut klines = Vec::new();
    while let Some(kline) = feed.next_kline(&symbol, Timeframe::M1) {
        klines.push(kline);
    }

    let initial_price = klines.first().map(|k| k.close).unwrap_or(dec!(42000));

    println!("\n════════════════════════════════════════════════════");
    println!("         전략 비교 테스트 (BTC/USDT 1분봉)");
    println!("════════════════════════════════════════════════════\n");

    // Common backtest config (low fees for fair comparison)
    let make_config = || {
        BacktestConfig::new(dec!(10_000_000))
            .with_commission_rate(dec!(0.0005))
            .with_slippage_rate(dec!(0.0001))
            .with_max_position_size_pct(dec!(0.05))
    };

    // Results storage
    let mut results: Vec<(&str, Decimal, Decimal, usize)> = Vec::new();

    // 1. Grid Trading
    {
        let mut strategy = GridStrategy::new();
        strategy.initialize(json!({
            "symbol": "BTC/USDT",
            "center_price": initial_price.to_string(),
            "grid_spacing_pct": 0.5,
            "grid_levels": 8,
            "amount_per_level": "50000",
            "dynamic_spacing": false,
            "trend_filter": false,
            "reset_threshold_pct": 3.0
        })).await.unwrap();

        let mut engine = BacktestEngine::new(make_config());
        let report = engine.run(&mut strategy, &klines).await.unwrap();

        results.push((
            "그리드 트레이딩",
            report.metrics.total_return_pct,
            report.metrics.win_rate_pct,
            report.metrics.total_trades,
        ));
        println!("1. 그리드 트레이딩: {:.2}% 수익률, {:.1}% 승률, {} 거래",
            report.metrics.total_return_pct,
            report.metrics.win_rate_pct,
            report.metrics.total_trades);
    }

    // 2. Bollinger Bands
    {
        let mut strategy = BollingerStrategy::new();
        strategy.initialize(json!({
            "symbol": "BTC/USDT",
            "period": 20,
            "std_multiplier": 2.0,
            "use_rsi_confirmation": true,
            "exit_at_middle_band": true,
            "stop_loss_pct": 1.5,
            "take_profit_pct": 3.0
        })).await.unwrap();

        let mut engine = BacktestEngine::new(make_config());
        let report = engine.run(&mut strategy, &klines).await.unwrap();

        results.push((
            "볼린저 밴드",
            report.metrics.total_return_pct,
            report.metrics.win_rate_pct,
            report.metrics.total_trades,
        ));
        println!("2. 볼린저 밴드: {:.2}% 수익률, {:.1}% 승률, {} 거래",
            report.metrics.total_return_pct,
            report.metrics.win_rate_pct,
            report.metrics.total_trades);
    }

    // 3. SMA Crossover (for comparison)
    {
        let mut strategy = SmaCrossoverStrategy::new(symbol.clone(), 10, 30);
        strategy.initialize(json!({})).await.unwrap();

        let mut engine = BacktestEngine::new(make_config());
        let report = engine.run(&mut strategy, &klines).await.unwrap();

        results.push((
            "SMA 크로스오버",
            report.metrics.total_return_pct,
            report.metrics.win_rate_pct,
            report.metrics.total_trades,
        ));
        println!("3. SMA 크로스오버: {:.2}% 수익률, {:.1}% 승률, {} 거래",
            report.metrics.total_return_pct,
            report.metrics.win_rate_pct,
            report.metrics.total_trades);
    }

    println!("\n════════════════════════════════════════════════════");
    println!("                    결과 요약");
    println!("════════════════════════════════════════════════════");

    // Find best strategy
    let best = results.iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .unwrap();

    println!("\n최고 수익률: {} ({:.2}%)", best.0, best.1);

    // Find highest win rate
    let highest_wr = results.iter()
        .max_by(|a, b| a.2.partial_cmp(&b.2).unwrap())
        .unwrap();

    println!("최고 승률: {} ({:.1}%)", highest_wr.0, highest_wr.2);

    println!("\n암호화폐 거래 권장 전략:");
    println!("  - 횡보장: 그리드 트레이딩, 볼린저 밴드");
    println!("  - 추세장: 변동성 돌파 (1시간봉 이상)");
    println!("  - 복합: 그리드 + 볼린저 조합");

    println!("\n=== 전략 비교 완료 ===");
}
