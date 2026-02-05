//! 거래소 trait 정의.

use async_trait::async_trait;
use trader_core::{
    Kline, OrderBook, OrderRequest, OrderStatus, Position, Ticker, Timeframe, TradeTick,
};

use crate::ExchangeError;

/// 거래소 작업을 위한 Result 타입.
pub type ExchangeResult<T> = Result<T, ExchangeError>;

/// 자산의 잔고 정보.
#[derive(Debug, Clone)]
pub struct Balance {
    /// 자산 이름 (예: "BTC", "USDT")
    pub asset: String,
    /// 사용 가능한 잔고
    pub free: rust_decimal::Decimal,
    /// 주문에 묶인 잔고
    pub locked: rust_decimal::Decimal,
}

impl Balance {
    /// 총 잔고 반환 (사용 가능 + 묶인 잔고).
    pub fn total(&self) -> rust_decimal::Decimal {
        self.free + self.locked
    }
}

/// 거래소의 계좌 정보.
#[derive(Debug, Clone)]
pub struct AccountInfo {
    /// 계좌 잔고
    pub balances: Vec<Balance>,
    /// 거래 가능 여부
    pub can_trade: bool,
    /// 출금 가능 여부
    pub can_withdraw: bool,
    /// 입금 가능 여부
    pub can_deposit: bool,
}

/// 통합 거래소 인터페이스를 위한 Exchange trait.
#[async_trait]
pub trait Exchange: Send + Sync {
    /// 거래소 이름 반환.
    fn name(&self) -> &str;

    /// 거래소 연결 여부 확인.
    async fn is_connected(&self) -> bool;

    /// 거래소에 연결.
    async fn connect(&mut self) -> ExchangeResult<()>;

    /// 거래소 연결 해제.
    async fn disconnect(&mut self) -> ExchangeResult<()>;

    // === 계좌 작업 ===

    /// 계좌 정보 조회.
    async fn get_account(&self) -> ExchangeResult<AccountInfo>;

    /// 특정 자산의 잔고 조회.
    async fn get_balance(&self, asset: &str) -> ExchangeResult<Balance>;

    // === 시장 데이터 ===

    /// 심볼의 현재 시세 조회.
    async fn get_ticker(&self, symbol: &str) -> ExchangeResult<Ticker>;

    /// 심볼의 호가창 조회.
    async fn get_order_book(&self, symbol: &str, limit: Option<u32>) -> ExchangeResult<OrderBook>;

    /// 심볼의 최근 체결 조회.
    async fn get_recent_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> ExchangeResult<Vec<TradeTick>>;

    /// 과거 캔들스틱 조회.
    async fn get_klines(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        limit: Option<u32>,
    ) -> ExchangeResult<Vec<Kline>>;

    // === 주문 작업 ===

    /// 새 주문 제출.
    async fn place_order(&self, request: &OrderRequest) -> ExchangeResult<String>;

    /// 주문 취소.
    async fn cancel_order(&self, symbol: &str, order_id: &str) -> ExchangeResult<()>;

    /// 주문 상태 조회.
    async fn get_order(&self, symbol: &str, order_id: &str) -> ExchangeResult<OrderStatus>;

    /// 심볼의 미체결 주문 조회.
    async fn get_open_orders(&self, symbol: Option<&str>) -> ExchangeResult<Vec<OrderStatus>>;

    // === 포지션 작업 (선물) ===

    /// 현재 포지션 조회.
    async fn get_positions(&self) -> ExchangeResult<Vec<Position>> {
        Ok(vec![]) // 현물 거래소를 위한 기본 구현
    }
}

/// 시장 데이터 스트림 이벤트.
#[derive(Debug, Clone)]
pub enum MarketEvent {
    /// 시세 업데이트
    Ticker(Ticker),
    /// 캔들스틱 업데이트
    Kline(Kline),
    /// 호가창 업데이트
    OrderBook(OrderBook),
    /// 체결 틱
    Trade(TradeTick),
    /// 연결 상태 변경
    Connected,
    /// 연결 해제
    Disconnected,
    /// 에러 발생
    Error(String),
}

/// 사용자 데이터 스트림 이벤트.
#[derive(Debug, Clone)]
pub enum UserEvent {
    /// 주문 업데이트
    OrderUpdate(OrderStatus),
    /// 잔고 업데이트
    BalanceUpdate(Balance),
    /// 포지션 업데이트 (선물)
    PositionUpdate(Position),
}

/// WebSocket 스트림 구독.
#[async_trait]
pub trait MarketStream: Send + Sync {
    /// 시세 업데이트 구독.
    async fn subscribe_ticker(&mut self, symbol: &str) -> ExchangeResult<()>;

    /// 캔들스틱 업데이트 구독.
    async fn subscribe_kline(&mut self, symbol: &str, timeframe: Timeframe) -> ExchangeResult<()>;

    /// 호가창 업데이트 구독.
    async fn subscribe_order_book(&mut self, symbol: &str) -> ExchangeResult<()>;

    /// 체결 업데이트 구독.
    async fn subscribe_trades(&mut self, symbol: &str) -> ExchangeResult<()>;

    /// 심볼 구독 해제.
    async fn unsubscribe(&mut self, symbol: &str) -> ExchangeResult<()>;

    /// 다음 시장 이벤트 반환.
    async fn next_event(&mut self) -> Option<MarketEvent>;
}

/// 사용자 데이터 스트림 구독.
#[async_trait]
pub trait UserStream: Send + Sync {
    /// 사용자 데이터 스트림 시작.
    async fn start(&mut self) -> ExchangeResult<()>;

    /// 사용자 데이터 스트림 중지.
    async fn stop(&mut self) -> ExchangeResult<()>;

    /// 다음 사용자 이벤트 반환.
    async fn next_event(&mut self) -> Option<UserEvent>;
}
