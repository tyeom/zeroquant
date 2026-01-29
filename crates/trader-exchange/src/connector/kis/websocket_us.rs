//! KIS 해외 주식 실시간 시세 WebSocket 클라이언트.
//!
//! 한국투자증권 WebSocket API를 통해 미국 주식의 실시간 체결가를 수신합니다.
//!
//! # 지원 채널
//!
//! - `HDFSCNT0`: 해외 실시간 체결
//! - `HDFSASP0`: 해외 실시간 호가
//!
//! # 거래소 코드
//!
//! - `NAS`: NASDAQ
//! - `NYS`: NYSE
//! - `AMS`: AMEX
//!
//! # 사용 예제
//!
//! ```rust,ignore
//! use trader_exchange::connector::kis::{KisConfig, KisOAuth, KisUsWebSocket};
//!
//! let config = KisConfig::new("app_key", "app_secret", "12345678-01");
//! let oauth = KisOAuth::new(config);
//! let mut ws = KisUsWebSocket::new(oauth);
//!
//! // AAPL 실시간 체결가 구독
//! ws.add_trade_subscription("AAPL", "NAS");
//!
//! // 연결 및 메시지 수신
//! ws.connect().await?;
//! ```

use super::auth::KisOAuth;
use super::tr_id;
use crate::ExchangeError;
use futures::{SinkExt, StreamExt};
use rust_decimal::Decimal;
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

/// 재연결 최대 시도 횟수.
const MAX_RECONNECT_ATTEMPTS: u32 = 3;

/// 재연결 대기 시간 (초).
const RECONNECT_DELAY_SECS: u64 = 5;

/// Ping 간격 (초).
const PING_INTERVAL_SECS: u64 = 30;

/// 해외 주식 실시간 체결 데이터.
#[derive(Debug, Clone)]
pub struct UsRealtimeTrade {
    /// 종목코드 (예: AAPL)
    pub symbol: String,
    /// 거래소 코드 (NAS, NYS, AMS)
    pub exchange_code: String,
    /// 체결가
    pub price: Decimal,
    /// 체결량
    pub volume: i64,
    /// 체결시간 (현지 시간)
    pub trade_time: String,
    /// 전일종가
    pub prev_close: Decimal,
    /// 전일대비
    pub change: Decimal,
    /// 등락률
    pub change_rate: Decimal,
}

/// 해외 주식 실시간 호가 데이터.
#[derive(Debug, Clone)]
pub struct UsRealtimeOrderbook {
    /// 종목코드
    pub symbol: String,
    /// 거래소 코드
    pub exchange_code: String,
    /// 매도호가
    pub ask_price: Decimal,
    /// 매도호가 잔량
    pub ask_volume: i64,
    /// 매수호가
    pub bid_price: Decimal,
    /// 매수호가 잔량
    pub bid_volume: i64,
    /// 호가시간
    pub orderbook_time: String,
}

/// 해외 실시간 메시지 타입.
#[derive(Debug, Clone)]
pub enum UsRealtimeMessage {
    /// 체결가
    Trade(UsRealtimeTrade),
    /// 호가
    Orderbook(UsRealtimeOrderbook),
    /// 연결 상태 변경
    ConnectionStatus(bool),
    /// 에러
    Error(String),
}

/// 구독 종목 정보.
#[derive(Debug, Clone)]
struct SubscriptionInfo {
    symbol: String,
    exchange_code: String,
}

/// WebSocket 구독 요청 메시지.
#[derive(Debug, Serialize)]
struct WsSubscribeRequest {
    header: WsHeader,
    body: WsBody,
}

#[derive(Debug, Serialize)]
struct WsHeader {
    approval_key: String,
    custtype: String,
    tr_type: String, // "1": 구독 등록, "2": 구독 해제
    #[serde(rename = "content-type")]
    content_type: String,
}

#[derive(Debug, Serialize)]
struct WsBody {
    input: WsInput,
}

#[derive(Debug, Serialize)]
struct WsInput {
    tr_id: String,
    tr_key: String, // 거래소코드+종목코드 (예: DNASAAPL)
}

/// KIS 해외 주식 실시간 WebSocket 클라이언트.
pub struct KisUsWebSocket {
    oauth: KisOAuth,
    tx: Option<mpsc::Sender<UsRealtimeMessage>>,
    rx: Option<mpsc::Receiver<UsRealtimeMessage>>,
    subscribed_trades: Vec<SubscriptionInfo>,
    subscribed_orderbooks: Vec<SubscriptionInfo>,
    is_connected: Arc<tokio::sync::RwLock<bool>>,
}

impl KisUsWebSocket {
    /// 새로운 해외 WebSocket 클라이언트 생성.
    pub fn new(oauth: KisOAuth) -> Self {
        let (tx, rx) = mpsc::channel(1000);
        Self {
            oauth,
            tx: Some(tx),
            rx: Some(rx),
            subscribed_trades: Vec::new(),
            subscribed_orderbooks: Vec::new(),
            is_connected: Arc::new(tokio::sync::RwLock::new(false)),
        }
    }

    /// 메시지 수신 채널 가져오기.
    pub fn take_receiver(&mut self) -> Option<mpsc::Receiver<UsRealtimeMessage>> {
        self.rx.take()
    }

    /// 연결 상태 확인.
    pub async fn is_connected(&self) -> bool {
        *self.is_connected.read().await
    }

    /// tr_key 생성 (거래소코드 + 종목코드).
    ///
    /// 형식: D{EXCD}{SYMBOL} (예: DNASAAPL)
    fn make_tr_key(exchange_code: &str, symbol: &str) -> String {
        format!("D{}{}", exchange_code, symbol)
    }

    /// WebSocket 연결 및 메시지 수신 시작.
    pub async fn connect(&mut self) -> Result<(), ExchangeError> {
        let mut reconnect_attempts = 0;

        loop {
            match self.connect_internal().await {
                Ok(_) => {
                    info!("KIS US WebSocket 연결 종료");
                    break;
                }
                Err(e) => {
                    error!("KIS US WebSocket 에러: {}", e);
                    reconnect_attempts += 1;

                    if reconnect_attempts > MAX_RECONNECT_ATTEMPTS {
                        error!("최대 재연결 시도 횟수 초과 ({}회)", MAX_RECONNECT_ATTEMPTS);
                        if let Some(tx) = &self.tx {
                            let _ = tx
                                .send(UsRealtimeMessage::Error(format!(
                                    "최대 재연결 시도 횟수 초과: {}",
                                    e
                                )))
                                .await;
                        }
                        return Err(e);
                    }

                    warn!(
                        "{}초 후 재연결 시도 ({}/{})",
                        RECONNECT_DELAY_SECS, reconnect_attempts, MAX_RECONNECT_ATTEMPTS
                    );
                    tokio::time::sleep(Duration::from_secs(RECONNECT_DELAY_SECS)).await;

                    // WebSocket 키 초기화
                    self.oauth.clear_websocket_key().await;
                }
            }
        }

        Ok(())
    }

    /// 내부 연결 로직.
    async fn connect_internal(&mut self) -> Result<(), ExchangeError> {
        // WebSocket 접속키 발급
        let approval_key = self.oauth.get_websocket_key().await?;
        let ws_url = self.oauth.config().websocket_url();

        info!("KIS US WebSocket 연결 중: {}", ws_url);

        // WebSocket 연결
        let (ws_stream, _) = connect_async(ws_url)
            .await
            .map_err(|e| ExchangeError::NetworkError(format!("WebSocket 연결 실패: {}", e)))?;

        let (mut write, mut read) = ws_stream.split();

        // 연결 상태 업데이트
        {
            let mut connected = self.is_connected.write().await;
            *connected = true;
        }

        if let Some(tx) = &self.tx {
            let _ = tx.send(UsRealtimeMessage::ConnectionStatus(true)).await;
        }

        info!("KIS US WebSocket 연결 성공");

        // 기존 구독 복원
        let trades = self.subscribed_trades.clone();
        let orderbooks = self.subscribed_orderbooks.clone();

        for info in &trades {
            let tr_key = Self::make_tr_key(&info.exchange_code, &info.symbol);
            let msg = self.create_subscribe_message(&approval_key, tr_id::WS_US_TRADE, &tr_key, true);
            write
                .send(Message::Text(msg))
                .await
                .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;
            debug!("해외 체결가 구독 복원: {} ({})", info.symbol, info.exchange_code);
        }

        for info in &orderbooks {
            let tr_key = Self::make_tr_key(&info.exchange_code, &info.symbol);
            let msg =
                self.create_subscribe_message(&approval_key, tr_id::WS_US_ORDERBOOK, &tr_key, true);
            write
                .send(Message::Text(msg))
                .await
                .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;
            debug!("해외 호가 구독 복원: {} ({})", info.symbol, info.exchange_code);
        }

        // Ping 타이머
        let mut ping_interval = tokio::time::interval(Duration::from_secs(PING_INTERVAL_SECS));

        // 메시지 수신 루프
        loop {
            tokio::select! {
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            self.handle_message(&text).await;
                        }
                        Some(Ok(Message::Ping(data))) => {
                            debug!("Ping 수신, Pong 응답");
                            let _ = write.send(Message::Pong(data)).await;
                        }
                        Some(Ok(Message::Close(_))) => {
                            warn!("서버에서 연결 종료 요청");
                            break;
                        }
                        Some(Err(e)) => {
                            error!("WebSocket 수신 에러: {}", e);
                            break;
                        }
                        None => {
                            warn!("WebSocket 스트림 종료");
                            break;
                        }
                        _ => {}
                    }
                }
                _ = ping_interval.tick() => {
                    debug!("Ping 전송");
                    if let Err(e) = write.send(Message::Ping(vec![])).await {
                        error!("Ping 전송 실패: {}", e);
                        break;
                    }
                }
            }
        }

        // 연결 상태 업데이트
        {
            let mut connected = self.is_connected.write().await;
            *connected = false;
        }

        if let Some(tx) = &self.tx {
            let _ = tx.send(UsRealtimeMessage::ConnectionStatus(false)).await;
        }

        Err(ExchangeError::NetworkError("연결 끊김".to_string()))
    }

    /// 구독 메시지 생성.
    fn create_subscribe_message(
        &self,
        approval_key: &str,
        tr_id: &str,
        tr_key: &str,
        subscribe: bool,
    ) -> String {
        let request = WsSubscribeRequest {
            header: WsHeader {
                approval_key: approval_key.to_string(),
                custtype: "P".to_string(),
                tr_type: if subscribe { "1" } else { "2" }.to_string(),
                content_type: "utf-8".to_string(),
            },
            body: WsBody {
                input: WsInput {
                    tr_id: tr_id.to_string(),
                    tr_key: tr_key.to_string(),
                },
            },
        };

        serde_json::to_string(&request).unwrap_or_default()
    }

    /// 수신 메시지 처리.
    async fn handle_message(&self, text: &str) {
        // KIS WebSocket 메시지 형식: 0|HDFSCNT0|001|DNASAAPL^...
        let parts: Vec<&str> = text.split('|').collect();

        if parts.len() < 4 {
            debug!("JSON 응답: {}", text);
            return;
        }

        let tr_id = parts[1];
        let data = parts[3];

        match tr_id {
            "HDFSCNT0" => {
                if let Some(trade) = self.parse_trade_data(data) {
                    if let Some(tx) = &self.tx {
                        let _ = tx.send(UsRealtimeMessage::Trade(trade)).await;
                    }
                }
            }
            "HDFSASP0" => {
                if let Some(orderbook) = self.parse_orderbook_data(data) {
                    if let Some(tx) = &self.tx {
                        let _ = tx.send(UsRealtimeMessage::Orderbook(orderbook)).await;
                    }
                }
            }
            _ => {
                debug!("알 수 없는 tr_id: {}", tr_id);
            }
        }
    }

    /// 체결 데이터 파싱.
    ///
    /// 해외 실시간 체결 데이터 필드 (KIS API 문서 참조)
    fn parse_trade_data(&self, data: &str) -> Option<UsRealtimeTrade> {
        let fields: Vec<&str> = data.split('^').collect();

        if fields.len() < 15 {
            warn!("해외 체결 데이터 필드 부족: {}", fields.len());
            return None;
        }

        // 필드 위치는 KIS API 문서에 따름
        // RSYM(실시간종목코드): D+거래소코드+종목코드 (예: DNASAAPL)
        let rsym = fields[0];
        let (exchange_code, symbol) = if rsym.len() > 4 {
            let excd = &rsym[1..4]; // NAS, NYS, AMS
            let sym = &rsym[4..];
            (excd.to_string(), sym.to_string())
        } else {
            return None;
        };

        Some(UsRealtimeTrade {
            symbol,
            exchange_code,
            trade_time: fields[1].to_string(),
            price: fields[2].parse().unwrap_or(Decimal::ZERO),
            volume: fields[9].parse().unwrap_or(0),
            prev_close: fields[6].parse().unwrap_or(Decimal::ZERO),
            change: fields[4].parse().unwrap_or(Decimal::ZERO),
            change_rate: fields[5].parse().unwrap_or(Decimal::ZERO),
        })
    }

    /// 호가 데이터 파싱.
    fn parse_orderbook_data(&self, data: &str) -> Option<UsRealtimeOrderbook> {
        let fields: Vec<&str> = data.split('^').collect();

        if fields.len() < 10 {
            warn!("해외 호가 데이터 필드 부족: {}", fields.len());
            return None;
        }

        let rsym = fields[0];
        let (exchange_code, symbol) = if rsym.len() > 4 {
            let excd = &rsym[1..4];
            let sym = &rsym[4..];
            (excd.to_string(), sym.to_string())
        } else {
            return None;
        };

        Some(UsRealtimeOrderbook {
            symbol,
            exchange_code,
            orderbook_time: fields[1].to_string(),
            bid_price: fields[2].parse().unwrap_or(Decimal::ZERO),
            bid_volume: fields[3].parse().unwrap_or(0),
            ask_price: fields[4].parse().unwrap_or(Decimal::ZERO),
            ask_volume: fields[5].parse().unwrap_or(0),
        })
    }

    /// 실시간 체결가 구독 추가.
    ///
    /// # Arguments
    /// * `symbol` - 종목코드 (예: "AAPL")
    /// * `exchange_code` - 거래소 코드 (NAS, NYS, AMS)
    pub fn add_trade_subscription(&mut self, symbol: &str, exchange_code: &str) {
        let info = SubscriptionInfo {
            symbol: symbol.to_string(),
            exchange_code: exchange_code.to_string(),
        };

        if !self
            .subscribed_trades
            .iter()
            .any(|s| s.symbol == symbol && s.exchange_code == exchange_code)
        {
            self.subscribed_trades.push(info);
        }
    }

    /// 실시간 호가 구독 추가.
    pub fn add_orderbook_subscription(&mut self, symbol: &str, exchange_code: &str) {
        let info = SubscriptionInfo {
            symbol: symbol.to_string(),
            exchange_code: exchange_code.to_string(),
        };

        if !self
            .subscribed_orderbooks
            .iter()
            .any(|s| s.symbol == symbol && s.exchange_code == exchange_code)
        {
            self.subscribed_orderbooks.push(info);
        }
    }

    /// 체결가 구독 제거.
    pub fn remove_trade_subscription(&mut self, symbol: &str, exchange_code: &str) {
        self.subscribed_trades
            .retain(|s| !(s.symbol == symbol && s.exchange_code == exchange_code));
    }

    /// 호가 구독 제거.
    pub fn remove_orderbook_subscription(&mut self, symbol: &str, exchange_code: &str) {
        self.subscribed_orderbooks
            .retain(|s| !(s.symbol == symbol && s.exchange_code == exchange_code));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_tr_key() {
        assert_eq!(KisUsWebSocket::make_tr_key("NAS", "AAPL"), "DNASAAPL");
        assert_eq!(KisUsWebSocket::make_tr_key("NYS", "KO"), "DNYSKO");
        assert_eq!(KisUsWebSocket::make_tr_key("NAS", "TQQQ"), "DNASTQQQ");
    }

    #[test]
    fn test_parse_trade_data() {
        // 테스트용 체결 데이터 (형식: RSYM^TIME^PRICE^...)
        let data = "DNASAAPL^093000^150.25^0^2.50^1.69^147.75^0^0^100^0^0^0^0^0";

        let oauth = create_mock_oauth();
        let ws = KisUsWebSocket::new(oauth);

        let trade = ws.parse_trade_data(data);
        assert!(trade.is_some());

        let trade = trade.unwrap();
        assert_eq!(trade.symbol, "AAPL");
        assert_eq!(trade.exchange_code, "NAS");
        assert_eq!(trade.price, Decimal::new(15025, 2));
    }

    #[test]
    fn test_subscribe_message_format() {
        let oauth = create_mock_oauth();
        let ws = KisUsWebSocket::new(oauth);

        let msg = ws.create_subscribe_message("test_key", "HDFSCNT0", "DNASAAPL", true);

        assert!(msg.contains("approval_key"));
        assert!(msg.contains("HDFSCNT0"));
        assert!(msg.contains("DNASAAPL"));
        assert!(msg.contains("\"tr_type\":\"1\""));
    }

    #[test]
    fn test_subscription_management() {
        let oauth = create_mock_oauth();
        let mut ws = KisUsWebSocket::new(oauth);

        ws.add_trade_subscription("AAPL", "NAS");
        ws.add_trade_subscription("MSFT", "NAS");
        assert_eq!(ws.subscribed_trades.len(), 2);

        // 중복 추가 방지
        ws.add_trade_subscription("AAPL", "NAS");
        assert_eq!(ws.subscribed_trades.len(), 2);

        // 제거
        ws.remove_trade_subscription("AAPL", "NAS");
        assert_eq!(ws.subscribed_trades.len(), 1);
        assert_eq!(ws.subscribed_trades[0].symbol, "MSFT");
    }

    fn create_mock_oauth() -> KisOAuth {
        use super::super::config::{KisAccountType, KisConfig};
        let config = KisConfig::new(
            "test_app_key".to_string(),
            "test_app_secret".to_string(),
            "12345678-01".to_string(),
            KisAccountType::Paper,
        );
        KisOAuth::new(config)
    }
}
