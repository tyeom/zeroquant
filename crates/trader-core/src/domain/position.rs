//! 포지션 추적 및 관리.
//!
//! 이 모듈은 트레이딩 포지션 관련 타입을 정의합니다:
//! - `Position` - 개별 포지션 엔티티
//! - `PositionSummary` - 포트폴리오 요약

use crate::domain::Side;
use crate::types::{Price, Quantity, Symbol};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 심볼의 보유량을 나타내는 트레이딩 포지션.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    /// 내부 포지션 ID
    pub id: Uuid,
    /// 거래소 이름
    pub exchange: String,
    /// 거래 심볼
    pub symbol: Symbol,
    /// 포지션 방향 (롱 = Buy, 숏 = Sell)
    pub side: Side,
    /// 현재 보유 수량
    pub quantity: Quantity,
    /// 평균 진입 가격
    pub entry_price: Price,
    /// 현재 시장 가격
    pub current_price: Price,
    /// 미실현 손익
    pub unrealized_pnl: Decimal,
    /// 실현 손익
    pub realized_pnl: Decimal,
    /// 이 포지션을 연 전략
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy_id: Option<String>,
    /// 포지션 오픈 타임스탬프
    pub opened_at: DateTime<Utc>,
    /// 마지막 업데이트 타임스탬프
    pub updated_at: DateTime<Utc>,
    /// 포지션 종료 타임스탬프 (오픈 상태면 None)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<DateTime<Utc>>,
    /// 추가 메타데이터
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl Position {
    /// 새 포지션을 생성합니다.
    pub fn new(
        exchange: impl Into<String>,
        symbol: Symbol,
        side: Side,
        quantity: Quantity,
        entry_price: Price,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            exchange: exchange.into(),
            symbol,
            side,
            quantity,
            entry_price,
            current_price: entry_price,
            unrealized_pnl: Decimal::ZERO,
            realized_pnl: Decimal::ZERO,
            strategy_id: None,
            opened_at: now,
            updated_at: now,
            closed_at: None,
            metadata: serde_json::Value::Null,
        }
    }

    /// 전략 ID를 설정합니다.
    pub fn with_strategy(mut self, strategy_id: impl Into<String>) -> Self {
        self.strategy_id = Some(strategy_id.into());
        self
    }

    /// 현재 가격을 업데이트하고 손익을 재계산합니다.
    pub fn update_price(&mut self, current_price: Price) {
        self.current_price = current_price;
        self.calculate_unrealized_pnl();
        self.updated_at = Utc::now();
    }

    /// 현재 가격을 기반으로 미실현 손익을 계산합니다.
    fn calculate_unrealized_pnl(&mut self) {
        let price_diff = match self.side {
            Side::Buy => self.current_price - self.entry_price,
            Side::Sell => self.entry_price - self.current_price,
        };
        self.unrealized_pnl = price_diff * self.quantity;
    }

    /// 포지션의 명목 가치를 반환합니다.
    pub fn notional_value(&self) -> Decimal {
        self.current_price * self.quantity
    }

    /// 진입 시점의 명목 가치를 반환합니다.
    pub fn entry_notional_value(&self) -> Decimal {
        self.entry_price * self.quantity
    }

    /// 수익률(%)을 반환합니다.
    pub fn return_pct(&self) -> Decimal {
        if self.entry_price.is_zero() {
            return Decimal::ZERO;
        }
        (self.unrealized_pnl / self.entry_notional_value()) * Decimal::from(100)
    }

    /// 포지션이 오픈 상태인지 확인합니다.
    pub fn is_open(&self) -> bool {
        self.closed_at.is_none() && self.quantity > Decimal::ZERO
    }

    /// 포지션이 종료되었는지 확인합니다.
    pub fn is_closed(&self) -> bool {
        self.closed_at.is_some() || self.quantity.is_zero()
    }

    /// 포지션에 추가합니다 (물타기).
    pub fn add(&mut self, quantity: Quantity, price: Price) {
        let total_cost = (self.entry_price * self.quantity) + (price * quantity);
        self.quantity += quantity;
        if !self.quantity.is_zero() {
            self.entry_price = total_cost / self.quantity;
        }
        self.update_price(self.current_price);
    }

    /// 포지션을 줄입니다 (부분 청산).
    pub fn reduce(&mut self, quantity: Quantity, price: Price) -> Decimal {
        let reduce_qty = quantity.min(self.quantity);
        let pnl = match self.side {
            Side::Buy => (price - self.entry_price) * reduce_qty,
            Side::Sell => (self.entry_price - price) * reduce_qty,
        };

        self.quantity -= reduce_qty;
        self.realized_pnl += pnl;
        self.updated_at = Utc::now();

        if self.quantity.is_zero() {
            self.closed_at = Some(Utc::now());
        }

        self.update_price(self.current_price);
        pnl
    }

    /// 전체 포지션을 종료합니다.
    pub fn close(&mut self, price: Price) -> Decimal {
        self.reduce(self.quantity, price)
    }
}

/// 포트폴리오 개요를 위한 포지션 요약.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSummary {
    /// 오픈 포지션 총 개수
    pub total_positions: usize,
    /// 총 미실현 손익
    pub total_unrealized_pnl: Decimal,
    /// 총 실현 손익
    pub total_realized_pnl: Decimal,
    /// 총 명목 가치
    pub total_notional_value: Decimal,
    /// 롱 포지션 개수
    pub long_count: usize,
    /// 숏 포지션 개수
    pub short_count: usize,
}

impl PositionSummary {
    /// 포지션 목록으로부터 요약을 생성합니다.
    pub fn from_positions(positions: &[Position]) -> Self {
        let open_positions: Vec<_> = positions.iter().filter(|p| p.is_open()).collect();

        Self {
            total_positions: open_positions.len(),
            total_unrealized_pnl: open_positions.iter().map(|p| p.unrealized_pnl).sum(),
            total_realized_pnl: positions.iter().map(|p| p.realized_pnl).sum(),
            total_notional_value: open_positions.iter().map(|p| p.notional_value()).sum(),
            long_count: open_positions.iter().filter(|p| p.side == Side::Buy).count(),
            short_count: open_positions.iter().filter(|p| p.side == Side::Sell).count(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_position_pnl() {
        let symbol = Symbol::crypto("BTC", "USDT");
        let mut position = Position::new("binance", symbol, Side::Buy, dec!(1.0), dec!(50000));

        position.update_price(dec!(55000));
        assert_eq!(position.unrealized_pnl, dec!(5000));

        position.update_price(dec!(48000));
        assert_eq!(position.unrealized_pnl, dec!(-2000));
    }

    #[test]
    fn test_position_add() {
        let symbol = Symbol::crypto("ETH", "USDT");
        let mut position = Position::new("binance", symbol, Side::Buy, dec!(1.0), dec!(2000));

        // Add more at a higher price
        position.add(dec!(1.0), dec!(2200));

        // Average entry should be 2100
        assert_eq!(position.quantity, dec!(2.0));
        assert_eq!(position.entry_price, dec!(2100));
    }

    #[test]
    fn test_position_reduce() {
        let symbol = Symbol::crypto("BTC", "USDT");
        let mut position = Position::new("binance", symbol, Side::Buy, dec!(2.0), dec!(50000));

        let pnl = position.reduce(dec!(1.0), dec!(55000));
        assert_eq!(pnl, dec!(5000));
        assert_eq!(position.quantity, dec!(1.0));
        assert_eq!(position.realized_pnl, dec!(5000));
    }
}
