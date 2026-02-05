//! 통합 평균회귀 전략.
//!
//! Grid Trading, RSI Mean Reversion, Bollinger Bands, Magic Split을
//! 단일 전략으로 통합합니다.
//!
//! # 지원 변형
//!
//! - `Rsi`: RSI 과매도/과매수 기반 평균회귀
//! - `Bollinger`: 볼린저 밴드 이탈 후 복귀
//! - `Grid`: 가격 대역 분할 자동 거래
//! - `MagicSplit`: 단계적 분할 매수
//!
//! # 공통 로직
//!
//! - `RouteState`: 진입 가능 여부 판단 (Armed, Attack만 허용)
//! - `GlobalScore`: 종목 품질 필터링
//! - 손절/익절: 설정된 비율로 자동 청산

use crate::Strategy;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use trader_strategy_macro::StrategyConfig;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use trader_core::domain::{RouteState, StrategyContext};
use trader_core::{MarketData, MarketDataType, Order, Position, Side, Signal, SignalType};

// ================================================================================================
// 설정 타입
// ================================================================================================

/// 전략 변형.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum StrategyVariant {
    /// RSI 평균회귀.
    #[default]
    Rsi,
    /// 볼린저 밴드.
    Bollinger,
    /// 그리드 트레이딩.
    Grid,
    /// 매직 분할.
    MagicSplit,
}


/// 진입 신호 타입.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EntrySignalConfig {
    /// RSI 기반 진입.
    Rsi {
        /// 과매도 임계값 (기본: 30).
        #[serde(default = "default_oversold")]
        oversold: Decimal,
        /// 과매수 임계값 (기본: 70).
        #[serde(default = "default_overbought")]
        overbought: Decimal,
        /// RSI 기간 (기본: 14).
        #[serde(default = "default_rsi_period")]
        period: usize,
    },
    /// 볼린저 밴드 기반 진입.
    Bollinger {
        /// SMA 기간 (기본: 20).
        #[serde(default = "default_bb_period")]
        period: usize,
        /// 표준편차 승수 (기본: 2.0).
        #[serde(default = "default_std_multiplier")]
        std_multiplier: Decimal,
        /// RSI 확인 사용 여부 (기본: true).
        #[serde(default = "default_true")]
        use_rsi_confirmation: bool,
        /// 최소 밴드폭 % (기본: 1.0).
        #[serde(default = "default_min_bandwidth")]
        min_bandwidth_pct: Decimal,
    },
    /// 그리드 기반 진입.
    Grid {
        /// 그리드 간격 % (기본: 1.0).
        #[serde(default = "default_grid_spacing")]
        spacing_pct: Decimal,
        /// 그리드 레벨 수 (기본: 5).
        #[serde(default = "default_grid_levels")]
        levels: usize,
        /// ATR 기반 동적 간격 사용 (기본: false).
        #[serde(default)]
        use_atr: bool,
        /// ATR 기간 (기본: 14).
        #[serde(default = "default_atr_period")]
        atr_period: usize,
    },
    /// 분할 매수 기반 진입.
    Split {
        /// 분할 레벨 정의.
        levels: Vec<SplitLevel>,
    },
}

impl Default for EntrySignalConfig {
    fn default() -> Self {
        Self::Rsi {
            oversold: dec!(30),
            overbought: dec!(70),
            period: 14,
        }
    }
}

/// 분할 매수 레벨 정의.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitLevel {
    /// 추가 매수 트리거 손실률 (예: -3%).
    pub trigger_rate: Decimal,
    /// 목표 수익률 (예: 5%).
    pub target_rate: Decimal,
    /// 투자 금액.
    pub amount: Decimal,
}

/// 청산 조건 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExitConfig {
    /// 손절 비율 % (기본: 2.0).
    #[serde(default = "default_stop_loss")]
    pub stop_loss_pct: Decimal,
    /// 익절 비율 % (기본: 4.0).
    #[serde(default = "default_take_profit")]
    pub take_profit_pct: Decimal,
    /// 중립점 청산 (RSI 50, 중간밴드).
    #[serde(default)]
    pub exit_on_neutral: bool,
    /// 쿨다운 캔들 수 (기본: 5).
    #[serde(default = "default_cooldown")]
    pub cooldown_candles: usize,
}

impl Default for ExitConfig {
    fn default() -> Self {
        Self {
            stop_loss_pct: dec!(2),
            take_profit_pct: dec!(4),
            exit_on_neutral: false,
            cooldown_candles: 5,
        }
    }
}

// ================================================================================================
// 전략별 UI Config (SDUI용)
// ================================================================================================

/// RSI 평균회귀 전략 설정.
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "rsi",
    name = "RSI 평균회귀",
    description = "RSI 과매수/과매도 구간에서 평균회귀 매매",
    category = "Intraday"
)]
pub struct RsiConfig {
    /// 거래 티커.
    #[serde(default = "default_ticker")]
    #[schema(label = "거래 종목", field_type = "symbol", default = "005930")]
    pub ticker: String,

    /// 거래 금액.
    #[serde(default = "default_amount")]
    #[schema(label = "거래 금액", field_type = "number", min = 10000, max = 100000000, default = 1000000)]
    pub amount: Decimal,

    /// RSI 기간.
    #[serde(default = "default_rsi_period")]
    #[schema(label = "RSI 기간", field_type = "integer", min = 2, max = 100, default = 14)]
    pub rsi_period: usize,

    /// 과매도 임계값.
    #[serde(default = "default_oversold")]
    #[schema(label = "과매도 임계값", field_type = "number", min = 0, max = 50, default = 30)]
    pub oversold: Decimal,

    /// 과매수 임계값.
    #[serde(default = "default_overbought")]
    #[schema(label = "과매수 임계값", field_type = "number", min = 50, max = 100, default = 70)]
    pub overbought: Decimal,

    /// 청산 설정.
    #[serde(default)]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,

    /// 최대 포지션 수.
    #[serde(default = "default_max_positions")]
    #[schema(label = "최대 포지션 수", field_type = "integer", min = 1, max = 10, default = 1)]
    pub max_positions: usize,

    /// 최소 GlobalScore.
    #[serde(default = "default_min_score")]
    #[schema(label = "최소 GlobalScore", field_type = "number", min = 0, max = 100, default = 50)]
    pub min_global_score: Decimal,
}

fn default_ticker() -> String {
    "005930".to_string()
}

impl From<RsiConfig> for MeanReversionConfig {
    fn from(cfg: RsiConfig) -> Self {
        Self {
            variant: StrategyVariant::Rsi,
            ticker: cfg.ticker,
            amount: cfg.amount,
            entry_signal: EntrySignalConfig::Rsi {
                oversold: cfg.oversold,
                overbought: cfg.overbought,
                period: cfg.rsi_period,
            },
            exit_config: cfg.exit_config,
            max_positions: cfg.max_positions,
            min_global_score: cfg.min_global_score,
        }
    }
}

/// 볼린저 밴드 전략 설정.
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "bollinger",
    name = "볼린저 밴드",
    description = "볼린저 밴드 상/하단 터치 시 평균회귀 매매",
    category = "Intraday"
)]
pub struct BollingerConfig {
    /// 거래 티커.
    #[serde(default = "default_ticker")]
    #[schema(label = "거래 종목", field_type = "symbol", default = "005930")]
    pub ticker: String,

    /// 거래 금액.
    #[serde(default = "default_amount")]
    #[schema(label = "거래 금액", field_type = "number", min = 10000, max = 100000000, default = 1000000)]
    pub amount: Decimal,

    /// 볼린저 밴드 기간.
    #[serde(default = "default_bb_period")]
    #[schema(label = "기간", field_type = "integer", min = 5, max = 100, default = 20)]
    pub period: usize,

    /// 표준편차 배수.
    #[serde(default = "default_std_multiplier")]
    #[schema(label = "표준편차 배수", field_type = "number", min = 0.5, max = 5, default = 2)]
    pub std_multiplier: Decimal,

    /// RSI 확인 사용 여부.
    #[serde(default = "default_true")]
    #[schema(label = "RSI 확인 사용", field_type = "boolean", default = true)]
    pub use_rsi_confirmation: bool,

    /// 최소 밴드폭 (%).
    #[serde(default = "default_min_bandwidth")]
    #[schema(label = "최소 밴드폭 (%)", field_type = "number", min = 0, max = 10, default = 1)]
    pub min_bandwidth_pct: Decimal,

    /// 청산 설정.
    #[serde(default)]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,

    /// 최대 포지션 수.
    #[serde(default = "default_max_positions")]
    #[schema(label = "최대 포지션 수", field_type = "integer", min = 1, max = 10, default = 1)]
    pub max_positions: usize,

    /// 최소 GlobalScore.
    #[serde(default = "default_min_score")]
    #[schema(label = "최소 GlobalScore", field_type = "number", min = 0, max = 100, default = 50)]
    pub min_global_score: Decimal,
}

impl From<BollingerConfig> for MeanReversionConfig {
    fn from(cfg: BollingerConfig) -> Self {
        Self {
            variant: StrategyVariant::Bollinger,
            ticker: cfg.ticker,
            amount: cfg.amount,
            entry_signal: EntrySignalConfig::Bollinger {
                period: cfg.period,
                std_multiplier: cfg.std_multiplier,
                use_rsi_confirmation: cfg.use_rsi_confirmation,
                min_bandwidth_pct: cfg.min_bandwidth_pct,
            },
            exit_config: cfg.exit_config,
            max_positions: cfg.max_positions,
            min_global_score: cfg.min_global_score,
        }
    }
}

/// 그리드 트레이딩 전략 설정.
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "grid",
    name = "그리드 트레이딩",
    description = "일정 가격 간격으로 매수/매도 주문 배치",
    category = "Realtime"
)]
pub struct GridTradingConfig {
    /// 거래 티커.
    #[serde(default = "default_ticker")]
    #[schema(label = "거래 종목", field_type = "symbol", default = "005930")]
    pub ticker: String,

    /// 거래 금액.
    #[serde(default = "default_amount")]
    #[schema(label = "거래 금액", field_type = "number", min = 10000, max = 100000000, default = 1000000)]
    pub amount: Decimal,

    /// 그리드 간격 (%).
    #[serde(default = "default_grid_spacing")]
    #[schema(label = "그리드 간격 (%)", field_type = "number", min = 0.1, max = 10, default = 1)]
    pub spacing_pct: Decimal,

    /// 그리드 레벨 수.
    #[serde(default = "default_grid_levels")]
    #[schema(label = "그리드 레벨 수", field_type = "integer", min = 1, max = 20, default = 5)]
    pub levels: usize,

    /// ATR 기반 동적 간격 사용.
    #[serde(default)]
    #[schema(label = "ATR 동적 간격", field_type = "boolean", default = false)]
    pub use_atr: bool,

    /// ATR 기간.
    #[serde(default = "default_atr_period")]
    #[schema(label = "ATR 기간", field_type = "integer", min = 5, max = 50, default = 14)]
    pub atr_period: usize,

    /// 청산 설정.
    #[serde(default)]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,

    /// 최대 포지션 수.
    #[serde(default = "default_grid_max_positions")]
    #[schema(label = "최대 포지션 수", field_type = "integer", min = 1, max = 20, default = 10)]
    pub max_positions: usize,

    /// 최소 GlobalScore.
    #[serde(default = "default_min_score")]
    #[schema(label = "최소 GlobalScore", field_type = "number", min = 0, max = 100, default = 50)]
    pub min_global_score: Decimal,
}

fn default_grid_max_positions() -> usize {
    10
}

impl From<GridTradingConfig> for MeanReversionConfig {
    fn from(cfg: GridTradingConfig) -> Self {
        Self {
            variant: StrategyVariant::Grid,
            ticker: cfg.ticker,
            amount: cfg.amount,
            entry_signal: EntrySignalConfig::Grid {
                spacing_pct: cfg.spacing_pct,
                levels: cfg.levels,
                use_atr: cfg.use_atr,
                atr_period: cfg.atr_period,
            },
            exit_config: cfg.exit_config,
            max_positions: cfg.max_positions,
            min_global_score: cfg.min_global_score,
        }
    }
}

/// 매직 분할매수 전략 설정.
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "magic_split",
    name = "매직 분할매수",
    description = "가격 구간별 분할 매수 및 목표 수익 시 청산",
    category = "Daily"
)]
pub struct MagicSplitConfig {
    /// 거래 티커.
    #[serde(default = "default_ticker")]
    #[schema(label = "거래 종목", field_type = "symbol", default = "005930")]
    pub ticker: String,

    /// 분할 매수 레벨.
    #[serde(default = "default_split_levels")]
    #[schema(label = "분할 레벨", skip)]
    pub levels: Vec<SplitLevel>,

    /// 청산 설정.
    #[serde(default)]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,

    /// 최대 포지션 수.
    #[serde(default = "default_split_max_positions")]
    #[schema(label = "최대 포지션 수", field_type = "integer", min = 1, max = 10, default = 5)]
    pub max_positions: usize,

    /// 최소 GlobalScore.
    #[serde(default = "default_min_score")]
    #[schema(label = "최소 GlobalScore", field_type = "number", min = 0, max = 100, default = 50)]
    pub min_global_score: Decimal,
}

fn default_split_levels() -> Vec<SplitLevel> {
    vec![
        SplitLevel { trigger_rate: dec!(0), target_rate: dec!(10), amount: dec!(100000) },
        SplitLevel { trigger_rate: dec!(-3), target_rate: dec!(8), amount: dec!(150000) },
        SplitLevel { trigger_rate: dec!(-5), target_rate: dec!(6), amount: dec!(200000) },
        SplitLevel { trigger_rate: dec!(-7), target_rate: dec!(5), amount: dec!(250000) },
        SplitLevel { trigger_rate: dec!(-10), target_rate: dec!(4), amount: dec!(300000) },
    ]
}

fn default_split_max_positions() -> usize {
    5
}

impl From<MagicSplitConfig> for MeanReversionConfig {
    fn from(cfg: MagicSplitConfig) -> Self {
        Self {
            variant: StrategyVariant::MagicSplit,
            ticker: cfg.ticker,
            amount: cfg.levels.first().map(|l| l.amount).unwrap_or(dec!(100000)),
            entry_signal: EntrySignalConfig::Split { levels: cfg.levels },
            exit_config: cfg.exit_config,
            max_positions: cfg.max_positions,
            min_global_score: cfg.min_global_score,
        }
    }
}

// ================================================================================================
// 내부용 통합 설정 (런타임에서 사용)
// ================================================================================================

/// 통합 평균회귀 전략 설정 (내부용).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeanReversionConfig {
    /// 전략 변형.
    pub variant: StrategyVariant,

    /// 거래 티커.
    pub ticker: String,

    /// 거래 금액.
    pub amount: Decimal,

    /// 진입 신호 설정.
    pub entry_signal: EntrySignalConfig,

    /// 청산 설정.
    pub exit_config: ExitConfig,

    /// 최대 포지션 수.
    pub max_positions: usize,

    /// 최소 GlobalScore.
    pub min_global_score: Decimal,
}

// 기본값 함수들
fn default_oversold() -> Decimal {
    dec!(30)
}
fn default_overbought() -> Decimal {
    dec!(70)
}
fn default_rsi_period() -> usize {
    14
}
fn default_bb_period() -> usize {
    20
}
fn default_std_multiplier() -> Decimal {
    dec!(2)
}
fn default_true() -> bool {
    true
}
fn default_min_bandwidth() -> Decimal {
    dec!(1)
}
fn default_grid_spacing() -> Decimal {
    dec!(1)
}
fn default_grid_levels() -> usize {
    5
}
fn default_atr_period() -> usize {
    14
}
fn default_stop_loss() -> Decimal {
    dec!(2)
}
fn default_take_profit() -> Decimal {
    dec!(4)
}
fn default_cooldown() -> usize {
    5
}
fn default_amount() -> Decimal {
    dec!(100000)
}
fn default_max_positions() -> usize {
    1
}
fn default_min_score() -> Decimal {
    dec!(50)
}

impl MeanReversionConfig {
    /// RSI 전략 기본 설정.
    pub fn rsi_default(ticker: &str) -> Self {
        Self {
            variant: StrategyVariant::Rsi,
            ticker: ticker.to_string(),
            amount: dec!(100000),
            entry_signal: EntrySignalConfig::Rsi {
                oversold: dec!(30),
                overbought: dec!(70),
                period: 14,
            },
            exit_config: ExitConfig {
                stop_loss_pct: dec!(2),
                take_profit_pct: dec!(4),
                exit_on_neutral: false,
                cooldown_candles: 5,
            },
            max_positions: 1,
            min_global_score: dec!(60),
        }
    }

    /// 볼린저 밴드 전략 기본 설정.
    pub fn bollinger_default(ticker: &str) -> Self {
        Self {
            variant: StrategyVariant::Bollinger,
            ticker: ticker.to_string(),
            amount: dec!(100000),
            entry_signal: EntrySignalConfig::Bollinger {
                period: 20,
                std_multiplier: dec!(2),
                use_rsi_confirmation: true,
                min_bandwidth_pct: dec!(1),
            },
            exit_config: ExitConfig {
                stop_loss_pct: dec!(2),
                take_profit_pct: dec!(4),
                exit_on_neutral: true, // 중간밴드 청산
                cooldown_candles: 5,
            },
            max_positions: 1,
            min_global_score: dec!(50),
        }
    }

    /// 그리드 전략 기본 설정.
    pub fn grid_default(ticker: &str) -> Self {
        Self {
            variant: StrategyVariant::Grid,
            ticker: ticker.to_string(),
            amount: dec!(100000),
            entry_signal: EntrySignalConfig::Grid {
                spacing_pct: dec!(1),
                levels: 5,
                use_atr: false,
                atr_period: 14,
            },
            exit_config: ExitConfig {
                stop_loss_pct: dec!(3),
                take_profit_pct: dec!(5),
                exit_on_neutral: false,
                cooldown_candles: 0, // 그리드는 쿨다운 없음
            },
            max_positions: 10, // 그리드는 다중 포지션
            min_global_score: dec!(50),
        }
    }

    /// 매직 분할 전략 기본 설정.
    pub fn magic_split_default(ticker: &str) -> Self {
        Self {
            variant: StrategyVariant::MagicSplit,
            ticker: ticker.to_string(),
            amount: dec!(100000),
            entry_signal: EntrySignalConfig::Split {
                levels: vec![
                    SplitLevel {
                        trigger_rate: dec!(0),
                        target_rate: dec!(10),
                        amount: dec!(100000),
                    },
                    SplitLevel {
                        trigger_rate: dec!(-3),
                        target_rate: dec!(8),
                        amount: dec!(150000),
                    },
                    SplitLevel {
                        trigger_rate: dec!(-5),
                        target_rate: dec!(6),
                        amount: dec!(200000),
                    },
                    SplitLevel {
                        trigger_rate: dec!(-7),
                        target_rate: dec!(5),
                        amount: dec!(250000),
                    },
                    SplitLevel {
                        trigger_rate: dec!(-10),
                        target_rate: dec!(4),
                        amount: dec!(300000),
                    },
                ],
            },
            exit_config: ExitConfig {
                stop_loss_pct: dec!(0),   // 분할 매수는 손절 없음
                take_profit_pct: dec!(0), // 레벨별 익절
                exit_on_neutral: false,
                cooldown_candles: 0,
            },
            max_positions: 5, // 레벨 수와 동일
            min_global_score: dec!(50),
        }
    }
}

// ================================================================================================
// 내부 상태
// ================================================================================================

/// 포지션 상태.
#[derive(Debug, Clone, Default)]
struct PositionState {
    /// 포지션 방향.
    side: Option<Side>,
    /// 진입가.
    entry_price: Decimal,
    /// 수량.
    quantity: Decimal,
    /// 진입 시각.
    entry_time: Option<DateTime<Utc>>,
}

/// 분할 레벨 상태.
#[derive(Debug, Clone, Default)]
struct SplitLevelState {
    /// 매수 여부.
    is_bought: bool,
    /// 진입가.
    entry_price: Decimal,
    /// 수량.
    quantity: Decimal,
}

/// 그리드 레벨 상태.
#[derive(Debug, Clone)]
struct GridLevel {
    /// 매수 가격 (이 가격에서 매수).
    buy_price: Decimal,
    /// 매도 가격 (매수 후 이 가격에서 매도).
    sell_price: Decimal,
    /// 레벨 상태.
    state: GridLevelState,
}

/// 그리드 레벨 상태.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GridLevelState {
    /// 매수 대기 (가격이 buy_price에 도달하면 매수).
    WaitingBuy,
    /// 매도 대기 (매수 완료, 가격이 sell_price에 도달하면 매도).
    WaitingSell,
}

/// 분할 매수 액션.
#[derive(Debug, Clone, Copy)]
enum SplitAction {
    /// 매수.
    Buy,
    /// 매도.
    Sell,
}

/// RSI 계산기.
#[derive(Debug, Clone, Default)]
struct RsiCalculator {
    gains: VecDeque<Decimal>,
    losses: VecDeque<Decimal>,
    prev_close: Option<Decimal>,
    period: usize,
}

impl RsiCalculator {
    fn new(period: usize) -> Self {
        Self {
            gains: VecDeque::with_capacity(period + 1),
            losses: VecDeque::with_capacity(period + 1),
            prev_close: None,
            period,
        }
    }

    fn update(&mut self, close: Decimal) -> Option<Decimal> {
        if let Some(prev) = self.prev_close {
            let change = close - prev;
            let gain = if change > Decimal::ZERO {
                change
            } else {
                Decimal::ZERO
            };
            let loss = if change < Decimal::ZERO {
                -change
            } else {
                Decimal::ZERO
            };

            self.gains.push_back(gain);
            self.losses.push_back(loss);

            while self.gains.len() > self.period {
                self.gains.pop_front();
            }
            while self.losses.len() > self.period {
                self.losses.pop_front();
            }
        }
        self.prev_close = Some(close);

        if self.gains.len() < self.period {
            return None;
        }

        let avg_gain: Decimal = self.gains.iter().sum::<Decimal>() / Decimal::from(self.period);
        let avg_loss: Decimal = self.losses.iter().sum::<Decimal>() / Decimal::from(self.period);

        if avg_loss == Decimal::ZERO {
            return Some(dec!(100));
        }

        let rs = avg_gain / avg_loss;
        let rsi = dec!(100) - (dec!(100) / (dec!(1) + rs));
        Some(rsi)
    }
}

/// 볼린저 밴드 계산기.
#[derive(Debug, Clone, Default)]
struct BollingerCalculator {
    prices: VecDeque<Decimal>,
    period: usize,
    std_multiplier: Decimal,
}

impl BollingerCalculator {
    fn new(period: usize, std_multiplier: Decimal) -> Self {
        Self {
            prices: VecDeque::with_capacity(period + 1),
            period,
            std_multiplier,
        }
    }

    fn update(&mut self, close: Decimal) -> Option<(Decimal, Decimal, Decimal)> {
        self.prices.push_back(close);
        while self.prices.len() > self.period {
            self.prices.pop_front();
        }

        if self.prices.len() < self.period {
            return None;
        }

        let sum: Decimal = self.prices.iter().sum();
        let sma = sum / Decimal::from(self.period);

        let variance: Decimal = self
            .prices
            .iter()
            .map(|p| (*p - sma) * (*p - sma))
            .sum::<Decimal>()
            / Decimal::from(self.period);

        // 간단한 제곱근 근사 (Newton's method)
        let std_dev = self.sqrt_approx(variance);

        let upper = sma + self.std_multiplier * std_dev;
        let lower = sma - self.std_multiplier * std_dev;

        Some((lower, sma, upper))
    }

    fn sqrt_approx(&self, n: Decimal) -> Decimal {
        if n <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        let mut x = n;
        for _ in 0..10 {
            x = (x + n / x) / dec!(2);
        }
        x
    }

    fn bandwidth(&self) -> Option<Decimal> {
        if self.prices.len() < self.period {
            return None;
        }

        // update()를 호출하지 않고 직접 계산 (immutable 유지)
        let sum: Decimal = self.prices.iter().sum();
        let sma = sum / Decimal::from(self.period);

        if sma == Decimal::ZERO {
            return None;
        }

        let variance: Decimal = self
            .prices
            .iter()
            .map(|p| (*p - sma) * (*p - sma))
            .sum::<Decimal>()
            / Decimal::from(self.period);

        let std_dev = self.sqrt_approx(variance);
        let upper = sma + self.std_multiplier * std_dev;
        let lower = sma - self.std_multiplier * std_dev;

        Some((upper - lower) / sma * dec!(100))
    }
}

// ================================================================================================
// 전략 구현
// ================================================================================================

/// 통합 평균회귀 전략.
pub struct MeanReversionStrategy {
    config: Option<MeanReversionConfig>,
    context: Option<Arc<RwLock<StrategyContext>>>,

    // 공통 상태
    prices: VecDeque<Decimal>,
    position: PositionState,
    cooldown_counter: usize,
    initialized: bool,

    // RSI 상태
    rsi_calculator: RsiCalculator,
    prev_rsi: Option<Decimal>,

    // 볼린저 상태
    bollinger_calculator: BollingerCalculator,

    // 그리드 상태
    grid_levels: Vec<GridLevel>,
    grid_base_price: Decimal,

    // 분할 매수 상태
    split_states: Vec<SplitLevelState>,
    split_entry_date: Option<String>,
}

impl MeanReversionStrategy {
    /// 새 전략 생성.
    pub fn new() -> Self {
        Self {
            config: None,
            context: None,
            prices: VecDeque::new(),
            position: PositionState::default(),
            cooldown_counter: 0,
            initialized: false,
            rsi_calculator: RsiCalculator::new(14),
            prev_rsi: None,
            bollinger_calculator: BollingerCalculator::new(20, dec!(2)),
            grid_levels: Vec::new(),
            grid_base_price: Decimal::ZERO,
            split_states: Vec::new(),
            split_entry_date: None,
        }
    }

    /// 설정으로 초기화된 전략 생성.
    pub fn with_config(config: MeanReversionConfig) -> Self {
        let mut strategy = Self::new();

        // RSI 계산기 초기화
        if let EntrySignalConfig::Rsi { period, .. } = &config.entry_signal {
            strategy.rsi_calculator = RsiCalculator::new(*period);
        }

        // 볼린저 계산기 초기화
        if let EntrySignalConfig::Bollinger {
            period,
            std_multiplier,
            ..
        } = &config.entry_signal
        {
            strategy.bollinger_calculator = BollingerCalculator::new(*period, *std_multiplier);
        }

        strategy.config = Some(config);
        strategy
    }

    /// RSI 평균회귀 전략 팩토리.
    pub fn rsi() -> Self {
        Self::with_config(MeanReversionConfig::rsi_default("BTCUSDT"))
    }

    /// 볼린저 밴드 전략 팩토리.
    pub fn bollinger() -> Self {
        Self::with_config(MeanReversionConfig::bollinger_default("BTCUSDT"))
    }

    /// 그리드 전략 팩토리.
    pub fn grid() -> Self {
        Self::with_config(MeanReversionConfig::grid_default("BTCUSDT"))
    }

    /// 매직 분할 전략 팩토리.
    pub fn magic_split() -> Self {
        Self::with_config(MeanReversionConfig::magic_split_default("BTCUSDT"))
    }

    /// StrategyContext 기반 진입 가능 여부 체크.
    fn can_enter(&self) -> bool {
        let Some(config) = self.config.as_ref() else {
            return false;
        };

        let Some(ctx) = self.context.as_ref() else {
            return true; // Context 없으면 진입 허용 (하위 호환성)
        };

        let Ok(ctx_lock) = ctx.try_read() else {
            return true;
        };

        // RouteState 체크
        if let Some(route_state) = ctx_lock.get_route_state(&config.ticker) {
            match route_state {
                RouteState::Overheat | RouteState::Wait | RouteState::Neutral => {
                    debug!(
                        ticker = %config.ticker,
                        route_state = ?route_state,
                        "RouteState 진입 제한"
                    );
                    return false;
                }
                RouteState::Armed | RouteState::Attack => {
                    // 진입 가능
                }
            }
        }

        // GlobalScore 체크
        if let Some(score) = ctx_lock.get_global_score(&config.ticker) {
            if score.overall_score < config.min_global_score {
                debug!(
                    ticker = %config.ticker,
                    score = %score.overall_score,
                    min_required = %config.min_global_score,
                    "GlobalScore 미달"
                );
                return false;
            }
        }

        true
    }

    /// 쿨다운 체크.
    fn is_in_cooldown(&self) -> bool {
        self.cooldown_counter > 0
    }

    /// 쿨다운 시작.
    fn start_cooldown(&mut self) {
        if let Some(config) = &self.config {
            self.cooldown_counter = config.exit_config.cooldown_candles;
        }
    }

    /// 쿨다운 감소.
    fn tick_cooldown(&mut self) {
        if self.cooldown_counter > 0 {
            self.cooldown_counter -= 1;
        }
    }

    /// 포지션 여부 체크.
    fn has_position(&self) -> bool {
        self.position.side.is_some() && self.position.quantity > Decimal::ZERO
    }

    /// RSI 신호 생성.
    fn generate_rsi_signals(&mut self, price: Decimal) -> Vec<Signal> {
        let Some(config) = self.config.as_ref() else {
            return vec![];
        };

        let EntrySignalConfig::Rsi {
            oversold,
            overbought,
            ..
        } = &config.entry_signal
        else {
            return vec![];
        };

        let Some(rsi) = self.rsi_calculator.update(price) else {
            return vec![];
        };

        let mut signals = vec![];

        // 포지션 없을 때 진입 체크
        if !self.has_position() && !self.is_in_cooldown() && self.can_enter() {
            // RSI 과매도 → 매수
            if rsi < *oversold {
                if let Some(prev) = self.prev_rsi {
                    if prev < *oversold && rsi > prev {
                        // RSI 상향 크로스
                        let strength = ((*oversold - rsi) / *oversold).to_f64().unwrap_or(0.5);
                        signals.push(
                            Signal::new(
                                "mean_reversion",
                                config.ticker.clone(),
                                Side::Buy,
                                SignalType::Entry,
                            )
                            .with_strength(strength)
                            .with_prices(Some(price), None, None)
                            .with_metadata("variant", json!("rsi"))
                            .with_metadata("rsi", json!(rsi.to_string())),
                        );
                    }
                }
            }
        }

        // 포지션 있을 때 청산 체크
        if self.has_position() && self.position.side == Some(Side::Buy) {
            let entry = self.position.entry_price;

            // 손절 체크
            let stop_price = entry * (dec!(1) - config.exit_config.stop_loss_pct / dec!(100));
            if price <= stop_price && config.exit_config.stop_loss_pct > Decimal::ZERO {
                signals.push(
                    Signal::new(
                        "mean_reversion",
                        config.ticker.clone(),
                        Side::Sell,
                        SignalType::Exit,
                    )
                    .with_strength(1.0)
                    .with_prices(Some(price), None, None)
                    .with_metadata("reason", json!("stop_loss")),
                );
            }
            // 익절 체크
            else {
                let target_price =
                    entry * (dec!(1) + config.exit_config.take_profit_pct / dec!(100));
                if price >= target_price && config.exit_config.take_profit_pct > Decimal::ZERO {
                    signals.push(
                        Signal::new(
                            "mean_reversion",
                            config.ticker.clone(),
                            Side::Sell,
                            SignalType::Exit,
                        )
                        .with_strength(1.0)
                        .with_prices(Some(price), None, None)
                        .with_metadata("reason", json!("take_profit")),
                    );
                }
                // RSI 과매수 청산
                else if rsi > *overbought {
                    signals.push(
                        Signal::new(
                            "mean_reversion",
                            config.ticker.clone(),
                            Side::Sell,
                            SignalType::Exit,
                        )
                        .with_strength(0.8)
                        .with_prices(Some(price), None, None)
                        .with_metadata("reason", json!("rsi_overbought")),
                    );
                }
                // 중립점 청산
                else if config.exit_config.exit_on_neutral && rsi >= dec!(50) {
                    signals.push(
                        Signal::new(
                            "mean_reversion",
                            config.ticker.clone(),
                            Side::Sell,
                            SignalType::Exit,
                        )
                        .with_strength(0.6)
                        .with_prices(Some(price), None, None)
                        .with_metadata("reason", json!("rsi_neutral")),
                    );
                }
            }
        }

        self.prev_rsi = Some(rsi);
        signals
    }

    /// 볼린저 밴드 신호 생성.
    fn generate_bollinger_signals(&mut self, price: Decimal) -> Vec<Signal> {
        let Some(config) = self.config.as_ref() else {
            return vec![];
        };

        let EntrySignalConfig::Bollinger {
            min_bandwidth_pct,
            use_rsi_confirmation,
            ..
        } = &config.entry_signal
        else {
            return vec![];
        };

        let Some((lower, middle, _upper)) = self.bollinger_calculator.update(price) else {
            return vec![];
        };

        // 밴드폭 체크 (스퀴즈 회피)
        let bandwidth = self
            .bollinger_calculator
            .bandwidth()
            .unwrap_or(Decimal::ZERO);
        if bandwidth < *min_bandwidth_pct {
            debug!(bandwidth = %bandwidth, "볼린저 스퀴즈 - 대기");
            return vec![];
        }

        let mut signals = vec![];

        // 포지션 없을 때 진입 체크
        if !self.has_position() && !self.is_in_cooldown() && self.can_enter() {
            // 하단밴드 터치 → 매수
            if price <= lower {
                // RSI 확인 (선택)
                let rsi_ok = if *use_rsi_confirmation {
                    self.prev_rsi.map(|r| r < dec!(30)).unwrap_or(false)
                } else {
                    true
                };

                if rsi_ok {
                    signals.push(
                        Signal::new(
                            "mean_reversion",
                            config.ticker.clone(),
                            Side::Buy,
                            SignalType::Entry,
                        )
                        .with_strength(0.8)
                        .with_prices(Some(price), None, None)
                        .with_metadata("variant", json!("bollinger"))
                        .with_metadata("lower_band", json!(lower.to_string())),
                    );
                }
            }
        }

        // 포지션 있을 때 청산 체크
        if self.has_position() && self.position.side == Some(Side::Buy) {
            let entry = self.position.entry_price;

            // 손절 체크
            let stop_price = entry * (dec!(1) - config.exit_config.stop_loss_pct / dec!(100));
            if price <= stop_price && config.exit_config.stop_loss_pct > Decimal::ZERO {
                signals.push(
                    Signal::new(
                        "mean_reversion",
                        config.ticker.clone(),
                        Side::Sell,
                        SignalType::Exit,
                    )
                    .with_strength(1.0)
                    .with_prices(Some(price), None, None)
                    .with_metadata("reason", json!("stop_loss")),
                );
            }
            // 익절 체크
            else {
                let target_price =
                    entry * (dec!(1) + config.exit_config.take_profit_pct / dec!(100));
                if price >= target_price && config.exit_config.take_profit_pct > Decimal::ZERO {
                    signals.push(
                        Signal::new(
                            "mean_reversion",
                            config.ticker.clone(),
                            Side::Sell,
                            SignalType::Exit,
                        )
                        .with_strength(1.0)
                        .with_prices(Some(price), None, None)
                        .with_metadata("reason", json!("take_profit")),
                    );
                }
                // 중간밴드 청산
                else if config.exit_config.exit_on_neutral && price >= middle {
                    signals.push(
                        Signal::new(
                            "mean_reversion",
                            config.ticker.clone(),
                            Side::Sell,
                            SignalType::Exit,
                        )
                        .with_strength(0.7)
                        .with_prices(Some(price), None, None)
                        .with_metadata("reason", json!("middle_band")),
                    );
                }
            }
        }

        signals
    }

    /// 그리드 신호 생성.
    ///
    /// # 핵심 로직
    ///
    /// 1. **매수 대기 상태** (WaitingBuy): 가격이 buy_price 이하 → 매수 신호, WaitingSell로 전환
    /// 2. **매도 대기 상태** (WaitingSell): 가격이 sell_price 이상 → 매도 신호, WaitingBuy로 전환
    /// 3. 이 사이클이 무한 반복되어 그리드 트레이딩 실현
    fn generate_grid_signals(&mut self, price: Decimal) -> Vec<Signal> {
        // config에서 필요한 값들을 먼저 추출
        let grid_params = match self.config.as_ref() {
            Some(config) => {
                if let EntrySignalConfig::Grid {
                    spacing_pct,
                    levels,
                    ..
                } = &config.entry_signal
                {
                    Some((config.ticker.clone(), *spacing_pct, *levels))
                } else {
                    None
                }
            }
            None => None,
        };

        let Some((ticker, spacing_pct, levels)) = grid_params else {
            return vec![];
        };

        // 그리드 초기화 (첫 진입 시에만)
        if self.grid_base_price == Decimal::ZERO {
            self.initialize_grid(price, spacing_pct, levels);
        }
        // 참고: 기존 5% 이탈 시 리셋 로직 제거 - 그리드는 계속 유지되어야 함

        let mut signals = vec![];

        // 신호 생성을 위한 레벨 상태 수집
        let mut updates: Vec<(usize, Side, Decimal)> = vec![];

        for (i, level) in self.grid_levels.iter().enumerate() {
            match level.state {
                GridLevelState::WaitingBuy => {
                    // 가격이 매수 가격 이하로 하락 → 매수 신호
                    if price <= level.buy_price {
                        // 매수 시에만 can_enter() 체크 (매도는 항상 허용)
                        if self.can_enter() {
                            updates.push((i, Side::Buy, level.buy_price));
                        }
                    }
                }
                GridLevelState::WaitingSell => {
                    // 가격이 매도 가격 이상으로 상승 → 매도 신호
                    // 매도(청산)는 can_enter() 체크 없이 항상 실행
                    if price >= level.sell_price {
                        updates.push((i, Side::Sell, level.sell_price));
                    }
                }
            }
        }

        // 신호 생성 및 상태 전환
        for (i, side, level_price) in updates {
            let signal_type = if side == Side::Buy {
                SignalType::Entry
            } else {
                SignalType::Exit
            };
            signals.push(
                Signal::new("mean_reversion", ticker.clone(), side, signal_type)
                    .with_strength(0.7)
                    .with_prices(Some(price), None, None)
                    .with_metadata("variant", json!("grid"))
                    .with_metadata("grid_level", json!(level_price.to_string())),
            );

            // 상태 전환 (핵심: 매수↔매도 사이클)
            match side {
                Side::Buy => {
                    self.grid_levels[i].state = GridLevelState::WaitingSell;
                    debug!(
                        level = i,
                        buy_price = %level_price,
                        sell_target = %self.grid_levels[i].sell_price,
                        "그리드 매수 → 매도 대기로 전환"
                    );
                }
                Side::Sell => {
                    self.grid_levels[i].state = GridLevelState::WaitingBuy;
                    debug!(
                        level = i,
                        sell_price = %level_price,
                        buy_target = %self.grid_levels[i].buy_price,
                        "그리드 매도 → 매수 대기로 재활성화"
                    );
                }
            }
        }

        signals
    }

    /// 그리드 초기화.
    ///
    /// # 그리드 트레이딩 핵심 로직
    ///
    /// - 기준가 아래에 N개의 매수 레벨 생성
    /// - 각 매수 레벨은 대응하는 매도 가격을 가짐 (매수가 + 간격)
    /// - 매수 실행 → 해당 레벨은 WaitingSell 상태로 전환
    /// - 매도 실행 → 해당 레벨은 WaitingBuy 상태로 재활성화 (무한 반복)
    fn initialize_grid(&mut self, base_price: Decimal, spacing_pct: Decimal, levels: usize) {
        self.grid_base_price = base_price;
        self.grid_levels.clear();

        let spacing = base_price * spacing_pct / dec!(100);

        // 매수/매도 쌍 레벨 생성
        // Level 1: buy at (base - spacing), sell at base
        // Level 2: buy at (base - 2*spacing), sell at (base - spacing)
        // ...
        for i in 1..=levels {
            let buy_price = base_price - spacing * Decimal::from(i as i32);
            let sell_price = base_price - spacing * Decimal::from(i as i32 - 1);

            self.grid_levels.push(GridLevel {
                buy_price,
                sell_price,
                state: GridLevelState::WaitingBuy,
            });
        }

        info!(
            base_price = %base_price,
            spacing_pct = %spacing_pct,
            levels = levels,
            grid_levels = ?self.grid_levels.len(),
            "그리드 초기화 - 쌍 기반"
        );
    }

    /// 분할 매수 신호 생성.
    fn generate_split_signals(&mut self, price: Decimal, timestamp: DateTime<Utc>) -> Vec<Signal> {
        // config에서 필요한 값들을 먼저 추출
        let split_params = match self.config.as_ref() {
            Some(config) => {
                if let EntrySignalConfig::Split { levels } = &config.entry_signal {
                    Some((config.ticker.clone(), levels.clone()))
                } else {
                    None
                }
            }
            None => None,
        };

        let Some((ticker, levels)) = split_params else {
            return vec![];
        };

        // 상태 초기화
        if self.split_states.is_empty() {
            self.split_states = vec![SplitLevelState::default(); levels.len()];
        }

        // 당일 체크
        let today = format!("{}", timestamp.format("%Y-%m-%d"));
        if let Some(ref entry_date) = self.split_entry_date {
            if entry_date == &today && self.all_split_sold() {
                return vec![]; // 당일 재진입 불가
            }
        }

        if !self.can_enter() {
            return vec![];
        }

        // 먼저 어떤 레벨이 진입/청산해야 하는지 계산 (빌림 없이)
        let mut actions: Vec<(usize, SplitAction, Decimal)> = vec![];

        for (i, level) in levels.iter().enumerate() {
            let is_bought = self.split_states[i].is_bought;
            let entry_price = self.split_states[i].entry_price;

            if !is_bought {
                // 진입 체크
                let should_buy = if i == 0 {
                    true
                } else {
                    let prev_is_bought = self.split_states[i - 1].is_bought;
                    let prev_entry_price = self.split_states[i - 1].entry_price;
                    if prev_is_bought && prev_entry_price > Decimal::ZERO {
                        let loss_rate = (price - prev_entry_price) / prev_entry_price * dec!(100);
                        loss_rate <= level.trigger_rate
                    } else {
                        false
                    }
                };

                if should_buy {
                    actions.push((i, SplitAction::Buy, level.amount));
                }
            } else {
                // 익절 체크
                if entry_price > Decimal::ZERO {
                    let profit_rate = (price - entry_price) / entry_price * dec!(100);
                    if profit_rate >= level.target_rate {
                        actions.push((i, SplitAction::Sell, profit_rate));
                    }
                }
            }
        }

        // 신호 생성 및 상태 업데이트
        let mut signals = vec![];
        for (i, action, value) in actions {
            match action {
                SplitAction::Buy => {
                    signals.push(
                        Signal::new(
                            "mean_reversion",
                            ticker.clone(),
                            Side::Buy,
                            SignalType::Entry,
                        )
                        .with_strength(0.8)
                        .with_prices(Some(price), None, None)
                        .with_metadata("variant", json!("split"))
                        .with_metadata("level", json!(i + 1))
                        .with_metadata("amount", json!(value.to_string())),
                    );
                    self.split_states[i].is_bought = true;
                    self.split_states[i].entry_price = price;
                    self.split_states[i].quantity = value / price;

                    if i == 0 {
                        self.split_entry_date = Some(today.clone());
                    }
                }
                SplitAction::Sell => {
                    signals.push(
                        Signal::new(
                            "mean_reversion",
                            ticker.clone(),
                            Side::Sell,
                            SignalType::Exit,
                        )
                        .with_strength(0.9)
                        .with_prices(Some(price), None, None)
                        .with_metadata("variant", json!("split"))
                        .with_metadata("level", json!(i + 1))
                        .with_metadata("profit_rate", json!(value.to_string())),
                    );
                    self.split_states[i].is_bought = false;
                    self.split_states[i].entry_price = Decimal::ZERO;
                    self.split_states[i].quantity = Decimal::ZERO;
                }
            }
        }

        signals
    }

    /// 모든 분할 레벨이 매도되었는지 체크.
    fn all_split_sold(&self) -> bool {
        self.split_states.iter().all(|s| !s.is_bought)
    }
}

impl Default for MeanReversionStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for MeanReversionStrategy {
    fn name(&self) -> &str {
        match self.config.as_ref().map(|c| c.variant) {
            Some(StrategyVariant::Rsi) => "MeanReversion-RSI",
            Some(StrategyVariant::Bollinger) => "MeanReversion-Bollinger",
            Some(StrategyVariant::Grid) => "MeanReversion-Grid",
            Some(StrategyVariant::MagicSplit) => "MeanReversion-MagicSplit",
            None => "MeanReversion",
        }
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "통합 평균회귀 전략 (RSI, Bollinger, Grid, MagicSplit)"
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 1. config에서 variant 확인 (테스트 등에서 직접 JSON 전달 시)
        // 2. self.config에서 variant 확인 (팩토리 메서드 사용 시)
        // 3. 기본값: Rsi
        // Note: serde rename_all="snake_case" 적용됨
        let variant = config
            .get("variant")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "rsi" | "Rsi" => Some(StrategyVariant::Rsi),
                "bollinger" | "Bollinger" => Some(StrategyVariant::Bollinger),
                "grid" | "Grid" => Some(StrategyVariant::Grid),
                "magic_split" | "MagicSplit" => Some(StrategyVariant::MagicSplit),
                _ => None,
            })
            .or_else(|| self.config.as_ref().map(|c| c.variant))
            .unwrap_or_default();

        // variant에 맞는 Config로 파싱 후 MeanReversionConfig로 변환
        let mr_config: MeanReversionConfig = match variant {
            StrategyVariant::Rsi => {
                let cfg: RsiConfig = serde_json::from_value(config)?;
                cfg.into()
            }
            StrategyVariant::Bollinger => {
                let cfg: BollingerConfig = serde_json::from_value(config)?;
                cfg.into()
            }
            StrategyVariant::Grid => {
                let cfg: GridTradingConfig = serde_json::from_value(config)?;
                cfg.into()
            }
            StrategyVariant::MagicSplit => {
                let cfg: MagicSplitConfig = serde_json::from_value(config)?;
                cfg.into()
            }
        };

        info!(
            variant = ?mr_config.variant,
            ticker = %mr_config.ticker,
            "[MeanReversion] 전략 초기화"
        );

        // 변형별 계산기 초기화
        match &mr_config.entry_signal {
            EntrySignalConfig::Rsi { period, .. } => {
                self.rsi_calculator = RsiCalculator::new(*period);
            }
            EntrySignalConfig::Bollinger {
                period,
                std_multiplier,
                ..
            } => {
                self.bollinger_calculator = BollingerCalculator::new(*period, *std_multiplier);
                self.rsi_calculator = RsiCalculator::new(14); // RSI 확인용
            }
            EntrySignalConfig::Grid { levels, .. } => {
                // 쌍 기반 레벨이므로 levels 개수만큼만 필요
                self.grid_levels = Vec::with_capacity(*levels);
            }
            EntrySignalConfig::Split { levels } => {
                self.split_states = vec![SplitLevelState::default(); levels.len()];
            }
        }

        self.config = Some(mr_config);
        self.initialized = true;

        Ok(())
    }

    async fn on_market_data(
        &mut self,
        data: &MarketData,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
        if !self.initialized {
            return Ok(vec![]);
        }

        // config에서 필요한 값들을 먼저 추출
        let (ticker, variant) = match self.config.as_ref() {
            Some(config) => (config.ticker.clone(), config.variant),
            None => return Ok(vec![]),
        };

        // 티커 확인
        if data.ticker != ticker {
            return Ok(vec![]);
        }

        // 가격 추출
        let price = match &data.data {
            MarketDataType::Kline(kline) => kline.close,
            MarketDataType::Ticker(ticker_data) => ticker_data.last,
            MarketDataType::Trade(trade) => trade.price,
            _ => return Ok(vec![]),
        };

        // 가격 히스토리 업데이트
        self.prices.push_back(price);
        if self.prices.len() > 300 {
            self.prices.pop_front();
        }

        // 쿨다운 감소
        self.tick_cooldown();

        // RSI 업데이트 (볼린저에서도 사용)
        let _ = self.rsi_calculator.update(price);

        // 변형별 신호 생성
        let signals = match variant {
            StrategyVariant::Rsi => self.generate_rsi_signals(price),
            StrategyVariant::Bollinger => self.generate_bollinger_signals(price),
            StrategyVariant::Grid => self.generate_grid_signals(price),
            StrategyVariant::MagicSplit => self.generate_split_signals(price, data.timestamp),
        };

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "[MeanReversion] 주문 체결: {:?} {} @ {:?}",
            order.side, order.quantity, order.average_fill_price
        );

        // 포지션 업데이트
        if let Some(fill_price) = order.average_fill_price {
            match order.side {
                Side::Buy => {
                    self.position.side = Some(Side::Buy);
                    self.position.entry_price = fill_price;
                    self.position.quantity += order.quantity;
                    self.position.entry_time = Some(Utc::now());
                }
                Side::Sell => {
                    self.position.quantity -= order.quantity;
                    if self.position.quantity <= Decimal::ZERO {
                        self.position = PositionState::default();
                        self.start_cooldown();
                    }
                }
            }
        }

        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(config) = self.config.as_ref() else {
            return Ok(());
        };

        if position.ticker != config.ticker {
            return Ok(());
        }

        // 외부 포지션 동기화
        if position.quantity > Decimal::ZERO {
            self.position.side = Some(position.side);
            self.position.entry_price = position.entry_price;
            self.position.quantity = position.quantity;
        } else {
            self.position = PositionState::default();
        }

        info!(
            "[MeanReversion] 포지션 업데이트: {} = {}",
            position.ticker, position.quantity
        );

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("[MeanReversion] 전략 종료");
        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "name": self.name(),
            "version": self.version(),
            "variant": self.config.as_ref().map(|c| format!("{:?}", c.variant)),
            "initialized": self.initialized,
            "has_position": self.has_position(),
            "position": {
                "side": self.position.side.map(|s| format!("{:?}", s)),
                "entry_price": self.position.entry_price.to_string(),
                "quantity": self.position.quantity.to_string(),
            },
            "cooldown": self.cooldown_counter,
            "prev_rsi": self.prev_rsi.map(|r| r.to_string()),
        })
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("[MeanReversion] StrategyContext 주입 완료");
    }
}

// ================================================================================================
// 전략 레지스트리 등록
// ================================================================================================

use crate::register_strategy;

// RSI 평균회귀 전략
register_strategy! {
    id: "rsi",
    aliases: ["rsi_mean_reversion", "rsi_strategy"],
    name: "RSI 평균회귀",
    description: "RSI 과매수/과매도 구간에서 평균회귀 매매",
    timeframe: "15m",
    tickers: [],
    category: Intraday,
    markets: [Crypto, Stock],
    factory: MeanReversionStrategy::rsi,
    config: RsiConfig
}

// 볼린저 밴드 전략
register_strategy! {
    id: "bollinger",
    aliases: ["bollinger_bands", "bb_strategy"],
    name: "볼린저 밴드",
    description: "볼린저 밴드 상/하단 터치 시 평균회귀 매매",
    timeframe: "15m",
    tickers: [],
    category: Intraday,
    markets: [Crypto, Stock],
    factory: MeanReversionStrategy::bollinger,
    config: BollingerConfig
}

// 그리드 전략
register_strategy! {
    id: "grid",
    aliases: ["grid_trading", "grid_strategy"],
    name: "그리드 트레이딩",
    description: "일정 가격 간격으로 매수/매도 주문 배치",
    timeframe: "1m",
    tickers: [],
    category: Realtime,
    markets: [Crypto, Stock],
    factory: MeanReversionStrategy::grid,
    config: GridTradingConfig
}

// 매직 분할 전략
register_strategy! {
    id: "magic_split",
    aliases: ["split_entry", "pyramid"],
    name: "매직 분할매수",
    description: "가격 구간별 분할 매수 및 목표 수익 시 청산",
    timeframe: "1d",
    tickers: [],
    category: Daily,
    markets: [Crypto, Stock],
    factory: MeanReversionStrategy::magic_split,
    config: MagicSplitConfig
}

// 통합 테스트는 tests/mean_reversion_test.rs에서 수행
