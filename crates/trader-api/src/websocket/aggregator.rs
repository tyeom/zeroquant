//! 거래소 중립적 실시간 데이터 어그리게이터.
//!
//! 다양한 거래소의 MarketStream에서 데이터를 수신하여
//! WebSocket 클라이언트에게 통합된 형식으로 브로드캐스트합니다.
//!
//! # 사용 예제
//!
//! ```rust,ignore
//! use trader_api::websocket::aggregator::MarketDataAggregator;
//! use trader_exchange::stream::UnifiedMarketStream;
//!
//! let aggregator = MarketDataAggregator::new(subscriptions);
//! aggregator.run(market_stream).await;
//! ```

use tracing::{debug, error, info, warn};

use trader_core::{OrderBook, Ticker};
use trader_exchange::traits::{MarketEvent, MarketStream};

use super::messages::{ServerMessage, TickerData, TradeData, OrderBookData, OrderBookLevel};
use super::subscriptions::SharedSubscriptionManager;

/// 거래소 데이터를 WebSocket 클라이언트에게 전달하는 어그리게이터.
///
/// MarketStream trait을 구현한 모든 거래소 스트림에서 데이터를 수신하여
/// SubscriptionManager를 통해 브로드캐스트합니다.
pub struct MarketDataAggregator {
    subscriptions: SharedSubscriptionManager,
}

impl MarketDataAggregator {
    /// 새로운 어그리게이터 생성.
    pub fn new(subscriptions: SharedSubscriptionManager) -> Self {
        Self { subscriptions }
    }

    /// 어그리게이터 실행.
    ///
    /// MarketStream에서 이벤트를 수신하여 WebSocket 클라이언트에게 브로드캐스트합니다.
    /// 이 메서드는 스트림이 종료될 때까지 블로킹됩니다.
    ///
    /// # Arguments
    ///
    /// * `stream` - 거래소 데이터 스트림 (MarketStream trait 구현)
    pub async fn run<S: MarketStream>(self, mut stream: S) {
        info!("MarketDataAggregator 시작");

        while let Some(event) = stream.next_event().await {
            match event {
                MarketEvent::Ticker(ticker) => {
                    self.handle_ticker(ticker);
                }
                MarketEvent::OrderBook(orderbook) => {
                    self.handle_orderbook(orderbook);
                }
                MarketEvent::Trade(trade) => {
                    self.handle_trade(trade);
                }
                MarketEvent::Kline(_kline) => {
                    // 캔들스틱은 현재 브로드캐스트하지 않음
                    debug!("Kline event received (not broadcasted)");
                }
                MarketEvent::Connected => {
                    info!("거래소 연결됨");
                    // 연결 상태 메시지 브로드캐스트 가능
                }
                MarketEvent::Disconnected => {
                    warn!("거래소 연결 끊김");
                    // 재연결 로직은 MarketStream 내부에서 처리
                }
                MarketEvent::Error(msg) => {
                    error!("거래소 에러: {}", msg);
                }
            }
        }

        warn!("MarketDataAggregator 종료 - 스트림 완료");
    }

    /// Ticker 이벤트 처리.
    fn handle_ticker(&self, ticker: Ticker) {
        let symbol = format!("{}", ticker.symbol);
        let timestamp = ticker.timestamp.timestamp_millis();

        // 24시간 변화율 계산 (이미 계산된 값 사용)
        let change_24h = ticker.change_24h_percent;

        let ticker_data = TickerData {
            symbol: symbol.clone(),
            price: ticker.last,
            change_24h,
            volume_24h: ticker.volume_24h,
            high_24h: ticker.high_24h,
            low_24h: ticker.low_24h,
            timestamp,
        };

        let message = ServerMessage::Ticker(ticker_data);

        debug!(
            symbol = %symbol,
            price = %ticker.last,
            "Ticker broadcast"
        );

        if let Err(e) = self.subscriptions.broadcast(message) {
            // 구독자가 없으면 에러 발생 가능 - 무시
            debug!("Broadcast error (likely no subscribers): {}", e);
        }
    }

    /// OrderBook 이벤트 처리.
    fn handle_orderbook(&self, orderbook: OrderBook) {
        let symbol = format!("{}", orderbook.symbol);
        let timestamp = orderbook.timestamp.timestamp_millis();

        let bids: Vec<OrderBookLevel> = orderbook
            .bids
            .iter()
            .map(|level| OrderBookLevel {
                price: level.price,
                quantity: level.quantity,
            })
            .collect();

        let asks: Vec<OrderBookLevel> = orderbook
            .asks
            .iter()
            .map(|level| OrderBookLevel {
                price: level.price,
                quantity: level.quantity,
            })
            .collect();

        let orderbook_data = OrderBookData {
            symbol: symbol.clone(),
            bids,
            asks,
            timestamp,
        };

        let message = ServerMessage::OrderBook(orderbook_data);

        debug!(
            symbol = %symbol,
            bid_levels = orderbook.bids.len(),
            ask_levels = orderbook.asks.len(),
            "OrderBook broadcast"
        );

        if let Err(e) = self.subscriptions.broadcast(message) {
            debug!("Broadcast error: {}", e);
        }
    }

    /// Trade 이벤트 처리.
    fn handle_trade(&self, trade: trader_core::TradeTick) {
        let symbol = format!("{}", trade.symbol);
        let timestamp = trade.timestamp.timestamp_millis();

        let trade_data = TradeData {
            symbol: symbol.clone(),
            trade_id: trade.id.clone(),
            price: trade.price,
            quantity: trade.quantity,
            side: format!("{:?}", trade.side).to_lowercase(),
            timestamp,
        };

        let message = ServerMessage::Trade(trade_data);

        debug!(
            symbol = %symbol,
            price = %trade.price,
            quantity = %trade.quantity,
            "Trade broadcast"
        );

        if let Err(e) = self.subscriptions.broadcast(message) {
            debug!("Broadcast error: {}", e);
        }
    }
}

/// 백그라운드에서 어그리게이터 실행.
///
/// # Arguments
///
/// * `subscriptions` - WebSocket 구독 관리자
/// * `stream` - 거래소 데이터 스트림
pub fn start_aggregator<S: MarketStream + Send + 'static>(
    subscriptions: SharedSubscriptionManager,
    stream: S,
) {
    let aggregator = MarketDataAggregator::new(subscriptions);

    tokio::spawn(async move {
        aggregator.run(stream).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::websocket::subscriptions::create_subscription_manager;

    #[test]
    fn test_aggregator_creation() {
        let subscriptions = create_subscription_manager(100);
        let _aggregator = MarketDataAggregator::new(subscriptions);
    }
}
