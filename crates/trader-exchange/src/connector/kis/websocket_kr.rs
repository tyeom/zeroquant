//! KIS 국내 주식 실시간 시세 WebSocket 클라이언트.
//!
//! 한국투자증권 WebSocket API를 통해 국내 주식의 실시간 체결가와 호가를 수신합니다.
//!
//! # 지원 채널
//!
//! - `H0STCNT0`: 실시간 체결가
//! - `H0STASP0`: 실시간 호가
//!
//! # 사용 예제
//!
//! ```rust,ignore
//! use trader_exchange::connector::kis::{KisConfig, KisOAuth, KisKrWebSocket};
//!
//! let config = KisConfig::new("app_key", "app_secret", "12345678-01");
//! let oauth = KisOAuth::new(config);
//! let mut ws = KisKrWebSocket::new(oauth);
//!
//! // 삼성전자(005930) 실시간 체결가 구독
//! ws.subscribe_trade("005930").await?;
//!
//! // 메시지 수신
//! while let Some(msg) = ws.recv().await {
//!     println!("Received: {:?}", msg);
//! }
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
use tokio::time::interval;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

/// 재연결 최대 시도 횟수.
const MAX_RECONNECT_ATTEMPTS: u32 = 3;

/// 재연결 대기 시간 (초).
const RECONNECT_DELAY_SECS: u64 = 5;

/// Ping 간격 (초).
const PING_INTERVAL_SECS: u64 = 30;

/// 국내 주식 실시간 체결 데이터.
#[derive(Debug, Clone)]
pub struct KrRealtimeTrade {
    /// 종목코드
    pub symbol: String,
    /// 체결가
    pub price: Decimal,
    /// 체결량
    pub volume: i64,
    /// 누적거래량
    pub acc_volume: i64,
    /// 체결시간 (HHMMSS)
    pub trade_time: String,
    /// 전일대비 부호 (1:상한, 2:상승, 3:보합, 4:하한, 5:하락)
    pub sign: String,
    /// 전일대비
    pub change: Decimal,
    /// 등락률
    pub change_rate: Decimal,
}

/// 국내 주식 실시간 호가 데이터.
#[derive(Debug, Clone)]
pub struct KrRealtimeOrderbook {
    /// 종목코드
    pub symbol: String,
    /// 매도호가 (1~10호가)
    pub ask_prices: Vec<Decimal>,
    /// 매도호가 잔량
    pub ask_volumes: Vec<i64>,
    /// 매수호가 (1~10호가)
    pub bid_prices: Vec<Decimal>,
    /// 매수호가 잔량
    pub bid_volumes: Vec<i64>,
    /// 호가시간 (HHMMSS)
    pub orderbook_time: String,
}

/// 국내 실시간 메시지 타입.
#[derive(Debug, Clone)]
pub enum KrRealtimeMessage {
    /// 체결가
    Trade(KrRealtimeTrade),
    /// 호가
    Orderbook(KrRealtimeOrderbook),
    /// 연결 상태 변경
    ConnectionStatus(bool),
    /// 에러
    Error(String),
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
    tr_key: String, // 종목코드
}

/// KIS 국내 주식 실시간 WebSocket 클라이언트.
pub struct KisKrWebSocket {
    oauth: KisOAuth,
    tx: Option<mpsc::Sender<KrRealtimeMessage>>,
    rx: Option<mpsc::Receiver<KrRealtimeMessage>>,
    subscribed_trades: Vec<String>,
    subscribed_orderbooks: Vec<String>,
    is_connected: Arc<tokio::sync::RwLock<bool>>,
}

impl KisKrWebSocket {
    /// 새로운 국내 WebSocket 클라이언트 생성.
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
    pub fn take_receiver(&mut self) -> Option<mpsc::Receiver<KrRealtimeMessage>> {
        self.rx.take()
    }

    /// 연결 상태 확인.
    pub async fn is_connected(&self) -> bool {
        *self.is_connected.read().await
    }

    /// WebSocket 연결 및 메시지 수신 시작.
    ///
    /// 이 메서드는 별도 태스크에서 실행해야 합니다.
    pub async fn connect(&mut self) -> Result<(), ExchangeError> {
        let mut reconnect_attempts = 0;

        loop {
            match self.connect_internal().await {
                Ok(_) => {
                    // 정상 종료
                    info!("KIS KR WebSocket 연결 종료");
                    break;
                }
                Err(e) => {
                    error!("KIS KR WebSocket 에러: {}", e);
                    reconnect_attempts += 1;

                    if reconnect_attempts > MAX_RECONNECT_ATTEMPTS {
                        error!("최대 재연결 시도 횟수 초과 ({}회)", MAX_RECONNECT_ATTEMPTS);
                        if let Some(tx) = &self.tx {
                            let _ = tx
                                .send(KrRealtimeMessage::Error(format!(
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

                    // WebSocket 키 초기화 (재발급 필요)
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

        info!("KIS KR WebSocket 연결 중: {}", ws_url);

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
            let _ = tx.send(KrRealtimeMessage::ConnectionStatus(true)).await;
        }

        info!("KIS KR WebSocket 연결 성공");

        // 기존 구독 복원
        let trades = self.subscribed_trades.clone();
        let orderbooks = self.subscribed_orderbooks.clone();

        for symbol in &trades {
            let msg = self.create_subscribe_message(&approval_key, tr_id::WS_KR_TRADE, symbol, true);
            write
                .send(Message::Text(msg))
                .await
                .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;
            debug!("체결가 구독 복원: {}", symbol);
        }

        for symbol in &orderbooks {
            let msg =
                self.create_subscribe_message(&approval_key, tr_id::WS_KR_ORDERBOOK, symbol, true);
            write
                .send(Message::Text(msg))
                .await
                .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;
            debug!("호가 구독 복원: {}", symbol);
        }

        // Ping 타이머
        let mut ping_interval = interval(Duration::from_secs(PING_INTERVAL_SECS));

        // 메시지 수신 루프
        loop {
            tokio::select! {
                // WebSocket 메시지 수신
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
                // Ping 전송
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
            let _ = tx.send(KrRealtimeMessage::ConnectionStatus(false)).await;
        }

        Err(ExchangeError::NetworkError("연결 끊김".to_string()))
    }

    /// 구독 메시지 생성.
    fn create_subscribe_message(
        &self,
        approval_key: &str,
        tr_id: &str,
        symbol: &str,
        subscribe: bool,
    ) -> String {
        let request = WsSubscribeRequest {
            header: WsHeader {
                approval_key: approval_key.to_string(),
                custtype: "P".to_string(), // P: 개인
                tr_type: if subscribe { "1" } else { "2" }.to_string(),
                content_type: "utf-8".to_string(),
            },
            body: WsBody {
                input: WsInput {
                    tr_id: tr_id.to_string(),
                    tr_key: symbol.to_string(),
                },
            },
        };

        serde_json::to_string(&request).unwrap_or_default()
    }

    /// 수신 메시지 처리.
    async fn handle_message(&self, text: &str) {
        // KIS WebSocket 메시지는 | 구분자로 분리됨
        // 형식: 0|H0STCNT0|001|005930^...
        let parts: Vec<&str> = text.split('|').collect();

        if parts.len() < 4 {
            // JSON 응답 (구독 확인 등)
            debug!("JSON 응답: {}", text);
            return;
        }

        let tr_id = parts[1];
        let data = parts[3];

        match tr_id {
            "H0STCNT0" => {
                // 실시간 체결
                if let Some(trade) = self.parse_trade_data(data) {
                    if let Some(tx) = &self.tx {
                        let _ = tx.send(KrRealtimeMessage::Trade(trade)).await;
                    }
                }
            }
            "H0STASP0" => {
                // 실시간 호가
                if let Some(orderbook) = self.parse_orderbook_data(data) {
                    if let Some(tx) = &self.tx {
                        let _ = tx.send(KrRealtimeMessage::Orderbook(orderbook)).await;
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
    /// 데이터 형식: 종목코드^체결시간^체결가^...
    fn parse_trade_data(&self, data: &str) -> Option<KrRealtimeTrade> {
        let fields: Vec<&str> = data.split('^').collect();

        if fields.len() < 20 {
            warn!("체결 데이터 필드 부족: {}", fields.len());
            return None;
        }

        Some(KrRealtimeTrade {
            symbol: fields[0].to_string(),
            trade_time: fields[1].to_string(),
            price: fields[2].parse().unwrap_or(Decimal::ZERO),
            change: fields[4].parse().unwrap_or(Decimal::ZERO),
            change_rate: fields[5].parse().unwrap_or(Decimal::ZERO),
            sign: fields[3].to_string(),
            volume: fields[12].parse().unwrap_or(0),
            acc_volume: fields[13].parse().unwrap_or(0),
        })
    }

    /// 호가 데이터 파싱.
    fn parse_orderbook_data(&self, data: &str) -> Option<KrRealtimeOrderbook> {
        let fields: Vec<&str> = data.split('^').collect();

        if fields.len() < 50 {
            warn!("호가 데이터 필드 부족: {}", fields.len());
            return None;
        }

        let mut ask_prices = Vec::with_capacity(10);
        let mut ask_volumes = Vec::with_capacity(10);
        let mut bid_prices = Vec::with_capacity(10);
        let mut bid_volumes = Vec::with_capacity(10);

        // 호가 데이터 파싱 (매도1~10호가, 매수1~10호가)
        // 실제 필드 위치는 KIS API 문서 참조
        for i in 0..10 {
            let ask_price_idx = 3 + i * 2;
            let ask_vol_idx = 4 + i * 2;
            let bid_price_idx = 23 + i * 2;
            let bid_vol_idx = 24 + i * 2;

            if ask_price_idx < fields.len() && ask_vol_idx < fields.len() {
                ask_prices.push(fields[ask_price_idx].parse().unwrap_or(Decimal::ZERO));
                ask_volumes.push(fields[ask_vol_idx].parse().unwrap_or(0));
            }

            if bid_price_idx < fields.len() && bid_vol_idx < fields.len() {
                bid_prices.push(fields[bid_price_idx].parse().unwrap_or(Decimal::ZERO));
                bid_volumes.push(fields[bid_vol_idx].parse().unwrap_or(0));
            }
        }

        Some(KrRealtimeOrderbook {
            symbol: fields[0].to_string(),
            orderbook_time: fields[1].to_string(),
            ask_prices,
            ask_volumes,
            bid_prices,
            bid_volumes,
        })
    }

    /// 실시간 체결가 구독 추가.
    pub fn add_trade_subscription(&mut self, symbol: &str) {
        if !self.subscribed_trades.contains(&symbol.to_string()) {
            self.subscribed_trades.push(symbol.to_string());
        }
    }

    /// 실시간 호가 구독 추가.
    pub fn add_orderbook_subscription(&mut self, symbol: &str) {
        if !self.subscribed_orderbooks.contains(&symbol.to_string()) {
            self.subscribed_orderbooks.push(symbol.to_string());
        }
    }

    /// 체결가 구독 제거.
    pub fn remove_trade_subscription(&mut self, symbol: &str) {
        self.subscribed_trades.retain(|s| s != symbol);
    }

    /// 호가 구독 제거.
    pub fn remove_orderbook_subscription(&mut self, symbol: &str) {
        self.subscribed_orderbooks.retain(|s| s != symbol);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_trade_data() {
        // 테스트용 체결 데이터 (실제 형식과 다를 수 있음)
        let data = "005930^093000^70000^2^500^0.72^0^0^0^0^0^0^1000^50000000^0^0^0^0^0^0";

        let oauth = create_mock_oauth();
        let ws = KisKrWebSocket::new(oauth);

        let trade = ws.parse_trade_data(data);
        assert!(trade.is_some());

        let trade = trade.unwrap();
        assert_eq!(trade.symbol, "005930");
        assert_eq!(trade.trade_time, "093000");
        assert_eq!(trade.price, Decimal::new(70000, 0));
    }

    #[test]
    fn test_subscribe_message_format() {
        let oauth = create_mock_oauth();
        let ws = KisKrWebSocket::new(oauth);

        let msg = ws.create_subscribe_message("test_key", "H0STCNT0", "005930", true);

        assert!(msg.contains("approval_key"));
        assert!(msg.contains("H0STCNT0"));
        assert!(msg.contains("005930"));
        assert!(msg.contains("\"tr_type\":\"1\""));
    }

    #[test]
    fn test_unsubscribe_message_format() {
        let oauth = create_mock_oauth();
        let ws = KisKrWebSocket::new(oauth);

        let msg = ws.create_subscribe_message("test_key", "H0STCNT0", "005930", false);

        assert!(msg.contains("\"tr_type\":\"2\""));
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
