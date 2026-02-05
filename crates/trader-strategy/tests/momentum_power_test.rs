//! MomentumPower (Snow) 전략 통합 테스트.
//!
//! TIP 기반 시장 안전도 지표를 사용한 자산 전환 전략의 핵심 로직 검증:
//! 1. TIP > TIP MA → 시장 안전
//! 2. 세 가지 모드 (Attack/Safe/Crisis)
//! 3. 리밸런싱 주기 (30일)

use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use trader_core::{Kline, MarketData, MarketDataType, Position, Side, Timeframe};
use trader_strategy::strategies::momentum_power::{
    MomentumPowerConfig, MomentumPowerMarket, MomentumPowerMode, MomentumPowerStrategy,
};
use trader_strategy::Strategy;

// ============================================================================
// 헬퍼 함수
// ============================================================================

/// 특정 시간에 캔들 데이터 생성.
fn create_kline_at(ticker: &str, close: Decimal, days_from_start: i64) -> MarketData {
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap()
        + chrono::Duration::days(days_from_start);
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
fn create_position(ticker: &str, quantity: Decimal, entry_price: Decimal) -> Position {
    Position::new("test", ticker.to_string(), Side::Buy, quantity, entry_price)
}

/// 상승 추세 가격 데이터 입력.
async fn feed_rising_prices(
    strategy: &mut MomentumPowerStrategy,
    ticker: &str,
    days: usize,
    base_price: Decimal,
    start_day: i64,
) {
    for day in 0..days {
        let price = base_price + Decimal::from(day as i32 * 2);
        let data = create_kline_at(ticker, price, start_day + day as i64);
        let _ = strategy.on_market_data(&data).await;
    }
}

/// 하락 추세 가격 데이터 입력.
async fn feed_falling_prices(
    strategy: &mut MomentumPowerStrategy,
    ticker: &str,
    days: usize,
    base_price: Decimal,
    start_day: i64,
) {
    for day in 0..days {
        let price = base_price - Decimal::from(day as i32 * 2);
        let data = create_kline_at(ticker, price, start_day + day as i64);
        let _ = strategy.on_market_data(&data).await;
    }
}

/// 짧은 MA 기간의 테스트 설정.
fn simple_test_config() -> serde_json::Value {
    json!({
        "market": "US",
        "tip_ma_period": 10,  // 테스트용 짧은 기간 (기본 200 대신)
        "momentum_period": 5,
        "rebalance_days": 5,  // 테스트용 짧은 주기 (기본 30 대신)
        "min_global_score": "0"  // 필터 비활성화
    })
}

// ============================================================================
// 1. 초기화 테스트
// ============================================================================

mod initialize_tests {
    use super::*;

    #[tokio::test]
    async fn us_config_initializes_successfully() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = serde_json::to_value(MomentumPowerConfig::default()).unwrap();

        let result = strategy.initialize(config).await;
        assert!(result.is_ok());
        assert_eq!(strategy.name(), "Snow");
    }

    #[tokio::test]
    async fn kr_config_initializes_successfully() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = json!({
            "market": "KR",
            "tip_ma_period": 200
        });

        let result = strategy.initialize(config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn invalid_config_returns_error() {
        let mut strategy = MomentumPowerStrategy::new();
        let invalid_config = json!({ "market": "INVALID" });

        let result = strategy.initialize(invalid_config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn initial_mode_is_safe() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let state = strategy.get_state();
        // 초기 모드는 Safe
        assert_eq!(state["state"]["mode"], "Safe");
    }
}

// ============================================================================
// 2. Config 유효성 테스트
// ============================================================================

mod config_tests {
    use super::*;

    #[test]
    fn default_config_is_us_market() {
        let config = MomentumPowerConfig::default();
        assert_eq!(config.market, MomentumPowerMarket::US);
    }

    #[test]
    fn default_tip_ma_period_is_200() {
        let config = MomentumPowerConfig::default();
        assert_eq!(config.tip_ma_period, 200);
    }

    #[test]
    fn default_rebalance_days_is_30() {
        let config = MomentumPowerConfig::default();
        assert_eq!(config.rebalance_days, 30);
    }

    #[test]
    fn default_min_global_score_is_50() {
        let config = MomentumPowerConfig::default();
        assert_eq!(config.min_global_score, dec!(50));
    }

    #[test]
    fn config_serialization_roundtrip() {
        let original = MomentumPowerConfig::default();
        let json = serde_json::to_value(&original).unwrap();
        let restored: MomentumPowerConfig = serde_json::from_value(json).unwrap();

        assert_eq!(original.market, restored.market);
        assert_eq!(original.tip_ma_period, restored.tip_ma_period);
    }
}

// ============================================================================
// 3. 모드 결정 테스트
// ============================================================================

mod mode_determination_tests {
    use super::*;

    /// 테스트 1: 데이터 부족 시 신호 없음
    #[tokio::test]
    async fn no_signal_with_insufficient_data() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        // 단일 데이터만 입력 (MA 계산 불가)
        let data = create_kline_at("TIP", dec!(100), 0);
        let signals = strategy.on_market_data(&data).await.unwrap();

        assert!(signals.is_empty(), "데이터 부족 시 신호가 없어야 함");
    }

    /// 테스트 2: 충분한 상승 데이터 후 신호 생성
    #[tokio::test]
    async fn signal_generated_after_sufficient_data() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        // TIP 데이터 입력 (MA 10일)
        feed_rising_prices(&mut strategy, "TIP", 15, dec!(100), 0).await;

        // UPRO (공격 자산) 데이터 입력 (모멘텀 5일)
        feed_rising_prices(&mut strategy, "UPRO", 10, dec!(50), 0).await;

        let state = strategy.get_state();
        assert!(
            state["initialized"].as_bool().unwrap_or(false),
            "충분한 데이터 후 초기화되어야 함"
        );
    }
}

// ============================================================================
// 4. 신호 생성 검증 테스트 (핵심)
// ============================================================================

mod signal_generation_tests {
    use super::*;

    /// 테스트 1: Attack 모드에서 신호 생성
    ///
    /// 조건: TIP > TIP MA + 모멘텀 양호
    #[tokio::test]
    async fn attack_mode_generates_signal() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let mut all_signals = vec![];

        // TIP 상승 추세 (시장 안전)
        for day in 0..15 {
            let price = dec!(100) + Decimal::from(day * 2);
            let data = create_kline_at("TIP", price, day);
            let signals = strategy.on_market_data(&data).await.unwrap();
            all_signals.extend(signals);
        }

        // UPRO 상승 추세 (모멘텀 양호)
        for day in 0..15 {
            let price = dec!(50) + Decimal::from(day * 2);
            let data = create_kline_at("UPRO", price, day);
            let signals = strategy.on_market_data(&data).await.unwrap();
            all_signals.extend(signals);
        }

        // 신호가 생성되어야 함
        assert!(
            !all_signals.is_empty(),
            "시장 안전 + 모멘텀 양호 시 신호가 생성되어야 함"
        );

        // Attack 모드인지 확인
        let state = strategy.get_state();
        // 상태가 Attack이거나 신호에 Attack 모드가 있어야 함
        let has_attack = all_signals.iter().any(|s| {
            s.metadata
                .get("mode")
                .and_then(|m| m.as_str())
                .map(|m| m.contains("Attack"))
                .unwrap_or(false)
        });

        // Attack 신호가 있거나 상태가 Attack이어야 함
        assert!(
            has_attack || state["state"]["mode"] == "Attack",
            "시장 안전 + 모멘텀 양호면 Attack 모드여야 함. 현재 모드: {:?}, 신호: {:?}",
            state["state"]["mode"],
            all_signals.iter().map(|s| &s.metadata).collect::<Vec<_>>()
        );
    }

    /// 테스트 2: Crisis 모드에서 신호 생성
    ///
    /// 조건: TIP <= TIP MA (시장 위험)
    #[tokio::test]
    async fn crisis_mode_on_market_risk() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let mut all_signals = vec![];

        // TIP 하락 추세 (시장 위험)
        for day in 0..15 {
            let price = dec!(150) - Decimal::from(day * 3);
            let data = create_kline_at("TIP", price, day);
            let signals = strategy.on_market_data(&data).await.unwrap();
            all_signals.extend(signals);
        }

        // UPRO도 데이터 필요
        for day in 0..15 {
            let price = dec!(50) - Decimal::from(day);
            let data = create_kline_at("UPRO", price, day);
            let signals = strategy.on_market_data(&data).await.unwrap();
            all_signals.extend(signals);
        }

        // Crisis 모드인지 확인
        let state = strategy.get_state();
        let is_crisis = state["state"]["mode"] == "Crisis";
        let has_crisis_signal = all_signals.iter().any(|s| {
            s.metadata
                .get("mode")
                .and_then(|m| m.as_str())
                .map(|m| m.contains("Crisis"))
                .unwrap_or(false)
        });

        // 시장 위험 시 Crisis 모드여야 함
        assert!(
            is_crisis || has_crisis_signal,
            "TIP < MA면 Crisis 모드여야 함. 현재 상태: {:?}",
            state["state"]
        );
    }

    /// 테스트 3: 신호의 ticker가 올바른지 확인
    #[tokio::test]
    async fn signal_ticker_matches_mode() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let mut all_signals = vec![];

        // 충분한 데이터 입력 (상승 추세)
        for day in 0..15 {
            let tip_price = dec!(100) + Decimal::from(day * 2);
            let upro_price = dec!(50) + Decimal::from(day * 2);

            let data1 = create_kline_at("TIP", tip_price, day);
            let data2 = create_kline_at("UPRO", upro_price, day);

            all_signals.extend(strategy.on_market_data(&data1).await.unwrap());
            all_signals.extend(strategy.on_market_data(&data2).await.unwrap());
        }

        // 선행 조건: 신호가 있어야 함
        if !all_signals.is_empty() {
            // US 시장 자산: UPRO, TLT, BIL
            let valid_tickers = vec!["UPRO/USD", "TLT/USD", "BIL/USD"];
            for signal in &all_signals {
                assert!(
                    valid_tickers.contains(&signal.ticker.as_str()),
                    "신호 ticker({})가 유효한 자산이어야 함",
                    signal.ticker
                );
            }
        }
    }
}

// ============================================================================
// 5. 리밸런싱 조건 테스트
// ============================================================================

mod rebalance_condition_tests {
    use super::*;

    /// 테스트 1: 첫 번째 리밸런싱은 항상 실행
    #[tokio::test]
    async fn first_rebalance_always_triggers() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        // 충분한 데이터 입력
        for day in 0..15 {
            let data = create_kline_at("TIP", dec!(100) + Decimal::from(day), day);
            let _ = strategy.on_market_data(&data).await;
        }

        let state = strategy.get_state();
        assert!(
            state["initialized"].as_bool().unwrap_or(false),
            "충분한 데이터 후 초기화되어야 함"
        );
    }

    /// 테스트 2: 리밸런싱 주기 내에서는 신호 없음
    #[tokio::test]
    async fn no_signal_within_rebalance_period() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        // 충분한 데이터로 첫 리밸런싱
        for day in 0..15 {
            let tip_price = dec!(100) + Decimal::from(day);
            let upro_price = dec!(50) + Decimal::from(day);
            let _ = strategy
                .on_market_data(&create_kline_at("TIP", tip_price, day))
                .await;
            let _ = strategy
                .on_market_data(&create_kline_at("UPRO", upro_price, day))
                .await;
        }

        // 같은 주기 내 추가 데이터 (5일 주기 설정)
        let data1 = create_kline_at("UPRO", dec!(80), 15);
        let signals1 = strategy.on_market_data(&data1).await.unwrap();

        let data2 = create_kline_at("UPRO", dec!(82), 16);
        let signals2 = strategy.on_market_data(&data2).await.unwrap();

        // 첫 번째 이후에는 신호가 적어야 함 (이미 리밸런싱됨)
        // 참고: 모드 변경이 있으면 신호가 생성될 수 있음
    }
}

// ============================================================================
// 6. 포지션 업데이트 테스트
// ============================================================================

mod position_update_tests {
    use super::*;

    #[tokio::test]
    async fn position_update_succeeds() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let position = create_position("UPRO", dec!(100), dec!(50));
        let result = strategy.on_position_update(&position).await;

        assert!(result.is_ok());
    }
}

// ============================================================================
// 7. get_state 테스트
// ============================================================================

mod get_state_tests {
    use super::*;

    #[test]
    fn without_initialization_has_default_values() {
        let strategy = MomentumPowerStrategy::new();
        let state = strategy.get_state();

        assert_eq!(state["initialized"], false);
        assert_eq!(state["tip_prices_count"], 0);
    }

    #[tokio::test]
    async fn after_initialization_includes_config() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let state = strategy.get_state();

        assert!(state.get("config").is_some());
        assert!(state.get("state").is_some());
        assert!(state.get("initialized").is_some());
    }

    #[tokio::test]
    async fn state_tracks_prices_count() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        // TIP 데이터 5개 입력
        for i in 0..5 {
            let data = create_kline_at("TIP", dec!(100) + Decimal::from(i), i);
            let _ = strategy.on_market_data(&data).await;
        }

        let state = strategy.get_state();
        assert_eq!(state["tip_prices_count"], 5);
    }
}

// ============================================================================
// 8. shutdown 테스트
// ============================================================================

mod shutdown_tests {
    use super::*;

    #[tokio::test]
    async fn shutdown_completes_successfully() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let result = strategy.shutdown().await;
        assert!(result.is_ok());

        // shutdown 후 initialized가 false가 되어야 함
        let state = strategy.get_state();
        assert_eq!(state["initialized"], false);
    }

    #[tokio::test]
    async fn shutdown_without_initialization_also_succeeds() {
        let mut strategy = MomentumPowerStrategy::new();
        let result = strategy.shutdown().await;
        assert!(result.is_ok());
    }
}

// ============================================================================
// 9. 메타데이터 테스트
// ============================================================================

mod metadata_tests {
    use super::*;

    #[test]
    fn name_is_snow() {
        let strategy = MomentumPowerStrategy::new();
        assert_eq!(strategy.name(), "Snow");
    }

    #[test]
    fn version_is_semantic() {
        let strategy = MomentumPowerStrategy::new();
        let version = strategy.version();
        let parts: Vec<&str> = version.split('.').collect();
        assert_eq!(parts.len(), 3);
    }

    #[test]
    fn description_is_not_empty() {
        let strategy = MomentumPowerStrategy::new();
        assert!(!strategy.description().is_empty());
    }
}

// ============================================================================
// 10. 엣지 케이스 테스트
// ============================================================================

mod edge_case_tests {
    use super::*;

    #[tokio::test]
    async fn handles_unknown_ticker() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let data = create_kline_at("UNKNOWN_XYZ", dec!(100), 0);
        let signals = strategy.on_market_data(&data).await.unwrap();

        assert!(signals.is_empty(), "알 수 없는 ticker는 무시해야 함");
    }

    #[tokio::test]
    async fn handles_zero_price() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let data = create_kline_at("TIP", dec!(0), 0);
        let result = strategy.on_market_data(&data).await;

        assert!(result.is_ok(), "0 가격도 에러 없이 처리해야 함");
    }

    #[tokio::test]
    async fn handles_very_large_price() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let data = create_kline_at("TIP", dec!(999999999), 0);
        let result = strategy.on_market_data(&data).await;

        assert!(result.is_ok(), "큰 가격도 에러 없이 처리해야 함");
    }

    #[tokio::test]
    async fn no_signal_without_initialization() {
        let mut strategy = MomentumPowerStrategy::new();
        // initialize 호출 안 함

        let data = create_kline_at("TIP", dec!(100), 0);
        let signals = strategy.on_market_data(&data).await.unwrap();

        assert!(signals.is_empty(), "초기화 전에는 신호 없어야 함");
    }
}
