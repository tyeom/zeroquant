//! 전략의 트레이딩 시그널.
//!
//! 이 모듈은 전략이 생성하는 매매 신호 관련 타입을 정의합니다:
//! - `SignalType` - 신호 유형 (진입, 청산 등)
//! - `Signal` - 매매 신호 엔티티
//! - `SignalValidation` - 신호 검증 결과

use crate::domain::{RouteState, Side};
use crate::types::Symbol;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// 수행할 액션의 종류를 나타내는 신호 유형.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalType {
    /// 새 포지션 진입
    Entry,
    /// 기존 포지션 청산
    Exit,
    /// 알림 (실행하지 않음)
    Alert,
    /// 기존 포지션에 추가 (물타기)
    AddToPosition,
    /// 기존 포지션 축소 (부분 청산)
    ReducePosition,
    /// 스케일 인/아웃
    Scale,
}

impl std::fmt::Display for SignalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignalType::Entry => write!(f, "ENTRY"),
            SignalType::Exit => write!(f, "EXIT"),
            SignalType::Alert => write!(f, "ALERT"),
            SignalType::AddToPosition => write!(f, "ADD_TO_POSITION"),
            SignalType::ReducePosition => write!(f, "REDUCE_POSITION"),
            SignalType::Scale => write!(f, "SCALE"),
        }
    }
}

/// 전략이 생성한 트레이딩 신호.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    /// 고유 신호 ID
    pub id: Uuid,
    /// 이 신호를 생성한 전략
    pub strategy_id: String,
    /// 거래 심볼
    pub symbol: Symbol,
    /// 신호 방향 (매수/매도)
    pub side: Side,
    /// 신호 유형
    pub signal_type: SignalType,
    /// 신호 강도 (0.0 ~ 1.0)
    pub strength: f64,
    /// 제안 진입 가격 (선택)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_price: Option<rust_decimal::Decimal>,
    /// 제안 손절가 (선택)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_loss: Option<rust_decimal::Decimal>,
    /// 제안 익절가 (선택)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub take_profit: Option<rust_decimal::Decimal>,
    /// 신호 생성 타임스탬프
    pub timestamp: DateTime<Utc>,
    /// 추가 메타데이터
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Signal {
    /// 새 신호를 생성합니다.
    pub fn new(
        strategy_id: impl Into<String>,
        symbol: Symbol,
        side: Side,
        signal_type: SignalType,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            strategy_id: strategy_id.into(),
            symbol,
            side,
            signal_type,
            strength: 1.0,
            suggested_price: None,
            stop_loss: None,
            take_profit: None,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// 진입 신호를 생성합니다.
    pub fn entry(strategy_id: impl Into<String>, symbol: Symbol, side: Side) -> Self {
        Self::new(strategy_id, symbol, side, SignalType::Entry)
    }

    /// 청산 신호를 생성합니다.
    pub fn exit(strategy_id: impl Into<String>, symbol: Symbol, side: Side) -> Self {
        Self::new(strategy_id, symbol, side, SignalType::Exit)
    }

    /// 신호 강도를 설정합니다.
    pub fn with_strength(mut self, strength: f64) -> Self {
        self.strength = strength.clamp(0.0, 1.0);
        self
    }

    /// 제안 가격 수준을 설정합니다.
    pub fn with_prices(
        mut self,
        entry: Option<rust_decimal::Decimal>,
        stop_loss: Option<rust_decimal::Decimal>,
        take_profit: Option<rust_decimal::Decimal>,
    ) -> Self {
        self.suggested_price = entry;
        self.stop_loss = stop_loss;
        self.take_profit = take_profit;
        self
    }

    /// 메타데이터를 추가합니다.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// 강한 신호인지 확인합니다 (강도 >= 0.7).
    pub fn is_strong(&self) -> bool {
        self.strength >= 0.7
    }

    /// 진입 신호인지 확인합니다.
    pub fn is_entry(&self) -> bool {
        self.signal_type == SignalType::Entry
    }

    /// 청산 신호인지 확인합니다.
    pub fn is_exit(&self) -> bool {
        self.signal_type == SignalType::Exit
    }
}

/// 신호 검증 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalValidation {
    /// 신호 유효 여부
    pub is_valid: bool,
    /// 검증 메시지
    pub messages: Vec<String>,
    /// 수정된 신호 (조정이 이루어진 경우)
    pub modified_signal: Option<Signal>,
}

impl SignalValidation {
    /// 유효한 결과를 생성합니다.
    pub fn valid() -> Self {
        Self {
            is_valid: true,
            messages: vec![],
            modified_signal: None,
        }
    }

    /// 무효한 결과를 생성합니다.
    pub fn invalid(reason: impl Into<String>) -> Self {
        Self {
            is_valid: false,
            messages: vec![reason.into()],
            modified_signal: None,
        }
    }

    /// 메시지를 추가합니다.
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.messages.push(message.into());
        self
    }

    /// 수정된 신호를 설정합니다.
    pub fn with_modified_signal(mut self, signal: Signal) -> Self {
        self.modified_signal = Some(signal);
        self
    }
}

// ==================== SignalMarker (신호 마커) ====================

/// 기술 신호 마커 - 캔들 차트에 표시할 신호 정보.
///
/// Signal과 달리 SignalMarker는 백테스트와 실거래에서 발생한
/// 신호를 저장하고 분석하기 위한 확장된 정보를 포함합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalMarker {
    /// 고유 ID
    pub id: Uuid,
    /// 거래 심볼
    pub symbol: Symbol,
    /// 신호 발생 시각
    pub timestamp: DateTime<Utc>,
    /// 신호 유형 (Entry, Exit, Alert 등)
    pub signal_type: SignalType,
    /// 신호 방향 (매수/매도)
    pub side: Option<Side>,
    /// 신호 발생 시점 가격
    pub price: Decimal,
    /// 신호 강도 (0.0 ~ 1.0)
    pub strength: f64,

    /// 신호 생성에 사용된 지표 값들
    pub indicators: SignalIndicators,

    /// 신호 생성 이유 (사람이 읽을 수 있는 형태)
    pub reason: String,

    /// 전략 ID
    pub strategy_id: String,
    /// 전략 이름
    pub strategy_name: String,

    /// 실행 여부 (백테스트에서 실제 체결되었는지)
    pub executed: bool,

    /// 메타데이터 (확장용)
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl SignalMarker {
    /// 새 신호 마커 생성.
    pub fn new(
        symbol: Symbol,
        timestamp: DateTime<Utc>,
        signal_type: SignalType,
        price: Decimal,
        strategy_id: impl Into<String>,
        strategy_name: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            symbol,
            timestamp,
            signal_type,
            side: None,
            price,
            strength: 0.0,
            indicators: SignalIndicators::default(),
            reason: String::new(),
            strategy_id: strategy_id.into(),
            strategy_name: strategy_name.into(),
            executed: false,
            metadata: HashMap::new(),
        }
    }

    /// Signal로부터 SignalMarker 생성.
    pub fn from_signal(
        signal: &Signal,
        price: Decimal,
        timestamp: DateTime<Utc>,
        strategy_name: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            symbol: signal.symbol.clone(),
            timestamp,
            signal_type: signal.signal_type,
            side: Some(signal.side),
            price,
            strength: signal.strength,
            indicators: SignalIndicators::default(),
            reason: String::new(),
            strategy_id: signal.strategy_id.clone(),
            strategy_name: strategy_name.into(),
            executed: false,
            metadata: signal.metadata.clone(),
        }
    }

    /// 신호 방향 설정.
    pub fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    /// 신호 강도 설정.
    pub fn with_strength(mut self, strength: f64) -> Self {
        self.strength = strength.clamp(0.0, 1.0);
        self
    }

    /// 지표 정보 설정.
    pub fn with_indicators(mut self, indicators: SignalIndicators) -> Self {
        self.indicators = indicators;
        self
    }

    /// 신호 이유 설정.
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = reason.into();
        self
    }

    /// 실행 여부 설정.
    pub fn with_executed(mut self, executed: bool) -> Self {
        self.executed = executed;
        self
    }

    /// 메타데이터 추가.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// 강한 신호인지 확인 (강도 >= 0.8).
    pub fn is_strong(&self) -> bool {
        self.strength >= 0.8
    }

    /// 진입 신호인지 확인.
    pub fn is_entry(&self) -> bool {
        self.signal_type == SignalType::Entry
    }

    /// 청산 신호인지 확인.
    pub fn is_exit(&self) -> bool {
        self.signal_type == SignalType::Exit
    }
}

/// 신호 생성에 사용된 기술적 지표 값들.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa-support", derive(utoipa::ToSchema))]
pub struct SignalIndicators {
    // ===== 추세 지표 =====
    /// SMA (단기)
    pub sma_short: Option<Decimal>,
    /// SMA (장기)
    pub sma_long: Option<Decimal>,
    /// EMA (단기)
    pub ema_short: Option<Decimal>,
    /// EMA (장기)
    pub ema_long: Option<Decimal>,

    // ===== 모멘텀 지표 =====
    /// RSI (14일)
    pub rsi: Option<f64>,
    /// MACD
    pub macd: Option<Decimal>,
    /// MACD 시그널
    pub macd_signal: Option<Decimal>,
    /// MACD 히스토그램
    pub macd_histogram: Option<Decimal>,

    // ===== 변동성 지표 =====
    /// 볼린저 밴드 상단
    pub bb_upper: Option<Decimal>,
    /// 볼린저 밴드 중간
    pub bb_middle: Option<Decimal>,
    /// 볼린저 밴드 하단
    pub bb_lower: Option<Decimal>,
    /// ATR (Average True Range)
    pub atr: Option<Decimal>,

    // ===== TTM Squeeze =====
    /// Squeeze 상태 (압축 중)
    pub squeeze_on: Option<bool>,
    /// Squeeze 모멘텀
    pub squeeze_momentum: Option<Decimal>,

    // ===== 구조적 피처 =====
    /// RouteState (매매 단계)
    pub route_state: Option<RouteState>,
    /// 박스권 내 위치 (0.0 ~ 1.0)
    pub range_pos: Option<f64>,
    /// 거래량 품질
    pub vol_quality: Option<f64>,
    /// 돌파 점수
    pub breakout_score: Option<f64>,
}

impl SignalIndicators {
    /// 빈 지표 정보 생성.
    pub fn new() -> Self {
        Self::default()
    }

    /// RSI 설정.
    pub fn with_rsi(mut self, rsi: f64) -> Self {
        self.rsi = Some(rsi);
        self
    }

    /// MACD 설정.
    pub fn with_macd(mut self, macd: Decimal, signal: Decimal, histogram: Decimal) -> Self {
        self.macd = Some(macd);
        self.macd_signal = Some(signal);
        self.macd_histogram = Some(histogram);
        self
    }

    /// 볼린저 밴드 설정.
    pub fn with_bollinger_bands(mut self, upper: Decimal, middle: Decimal, lower: Decimal) -> Self {
        self.bb_upper = Some(upper);
        self.bb_middle = Some(middle);
        self.bb_lower = Some(lower);
        self
    }

    /// RouteState 설정.
    pub fn with_route_state(mut self, state: RouteState) -> Self {
        self.route_state = Some(state);
        self
    }

    /// TTM Squeeze 설정.
    pub fn with_squeeze(mut self, on: bool, momentum: Decimal) -> Self {
        self.squeeze_on = Some(on);
        self.squeeze_momentum = Some(momentum);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_creation() {
        let symbol = Symbol::crypto("BTC", "USDT");
        let signal = Signal::entry("grid_trading", symbol, Side::Buy)
            .with_strength(0.85)
            .with_metadata("reason", serde_json::json!("grid_level_hit"));

        assert_eq!(signal.strategy_id, "grid_trading");
        assert_eq!(signal.signal_type, SignalType::Entry);
        assert_eq!(signal.strength, 0.85);
        assert!(signal.is_strong());
        assert!(signal.is_entry());
    }

    #[test]
    fn test_signal_strength_clamping() {
        let symbol = Symbol::crypto("ETH", "USDT");
        let signal = Signal::exit("rsi_strategy", symbol, Side::Sell).with_strength(1.5);

        assert_eq!(signal.strength, 1.0);
    }

    #[test]
    fn test_signal_marker_creation() {
        use rust_decimal_macros::dec;

        let symbol = Symbol::crypto("BTC", "USDT");
        let marker = SignalMarker::new(
            symbol,
            Utc::now(),
            SignalType::Entry,
            dec!(50000),
            "rsi_strategy",
            "RSI 평균회귀",
        )
        .with_side(Side::Buy)
        .with_strength(0.9)
        .with_reason("RSI 과매도 (25)")
        .with_indicators(SignalIndicators::new().with_rsi(25.0));

        assert!(marker.is_strong());
        assert!(marker.is_entry());
        assert_eq!(marker.reason, "RSI 과매도 (25)");
        assert_eq!(marker.indicators.rsi, Some(25.0));
    }
}
