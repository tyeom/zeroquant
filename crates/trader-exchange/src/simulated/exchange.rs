//! 시뮬레이션 거래소 구현.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use trader_core::{
    Kline, OrderBook, OrderBookLevel, OrderRequest, OrderStatus, OrderStatusType, OrderType,
    Position, Side, Symbol, Ticker, Timeframe, TradeTick,
};

use crate::traits::{AccountInfo, Balance, Exchange, ExchangeResult, MarketEvent, UserEvent};
use crate::ExchangeError;

use super::data_feed::{DataFeed, DataFeedConfig};
use super::matching_engine::{FillType, MatchingEngine, OrderMatch};
use super::stream::{EventBroadcaster, SimulatedMarketStream, SimulatedUserStream};

/// 시뮬레이션 거래소 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulatedConfig {
    /// 자산별 초기 잔고
    pub initial_balances: HashMap<String, Decimal>,
    /// 거래 수수료율 (예: 0.1%의 경우 0.001)
    pub fee_rate: Decimal,
    /// 시장가 주문의 슬리피지율
    pub slippage_rate: Decimal,
    /// 포지션 추적 활성화 여부
    pub enable_positions: bool,
    /// 데이터 피드 설정
    pub data_feed_config: DataFeedConfig,
}

impl Default for SimulatedConfig {
    fn default() -> Self {
        let mut initial_balances = HashMap::new();
        initial_balances.insert("USDT".to_string(), dec!(10000));

        Self {
            initial_balances,
            fee_rate: dec!(0.001),       // 0.1%
            slippage_rate: dec!(0.0005), // 0.05%
            enable_positions: false,
            data_feed_config: DataFeedConfig::default(),
        }
    }
}

impl SimulatedConfig {
    /// 자산의 초기 잔고를 추가합니다.
    pub fn with_initial_balance(mut self, asset: &str, amount: Decimal) -> Self {
        self.initial_balances.insert(asset.to_string(), amount);
        self
    }

    /// 수수료율을 설정합니다.
    pub fn with_fee_rate(mut self, rate: Decimal) -> Self {
        self.fee_rate = rate;
        self
    }

    /// 슬리피지율을 설정합니다.
    pub fn with_slippage_rate(mut self, rate: Decimal) -> Self {
        self.slippage_rate = rate;
        self
    }
}

/// 내부 계정 상태.
#[derive(Debug, Clone)]
struct AccountState {
    /// 현재 잔고
    balances: HashMap<String, Balance>,
    /// 오픈 포지션 (선물 모드용)
    positions: HashMap<Symbol, Position>,
    /// 체결된 주문 이력
    order_history: Vec<OrderMatch>,
}

impl AccountState {
    fn new(initial_balances: &HashMap<String, Decimal>) -> Self {
        let balances = initial_balances
            .iter()
            .map(|(asset, amount)| {
                (
                    asset.clone(),
                    Balance {
                        asset: asset.clone(),
                        free: *amount,
                        locked: dec!(0),
                    },
                )
            })
            .collect();

        Self {
            balances,
            positions: HashMap::new(),
            order_history: Vec::new(),
        }
    }

    fn get_balance(&self, asset: &str) -> Balance {
        self.balances.get(asset).cloned().unwrap_or(Balance {
            asset: asset.to_string(),
            free: dec!(0),
            locked: dec!(0),
        })
    }

    fn update_balance(&mut self, asset: &str, free_delta: Decimal, locked_delta: Decimal) {
        let balance = self.balances.entry(asset.to_string()).or_insert(Balance {
            asset: asset.to_string(),
            free: dec!(0),
            locked: dec!(0),
        });

        balance.free += free_delta;
        balance.locked += locked_delta;
    }
}

/// 주문 추적 상태.
#[derive(Debug, Clone)]
struct OrderState {
    /// 원본 요청
    request: OrderRequest,
    /// 주문 ID
    order_id: String,
    /// 상태 유형
    status_type: OrderStatusType,
    /// 체결 수량
    filled_quantity: Decimal,
    /// 평균 체결 가격
    average_price: Option<Decimal>,
    /// 생성 시각
    created_at: DateTime<Utc>,
    /// 갱신 시각
    updated_at: DateTime<Utc>,
}

impl OrderState {
    fn to_order_status(&self) -> OrderStatus {
        OrderStatus {
            order_id: self.order_id.clone(),
            client_order_id: self.request.client_order_id.clone(),
            status: self.status_type,
            filled_quantity: self.filled_quantity,
            average_price: self.average_price,
            updated_at: self.updated_at,
        }
    }
}

/// 백테스팅 및 모의투자를 위한 시뮬레이션 거래소.
pub struct SimulatedExchange {
    /// 설정
    config: SimulatedConfig,
    /// 연결 상태
    connected: bool,
    /// 계정 상태
    account: Arc<RwLock<AccountState>>,
    /// 데이터 피드
    data_feed: Arc<RwLock<DataFeed>>,
    /// 매칭 엔진
    matching_engine: Arc<RwLock<MatchingEngine>>,
    /// 주문 상태
    orders: Arc<RwLock<HashMap<String, OrderState>>>,
    /// 시장 이벤트 브로드캐스터
    market_broadcaster: Arc<EventBroadcaster<MarketEvent>>,
    /// 사용자 이벤트 브로드캐스터
    user_broadcaster: Arc<EventBroadcaster<UserEvent>>,
}

impl SimulatedExchange {
    /// 새로운 시뮬레이션 거래소를 생성합니다.
    pub fn new(config: SimulatedConfig) -> Self {
        let account = AccountState::new(&config.initial_balances);
        let data_feed = DataFeed::new(config.data_feed_config.clone());
        let matching_engine = MatchingEngine::new(config.fee_rate, config.slippage_rate);

        Self {
            config,
            connected: false,
            account: Arc::new(RwLock::new(account)),
            data_feed: Arc::new(RwLock::new(data_feed)),
            matching_engine: Arc::new(RwLock::new(matching_engine)),
            orders: Arc::new(RwLock::new(HashMap::new())),
            market_broadcaster: Arc::new(EventBroadcaster::new()),
            user_broadcaster: Arc::new(EventBroadcaster::new()),
        }
    }

    /// 심볼의 과거 Kline 데이터를 로드합니다.
    pub async fn load_klines(&self, symbol: Symbol, timeframe: Timeframe, klines: Vec<Kline>) {
        let mut feed = self.data_feed.write().await;
        feed.load_klines(symbol, timeframe, klines);
    }

    /// CSV 파일에서 Kline 데이터를 로드합니다.
    pub async fn load_from_csv(
        &self,
        symbol: Symbol,
        timeframe: Timeframe,
        path: impl AsRef<std::path::Path>,
    ) -> ExchangeResult<usize> {
        let mut feed = self.data_feed.write().await;
        feed.load_from_csv(symbol, timeframe, path)
    }

    /// 이 거래소의 시장 스트림을 생성합니다.
    pub async fn create_market_stream(&self) -> SimulatedMarketStream {
        let rx = self.market_broadcaster.subscribe(1000).await;
        SimulatedMarketStream::new(rx)
    }

    /// 이 거래소의 사용자 스트림을 생성합니다.
    pub async fn create_user_stream(&self) -> SimulatedUserStream {
        let rx = self.user_broadcaster.subscribe(100).await;
        SimulatedUserStream::new(rx)
    }

    /// 시뮬레이션을 한 단계 진행합니다.
    /// 처리된 Kline을 반환합니다.
    pub async fn step(&self, symbol: &Symbol, timeframe: Timeframe) -> Option<Kline> {
        let kline = {
            let mut feed = self.data_feed.write().await;
            feed.next_kline(symbol, timeframe)?
        };

        // 대기 중인 주문 처리
        let matches = {
            let mut engine = self.matching_engine.write().await;
            engine.process_kline(symbol, &kline)
        };

        // 주문 매칭 결과를 계정에 적용
        for order_match in matches {
            self.apply_order_match(&order_match).await;
        }

        // 시장 이벤트 브로드캐스트
        self.market_broadcaster
            .broadcast(MarketEvent::Kline(kline.clone()))
            .await;

        // 티커 업데이트도 브로드캐스트
        let ticker = self.kline_to_ticker(&kline);
        self.market_broadcaster
            .broadcast(MarketEvent::Ticker(ticker))
            .await;

        Some(kline)
    }

    /// 데이터가 소진될 때까지 시뮬레이션을 실행합니다.
    /// 각 단계마다 콜백을 호출합니다.
    pub async fn run_simulation<F>(&self, symbol: &Symbol, timeframe: Timeframe, mut on_kline: F)
    where
        F: FnMut(&Kline),
    {
        while let Some(kline) = self.step(symbol, timeframe).await {
            on_kline(&kline);
        }
    }

    /// 시뮬레이션을 처음으로 리셋합니다.
    pub async fn reset(&self) {
        // 데이터 피드 리셋
        {
            let mut feed = self.data_feed.write().await;
            feed.reset();
        }

        // 계정 리셋
        {
            let mut account = self.account.write().await;
            *account = AccountState::new(&self.config.initial_balances);
        }

        // 매칭 엔진 초기화
        {
            let mut engine = self.matching_engine.write().await;
            engine.clear();
        }

        // 주문 초기화
        {
            let mut orders = self.orders.write().await;
            orders.clear();
        }
    }

    /// 현재 시뮬레이션 시간을 가져옵니다.
    pub async fn current_time(&self) -> Option<DateTime<Utc>> {
        let feed = self.data_feed.read().await;
        feed.current_time()
    }

    /// 시뮬레이션에 더 많은 데이터가 있는지 확인합니다.
    pub async fn has_more_data(&self) -> bool {
        let feed = self.data_feed.read().await;
        !feed.is_exhausted()
    }

    /// 주문 이력을 가져옵니다.
    pub async fn get_order_history(&self) -> Vec<OrderMatch> {
        let account = self.account.read().await;
        account.order_history.clone()
    }

    /// 총 손익을 가져옵니다.
    pub async fn get_total_pnl(&self) -> Decimal {
        let account = self.account.read().await;
        let mut total_pnl = dec!(0);

        // 주문 이력에서 계산
        for fill in &account.order_history {
            match fill.fill_type {
                FillType::Full | FillType::Partial => {
                    // 이것은 단순화된 손익 계산입니다
                    // 실제 구현에서는 포지션을 제대로 추적해야 합니다
                    total_pnl -= fill.commission;
                }
                FillType::None => {}
            }
        }

        total_pnl
    }

    /// 주문 매칭 결과를 계정에 적용합니다.
    async fn apply_order_match(&self, order_match: &OrderMatch) {
        let mut account = self.account.write().await;

        // 주문 상세 정보 가져오기
        let order_state = {
            let orders = self.orders.read().await;
            orders.get(&order_match.order_id).cloned()
        };

        if let Some(order_state) = order_state {
            let request = &order_state.request;
            let symbol = &request.symbol;

            match request.side {
                Side::Buy => {
                    // 견적 통화 차감, 기준 통화 추가
                    let total_cost =
                        order_match.filled_quantity * order_match.fill_price + order_match.commission;
                    account.update_balance(&symbol.quote, -total_cost, dec!(0));
                    account.update_balance(&symbol.base, order_match.filled_quantity, dec!(0));
                }
                Side::Sell => {
                    // 기준 통화 차감, 견적 통화 추가
                    let total_received =
                        order_match.filled_quantity * order_match.fill_price - order_match.commission;
                    account.update_balance(&symbol.base, -order_match.filled_quantity, dec!(0));
                    account.update_balance(&symbol.quote, total_received, dec!(0));
                }
            }

            // 주문 상태 업데이트
            {
                let mut orders = self.orders.write().await;
                if let Some(state) = orders.get_mut(&order_match.order_id) {
                    state.status_type = if order_match.fill_type == FillType::Full {
                        OrderStatusType::Filled
                    } else {
                        OrderStatusType::PartiallyFilled
                    };
                    state.filled_quantity = order_match.filled_quantity;
                    state.average_price = Some(order_match.fill_price);
                    state.updated_at = order_match.timestamp;
                }
            }

            // 사용자 이벤트 브로드캐스트 - 주문 업데이트
            let updated_status = OrderStatus {
                order_id: order_match.order_id.clone(),
                client_order_id: request.client_order_id.clone(),
                status: if order_match.fill_type == FillType::Full {
                    OrderStatusType::Filled
                } else {
                    OrderStatusType::PartiallyFilled
                },
                filled_quantity: order_match.filled_quantity,
                average_price: Some(order_match.fill_price),
                updated_at: order_match.timestamp,
            };
            self.user_broadcaster
                .broadcast(UserEvent::OrderUpdate(updated_status))
                .await;

            // 잔고 업데이트 브로드캐스트
            let quote_balance = account.get_balance(&symbol.quote);
            self.user_broadcaster
                .broadcast(UserEvent::BalanceUpdate(quote_balance))
                .await;
        }

        // 주문 이력에 추가
        account.order_history.push(order_match.clone());
    }

    /// Kline을 티커로 변환합니다.
    fn kline_to_ticker(&self, kline: &Kline) -> Ticker {
        let price_change = kline.close - kline.open;
        let price_change_pct = if kline.open != dec!(0) {
            (price_change / kline.open) * dec!(100)
        } else {
            dec!(0)
        };

        Ticker {
            symbol: kline.symbol.clone(),
            last: kline.close,
            bid: kline.close * dec!(0.9999),
            ask: kline.close * dec!(1.0001),
            high_24h: kline.high,
            low_24h: kline.low,
            volume_24h: kline.volume,
            change_24h: price_change,
            change_24h_percent: price_change_pct,
            timestamp: kline.close_time,
        }
    }

    /// 주문 요청을 검증합니다.
    fn validate_order(&self, request: &OrderRequest, _current_price: Decimal) -> ExchangeResult<()> {
        if request.quantity <= dec!(0) {
            return Err(ExchangeError::InvalidQuantity("Quantity must be positive".into()));
        }

        match request.order_type {
            OrderType::Limit => {
                if request.price.is_none() {
                    return Err(ExchangeError::OrderRejected(
                        "Limit order requires price".into(),
                    ));
                }
            }
            OrderType::StopLoss | OrderType::StopLossLimit | OrderType::TakeProfit | OrderType::TakeProfitLimit => {
                if request.stop_price.is_none() {
                    return Err(ExchangeError::OrderRejected(
                        "Stop order requires stop price".into(),
                    ));
                }
            }
            _ => {}
        }

        Ok(())
    }
}

#[async_trait]
impl Exchange for SimulatedExchange {
    fn name(&self) -> &str {
        "SimulatedExchange"
    }

    async fn is_connected(&self) -> bool {
        self.connected
    }

    async fn connect(&mut self) -> ExchangeResult<()> {
        self.connected = true;
        self.market_broadcaster
            .broadcast(MarketEvent::Connected)
            .await;
        Ok(())
    }

    async fn disconnect(&mut self) -> ExchangeResult<()> {
        self.connected = false;
        self.market_broadcaster
            .broadcast(MarketEvent::Disconnected)
            .await;
        Ok(())
    }

    async fn get_account(&self) -> ExchangeResult<AccountInfo> {
        let account = self.account.read().await;
        Ok(AccountInfo {
            balances: account.balances.values().cloned().collect(),
            can_trade: true,
            can_withdraw: false,
            can_deposit: false,
        })
    }

    async fn get_balance(&self, asset: &str) -> ExchangeResult<Balance> {
        let account = self.account.read().await;
        Ok(account.get_balance(asset))
    }

    async fn get_ticker(&self, symbol: &Symbol) -> ExchangeResult<Ticker> {
        let feed = self.data_feed.read().await;
        feed.get_ticker(symbol)
            .ok_or_else(|| ExchangeError::SymbolNotFound(symbol.to_string()))
    }

    async fn get_order_book(&self, symbol: &Symbol, _limit: Option<u32>) -> ExchangeResult<OrderBook> {
        // 현재 가격에서 시뮬레이션된 호가창 생성
        let feed = self.data_feed.read().await;
        let current_price = feed
            .get_current_price(symbol)
            .ok_or_else(|| ExchangeError::SymbolNotFound(symbol.to_string()))?;

        // 스프레드를 포함한 간단한 호가창 생성
        let mut bids = Vec::new();
        let mut asks = Vec::new();

        for i in 1..=10 {
            let spread = current_price * dec!(0.0001) * Decimal::from(i);
            let volume = Decimal::from_f64_retain(100.0 / i as f64).unwrap_or(dec!(10));

            bids.push(OrderBookLevel {
                price: current_price - spread,
                quantity: volume,
            });
            asks.push(OrderBookLevel {
                price: current_price + spread,
                quantity: volume,
            });
        }

        Ok(OrderBook {
            symbol: symbol.clone(),
            bids,
            asks,
            timestamp: Utc::now(),
        })
    }

    async fn get_recent_trades(
        &self,
        symbol: &Symbol,
        limit: Option<u32>,
    ) -> ExchangeResult<Vec<TradeTick>> {
        // Kline 데이터에서 시뮬레이션된 거래 생성
        let feed = self.data_feed.read().await;
        let klines = feed.get_historical_klines(symbol, Timeframe::M1, limit.unwrap_or(100) as usize);

        let trades: Vec<TradeTick> = klines
            .iter()
            .enumerate()
            .map(|(idx, kline)| {
                let num_trades = kline.num_trades.unwrap_or(1);
                let trade_qty = if num_trades > 0 {
                    kline.volume / Decimal::from(num_trades)
                } else {
                    kline.volume
                };

                TradeTick {
                    symbol: symbol.clone(),
                    id: format!("{}_{}", kline.open_time.timestamp_millis(), idx),
                    price: kline.close,
                    quantity: trade_qty,
                    side: if kline.close >= kline.open { Side::Buy } else { Side::Sell },
                    timestamp: kline.close_time,
                }
            })
            .collect();

        Ok(trades)
    }

    async fn get_klines(
        &self,
        symbol: &Symbol,
        timeframe: Timeframe,
        limit: Option<u32>,
    ) -> ExchangeResult<Vec<Kline>> {
        let feed = self.data_feed.read().await;
        Ok(feed.get_historical_klines(symbol, timeframe, limit.unwrap_or(100) as usize))
    }

    async fn place_order(&self, request: &OrderRequest) -> ExchangeResult<String> {
        // 현재 가격 가져오기
        let current_price = {
            let feed = self.data_feed.read().await;
            feed.get_current_price(&request.symbol)
                .ok_or_else(|| ExchangeError::SymbolNotFound(request.symbol.to_string()))?
        };

        // 주문 검증
        self.validate_order(request, current_price)?;

        // 잔고 확인
        {
            let account = self.account.read().await;
            match request.side {
                Side::Buy => {
                    let required = request.quantity * request.price.unwrap_or(current_price);
                    let balance = account.get_balance(&request.symbol.quote);
                    if balance.free < required {
                        return Err(ExchangeError::InsufficientBalance(format!(
                            "Need {} {}, have {}",
                            required, request.symbol.quote, balance.free
                        )));
                    }
                }
                Side::Sell => {
                    let balance = account.get_balance(&request.symbol.base);
                    if balance.free < request.quantity {
                        return Err(ExchangeError::InsufficientBalance(format!(
                            "Need {} {}, have {}",
                            request.quantity, request.symbol.base, balance.free
                        )));
                    }
                }
            }
        }

        // 잔고 잠금
        {
            let mut account = self.account.write().await;
            match request.side {
                Side::Buy => {
                    let required = request.quantity * request.price.unwrap_or(current_price);
                    account.update_balance(&request.symbol.quote, -required, required);
                }
                Side::Sell => {
                    account.update_balance(&request.symbol.base, -request.quantity, request.quantity);
                }
            }
        }

        // 매칭 엔진에 제출
        let order_match = {
            let mut engine = self.matching_engine.write().await;
            let timestamp = {
                let feed = self.data_feed.read().await;
                feed.current_time().unwrap_or_else(Utc::now)
            };
            engine.submit_order(request, current_price, timestamp)
        };

        let order_id = order_match.order_id.clone();

        // 초기 상태 결정
        let initial_status = match order_match.fill_type {
            FillType::Full => OrderStatusType::Filled,
            FillType::Partial => OrderStatusType::PartiallyFilled,
            FillType::None => OrderStatusType::Open,
        };

        // 주문 상태 생성
        let order_state = OrderState {
            request: request.clone(),
            order_id: order_id.clone(),
            status_type: initial_status,
            filled_quantity: order_match.filled_quantity,
            average_price: if order_match.fill_type != FillType::None {
                Some(order_match.fill_price)
            } else {
                None
            },
            created_at: order_match.timestamp,
            updated_at: order_match.timestamp,
        };

        // 주문 상태 저장
        {
            let mut orders = self.orders.write().await;
            orders.insert(order_id.clone(), order_state);
        }

        // 즉시 체결된 경우, 매칭 적용
        if order_match.fill_type != FillType::None {
            // 먼저 잔고 잠금 해제 (위에서 잠갔으므로)
            {
                let mut account = self.account.write().await;
                match request.side {
                    Side::Buy => {
                        let required = request.quantity * request.price.unwrap_or(current_price);
                        account.update_balance(&request.symbol.quote, required, -required);
                    }
                    Side::Sell => {
                        account.update_balance(&request.symbol.base, request.quantity, -request.quantity);
                    }
                }
            }

            self.apply_order_match(&order_match).await;
        }

        Ok(order_id)
    }

    async fn cancel_order(&self, symbol: &Symbol, order_id: &str) -> ExchangeResult<()> {
        // 매칭 엔진에서 제거
        let cancelled = {
            let mut engine = self.matching_engine.write().await;
            engine.cancel_order(symbol, order_id)
        };

        if !cancelled {
            return Err(ExchangeError::OrderNotFound(order_id.to_string()));
        }

        // 주문 상태 업데이트
        {
            let mut orders = self.orders.write().await;
            if let Some(state) = orders.get_mut(order_id) {
                state.status_type = OrderStatusType::Cancelled;
                state.updated_at = Utc::now();

                // 잔고 잠금 해제
                let mut account = self.account.write().await;
                let request = &state.request;
                match request.side {
                    Side::Buy => {
                        let locked = request.quantity * request.price.unwrap_or(dec!(0));
                        account.update_balance(&request.symbol.quote, locked, -locked);
                    }
                    Side::Sell => {
                        account.update_balance(&request.symbol.base, request.quantity, -request.quantity);
                    }
                }
            }
        }

        Ok(())
    }

    async fn get_order(&self, _symbol: &Symbol, order_id: &str) -> ExchangeResult<OrderStatus> {
        let orders = self.orders.read().await;
        orders
            .get(order_id)
            .map(|s| s.to_order_status())
            .ok_or_else(|| ExchangeError::OrderNotFound(order_id.to_string()))
    }

    async fn get_open_orders(&self, symbol: Option<&Symbol>) -> ExchangeResult<Vec<OrderStatus>> {
        let orders = self.orders.read().await;
        let open_orders: Vec<OrderStatus> = orders
            .values()
            .filter(|state| {
                state.status_type == OrderStatusType::Open
                    || state.status_type == OrderStatusType::PartiallyFilled
            })
            .filter(|state| symbol.map_or(true, |s| &state.request.symbol == s))
            .map(|state| state.to_order_status())
            .collect();

        Ok(open_orders)
    }

    async fn get_positions(&self) -> ExchangeResult<Vec<Position>> {
        if !self.config.enable_positions {
            return Ok(vec![]);
        }

        let account = self.account.read().await;
        Ok(account.positions.values().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulated::data_feed::generate_sample_klines;
    use trader_core::TimeInForce;

    fn create_test_symbol() -> Symbol {
        Symbol::crypto("BTC", "USDT")
    }

    #[tokio::test]
    async fn test_simulated_exchange_basic() {
        let config = SimulatedConfig::default()
            .with_initial_balance("USDT", dec!(10000))
            .with_initial_balance("BTC", dec!(1));

        let mut exchange = SimulatedExchange::new(config);
        exchange.connect().await.unwrap();

        assert!(exchange.is_connected().await);

        let account = exchange.get_account().await.unwrap();
        assert!(account.can_trade);

        let usdt_balance = exchange.get_balance("USDT").await.unwrap();
        assert_eq!(usdt_balance.free, dec!(10000));
    }

    #[tokio::test]
    async fn test_load_and_step() {
        let config = SimulatedConfig::default();
        let exchange = SimulatedExchange::new(config);
        let symbol = create_test_symbol();

        // 샘플 데이터 로드
        let klines = generate_sample_klines(symbol.clone(), Timeframe::M1, 100, dec!(50000), dec!(0.02));
        exchange.load_klines(symbol.clone(), Timeframe::M1, klines).await;

        // 데이터 순차 처리
        let mut count = 0;
        while exchange.step(&symbol, Timeframe::M1).await.is_some() {
            count += 1;
        }

        assert_eq!(count, 100);
    }

    #[tokio::test]
    async fn test_market_order() {
        let config = SimulatedConfig::default()
            .with_initial_balance("USDT", dec!(100000));

        let exchange = SimulatedExchange::new(config);
        let symbol = create_test_symbol();

        // 데이터 로드
        let klines = generate_sample_klines(symbol.clone(), Timeframe::M1, 10, dec!(50000), dec!(0.02));
        exchange.load_klines(symbol.clone(), Timeframe::M1, klines).await;

        // 첫 번째 Kline으로 진행
        exchange.step(&symbol, Timeframe::M1).await;

        // 시장가 매수 주문
        let request = OrderRequest {
            symbol: symbol.clone(),
            side: Side::Buy,
            order_type: OrderType::Market,
            quantity: dec!(0.1),
            price: None,
            stop_price: None,
            time_in_force: TimeInForce::GTC,
            client_order_id: None,
            strategy_id: None,
        };

        let order_id = exchange.place_order(&request).await.unwrap();
        let status = exchange.get_order(&symbol, &order_id).await.unwrap();

        assert_eq!(status.status, OrderStatusType::Filled);

        // 잔고가 변경되었는지 확인
        let btc_balance = exchange.get_balance("BTC").await.unwrap();
        assert!(btc_balance.free > dec!(0));
    }

    #[tokio::test]
    async fn test_limit_order_pending() {
        let config = SimulatedConfig::default()
            .with_initial_balance("USDT", dec!(100000));

        let exchange = SimulatedExchange::new(config);
        let symbol = create_test_symbol();

        // 가격이 약 50000인 데이터 로드
        let klines = generate_sample_klines(symbol.clone(), Timeframe::M1, 10, dec!(50000), dec!(0.01));
        exchange.load_klines(symbol.clone(), Timeframe::M1, klines).await;

        // 첫 번째 Kline으로 진행
        exchange.step(&symbol, Timeframe::M1).await;

        // 현재 가격보다 훨씬 낮은 지정가 매수 주문
        let request = OrderRequest {
            symbol: symbol.clone(),
            side: Side::Buy,
            order_type: OrderType::Limit,
            quantity: dec!(0.1),
            price: Some(dec!(40000)), // 현재 ~50000보다 낮음
            stop_price: None,
            time_in_force: TimeInForce::GTC,
            client_order_id: None,
            strategy_id: None,
        };

        let order_id = exchange.place_order(&request).await.unwrap();
        let status = exchange.get_order(&symbol, &order_id).await.unwrap();

        // 대기 중이어야 함 (즉시 체결되지 않음)
        assert_eq!(status.status, OrderStatusType::Open);

        // 미체결 주문 확인
        let open_orders = exchange.get_open_orders(Some(&symbol)).await.unwrap();
        assert_eq!(open_orders.len(), 1);
    }

    #[tokio::test]
    async fn test_cancel_order() {
        let config = SimulatedConfig::default()
            .with_initial_balance("USDT", dec!(100000));

        let exchange = SimulatedExchange::new(config);
        let symbol = create_test_symbol();

        let klines = generate_sample_klines(symbol.clone(), Timeframe::M1, 10, dec!(50000), dec!(0.01));
        exchange.load_klines(symbol.clone(), Timeframe::M1, klines).await;
        exchange.step(&symbol, Timeframe::M1).await;

        // 지정가 주문 생성 및 취소
        let request = OrderRequest {
            symbol: symbol.clone(),
            side: Side::Buy,
            order_type: OrderType::Limit,
            quantity: dec!(0.1),
            price: Some(dec!(40000)),
            stop_price: None,
            time_in_force: TimeInForce::GTC,
            client_order_id: None,
            strategy_id: None,
        };

        let order_id = exchange.place_order(&request).await.unwrap();
        exchange.cancel_order(&symbol, &order_id).await.unwrap();

        let status = exchange.get_order(&symbol, &order_id).await.unwrap();
        assert_eq!(status.status, OrderStatusType::Cancelled);

        // 잔고가 복원되어야 함
        let balance = exchange.get_balance("USDT").await.unwrap();
        assert_eq!(balance.free, dec!(100000));
    }
}
