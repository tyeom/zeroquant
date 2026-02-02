//! 구간분할 전략 (Stock Gugan)
//!
//! 가격대를 여러 구간으로 나누어 구간 변동 시 매매하는 장기 투자 전략.
//!
//! # 전략 로직
//! - **구간 계산**: 지정 기간(기본 20일)의 최고가/최저가를 N등분(기본 15구간)
//! - **구간 상승**: 가격이 상위 구간으로 진입 시 매수 (MA20 필터)
//! - **구간 하락**: 가격이 하위 구간으로 진입 시 매도 (MA5 필터)
//! - **분할 매매**: 구간 변화량 × (투자금/분할수) 만큼 거래
//!
//! # 장점
//! - 박스권 및 상승장에서 효과적
//! - 단순하고 기계적인 규칙
//! - 감정 배제 가능
//!
//! # 권장 타임프레임
//! - 일봉 (1D)

use crate::strategies::common::deserialize_symbol;
use crate::Strategy;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;
use tracing::{debug, info};
use trader_core::{MarketData, MarketDataType, MarketType, Order, Position, Side, Signal, Symbol};

/// 구간분할 전략 설정.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StockGuganConfig {
    /// 거래 심볼 (예: "TSLA", "005930")
    #[serde(deserialize_with = "deserialize_symbol")]
    pub symbol: String,

    /// 구간 분할 수 (기본값: 15)
    #[serde(default = "default_div_num")]
    pub div_num: usize,

    /// 구간 계산 기간 (기본값: 20일)
    #[serde(default = "default_target_period")]
    pub target_period: usize,

    /// MA 필터 사용 (기본값: true)
    #[serde(default = "default_use_ma_filter")]
    pub use_ma_filter: bool,

    /// 매수 MA 기간 (기본값: 20)
    #[serde(default = "default_buy_ma_period")]
    pub buy_ma_period: usize,

    /// 매도 MA 기간 (기본값: 5)
    #[serde(default = "default_sell_ma_period")]
    pub sell_ma_period: usize,

    /// 최초 매수 시 기본 구간 수량 비율 (기본값: 1.0 = 1구간)
    #[serde(default = "default_initial_buy_ratio")]
    pub initial_buy_ratio: f64,

    /// 손절 비율 (기본값: 0.0 = 사용안함)
    #[serde(default)]
    pub stop_loss_pct: f64,
}

fn default_div_num() -> usize {
    15
}
fn default_target_period() -> usize {
    20
}
fn default_use_ma_filter() -> bool {
    true
}
fn default_buy_ma_period() -> usize {
    20
}
fn default_sell_ma_period() -> usize {
    5
}
fn default_initial_buy_ratio() -> f64 {
    1.0
}

impl Default for StockGuganConfig {
    fn default() -> Self {
        Self {
            symbol: "TSLA".to_string(),
            div_num: 15,
            target_period: 20,
            use_ma_filter: true,
            buy_ma_period: 20,
            sell_ma_period: 5,
            initial_buy_ratio: 1.0,
            stop_loss_pct: 0.0,
        }
    }
}

/// 일봉 데이터.
#[derive(Debug, Clone)]
struct DailyData {
    high: Decimal,
    low: Decimal,
    close: Decimal,
    timestamp: DateTime<Utc>,
}

/// 구간분할 전략.
pub struct StockGuganStrategy {
    config: Option<StockGuganConfig>,
    symbol: Option<Symbol>,

    /// 과거 일봉 데이터
    daily_history: VecDeque<DailyData>,

    /// 현재 일봉 데이터 (구축 중)
    current_day: Option<DailyData>,

    /// 현재 구간 (1 ~ div_num)
    current_zone: Option<usize>,

    /// 이전 구간
    prev_zone: Option<usize>,

    /// 구간 범위 정보
    zone_high: Option<Decimal>,
    zone_low: Option<Decimal>,
    zone_gap: Option<Decimal>,

    /// 전략 시작 여부
    started: bool,

    /// 보유 수량 (시뮬레이션용)
    holdings: Decimal,

    /// 통계
    trades_count: u32,
    total_buy_amount: Decimal,
    total_sell_amount: Decimal,

    initialized: bool,
}

impl StockGuganStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            symbol: None,
            daily_history: VecDeque::new(),
            current_day: None,
            current_zone: None,
            prev_zone: None,
            zone_high: None,
            zone_low: None,
            zone_gap: None,
            started: false,
            holdings: Decimal::ZERO,
            trades_count: 0,
            total_buy_amount: Decimal::ZERO,
            total_sell_amount: Decimal::ZERO,
            initialized: false,
        }
    }

    /// 새로운 날인지 확인.
    fn is_new_day(&self, current_time: DateTime<Utc>) -> bool {
        match &self.current_day {
            Some(day) => current_time.date_naive() != day.timestamp.date_naive(),
            None => true,
        }
    }

    /// 이동평균 계산.
    fn calculate_ma(&self, period: usize) -> Option<Decimal> {
        if self.daily_history.len() < period {
            return None;
        }

        let sum: Decimal = self
            .daily_history
            .iter()
            .take(period)
            .map(|d| d.close)
            .sum();

        Some(sum / Decimal::from(period))
    }

    /// 구간 계산: 지정 기간의 최고가/최저가 범위를 N등분.
    fn calculate_zones(&mut self) {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return,
        };

        if self.daily_history.len() < config.target_period {
            return;
        }

        // 지정 기간의 최고가/최저가 찾기
        let mut high_price = Decimal::MIN;
        let mut low_price = Decimal::MAX;

        for data in self.daily_history.iter().take(config.target_period) {
            if data.high > high_price {
                high_price = data.high;
            }
            if data.low < low_price {
                low_price = data.low;
            }
        }

        if high_price > low_price {
            self.zone_high = Some(high_price);
            self.zone_low = Some(low_price);
            self.zone_gap = Some((high_price - low_price) / Decimal::from(config.div_num));

            debug!(
                high = %high_price,
                low = %low_price,
                gap = ?self.zone_gap,
                "구간 계산 완료"
            );
        }
    }

    /// 현재 가격이 속한 구간 계산 (1 ~ div_num).
    fn get_current_zone(&self, current_price: Decimal) -> Option<usize> {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return None,
        };

        let (zone_low, zone_gap) = match (self.zone_low, self.zone_gap) {
            (Some(l), Some(g)) if g > Decimal::ZERO => (l, g),
            _ => return None,
        };

        // 구간 계산: 가격이 low + (gap * step) 미만이면 해당 step이 구간
        for step in 1..=config.div_num {
            if current_price < zone_low + zone_gap * Decimal::from(step) {
                return Some(step);
            }
        }

        Some(config.div_num)
    }

    /// 일봉 종료 시 처리.
    fn on_day_close(&mut self) {
        if let Some(day) = self.current_day.take() {
            let target_period = match self.config.as_ref() {
                Some(c) => c.target_period,
                None => return,
            };

            // 일봉 데이터 저장
            self.daily_history.push_front(day);
            while self.daily_history.len() > target_period + 10 {
                self.daily_history.pop_back();
            }

            // 구간 재계산
            self.calculate_zones();
        }
    }

    /// 신호 생성.
    fn generate_signals(
        &mut self,
        current_price: Decimal,
        _timestamp: DateTime<Utc>,
    ) -> Vec<Signal> {
        let config = match self.config.as_ref() {
            Some(c) => c.clone(),
            None => return Vec::new(),
        };
        let symbol = match self.symbol.as_ref() {
            Some(s) => s.clone(),
            None => return Vec::new(),
        };

        let mut signals = Vec::new();

        // 구간 정보 확인
        if self.zone_gap.is_none() {
            return signals;
        }

        // 현재 구간 계산
        let current_zone = match self.get_current_zone(current_price) {
            Some(z) => z,
            None => return signals,
        };

        // 첫 실행 시 구간 저장 및 초기 매수
        if !self.started {
            self.current_zone = Some(current_zone);
            self.prev_zone = Some(current_zone);
            self.started = true;

            // 첫 매수 신호
            let strength = config.initial_buy_ratio / config.div_num as f64;
            signals.push(
                Signal::entry("stock_gugan", symbol.clone(), Side::Buy)
                    .with_strength(strength)
                    .with_prices(Some(current_price), None, None)
                    .with_metadata("zone", json!(current_zone))
                    .with_metadata("action", json!("initial")),
            );

            info!(
                zone = current_zone,
                price = %current_price,
                "구간분할 전략 시작 - 초기 매수"
            );

            return signals;
        }

        // 구간 변경 확인
        let prev_zone = self.current_zone.unwrap_or(current_zone);
        if current_zone == prev_zone {
            return signals;
        }

        let zone_change = current_zone as i32 - prev_zone as i32;
        self.prev_zone = self.current_zone;
        self.current_zone = Some(current_zone);

        // MA 필터 계산
        let ma20 = self.calculate_ma(config.buy_ma_period);
        let ma5 = self.calculate_ma(config.sell_ma_period);
        let prev_close = self.daily_history.front().map(|d| d.close);

        if zone_change > 0 {
            // 구간 상승 → 매수
            // MA20 필터: 전일 종가가 MA20 위에 있어야 함
            let ma_condition = if config.use_ma_filter {
                match (prev_close, ma20) {
                    (Some(close), Some(ma)) => close > ma,
                    _ => true,
                }
            } else {
                true
            };

            if ma_condition {
                let strength = (zone_change.abs() as f64) / config.div_num as f64;
                signals.push(
                    Signal::entry("stock_gugan", symbol.clone(), Side::Buy)
                        .with_strength(strength)
                        .with_prices(Some(current_price), None, None)
                        .with_metadata("zone", json!(current_zone))
                        .with_metadata("prev_zone", json!(prev_zone))
                        .with_metadata("zone_change", json!(zone_change))
                        .with_metadata("action", json!("zone_up_buy")),
                );

                info!(
                    prev_zone = prev_zone,
                    current_zone = current_zone,
                    change = zone_change,
                    price = %current_price,
                    "구간 상승 → 매수"
                );
            } else {
                debug!(
                    prev_zone = prev_zone,
                    current_zone = current_zone,
                    "구간 상승하였으나 MA 필터 미충족"
                );
            }
        } else if zone_change < 0 {
            // 구간 하락 → 매도
            // MA5 필터: 전일 종가가 MA5 아래에 있어야 함
            let ma_condition = if config.use_ma_filter {
                match (prev_close, ma5) {
                    (Some(close), Some(ma)) => close < ma,
                    _ => true,
                }
            } else {
                true
            };

            if ma_condition && self.holdings > Decimal::ZERO {
                let strength = (zone_change.abs() as f64) / config.div_num as f64;
                signals.push(
                    Signal::exit("stock_gugan", symbol.clone(), Side::Sell)
                        .with_strength(strength)
                        .with_prices(Some(current_price), None, None)
                        .with_metadata("zone", json!(current_zone))
                        .with_metadata("prev_zone", json!(prev_zone))
                        .with_metadata("zone_change", json!(zone_change))
                        .with_metadata("action", json!("zone_down_sell")),
                );

                info!(
                    prev_zone = prev_zone,
                    current_zone = current_zone,
                    change = zone_change,
                    price = %current_price,
                    "구간 하락 → 매도"
                );
            } else {
                debug!(
                    prev_zone = prev_zone,
                    current_zone = current_zone,
                    "구간 하락하였으나 MA 필터 미충족 또는 보유량 없음"
                );
            }
        }

        signals
    }
}

impl Default for StockGuganStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for StockGuganStrategy {
    fn name(&self) -> &str {
        "Stock Gugan"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "구간분할 전략. 가격대를 여러 구간으로 나누어 구간 상승 시 매수, \
         구간 하락 시 매도. 장기 투자에 적합."
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let gugan_config: StockGuganConfig = serde_json::from_value(config)?;

        info!(
            symbol = %gugan_config.symbol,
            div_num = gugan_config.div_num,
            target_period = gugan_config.target_period,
            "구간분할 전략 초기화"
        );

        // 시장 타입 결정 (숫자로 시작하면 한국, 아니면 미국)
        let market_type = if gugan_config
            .symbol
            .chars()
            .next()
            .map(|c| c.is_numeric())
            .unwrap_or(false)
        {
            MarketType::KrStock
        } else {
            MarketType::UsStock
        };

        let quote = if market_type == MarketType::KrStock {
            "KRW"
        } else {
            "USD"
        };
        self.symbol = Some(Symbol::stock(&gugan_config.symbol, quote));
        self.config = Some(gugan_config);
        self.initialized = true;

        Ok(())
    }

    async fn on_market_data(
        &mut self,
        data: &MarketData,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
        if !self.initialized {
            return Ok(vec![]);
        }

        let symbol_str = match self.config.as_ref() {
            Some(c) => c.symbol.clone(),
            None => return Ok(vec![]),
        };

        if data.symbol.to_string() != symbol_str {
            return Ok(vec![]);
        }

        // kline에서 OHLCV 추출
        let (high, low, close, timestamp) = match &data.data {
            MarketDataType::Kline(kline) => (kline.high, kline.low, kline.close, kline.open_time),
            _ => return Ok(vec![]),
        };

        // 새 날짜 확인
        if self.is_new_day(timestamp) {
            self.on_day_close();

            // 새 일봉 시작
            self.current_day = Some(DailyData {
                high,
                low,
                close,
                timestamp,
            });
        } else {
            // 현재 일봉 업데이트
            if let Some(day) = &mut self.current_day {
                day.high = day.high.max(high);
                day.low = day.low.min(low);
                day.close = close;
            }
        }

        // 신호 생성
        let signals = self.generate_signals(close, timestamp);

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match order.side {
            Side::Buy => {
                self.holdings += order.quantity;
                self.total_buy_amount += order.quantity * order.price.unwrap_or(Decimal::ZERO);
            }
            Side::Sell => {
                self.holdings -= order.quantity;
                self.total_sell_amount += order.quantity * order.price.unwrap_or(Decimal::ZERO);
            }
        }
        self.trades_count += 1;

        debug!(
            side = ?order.side,
            quantity = %order.quantity,
            holdings = %self.holdings,
            "주문 체결"
        );
        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.holdings = position.quantity;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            trades = self.trades_count,
            total_buy = %self.total_buy_amount,
            total_sell = %self.total_sell_amount,
            final_holdings = %self.holdings,
            "구간분할 전략 종료"
        );

        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "initialized": self.initialized,
            "started": self.started,
            "current_zone": self.current_zone,
            "prev_zone": self.prev_zone,
            "zone_high": self.zone_high.map(|v| v.to_string()),
            "zone_low": self.zone_low.map(|v| v.to_string()),
            "zone_gap": self.zone_gap.map(|v| v.to_string()),
            "holdings": self.holdings.to_string(),
            "trades_count": self.trades_count,
            "daily_history_len": self.daily_history.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use rust_decimal_macros::dec;
    use trader_core::{Kline, Timeframe};

    fn create_daily_kline(
        symbol: &Symbol,
        high: Decimal,
        low: Decimal,
        close: Decimal,
        day: u32,
    ) -> MarketData {
        let timestamp = Utc.with_ymd_and_hms(2024, 1, day, 9, 0, 0).unwrap();
        let kline = Kline::new(
            symbol.clone(),
            Timeframe::D1,
            timestamp,
            close,
            high,
            low,
            close,
            dec!(1000000),
            timestamp,
        );
        MarketData::from_kline("test", kline)
    }

    #[tokio::test]
    async fn test_stock_gugan_initialization() {
        let mut strategy = StockGuganStrategy::new();

        let config = json!({
            "symbol": "TSLA/USD",
            "div_num": 15,
            "target_period": 20
        });

        strategy.initialize(config).await.unwrap();
        assert!(strategy.initialized);
        assert_eq!(strategy.config.as_ref().unwrap().div_num, 15);
    }

    #[tokio::test]
    async fn test_zone_calculation() {
        let mut strategy = StockGuganStrategy::new();

        let config = json!({
            "symbol": "TSLA/USD",
            "div_num": 10,
            "target_period": 5,
            "use_ma_filter": false
        });

        strategy.initialize(config).await.unwrap();

        let symbol = Symbol::stock("TSLA", "USD");

        // 6일치 데이터 추가 (마지막 일봉이 history에 저장되려면 다음 날 데이터 필요)
        // target_period=5 이므로 5일치가 history에 쌓여야 zone 계산됨
        for day in 1..=6 {
            let high = dec!(100) + Decimal::from(day * 2);
            let low = dec!(100) - Decimal::from(day * 2);
            let close = dec!(100);
            let data = create_daily_kline(&symbol, high, low, close, day);
            strategy.on_market_data(&data).await.unwrap();
        }

        // 구간 계산 확인
        assert!(strategy.zone_high.is_some());
        assert!(strategy.zone_low.is_some());
        assert!(strategy.zone_gap.is_some());
    }

    #[test]
    fn test_get_current_zone() {
        let mut strategy = StockGuganStrategy::new();
        strategy.config = Some(StockGuganConfig {
            div_num: 10,
            ..Default::default()
        });
        strategy.zone_low = Some(dec!(90));
        strategy.zone_high = Some(dec!(110));
        strategy.zone_gap = Some(dec!(2)); // (110-90)/10 = 2

        // 가격 91 → 구간 1 (90+2 미만)
        assert_eq!(strategy.get_current_zone(dec!(91)), Some(1));

        // 가격 95 → 구간 3 (90+6 미만)
        assert_eq!(strategy.get_current_zone(dec!(95)), Some(3));

        // 가격 109 → 구간 10
        assert_eq!(strategy.get_current_zone(dec!(109)), Some(10));
    }
}

// 전략 레지스트리에 자동 등록
use crate::register_strategy;

register_strategy! {
    id: "stock_gugan",
    aliases: ["gugan"],
    name: "주식 구간 매매",
    description: "가격 구간별 매매 전략입니다.",
    timeframe: "1m",
    symbols: [],
    category: Realtime,
    markets: [KrStock, UsStock],
    type: StockGuganStrategy
}
