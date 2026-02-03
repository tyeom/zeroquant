//! 섹터 변동성 돌파 전략 (Sector Volatility Breakout)
//!
//! 한국 섹터 ETF를 대상으로 하는 변동성 돌파 전략.
//! 섹터별로 가장 강한 모멘텀을 보이는 섹터를 선택하여 투자.
//!
//! # 전략 로직
//! - **섹터 순위**: 여러 섹터 ETF 중 전일 수익률이 가장 높은 섹터 선택
//! - **변동성 돌파**: 선택된 섹터에서 시가 + (전일 레인지 × K) 돌파 시 진입
//! - **청산**: 장 마감 전 자동 청산 (당일 매매)
//!
//! # 대상 섹터 ETF (한국)
//! - 091160: KODEX 반도체
//! - 091230: TIGER 반도체
//! - 305720: KODEX 2차전지산업
//! - 305540: TIGER 2차전지테마
//! - 091170: KODEX 은행
//! - 091220: TIGER 은행
//! - 102970: KODEX 철강
//! - 117460: KODEX 건설
//! - 091180: TIGER 자동차
//! - 102960: KODEX 기계장비
//!
//! # 권장 타임프레임
//! - 분봉 (5m, 15m) - 장중 돌파 감지

use std::sync::Arc;
use tokio::sync::RwLock;
use trader_core::domain::StrategyContext;
use crate::strategies::common::deserialize_symbols;
use crate::Strategy;
use async_trait::async_trait;
use chrono::{DateTime, Timelike, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{debug, info};
use trader_core::{MarketData, MarketDataType, Order, Position, Side, Signal, Symbol};

/// 섹터 변동성 돌파 전략 설정.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SectorVbConfig {
    /// 거래 대상 섹터 ETF 리스트
    #[serde(
        default = "default_sector_list",
        deserialize_with = "deserialize_symbols"
    )]
    pub symbols: Vec<String>,

    /// 돌파 K 계수 (기본값: 0.5)
    #[serde(default = "default_k_factor")]
    pub k_factor: f64,

    /// 섹터 선정 기준 (기본값: "returns" - 전일 수익률)
    #[serde(default = "default_selection_method")]
    pub selection_method: String,

    /// 선택할 상위 섹터 수 (기본값: 1)
    #[serde(default = "default_top_n")]
    pub top_n: usize,

    /// 최소 전일 거래량 (기본값: 100000)
    #[serde(default = "default_min_volume")]
    pub min_volume: u64,

    /// 장 마감 전 청산 시간 (분, 기본값: 10분 전)
    #[serde(default = "default_close_before_minutes")]
    pub close_before_minutes: u32,

    /// 손절 비율 (기본값: 2%)
    #[serde(default = "default_stop_loss_pct")]
    pub stop_loss_pct: f64,

    /// 익절 비율 (기본값: 3%)
    #[serde(default = "default_take_profit_pct")]
    pub take_profit_pct: f64,
}

fn default_sector_list() -> Vec<String> {
    vec![
        "091160".to_string(), // KODEX 반도체
        "091230".to_string(), // TIGER 반도체
        "305720".to_string(), // KODEX 2차전지산업
        "305540".to_string(), // TIGER 2차전지테마
        "091170".to_string(), // KODEX 은행
        "091220".to_string(), // TIGER 은행
        "102970".to_string(), // KODEX 철강
        "117460".to_string(), // KODEX 건설
        "091180".to_string(), // TIGER 자동차
        "102960".to_string(), // KODEX 기계장비
    ]
}

fn default_k_factor() -> f64 {
    0.5
}
fn default_selection_method() -> String {
    "returns".to_string()
}
fn default_top_n() -> usize {
    1
}
fn default_min_volume() -> u64 {
    100_000
}
fn default_close_before_minutes() -> u32 {
    10
}
fn default_stop_loss_pct() -> f64 {
    2.0
}
fn default_take_profit_pct() -> f64 {
    3.0
}

impl Default for SectorVbConfig {
    fn default() -> Self {
        Self {
            symbols: default_sector_list(),
            k_factor: 0.5,
            selection_method: "returns".to_string(),
            top_n: 1,
            min_volume: 100_000,
            close_before_minutes: 10,
            stop_loss_pct: 2.0,
            take_profit_pct: 3.0,
        }
    }
}

/// 섹터 데이터.
#[derive(Debug, Clone)]
struct SectorData {
    symbol: String,
    prev_close: Decimal,
    prev_high: Decimal,
    prev_low: Decimal,
    prev_volume: Decimal,
    today_open: Option<Decimal>,
    today_high: Decimal,
    today_low: Decimal,
    current_price: Decimal,
    target_price: Option<Decimal>,
    returns: Decimal, // 전일 수익률
}

impl SectorData {
    fn new(symbol: String) -> Self {
        Self {
            symbol,
            prev_close: Decimal::ZERO,
            prev_high: Decimal::ZERO,
            prev_low: Decimal::ZERO,
            prev_volume: Decimal::ZERO,
            today_open: None,
            today_high: Decimal::ZERO,
            today_low: Decimal::MAX,
            current_price: Decimal::ZERO,
            target_price: None,
            returns: Decimal::ZERO,
        }
    }
}

/// 포지션 상태.
#[derive(Debug, Clone)]
struct PositionState {
    symbol: String,
    entry_price: Decimal,
    stop_loss: Decimal,
    take_profit: Decimal,
    #[allow(dead_code)]
    entry_time: DateTime<Utc>,
}

/// 전략 상태.
#[derive(Debug, Clone, PartialEq)]
enum StrategyState {
    Rest,      // 대기 (조건 불만족)
    Ready,     // 돌파 체크 준비
    Investing, // 투자 중
}

/// 섹터 변동성 돌파 전략.
pub struct SectorVbStrategy {
    config: Option<SectorVbConfig>,
    symbols: Vec<Symbol>,
    context: Option<Arc<RwLock<StrategyContext>>>,

    /// 섹터별 데이터
    sector_data: HashMap<String, SectorData>,

    /// 선택된 섹터 (오늘 투자 대상)
    selected_sector: Option<String>,

    /// 전략 상태
    state: StrategyState,

    /// 현재 포지션
    position: Option<PositionState>,

    /// 오늘 날짜 (거래일 관리)
    today_date: Option<chrono::NaiveDate>,

    /// 통계
    trades_count: u32,
    wins: u32,
    total_pnl: Decimal,

    initialized: bool,
}

impl SectorVbStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            symbols: Vec::new(),
            context: None,
            sector_data: HashMap::new(),
            selected_sector: None,
            state: StrategyState::Rest,
            position: None,
            today_date: None,
            trades_count: 0,
            wins: 0,
            total_pnl: Decimal::ZERO,
            initialized: false,
        }
    }

    /// 새로운 거래일인지 확인.
    fn is_new_trading_day(&self, current_time: DateTime<Utc>) -> bool {
        match self.today_date {
            Some(date) => current_time.date_naive() != date,
            None => true,
        }
    }

    /// 새 거래일 시작 처리.
    fn on_new_day(&mut self, current_time: DateTime<Utc>) {
        self.today_date = Some(current_time.date_naive());
        self.state = StrategyState::Rest;
        self.selected_sector = None;

        // 전일 데이터 초기화 (today → prev)
        for data in self.sector_data.values_mut() {
            if data.today_open.is_some() {
                // 전일 데이터로 이동
                data.prev_close = data.current_price;
                data.prev_high = data.today_high;
                data.prev_low = data.today_low;
            }
            // 오늘 데이터 리셋
            data.today_open = None;
            data.today_high = Decimal::ZERO;
            data.today_low = Decimal::MAX;
            data.target_price = None;
        }

        info!(date = %current_time.date_naive(), "새 거래일 시작");
    }

    /// 섹터 순위 계산 및 선택.
    fn select_top_sectors(&mut self) {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return,
        };

        // 수익률 기준 정렬
        let mut ranked: Vec<_> = self
            .sector_data
            .values()
            .filter(|d| {
                d.prev_close > Decimal::ZERO && d.prev_volume >= Decimal::from(config.min_volume)
            })
            .collect();

        ranked.sort_by(|a, b| b.returns.cmp(&a.returns));

        if let Some(top) = ranked.first() {
            self.selected_sector = Some(top.symbol.clone());
            self.state = StrategyState::Ready;

            info!(
                sector = %top.symbol,
                returns = %top.returns,
                "상위 섹터 선택"
            );
        }
    }

    /// 돌파 목표가 계산.
    fn calculate_target_price(&mut self, symbol: &str) {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return,
        };

        if let Some(data) = self.sector_data.get_mut(symbol) {
            if let Some(today_open) = data.today_open {
                let prev_range = data.prev_high - data.prev_low;
                let k = Decimal::from_f64_retain(config.k_factor).unwrap_or(dec!(0.5));
                data.target_price = Some(today_open + prev_range * k);

                debug!(
                    symbol = %symbol,
                    open = %today_open,
                    range = %prev_range,
                    target = ?data.target_price,
                    "돌파 목표가 계산"
                );
            }
        }
    }

    /// 신호 생성.
    fn generate_signals(
        &mut self,
        symbol: &str,
        current_price: Decimal,
        timestamp: DateTime<Utc>,
    ) -> Vec<Signal> {
        let config = match self.config.as_ref() {
            Some(c) => c.clone(),
            None => return Vec::new(),
        };

        let mut signals = Vec::new();

        // 선택된 섹터가 아니면 무시
        if self.selected_sector.as_ref() != Some(&symbol.to_string()) {
            return signals;
        }

        // 포지션 있을 때: 손절/익절/청산 확인
        if let Some(pos) = &self.position {
            if pos.symbol != symbol {
                return signals;
            }

            // 손절
            if current_price <= pos.stop_loss {
                let sym = self.symbols.iter().find(|s| s.base == symbol).cloned();
                if let Some(sym) = sym {
                    signals.push(
                        Signal::exit("sector_vb", sym, Side::Sell)
                            .with_strength(1.0)
                            .with_prices(Some(current_price), None, None)
                            .with_metadata("exit_reason", json!("stop_loss")),
                    );

                    let pnl = current_price - pos.entry_price;
                    self.total_pnl += pnl;
                    self.trades_count += 1;
                    self.position = None;
                    self.state = StrategyState::Rest;

                    info!(price = %current_price, pnl = %pnl, "손절 청산");
                }
                return signals;
            }

            // 익절
            if current_price >= pos.take_profit {
                let sym = self.symbols.iter().find(|s| s.base == symbol).cloned();
                if let Some(sym) = sym {
                    signals.push(
                        Signal::exit("sector_vb", sym, Side::Sell)
                            .with_strength(1.0)
                            .with_prices(Some(current_price), None, None)
                            .with_metadata("exit_reason", json!("take_profit")),
                    );

                    let pnl = current_price - pos.entry_price;
                    self.total_pnl += pnl;
                    self.trades_count += 1;
                    self.wins += 1;
                    self.position = None;
                    self.state = StrategyState::Rest;

                    info!(price = %current_price, pnl = %pnl, "익절 청산");
                }
                return signals;
            }

            // 장 마감 전 청산 (한국장 15:30 - close_before_minutes 기준)
            // UTC 기준 한국 15:20 = UTC 06:20
            let close_hour = 6;
            let close_minute = 30 - config.close_before_minutes as i32;
            if timestamp.hour() == close_hour as u32 && timestamp.minute() >= close_minute as u32 {
                let sym = self.symbols.iter().find(|s| s.base == symbol).cloned();
                if let Some(sym) = sym {
                    signals.push(
                        Signal::exit("sector_vb", sym, Side::Sell)
                            .with_strength(1.0)
                            .with_prices(Some(current_price), None, None)
                            .with_metadata("exit_reason", json!("market_close")),
                    );

                    let pnl = current_price - pos.entry_price;
                    self.total_pnl += pnl;
                    self.trades_count += 1;
                    if pnl > Decimal::ZERO {
                        self.wins += 1;
                    }
                    self.position = None;
                    self.state = StrategyState::Rest;

                    info!(price = %current_price, pnl = %pnl, "장마감 전 청산");
                }
                return signals;
            }

            return signals;
        }

        // 포지션 없을 때: 돌파 확인
        if self.state != StrategyState::Ready {
            return signals;
        }

        if let Some(data) = self.sector_data.get(symbol) {
            if let Some(target) = data.target_price {
                if current_price >= target {
                    // 돌파! 매수
                    let sym = self.symbols.iter().find(|s| s.base == symbol).cloned();
                    if let Some(sym) = sym {
                        let stop_loss = current_price
                            * (dec!(1)
                                - Decimal::from_f64_retain(config.stop_loss_pct / 100.0).unwrap());
                        let take_profit = current_price
                            * (dec!(1)
                                + Decimal::from_f64_retain(config.take_profit_pct / 100.0)
                                    .unwrap());

                        signals.push(
                            Signal::entry("sector_vb", sym, Side::Buy)
                                .with_strength(0.5)
                                .with_prices(
                                    Some(current_price),
                                    Some(stop_loss),
                                    Some(take_profit),
                                )
                                .with_metadata("target_price", json!(target.to_string()))
                                .with_metadata("sector", json!(symbol)),
                        );

                        self.position = Some(PositionState {
                            symbol: symbol.to_string(),
                            entry_price: current_price,
                            stop_loss,
                            take_profit,
                            entry_time: timestamp,
                        });
                        self.state = StrategyState::Investing;

                        info!(
                            sector = %symbol,
                            target = %target,
                            entry = %current_price,
                            "돌파 진입"
                        );
                    }
                }
            }
        }

        signals
    }
}

impl Default for SectorVbStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for SectorVbStrategy {
    fn name(&self) -> &str {
        "Sector VB"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "섹터 변동성 돌파 전략. 한국 섹터 ETF 중 전일 수익률이 가장 높은 섹터를 \
         선택하여 변동성 돌파 시 진입. 당일 청산."
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let vb_config: SectorVbConfig = serde_json::from_value(config)?;

        info!(
            symbols = ?vb_config.symbols,
            k_factor = vb_config.k_factor,
            top_n = vb_config.top_n,
            "섹터 변동성 돌파 전략 초기화"
        );

        // 심볼 생성
        for symbol_str in &vb_config.symbols {
            let symbol = Symbol::stock(symbol_str, "KRW");
            self.symbols.push(symbol);
            self.sector_data
                .insert(symbol_str.clone(), SectorData::new(symbol_str.clone()));
        }

        self.config = Some(vb_config);
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

        // base 심볼만 추출 (XLK/USD -> XLK)
        let symbol_str = data.symbol.base.clone();

        // 등록된 섹터인지 확인
        if !self.sector_data.contains_key(&symbol_str) {
            return Ok(vec![]);
        }

        // kline에서 OHLCV 추출
        let (open, high, low, close, volume, timestamp) = match &data.data {
            MarketDataType::Kline(kline) => (
                kline.open,
                kline.high,
                kline.low,
                kline.close,
                kline.volume,
                kline.open_time,
            ),
            _ => return Ok(vec![]),
        };

        // 새 거래일 확인
        if self.is_new_trading_day(timestamp) {
            self.on_new_day(timestamp);
        }

        // 섹터 데이터 업데이트
        let need_calc_target = if let Some(sector) = self.sector_data.get_mut(&symbol_str) {
            sector.current_price = close;

            // 오늘 첫 데이터인지 확인하고 처리
            let calc_target = if sector.today_open.is_none() {
                sector.today_open = Some(open);
                sector.today_high = high;
                sector.today_low = low;

                // 전일 수익률 계산
                if sector.prev_close > Decimal::ZERO {
                    sector.returns = (open - sector.prev_close) / sector.prev_close * dec!(100);
                }

                true // 돌파 목표가 계산 필요
            } else {
                // 고저 업데이트
                sector.today_high = sector.today_high.max(high);
                sector.today_low = sector.today_low.min(low);
                false
            };

            sector.prev_volume = volume;
            calc_target
        } else {
            false
        };

        // mutable borrow가 끝난 후 돌파 목표가 계산
        if need_calc_target {
            self.calculate_target_price(&symbol_str);
        }

        // 섹터 선택 (아직 안 됐으면)
        if self.state == StrategyState::Rest && self.selected_sector.is_none() {
            // 모든 섹터의 오늘 시가가 있는지 확인
            let all_have_open = self.sector_data.values().all(|d| d.today_open.is_some());
            if all_have_open {
                self.select_top_sectors();
            }
        }

        // 신호 생성
        let signals = self.generate_signals(&symbol_str, close, timestamp);

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!(
            symbol = %order.symbol,
            side = ?order.side,
            quantity = %order.quantity,
            "주문 체결"
        );
        Ok(())
    }

    async fn on_position_update(
        &mut self,
        _position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let win_rate = if self.trades_count > 0 {
            (self.wins as f64 / self.trades_count as f64) * 100.0
        } else {
            0.0
        };

        info!(
            trades = self.trades_count,
            wins = self.wins,
            win_rate = %format!("{:.1}%", win_rate),
            total_pnl = %self.total_pnl,
            "섹터 변동성 돌파 전략 종료"
        );

        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "initialized": self.initialized,
            "state": format!("{:?}", self.state),
            "selected_sector": self.selected_sector,
            "has_position": self.position.is_some(),
            "sector_count": self.sector_data.len(),
            "trades_count": self.trades_count,
            "wins": self.wins,
            "total_pnl": self.total_pnl.to_string(),
        })
    }
    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("StrategyContext injected into SectorVb strategy");
    }


}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sector_vb_initialization() {
        let mut strategy = SectorVbStrategy::new();

        let config = json!({
            "symbols": ["091160", "091230", "305720"],
            "k_factor": 0.5,
            "top_n": 1
        });

        strategy.initialize(config).await.unwrap();
        assert!(strategy.initialized);
        assert_eq!(strategy.sector_data.len(), 3);
    }

    #[test]
    fn test_default_sector_list() {
        let sectors = default_sector_list();
        assert_eq!(sectors.len(), 10);
        assert!(sectors.contains(&"091160".to_string()));
    }
}

// 전략 레지스트리에 자동 등록
use crate::register_strategy;

register_strategy! {
    id: "sector_vb",
    aliases: ["sector_volatility"],
    name: "섹터 변동성 돌파",
    description: "섹터별 변동성 돌파 전략입니다.",
    timeframe: "1d",
    symbols: ["091160", "091170", "091180", "091220", "091230"],
    category: Daily,
    markets: [Stock],
    type: SectorVbStrategy
}
