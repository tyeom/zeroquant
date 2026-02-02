//! 시뮬레이션 모드 API 엔드포인트
//!
//! 실시간 시장 데이터를 기반으로 가상 거래를 실행하는 시뮬레이션 모드를 제공합니다.
//!
//! # 엔드포인트
//!
//! - `POST /api/v1/simulation/start` - 시뮬레이션 시작
//! - `POST /api/v1/simulation/stop` - 시뮬레이션 중지
//! - `GET /api/v1/simulation/status` - 현재 상태 조회
//! - `POST /api/v1/simulation/order` - 가상 주문 실행
//! - `GET /api/v1/simulation/positions` - 가상 포지션 조회
//! - `GET /api/v1/simulation/trades` - 거래 내역 조회

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use trader_core::{unrealized_pnl, Side};
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

/// 가상 포지션
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationPosition {
    /// 심볼
    pub symbol: String,
    /// 표시 이름 (예: "005930(삼성전자)")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// 방향 (Long/Short)
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

/// 가상 거래 내역
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationTrade {
    /// 거래 ID
    pub id: String,
    /// 심볼
    pub symbol: String,
    /// 표시 이름 (예: "005930(삼성전자)")
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
    /// 거래 시간
    pub timestamp: DateTime<Utc>,
}

/// 시뮬레이션 엔진 상태
#[derive(Debug)]
pub struct SimulationEngine {
    /// 현재 상태
    pub state: SimulationState,
    /// 전략 ID
    pub strategy_id: Option<String>,
    /// 초기 잔고
    pub initial_balance: Decimal,
    /// 현재 잔고
    pub current_balance: Decimal,
    /// 가상 포지션 목록
    pub positions: HashMap<String, SimulationPosition>,
    /// 거래 내역
    pub trades: Vec<SimulationTrade>,
    /// 시작 시간 (실제 시간)
    pub started_at: Option<DateTime<Utc>>,
    /// 시뮬레이션 속도 (1.0 = 실시간)
    pub speed: f64,
    /// 총 실현 손익
    pub total_realized_pnl: Decimal,
    /// 총 수수료
    pub total_commission: Decimal,
    /// 시뮬레이션(백테스트) 시작 날짜
    pub simulation_start_date: Option<String>,
    /// 시뮬레이션(백테스트) 종료 날짜
    pub simulation_end_date: Option<String>,
}

impl Default for SimulationEngine {
    fn default() -> Self {
        Self {
            state: SimulationState::Stopped,
            strategy_id: None,
            initial_balance: dec!(10_000_000), // 1천만원
            current_balance: dec!(10_000_000),
            positions: HashMap::new(),
            trades: Vec::new(),
            started_at: None,
            speed: 1.0,
            total_realized_pnl: Decimal::ZERO,
            total_commission: Decimal::ZERO,
            simulation_start_date: None,
            simulation_end_date: None,
        }
    }
}

impl SimulationEngine {
    /// 새로운 시뮬레이션 엔진 생성
    pub fn new(initial_balance: Decimal) -> Self {
        Self {
            initial_balance,
            current_balance: initial_balance,
            ..Default::default()
        }
    }

    /// 시뮬레이션 시작
    pub fn start(
        &mut self,
        strategy_id: String,
        initial_balance: Decimal,
        speed: f64,
        start_date: Option<String>,
        end_date: Option<String>,
    ) {
        self.state = SimulationState::Running;
        self.strategy_id = Some(strategy_id);
        self.initial_balance = initial_balance;
        self.current_balance = initial_balance;
        self.positions.clear();
        self.trades.clear();
        self.started_at = Some(Utc::now());
        self.speed = speed;
        self.total_realized_pnl = Decimal::ZERO;
        self.total_commission = Decimal::ZERO;
        self.simulation_start_date = start_date;
        self.simulation_end_date = end_date;
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

    /// 가상 주문 실행
    pub fn execute_order(
        &mut self,
        symbol: &str,
        side: Side,
        quantity: Decimal,
        price: Decimal,
    ) -> Result<SimulationTrade, String> {
        if self.state != SimulationState::Running {
            return Err("시뮬레이션이 실행 중이 아닙니다".to_string());
        }

        let commission_rate = dec!(0.001); // 0.1% 수수료
        let commission = price * quantity * commission_rate;
        let total_cost = price * quantity + commission;

        // 매수 주문 검증
        if side == Side::Buy && total_cost > self.current_balance {
            return Err(format!(
                "잔고 부족: 필요 {}, 가용 {}",
                total_cost, self.current_balance
            ));
        }

        // 거래 생성
        let trade_id = Uuid::new_v4().to_string();
        let mut realized_pnl = None;

        // 포지션 업데이트
        match side {
            Side::Buy => {
                self.current_balance -= total_cost;

                // 기존 롱 포지션이 있으면 평균 단가 계산
                if let Some(pos) = self.positions.get_mut(symbol) {
                    if pos.side == "Long" {
                        let total_qty = pos.quantity + quantity;
                        let total_value = pos.entry_price * pos.quantity + price * quantity;
                        pos.entry_price = total_value / total_qty;
                        pos.quantity = total_qty;
                        pos.current_price = price;
                    } else {
                        // 숏 포지션 청산
                        let pnl = (pos.entry_price - price) * quantity.min(pos.quantity);
                        realized_pnl = Some(pnl);
                        self.total_realized_pnl += pnl;
                        pos.quantity -= quantity;
                        if pos.quantity <= Decimal::ZERO {
                            self.positions.remove(symbol);
                        }
                    }
                } else {
                    // 새 롱 포지션
                    self.positions.insert(
                        symbol.to_string(),
                        SimulationPosition {
                            symbol: symbol.to_string(),
                            display_name: None,
                            side: "Long".to_string(),
                            quantity,
                            entry_price: price,
                            current_price: price,
                            unrealized_pnl: Decimal::ZERO,
                            return_pct: Decimal::ZERO,
                            entry_time: Utc::now(),
                        },
                    );
                }
            }
            Side::Sell => {
                // Sell
                if let Some(pos) = self.positions.get_mut(symbol) {
                    if pos.side == "Long" {
                        // 롱 포지션 청산
                        let pnl = (price - pos.entry_price) * quantity.min(pos.quantity);
                        realized_pnl = Some(pnl);
                        self.total_realized_pnl += pnl;
                        self.current_balance += price * quantity - commission;
                        pos.quantity -= quantity;
                        if pos.quantity <= Decimal::ZERO {
                            self.positions.remove(symbol);
                        }
                    }
                } else {
                    // 새 숏 포지션
                    self.positions.insert(
                        symbol.to_string(),
                        SimulationPosition {
                            symbol: symbol.to_string(),
                            display_name: None,
                            side: "Short".to_string(),
                            quantity,
                            entry_price: price,
                            current_price: price,
                            unrealized_pnl: Decimal::ZERO,
                            return_pct: Decimal::ZERO,
                            entry_time: Utc::now(),
                        },
                    );
                }
            }
        }

        self.total_commission += commission;

        // side를 표시용 문자열로 변환
        let side_str = match side {
            Side::Buy => "Buy",
            Side::Sell => "Sell",
        };

        let trade = SimulationTrade {
            id: trade_id,
            symbol: symbol.to_string(),
            display_name: None,
            side: side_str.to_string(),
            quantity,
            price,
            commission,
            realized_pnl,
            timestamp: Utc::now(),
        };

        self.trades.push(trade.clone());

        Ok(trade)
    }

    /// 포지션 가격 업데이트
    pub fn update_price(&mut self, symbol: &str, price: Decimal) {
        if let Some(pos) = self.positions.get_mut(symbol) {
            pos.current_price = price;
            let side = if pos.side == "Long" {
                Side::Buy
            } else {
                Side::Sell
            };
            pos.unrealized_pnl = unrealized_pnl(pos.entry_price, price, pos.quantity, side);
            if pos.entry_price > Decimal::ZERO {
                pos.return_pct = pos.unrealized_pnl / (pos.entry_price * pos.quantity) * dec!(100);
            }
        }
    }

    /// 총 자산 계산
    pub fn total_equity(&self) -> Decimal {
        let positions_value: Decimal = self
            .positions
            .values()
            .map(|p| p.current_price * p.quantity + p.unrealized_pnl)
            .sum();
        self.current_balance + positions_value
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
    /// 초기 잔고 (선택)
    #[serde(default = "default_initial_balance")]
    pub initial_balance: Decimal,
    /// 시뮬레이션 속도 (1.0 = 실시간, 2.0 = 2배속)
    #[serde(default = "default_speed")]
    pub speed: f64,
    /// 시뮬레이션(백테스트) 시작 날짜 (YYYY-MM-DD)
    #[serde(default)]
    pub start_date: Option<String>,
    /// 시뮬레이션(백테스트) 종료 날짜 (YYYY-MM-DD)
    #[serde(default)]
    pub end_date: Option<String>,
}

fn default_initial_balance() -> Decimal {
    dec!(10_000_000)
}

fn default_speed() -> f64 {
    1.0
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
    /// 시작 시간 (실제 시간)
    pub started_at: Option<DateTime<Utc>>,
    /// 시뮬레이션 속도
    pub speed: f64,
    /// 현재 시뮬레이션 시간 (배속 적용된 가상 시간)
    pub current_simulation_time: Option<DateTime<Utc>>,
    /// 시뮬레이션(백테스트) 시작 날짜 (YYYY-MM-DD)
    pub simulation_start_date: Option<String>,
    /// 시뮬레이션(백테스트) 종료 날짜 (YYYY-MM-DD)
    pub simulation_end_date: Option<String>,
}

/// 가상 주문 요청
#[derive(Debug, Deserialize)]
pub struct SimulationOrderRequest {
    /// 심볼
    pub symbol: String,
    /// 방향 (Buy/Sell)
    pub side: String,
    /// 수량
    pub quantity: Decimal,
    /// 가격 (시장가면 None)
    pub price: Option<Decimal>,
}

/// 가상 주문 응답
#[derive(Debug, Serialize)]
pub struct SimulationOrderResponse {
    /// 성공 여부
    pub success: bool,
    /// 거래 정보 (성공 시)
    pub trade: Option<SimulationTrade>,
    /// 에러 메시지 (실패 시)
    pub error: Option<String>,
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
}

// ==================== 핸들러 ====================

/// 시뮬레이션 시작
///
/// POST /api/v1/simulation/start
pub async fn start_simulation(
    State(_state): State<Arc<AppState>>,
    Json(request): Json<SimulationStartRequest>,
) -> Result<Json<SimulationStartResponse>, (StatusCode, Json<SimulationApiError>)> {
    // 락 획득 전에 입력 검증 수행
    if request.speed <= 0.0 || request.speed > 100.0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(SimulationApiError::new(
                "INVALID_SPEED",
                "속도는 0.1 ~ 100 사이여야 합니다",
            )),
        ));
    }

    // 최소 락 홀드: 상태 확인, 시작, 결과 추출 후 즉시 해제
    let started_at = {
        let mut engine = SIMULATION_ENGINE.write().await;

        // 이미 실행 중인지 확인
        if engine.state == SimulationState::Running {
            return Err((
                StatusCode::CONFLICT,
                Json(SimulationApiError::new(
                    "ALREADY_RUNNING",
                    "시뮬레이션이 이미 실행 중입니다",
                )),
            ));
        }

        // 시뮬레이션 시작
        engine.start(
            request.strategy_id,
            request.initial_balance,
            request.speed,
            request.start_date,
            request.end_date,
        );

        engine.started_at.ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SimulationApiError::new(
                    "ENGINE_NOT_STARTED",
                    "시뮬레이션 엔진이 올바르게 시작되지 않았습니다",
                )),
            )
        })?
    }; // 락 해제됨

    Ok(Json(SimulationStartResponse {
        success: true,
        message: "시뮬레이션이 시작되었습니다".to_string(),
        started_at,
    }))
}

/// 시뮬레이션 중지
///
/// POST /api/v1/simulation/stop
pub async fn stop_simulation(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<SimulationStopResponse>, (StatusCode, Json<SimulationApiError>)> {
    // 최소 락 홀드: 필요한 데이터만 추출하고 상태 변경 후 즉시 해제
    let (final_equity, initial_balance, total_trades) = {
        let mut engine = SIMULATION_ENGINE.write().await;

        if engine.state == SimulationState::Stopped {
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
    }; // 락 해제됨

    // 락 없이 계산 수행
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
    // 최소 락 홀드: 필요한 모든 데이터를 빠르게 추출
    let (
        state,
        strategy_id,
        initial_balance,
        current_balance,
        total_equity,
        unrealized_pnl,
        total_realized_pnl,
        position_count,
        trade_count,
        started_at,
        speed,
        simulation_start_date,
        simulation_end_date,
    ) = {
        let engine = SIMULATION_ENGINE.read().await;
        (
            engine.state,
            engine.strategy_id.clone(),
            engine.initial_balance,
            engine.current_balance,
            engine.total_equity(),
            engine
                .positions
                .values()
                .map(|p| p.unrealized_pnl)
                .sum::<Decimal>(),
            engine.total_realized_pnl,
            engine.positions.len(),
            engine.trades.len(),
            engine.started_at,
            engine.speed,
            engine.simulation_start_date.clone(),
            engine.simulation_end_date.clone(),
        )
    }; // 락 해제됨

    // 락 없이 계산 수행
    let return_pct = if initial_balance > Decimal::ZERO {
        (total_equity - initial_balance) / initial_balance * dec!(100)
    } else {
        Decimal::ZERO
    };

    // 현재 시뮬레이션 시간 계산 (배속 적용)
    let current_simulation_time = if state == SimulationState::Running {
        started_at.map(|start| {
            let now = Utc::now();
            let elapsed_real_ms = (now - start).num_milliseconds();
            let elapsed_sim_ms = (elapsed_real_ms as f64 * speed) as i64;
            start + chrono::Duration::milliseconds(elapsed_sim_ms)
        })
    } else {
        None
    };

    Json(SimulationStatusResponse {
        state,
        strategy_id,
        initial_balance,
        current_balance,
        total_equity,
        unrealized_pnl,
        realized_pnl: total_realized_pnl,
        return_pct,
        position_count,
        trade_count,
        started_at,
        speed,
        current_simulation_time,
        simulation_start_date,
        simulation_end_date,
    })
}

/// 가상 주문 실행
///
/// POST /api/v1/simulation/order
pub async fn execute_simulation_order(
    State(_state): State<Arc<AppState>>,
    Json(request): Json<SimulationOrderRequest>,
) -> Result<Json<SimulationOrderResponse>, (StatusCode, Json<SimulationApiError>)> {
    let mut engine = SIMULATION_ENGINE.write().await;

    // 가격이 없으면 현재 시장가 시뮬레이션 (임시로 고정값 사용)
    let price = request.price.unwrap_or(dec!(50000));

    // side 문자열을 Side enum으로 파싱
    let side = match Side::from_str_flexible(&request.side) {
        Ok(s) => s,
        Err(_) => {
            return Ok(Json(SimulationOrderResponse {
                success: false,
                trade: None,
                error: Some(format!("잘못된 주문 방향: {}", request.side)),
            }));
        }
    };

    match engine.execute_order(&request.symbol, side, request.quantity, price) {
        Ok(trade) => Ok(Json(SimulationOrderResponse {
            success: true,
            trade: Some(trade),
            error: None,
        })),
        Err(e) => Ok(Json(SimulationOrderResponse {
            success: false,
            trade: None,
            error: Some(e),
        })),
    }
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

/// 시뮬레이션 리셋
///
/// POST /api/v1/simulation/reset
pub async fn reset_simulation(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
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
        // 가상 주문
        .route("/order", post(execute_simulation_order))
}

// ==================== 테스트 ====================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_simulation_engine_basic() {
        let mut engine = SimulationEngine::new(dec!(1_000_000));

        assert_eq!(engine.state, SimulationState::Stopped);
        assert_eq!(engine.current_balance, dec!(1_000_000));

        engine.start(
            "test_strategy".to_string(),
            dec!(1_000_000),
            1.0,
            None,
            None,
        );
        assert_eq!(engine.state, SimulationState::Running);

        engine.pause();
        assert_eq!(engine.state, SimulationState::Paused);

        engine.resume();
        assert_eq!(engine.state, SimulationState::Running);

        engine.stop();
        assert_eq!(engine.state, SimulationState::Stopped);
    }

    #[tokio::test]
    async fn test_simulation_order_execution() {
        let mut engine = SimulationEngine::new(dec!(1_000_000));
        engine.start("test".to_string(), dec!(1_000_000), 1.0, None, None);

        // 매수 주문
        let result = engine.execute_order("BTC/USDT", Side::Buy, dec!(0.1), dec!(50000));
        assert!(result.is_ok());

        let trade = result.unwrap();
        assert_eq!(trade.symbol, "BTC/USDT");
        assert_eq!(trade.side, "Buy");

        // 포지션 확인
        assert!(engine.positions.contains_key("BTC/USDT"));
        let pos = engine.positions.get("BTC/USDT").unwrap();
        assert_eq!(pos.quantity, dec!(0.1));

        // 매도 주문 (청산)
        let result = engine.execute_order("BTC/USDT", Side::Sell, dec!(0.1), dec!(51000));
        assert!(result.is_ok());

        let trade = result.unwrap();
        assert!(trade.realized_pnl.is_some());
    }

    #[tokio::test]
    async fn test_simulation_insufficient_balance() {
        let mut engine = SimulationEngine::new(dec!(1000));
        engine.start("test".to_string(), dec!(1000), 1.0, None, None);

        // 잔고 부족 주문
        let result = engine.execute_order("BTC/USDT", Side::Buy, dec!(1), dec!(50000));
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_simulation_status() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/status", get(get_simulation_status))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let status: SimulationStatusResponse = serde_json::from_slice(&body).unwrap();

        // 기본 상태 확인
        assert_eq!(status.state, SimulationState::Stopped);
    }

    #[tokio::test]
    async fn test_start_and_stop_simulation() {
        use crate::state::create_test_state;

        // 먼저 리셋
        {
            let mut engine = SIMULATION_ENGINE.write().await;
            *engine = SimulationEngine::default();
        }

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/start", post(start_simulation))
            .route("/stop", post(stop_simulation))
            .with_state(state);

        // 시작
        let start_request = serde_json::json!({
            "strategy_id": "test_strategy",
            "initial_balance": 5000000,
            "speed": 2.0
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/start")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&start_request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // 중지
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/stop")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn test_simulation_api_error() {
        let error = SimulationApiError::new("TEST_ERROR", "테스트 에러");
        assert_eq!(error.code, "TEST_ERROR");
        assert_eq!(error.message, "테스트 에러");
    }
}
