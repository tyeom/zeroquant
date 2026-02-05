//! RotationStrategy 통합 테스트.
//!
//! Strategy trait의 public API만 테스트합니다:
//! - initialize()
//! - on_market_data()
//! - on_position_update()
//! - on_order_filled()
//! - get_state()
//! - shutdown()

use chrono::{DateTime, TimeZone, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use trader_core::{
    Kline, MarketData, MarketDataType, Order, OrderStatusType, Position, Side, Timeframe,
};
use trader_strategy::strategies::rotation::{
    AssetInfo, MarketType, RankingMetric, RebalanceFrequency, RotationConfig, RotationStrategy,
    RotationVariant, WeightingMethod,
};
use trader_strategy::Strategy;
use uuid::Uuid;

// ============================================================================
// 테스트 헬퍼 함수
// ============================================================================

/// 테스트용 Kline 데이터 생성.
fn create_kline(ticker: &str, close: Decimal, timestamp: DateTime<Utc>) -> MarketData {
    MarketData {
        exchange: "test".to_string(),
        ticker: ticker.to_string(),
        timestamp,
        data: MarketDataType::Kline(Kline {
            ticker: ticker.to_string(),
            timeframe: Timeframe::D1,
            open_time: timestamp,
            close_time: timestamp,
            open: close - dec!(1),
            high: close + dec!(1),
            low: close - dec!(2),
            close,
            volume: dec!(10000),
            quote_volume: Some(close * dec!(10000)),
            num_trades: Some(100),
        }),
    }
}

/// 테스트용 Position 생성.
fn create_position(ticker: &str, side: Side, quantity: Decimal, entry_price: Decimal) -> Position {
    Position::new("test", ticker.to_string(), side, quantity, entry_price)
}

/// 테스트용 Order 생성.
fn create_order(ticker: &str, side: Side, quantity: Decimal, price: Decimal) -> Order {
    Order {
        id: Uuid::new_v4(),
        exchange: "test".to_string(),
        exchange_order_id: None,
        ticker: ticker.to_string(),
        side,
        order_type: trader_core::OrderType::Limit,
        quantity,
        price: Some(price),
        stop_price: None,
        status: OrderStatusType::Filled,
        filled_quantity: quantity,
        average_fill_price: Some(price),
        time_in_force: trader_core::TimeInForce::GTC,
        strategy_id: None,
        client_order_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: serde_json::Value::Null,
    }
}

/// 여러 날짜에 걸쳐 가격 데이터 생성 (상승 추세).
#[allow(dead_code)]
fn generate_price_history(
    ticker: &str,
    days: usize,
    start_price: Decimal,
    daily_return: Decimal,
) -> Vec<MarketData> {
    let base_time = Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap();

    (0..days)
        .map(|i| {
            // 단순 선형 증가 사용 (powi 대신)
            let price = start_price + start_price * daily_return * Decimal::from(i as u32);
            let timestamp = base_time + chrono::Duration::days(i as i64);
            create_kline(ticker, price, timestamp)
        })
        .collect()
}

// ============================================================================
// 섹터 모멘텀 테스트
// ============================================================================

mod sector_momentum_tests {
    use super::*;

    #[tokio::test]
    async fn test_initialization() {
        let mut strategy = RotationStrategy::new();

        let config = json!({
            "variant": "SectorMomentum",
            "market": "US",
            "top_n": 3,
            "universe": [
                {"ticker": "XLK", "name": "Technology"},
                {"ticker": "XLF", "name": "Financials"},
                {"ticker": "XLV", "name": "Healthcare"}
            ]
        });

        let result = strategy.initialize(config).await;
        assert!(result.is_ok());

        let state = strategy.get_state();
        assert!(state["initialized"].as_bool().unwrap());
        assert_eq!(state["variant"].as_str().unwrap(), "SectorMomentum");
    }

    #[tokio::test]
    async fn test_factory_method() {
        let strategy = RotationStrategy::sector_momentum();
        assert_eq!(strategy.name(), "Sector Momentum");
        assert_eq!(strategy.version(), "2.0.0");
    }

    #[tokio::test]
    async fn test_us_sector_universe() {
        let config = RotationConfig::sector_momentum_default();
        assert_eq!(config.universe.len(), 11); // US 섹터 11개
        assert_eq!(config.top_n, 3);
    }

    #[tokio::test]
    async fn test_kr_sector_universe() {
        let config = RotationConfig::sector_momentum_kr();
        assert_eq!(config.market, MarketType::KR);
        assert_eq!(config.universe.len(), 10); // KR 섹터 10개
    }

    #[tokio::test]
    async fn test_on_market_data_collects_prices() {
        let mut strategy = RotationStrategy::new();

        let config = json!({
            "variant": "SectorMomentum",
            "market": "US",
            "top_n": 2,
            "universe": [
                {"ticker": "XLK", "name": "Technology"},
                {"ticker": "XLF", "name": "Financials"}
            ]
        });

        strategy.initialize(config).await.unwrap();

        // 가격 데이터 전송
        let data = create_kline("XLK", dec!(150), Utc::now());
        let signals = strategy.on_market_data(&data).await.unwrap();

        // 첫 번째 데이터는 신호 없음 (모든 자산 데이터 필요)
        assert!(signals.is_empty());
    }
}

// ============================================================================
// 종목 로테이션 테스트
// ============================================================================

mod stock_rotation_tests {
    use super::*;

    #[tokio::test]
    async fn test_initialization() {
        let mut strategy = RotationStrategy::new();

        let config = json!({
            "variant": "StockMomentum",
            "market": "US",
            "top_n": 5,
            "universe": [
                {"ticker": "AAPL", "name": "Apple"},
                {"ticker": "MSFT", "name": "Microsoft"},
                {"ticker": "GOOGL", "name": "Alphabet"},
                {"ticker": "AMZN", "name": "Amazon"},
                {"ticker": "NVDA", "name": "NVIDIA"}
            ]
        });

        let result = strategy.initialize(config).await;
        assert!(result.is_ok());

        let state = strategy.get_state();
        assert_eq!(state["variant"].as_str().unwrap(), "StockMomentum");
    }

    #[tokio::test]
    async fn test_factory_method() {
        let strategy = RotationStrategy::stock_rotation();
        assert_eq!(strategy.name(), "Stock Rotation");
    }

    #[tokio::test]
    async fn test_stock_rotation_default_config() {
        let config = RotationConfig::stock_rotation_default();

        assert_eq!(config.variant, RotationVariant::StockMomentum);
        assert_eq!(config.top_n, 5);

        // AverageMomentum 랭킹 메트릭
        match config.ranking_metric {
            RankingMetric::AverageMomentum { periods } => {
                assert_eq!(periods.len(), 4); // 1M, 3M, 6M, 12M
            }
            _ => panic!("Expected AverageMomentum ranking metric"),
        }
    }

    #[tokio::test]
    async fn test_kr_stock_rotation() {
        let strategy = RotationStrategy::stock_rotation_kr();
        let state = strategy.get_state();
        assert_eq!(state["market"].as_str().unwrap(), "KR");
    }
}

// ============================================================================
// 시가총액 상위 테스트
// ============================================================================

mod market_cap_top_tests {
    use super::*;

    #[tokio::test]
    async fn test_initialization() {
        let mut strategy = RotationStrategy::new();

        let config = json!({
            "variant": "MarketCapTop",
            "market": "US",
            "top_n": 10,
            "universe": [
                {"ticker": "AAPL", "name": "Apple"},
                {"ticker": "MSFT", "name": "Microsoft"}
            ]
        });

        let result = strategy.initialize(config).await;
        assert!(result.is_ok());

        let state = strategy.get_state();
        assert_eq!(state["variant"].as_str().unwrap(), "MarketCapTop");
    }

    #[tokio::test]
    async fn test_factory_method() {
        let strategy = RotationStrategy::market_cap_top();
        assert_eq!(strategy.name(), "Market Cap Top");
    }

    #[tokio::test]
    async fn test_market_cap_top_default_config() {
        let config = RotationConfig::market_cap_top_default();

        assert_eq!(config.variant, RotationVariant::MarketCapTop);
        assert_eq!(config.top_n, 10);

        // Days-based rebalancing
        match config.rebalance_frequency {
            RebalanceFrequency::Days(days) => {
                assert_eq!(days, 30);
            }
            _ => panic!("Expected Days rebalancing"),
        }
    }
}

// ============================================================================
// 공통 기능 테스트
// ============================================================================

mod common_tests {
    use super::*;

    #[tokio::test]
    async fn test_on_position_update() {
        let mut strategy = RotationStrategy::new();

        let config = json!({
            "variant": "SectorMomentum",
            "market": "US",
            "top_n": 3,
            "universe": [
                {"ticker": "XLK", "name": "Technology"}
            ]
        });

        strategy.initialize(config).await.unwrap();

        // 포지션 업데이트
        let position = create_position("XLK", Side::Buy, dec!(100), dec!(150));
        let result = strategy.on_position_update(&position).await;

        assert!(result.is_ok());

        let state = strategy.get_state();
        assert_eq!(state["holdings_count"].as_u64().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_on_order_filled() {
        let mut strategy = RotationStrategy::new();

        let config = json!({
            "variant": "SectorMomentum",
            "market": "US",
            "top_n": 3,
            "universe": [
                {"ticker": "XLK", "name": "Technology"}
            ]
        });

        strategy.initialize(config).await.unwrap();

        // 주문 체결
        let order = create_order("XLK", Side::Buy, dec!(100), dec!(150));
        let result = strategy.on_order_filled(&order).await;

        assert!(result.is_ok());

        let state = strategy.get_state();
        assert_eq!(state["trades_count"].as_u64().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_shutdown() {
        let mut strategy = RotationStrategy::new();

        let config = json!({
            "variant": "SectorMomentum",
            "market": "US",
            "top_n": 3,
            "universe": [
                {"ticker": "XLK", "name": "Technology"}
            ]
        });

        strategy.initialize(config).await.unwrap();

        let result = strategy.shutdown().await;
        assert!(result.is_ok());

        let state = strategy.get_state();
        assert!(!state["initialized"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_get_state_structure() {
        let strategy = RotationStrategy::sector_momentum();
        let state = strategy.get_state();

        // 필수 필드 확인
        assert!(state.get("variant").is_some());
        assert!(state.get("market").is_some());
        assert!(state.get("initialized").is_some());
        assert!(state.get("holdings_count").is_some());
        assert!(state.get("current_holdings").is_some());
        assert!(state.get("trades_count").is_some());
        assert!(state.get("cash_balance").is_some());
    }
}

// ============================================================================
// 설정 테스트
// ============================================================================

mod config_tests {
    use super::*;

    #[test]
    fn test_market_type_quote_currency() {
        assert_eq!(MarketType::US.quote_currency(), "USD");
        assert_eq!(MarketType::KR.quote_currency(), "KRW");
    }

    #[test]
    fn test_ranking_metric_default() {
        let metric = RankingMetric::default();

        match metric {
            RankingMetric::MultiPeriodMomentum {
                short_period,
                medium_period,
                long_period,
                ..
            } => {
                assert_eq!(short_period, 20);
                assert_eq!(medium_period, 60);
                assert_eq!(long_period, 120);
            }
            _ => panic!("Expected MultiPeriodMomentum"),
        }
    }

    #[test]
    fn test_weighting_method_default() {
        let method = WeightingMethod::default();
        assert_eq!(method, WeightingMethod::Equal);
    }

    #[test]
    fn test_rebalance_frequency_default() {
        let freq = RebalanceFrequency::default();

        match freq {
            RebalanceFrequency::Monthly => (),
            _ => panic!("Expected Monthly"),
        }
    }

    #[test]
    fn test_config_all_tickers() {
        let config = RotationConfig::sector_momentum_default();
        let tickers = config.all_tickers();

        assert!(tickers.contains(&"XLK".to_string()));
        assert!(tickers.contains(&"XLF".to_string()));
        assert_eq!(tickers.len(), 11);
    }

    #[test]
    fn test_custom_config() {
        let config = RotationConfig {
            variant: RotationVariant::StockMomentum,
            market: MarketType::US,
            universe: vec![
                AssetInfo::new("AAPL", "Apple"),
                AssetInfo::new("MSFT", "Microsoft"),
            ],
            top_n: 2,
            total_amount: dec!(100000),
            ranking_metric: RankingMetric::SinglePeriodMomentum { period: 60 },
            weighting_method: WeightingMethod::MomentumProportional,
            rebalance_frequency: RebalanceFrequency::Days(7),
            rebalance_threshold: dec!(3),
            min_momentum: Some(dec!(0.01)),
            cash_reserve_rate: dec!(0.1),
            use_momentum_filter: true,
            min_global_score: dec!(55),
        };

        assert_eq!(config.top_n, 2);
        assert_eq!(config.total_amount, dec!(100000));
        assert_eq!(config.cash_reserve_rate, dec!(0.1));
        assert_eq!(config.min_global_score, dec!(55));
    }
}

// ============================================================================
// 에러 케이스 테스트
// ============================================================================

mod error_tests {
    use super::*;

    #[tokio::test]
    async fn test_invalid_config() {
        let mut strategy = RotationStrategy::new();

        // 완전히 잘못된 JSON 타입 (객체가 아닌 문자열)
        let config = json!("not an object");

        let result = strategy.initialize(config).await;
        assert!(result.is_err(), "잘못된 JSON 타입은 에러를 반환해야 함");
    }

    /// 잘못된 variant 값은 기본값(SectorMomentum)으로 처리됨.
    ///
    /// 현재 전략 설계: 인식되지 않는 variant → 기본값 사용 (실패하지 않음)
    #[tokio::test]
    async fn test_unknown_variant_uses_default() {
        let mut strategy = RotationStrategy::new();

        let config = json!({
            "variant": "InvalidVariant",
            "market": "US"
        });

        // 잘못된 variant는 기본값(SectorMomentum)으로 처리됨
        let result = strategy.initialize(config).await;
        assert!(result.is_ok(), "인식되지 않는 variant는 기본값으로 처리됨");

        let state = strategy.get_state();
        assert_eq!(state["variant"].as_str().unwrap(), "SectorMomentum");
    }

    #[tokio::test]
    async fn test_on_market_data_before_init() {
        let mut strategy = RotationStrategy::new();

        let data = create_kline("XLK", dec!(150), Utc::now());
        let signals = strategy.on_market_data(&data).await.unwrap();

        // 초기화 전에는 신호 없음
        assert!(signals.is_empty());
    }

    #[tokio::test]
    async fn test_unknown_ticker_ignored() {
        let mut strategy = RotationStrategy::new();

        let config = json!({
            "variant": "SectorMomentum",
            "market": "US",
            "top_n": 2,
            "universe": [
                {"ticker": "XLK", "name": "Technology"}
            ]
        });

        strategy.initialize(config).await.unwrap();

        // 유니버스에 없는 티커
        let data = create_kline("UNKNOWN", dec!(100), Utc::now());
        let signals = strategy.on_market_data(&data).await.unwrap();

        assert!(signals.is_empty());
    }
}

// ============================================================================
// 비중 배분 방식 테스트
// ============================================================================

mod weighting_tests {
    use super::*;

    #[tokio::test]
    async fn test_equal_weighting_config() {
        let config = RotationConfig {
            weighting_method: WeightingMethod::Equal,
            ..RotationConfig::sector_momentum_default()
        };

        assert_eq!(config.weighting_method, WeightingMethod::Equal);
    }

    #[tokio::test]
    async fn test_momentum_proportional_weighting_config() {
        let config = RotationConfig {
            weighting_method: WeightingMethod::MomentumProportional,
            ..RotationConfig::sector_momentum_default()
        };

        assert_eq!(
            config.weighting_method,
            WeightingMethod::MomentumProportional
        );
    }

    #[tokio::test]
    async fn test_inverse_volatility_weighting_config() {
        let config = RotationConfig {
            weighting_method: WeightingMethod::InverseVolatility,
            ..RotationConfig::market_cap_top_default()
        };

        assert_eq!(config.weighting_method, WeightingMethod::InverseVolatility);
    }
}

// ============================================================================
// 경계값 테스트
// ============================================================================

mod boundary_tests {
    use super::*;

    #[tokio::test]
    async fn test_top_n_equals_universe_size() {
        let mut strategy = RotationStrategy::new();

        let config = json!({
            "variant": "SectorMomentum",
            "market": "US",
            "top_n": 3,  // 유니버스 크기와 동일
            "universe": [
                {"ticker": "XLK", "name": "Technology"},
                {"ticker": "XLF", "name": "Financials"},
                {"ticker": "XLV", "name": "Healthcare"}
            ]
        });

        let result = strategy.initialize(config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_top_n_greater_than_universe() {
        let mut strategy = RotationStrategy::new();

        // top_n이 유니버스보다 큼
        let config = json!({
            "variant": "SectorMomentum",
            "market": "US",
            "top_n": 10,
            "universe": [
                {"ticker": "XLK", "name": "Technology"},
                {"ticker": "XLF", "name": "Financials"}
            ]
        });

        let result = strategy.initialize(config).await;
        // 정상 초기화 (실제 선택은 유니버스 크기로 제한됨)
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cash_reserve_rate_full() {
        let config = RotationConfig {
            cash_reserve_rate: dec!(1.0), // 100% 현금
            ..RotationConfig::sector_momentum_default()
        };

        assert_eq!(config.cash_reserve_rate, dec!(1.0));
    }

    #[tokio::test]
    async fn test_min_momentum_filter() {
        let config = RotationConfig {
            min_momentum: Some(dec!(0.05)), // 5% 최소 모멘텀
            ..RotationConfig::stock_rotation_default()
        };

        assert_eq!(config.min_momentum, Some(dec!(0.05)));
    }

    #[tokio::test]
    async fn test_empty_universe() {
        let mut strategy = RotationStrategy::new();

        let config = json!({
            "variant": "SectorMomentum",
            "market": "US",
            "top_n": 3,
            "universe": []
        });

        let result = strategy.initialize(config).await;
        assert!(result.is_ok());

        // 빈 유니버스에서 시장 데이터 처리
        let data = create_kline("XLK", dec!(150), Utc::now());
        let signals = strategy.on_market_data(&data).await.unwrap();
        assert!(signals.is_empty());
    }

    #[test]
    fn test_zero_total_amount() {
        let config = RotationConfig {
            total_amount: dec!(0),
            ..RotationConfig::sector_momentum_default()
        };

        assert_eq!(config.total_amount, dec!(0));
    }
}

// ============================================================================
// 리밸런싱 빈도 테스트
// ============================================================================

mod rebalance_frequency_tests {
    use super::*;

    #[test]
    fn test_monthly_rebalancing() {
        let config = RotationConfig {
            rebalance_frequency: RebalanceFrequency::Monthly,
            ..RotationConfig::sector_momentum_default()
        };

        match config.rebalance_frequency {
            RebalanceFrequency::Monthly => (),
            _ => panic!("Expected Monthly"),
        }
    }

    #[test]
    fn test_weekly_rebalancing() {
        let config = RotationConfig {
            rebalance_frequency: RebalanceFrequency::Days(7),
            ..RotationConfig::sector_momentum_default()
        };

        match config.rebalance_frequency {
            RebalanceFrequency::Days(7) => (),
            _ => panic!("Expected Days(7)"),
        }
    }

    #[test]
    fn test_daily_rebalancing() {
        let config = RotationConfig {
            rebalance_frequency: RebalanceFrequency::Days(1),
            ..RotationConfig::sector_momentum_default()
        };

        match config.rebalance_frequency {
            RebalanceFrequency::Days(1) => (),
            _ => panic!("Expected Days(1)"),
        }
    }
}

// ============================================================================
// 변형별 이름 테스트
// ============================================================================

mod name_tests {
    use super::*;

    #[test]
    fn test_sector_momentum_name() {
        let strategy = RotationStrategy::sector_momentum();
        assert_eq!(strategy.name(), "Sector Momentum");
    }

    #[test]
    fn test_stock_rotation_name() {
        let strategy = RotationStrategy::stock_rotation();
        assert_eq!(strategy.name(), "Stock Rotation");
    }

    #[test]
    fn test_market_cap_top_name() {
        let strategy = RotationStrategy::market_cap_top();
        assert_eq!(strategy.name(), "Market Cap Top");
    }

    #[test]
    fn test_default_name() {
        let strategy = RotationStrategy::new();
        assert_eq!(strategy.name(), "Rotation");
    }
}

// ============================================================================
// 신호 생성 검증 테스트 (핵심)
// ============================================================================

mod signal_generation_tests {
    use super::*;

    /// 짧은 모멘텀 기간의 테스트 설정 생성.
    /// 기본 설정(20, 60, 120, 240일)은 너무 많은 데이터가 필요하므로
    /// 5일 모멘텀으로 테스트 (6일 데이터면 충분)
    fn simple_test_config() -> serde_json::Value {
        json!({
            "variant": "SectorMomentum",
            "market": "US",
            "top_n": 2,
            "universe": [
                {"ticker": "XLK", "name": "Technology"},
                {"ticker": "XLF", "name": "Financials"},
                {"ticker": "XLV", "name": "Healthcare"}
            ],
            "ranking_metric": {
                "AverageMomentum": { "periods": [5] }  // 5일 모멘텀
            },
            "rebalance_frequency": "Monthly",
            "total_amount": "100000",
            "cash_reserve_rate": "0"
        })
    }

    /// 특정 연월에 상승 추세 가격 데이터 입력.
    async fn feed_rising_prices_in_month(
        strategy: &mut RotationStrategy,
        ticker: &str,
        days: usize,
        base_price: Decimal,
        year: i32,
        month: u32,
    ) {
        for day in 1..=days {
            let day_num = (day as u32).min(28);
            let price = base_price + Decimal::from(day as i32 * 2);
            let timestamp = Utc
                .with_ymd_and_hms(year, month, day_num, 12, 0, 0)
                .unwrap();
            let data = create_kline(ticker, price, timestamp);
            let _ = strategy.on_market_data(&data).await;
        }
    }

    /// 테스트 1: 신호가 실제로 생성되는지 확인
    ///
    /// 핵심 로직:
    /// 1. 2025년 12월에 모든 ticker 데이터 입력 → `last_rebalance = "2025_12"` 설정
    /// 2. 2026년 1월 데이터로 리밸런싱 트리거 → 월이 바뀌었으므로 리밸런싱 실행
    /// 3. 모든 ticker에 데이터가 있으므로 모멘텀 계산 가능
    #[tokio::test]
    async fn signals_are_actually_generated() {
        let mut strategy = RotationStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let tickers = vec!["XLK", "XLF", "XLV"];
        let mut all_signals = vec![];

        // 1단계: 2025년 12월에 모든 ticker에 충분한 데이터 입력
        // 5일 모멘텀용으로 10일 입력 (margin 포함)
        for ticker in &tickers {
            feed_rising_prices_in_month(&mut strategy, ticker, 10, dec!(100), 2025, 12).await;
        }

        // 2단계: 2026년 1월 데이터로 리밸런싱 트리거
        // 월이 바뀌었으므로 should_rebalance() = true
        let jan_timestamp = Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap();
        for ticker in &tickers {
            let data = create_kline(ticker, dec!(150), jan_timestamp);
            let signals = strategy.on_market_data(&data).await.unwrap();
            all_signals.extend(signals);
        }

        // 핵심 검증: 신호가 실제로 생성되어야 함
        assert!(
            !all_signals.is_empty(),
            "리밸런싱 시 신호가 생성되어야 함. \
            원인 분석: \
            1) 모든 ticker에 6일+ 데이터 필요 (5일 모멘텀) \
            2) 월이 바뀌어야 should_rebalance() = true \
            3) all_have_data = true 조건 충족 필요"
        );

        // 신호 구조 검증
        for signal in &all_signals {
            assert!(
                signal.side == Side::Buy || signal.side == Side::Sell,
                "신호는 Buy 또는 Sell이어야 함"
            );
        }
    }

    /// 테스트 2: 생성된 신호의 ticker가 유니버스에 있는지 확인
    #[tokio::test]
    async fn signal_tickers_are_from_universe() {
        let mut strategy = RotationStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let tickers = vec!["XLK", "XLF", "XLV"];
        let mut all_signals = vec![];

        // 1단계: 2025년 12월에 데이터 입력
        for ticker in &tickers {
            feed_rising_prices_in_month(&mut strategy, ticker, 10, dec!(100), 2025, 12).await;
        }

        // 2단계: 2026년 1월 데이터로 리밸런싱 트리거
        let jan_timestamp = Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap();
        for ticker in &tickers {
            let data = create_kline(ticker, dec!(150), jan_timestamp);
            let signals = strategy.on_market_data(&data).await.unwrap();
            all_signals.extend(signals);
        }

        // 선행 조건: 신호가 먼저 생성되어야 함
        assert!(
            !all_signals.is_empty(),
            "테스트 전제 조건 실패: 신호가 생성되어야 함"
        );

        // 신호의 ticker가 유니버스에 있는지 확인
        // ticker는 quote currency 없이 그대로 사용됨
        for signal in &all_signals {
            assert!(
                tickers.contains(&signal.ticker.as_str()),
                "신호 ticker({})가 유니버스에 없음. 유니버스: {:?}",
                signal.ticker,
                tickers
            );
        }
    }

    /// 테스트 3: top_n에 따라 선택된 자산 수 확인
    #[tokio::test]
    async fn top_n_limits_signal_count() {
        let mut strategy = RotationStrategy::new();
        // top_n = 2이고 유니버스 = 3이므로 최대 2개 자산 선택
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let tickers = vec!["XLK", "XLF", "XLV"];
        let mut all_signals = vec![];

        // 데이터 입력 (각 자산에 다른 모멘텀 유도)
        // XLK: 가장 높은 모멘텀 (큰 폭 상승)
        feed_rising_prices_in_month(&mut strategy, "XLK", 10, dec!(100), 2025, 12).await;
        // XLF: 중간 모멘텀
        feed_rising_prices_in_month(&mut strategy, "XLF", 10, dec!(100), 2025, 12).await;
        // XLV: 가장 낮은 모멘텀 (작은 폭 상승)
        feed_rising_prices_in_month(&mut strategy, "XLV", 10, dec!(100), 2025, 12).await;

        // 리밸런싱 트리거
        let jan_timestamp = Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap();
        for ticker in &tickers {
            let data = create_kline(ticker, dec!(150), jan_timestamp);
            let signals = strategy.on_market_data(&data).await.unwrap();
            all_signals.extend(signals);
        }

        // 고유 ticker 수 계산
        let unique_tickers: std::collections::HashSet<_> =
            all_signals.iter().map(|s| &s.ticker).collect();

        // top_n = 2이므로 최대 2개 자산
        assert!(
            unique_tickers.len() <= 2,
            "top_n=2인데 {}개 자산에 대한 신호 생성됨: {:?}",
            unique_tickers.len(),
            unique_tickers
        );
    }

    /// 테스트 4: 같은 월에는 리밸런싱 스킵
    #[tokio::test]
    async fn same_month_no_additional_signals() {
        let mut strategy = RotationStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let tickers = vec!["XLK", "XLF", "XLV"];

        // 1단계: 2025년 12월에 데이터 입력
        for ticker in &tickers {
            feed_rising_prices_in_month(&mut strategy, ticker, 10, dec!(100), 2025, 12).await;
        }

        // 2단계: 2026년 1월 첫 번째 리밸런싱
        let jan_timestamp = Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap();
        let mut first_rebalance_signals = vec![];
        for ticker in &tickers {
            let data = create_kline(ticker, dec!(150), jan_timestamp);
            let signals = strategy.on_market_data(&data).await.unwrap();
            first_rebalance_signals.extend(signals);
        }

        // 3단계: 같은 월 (1월) 두 번째 시도 → 신호 없어야 함
        let jan_timestamp_2 = Utc.with_ymd_and_hms(2026, 1, 20, 12, 0, 0).unwrap();
        let mut second_attempt_signals = vec![];
        for ticker in &tickers {
            let data = create_kline(ticker, dec!(155), jan_timestamp_2);
            let signals = strategy.on_market_data(&data).await.unwrap();
            second_attempt_signals.extend(signals);
        }

        // 첫 번째 리밸런싱에서는 신호 생성
        assert!(
            !first_rebalance_signals.is_empty(),
            "첫 번째 리밸런싱에서 신호가 생성되어야 함"
        );

        // 같은 월 두 번째 시도에서는 신호 없음 (이미 리밸런싱 완료)
        assert!(
            second_attempt_signals.is_empty(),
            "같은 월에는 추가 리밸런싱 없어야 함. 생성된 신호: {}개",
            second_attempt_signals.len()
        );
    }

    /// 테스트 5: 다음 월에는 새 리밸런싱
    #[tokio::test]
    async fn next_month_triggers_new_rebalance() {
        let mut strategy = RotationStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let tickers = vec!["XLK", "XLF", "XLV"];

        // 1단계: 2025년 12월에 데이터 입력
        for ticker in &tickers {
            feed_rising_prices_in_month(&mut strategy, ticker, 10, dec!(100), 2025, 12).await;
        }

        // 2단계: 2026년 1월 첫 번째 리밸런싱
        let jan_timestamp = Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap();
        for ticker in &tickers {
            let data = create_kline(ticker, dec!(150), jan_timestamp);
            let _ = strategy.on_market_data(&data).await;
        }

        // 3단계: 2026년 2월 두 번째 리밸런싱 → 신호 생성되어야 함
        let feb_timestamp = Utc.with_ymd_and_hms(2026, 2, 15, 12, 0, 0).unwrap();
        let mut feb_signals = vec![];
        for ticker in &tickers {
            let data = create_kline(ticker, dec!(160), feb_timestamp);
            let signals = strategy.on_market_data(&data).await.unwrap();
            feb_signals.extend(signals);
        }

        // 다음 월에는 새 리밸런싱
        assert!(
            !feb_signals.is_empty(),
            "다음 월(2월)에는 새 리밸런싱이 트리거되어야 함"
        );
    }
}
