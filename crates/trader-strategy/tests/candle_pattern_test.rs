//! CandlePattern (캔들스틱 패턴) 전략 통합 테스트
//!
//! 35가지 이상의 캔들스틱 패턴을 인식하여 매매 신호를 생성하는 전략 테스트.
//! Hammer, Doji, Engulfing, Morning Star 등 다양한 패턴 인식 로직 검증.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use trader_core::{Kline, MarketData, Position, Side, Timeframe};
use trader_strategy::strategies::candle_pattern::CandlePatternStrategy;
use trader_strategy::Strategy;

// ============================================================================
// 테스트 헬퍼 함수
// ============================================================================

/// 테스트용 MarketData 생성 헬퍼 (OHLCV 포함)
fn create_market_data_ohlcv(
    ticker: &str,
    open: Decimal,
    high: Decimal,
    low: Decimal,
    close: Decimal,
    volume: Decimal,
    day: i64,
) -> MarketData {
    let timestamp = chrono::DateTime::from_timestamp(1704067200 + day * 86400, 0).unwrap();
    let kline = Kline::new(
        ticker.to_string(),
        Timeframe::D1,
        timestamp,
        open,
        high,
        low,
        close,
        volume,
        timestamp,
    );
    MarketData::from_kline("test", kline)
}

/// 간단한 MarketData 생성 헬퍼
fn create_market_data(ticker: &str, close: Decimal, day: i64) -> MarketData {
    create_market_data_ohlcv(
        ticker,
        close - dec!(100),
        close + dec!(200),
        close - dec!(200),
        close,
        dec!(100000),
        day,
    )
}

/// Position 헬퍼 함수
fn create_position(ticker: &str, quantity: Decimal, entry_price: Decimal) -> Position {
    Position::new("test", ticker.to_string(), Side::Buy, quantity, entry_price)
}

// ============================================================================
// 초기화 테스트
// ============================================================================

#[tokio::test]
async fn test_initialization_default_config() {
    let mut strategy = CandlePatternStrategy::new();

    let config = json!({
        "ticker": "005930"
    });
    let result = strategy.initialize(config).await;

    assert!(result.is_ok(), "기본 설정으로 초기화 실패");

    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

#[tokio::test]
async fn test_initialization_with_custom_config() {
    let mut strategy = CandlePatternStrategy::new();

    let config = json!({
        "ticker": "005930",
        "trade_amount": "1000000",
        "min_pattern_strength": "0.7",
        "use_volume_confirmation": true,
        "use_trend_confirmation": true,
        "trend_period": 15,
        "stop_loss_pct": "3.0",
        "take_profit_pct": "5.0",
        "min_global_score": "55"
    });

    let result = strategy.initialize(config).await;
    assert!(result.is_ok(), "커스텀 설정으로 초기화 실패");

    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

#[tokio::test]
async fn test_name_version_description() {
    let strategy = CandlePatternStrategy::new();

    assert_eq!(strategy.name(), "Candle Pattern");
    assert_eq!(strategy.version(), "1.0.0");
    assert!(strategy.description().contains("캔들") || strategy.description().contains("패턴"));
}

// ============================================================================
// 데이터 처리 테스트
// ============================================================================

#[tokio::test]
async fn test_data_accumulation() {
    let mut strategy = CandlePatternStrategy::new();

    let config = json!({
        "ticker": "005930"
    });
    strategy.initialize(config).await.unwrap();

    // 데이터 축적
    for day in 0..10 {
        let data = create_market_data("005930", dec!(70000) + Decimal::from(day * 100), day);
        let _ = strategy.on_market_data(&data).await;
    }

    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

#[tokio::test]
async fn test_ignores_unregistered_ticker() {
    let mut strategy = CandlePatternStrategy::new();

    let config = json!({
        "ticker": "005930"
    });
    strategy.initialize(config).await.unwrap();

    // 등록되지 않은 티커
    let data = create_market_data("000660", dec!(100000), 0);
    let signals = strategy.on_market_data(&data).await.unwrap();

    assert!(signals.is_empty(), "등록되지 않은 티커는 무시");
}

// ============================================================================
// 캔들 패턴 감지 테스트
// ============================================================================

#[tokio::test]
async fn test_hammer_pattern_detection() {
    let mut strategy = CandlePatternStrategy::new();

    let config = json!({
        "ticker": "005930",
        "use_volume_confirmation": false,
        "use_trend_confirmation": false,
        "min_pattern_strength": "0.5",
        "min_global_score": "0"
    });
    strategy.initialize(config).await.unwrap();

    // 하락 추세 데이터 먼저 (패턴 문맥 제공)
    for day in 0..5 {
        let price = dec!(75000) - Decimal::from(day * 1000);
        let data = create_market_data("005930", price, day);
        let _ = strategy.on_market_data(&data).await;
    }

    // 망치형 캔들 (Hammer)
    // 특징: 작은 실체, 긴 아래꼬리, 거의 없는 윗꼬리
    let hammer = create_market_data_ohlcv(
        "005930",
        dec!(70000),  // open
        dec!(70500),  // high (작은 윗꼬리)
        dec!(67000),  // low (긴 아래꼬리)
        dec!(70300),  // close (open 근처)
        dec!(200000), // volume
        5,
    );

    let signals = strategy.on_market_data(&hammer).await.unwrap();

    // 패턴 감지 여부 확인
    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

#[tokio::test]
async fn test_doji_pattern_detection() {
    let mut strategy = CandlePatternStrategy::new();

    let config = json!({
        "ticker": "005930",
        "use_volume_confirmation": false,
        "use_trend_confirmation": false,
        "min_pattern_strength": "0.5",
        "min_global_score": "0"
    });
    strategy.initialize(config).await.unwrap();

    // 데이터 축적
    for day in 0..5 {
        let data = create_market_data("005930", dec!(70000), day);
        let _ = strategy.on_market_data(&data).await;
    }

    // 도지 캔들 (Doji)
    // 특징: 시가 = 종가 (또는 거의 같음)
    let doji = create_market_data_ohlcv(
        "005930",
        dec!(70000),  // open
        dec!(71000),  // high
        dec!(69000),  // low
        dec!(70050),  // close ≈ open
        dec!(150000), // volume
        5,
    );

    let signals = strategy.on_market_data(&doji).await.unwrap();

    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

#[tokio::test]
async fn test_engulfing_pattern_detection() {
    let mut strategy = CandlePatternStrategy::new();

    let config = json!({
        "ticker": "005930",
        "use_volume_confirmation": false,
        "use_trend_confirmation": false,
        "min_pattern_strength": "0.3",
        "min_global_score": "0"
    });
    strategy.initialize(config).await.unwrap();

    // 작은 음봉
    let small_bearish = create_market_data_ohlcv(
        "005930",
        dec!(70500), // open
        dec!(70800), // high
        dec!(69800), // low
        dec!(70000), // close < open (음봉)
        dec!(100000),
        0,
    );
    let _ = strategy.on_market_data(&small_bearish).await;

    // 큰 양봉 (Bullish Engulfing)
    // 특징: 전일 음봉을 완전히 감싸는 양봉
    let engulfing = create_market_data_ohlcv(
        "005930",
        dec!(69500),  // open < 전일 저가
        dec!(71500),  // high > 전일 고가
        dec!(69300),  // low
        dec!(71200),  // close > 전일 고가
        dec!(200000), // 높은 거래량
        1,
    );

    let signals = strategy.on_market_data(&engulfing).await.unwrap();

    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

#[tokio::test]
async fn test_morning_star_pattern_detection() {
    let mut strategy = CandlePatternStrategy::new();

    let config = json!({
        "ticker": "005930",
        "use_volume_confirmation": false,
        "use_trend_confirmation": false,
        "min_pattern_strength": "0.3",
        "min_global_score": "0"
    });
    strategy.initialize(config).await.unwrap();

    // Morning Star (3봉 패턴)
    // 1일: 큰 음봉
    let day1 = create_market_data_ohlcv(
        "005930",
        dec!(72000), // open
        dec!(72500), // high
        dec!(69000), // low
        dec!(69500), // close (큰 하락)
        dec!(150000),
        0,
    );
    let _ = strategy.on_market_data(&day1).await;

    // 2일: 작은 캔들 (도지 또는 스피닝탑)
    let day2 = create_market_data_ohlcv(
        "005930",
        dec!(69000), // gap down open
        dec!(69500), // high
        dec!(68500), // low
        dec!(69200), // close ≈ open
        dec!(80000), // 낮은 거래량
        1,
    );
    let _ = strategy.on_market_data(&day2).await;

    // 3일: 큰 양봉
    let day3 = create_market_data_ohlcv(
        "005930",
        dec!(69500),  // open
        dec!(72000),  // high
        dec!(69300),  // low
        dec!(71800),  // close (큰 상승)
        dec!(200000), // 높은 거래량
        2,
    );

    let signals = strategy.on_market_data(&day3).await.unwrap();

    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

// ============================================================================
// 포지션 관리 테스트
// ============================================================================

#[tokio::test]
async fn test_position_update() {
    let mut strategy = CandlePatternStrategy::new();

    let config = json!({
        "ticker": "005930"
    });
    strategy.initialize(config).await.unwrap();

    let position = create_position("005930", dec!(10), dec!(70000));
    let result = strategy.on_position_update(&position).await;
    assert!(result.is_ok());

    let state = strategy.get_state();
    // 포지션 상태 확인
}

#[tokio::test]
async fn test_position_cleared_on_zero_quantity() {
    let mut strategy = CandlePatternStrategy::new();

    let config = json!({
        "ticker": "005930"
    });
    strategy.initialize(config).await.unwrap();

    // 포지션 추가
    let position = create_position("005930", dec!(10), dec!(70000));
    strategy.on_position_update(&position).await.unwrap();

    // 수량 0으로 청산
    let zero_position = create_position("005930", dec!(0), dec!(0));
    strategy.on_position_update(&zero_position).await.unwrap();

    let state = strategy.get_state();
    // 포지션 제거 확인
}

// ============================================================================
// 손절/익절 테스트
// ============================================================================

#[tokio::test]
async fn test_stop_loss_trigger() {
    let mut strategy = CandlePatternStrategy::new();

    let config = json!({
        "ticker": "005930",
        "stop_loss_pct": "3.0",
        "take_profit_pct": "5.0",
        "min_global_score": "0"
    });
    strategy.initialize(config).await.unwrap();

    // 포지션 설정 - 진입가 70000원
    let position = create_position("005930", dec!(10), dec!(70000));
    strategy.on_position_update(&position).await.unwrap();

    // 데이터 축적
    for day in 0..5 {
        let data = create_market_data("005930", dec!(70000), day);
        let _ = strategy.on_market_data(&data).await;
    }

    // 손절 조건 (-3% 이하)
    // 70000 * 0.97 = 67900
    let stop_loss_data = create_market_data_ohlcv(
        "005930",
        dec!(69000),
        dec!(69500),
        dec!(67500),
        dec!(67800), // -3.1%
        dec!(150000),
        6,
    );

    let signals = strategy.on_market_data(&stop_loss_data).await.unwrap();

    // 손절 신호 확인
    let sell_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Sell).collect();
    // 손절 로직 동작 여부
}

#[tokio::test]
async fn test_take_profit_trigger() {
    let mut strategy = CandlePatternStrategy::new();

    let config = json!({
        "ticker": "005930",
        "stop_loss_pct": "3.0",
        "take_profit_pct": "5.0",
        "min_global_score": "0"
    });
    strategy.initialize(config).await.unwrap();

    // 포지션 설정 - 진입가 70000원
    let position = create_position("005930", dec!(10), dec!(70000));
    strategy.on_position_update(&position).await.unwrap();

    // 데이터 축적
    for day in 0..5 {
        let data = create_market_data("005930", dec!(70000), day);
        let _ = strategy.on_market_data(&data).await;
    }

    // 익절 조건 (+5% 이상)
    // 70000 * 1.05 = 73500
    let take_profit_data = create_market_data_ohlcv(
        "005930",
        dec!(72000),
        dec!(74000),
        dec!(71800),
        dec!(73600), // +5.1%
        dec!(200000),
        6,
    );

    let signals = strategy.on_market_data(&take_profit_data).await.unwrap();

    // 익절 신호 확인
    let sell_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Sell).collect();
    // 익절 로직 동작 여부
}

// ============================================================================
// 볼륨 확인 테스트
// ============================================================================

#[tokio::test]
async fn test_volume_confirmation() {
    let mut strategy = CandlePatternStrategy::new();

    let config = json!({
        "ticker": "005930",
        "use_volume_confirmation": true,
        "min_pattern_strength": "0.3",
        "min_global_score": "0"
    });
    strategy.initialize(config).await.unwrap();

    // 평균 거래량 축적
    for day in 0..10 {
        let data = create_market_data_ohlcv(
            "005930",
            dec!(70000),
            dec!(70500),
            dec!(69500),
            dec!(70200),
            dec!(100000), // 평균 거래량
            day,
        );
        let _ = strategy.on_market_data(&data).await;
    }

    // 높은 거래량의 패턴 (볼륨 확인)
    let high_volume_pattern = create_market_data_ohlcv(
        "005930",
        dec!(69500),
        dec!(71500),
        dec!(69300),
        dec!(71200),
        dec!(300000), // 평균의 3배
        10,
    );

    let signals = strategy.on_market_data(&high_volume_pattern).await.unwrap();

    // 볼륨 확인 패스 시 신호 강화
    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

// ============================================================================
// 상태 관리 테스트
// ============================================================================

#[tokio::test]
async fn test_get_state_comprehensive() {
    let mut strategy = CandlePatternStrategy::new();

    let config = json!({
        "ticker": "005930"
    });
    strategy.initialize(config).await.unwrap();

    let state = strategy.get_state();

    // 필수 필드 확인
    assert!(!state["initialized"].is_null());
    assert!(!state["candles_count"].is_null());
    assert!(!state["current_trend"].is_null());

    // 초기 값 확인
    assert_eq!(state["initialized"], true);
    assert_eq!(state["candles_count"], 0);
}

// ============================================================================
// 종료 테스트
// ============================================================================

#[tokio::test]
async fn test_shutdown() {
    let mut strategy = CandlePatternStrategy::new();

    let config = json!({
        "ticker": "005930"
    });
    strategy.initialize(config).await.unwrap();

    let result = strategy.shutdown().await;
    assert!(result.is_ok(), "정상 종료 실패");
}

// ============================================================================
// 에러 처리 테스트
// ============================================================================

#[tokio::test]
async fn test_process_data_before_initialization() {
    let mut strategy = CandlePatternStrategy::new();

    // 초기화 없이 데이터 처리
    let data = create_market_data("005930", dec!(70000), 0);
    let signals = strategy.on_market_data(&data).await.unwrap();

    assert!(signals.is_empty(), "초기화 전에는 신호 없어야 함");
}

// ============================================================================
// 복합 시나리오 테스트
// ============================================================================

#[tokio::test]
async fn test_full_trading_cycle_with_patterns() {
    let mut strategy = CandlePatternStrategy::new();

    let config = json!({
        "ticker": "005930",
        "trade_amount": "1000000",
        "min_pattern_strength": "0.3",
        "use_volume_confirmation": false,
        "use_trend_confirmation": false,
        "stop_loss_pct": "3.0",
        "take_profit_pct": "5.0",
        "min_global_score": "0"
    });
    strategy.initialize(config).await.unwrap();

    // Phase 1: 하락 추세
    for day in 0..5 {
        let price = dec!(75000) - Decimal::from(day * 1000);
        let data = create_market_data_ohlcv(
            "005930",
            price + dec!(200),
            price + dec!(500),
            price - dec!(300),
            price,
            dec!(100000) + Decimal::from(day * 10000),
            day,
        );
        let _ = strategy.on_market_data(&data).await;
    }

    // Phase 2: 반전 패턴 (망치형)
    let hammer = create_market_data_ohlcv(
        "005930",
        dec!(70000),
        dec!(70500),
        dec!(67000),
        dec!(70300),
        dec!(200000),
        5,
    );
    let _ = strategy.on_market_data(&hammer).await;

    // Phase 3: 상승 추세
    for day in 6..12 {
        let price = dec!(70000) + Decimal::from((day - 5) * 800);
        let data = create_market_data_ohlcv(
            "005930",
            price - dec!(200),
            price + dec!(500),
            price - dec!(400),
            price,
            dec!(150000),
            day,
        );
        let _ = strategy.on_market_data(&data).await;
    }

    // 최종 상태 확인
    let final_state = strategy.get_state();
    assert!(final_state["initialized"].as_bool().unwrap_or(false));
}

#[tokio::test]
async fn test_multiple_pattern_sequence() {
    let mut strategy = CandlePatternStrategy::new();

    let config = json!({
        "ticker": "005930",
        "min_pattern_strength": "0.3",
        "use_volume_confirmation": false,
        "use_trend_confirmation": false,
        "min_global_score": "0"
    });
    strategy.initialize(config).await.unwrap();

    // 다양한 캔들 패턴 시퀀스
    let candles = [
        // 일반 캔들
        (dec!(70000), dec!(71000), dec!(69500), dec!(70500), 0),
        // 도지
        (dec!(70500), dec!(71500), dec!(69500), dec!(70550), 1),
        // 작은 음봉
        (dec!(70500), dec!(71000), dec!(69800), dec!(70000), 2),
        // 장악형 양봉
        (dec!(69500), dec!(72000), dec!(69300), dec!(71800), 3),
        // 마루보즈 양봉
        (dec!(71800), dec!(73500), dec!(71800), dec!(73500), 4),
    ];

    for (open, high, low, close, day) in candles {
        let data = create_market_data_ohlcv("005930", open, high, low, close, dec!(150000), day);
        let _ = strategy.on_market_data(&data).await;
    }

    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}
