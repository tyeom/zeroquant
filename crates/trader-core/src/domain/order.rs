//! 주문 타입 및 관리.
//!
//! 이 모듈은 트레이딩 시스템의 주문 관련 타입을 정의합니다:
//! - `Side` - 주문 방향 (매수/매도)
//! - `OrderType` - 주문 유형 (시장가, 지정가 등)
//! - `OrderStatusType` - 주문 상태
//! - `TimeInForce` - 주문 유효 기간
//! - `OrderRequest` - 주문 요청
//! - `Order` - 주문 엔티티

use crate::types::{Price, Quantity, Symbol};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 주문 방향 (매수 또는 매도).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    /// 매수
    Buy,
    /// 매도
    Sell,
}

impl Side {
    /// 반대 방향을 반환합니다.
    pub fn opposite(&self) -> Self {
        match self {
            Side::Buy => Side::Sell,
            Side::Sell => Side::Buy,
        }
    }
}

impl std::fmt::Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Side::Buy => write!(f, "BUY"),
            Side::Sell => write!(f, "SELL"),
        }
    }
}

/// 주문 유형.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderType {
    /// 시장가 주문 - 현재 시장 가격으로 즉시 체결
    Market,
    /// 지정가 주문 - 지정 가격 이상/이하에서 체결
    Limit,
    /// 손절 주문
    StopLoss,
    /// 지정가 손절 주문
    StopLossLimit,
    /// 익절 주문
    TakeProfit,
    /// 지정가 익절 주문
    TakeProfitLimit,
    /// 트레일링 스톱 주문
    TrailingStop,
}

impl std::fmt::Display for OrderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderType::Market => write!(f, "MARKET"),
            OrderType::Limit => write!(f, "LIMIT"),
            OrderType::StopLoss => write!(f, "STOP_LOSS"),
            OrderType::StopLossLimit => write!(f, "STOP_LOSS_LIMIT"),
            OrderType::TakeProfit => write!(f, "TAKE_PROFIT"),
            OrderType::TakeProfitLimit => write!(f, "TAKE_PROFIT_LIMIT"),
            OrderType::TrailingStop => write!(f, "TRAILING_STOP"),
        }
    }
}

/// 주문 상태 유형.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatusType {
    /// 주문 생성됨 (아직 제출되지 않음)
    Pending,
    /// 거래소에 제출됨 (대기 중)
    Open,
    /// 부분 체결됨
    PartiallyFilled,
    /// 전량 체결됨
    Filled,
    /// 사용자 또는 시스템에 의해 취소됨
    Cancelled,
    /// 거래소에서 거부됨
    Rejected,
    /// 유효 기간 만료
    Expired,
}

impl OrderStatusType {
    /// 주문이 최종 상태인지 확인합니다.
    pub fn is_final(&self) -> bool {
        matches!(
            self,
            OrderStatusType::Filled | OrderStatusType::Cancelled | OrderStatusType::Rejected | OrderStatusType::Expired
        )
    }

    /// 주문이 여전히 활성 상태인지 확인합니다.
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            OrderStatusType::Pending | OrderStatusType::Open | OrderStatusType::PartiallyFilled
        )
    }
}

/// 거래소에서 반환하는 주문 상태 응답.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderStatus {
    /// 거래소 주문 ID
    pub order_id: String,
    /// 클라이언트 주문 ID (있는 경우)
    pub client_order_id: Option<String>,
    /// 현재 상태
    pub status: OrderStatusType,
    /// 체결된 수량
    pub filled_quantity: Quantity,
    /// 평균 체결 가격 (체결이 있는 경우)
    pub average_price: Option<Price>,
    /// 마지막 업데이트 시각
    pub updated_at: DateTime<Utc>,
}

/// 주문 유효 기간.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum TimeInForce {
    /// 취소될 때까지 유효 (Good Till Cancelled)
    GTC,
    /// 즉시 체결 또는 취소 (Immediate Or Cancel)
    IOC,
    /// 전량 체결 또는 취소 (Fill Or Kill)
    FOK,
    /// 지정일까지 유효 (Good Till Date)
    GTD,
}

/// 새 주문 생성을 위한 주문 요청.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRequest {
    /// 거래 심볼
    pub symbol: Symbol,
    /// 주문 방향
    pub side: Side,
    /// 주문 유형
    pub order_type: OrderType,
    /// 거래 수량
    pub quantity: Quantity,
    /// 지정가 (지정가 주문에 필수)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<Price>,
    /// 스톱 가격 (스톱 주문용)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_price: Option<Price>,
    /// 주문 유효 기간
    pub time_in_force: TimeInForce,
    /// 클라이언트 주문 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
    /// 이 주문을 생성한 전략
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy_id: Option<String>,
}

impl OrderRequest {
    /// 시장가 매수 주문을 생성합니다.
    pub fn market_buy(symbol: Symbol, quantity: Quantity) -> Self {
        Self {
            symbol,
            side: Side::Buy,
            order_type: OrderType::Market,
            quantity,
            price: None,
            stop_price: None,
            time_in_force: TimeInForce::GTC,
            client_order_id: None,
            strategy_id: None,
        }
    }

    /// 시장가 매도 주문을 생성합니다.
    pub fn market_sell(symbol: Symbol, quantity: Quantity) -> Self {
        Self {
            symbol,
            side: Side::Sell,
            order_type: OrderType::Market,
            quantity,
            price: None,
            stop_price: None,
            time_in_force: TimeInForce::GTC,
            client_order_id: None,
            strategy_id: None,
        }
    }

    /// 지정가 매수 주문을 생성합니다.
    pub fn limit_buy(symbol: Symbol, quantity: Quantity, price: Price) -> Self {
        Self {
            symbol,
            side: Side::Buy,
            order_type: OrderType::Limit,
            quantity,
            price: Some(price),
            stop_price: None,
            time_in_force: TimeInForce::GTC,
            client_order_id: None,
            strategy_id: None,
        }
    }

    /// 지정가 매도 주문을 생성합니다.
    pub fn limit_sell(symbol: Symbol, quantity: Quantity, price: Price) -> Self {
        Self {
            symbol,
            side: Side::Sell,
            order_type: OrderType::Limit,
            quantity,
            price: Some(price),
            stop_price: None,
            time_in_force: TimeInForce::GTC,
            client_order_id: None,
            strategy_id: None,
        }
    }

    /// 전략 ID를 설정합니다.
    pub fn with_strategy(mut self, strategy_id: impl Into<String>) -> Self {
        self.strategy_id = Some(strategy_id.into());
        self
    }

    /// 클라이언트 주문 ID를 설정합니다.
    pub fn with_client_id(mut self, client_id: impl Into<String>) -> Self {
        self.client_order_id = Some(client_id.into());
        self
    }
}

/// 제출된 주문을 나타내는 주문 엔티티.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    /// 내부 주문 ID
    pub id: Uuid,
    /// 거래소 이름
    pub exchange: String,
    /// 거래소 주문 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exchange_order_id: Option<String>,
    /// 거래 심볼
    pub symbol: Symbol,
    /// 주문 방향
    pub side: Side,
    /// 주문 유형
    pub order_type: OrderType,
    /// 원래 수량
    pub quantity: Quantity,
    /// 지정가
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<Price>,
    /// 스톱 가격
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_price: Option<Price>,
    /// 현재 상태
    pub status: OrderStatusType,
    /// 체결된 수량
    pub filled_quantity: Quantity,
    /// 평균 체결 가격
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average_fill_price: Option<Price>,
    /// 주문 유효 기간
    pub time_in_force: TimeInForce,
    /// 이 주문을 생성한 전략
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy_id: Option<String>,
    /// 클라이언트 주문 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
    /// 생성 타임스탬프
    pub created_at: DateTime<Utc>,
    /// 마지막 업데이트 타임스탬프
    pub updated_at: DateTime<Utc>,
    /// 추가 메타데이터
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl Order {
    /// 요청으로부터 새 주문을 생성합니다.
    pub fn from_request(request: OrderRequest, exchange: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            exchange: exchange.into(),
            exchange_order_id: None,
            symbol: request.symbol,
            side: request.side,
            order_type: request.order_type,
            quantity: request.quantity,
            price: request.price,
            stop_price: request.stop_price,
            status: OrderStatusType::Pending,
            filled_quantity: Decimal::ZERO,
            average_fill_price: None,
            time_in_force: request.time_in_force,
            strategy_id: request.strategy_id,
            client_order_id: request.client_order_id,
            created_at: now,
            updated_at: now,
            metadata: serde_json::Value::Null,
        }
    }

    /// 남은 체결 수량을 반환합니다.
    pub fn remaining_quantity(&self) -> Quantity {
        self.quantity - self.filled_quantity
    }

    /// 주문이 전량 체결되었는지 확인합니다.
    pub fn is_filled(&self) -> bool {
        self.status == OrderStatusType::Filled
    }

    /// 주문이 활성 상태인지 확인합니다.
    pub fn is_active(&self) -> bool {
        self.status.is_active()
    }

    /// 주문의 명목 가치를 계산합니다.
    pub fn notional_value(&self) -> Option<Decimal> {
        self.price.map(|p| p * self.quantity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MarketType;
    use rust_decimal_macros::dec;

    #[test]
    fn test_order_request() {
        let symbol = Symbol::crypto("BTC", "USDT");
        let order = OrderRequest::limit_buy(symbol.clone(), dec!(0.1), dec!(50000))
            .with_strategy("grid_trading");

        assert_eq!(order.side, Side::Buy);
        assert_eq!(order.order_type, OrderType::Limit);
        assert_eq!(order.quantity, dec!(0.1));
        assert_eq!(order.price, Some(dec!(50000)));
        assert_eq!(order.strategy_id, Some("grid_trading".to_string()));
    }

    #[test]
    fn test_order_from_request() {
        let symbol = Symbol::crypto("ETH", "USDT");
        let request = OrderRequest::market_sell(symbol, dec!(1.0));
        let order = Order::from_request(request, "binance");

        assert_eq!(order.exchange, "binance");
        assert_eq!(order.status, OrderStatusType::Pending);
        assert_eq!(order.filled_quantity, Decimal::ZERO);
    }

    #[test]
    fn test_side_opposite() {
        assert_eq!(Side::Buy.opposite(), Side::Sell);
        assert_eq!(Side::Sell.opposite(), Side::Buy);
    }
}
