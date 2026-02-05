//! MeanReversion 전략 통합 테스트.
//!
//! 각 variant별 핵심 케이스를 모두 검증합니다:
//! - RSI: 과매도 진입, 과매수 청산, 손절/익절
//! - Bollinger: 하단밴드 터치, 스퀴즈 회피
//! - Grid: 레벨별 매수/매도, 사이클 반복
//! - MagicSplit: 분할 진입, 목표가 청산

use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use trader_core::types::Timeframe;
use trader_core::{Kline, MarketData, MarketDataType, Position, Side, Ticker};
use trader_strategy::strategies::mean_reversion::{
    BollingerConfig, EntrySignalConfig, ExitConfig, MeanReversionConfig, MeanReversionStrategy,
    SplitLevel, StrategyVariant as MeanReversionVariant,
};
use trader_strategy::Strategy;

// ================================================================================================
// 헬퍼 함수
// ================================================================================================

/// 테스트용 Kline 마켓 데이터 생성.
fn create_kline_data(ticker_name: &str, close: Decimal) -> MarketData {
    let now = Utc::now();
    MarketData {
        exchange: "test".to_string(),
        ticker: ticker_name.to_string(),
        timestamp: now,
        data: MarketDataType::Kline(Kline {
            ticker: ticker_name.to_string(),
            timeframe: Timeframe::D1,
            open_time: now,
            open: close,
            high: close + dec!(100),
            low: close - dec!(100),
            close,
            volume: dec!(1000000),
            close_time: now,
            quote_volume: Some(dec!(100000000000)),
            num_trades: Some(1000),
        }),
    }
}

/// 테스트용 Ticker 마켓 데이터 생성.
fn create_ticker_data(ticker_name: &str, last: Decimal) -> MarketData {
    let now = Utc::now();
    MarketData {
        exchange: "test".to_string(),
        ticker: ticker_name.to_string(),
        timestamp: now,
        data: MarketDataType::Ticker(Ticker {
            ticker: ticker_name.to_string(),
            bid: last - dec!(10),
            ask: last + dec!(10),
            last,
            volume_24h: dec!(1000000),
            high_24h: last + dec!(100),
            low_24h: last - dec!(100),
            change_24h: dec!(0),
            change_24h_percent: dec!(0),
            timestamp: now,
        }),
    }
}

/// 테스트용 포지션 생성.
fn create_position(
    ticker_name: &str,
    side: Side,
    quantity: Decimal,
    entry_price: Decimal,
) -> Position {
    Position::new("test", ticker_name.to_string(), side, quantity, entry_price)
}

// ================================================================================================
// RSI Variant 테스트 - 핵심 케이스 완전 검증
// ================================================================================================

mod rsi_tests {
    use super::*;

    /// 테스트 1: 초기화 성공
    #[tokio::test]
    async fn test_rsi_initialization() {
        let mut strategy = MeanReversionStrategy::new();
        let config = MeanReversionConfig::rsi_default("005930");
        let config_json = serde_json::to_value(config).unwrap();

        let result = strategy.initialize(config_json).await;

        assert!(result.is_ok(), "RSI 설정으로 초기화 실패");
        assert_eq!(strategy.name(), "MeanReversion-RSI");
    }

    /// 테스트 2: RSI 과매도 후 상승 시 매수 신호 발생
    ///
    /// RSI 진입 조건:
    /// 1. 포지션 없음
    /// 2. 쿨다운 아님
    /// 3. can_enter() 통과 (Context 없으면 true)
    /// 4. prev_rsi < oversold && current_rsi > prev_rsi (과매도 상태에서 상승 중)
    #[tokio::test]
    async fn test_rsi_oversold_buy_signal() {
        let mut strategy = MeanReversionStrategy::new();
        let config = MeanReversionConfig {
            variant: MeanReversionVariant::Rsi,
            ticker: "005930".to_string(),
            amount: dec!(1000000),
            entry_signal: EntrySignalConfig::Rsi {
                oversold: dec!(30),
                overbought: dec!(70),
                period: 14,
            },
            exit_config: ExitConfig {
                stop_loss_pct: dec!(3),
                take_profit_pct: dec!(10),
                exit_on_neutral: false,
                cooldown_candles: 0, // 쿨다운 비활성화
            },
            max_positions: 1,
            min_global_score: dec!(0), // 필터 비활성화
        };
        let config_json = serde_json::to_value(config).unwrap();
        strategy.initialize(config_json).await.unwrap();

        // RSI 계산에 최소 14+1개 데이터 필요
        // 급격한 하락으로 RSI를 과매도 영역으로 만듦
        let mut all_signals = vec![];

        // Phase 1: 급격한 하락 (RSI를 과매도로)
        for i in 0..16 {
            let price = dec!(100000) - Decimal::from(i * 3000); // 급격한 하락
            let data = create_kline_data("005930", price);
            let signals = strategy.on_market_data(&data).await.unwrap();
            all_signals.extend(signals);
        }

        // Phase 2: 과매도 상태에서 반등 (RSI 상향)
        // 이전 RSI가 과매도이고, 현재 RSI가 상승하면 신호 발생
        let rebound_prices = [
            dec!(55000),
            dec!(57000),
            dec!(59000),
            dec!(61000),
            dec!(63000),
        ];
        for price in rebound_prices {
            let data = create_kline_data("005930", price);
            let signals = strategy.on_market_data(&data).await.unwrap();
            all_signals.extend(signals);
        }

        // 검증: 매수 신호가 발생해야 함
        let buy_signals: Vec<_> = all_signals.iter().filter(|s| s.side == Side::Buy).collect();
        assert!(
            !buy_signals.is_empty(),
            "RSI 과매도 후 반등 시 매수 신호가 발생해야 함. 총 신호: {}, 매수: {}",
            all_signals.len(),
            buy_signals.len()
        );
    }

    /// 테스트 3: 포지션 보유 중 손절가 도달 시 매도 신호
    #[tokio::test]
    async fn test_rsi_stop_loss_exit() {
        let mut strategy = MeanReversionStrategy::new();
        let config = MeanReversionConfig {
            variant: MeanReversionVariant::Rsi,
            ticker: "005930".to_string(),
            amount: dec!(1000000),
            entry_signal: EntrySignalConfig::Rsi {
                oversold: dec!(30),
                overbought: dec!(70),
                period: 14,
            },
            exit_config: ExitConfig {
                stop_loss_pct: dec!(5), // 5% 손절
                take_profit_pct: dec!(10),
                exit_on_neutral: false,
                cooldown_candles: 0,
            },
            max_positions: 1,
            min_global_score: dec!(0),
        };
        let config_json = serde_json::to_value(config).unwrap();
        strategy.initialize(config_json).await.unwrap();

        // RSI 데이터 축적
        for i in 0..15 {
            let price = dec!(50000) + Decimal::from((i % 3) * 100);
            let data = create_kline_data("005930", price);
            let _ = strategy.on_market_data(&data).await;
        }

        // 포지션 설정 (50000원에 진입했다고 가정)
        let position = create_position("005930", Side::Buy, dec!(10), dec!(50000));
        strategy.on_position_update(&position).await.unwrap();

        // 5% 손실 발생 (50000 -> 47500)
        let data = create_kline_data("005930", dec!(47500));
        let signals = strategy.on_market_data(&data).await.unwrap();

        // 검증: 손절 매도 신호 발생
        let sell_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Sell).collect();
        assert!(
            !sell_signals.is_empty(),
            "5% 손실 시 손절 매도 신호가 발생해야 함"
        );

        // 손절 이유 확인
        let has_stop_loss = sell_signals.iter().any(|s| {
            s.metadata
                .get("reason")
                .map(|r| r == "stop_loss")
                .unwrap_or(false)
        });
        assert!(has_stop_loss, "매도 신호는 stop_loss 이유여야 함");
    }

    /// 테스트 4: 포지션 보유 중 익절가 도달 시 매도 신호
    #[tokio::test]
    async fn test_rsi_take_profit_exit() {
        let mut strategy = MeanReversionStrategy::new();
        let config = MeanReversionConfig {
            variant: MeanReversionVariant::Rsi,
            ticker: "005930".to_string(),
            amount: dec!(1000000),
            entry_signal: EntrySignalConfig::Rsi {
                oversold: dec!(30),
                overbought: dec!(70),
                period: 14,
            },
            exit_config: ExitConfig {
                stop_loss_pct: dec!(5),
                take_profit_pct: dec!(10), // 10% 익절
                exit_on_neutral: false,
                cooldown_candles: 0,
            },
            max_positions: 1,
            min_global_score: dec!(0),
        };
        let config_json = serde_json::to_value(config).unwrap();
        strategy.initialize(config_json).await.unwrap();

        // RSI 데이터 축적
        for i in 0..15 {
            let price = dec!(50000) + Decimal::from((i % 3) * 100);
            let data = create_kline_data("005930", price);
            let _ = strategy.on_market_data(&data).await;
        }

        // 포지션 설정 (50000원에 진입)
        let position = create_position("005930", Side::Buy, dec!(10), dec!(50000));
        strategy.on_position_update(&position).await.unwrap();

        // 10% 수익 발생 (50000 -> 55000)
        let data = create_kline_data("005930", dec!(55000));
        let signals = strategy.on_market_data(&data).await.unwrap();

        // 검증: 익절 매도 신호 발생
        let sell_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Sell).collect();
        assert!(
            !sell_signals.is_empty(),
            "10% 수익 시 익절 매도 신호가 발생해야 함"
        );

        // 익절 이유 확인
        let has_take_profit = sell_signals.iter().any(|s| {
            s.metadata
                .get("reason")
                .map(|r| r == "take_profit")
                .unwrap_or(false)
        });
        assert!(has_take_profit, "매도 신호는 take_profit 이유여야 함");
    }

    /// 테스트 5: 포지션 보유 중 RSI 과매수 시 청산
    #[tokio::test]
    async fn test_rsi_overbought_exit() {
        let mut strategy = MeanReversionStrategy::new();
        let config = MeanReversionConfig {
            variant: MeanReversionVariant::Rsi,
            ticker: "005930".to_string(),
            amount: dec!(1000000),
            entry_signal: EntrySignalConfig::Rsi {
                oversold: dec!(30),
                overbought: dec!(70),
                period: 14,
            },
            exit_config: ExitConfig {
                stop_loss_pct: dec!(0),   // 손절 비활성화
                take_profit_pct: dec!(0), // 익절 비활성화 (RSI 과매수만 트리거되도록)
                exit_on_neutral: false,
                cooldown_candles: 0,
            },
            max_positions: 1,
            min_global_score: dec!(0),
        };
        let config_json = serde_json::to_value(config).unwrap();
        strategy.initialize(config_json).await.unwrap();

        // 급격한 상승으로 RSI를 과매수로 만듦
        for i in 0..16 {
            let price = dec!(50000) + Decimal::from(i * 2000);
            let data = create_kline_data("005930", price);
            let _ = strategy.on_market_data(&data).await;
        }

        // 포지션 설정 (현재 가격 수준에서 진입, 익절 조건 회피)
        let position = create_position("005930", Side::Buy, dec!(10), dec!(80000));
        strategy.on_position_update(&position).await.unwrap();

        // 계속 상승 (RSI 과매수 유지)
        let mut sell_signals = vec![];
        for i in 0..5 {
            let price = dec!(82000) + Decimal::from(i * 1000);
            let data = create_kline_data("005930", price);
            let signals = strategy.on_market_data(&data).await.unwrap();
            sell_signals.extend(signals.into_iter().filter(|s| s.side == Side::Sell));
        }

        // 검증: RSI 과매수 청산 신호 발생
        assert!(
            !sell_signals.is_empty(),
            "RSI 과매수 상태에서 청산 신호가 발생해야 함"
        );

        // 과매수 이유 확인
        let has_overbought = sell_signals.iter().any(|s| {
            s.metadata
                .get("reason")
                .map(|r| r == "rsi_overbought")
                .unwrap_or(false)
        });
        assert!(has_overbought, "매도 신호는 rsi_overbought 이유여야 함");
    }

    /// 테스트 6: 초기화 전 시장 데이터 처리
    #[tokio::test]
    async fn test_rsi_no_signal_without_init() {
        let mut strategy = MeanReversionStrategy::new();
        let data = create_kline_data("005930", dec!(50000));

        let result = strategy.on_market_data(&data).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty(), "초기화 전에는 신호 없어야 함");
    }

    /// 테스트 7: 다른 티커 데이터 무시
    #[tokio::test]
    async fn test_rsi_ignores_wrong_ticker() {
        let mut strategy = MeanReversionStrategy::new();
        let config = MeanReversionConfig::rsi_default("005930");
        let config_json = serde_json::to_value(config).unwrap();
        strategy.initialize(config_json).await.unwrap();

        let data = create_kline_data("000660", dec!(100000));
        let signals = strategy.on_market_data(&data).await.unwrap();

        assert!(signals.is_empty(), "다른 티커 데이터는 무시해야 함");
    }
}

// ================================================================================================
// Bollinger Variant 테스트
// ================================================================================================

mod bollinger_tests {
    use super::*;

    /// 테스트 1: 초기화 성공
    #[tokio::test]
    async fn test_bollinger_initialization() {
        let mut strategy = MeanReversionStrategy::new();
        let config = MeanReversionConfig::bollinger_default("005930");
        let config_json = serde_json::to_value(config).unwrap();

        let result = strategy.initialize(config_json).await;

        assert!(result.is_ok());
        assert_eq!(strategy.name(), "MeanReversion-Bollinger");
    }

    /// 테스트 2: 하단 밴드 터치 시 매수 신호
    ///
    /// # 전략 컨셉
    ///
    /// 볼린저 밴드 전략은 **충분한 변동성(bandwidth)이 있을 때만** 진입합니다.
    /// 스퀴즈(저변동성) 상태에서는 밴드를 터치해도 무시됩니다.
    ///
    /// - min_bandwidth_pct: 최소 밴드폭 % (이 이상이어야 신호 발생)
    /// - bandwidth = (upper - lower) / sma * 100
    /// - 따라서 테스트에서 충분한 가격 변동을 제공해야 함
    ///
    /// # 중요
    ///
    /// `initialize()`는 UI Config 타입(BollingerConfig)으로 파싱하므로,
    /// MeanReversionConfig가 아닌 BollingerConfig를 직접 생성해야 함!
    #[tokio::test]
    async fn test_bollinger_lower_band_buy_signal() {
        let mut strategy = MeanReversionStrategy::new();
        // BollingerConfig 직접 생성 (initialize가 이 타입으로 파싱함)
        let config = BollingerConfig {
            ticker: "005930".to_string(),
            amount: dec!(1000000),
            period: 20,
            std_multiplier: dec!(2),
            use_rsi_confirmation: false, // RSI 확인 비활성화 - 중요!
            min_bandwidth_pct: dec!(1),  // 1% 이상 밴드폭 필요
            exit_config: ExitConfig {
                stop_loss_pct: dec!(5),
                take_profit_pct: dec!(10),
                exit_on_neutral: false,
                cooldown_candles: 0,
            },
            max_positions: 1,
            min_global_score: dec!(0),
        };
        let config_json = serde_json::to_value(config).unwrap();
        strategy.initialize(config_json).await.unwrap();

        let mut all_signals = vec![];

        // Phase 1: 변동성 있는 가격 데이터로 볼린저 밴드 형성
        // bandwidth > 1%를 달성하려면 충분한 가격 변동 필요
        // bandwidth = 2 * std_multiplier * std_dev / sma * 100
        // 50000원 기준 1% = 500원 → 표준편차 약 125원 이상 필요
        // 가격 범위 ±1500원으로 설정 (48500 ~ 51500)
        let high_volatility_prices = [
            dec!(50000), dec!(51000), dec!(49000), dec!(51500), dec!(48500),
            dec!(50500), dec!(49500), dec!(51200), dec!(48800), dec!(50200),
            dec!(50000), dec!(51000), dec!(49000), dec!(51500), dec!(48500),
            dec!(50500), dec!(49500), dec!(51200), dec!(48800), dec!(50200),
            dec!(50000), dec!(50500), dec!(49500), dec!(50200), dec!(49800),
        ];
        for price in high_volatility_prices {
            let data = create_kline_data("005930", price);
            let signals = strategy.on_market_data(&data).await.unwrap();
            all_signals.extend(signals);
        }

        // Phase 2: 급격한 하락 (하단 밴드 터치)
        // 변동성이 유지된 상태에서 하단 밴드 아래로 하락
        // 계산된 Lower 밴드: 약 48000 → 이보다 낮은 가격으로 터치
        let drop_prices = [
            dec!(48000),
            dec!(47500),
            dec!(47000),
            dec!(46500),
            dec!(46000),
        ];
        for price in drop_prices {
            let data = create_kline_data("005930", price);
            let signals = strategy.on_market_data(&data).await.unwrap();
            all_signals.extend(signals);
        }

        // 검증: 하단 밴드 터치 시 매수 신호 발생
        let buy_signals: Vec<_> = all_signals.iter().filter(|s| s.side == Side::Buy).collect();
        assert!(
            !buy_signals.is_empty(),
            "볼린저 하단 밴드 터치 시 매수 신호가 발생해야 함. 총 신호: {}, 매수: {}",
            all_signals.len(),
            buy_signals.len()
        );
    }

    /// 테스트 3: 스퀴즈(저변동성) 상태에서 신호 없음
    #[tokio::test]
    async fn test_bollinger_squeeze_no_signal() {
        let mut strategy = MeanReversionStrategy::new();
        let config = MeanReversionConfig {
            variant: MeanReversionVariant::Bollinger,
            ticker: "005930".to_string(),
            amount: dec!(1000000),
            entry_signal: EntrySignalConfig::Bollinger {
                period: 20,
                std_multiplier: dec!(2),
                use_rsi_confirmation: false,
                min_bandwidth_pct: dec!(5), // 5% 이상 밴드폭 필요
            },
            exit_config: ExitConfig::default(),
            max_positions: 1,
            min_global_score: dec!(0),
        };
        let config_json = serde_json::to_value(config).unwrap();
        strategy.initialize(config_json).await.unwrap();

        // 동일한 가격으로 스퀴즈 상태 생성
        let mut all_signals = vec![];
        for _ in 0..30 {
            let data = create_kline_data("005930", dec!(50000));
            let signals = strategy.on_market_data(&data).await.unwrap();
            all_signals.extend(signals);
        }

        // 검증: 스퀴즈 상태에서 신호 없음
        assert!(
            all_signals.is_empty(),
            "볼린저 스퀴즈(저변동성) 상태에서는 신호가 없어야 함. 발생 신호: {}",
            all_signals.len()
        );
    }
}

// ================================================================================================
// Grid Variant 테스트 - 핵심 케이스 완전 검증
// ================================================================================================

mod grid_tests {
    use super::*;

    /// 테스트 1: 초기화 성공
    #[tokio::test]
    async fn test_grid_initialization() {
        let mut strategy = MeanReversionStrategy::new();
        let config = MeanReversionConfig::grid_default("005930");
        let config_json = serde_json::to_value(config).unwrap();

        let result = strategy.initialize(config_json).await;

        assert!(result.is_ok());
        assert_eq!(strategy.name(), "MeanReversion-Grid");
    }

    /// 테스트 2: 그리드 레벨 도달 시 매수 신호
    ///
    /// 기준가: 50000, 간격: 2%
    /// Level 1: buy at 49000, sell at 50000
    /// Level 2: buy at 48000, sell at 49000
    #[tokio::test]
    async fn test_grid_level_buy_signal() {
        let mut strategy = MeanReversionStrategy::new();
        let config = MeanReversionConfig {
            variant: MeanReversionVariant::Grid,
            ticker: "005930".to_string(),
            amount: dec!(100000),
            entry_signal: EntrySignalConfig::Grid {
                spacing_pct: dec!(2), // 2% 간격
                levels: 3,
                use_atr: false,
                atr_period: 14,
            },
            exit_config: ExitConfig::default(),
            max_positions: 6,
            min_global_score: dec!(0), // 필터 비활성화
        };
        let config_json = serde_json::to_value(config).unwrap();
        strategy.initialize(config_json).await.unwrap();

        // 1. 기준 가격 설정 (그리드 초기화)
        let data = create_kline_data("005930", dec!(50000));
        let signals = strategy.on_market_data(&data).await.unwrap();
        assert!(signals.is_empty(), "첫 데이터는 그리드 초기화만 수행");

        // 2. 1% 하락 - 그리드 전략 내부 로직에 따라 신호 발생 여부 결정됨
        let data = create_kline_data("005930", dec!(49500));
        let _ = strategy.on_market_data(&data).await.unwrap();
        // Note: 그리드 전략의 레벨 계산 로직에 따라 신호 여부가 결정됨

        // 3. 2% 하락 (첫 번째 레벨 49000 도달)
        let data = create_kline_data("005930", dec!(49000));
        let signals = strategy.on_market_data(&data).await.unwrap();

        assert!(
            !signals.is_empty(),
            "그리드 Level 1 (49000) 도달 시 매수 신호가 발생해야 함"
        );
        assert_eq!(
            signals[0].side,
            Side::Buy,
            "그리드 하락 시 매수 신호여야 함"
        );
    }

    /// 테스트 3: 그리드 매수 후 상승 시 매도 신호 (사이클 완성)
    #[tokio::test]
    async fn test_grid_buy_sell_cycle() {
        let mut strategy = MeanReversionStrategy::new();
        let config = MeanReversionConfig {
            variant: MeanReversionVariant::Grid,
            ticker: "005930".to_string(),
            amount: dec!(100000),
            entry_signal: EntrySignalConfig::Grid {
                spacing_pct: dec!(2),
                levels: 3,
                use_atr: false,
                atr_period: 14,
            },
            exit_config: ExitConfig::default(),
            max_positions: 6,
            min_global_score: dec!(0),
        };
        let config_json = serde_json::to_value(config).unwrap();
        strategy.initialize(config_json).await.unwrap();

        // 1. 기준가 설정
        let _ = strategy
            .on_market_data(&create_kline_data("005930", dec!(50000)))
            .await;

        // 2. Level 1 매수 (49000)
        let signals = strategy
            .on_market_data(&create_kline_data("005930", dec!(49000)))
            .await
            .unwrap();
        assert!(!signals.is_empty(), "Level 1 매수 신호 발생해야 함");
        assert_eq!(signals[0].side, Side::Buy);

        // 3. 다시 상승하여 매도가(50000) 도달
        let signals = strategy
            .on_market_data(&create_kline_data("005930", dec!(50000)))
            .await
            .unwrap();

        assert!(
            !signals.is_empty(),
            "그리드 매도가 도달 시 매도 신호가 발생해야 함"
        );
        assert_eq!(
            signals[0].side,
            Side::Sell,
            "그리드 상승 시 매도 신호여야 함"
        );
    }

    /// 테스트 4: 다중 레벨 매수
    #[tokio::test]
    async fn test_grid_multiple_level_buys() {
        let mut strategy = MeanReversionStrategy::new();
        let config = MeanReversionConfig {
            variant: MeanReversionVariant::Grid,
            ticker: "005930".to_string(),
            amount: dec!(100000),
            entry_signal: EntrySignalConfig::Grid {
                spacing_pct: dec!(2),
                levels: 3,
                use_atr: false,
                atr_period: 14,
            },
            exit_config: ExitConfig::default(),
            max_positions: 6,
            min_global_score: dec!(0),
        };
        let config_json = serde_json::to_value(config).unwrap();
        strategy.initialize(config_json).await.unwrap();

        // 기준가
        let _ = strategy
            .on_market_data(&create_kline_data("005930", dec!(50000)))
            .await;

        let mut total_buy_signals = 0;

        // Level 1: 49000
        let signals = strategy
            .on_market_data(&create_kline_data("005930", dec!(49000)))
            .await
            .unwrap();
        total_buy_signals += signals.iter().filter(|s| s.side == Side::Buy).count();

        // Level 2: 48000
        let signals = strategy
            .on_market_data(&create_kline_data("005930", dec!(48000)))
            .await
            .unwrap();
        total_buy_signals += signals.iter().filter(|s| s.side == Side::Buy).count();

        // Level 3: 47000
        let signals = strategy
            .on_market_data(&create_kline_data("005930", dec!(47000)))
            .await
            .unwrap();
        total_buy_signals += signals.iter().filter(|s| s.side == Side::Buy).count();

        // 검증: 레벨별 가격 하락 시 매수 신호 발생
        // Note: 그리드 전략의 정확한 신호 수는 구현 로직에 따라 다름
        assert!(
            total_buy_signals >= 3,
            "3개 이상의 레벨에서 매수 신호가 발생해야 함. 실제: {}",
            total_buy_signals
        );
    }
}

// ================================================================================================
// MagicSplit Variant 테스트
// ================================================================================================

mod magic_split_tests {
    use super::*;

    /// 테스트 1: 초기화 성공
    #[tokio::test]
    async fn test_magic_split_initialization() {
        let mut strategy = MeanReversionStrategy::new();
        let config = MeanReversionConfig::magic_split_default("005930");
        let config_json = serde_json::to_value(config).unwrap();

        let result = strategy.initialize(config_json).await;

        assert!(result.is_ok());
        assert_eq!(strategy.name(), "MeanReversion-MagicSplit");
    }

    /// 테스트 2: 첫 번째 레벨 즉시 진입 (trigger_rate=0)
    #[tokio::test]
    async fn test_magic_split_immediate_first_entry() {
        let mut strategy = MeanReversionStrategy::new();
        let config = MeanReversionConfig {
            variant: MeanReversionVariant::MagicSplit,
            ticker: "005930".to_string(),
            amount: dec!(100000),
            entry_signal: EntrySignalConfig::Split {
                levels: vec![
                    SplitLevel {
                        trigger_rate: dec!(0), // 즉시 진입
                        target_rate: dec!(10),
                        amount: dec!(100000),
                    },
                    SplitLevel {
                        trigger_rate: dec!(-5),
                        target_rate: dec!(8),
                        amount: dec!(150000),
                    },
                ],
            },
            exit_config: ExitConfig::default(),
            max_positions: 2,
            min_global_score: dec!(0),
        };
        let config_json = serde_json::to_value(config).unwrap();
        strategy.initialize(config_json).await.unwrap();

        // 첫 번째 가격 데이터
        let data = create_kline_data("005930", dec!(50000));
        let signals = strategy.on_market_data(&data).await.unwrap();

        // 검증: trigger_rate=0이므로 즉시 진입
        assert!(
            !signals.is_empty(),
            "MagicSplit 첫 번째 레벨(trigger_rate=0)에서 즉시 매수 신호가 발생해야 함"
        );
        assert_eq!(signals[0].side, Side::Buy);
    }

    /// 테스트 3: 손실 발생 시 2차수 진입
    #[tokio::test]
    async fn test_magic_split_second_entry_on_loss() {
        let mut strategy = MeanReversionStrategy::new();
        let config = MeanReversionConfig {
            variant: MeanReversionVariant::MagicSplit,
            ticker: "005930".to_string(),
            amount: dec!(100000),
            entry_signal: EntrySignalConfig::Split {
                levels: vec![
                    SplitLevel {
                        trigger_rate: dec!(0),
                        target_rate: dec!(10),
                        amount: dec!(100000),
                    },
                    SplitLevel {
                        trigger_rate: dec!(-5), // 5% 손실 시 진입
                        target_rate: dec!(8),
                        amount: dec!(150000),
                    },
                ],
            },
            exit_config: ExitConfig::default(),
            max_positions: 2,
            min_global_score: dec!(0),
        };
        let config_json = serde_json::to_value(config).unwrap();
        strategy.initialize(config_json).await.unwrap();

        // 1차수 진입 (50000원)
        let signals1 = strategy
            .on_market_data(&create_kline_data("005930", dec!(50000)))
            .await
            .unwrap();
        assert!(!signals1.is_empty(), "1차수 진입 신호 발생해야 함");

        // 5% 손실 발생 (47500원)
        let signals2 = strategy
            .on_market_data(&create_kline_data("005930", dec!(47500)))
            .await
            .unwrap();

        // 검증: 2차수 진입 신호
        let second_entry_signals: Vec<_> =
            signals2.iter().filter(|s| s.side == Side::Buy).collect();
        assert!(
            !second_entry_signals.is_empty(),
            "5% 손실 시 2차수 매수 신호가 발생해야 함"
        );
    }
}

// ================================================================================================
// 공통 기능 테스트
// ================================================================================================

mod common_tests {
    use super::*;

    #[tokio::test]
    async fn test_get_state_returns_valid_json() {
        let mut strategy = MeanReversionStrategy::new();
        let config = MeanReversionConfig::rsi_default("005930");
        let config_json = serde_json::to_value(config).unwrap();
        strategy.initialize(config_json).await.unwrap();

        let state = strategy.get_state();

        assert!(state.is_object());
        assert!(state.get("name").is_some());
    }

    #[tokio::test]
    async fn test_shutdown_returns_ok() {
        let mut strategy = MeanReversionStrategy::new();
        let config = MeanReversionConfig::bollinger_default("005930");
        let config_json = serde_json::to_value(config).unwrap();
        strategy.initialize(config_json).await.unwrap();

        let result = strategy.shutdown().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_strategy_name_reflects_variant() {
        let variants = [
            (
                MeanReversionConfig::rsi_default("TEST"),
                "MeanReversion-RSI",
            ),
            (
                MeanReversionConfig::bollinger_default("TEST"),
                "MeanReversion-Bollinger",
            ),
            (
                MeanReversionConfig::grid_default("TEST"),
                "MeanReversion-Grid",
            ),
            (
                MeanReversionConfig::magic_split_default("TEST"),
                "MeanReversion-MagicSplit",
            ),
        ];

        for (config, expected_name) in variants {
            let mut strategy = MeanReversionStrategy::new();
            let config_json = serde_json::to_value(config).unwrap();
            strategy.initialize(config_json).await.unwrap();
            assert_eq!(strategy.name(), expected_name);
        }
    }

    #[tokio::test]
    async fn test_ticker_data_type_works() {
        let mut strategy = MeanReversionStrategy::new();
        let config = MeanReversionConfig::rsi_default("005930");
        let config_json = serde_json::to_value(config).unwrap();
        strategy.initialize(config_json).await.unwrap();

        let data = create_ticker_data("005930", dec!(50000));
        let result = strategy.on_market_data(&data).await;

        assert!(result.is_ok());
    }
}

// ================================================================================================
// 에러 케이스 테스트
// ================================================================================================

mod error_tests {
    use super::*;

    #[tokio::test]
    async fn test_invalid_config_json_fails() {
        let mut strategy = MeanReversionStrategy::new();
        // 완전히 잘못된 타입 (문자열)은 파싱 실패해야 함
        let invalid_config = json!("not an object");

        let result = strategy.initialize(invalid_config).await;
        assert!(result.is_err(), "잘못된 설정은 에러를 반환해야 함");
    }

    #[tokio::test]
    async fn test_partial_config_uses_defaults() {
        // serde(default)가 적용되어 있으므로 부분 설정은 기본값으로 처리됨
        let mut strategy = MeanReversionStrategy::new();
        let partial_config = json!({
            "variant": "rsi"
            // 나머지 필드는 기본값 사용
        });

        let result = strategy.initialize(partial_config).await;
        // 기본값이 적용되어 성공해야 함
        assert!(result.is_ok(), "부분 설정은 기본값으로 처리되어야 함");
    }
}

// ================================================================================================
// 경계값 테스트
// ================================================================================================

mod boundary_tests {
    use super::*;

    #[tokio::test]
    async fn test_zero_price_handled() {
        let mut strategy = MeanReversionStrategy::new();
        let config = MeanReversionConfig::rsi_default("005930");
        let config_json = serde_json::to_value(config).unwrap();
        strategy.initialize(config_json).await.unwrap();

        let data = create_kline_data("005930", dec!(0));
        let result = strategy.on_market_data(&data).await;

        assert!(result.is_ok(), "0 가격도 에러 없이 처리해야 함");
    }

    #[tokio::test]
    async fn test_very_large_price_handled() {
        let mut strategy = MeanReversionStrategy::new();
        let config = MeanReversionConfig::bollinger_default("005930");
        let config_json = serde_json::to_value(config).unwrap();
        strategy.initialize(config_json).await.unwrap();

        let data = create_kline_data("005930", dec!(999999999999));
        let result = strategy.on_market_data(&data).await;

        assert!(result.is_ok(), "매우 큰 가격도 에러 없이 처리해야 함");
    }
}
