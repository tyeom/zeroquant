//! 전략의 트레이딩 시그널.
//!
//! 이 모듈은 전략이 생성하는 매매 신호 관련 타입을 정의합니다:
//! - `SignalType` - 신호 유형 (진입, 청산 등)
//! - `Signal` - 매매 신호 엔티티
//! - `SignalValidation` - 신호 검증 결과

use crate::domain::Side;
use crate::types::Symbol;
use chrono::{DateTime, Utc};
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
}
