//! 거래소 중립적 시장 데이터 스트림.
//!
//! 다양한 거래소의 WebSocket 연결을 `MarketStream` trait으로 래핑하여
//! 통합된 인터페이스를 제공합니다.
//!
//! # 주의사항
//!
//! 현재 KIS WebSocket은 연결 후 동적 구독을 지원하지 않습니다.
//! `subscribe_*` 메서드는 `start()` 호출 전에 실행되어야 합니다.
//!
//! # 사용 예제
//!
//! ```rust,ignore
//! use trader_exchange::stream::KisKrMarketStream;
//! use trader_exchange::connector::kis::KisOAuth;
//!
//! let mut stream = KisKrMarketStream::new(oauth);
//!
//! // 연결 전에 구독 설정
//! stream.subscribe_ticker(&symbol).await?;
//!
//! // 연결 시작
//! stream.start().await?;
//!
//! // 이벤트 수신
//! while let Some(event) = stream.next_event().await {
//!     match event {
//!         MarketEvent::Ticker(ticker) => println!("Ticker: {:?}", ticker),
//!         MarketEvent::OrderBook(book) => println!("Orderbook: {:?}", book),
//!         _ => {}
//!     }
//! }
//! ```

use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use trader_core::{OrderBook, OrderBookLevel, Side, Symbol, Ticker, Timeframe, TradeTick};

use crate::connector::kis::{
    KisKrWebSocket, KisOAuth, KisUsClient, KisUsWebSocket, KrRealtimeMessage, KrRealtimeOrderbook,
    KrRealtimeTrade, UsRealtimeMessage, UsRealtimeOrderbook, UsRealtimeTrade,
};
use crate::traits::{ExchangeResult, MarketEvent, MarketStream};
use crate::ExchangeError;

// ============================================================================
// KIS 국내 MarketStream
// ============================================================================

/// KIS 국내 주식용 MarketStream 구현.
///
/// `KisKrWebSocket`을 래핑하여 `MarketStream` trait을 구현합니다.
///
/// # 주의
///
/// 구독 설정은 `start()` 호출 전에 완료해야 합니다.
/// 연결 후 동적 구독 변경은 현재 지원되지 않습니다.
pub struct KisKrMarketStream {
    ws: Arc<RwLock<KisKrWebSocket>>,
    rx: Option<mpsc::Receiver<KrRealtimeMessage>>,
    subscribed_symbols: HashMap<String, SubscriptionType>,
    started: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum SubscriptionType {
    Trade,
    Orderbook,
    Both,
}

impl KisKrMarketStream {
    /// 새로운 KIS 국내 MarketStream 생성.
    pub fn new(oauth: KisOAuth) -> Self {
        let mut ws = KisKrWebSocket::new(oauth);
        let rx = ws.take_receiver();

        Self {
            ws: Arc::new(RwLock::new(ws)),
            rx,
            subscribed_symbols: HashMap::new(),
            started: false,
        }
    }

    /// WebSocket 연결 시작 (별도 태스크에서 실행).
    ///
    /// 이 메서드 호출 전에 구독을 설정해야 합니다.
    pub async fn start(&mut self) -> ExchangeResult<()> {
        if self.started {
            return Ok(());
        }

        let ws = self.ws.clone();
        self.started = true;

        tokio::spawn(async move {
            let mut ws_guard = ws.write().await;
            if let Err(e) = ws_guard.connect().await {
                error!("KIS KR WebSocket 연결 실패: {}", e);
            }
        });

        info!("KIS KR MarketStream 시작됨");
        Ok(())
    }

    /// 연결 시작 여부 확인.
    pub fn is_started(&self) -> bool {
        self.started
    }

    /// 종목코드에서 Symbol 생성 (국내).
    fn code_to_symbol(code: &str) -> Symbol {
        Symbol::stock(code, "KRW")
    }

    /// KrRealtimeTrade를 Ticker로 변환.
    fn trade_to_ticker(trade: &KrRealtimeTrade) -> Ticker {
        let symbol = Self::code_to_symbol(&trade.symbol);
        let change_percent = if trade.change_rate != Decimal::ZERO {
            trade.change_rate
        } else {
            dec!(0)
        };

        Ticker {
            symbol,
            bid: trade.price - dec!(10), // 근사값 (실제로는 호가 데이터 필요)
            ask: trade.price + dec!(10), // 근사값
            last: trade.price,
            volume_24h: Decimal::from(trade.acc_volume),
            high_24h: trade.price, // KIS 실시간에서 미제공 - 현재가로 대체
            low_24h: trade.price,  // KIS 실시간에서 미제공 - 현재가로 대체
            change_24h: trade.change,
            change_24h_percent: change_percent,
            timestamp: Utc::now(),
        }
    }

    /// KrRealtimeTrade를 TradeTick으로 변환.
    #[allow(dead_code)]
    fn trade_to_tick(trade: &KrRealtimeTrade) -> TradeTick {
        // KIS에서는 체결 방향을 직접 제공하지 않음 - sign 필드로 추정
        let side = match trade.sign.as_str() {
            "1" | "2" => Side::Buy,  // 상한, 상승
            "4" | "5" => Side::Sell, // 하한, 하락
            _ => Side::Buy,          // 보합 등 기타 - 기본값
        };

        TradeTick {
            symbol: Self::code_to_symbol(&trade.symbol),
            id: trade.trade_time.clone(), // 체결시간을 ID로 사용
            price: trade.price,
            quantity: Decimal::from(trade.volume),
            side,
            timestamp: Utc::now(),
        }
    }

    /// KrRealtimeOrderbook을 OrderBook으로 변환.
    fn orderbook_to_book(ob: &KrRealtimeOrderbook) -> OrderBook {
        let bids: Vec<OrderBookLevel> = ob
            .bid_prices
            .iter()
            .zip(ob.bid_volumes.iter())
            .map(|(price, volume)| OrderBookLevel {
                price: *price,
                quantity: Decimal::from(*volume),
            })
            .collect();

        let asks: Vec<OrderBookLevel> = ob
            .ask_prices
            .iter()
            .zip(ob.ask_volumes.iter())
            .map(|(price, volume)| OrderBookLevel {
                price: *price,
                quantity: Decimal::from(*volume),
            })
            .collect();

        OrderBook {
            symbol: Self::code_to_symbol(&ob.symbol),
            bids,
            asks,
            timestamp: Utc::now(),
        }
    }
}

#[async_trait]
impl MarketStream for KisKrMarketStream {
    async fn subscribe_ticker(&mut self, symbol: &Symbol) -> ExchangeResult<()> {
        if self.started {
            warn!("연결 후 구독 변경은 지원되지 않습니다. start() 전에 호출하세요.");
            return Err(ExchangeError::NotSupported(
                "Dynamic subscription not supported after connection".to_string(),
            ));
        }

        let code = symbol.base.clone();
        let mut ws = self.ws.write().await;

        // 체결가 구독으로 Ticker 정보 수신
        ws.add_trade_subscription(&code);

        self.subscribed_symbols
            .entry(code.clone())
            .and_modify(|t| {
                if *t == SubscriptionType::Orderbook {
                    *t = SubscriptionType::Both;
                }
            })
            .or_insert(SubscriptionType::Trade);

        info!("KR 티커 구독 설정: {}", code);
        Ok(())
    }

    async fn subscribe_kline(
        &mut self,
        _symbol: &Symbol,
        _timeframe: Timeframe,
    ) -> ExchangeResult<()> {
        // KIS WebSocket은 실시간 캔들스틱을 지원하지 않음
        // REST API 폴링으로 대체해야 함
        warn!("KIS는 실시간 캔들스틱을 지원하지 않습니다");
        Err(ExchangeError::NotSupported(
            "KIS does not support real-time kline streaming".to_string(),
        ))
    }

    async fn subscribe_order_book(&mut self, symbol: &Symbol) -> ExchangeResult<()> {
        if self.started {
            warn!("연결 후 구독 변경은 지원되지 않습니다. start() 전에 호출하세요.");
            return Err(ExchangeError::NotSupported(
                "Dynamic subscription not supported after connection".to_string(),
            ));
        }

        let code = symbol.base.clone();
        let mut ws = self.ws.write().await;

        ws.add_orderbook_subscription(&code);

        self.subscribed_symbols
            .entry(code.clone())
            .and_modify(|t| {
                if *t == SubscriptionType::Trade {
                    *t = SubscriptionType::Both;
                }
            })
            .or_insert(SubscriptionType::Orderbook);

        info!("KR 호가 구독 설정: {}", code);
        Ok(())
    }

    async fn subscribe_trades(&mut self, symbol: &Symbol) -> ExchangeResult<()> {
        // 체결 구독 = Ticker 구독과 동일
        self.subscribe_ticker(symbol).await
    }

    async fn unsubscribe(&mut self, symbol: &Symbol) -> ExchangeResult<()> {
        if self.started {
            warn!("연결 후 구독 해제는 지원되지 않습니다");
            return Err(ExchangeError::NotSupported(
                "Dynamic unsubscription not supported after connection".to_string(),
            ));
        }

        let code = symbol.base.clone();
        let mut ws = self.ws.write().await;

        if let Some(sub_type) = self.subscribed_symbols.remove(&code) {
            match sub_type {
                SubscriptionType::Trade | SubscriptionType::Both => {
                    ws.remove_trade_subscription(&code);
                }
                SubscriptionType::Orderbook => {
                    ws.remove_orderbook_subscription(&code);
                }
            }
            if sub_type == SubscriptionType::Both {
                ws.remove_orderbook_subscription(&code);
            }
        }

        Ok(())
    }

    async fn next_event(&mut self) -> Option<MarketEvent> {
        let rx = self.rx.as_mut()?;

        match rx.recv().await {
            Some(KrRealtimeMessage::Trade(trade)) => {
                debug!("KR Trade: {} @ {}", trade.symbol, trade.price);
                Some(MarketEvent::Ticker(Self::trade_to_ticker(&trade)))
            }
            Some(KrRealtimeMessage::Orderbook(ob)) => {
                debug!("KR Orderbook: {}", ob.symbol);
                Some(MarketEvent::OrderBook(Self::orderbook_to_book(&ob)))
            }
            Some(KrRealtimeMessage::ConnectionStatus(connected)) => {
                if connected {
                    info!("KIS KR WebSocket 연결됨");
                    Some(MarketEvent::Connected)
                } else {
                    warn!("KIS KR WebSocket 연결 끊김");
                    Some(MarketEvent::Disconnected)
                }
            }
            Some(KrRealtimeMessage::Error(msg)) => {
                error!("KIS KR WebSocket 에러: {}", msg);
                Some(MarketEvent::Error(msg))
            }
            None => None,
        }
    }
}

// ============================================================================
// KIS 해외 MarketStream
// ============================================================================

/// US 구독 정보 (거래소 코드 포함).
#[derive(Clone)]
struct UsSubscriptionInfo {
    sub_type: SubscriptionType,
    exchange_code: String,
}

/// KIS 해외 주식용 MarketStream 구현.
pub struct KisUsMarketStream {
    ws: Arc<RwLock<KisUsWebSocket>>,
    rx: Option<mpsc::Receiver<UsRealtimeMessage>>,
    subscribed_symbols: HashMap<String, UsSubscriptionInfo>,
    started: bool,
}

impl KisUsMarketStream {
    /// 새로운 KIS 해외 MarketStream 생성.
    pub fn new(oauth: KisOAuth) -> Self {
        let mut ws = KisUsWebSocket::new(oauth);
        let rx = ws.take_receiver();

        Self {
            ws: Arc::new(RwLock::new(ws)),
            rx,
            subscribed_symbols: HashMap::new(),
            started: false,
        }
    }

    /// WebSocket 연결 시작.
    pub async fn start(&mut self) -> ExchangeResult<()> {
        if self.started {
            return Ok(());
        }

        let ws = self.ws.clone();
        self.started = true;

        tokio::spawn(async move {
            let mut ws_guard = ws.write().await;
            if let Err(e) = ws_guard.connect().await {
                error!("KIS US WebSocket 연결 실패: {}", e);
            }
        });

        info!("KIS US MarketStream 시작됨");
        Ok(())
    }

    /// 티커에서 Symbol 생성 (해외).
    fn ticker_to_symbol(ticker: &str) -> Symbol {
        Symbol::stock(ticker, "USD")
    }

    /// UsRealtimeTrade를 Ticker로 변환.
    fn trade_to_ticker(trade: &UsRealtimeTrade) -> Ticker {
        let symbol = Self::ticker_to_symbol(&trade.symbol);

        Ticker {
            symbol,
            bid: trade.price - dec!(0.01),
            ask: trade.price + dec!(0.01),
            last: trade.price,
            volume_24h: Decimal::from(trade.volume), // 체결량 (누적거래량 미제공)
            high_24h: trade.price,                   // KIS 실시간에서 미제공 - 현재가로 대체
            low_24h: trade.price,                    // KIS 실시간에서 미제공 - 현재가로 대체
            change_24h: trade.change,
            change_24h_percent: trade.change_rate,
            timestamp: Utc::now(),
        }
    }

    /// UsRealtimeOrderbook을 OrderBook으로 변환.
    fn orderbook_to_book(ob: &UsRealtimeOrderbook) -> OrderBook {
        // US는 단일 호가만 제공
        let bids = vec![OrderBookLevel {
            price: ob.bid_price,
            quantity: Decimal::from(ob.bid_volume),
        }];

        let asks = vec![OrderBookLevel {
            price: ob.ask_price,
            quantity: Decimal::from(ob.ask_volume),
        }];

        OrderBook {
            symbol: Self::ticker_to_symbol(&ob.symbol),
            bids,
            asks,
            timestamp: Utc::now(),
        }
    }
}

#[async_trait]
impl MarketStream for KisUsMarketStream {
    async fn subscribe_ticker(&mut self, symbol: &Symbol) -> ExchangeResult<()> {
        if self.started {
            return Err(ExchangeError::NotSupported(
                "Dynamic subscription not supported after connection".to_string(),
            ));
        }

        let ticker = symbol.base.clone();
        let exchange_code = KisUsClient::get_exchange_code(&ticker).to_string();
        let mut ws = self.ws.write().await;

        ws.add_trade_subscription(&ticker, &exchange_code);

        self.subscribed_symbols
            .entry(ticker.clone())
            .and_modify(|info| {
                if info.sub_type == SubscriptionType::Orderbook {
                    info.sub_type = SubscriptionType::Both;
                }
            })
            .or_insert(UsSubscriptionInfo {
                sub_type: SubscriptionType::Trade,
                exchange_code: exchange_code.clone(),
            });

        info!("US 티커 구독 설정: {} ({})", ticker, exchange_code);
        Ok(())
    }

    async fn subscribe_kline(
        &mut self,
        _symbol: &Symbol,
        _timeframe: Timeframe,
    ) -> ExchangeResult<()> {
        warn!("KIS는 실시간 캔들스틱을 지원하지 않습니다");
        Err(ExchangeError::NotSupported(
            "KIS does not support real-time kline streaming".to_string(),
        ))
    }

    async fn subscribe_order_book(&mut self, symbol: &Symbol) -> ExchangeResult<()> {
        if self.started {
            return Err(ExchangeError::NotSupported(
                "Dynamic subscription not supported after connection".to_string(),
            ));
        }

        let ticker = symbol.base.clone();
        let exchange_code = KisUsClient::get_exchange_code(&ticker).to_string();
        let mut ws = self.ws.write().await;

        ws.add_orderbook_subscription(&ticker, &exchange_code);

        self.subscribed_symbols
            .entry(ticker.clone())
            .and_modify(|info| {
                if info.sub_type == SubscriptionType::Trade {
                    info.sub_type = SubscriptionType::Both;
                }
            })
            .or_insert(UsSubscriptionInfo {
                sub_type: SubscriptionType::Orderbook,
                exchange_code: exchange_code.clone(),
            });

        info!("US 호가 구독 설정: {} ({})", ticker, exchange_code);
        Ok(())
    }

    async fn subscribe_trades(&mut self, symbol: &Symbol) -> ExchangeResult<()> {
        self.subscribe_ticker(symbol).await
    }

    async fn unsubscribe(&mut self, symbol: &Symbol) -> ExchangeResult<()> {
        if self.started {
            return Err(ExchangeError::NotSupported(
                "Dynamic unsubscription not supported after connection".to_string(),
            ));
        }

        let ticker = symbol.base.clone();
        let mut ws = self.ws.write().await;

        if let Some(info) = self.subscribed_symbols.remove(&ticker) {
            match info.sub_type {
                SubscriptionType::Trade | SubscriptionType::Both => {
                    ws.remove_trade_subscription(&ticker, &info.exchange_code);
                }
                SubscriptionType::Orderbook => {
                    ws.remove_orderbook_subscription(&ticker, &info.exchange_code);
                }
            }
            if info.sub_type == SubscriptionType::Both {
                ws.remove_orderbook_subscription(&ticker, &info.exchange_code);
            }
        }

        Ok(())
    }

    async fn next_event(&mut self) -> Option<MarketEvent> {
        let rx = self.rx.as_mut()?;

        match rx.recv().await {
            Some(UsRealtimeMessage::Trade(trade)) => {
                debug!("US Trade: {} @ {}", trade.symbol, trade.price);
                Some(MarketEvent::Ticker(Self::trade_to_ticker(&trade)))
            }
            Some(UsRealtimeMessage::Orderbook(ob)) => {
                debug!("US Orderbook: {}", ob.symbol);
                Some(MarketEvent::OrderBook(Self::orderbook_to_book(&ob)))
            }
            Some(UsRealtimeMessage::ConnectionStatus(connected)) => {
                if connected {
                    info!("KIS US WebSocket 연결됨");
                    Some(MarketEvent::Connected)
                } else {
                    warn!("KIS US WebSocket 연결 끊김");
                    Some(MarketEvent::Disconnected)
                }
            }
            Some(UsRealtimeMessage::Error(msg)) => {
                error!("KIS US WebSocket 에러: {}", msg);
                Some(MarketEvent::Error(msg))
            }
            None => None,
        }
    }
}

// ============================================================================
// 통합 MarketStream (여러 거래소 지원)
// ============================================================================

/// 여러 거래소를 통합하는 MarketStream.
///
/// 국내(KR)와 해외(US) 시장을 모두 지원하며,
/// 심볼에 따라 적절한 스트림으로 라우팅합니다.
pub struct UnifiedMarketStream {
    kr_stream: Option<KisKrMarketStream>,
    us_stream: Option<KisUsMarketStream>,
    started: bool,
}

impl UnifiedMarketStream {
    /// 새로운 통합 MarketStream 생성.
    pub fn new() -> Self {
        Self {
            kr_stream: None,
            us_stream: None,
            started: false,
        }
    }

    /// KIS 국내 스트림 추가.
    pub fn with_kr_stream(mut self, oauth: KisOAuth) -> Self {
        self.kr_stream = Some(KisKrMarketStream::new(oauth));
        self
    }

    /// KIS 해외 스트림 추가.
    pub fn with_us_stream(mut self, oauth: KisOAuth) -> Self {
        self.us_stream = Some(KisUsMarketStream::new(oauth));
        self
    }

    /// 모든 스트림 시작.
    pub async fn start_all(&mut self) -> ExchangeResult<()> {
        if self.started {
            return Ok(());
        }

        if let Some(ref mut kr) = self.kr_stream {
            kr.start().await?;
        }
        if let Some(ref mut us) = self.us_stream {
            us.start().await?;
        }

        self.started = true;
        info!("UnifiedMarketStream 시작됨");
        Ok(())
    }

    /// 심볼이 국내인지 해외인지 판단.
    fn is_korean_symbol(symbol: &Symbol) -> bool {
        // 6자리 숫자 = 국내 주식
        symbol.base.len() == 6 && symbol.base.chars().all(|c| c.is_ascii_digit())
    }
}

impl Default for UnifiedMarketStream {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MarketStream for UnifiedMarketStream {
    async fn subscribe_ticker(&mut self, symbol: &Symbol) -> ExchangeResult<()> {
        if Self::is_korean_symbol(symbol) {
            if let Some(ref mut kr) = self.kr_stream {
                return kr.subscribe_ticker(symbol).await;
            }
        } else if let Some(ref mut us) = self.us_stream {
            return us.subscribe_ticker(symbol).await;
        }
        Err(ExchangeError::NotSupported(format!(
            "No stream available for symbol: {}",
            symbol
        )))
    }

    async fn subscribe_kline(
        &mut self,
        symbol: &Symbol,
        timeframe: Timeframe,
    ) -> ExchangeResult<()> {
        if Self::is_korean_symbol(symbol) {
            if let Some(ref mut kr) = self.kr_stream {
                return kr.subscribe_kline(symbol, timeframe).await;
            }
        } else if let Some(ref mut us) = self.us_stream {
            return us.subscribe_kline(symbol, timeframe).await;
        }
        Err(ExchangeError::NotSupported(format!(
            "No stream available for symbol: {}",
            symbol
        )))
    }

    async fn subscribe_order_book(&mut self, symbol: &Symbol) -> ExchangeResult<()> {
        if Self::is_korean_symbol(symbol) {
            if let Some(ref mut kr) = self.kr_stream {
                return kr.subscribe_order_book(symbol).await;
            }
        } else if let Some(ref mut us) = self.us_stream {
            return us.subscribe_order_book(symbol).await;
        }
        Err(ExchangeError::NotSupported(format!(
            "No stream available for symbol: {}",
            symbol
        )))
    }

    async fn subscribe_trades(&mut self, symbol: &Symbol) -> ExchangeResult<()> {
        if Self::is_korean_symbol(symbol) {
            if let Some(ref mut kr) = self.kr_stream {
                return kr.subscribe_trades(symbol).await;
            }
        } else if let Some(ref mut us) = self.us_stream {
            return us.subscribe_trades(symbol).await;
        }
        Err(ExchangeError::NotSupported(format!(
            "No stream available for symbol: {}",
            symbol
        )))
    }

    async fn unsubscribe(&mut self, symbol: &Symbol) -> ExchangeResult<()> {
        if Self::is_korean_symbol(symbol) {
            if let Some(ref mut kr) = self.kr_stream {
                return kr.unsubscribe(symbol).await;
            }
        } else if let Some(ref mut us) = self.us_stream {
            return us.unsubscribe(symbol).await;
        }
        Ok(())
    }

    async fn next_event(&mut self) -> Option<MarketEvent> {
        // 간단한 라운드로빈: KR 먼저, 그 다음 US
        // TODO: tokio::select!로 동시 수신 구현
        if let Some(ref mut kr) = self.kr_stream {
            if let Some(event) = kr.next_event().await {
                return Some(event);
            }
        }
        if let Some(ref mut us) = self.us_stream {
            if let Some(event) = us.next_event().await {
                return Some(event);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_korean_symbol_detection() {
        let kr_symbol = Symbol::stock("005930", "KRW");
        let us_symbol = Symbol::stock("AAPL", "USD");

        assert!(UnifiedMarketStream::is_korean_symbol(&kr_symbol));
        assert!(!UnifiedMarketStream::is_korean_symbol(&us_symbol));
    }
}
