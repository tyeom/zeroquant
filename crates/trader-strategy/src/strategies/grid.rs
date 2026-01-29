//! 그리드 트레이딩 전략.
//!
//! 기준 가격을 중심으로 일정 간격으로 매수/매도 주문을 배치하여
//! 시장 변동에서 수익을 얻는 전략입니다.
//!
//! # 그리드 유형
//! - 단순 그리드: 레벨 간 고정 간격
//! - 동적 그리드: ATR 기반 간격 조정
//! - 추세 필터 그리드: 추세 방향에 따라 활성화

use crate::strategies::common::deserialize_symbol;
use crate::Strategy;
use async_trait::async_trait;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;
use trader_core::{MarketData, MarketDataType, MarketType, Order, Position, Side, Signal, SignalType, Symbol};
use tracing::{debug, info};

/// 그리드 트레이딩 전략 설정.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GridConfig {
    /// 거래할 심볼 (예: "BTC/USDT")
    #[serde(deserialize_with = "deserialize_symbol")]
    pub symbol: String,

    /// 그리드 기준 가격 (None이면 현재 가격 사용)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub center_price: Option<Decimal>,

    /// 그리드 간격 비율 (예: 1.0 = 1%)
    #[serde(default = "default_spacing")]
    pub grid_spacing_pct: f64,

    /// 기준 가격 위아래 그리드 레벨 수
    #[serde(default = "default_levels")]
    pub grid_levels: usize,

    /// 각 레벨당 거래 금액 (호가 통화 기준)
    pub amount_per_level: Decimal,

    /// 가격 상한 (선택사항)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upper_limit: Option<Decimal>,

    /// 가격 하한 (선택사항)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lower_limit: Option<Decimal>,

    /// ATR 기반 동적 그리드 간격 활성화
    #[serde(default)]
    pub dynamic_spacing: bool,

    /// 동적 간격을 위한 ATR 기간
    #[serde(default = "default_atr_period")]
    pub atr_period: usize,

    /// 동적 간격을 위한 ATR 승수
    #[serde(default = "default_atr_multiplier")]
    pub atr_multiplier: f64,

    /// 추세 필터 활성화 (추세 방향으로만 거래)
    #[serde(default)]
    pub trend_filter: bool,

    /// 추세 감지를 위한 이동평균 기간
    #[serde(default = "default_ma_period")]
    pub ma_period: usize,

    /// 그리드 재설정을 트리거하는 최소 가격 변동 (백분율)
    #[serde(default = "default_reset_threshold")]
    pub reset_threshold_pct: f64,

    /// 최대 포지션 크기 (호가 통화 기준)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_position_size: Option<Decimal>,
}

fn default_spacing() -> f64 {
    1.0
}
fn default_levels() -> usize {
    10
}
fn default_atr_period() -> usize {
    14
}
fn default_atr_multiplier() -> f64 {
    1.0
}
fn default_ma_period() -> usize {
    20
}
fn default_reset_threshold() -> f64 {
    5.0
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            symbol: "BTC/USDT".to_string(),
            center_price: None,
            grid_spacing_pct: 1.0,
            grid_levels: 10,
            amount_per_level: dec!(100),
            upper_limit: None,
            lower_limit: None,
            dynamic_spacing: false,
            atr_period: 14,
            atr_multiplier: 1.0,
            trend_filter: false,
            ma_period: 20,
            reset_threshold_pct: 5.0,
            max_position_size: None,
        }
    }
}

/// 그리드 레벨 상태.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GridLevel {
    /// 이 레벨의 가격
    price: Decimal,
    /// 매수 또는 매도 레벨 여부
    side: Side,
    /// 이 레벨이 실행되었는지 여부
    executed: bool,
    /// 주문이 배치된 경우 주문 ID
    order_id: Option<String>,
    /// 레벨 인덱스 (참조용)
    index: i32,
}

/// 추세 방향.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrendDirection {
    Up,
    Down,
    Sideways,
}

/// 그리드 트레이딩 전략.
pub struct GridStrategy {
    /// 전략 설정
    config: Option<GridConfig>,

    /// 그리드 레벨
    grid_levels: Vec<GridLevel>,

    /// 현재 가격
    current_price: Option<Decimal>,

    /// 기준 가격 (그리드 기준점)
    center_price: Option<Decimal>,

    /// 거래 중인 심볼
    symbol: Option<Symbol>,

    /// ATR을 위한 고가/저가 히스토리
    high_history: VecDeque<Decimal>,
    low_history: VecDeque<Decimal>,
    close_history: VecDeque<Decimal>,

    /// 현재 ATR 값
    current_atr: Option<Decimal>,

    /// 현재 추세 방향
    trend: TrendDirection,

    /// 현재 포지션 크기
    position_size: Decimal,

    /// 총 실행된 거래 수
    trades_count: u32,

    /// 총 손익
    total_pnl: Decimal,

    /// 초기화 플래그
    initialized: bool,
}

impl GridStrategy {
    /// 새 그리드 트레이딩 전략 생성.
    pub fn new() -> Self {
        Self {
            config: None,
            grid_levels: Vec::new(),
            current_price: None,
            center_price: None,
            symbol: None,
            high_history: VecDeque::new(),
            low_history: VecDeque::new(),
            close_history: VecDeque::new(),
            current_atr: None,
            trend: TrendDirection::Sideways,
            position_size: Decimal::ZERO,
            trades_count: 0,
            total_pnl: Decimal::ZERO,
            initialized: false,
        }
    }

    /// 현재 가격을 기준으로 그리드 레벨 초기화.
    fn initialize_grid(&mut self, center: Decimal) {
        let config = self.config.as_ref().unwrap();
        self.grid_levels.clear();
        self.center_price = Some(center);

        // 간격 계산
        let spacing = if config.dynamic_spacing {
            self.calculate_dynamic_spacing(center)
        } else {
            center * Decimal::from_f64_retain(config.grid_spacing_pct / 100.0).unwrap_or(dec!(0.01))
        };

        // 매수 레벨 생성 (기준 아래)
        for i in 1..=config.grid_levels {
            let price = center - (spacing * Decimal::from(i as u32));

            // 하한 확인
            if let Some(lower) = config.lower_limit {
                if price < lower {
                    continue;
                }
            }

            // 가격이 양수인지 확인
            if price <= Decimal::ZERO {
                continue;
            }

            self.grid_levels.push(GridLevel {
                price,
                side: Side::Buy,
                executed: false,
                order_id: None,
                index: -(i as i32),
            });
        }

        // 매도 레벨 생성 (기준 위)
        for i in 1..=config.grid_levels {
            let price = center + (spacing * Decimal::from(i as u32));

            // 상한 확인
            if let Some(upper) = config.upper_limit {
                if price > upper {
                    continue;
                }
            }

            self.grid_levels.push(GridLevel {
                price,
                side: Side::Sell,
                executed: false,
                order_id: None,
                index: i as i32,
            });
        }

        // 가격순 정렬
        self.grid_levels.sort_by(|a, b| a.price.cmp(&b.price));

        info!(
            center = %center,
            spacing = %spacing,
            levels = self.grid_levels.len(),
            "Grid initialized"
        );
    }

    /// ATR 기반 동적 간격 계산.
    fn calculate_dynamic_spacing(&self, current_price: Decimal) -> Decimal {
        let config = self.config.as_ref().unwrap();

        if let Some(atr) = self.current_atr {
            let atr_multiplier =
                Decimal::from_f64_retain(config.atr_multiplier).unwrap_or(dec!(1.0));
            atr * atr_multiplier
        } else {
            // 백분율 기반 간격으로 폴백
            current_price * Decimal::from_f64_retain(config.grid_spacing_pct / 100.0).unwrap_or(dec!(0.01))
        }
    }

    /// ATR (Average True Range) 계산.
    fn calculate_atr(&mut self) {
        let config = self.config.as_ref().unwrap();
        let period = config.atr_period;

        if self.high_history.len() < period || self.low_history.len() < period {
            return;
        }

        let mut tr_sum = Decimal::ZERO;

        // 각 기간에 대한 True Range 계산
        for i in 0..period {
            let high = self.high_history[i];
            let low = self.low_history[i];

            // True Range = 고가 - 저가 (간소화, 현재는 이전 종가 무시)
            let tr = high - low;
            tr_sum += tr;
        }

        self.current_atr = Some(tr_sum / Decimal::from(period as u32));
    }

    /// 단순 이동평균을 사용하여 추세 방향 계산.
    fn calculate_trend(&mut self) {
        let config = self.config.as_ref().unwrap();

        if self.close_history.len() < config.ma_period {
            self.trend = TrendDirection::Sideways;
            return;
        }

        // SMA 계산
        let sum: Decimal = self.close_history.iter().take(config.ma_period).sum();
        let sma = sum / Decimal::from(config.ma_period as u32);

        if let Some(current) = self.current_price {
            let threshold = sma * dec!(0.002); // 0.2% 임계값

            if current > sma + threshold {
                self.trend = TrendDirection::Up;
            } else if current < sma - threshold {
                self.trend = TrendDirection::Down;
            } else {
                self.trend = TrendDirection::Sideways;
            }
        }
    }

    /// 현재 가격을 기반으로 신호 생성.
    fn generate_grid_signals(&mut self, current_price: Decimal) -> Vec<Signal> {
        let config = self.config.as_ref().unwrap();
        let symbol = self.symbol.as_ref().unwrap().clone();
        let amount_per_level = config.amount_per_level;
        let max_position = config.max_position_size;
        let trend_filter = config.trend_filter;
        let current_trend = self.trend;
        let position_size = self.position_size;
        let mut signals = Vec::new();

        for level in &mut self.grid_levels {
            // 실행된 레벨 건너뛰기
            if level.executed {
                continue;
            }

            // 추세 필터 확인
            if trend_filter {
                let allowed = matches!(
                    (current_trend, level.side),
                    (TrendDirection::Up, Side::Buy)
                        | (TrendDirection::Down, Side::Sell)
                        | (TrendDirection::Sideways, _)
                );
                if !allowed {
                    continue;
                }
            }

            // 포지션 한도 확인
            if let Some(max_pos) = max_position {
                match level.side {
                    Side::Buy if position_size >= max_pos => continue,
                    Side::Sell if position_size <= Decimal::ZERO => continue,
                    _ => {}
                }
            }

            let triggered = match level.side {
                // 매수: 가격이 레벨 이하로 하락할 때 트리거
                Side::Buy => current_price <= level.price,
                // 매도: 가격이 레벨 이상으로 상승할 때 트리거
                Side::Sell => current_price >= level.price,
            };

            if triggered {
                let signal_type = match level.side {
                    Side::Buy => SignalType::Entry,
                    Side::Sell => SignalType::Exit,
                };

                let signal = Signal::new("grid_trading", symbol.clone(), level.side, signal_type)
                    .with_strength(1.0)
                    .with_metadata("grid_level", json!(level.index))
                    .with_metadata("grid_price", json!(level.price.to_string()))
                    .with_metadata("amount", json!(amount_per_level.to_string()));

                signals.push(signal);
                level.executed = true;

                info!(
                    side = ?level.side,
                    level = level.index,
                    price = %level.price,
                    "Grid level triggered"
                );
            }
        }

        signals
    }

    /// 가격이 멀어질 때 실행된 레벨 재설정.
    fn reset_executed_levels(&mut self, current_price: Decimal) {
        let config = self.config.as_ref().unwrap();
        let reset_pct = Decimal::from_f64_retain(config.reset_threshold_pct / 100.0).unwrap_or(dec!(0.05));

        for level in &mut self.grid_levels {
            if !level.executed {
                continue;
            }

            // 가격이 레벨에서 상당히 멀어지면 재설정
            match level.side {
                // 매수 레벨: 가격이 다시 올라가면 재설정
                Side::Buy if current_price > level.price * (dec!(1) + reset_pct / dec!(2)) => {
                    level.executed = false;
                    debug!(level = level.index, "Reset buy level");
                }
                // 매도 레벨: 가격이 다시 내려가면 재설정
                Side::Sell if current_price < level.price * (dec!(1) - reset_pct / dec!(2)) => {
                    level.executed = false;
                    debug!(level = level.index, "Reset sell level");
                }
                _ => {}
            }
        }
    }

    /// 지표를 위한 가격 히스토리 업데이트.
    fn update_price_history(&mut self, high: Decimal, low: Decimal, close: Decimal) {
        let config = self.config.as_ref().unwrap();
        let max_len = config.atr_period.max(config.ma_period) + 1;

        self.high_history.push_front(high);
        self.low_history.push_front(low);
        self.close_history.push_front(close);

        // 최대 길이로 자르기
        while self.high_history.len() > max_len {
            self.high_history.pop_back();
        }
        while self.low_history.len() > max_len {
            self.low_history.pop_back();
        }
        while self.close_history.len() > max_len {
            self.close_history.pop_back();
        }

        // 지표 재계산
        self.calculate_atr();
        self.calculate_trend();
    }

    /// 그리드 재초기화 필요 여부 확인 (기준 가격이 너무 멀어진 경우).
    fn should_reinitialize_grid(&self) -> bool {
        let config = self.config.as_ref().unwrap();

        if let (Some(center), Some(current)) = (self.center_price, self.current_price) {
            let deviation = ((current - center) / center).abs();
            let threshold = Decimal::from_f64_retain(config.reset_threshold_pct / 100.0).unwrap_or(dec!(0.05));
            deviation > threshold
        } else {
            false
        }
    }

    /// 그리드 통계 조회.
    fn get_grid_stats(&self) -> GridStats {
        let executed_buys = self
            .grid_levels
            .iter()
            .filter(|l| l.side == Side::Buy && l.executed)
            .count();
        let executed_sells = self
            .grid_levels
            .iter()
            .filter(|l| l.side == Side::Sell && l.executed)
            .count();
        let pending_buys = self
            .grid_levels
            .iter()
            .filter(|l| l.side == Side::Buy && !l.executed)
            .count();
        let pending_sells = self
            .grid_levels
            .iter()
            .filter(|l| l.side == Side::Sell && !l.executed)
            .count();

        GridStats {
            total_levels: self.grid_levels.len(),
            executed_buys,
            executed_sells,
            pending_buys,
            pending_sells,
            center_price: self.center_price,
            current_price: self.current_price,
            current_atr: self.current_atr,
            trend: format!("{:?}", self.trend),
        }
    }
}

impl Default for GridStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for GridStrategy {
    fn name(&self) -> &str {
        "Grid Trading"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "Grid trading strategy that places buy/sell orders at regular intervals around a center price"
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let grid_config: GridConfig = serde_json::from_value(config)?;

        info!(
            symbol = %grid_config.symbol,
            levels = grid_config.grid_levels,
            spacing = grid_config.grid_spacing_pct,
            dynamic = grid_config.dynamic_spacing,
            trend_filter = grid_config.trend_filter,
            "Initializing Grid Trading strategy"
        );

        self.symbol = Symbol::from_string(&grid_config.symbol, MarketType::Crypto);
        self.config = Some(grid_config);
        self.initialized = true;

        // 그리드는 첫 번째 가격을 수신할 때 완전히 초기화됨

        Ok(())
    }

    async fn on_market_data(
        &mut self,
        data: &MarketData,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
        if !self.initialized {
            return Ok(vec![]);
        }

        // self를 가변 참조하기 전에 필요한 설정값 복사
        let (symbol_str, center_price_opt) = {
            let config = self.config.as_ref().unwrap();
            (config.symbol.clone(), config.center_price)
        };

        // 해당 심볼인지 확인
        if data.symbol.to_string() != symbol_str {
            return Ok(vec![]);
        }

        // 시장 데이터에서 가격 추출
        let current_price = match &data.data {
            MarketDataType::Kline(kline) => kline.close,
            MarketDataType::Ticker(ticker) => ticker.last,
            MarketDataType::Trade(trade) => trade.price,
            _ => return Ok(vec![]),
        };

        self.current_price = Some(current_price);

        // 캔들 데이터로 지표 업데이트
        if let MarketDataType::Kline(kline) = &data.data {
            self.update_price_history(kline.high, kline.low, kline.close);
        }

        // 아직 완료되지 않은 경우 그리드 초기화
        if self.grid_levels.is_empty() {
            let center = center_price_opt.unwrap_or(current_price);
            self.initialize_grid(center);
        }

        // 그리드 재초기화 필요 여부 확인
        if self.should_reinitialize_grid() {
            info!(
                old_center = ?self.center_price,
                new_price = %current_price,
                "Grid drifted too far, reinitializing"
            );
            self.initialize_grid(current_price);
        }

        // 가격 움직임에 따라 실행된 레벨 재설정
        self.reset_executed_levels(current_price);

        // 신호 생성
        let signals = self.generate_grid_signals(current_price);

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 체결 가격 조회 (평균 체결가 또는 지정가)
        let fill_price = order.average_fill_price
            .or(order.price)
            .unwrap_or(Decimal::ZERO);

        // 포지션 크기 업데이트
        let order_value = order.quantity * fill_price;
        match order.side {
            Side::Buy => {
                self.position_size += order_value;
                self.trades_count += 1;
            }
            Side::Sell => {
                self.position_size -= order_value;
                self.trades_count += 1;
                // 간단한 손익 추적 (실제 손익은 다른 곳에서 계산해야 함)
            }
        }

        info!(
            side = ?order.side,
            quantity = %order.quantity,
            price = %fill_price,
            position_size = %self.position_size,
            "Grid order filled"
        );

        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 포지션 추적 업데이트
        self.total_pnl = position.realized_pnl;

        debug!(
            quantity = %position.quantity,
            pnl = %position.realized_pnl,
            "Position updated"
        );

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            trades = self.trades_count,
            pnl = %self.total_pnl,
            "Grid Trading strategy shutdown"
        );

        self.grid_levels.clear();
        self.initialized = false;

        Ok(())
    }

    fn get_state(&self) -> Value {
        let stats = self.get_grid_stats();

        json!({
            "initialized": self.initialized,
            "symbol": self.config.as_ref().map(|c| &c.symbol),
            "stats": stats,
            "trades_count": self.trades_count,
            "position_size": self.position_size.to_string(),
            "total_pnl": self.total_pnl.to_string(),
        })
    }

    fn save_state(&self) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let state = GridState {
            grid_levels: self.grid_levels.clone(),
            center_price: self.center_price,
            position_size: self.position_size,
            trades_count: self.trades_count,
            total_pnl: self.total_pnl,
        };

        Ok(serde_json::to_vec(&state)?)
    }

    fn load_state(&mut self, data: &[u8]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let state: GridState = serde_json::from_slice(data)?;

        self.grid_levels = state.grid_levels;
        self.center_price = state.center_price;
        self.position_size = state.position_size;
        self.trades_count = state.trades_count;
        self.total_pnl = state.total_pnl;

        Ok(())
    }
}

/// 모니터링을 위한 그리드 통계.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridStats {
    pub total_levels: usize,
    pub executed_buys: usize,
    pub executed_sells: usize,
    pub pending_buys: usize,
    pub pending_sells: usize,
    pub center_price: Option<Decimal>,
    pub current_price: Option<Decimal>,
    pub current_atr: Option<Decimal>,
    pub trend: String,
}

/// 영속성을 위한 직렬화 가능한 그리드 상태.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GridState {
    grid_levels: Vec<GridLevel>,
    center_price: Option<Decimal>,
    position_size: Decimal,
    trades_count: u32,
    total_pnl: Decimal,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use trader_core::Timeframe;

    fn create_kline(symbol: &Symbol, close: Decimal) -> MarketData {
        use trader_core::Kline;

        let kline = Kline::new(
            symbol.clone(),
            Timeframe::M1,
            Utc::now(),
            close,
            close + dec!(10),
            close - dec!(10),
            close,
            dec!(100),
            Utc::now(),
        );

        MarketData::from_kline("binance", kline)
    }

    #[tokio::test]
    async fn test_grid_initialization() {
        let mut strategy = GridStrategy::new();

        let config = json!({
            "symbol": "BTC/USDT",
            "center_price": "50000",
            "grid_spacing_pct": 1.0,
            "grid_levels": 5,
            "amount_per_level": "100"
        });

        strategy.initialize(config).await.unwrap();

        // Send initial price to trigger grid creation
        let symbol = Symbol::crypto("BTC", "USDT");
        let data = create_kline(&symbol, dec!(50000));

        let signals = strategy.on_market_data(&data).await.unwrap();

        // No signals on initialization
        assert!(signals.is_empty());

        // Check grid was created
        assert_eq!(strategy.grid_levels.len(), 10); // 5 buy + 5 sell
    }

    #[tokio::test]
    async fn test_grid_buy_signal() {
        let mut strategy = GridStrategy::new();

        let config = json!({
            "symbol": "BTC/USDT",
            "center_price": "50000",
            "grid_spacing_pct": 1.0,
            "grid_levels": 5,
            "amount_per_level": "100"
        });

        strategy.initialize(config).await.unwrap();

        let symbol = Symbol::crypto("BTC", "USDT");

        // Initialize with center price
        let data = create_kline(&symbol, dec!(50000));
        let _init_signals = strategy.on_market_data(&data).await.unwrap();

        // Verify grid is initialized
        assert!(!strategy.grid_levels.is_empty());

        // Check that buy levels exist below center
        let has_buy_levels = strategy.grid_levels.iter()
            .any(|l| l.side == Side::Buy);
        assert!(has_buy_levels);

        // Price drops well below first buy level to trigger
        let data = create_kline(&symbol, dec!(49000));
        let signals = strategy.on_market_data(&data).await.unwrap();

        // Check if any buy level was executed
        let any_buy_executed = strategy.grid_levels.iter()
            .filter(|l| l.side == Side::Buy)
            .any(|l| l.executed);

        // Should trigger buy signals or have executed buy levels
        assert!(!signals.is_empty() || any_buy_executed);
    }

    #[tokio::test]
    async fn test_grid_sell_signal() {
        let mut strategy = GridStrategy::new();

        let config = json!({
            "symbol": "BTC/USDT",
            "center_price": "50000",
            "grid_spacing_pct": 1.0,
            "grid_levels": 5,
            "amount_per_level": "100"
        });

        strategy.initialize(config).await.unwrap();

        let symbol = Symbol::crypto("BTC", "USDT");

        // Initialize with center price
        let data = create_kline(&symbol, dec!(50000));
        strategy.on_market_data(&data).await.unwrap();

        // Verify grid is initialized
        assert!(!strategy.grid_levels.is_empty());

        // Check that sell levels exist above center
        let has_sell_levels = strategy.grid_levels.iter()
            .any(|l| l.side == Side::Sell);
        assert!(has_sell_levels);

        // Price rises well above first sell level
        let data = create_kline(&symbol, dec!(51000));
        let signals = strategy.on_market_data(&data).await.unwrap();

        // Check if any sell level was executed
        let any_sell_executed = strategy.grid_levels.iter()
            .filter(|l| l.side == Side::Sell)
            .any(|l| l.executed);

        // Should trigger sell signals or have executed sell levels
        assert!(!signals.is_empty() || any_sell_executed);
    }

    #[tokio::test]
    async fn test_grid_limits() {
        let mut strategy = GridStrategy::new();

        let config = json!({
            "symbol": "BTC/USDT",
            "center_price": "50000",
            "grid_spacing_pct": 1.0,
            "grid_levels": 5,
            "amount_per_level": "100",
            "upper_limit": "51000",
            "lower_limit": "49000"
        });

        strategy.initialize(config).await.unwrap();

        let symbol = Symbol::crypto("BTC", "USDT");
        let data = create_kline(&symbol, dec!(50000));
        strategy.on_market_data(&data).await.unwrap();

        // Grid levels should be limited
        let state = strategy.get_state();
        let stats: GridStats =
            serde_json::from_value(state["stats"].clone()).unwrap();

        // Should have fewer levels due to limits
        assert!(stats.total_levels < 10);
    }

    #[test]
    fn test_grid_config_defaults() {
        let config = GridConfig::default();

        assert_eq!(config.grid_spacing_pct, 1.0);
        assert_eq!(config.grid_levels, 10);
        assert!(!config.dynamic_spacing);
        assert!(!config.trend_filter);
    }
}
