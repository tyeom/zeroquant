//! 동적 백테스트 (시뮬레이션) API 엔드포인트
//!
//! 정적 백테스트와 동일한 전략 로직을 실행하되, 시간 흐름에 따라 점진적으로 진행합니다.
//! speed 파라미터로 배속을 조절할 수 있습니다.
//!
//! # 핵심 원칙
//!
//! - 정적 백테스트와 동일한 전략 실행 (`on_market_data` 호출)
//! - 동일한 신호 처리 (`process_signal`)
//! - 동일한 설정에서 동일한 시점에 동일한 신호/거래 발생
//!
//! # 엔드포인트
//!
//! - `POST /api/v1/simulation/start` - 시뮬레이션 시작 (자동 전략 실행)
//! - `POST /api/v1/simulation/stop` - 시뮬레이션 중지
//! - `POST /api/v1/simulation/pause` - 일시정지/재개 토글
//! - `GET /api/v1/simulation/status` - 현재 상태 조회
//! - `GET /api/v1/simulation/positions` - 포지션 조회
//! - `GET /api/v1/simulation/trades` - 거래 내역 조회
//! - `GET /api/v1/simulation/equity` - 자산 곡선 조회
//! - `GET /api/v1/simulation/signals` - 신호 마커 조회

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, NaiveDate, NaiveTime, TimeZone, Utc};
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use trader_core::{
    unrealized_pnl, Kline, MarketData, Side, Signal, SignalMarker, SignalType, Timeframe,
};
use trader_data::storage::ohlcv::OhlcvCache;
use trader_strategy::{Strategy, StrategyRegistry};
use uuid::Uuid;

use crate::state::AppState;

// ==================== 시뮬레이션 상태 ====================

/// 시뮬레이션 실행 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationState {
    /// 중지됨
    Stopped,
    /// 실행 중
    Running,
    /// 일시 정지
    Paused,
}

/// 시뮬레이션 포지션
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationPosition {
    /// 심볼
    pub symbol: String,
    /// 표시 이름
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// 방향
    pub side: String,
    /// 수량
    pub quantity: Decimal,
    /// 평균 진입가
    pub entry_price: Decimal,
    /// 현재가
    pub current_price: Decimal,
    /// 미실현 손익
    pub unrealized_pnl: Decimal,
    /// 수익률 (%)
    pub return_pct: Decimal,
    /// 진입 시간
    pub entry_time: DateTime<Utc>,
}

/// 시뮬레이션 거래 내역
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationTrade {
    /// 거래 ID
    pub id: String,
    /// 심볼
    pub symbol: String,
    /// 표시 이름
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// 방향 (Buy/Sell)
    pub side: String,
    /// 수량
    pub quantity: Decimal,
    /// 체결가
    pub price: Decimal,
    /// 수수료
    pub commission: Decimal,
    /// 실현 손익 (청산 거래인 경우)
    pub realized_pnl: Option<Decimal>,
    /// 거래 시간 (시뮬레이션 시간)
    pub timestamp: DateTime<Utc>,
}

/// 자산 곡선 포인트
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityPoint {
    /// 시간 (시뮬레이션 시간)
    pub timestamp: DateTime<Utc>,
    /// 총 자산
    pub equity: Decimal,
    /// 낙폭 (%)
    pub drawdown_pct: Decimal,
}

/// 시뮬레이션 엔진 상태
pub struct SimulationEngine {
    // === 상태 ===
    /// 현재 상태
    pub state: SimulationState,
    /// 전략 ID
    pub strategy_id: Option<String>,
    /// 전략 인스턴스 (비동기 전략 실행용)
    strategy: Option<Box<dyn Strategy>>,

    // === 자산 ===
    /// 초기 잔고
    pub initial_balance: Decimal,
    /// 현재 잔고
    pub current_balance: Decimal,
    /// 포지션 목록
    pub positions: HashMap<String, SimulationPosition>,

    // === 거래 기록 ===
    /// 거래 내역
    pub trades: Vec<SimulationTrade>,
    /// 신호 마커
    pub signal_markers: Vec<SignalMarker>,
    /// 자산 곡선
    pub equity_curve: Vec<EquityPoint>,

    // === 진행 상황 ===
    /// 로드된 캔들 데이터
    klines: Vec<Kline>,
    /// 현재 캔들 인덱스
    current_kline_index: usize,
    /// 현재 시뮬레이션 시간
    current_simulation_time: Option<DateTime<Utc>>,

    // === 설정 ===
    /// 시뮬레이션 속도 (1.0 = 1초에 1캔들)
    pub speed: f64,
    /// 수수료율 (예: 0.001 = 0.1%)
    pub commission_rate: Decimal,
    /// 슬리피지율 (예: 0.0005 = 0.05%)
    pub slippage_rate: Decimal,

    // === 통계 ===
    /// 총 실현 손익
    pub total_realized_pnl: Decimal,
    /// 총 수수료
    pub total_commission: Decimal,
    /// 최고 자산 (낙폭 계산용)
    peak_equity: Decimal,

    // === 백그라운드 태스크 ===
    /// 실제 시작 시간
    pub started_at: Option<DateTime<Utc>>,
}

impl Default for SimulationEngine {
    fn default() -> Self {
        Self {
            state: SimulationState::Stopped,
            strategy_id: None,
            strategy: None,
            initial_balance: dec!(10_000_000),
            current_balance: dec!(10_000_000),
            positions: HashMap::new(),
            trades: Vec::new(),
            signal_markers: Vec::new(),
            equity_curve: Vec::new(),
            klines: Vec::new(),
            current_kline_index: 0,
            current_simulation_time: None,
            speed: 1.0,
            commission_rate: dec!(0.001), // 0.1%
            slippage_rate: dec!(0.0005),  // 0.05%
            total_realized_pnl: Decimal::ZERO,
            total_commission: Decimal::ZERO,
            peak_equity: dec!(10_000_000),
            started_at: None,
        }
    }
}

impl SimulationEngine {
    /// 새로운 시뮬레이션 엔진 생성
    pub fn new(initial_balance: Decimal) -> Self {
        Self {
            initial_balance,
            current_balance: initial_balance,
            peak_equity: initial_balance,
            ..Default::default()
        }
    }

    /// 시뮬레이션 초기화 (전략 + 데이터 로드)
    #[allow(clippy::too_many_arguments)]
    pub async fn initialize(
        &mut self,
        strategy_id: &str,
        parameters: Option<serde_json::Value>,
        symbols: &[String],
        start_date: &str,
        end_date: &str,
        initial_balance: Decimal,
        speed: f64,
        commission_rate: Decimal,
        slippage_rate: Decimal,
        pool: &sqlx::PgPool,
    ) -> Result<(), String> {
        // 1. 전략 메타 조회
        let meta = StrategyRegistry::find(strategy_id)
            .ok_or_else(|| format!("Unknown strategy: {}", strategy_id))?;

        // 2. 전략 인스턴스 생성
        let mut strategy = (meta.factory)();

        // 3. 전략 초기화 (파라미터 적용)
        if let Some(params) = parameters {
            strategy
                .initialize(params)
                .await
                .map_err(|e| format!("전략 초기화 실패: {}", e))?;
        }

        // 4. 캔들 데이터 로드
        let cache = OhlcvCache::new(pool.clone());
        let start = NaiveDate::parse_from_str(start_date, "%Y-%m-%d")
            .map_err(|e| format!("시작일 파싱 실패: {}", e))?;
        let end = NaiveDate::parse_from_str(end_date, "%Y-%m-%d")
            .map_err(|e| format!("종료일 파싱 실패: {}", e))?;

        // 기본 심볼 (단일 자산) 또는 전략 기본 심볼 사용
        let symbol = if symbols.is_empty() {
            meta.default_tickers
                .first()
                .ok_or_else(|| "심볼이 지정되지 않았습니다".to_string())?
                .to_string()
        } else {
            symbols[0].clone()
        };

        // 타임프레임 (전략 메타에서 문자열 → enum 변환)
        let timeframe_str = meta.default_timeframe;
        let timeframe = timeframe_str
            .parse::<Timeframe>()
            .map_err(|_| format!("잘못된 타임프레임: {}", timeframe_str))?;

        // NaiveDate → DateTime<Utc> 변환 (당일 00:00 UTC)
        let start_dt = Utc.from_utc_datetime(&start.and_time(NaiveTime::MIN));
        let end_dt =
            Utc.from_utc_datetime(&end.and_time(NaiveTime::from_hms_opt(23, 59, 59).unwrap()));

        let klines = cache
            .get_cached_klines_range(&symbol, timeframe, start_dt, end_dt)
            .await
            .map_err(|e| format!("캔들 데이터 로드 실패: {}", e))?;

        if klines.is_empty() {
            return Err(format!(
                "기간 {} ~ {}에 대한 {} {} 데이터가 없습니다",
                start_date, end_date, symbol, timeframe
            ));
        }

        // 5. 상태 초기화
        self.state = SimulationState::Running;
        self.strategy_id = Some(strategy_id.to_string());
        self.strategy = Some(strategy);
        self.initial_balance = initial_balance;
        self.current_balance = initial_balance;
        self.peak_equity = initial_balance;
        self.positions.clear();
        self.trades.clear();
        self.signal_markers.clear();
        self.equity_curve.clear();
        self.klines = klines;
        self.current_kline_index = 0;
        self.current_simulation_time = None;
        self.speed = speed;
        self.commission_rate = commission_rate;
        self.slippage_rate = slippage_rate;
        self.total_realized_pnl = Decimal::ZERO;
        self.total_commission = Decimal::ZERO;
        self.started_at = Some(Utc::now());

        Ok(())
    }

    /// 다음 캔들 처리 (백테스트 엔진과 동일한 로직)
    pub async fn process_next_candle(&mut self) -> Result<bool, String> {
        if self.state != SimulationState::Running {
            return Ok(false);
        }

        // 현재 캔들 가져오기
        let kline = match self.klines.get(self.current_kline_index) {
            Some(k) => k.clone(),
            None => return Ok(false), // 데이터 끝
        };

        // 현재 시뮬레이션 시간 업데이트
        self.current_simulation_time = Some(kline.close_time);

        // 전략에 데이터 전달 (백테스트와 동일)
        let strategy = self
            .strategy
            .as_mut()
            .ok_or_else(|| "전략이 초기화되지 않았습니다".to_string())?;

        let exchange_name = "simulation";
        let market_data = MarketData::from_kline(exchange_name, kline.clone());

        let signals = strategy
            .on_market_data(&market_data)
            .await
            .map_err(|e| format!("전략 실행 오류: {}", e))?;

        // 신호 처리 (백테스트와 동일한 로직)
        for signal in signals {
            self.process_signal(&signal, &kline)?;
        }

        // 포지션 가격 업데이트
        self.update_positions_price(&kline);

        // 자산 곡선 업데이트
        self.update_equity_curve(&kline);

        // 다음 캔들로 이동
        self.current_kline_index += 1;

        Ok(self.current_kline_index < self.klines.len())
    }

    /// 신호 처리 (백테스트 엔진과 동일한 로직)
    fn process_signal(&mut self, signal: &Signal, kline: &Kline) -> Result<(), String> {
        let price = signal.suggested_price.unwrap_or(kline.close);

        // SignalMarker 저장
        let marker = SignalMarker::from_signal(signal, price, kline.open_time, &signal.strategy_id);
        self.signal_markers.push(marker);

        match signal.signal_type {
            SignalType::Entry | SignalType::AddToPosition => {
                self.open_position(signal, kline)?;
            }
            SignalType::Exit | SignalType::ReducePosition => {
                self.close_position(signal, kline)?;
            }
            SignalType::Scale => {
                let key = signal.ticker.clone();
                if self.positions.contains_key(&key) {
                    self.close_position(signal, kline)?;
                } else {
                    self.open_position(signal, kline)?;
                }
            }
            SignalType::Alert => {
                // Alert는 실행하지 않고 마커만 기록
            }
        }

        Ok(())
    }

    /// 포지션 오픈 (백테스트와 동일)
    ///
    /// 포지션 크기는 signal.strength와 현재 잔고를 기반으로 계산합니다.
    /// (백테스트 엔진과 동일한 방식: max_position_size_pct = 20%)
    fn open_position(&mut self, signal: &Signal, kline: &Kline) -> Result<(), String> {
        let key = signal.ticker.clone();

        // 이미 포지션이 있으면 무시
        if self.positions.contains_key(&key) {
            return Ok(());
        }

        // 실행 가격 계산 (슬리피지 적용)
        let base_price = signal.suggested_price.unwrap_or(kline.close);
        let slippage = base_price * self.slippage_rate;
        let execution_price = match signal.side {
            Side::Buy => base_price + slippage,
            Side::Sell => base_price - slippage,
        };

        // 포지션 크기 계산 (백테스트와 동일: strength 기반)
        let max_position_size_pct = dec!(0.2); // 20%
        let max_amount = self.current_balance * max_position_size_pct;
        let position_amount =
            max_amount * Decimal::from_f64(signal.strength).unwrap_or(Decimal::ONE);
        let quantity = position_amount / execution_price;

        // 수수료 계산
        let trade_value = execution_price * quantity;
        let commission = trade_value * self.commission_rate;

        // 잔고 확인 (매수)
        if signal.side == Side::Buy {
            let total_cost = trade_value + commission;
            if total_cost > self.current_balance {
                return Ok(()); // 잔고 부족 - 무시
            }
            self.current_balance -= total_cost;
        }

        // 포지션 생성
        let side_str = match signal.side {
            Side::Buy => "Long",
            Side::Sell => "Short",
        };

        self.positions.insert(
            key.clone(),
            SimulationPosition {
                symbol: key.clone(),
                display_name: None,
                side: side_str.to_string(),
                quantity,
                entry_price: execution_price,
                current_price: execution_price,
                unrealized_pnl: Decimal::ZERO,
                return_pct: Decimal::ZERO,
                entry_time: kline.close_time,
            },
        );

        // 거래 기록
        let side_trade = match signal.side {
            Side::Buy => "Buy",
            Side::Sell => "Sell",
        };

        self.trades.push(SimulationTrade {
            id: Uuid::new_v4().to_string(),
            symbol: key,
            display_name: None,
            side: side_trade.to_string(),
            quantity,
            price: execution_price,
            commission,
            realized_pnl: None,
            timestamp: kline.close_time,
        });

        self.total_commission += commission;

        Ok(())
    }

    /// 포지션 청산 (백테스트와 동일)
    fn close_position(&mut self, signal: &Signal, kline: &Kline) -> Result<(), String> {
        let key = signal.ticker.clone();

        let pos = match self.positions.get(&key) {
            Some(p) => p.clone(),
            None => return Ok(()), // 포지션 없음 - 무시
        };

        // 실행 가격 계산 (슬리피지 적용)
        let base_price = signal.suggested_price.unwrap_or(kline.close);
        let slippage = base_price * self.slippage_rate;
        let execution_price = match signal.side {
            Side::Buy => base_price + slippage,  // 숏 청산 (매수)
            Side::Sell => base_price - slippage, // 롱 청산 (매도)
        };

        // 청산 수량 (전량 청산)
        let close_qty = pos.quantity;

        // 수수료 계산
        let trade_value = execution_price * close_qty;
        let commission = trade_value * self.commission_rate;

        // 실현 손익 계산
        let realized_pnl = if pos.side == "Long" {
            (execution_price - pos.entry_price) * close_qty - commission
        } else {
            (pos.entry_price - execution_price) * close_qty - commission
        };

        // 잔고 업데이트
        self.current_balance += trade_value - commission;
        self.total_realized_pnl += realized_pnl;
        self.total_commission += commission;

        // 포지션 제거
        self.positions.remove(&key);

        // 거래 기록
        let side_trade = match signal.side {
            Side::Buy => "Buy",
            Side::Sell => "Sell",
        };

        self.trades.push(SimulationTrade {
            id: Uuid::new_v4().to_string(),
            symbol: key,
            display_name: None,
            side: side_trade.to_string(),
            quantity: close_qty,
            price: execution_price,
            commission,
            realized_pnl: Some(realized_pnl),
            timestamp: kline.close_time,
        });

        Ok(())
    }

    /// 포지션 가격 업데이트
    fn update_positions_price(&mut self, kline: &Kline) {
        for pos in self.positions.values_mut() {
            if pos.symbol == kline.ticker {
                pos.current_price = kline.close;
                let side = if pos.side == "Long" {
                    Side::Buy
                } else {
                    Side::Sell
                };
                pos.unrealized_pnl =
                    unrealized_pnl(pos.entry_price, kline.close, pos.quantity, side);
                if pos.entry_price > Decimal::ZERO {
                    pos.return_pct =
                        pos.unrealized_pnl / (pos.entry_price * pos.quantity) * dec!(100);
                }
            }
        }
    }

    /// 자산 곡선 업데이트
    fn update_equity_curve(&mut self, kline: &Kline) {
        let equity = self.total_equity();

        // 최고점 갱신
        if equity > self.peak_equity {
            self.peak_equity = equity;
        }

        // 낙폭 계산
        let drawdown_pct = if self.peak_equity > Decimal::ZERO {
            (self.peak_equity - equity) / self.peak_equity * dec!(100)
        } else {
            Decimal::ZERO
        };

        self.equity_curve.push(EquityPoint {
            timestamp: kline.close_time,
            equity,
            drawdown_pct,
        });
    }

    /// 총 자산 계산
    pub fn total_equity(&self) -> Decimal {
        let positions_value: Decimal = self
            .positions
            .values()
            .map(|p| {
                if p.side == "Long" {
                    p.current_price * p.quantity
                } else {
                    // 숏 포지션: 진입가 - (현재가 - 진입가)
                    p.entry_price * p.quantity + (p.entry_price - p.current_price) * p.quantity
                }
            })
            .sum();
        self.current_balance + positions_value
    }

    /// 시뮬레이션 중지
    pub fn stop(&mut self) {
        self.state = SimulationState::Stopped;
    }

    /// 일시 정지
    pub fn pause(&mut self) {
        if self.state == SimulationState::Running {
            self.state = SimulationState::Paused;
        }
    }

    /// 재개
    pub fn resume(&mut self) {
        if self.state == SimulationState::Paused {
            self.state = SimulationState::Running;
        }
    }

    /// 진행률 (%)
    pub fn progress_pct(&self) -> f64 {
        if self.klines.is_empty() {
            return 0.0;
        }
        (self.current_kline_index as f64 / self.klines.len() as f64) * 100.0
    }
}

/// 공유 가능한 시뮬레이션 엔진 타입
pub type SharedSimulationEngine = Arc<RwLock<SimulationEngine>>;

/// 새로운 공유 시뮬레이션 엔진 생성
pub fn create_simulation_engine() -> SharedSimulationEngine {
    Arc::new(RwLock::new(SimulationEngine::default()))
}

// ==================== 요청/응답 타입 ====================

/// 시뮬레이션 시작 요청
#[derive(Debug, Deserialize)]
pub struct SimulationStartRequest {
    /// 전략 ID
    pub strategy_id: String,
    /// 전략 파라미터 (JSON)
    #[serde(default)]
    pub parameters: Option<serde_json::Value>,
    /// 심볼 목록 (비어있으면 전략 기본 심볼 사용)
    #[serde(default)]
    pub symbols: Vec<String>,
    /// 초기 잔고
    #[serde(default = "default_initial_balance")]
    pub initial_balance: Decimal,
    /// 시뮬레이션 속도 (1.0 = 1초에 1캔들)
    #[serde(default = "default_speed")]
    pub speed: f64,
    /// 시작 날짜 (YYYY-MM-DD)
    pub start_date: String,
    /// 종료 날짜 (YYYY-MM-DD)
    pub end_date: String,
    /// 수수료율 (기본 0.001 = 0.1%)
    #[serde(default = "default_commission_rate")]
    pub commission_rate: Decimal,
    /// 슬리피지율 (기본 0.0005 = 0.05%)
    #[serde(default = "default_slippage_rate")]
    pub slippage_rate: Decimal,
}

fn default_initial_balance() -> Decimal {
    dec!(10_000_000)
}

fn default_speed() -> f64 {
    1.0
}

fn default_commission_rate() -> Decimal {
    dec!(0.001)
}

fn default_slippage_rate() -> Decimal {
    dec!(0.0005)
}

/// 시뮬레이션 시작 응답
#[derive(Debug, Serialize)]
pub struct SimulationStartResponse {
    /// 성공 여부
    pub success: bool,
    /// 메시지
    pub message: String,
    /// 시작 시간
    pub started_at: DateTime<Utc>,
    /// 총 캔들 수
    pub total_candles: usize,
}

/// 시뮬레이션 중지 응답
#[derive(Debug, Serialize)]
pub struct SimulationStopResponse {
    /// 성공 여부
    pub success: bool,
    /// 메시지
    pub message: String,
    /// 최종 자산
    pub final_equity: Decimal,
    /// 총 수익률 (%)
    pub total_return_pct: Decimal,
    /// 총 거래 횟수
    pub total_trades: usize,
}

/// 시뮬레이션 상태 응답
#[derive(Debug, Serialize, Deserialize)]
pub struct SimulationStatusResponse {
    /// 현재 상태
    pub state: SimulationState,
    /// 전략 ID
    pub strategy_id: Option<String>,
    /// 초기 잔고
    pub initial_balance: Decimal,
    /// 현재 잔고
    pub current_balance: Decimal,
    /// 총 자산
    pub total_equity: Decimal,
    /// 미실현 손익
    pub unrealized_pnl: Decimal,
    /// 실현 손익
    pub realized_pnl: Decimal,
    /// 수익률 (%)
    pub return_pct: Decimal,
    /// 포지션 수
    pub position_count: usize,
    /// 거래 횟수
    pub trade_count: usize,
    /// 실제 시작 시간
    pub started_at: Option<DateTime<Utc>>,
    /// 시뮬레이션 속도
    pub speed: f64,
    /// 현재 시뮬레이션 시간
    pub current_simulation_time: Option<DateTime<Utc>>,
    /// 진행률 (%)
    pub progress_pct: f64,
    /// 현재 캔들 인덱스
    pub current_candle_index: usize,
    /// 총 캔들 수
    pub total_candles: usize,
}

/// 포지션 목록 응답
#[derive(Debug, Serialize)]
pub struct SimulationPositionsResponse {
    /// 포지션 목록
    pub positions: Vec<SimulationPosition>,
    /// 총 미실현 손익
    pub total_unrealized_pnl: Decimal,
}

/// 거래 내역 응답
#[derive(Debug, Serialize)]
pub struct SimulationTradesResponse {
    /// 거래 목록
    pub trades: Vec<SimulationTrade>,
    /// 총 거래 수
    pub total: usize,
    /// 총 실현 손익
    pub total_realized_pnl: Decimal,
    /// 총 수수료
    pub total_commission: Decimal,
}

/// 자산 곡선 응답
#[derive(Debug, Serialize)]
pub struct SimulationEquityResponse {
    /// 자산 곡선
    pub equity_curve: Vec<EquityPoint>,
    /// 최대 낙폭 (%)
    pub max_drawdown_pct: Decimal,
}

/// 신호 마커 응답
#[derive(Debug, Serialize)]
pub struct SimulationSignalsResponse {
    /// 신호 마커 목록
    pub signals: Vec<SignalMarker>,
    /// 총 신호 수
    pub total: usize,
}

/// API 에러 응답
#[derive(Debug, Serialize)]
pub struct SimulationApiError {
    /// 에러 코드
    pub code: String,
    /// 에러 메시지
    pub message: String,
}

impl SimulationApiError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

// ==================== 전역 시뮬레이션 엔진 ====================

lazy_static::lazy_static! {
    /// 전역 시뮬레이션 엔진
    static ref SIMULATION_ENGINE: SharedSimulationEngine = create_simulation_engine();
    /// 백그라운드 러너 핸들
    static ref RUNNER_HANDLE: Arc<RwLock<Option<JoinHandle<()>>>> = Arc::new(RwLock::new(None));
}

// ==================== 백그라운드 러너 ====================

/// 시뮬레이션 백그라운드 러너
async fn simulation_runner(engine: SharedSimulationEngine, speed: f64) {
    // 1초에 처리할 캔들 수 = speed
    // 따라서 캔들당 대기 시간 = 1/speed 초
    let delay_per_candle = std::time::Duration::from_secs_f64(1.0 / speed);

    loop {
        // 다음 캔들 처리
        let should_continue = {
            let mut engine = engine.write().await;

            // 일시정지 상태면 대기
            if engine.state == SimulationState::Paused {
                drop(engine);
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
            }

            // 중지 상태면 종료
            if engine.state == SimulationState::Stopped {
                break;
            }

            // 다음 캔들 처리
            match engine.process_next_candle().await {
                Ok(has_more) => has_more,
                Err(e) => {
                    tracing::error!("시뮬레이션 오류: {}", e);
                    engine.stop();
                    false
                }
            }
        };

        if !should_continue {
            // 시뮬레이션 완료
            let mut engine = engine.write().await;
            engine.stop();
            tracing::info!("시뮬레이션 완료");
            break;
        }

        // 속도에 따른 대기
        tokio::time::sleep(delay_per_candle).await;
    }
}

// ==================== 핸들러 ====================

/// 시뮬레이션 시작
///
/// POST /api/v1/simulation/start
pub async fn start_simulation(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SimulationStartRequest>,
) -> Result<Json<SimulationStartResponse>, (StatusCode, Json<SimulationApiError>)> {
    // 입력 검증
    if request.speed <= 0.0 || request.speed > 1000.0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(SimulationApiError::new(
                "INVALID_SPEED",
                "속도는 0.1 ~ 1000 사이여야 합니다",
            )),
        ));
    }

    // 기존 러너 중지
    {
        let mut handle = RUNNER_HANDLE.write().await;
        if let Some(h) = handle.take() {
            h.abort();
        }
    }

    // 엔진 초기화
    let (started_at, total_candles) = {
        let mut engine = SIMULATION_ENGINE.write().await;

        // 이미 실행 중인지 확인
        if engine.state == SimulationState::Running {
            return Err((
                StatusCode::CONFLICT,
                Json(SimulationApiError::new(
                    "ALREADY_RUNNING",
                    "시뮬레이션이 이미 실행 중입니다. 먼저 중지해주세요.",
                )),
            ));
        }

        // DB 풀 확인
        let db_pool = state.db_pool.as_ref().ok_or_else(|| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(SimulationApiError::new(
                    "DB_UNAVAILABLE",
                    "데이터베이스가 연결되어 있지 않습니다",
                )),
            )
        })?;

        // 시뮬레이션 초기화
        engine
            .initialize(
                &request.strategy_id,
                request.parameters,
                &request.symbols,
                &request.start_date,
                &request.end_date,
                request.initial_balance,
                request.speed,
                request.commission_rate,
                request.slippage_rate,
                db_pool,
            )
            .await
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(SimulationApiError::new("INIT_FAILED", e)),
                )
            })?;

        let started_at = engine.started_at.unwrap_or_else(Utc::now);
        let total_candles = engine.klines.len();

        (started_at, total_candles)
    };

    // 백그라운드 러너 시작
    let engine_clone = SIMULATION_ENGINE.clone();
    let handle = tokio::spawn(simulation_runner(engine_clone, request.speed));

    {
        let mut runner = RUNNER_HANDLE.write().await;
        *runner = Some(handle);
    }

    Ok(Json(SimulationStartResponse {
        success: true,
        message: format!("시뮬레이션이 시작되었습니다 ({} 캔들)", total_candles),
        started_at,
        total_candles,
    }))
}

/// 시뮬레이션 중지
///
/// POST /api/v1/simulation/stop
pub async fn stop_simulation(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<SimulationStopResponse>, (StatusCode, Json<SimulationApiError>)> {
    // 러너 중지
    {
        let mut handle = RUNNER_HANDLE.write().await;
        if let Some(h) = handle.take() {
            h.abort();
        }
    }

    // 엔진 중지 및 결과 추출
    let (final_equity, initial_balance, total_trades) = {
        let mut engine = SIMULATION_ENGINE.write().await;

        if engine.state == SimulationState::Stopped && engine.started_at.is_none() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(SimulationApiError::new(
                    "NOT_RUNNING",
                    "시뮬레이션이 실행 중이 아닙니다",
                )),
            ));
        }

        let final_equity = engine.total_equity();
        let initial_balance = engine.initial_balance;
        let total_trades = engine.trades.len();
        engine.stop();

        (final_equity, initial_balance, total_trades)
    };

    let total_return_pct = if initial_balance > Decimal::ZERO {
        (final_equity - initial_balance) / initial_balance * dec!(100)
    } else {
        Decimal::ZERO
    };

    Ok(Json(SimulationStopResponse {
        success: true,
        message: "시뮬레이션이 중지되었습니다".to_string(),
        final_equity,
        total_return_pct,
        total_trades,
    }))
}

/// 시뮬레이션 일시정지/재개
///
/// POST /api/v1/simulation/pause
pub async fn pause_simulation(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut engine = SIMULATION_ENGINE.write().await;

    match engine.state {
        SimulationState::Running => {
            engine.pause();
            Json(serde_json::json!({
                "success": true,
                "state": "paused",
                "message": "시뮬레이션이 일시정지되었습니다"
            }))
        }
        SimulationState::Paused => {
            engine.resume();
            Json(serde_json::json!({
                "success": true,
                "state": "running",
                "message": "시뮬레이션이 재개되었습니다"
            }))
        }
        SimulationState::Stopped => Json(serde_json::json!({
            "success": false,
            "state": "stopped",
            "message": "시뮬레이션이 실행 중이 아닙니다"
        })),
    }
}

/// 시뮬레이션 상태 조회
///
/// GET /api/v1/simulation/status
pub async fn get_simulation_status(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    let engine = SIMULATION_ENGINE.read().await;

    let total_equity = engine.total_equity();
    let unrealized_pnl: Decimal = engine.positions.values().map(|p| p.unrealized_pnl).sum();

    let return_pct = if engine.initial_balance > Decimal::ZERO {
        (total_equity - engine.initial_balance) / engine.initial_balance * dec!(100)
    } else {
        Decimal::ZERO
    };

    Json(SimulationStatusResponse {
        state: engine.state,
        strategy_id: engine.strategy_id.clone(),
        initial_balance: engine.initial_balance,
        current_balance: engine.current_balance,
        total_equity,
        unrealized_pnl,
        realized_pnl: engine.total_realized_pnl,
        return_pct,
        position_count: engine.positions.len(),
        trade_count: engine.trades.len(),
        started_at: engine.started_at,
        speed: engine.speed,
        current_simulation_time: engine.current_simulation_time,
        progress_pct: engine.progress_pct(),
        current_candle_index: engine.current_kline_index,
        total_candles: engine.klines.len(),
    })
}

/// 포지션 목록 조회
///
/// GET /api/v1/simulation/positions
pub async fn get_simulation_positions(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let engine = SIMULATION_ENGINE.read().await;

    let mut positions: Vec<SimulationPosition> = engine.positions.values().cloned().collect();
    let total_unrealized_pnl: Decimal = positions.iter().map(|p| p.unrealized_pnl).sum();

    // display_name 설정
    let symbols: Vec<String> = positions.iter().map(|p| p.symbol.clone()).collect();
    let display_names = state.get_display_names(&symbols, false).await;
    for pos in positions.iter_mut() {
        if let Some(name) = display_names.get(&pos.symbol) {
            pos.display_name = Some(name.clone());
        }
    }

    Json(SimulationPositionsResponse {
        positions,
        total_unrealized_pnl,
    })
}

/// 거래 내역 조회
///
/// GET /api/v1/simulation/trades
pub async fn get_simulation_trades(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let engine = SIMULATION_ENGINE.read().await;

    let mut trades = engine.trades.clone();
    let total = trades.len();

    // display_name 설정
    let symbols: Vec<String> = trades.iter().map(|t| t.symbol.clone()).collect();
    let display_names = state.get_display_names(&symbols, false).await;
    for trade in trades.iter_mut() {
        if let Some(name) = display_names.get(&trade.symbol) {
            trade.display_name = Some(name.clone());
        }
    }

    Json(SimulationTradesResponse {
        trades,
        total,
        total_realized_pnl: engine.total_realized_pnl,
        total_commission: engine.total_commission,
    })
}

/// 자산 곡선 조회
///
/// GET /api/v1/simulation/equity
pub async fn get_simulation_equity(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    let engine = SIMULATION_ENGINE.read().await;

    let max_drawdown_pct = engine
        .equity_curve
        .iter()
        .map(|e| e.drawdown_pct)
        .max()
        .unwrap_or(Decimal::ZERO);

    Json(SimulationEquityResponse {
        equity_curve: engine.equity_curve.clone(),
        max_drawdown_pct,
    })
}

/// 신호 마커 조회
///
/// GET /api/v1/simulation/signals
pub async fn get_simulation_signals(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    let engine = SIMULATION_ENGINE.read().await;

    Json(SimulationSignalsResponse {
        signals: engine.signal_markers.clone(),
        total: engine.signal_markers.len(),
    })
}

/// 시뮬레이션 리셋
///
/// POST /api/v1/simulation/reset
pub async fn reset_simulation(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    // 러너 중지
    {
        let mut handle = RUNNER_HANDLE.write().await;
        if let Some(h) = handle.take() {
            h.abort();
        }
    }

    // 엔진 리셋
    let mut engine = SIMULATION_ENGINE.write().await;
    *engine = SimulationEngine::default();

    Json(serde_json::json!({
        "success": true,
        "message": "시뮬레이션이 초기화되었습니다"
    }))
}

// ==================== 라우터 ====================

/// 시뮬레이션 라우터 생성
pub fn simulation_router() -> Router<Arc<AppState>> {
    Router::new()
        // 시뮬레이션 제어
        .route("/start", post(start_simulation))
        .route("/stop", post(stop_simulation))
        .route("/pause", post(pause_simulation))
        .route("/reset", post(reset_simulation))
        // 상태 조회
        .route("/status", get(get_simulation_status))
        .route("/positions", get(get_simulation_positions))
        .route("/trades", get(get_simulation_trades))
        .route("/equity", get(get_simulation_equity))
        .route("/signals", get(get_simulation_signals))
}

// ==================== 테스트 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_simulation_engine_default() {
        let engine = SimulationEngine::default();
        assert_eq!(engine.state, SimulationState::Stopped);
        assert_eq!(engine.initial_balance, dec!(10_000_000));
        assert_eq!(engine.current_balance, dec!(10_000_000));
    }

    #[tokio::test]
    async fn test_simulation_engine_new() {
        let engine = SimulationEngine::new(dec!(5_000_000));
        assert_eq!(engine.initial_balance, dec!(5_000_000));
        assert_eq!(engine.current_balance, dec!(5_000_000));
        assert_eq!(engine.peak_equity, dec!(5_000_000));
    }

    #[test]
    fn test_simulation_api_error() {
        let error = SimulationApiError::new("TEST_ERROR", "테스트 에러");
        assert_eq!(error.code, "TEST_ERROR");
        assert_eq!(error.message, "테스트 에러");
    }
}
