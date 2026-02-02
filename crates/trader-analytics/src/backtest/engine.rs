//! 백테스팅 엔진
//!
//! 과거 데이터로 트레이딩 전략을 시뮬레이션하고 성과를 분석합니다.
//!
//! # 주요 기능
//!
//! - **전략 시뮬레이션**: 과거 시장 데이터로 전략의 신호 생성 및 실행
//! - **주문 체결 시뮬레이션**: 슬리피지, 수수료 등 현실적인 체결 모델
//! - **성과 분석**: PerformanceTracker와 통합된 상세한 성과 지표
//! - **자산 곡선**: 시간에 따른 자산 가치 변화 추적
//!
//! # 사용 예시
//!
//! ```rust,ignore
//! use trader_analytics::backtest::{BacktestConfig, BacktestEngine};
//! use trader_strategy::Strategy;
//! use rust_decimal_macros::dec;
//!
//! // 백테스트 설정
//! let config = BacktestConfig::new(dec!(10_000_000))
//!     .with_commission_rate(dec!(0.001))  // 0.1% 수수료
//!     .with_slippage_rate(dec!(0.0005));  // 0.05% 슬리피지
//!
//! // 백테스트 엔진 생성
//! let mut engine = BacktestEngine::new(config);
//!
//! // 백테스트 실행
//! let result = engine.run(&mut strategy, &historical_klines).await?;
//!
//! // 결과 분석
//! println!("총 수익률: {}%", result.metrics.total_return_pct);
//! println!("샤프 비율: {}", result.metrics.sharpe_ratio);
//! println!("최대 낙폭: {}%", result.metrics.max_drawdown_pct);
//! ```

use chrono::{DateTime, Utc};
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use trader_core::{unrealized_pnl, Kline, MarketData, Side, Signal, SignalType, Symbol, Trade};
use uuid::Uuid;

use crate::backtest::slippage::SlippageModel;
use crate::performance::{EquityPoint, PerformanceMetrics, PerformanceTracker, RoundTrip};

/// 백테스트 오류
#[derive(Debug, Error)]
pub enum BacktestError {
    /// 설정 오류
    #[error("백테스트 설정 오류: {0}")]
    ConfigError(String),

    /// 데이터 오류
    #[error("데이터 오류: {0}")]
    DataError(String),

    /// 전략 오류
    #[error("전략 실행 오류: {0}")]
    StrategyError(String),

    /// 실행 오류
    #[error("실행 오류: {0}")]
    ExecutionError(String),

    /// 자금 부족
    #[error("자금 부족: 필요={required}, 가용={available}")]
    InsufficientFunds {
        required: Decimal,
        available: Decimal,
    },
}

/// 백테스트 결과 타입
pub type BacktestResult<T> = Result<T, BacktestError>;

/// 백테스트 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    /// 초기 자본금
    #[serde(default = "default_initial_capital")]
    pub initial_capital: Decimal,

    /// 거래 수수료율 (예: 0.001 = 0.1%)
    #[serde(default = "default_commission_rate")]
    pub commission_rate: Decimal,

    /// 슬리피지율 (예: 0.0005 = 0.05%)
    ///
    /// 참고: slippage_model이 설정되면 무시됩니다.
    #[serde(default = "default_slippage_rate")]
    pub slippage_rate: Decimal,

    /// 동적 슬리피지 모델 (Optional)
    ///
    /// 설정되면 slippage_rate 대신 이 모델을 사용합니다.
    /// Fixed, Linear, VolatilityBased, Tiered 모델 지원.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slippage_model: Option<SlippageModel>,

    /// 최대 동시 포지션 수
    #[serde(default = "default_max_positions")]
    pub max_positions: usize,

    /// 포지션당 최대 자본 비율 (예: 0.1 = 10%)
    #[serde(default = "default_max_position_size_pct")]
    pub max_position_size_pct: Decimal,

    /// 무위험 이자율 (연율화 계산용)
    #[serde(default = "default_risk_free_rate")]
    pub risk_free_rate: f64,

    /// 거래소 이름 (시뮬레이션용)
    #[serde(default = "default_exchange_name")]
    pub exchange_name: String,

    /// 틱 데이터 사용 여부 (캔들 내 가격 변동 시뮬레이션)
    #[serde(default)]
    pub use_tick_simulation: bool,

    /// 마진 거래 허용 여부
    #[serde(default)]
    pub allow_margin: bool,

    /// 숏 포지션 허용 여부
    #[serde(default)]
    pub allow_short: bool,
}

// 설정 기본값 함수들 (serde default용)
fn default_initial_capital() -> Decimal {
    Decimal::new(10_000_000, 0)
}
fn default_commission_rate() -> Decimal {
    Decimal::new(1, 3)
} // 0.1%
fn default_slippage_rate() -> Decimal {
    Decimal::new(5, 4)
} // 0.05%
fn default_max_positions() -> usize {
    10
}
fn default_max_position_size_pct() -> Decimal {
    Decimal::new(2, 1)
} // 20%
fn default_risk_free_rate() -> f64 {
    0.05
} // 5%
fn default_exchange_name() -> String {
    "backtest".to_string()
}

impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            initial_capital: default_initial_capital(),
            commission_rate: default_commission_rate(),
            slippage_rate: default_slippage_rate(),
            slippage_model: None,
            max_positions: default_max_positions(),
            max_position_size_pct: default_max_position_size_pct(),
            risk_free_rate: default_risk_free_rate(),
            exchange_name: default_exchange_name(),
            use_tick_simulation: false,
            allow_margin: false,
            allow_short: false,
        }
    }
}

impl BacktestConfig {
    /// 새로운 백테스트 설정을 생성합니다.
    pub fn new(initial_capital: Decimal) -> Self {
        Self {
            initial_capital,
            ..Default::default()
        }
    }

    /// 수수료율 설정
    pub fn with_commission_rate(mut self, rate: Decimal) -> Self {
        self.commission_rate = rate;
        self
    }

    /// 슬리피지율 설정 (고정 비율)
    ///
    /// 참고: with_slippage_model()로 동적 모델을 설정하면 무시됩니다.
    pub fn with_slippage_rate(mut self, rate: Decimal) -> Self {
        self.slippage_rate = rate;
        self
    }

    /// 동적 슬리피지 모델 설정
    ///
    /// 설정되면 slippage_rate 대신 이 모델을 사용합니다.
    ///
    /// # 예시
    /// ```rust,ignore
    /// use trader_analytics::backtest::{BacktestConfig, SlippageModel};
    ///
    /// // 변동성 기반 모델 사용
    /// let config = BacktestConfig::default()
    ///     .with_slippage_model(SlippageModel::volatility_based(0.5));
    ///
    /// // 구간별 차등 모델 사용
    /// let config = BacktestConfig::default()
    ///     .with_slippage_model(SlippageModel::tiered(vec![
    ///         (dec!(10000), dec!(0.0003)),
    ///         (dec!(100000), dec!(0.0005)),
    ///         (dec!(1000000), dec!(0.001)),
    ///     ]));
    /// ```
    pub fn with_slippage_model(mut self, model: SlippageModel) -> Self {
        self.slippage_model = Some(model);
        self
    }

    /// 최대 포지션 수 설정
    pub fn with_max_positions(mut self, max: usize) -> Self {
        self.max_positions = max;
        self
    }

    /// 포지션 크기 제한 설정
    pub fn with_max_position_size_pct(mut self, pct: Decimal) -> Self {
        self.max_position_size_pct = pct;
        self
    }

    /// 무위험 이자율 설정
    pub fn with_risk_free_rate(mut self, rate: f64) -> Self {
        self.risk_free_rate = rate;
        self
    }

    /// 숏 포지션 허용 설정
    pub fn with_allow_short(mut self, allow: bool) -> Self {
        self.allow_short = allow;
        self
    }

    /// 설정 검증
    pub fn validate(&self) -> BacktestResult<()> {
        if self.initial_capital <= Decimal::ZERO {
            return Err(BacktestError::ConfigError(
                "초기 자본은 0보다 커야 합니다".to_string(),
            ));
        }
        if self.commission_rate < Decimal::ZERO {
            return Err(BacktestError::ConfigError(
                "수수료율은 0 이상이어야 합니다".to_string(),
            ));
        }
        if self.slippage_rate < Decimal::ZERO {
            return Err(BacktestError::ConfigError(
                "슬리피지율은 0 이상이어야 합니다".to_string(),
            ));
        }
        Ok(())
    }
}

/// 시뮬레이션된 포지션
#[derive(Debug, Clone)]
struct SimulatedPosition {
    /// 심볼
    symbol: Symbol,
    /// 방향
    side: Side,
    /// 수량
    quantity: Decimal,
    /// 평균 진입가
    entry_price: Decimal,
    /// 총 수수료 (나중에 비용 계산에 사용 예정)
    #[allow(dead_code)]
    fees: Decimal,
    /// 진입 시각 (보고서 생성에 사용 예정)
    #[allow(dead_code)]
    entry_time: DateTime<Utc>,
    /// 전략 ID
    strategy_id: String,
}

/// 백테스트 실행 리포트
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestReport {
    /// 설정 정보
    pub config: BacktestConfig,

    /// 성과 지표
    pub metrics: PerformanceMetrics,

    /// 완료된 거래 (라운드트립)
    pub trades: Vec<RoundTrip>,

    /// 자산 곡선
    pub equity_curve: Vec<EquityPoint>,

    /// 총 거래 횟수 (진입 + 청산)
    pub total_orders: usize,

    /// 총 수수료
    pub total_commission: Decimal,

    /// 총 슬리피지 비용
    pub total_slippage: Decimal,

    /// 백테스트 기간 시작
    pub start_time: DateTime<Utc>,

    /// 백테스트 기간 종료
    pub end_time: DateTime<Utc>,

    /// 데이터 포인트 수
    pub data_points: usize,

    /// 심볼별 성과
    pub performance_by_symbol: HashMap<String, PerformanceMetrics>,
}

impl BacktestReport {
    /// 요약 문자열 반환
    pub fn summary(&self) -> String {
        let duration_days = (self.end_time - self.start_time).num_days();

        format!(
            "백테스트 결과 요약\n\
             ═══════════════════════════════════════\n\
             기간: {} → {} ({} 일)\n\
             데이터 포인트: {}\n\
             ───────────────────────────────────────\n\
             초기 자본: {}\n\
             최종 자산: {:.2}\n\
             순수익: {:.2}\n\
             총 수익률: {:.2}%\n\
             연율화 수익률: {:.2}%\n\
             ───────────────────────────────────────\n\
             총 거래: {}\n\
             승률: {:.1}%\n\
             프로핏 팩터: {:.2}\n\
             ───────────────────────────────────────\n\
             샤프 비율: {:.2}\n\
             소르티노 비율: {:.2}\n\
             최대 낙폭: {:.2}%\n\
             칼마 비율: {:.2}\n\
             ───────────────────────────────────────\n\
             총 수수료: {:.2}\n\
             총 슬리피지: {:.2}\n\
             ═══════════════════════════════════════",
            self.start_time.format("%Y-%m-%d"),
            self.end_time.format("%Y-%m-%d"),
            duration_days,
            self.data_points,
            self.config.initial_capital,
            self.config.initial_capital + self.metrics.net_profit,
            self.metrics.net_profit,
            self.metrics.total_return_pct,
            self.metrics.annualized_return_pct,
            self.metrics.total_trades,
            self.metrics.win_rate_pct,
            self.metrics.profit_factor,
            self.metrics.sharpe_ratio,
            self.metrics.sortino_ratio,
            self.metrics.max_drawdown_pct,
            self.metrics.calmar_ratio,
            self.total_commission,
            self.total_slippage,
        )
    }
}

/// 백테스팅 엔진
///
/// 과거 데이터로 전략을 시뮬레이션하고 성과를 분석합니다.
pub struct BacktestEngine {
    /// 설정
    config: BacktestConfig,

    /// 현재 잔고
    balance: Decimal,

    /// 시뮬레이션 포지션
    positions: HashMap<String, SimulatedPosition>,

    /// 성과 추적기
    tracker: PerformanceTracker,

    /// 총 수수료
    total_commission: Decimal,

    /// 총 슬리피지
    total_slippage: Decimal,

    /// 총 주문 수
    total_orders: usize,

    /// 현재 시뮬레이션 시각
    current_time: DateTime<Utc>,

    /// 현재 가격 (심볼별)
    current_prices: HashMap<String, Decimal>,
}

impl BacktestEngine {
    /// 새로운 백테스트 엔진을 생성합니다.
    pub fn new(config: BacktestConfig) -> Self {
        // 백테스트용 트래커: 과거 데이터 자산 곡선 삭제 방지
        let tracker = PerformanceTracker::new(config.initial_capital)
            .with_risk_free_rate(config.risk_free_rate)
            .without_equity_history_limit();

        Self {
            balance: config.initial_capital,
            config,
            positions: HashMap::new(),
            tracker,
            total_commission: Decimal::ZERO,
            total_slippage: Decimal::ZERO,
            total_orders: 0,
            current_time: Utc::now(),
            current_prices: HashMap::new(),
        }
    }

    /// 캔들 데이터로 백테스트를 실행합니다.
    ///
    /// # 매개변수
    ///
    /// * `strategy` - 테스트할 전략 (Strategy trait 구현체)
    /// * `klines` - 과거 캔들 데이터 (시간순 정렬 필수)
    ///
    /// # 반환값
    ///
    /// 백테스트 결과 리포트
    pub async fn run<S>(
        &mut self,
        strategy: &mut S,
        klines: &[Kline],
    ) -> BacktestResult<BacktestReport>
    where
        S: trader_strategy::Strategy,
    {
        // 설정 검증
        self.config.validate()?;

        if klines.is_empty() {
            return Err(BacktestError::DataError(
                "캔들 데이터가 비어있습니다".to_string(),
            ));
        }

        // 시간순 정렬 확인
        for window in klines.windows(2) {
            if window[0].open_time > window[1].open_time {
                return Err(BacktestError::DataError(
                    "캔들 데이터가 시간순으로 정렬되어 있지 않습니다".to_string(),
                ));
            }
        }

        let start_time = klines.first().unwrap().open_time;
        let end_time = klines.last().unwrap().close_time;
        let data_points = klines.len();

        // 백테스트 시작 시간으로 equity curve 초기 timestamp 설정
        // (Utc::now() 대신 실제 백테스트 시작 시간 사용)
        self.tracker.set_initial_timestamp(start_time);

        // 각 캔들에 대해 시뮬레이션
        // 중요: Look-Ahead Bias 방지를 위해 캔들 완성 후 신호 생성
        for kline in klines {
            // 캔들 완성 시점으로 현재 시간 설정 (데이터 누수 방지)
            self.current_time = kline.close_time;
            self.current_prices
                .insert(kline.symbol.to_string(), kline.close);

            // 시장 데이터 생성 (완성된 캔들 정보 사용)
            let market_data = MarketData::from_kline(&self.config.exchange_name, kline.clone());

            // 전략에 데이터 전달 (캔들 완성 후 신호 생성)
            let signals = strategy
                .on_market_data(&market_data)
                .await
                .map_err(|e| BacktestError::StrategyError(e.to_string()))?;

            // 신호 처리 (다음 틱에서 체결된다고 가정)
            for signal in signals {
                self.process_signal(&signal, kline).await?;
            }

            // 미실현 손익 반영하여 자산 업데이트
            let equity = self.calculate_equity(kline);
            self.tracker.update_equity(kline.close_time, equity);
        }

        // 미청산 포지션 강제 청산
        self.close_all_positions(klines.last().unwrap()).await?;

        // 심볼별 성과 계산
        let performance_by_symbol = self.calculate_performance_by_symbol();

        // 결과 생성
        Ok(BacktestReport {
            config: self.config.clone(),
            metrics: self.tracker.get_metrics(),
            trades: self.tracker.get_round_trips().to_vec(),
            equity_curve: self.tracker.get_equity_curve().to_vec(),
            total_orders: self.total_orders,
            total_commission: self.total_commission,
            total_slippage: self.total_slippage,
            start_time,
            end_time,
            data_points,
            performance_by_symbol,
        })
    }

    /// 신호를 처리합니다.
    async fn process_signal(&mut self, signal: &Signal, kline: &Kline) -> BacktestResult<()> {
        match signal.signal_type {
            SignalType::Entry | SignalType::AddToPosition => {
                // 숏 포지션 확인
                if signal.side == Side::Sell && !self.config.allow_short {
                    return Ok(()); // 숏 비허용 시 무시
                }

                self.open_position(signal, kline).await?;
            }
            SignalType::Exit | SignalType::ReducePosition => {
                self.close_position(signal, kline).await?;
            }
            SignalType::Scale => {
                // 스케일 신호는 현재 포지션에 따라 처리
                let key = signal.symbol.to_string();
                if self.positions.contains_key(&key) {
                    self.close_position(signal, kline).await?;
                } else {
                    self.open_position(signal, kline).await?;
                }
            }
        }

        Ok(())
    }

    /// 포지션을 오픈합니다.
    async fn open_position(&mut self, signal: &Signal, kline: &Kline) -> BacktestResult<()> {
        let key = signal.symbol.to_string();

        // 최대 포지션 수 확인
        if self.positions.len() >= self.config.max_positions {
            return Ok(()); // 무시
        }

        // 이미 포지션이 있으면 무시 (간단한 구현)
        if self.positions.contains_key(&key) {
            return Ok(());
        }

        // 실행 가격 계산 (슬리피지 적용)
        // 다중 자산 전략에서는 신호 심볼과 현재 kline 심볼이 다를 수 있음
        // 1. signal.suggested_price가 있으면 사용
        // 2. current_prices에서 해당 심볼의 가격 사용
        // 3. fallback: kline.close (단일 자산 전략)
        let base_price = signal
            .suggested_price
            .or_else(|| self.current_prices.get(&key).copied())
            .unwrap_or(kline.close);
        let slippage = base_price * self.config.slippage_rate;
        let execution_price = match signal.side {
            Side::Buy => base_price + slippage,  // 매수는 높은 가격
            Side::Sell => base_price - slippage, // 매도는 낮은 가격
        };

        // 포지션 크기 계산
        let max_amount = self.balance * self.config.max_position_size_pct;
        let position_amount =
            max_amount * Decimal::from_f64(signal.strength).unwrap_or(Decimal::ONE);
        let quantity = position_amount / execution_price;

        // 자금 확인
        let required = position_amount;
        if required > self.balance {
            return Ok(()); // 자금 부족 시 무시
        }

        // 수수료 계산
        let commission = position_amount * self.config.commission_rate;

        // 잔고 차감
        self.balance -= required + commission;
        self.total_commission += commission;
        self.total_slippage += slippage * quantity;
        self.total_orders += 1;

        // 포지션 생성 (체결 시점을 close_time으로 통일)
        let position = SimulatedPosition {
            symbol: signal.symbol.clone(),
            side: signal.side,
            quantity,
            entry_price: execution_price,
            fees: commission,
            entry_time: kline.close_time,
            strategy_id: signal.strategy_id.clone(),
        };

        self.positions.insert(key.clone(), position);

        // 진입 거래 기록
        let trade = self.create_trade(signal, execution_price, quantity, commission, true);
        self.tracker
            .record_trade(&trade, true, Some(signal.strategy_id.clone()))
            .map_err(|e| BacktestError::ExecutionError(e.to_string()))?;

        Ok(())
    }

    /// 포지션을 청산합니다.
    async fn close_position(&mut self, signal: &Signal, kline: &Kline) -> BacktestResult<()> {
        let key = signal.symbol.to_string();

        let position = match self.positions.remove(&key) {
            Some(p) => p,
            None => return Ok(()), // 포지션 없으면 무시
        };

        // 실행 가격 계산 (슬리피지 적용)
        // 다중 자산 전략에서는 신호 심볼과 현재 kline 심볼이 다를 수 있음
        // 1. signal.suggested_price가 있으면 사용
        // 2. current_prices에서 해당 심볼의 가격 사용
        // 3. fallback: kline.close (단일 자산 전략)
        let base_price = signal
            .suggested_price
            .or_else(|| self.current_prices.get(&key).copied())
            .unwrap_or(kline.close);
        let slippage = base_price * self.config.slippage_rate;
        let execution_price = match position.side {
            Side::Buy => base_price - slippage,  // 롱 청산은 낮은 가격
            Side::Sell => base_price + slippage, // 숏 청산은 높은 가격
        };

        // 수수료 계산
        let position_value = execution_price * position.quantity;
        let commission = position_value * self.config.commission_rate;

        // PnL 계산 (디버깅용)
        let _gross_pnl = match position.side {
            Side::Buy => (execution_price - position.entry_price) * position.quantity,
            Side::Sell => (position.entry_price - execution_price) * position.quantity,
        };

        // 잔고 업데이트
        self.balance += position_value - commission;
        self.total_commission += commission;
        self.total_slippage += slippage * position.quantity;
        self.total_orders += 1;

        // 청산 거래 기록
        let exit_side = match position.side {
            Side::Buy => Side::Sell,
            Side::Sell => Side::Buy,
        };

        let trade = Trade::new(
            Uuid::new_v4(),
            &self.config.exchange_name,
            Uuid::new_v4().to_string(),
            signal.symbol.clone(),
            exit_side,
            position.quantity,
            execution_price,
        )
        .with_fee(commission, "USDT")
        .with_executed_at(kline.close_time);

        self.tracker
            .record_trade(&trade, false, Some(position.strategy_id.clone()))
            .map_err(|e| BacktestError::ExecutionError(e.to_string()))?;

        Ok(())
    }

    /// 모든 포지션을 청산합니다.
    async fn close_all_positions(&mut self, kline: &Kline) -> BacktestResult<()> {
        let positions: Vec<_> = self.positions.keys().cloned().collect();

        for key in positions {
            if let Some(position) = self.positions.get(&key) {
                let signal = Signal::exit(
                    &position.strategy_id,
                    position.symbol.clone(),
                    match position.side {
                        Side::Buy => Side::Sell,
                        Side::Sell => Side::Buy,
                    },
                );
                self.close_position(&signal, kline).await?;
            }
        }

        Ok(())
    }

    /// 현재 자산 가치를 계산합니다.
    fn calculate_equity(&self, kline: &Kline) -> Decimal {
        let mut equity = self.balance;

        for position in self.positions.values() {
            let current_price = self
                .current_prices
                .get(&position.symbol.to_string())
                .copied()
                .unwrap_or(kline.close);

            let position_value = match position.side {
                Side::Buy => current_price * position.quantity,
                Side::Sell => {
                    // 숏 포지션: 원금 + 미실현 손익
                    let entry_value = position.entry_price * position.quantity;
                    let pnl = unrealized_pnl(
                        position.entry_price,
                        current_price,
                        position.quantity,
                        position.side,
                    );
                    entry_value + pnl
                }
            };

            equity += position_value;
        }

        equity
    }

    /// Trade 객체를 생성합니다.
    fn create_trade(
        &self,
        signal: &Signal,
        price: Decimal,
        quantity: Decimal,
        fee: Decimal,
        _is_entry: bool,
    ) -> Trade {
        Trade::new(
            Uuid::new_v4(),
            &self.config.exchange_name,
            Uuid::new_v4().to_string(),
            signal.symbol.clone(),
            signal.side,
            quantity,
            price,
        )
        .with_fee(fee, "USDT")
        .with_executed_at(self.current_time)
    }

    /// 심볼별 성과를 계산합니다.
    fn calculate_performance_by_symbol(&self) -> HashMap<String, PerformanceMetrics> {
        let mut by_symbol: HashMap<String, Vec<RoundTrip>> = HashMap::new();

        for rt in self.tracker.get_round_trips() {
            by_symbol
                .entry(rt.symbol.clone())
                .or_default()
                .push(rt.clone());
        }

        by_symbol
            .into_iter()
            .map(|(symbol, trades)| {
                let metrics = PerformanceMetrics::from_round_trips(
                    &trades,
                    self.config.initial_capital,
                    Some(self.config.risk_free_rate),
                );
                (symbol, metrics)
            })
            .collect()
    }

    /// 현재 잔고를 반환합니다.
    pub fn balance(&self) -> Decimal {
        self.balance
    }

    /// 열린 포지션 수를 반환합니다.
    pub fn open_positions_count(&self) -> usize {
        self.positions.len()
    }
}

/// 간단한 테스트용 전략
#[cfg(test)]
pub mod test_strategies {
    use super::*;
    use async_trait::async_trait;
    use serde_json::Value;
    use trader_core::{MarketDataType, Order, Position};

    /// 단순 이동평균 크로스오버 전략 (테스트용)
    pub struct SimpleSmaStrategy {
        short_period: usize,
        long_period: usize,
        prices: Vec<Decimal>,
        position_open: bool,
    }

    impl SimpleSmaStrategy {
        pub fn new(short_period: usize, long_period: usize) -> Self {
            Self {
                short_period,
                long_period,
                prices: Vec::new(),
                position_open: false,
            }
        }

        fn calculate_sma(&self, period: usize) -> Option<Decimal> {
            if self.prices.len() < period {
                return None;
            }

            let sum: Decimal = self.prices.iter().rev().take(period).sum();
            Some(sum / Decimal::from(period))
        }
    }

    #[async_trait]
    impl trader_strategy::Strategy for SimpleSmaStrategy {
        fn name(&self) -> &str {
            "SimpleSMA"
        }

        fn version(&self) -> &str {
            "1.0.0"
        }

        fn description(&self) -> &str {
            "단순 이동평균 크로스오버 전략"
        }

        async fn initialize(
            &mut self,
            _config: Value,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.prices.clear();
            self.position_open = false;
            Ok(())
        }

        async fn on_market_data(
            &mut self,
            data: &MarketData,
        ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
            let price = match &data.data {
                MarketDataType::Kline(k) => k.close,
                _ => return Ok(vec![]),
            };

            self.prices.push(price);

            let short_sma = match self.calculate_sma(self.short_period) {
                Some(sma) => sma,
                None => return Ok(vec![]),
            };

            let long_sma = match self.calculate_sma(self.long_period) {
                Some(sma) => sma,
                None => return Ok(vec![]),
            };

            let mut signals = vec![];

            // 골든 크로스 (단기 > 장기)
            if short_sma > long_sma && !self.position_open {
                signals.push(Signal::entry("SimpleSMA", data.symbol.clone(), Side::Buy));
                self.position_open = true;
            }
            // 데드 크로스 (단기 < 장기)
            else if short_sma < long_sma && self.position_open {
                signals.push(Signal::exit("SimpleSMA", data.symbol.clone(), Side::Sell));
                self.position_open = false;
            }

            Ok(signals)
        }

        async fn on_order_filled(
            &mut self,
            _order: &Order,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        async fn on_position_update(
            &mut self,
            _position: &Position,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        fn get_state(&self) -> Value {
            serde_json::json!({
                "prices_count": self.prices.len(),
                "position_open": self.position_open
            })
        }
    }

    /// 항상 매수하는 전략 (테스트용)
    pub struct AlwaysBuyStrategy {
        bought: bool,
    }

    impl AlwaysBuyStrategy {
        pub fn new() -> Self {
            Self { bought: false }
        }
    }

    #[async_trait]
    impl trader_strategy::Strategy for AlwaysBuyStrategy {
        fn name(&self) -> &str {
            "AlwaysBuy"
        }

        fn version(&self) -> &str {
            "1.0.0"
        }

        fn description(&self) -> &str {
            "항상 매수하는 테스트 전략"
        }

        async fn initialize(
            &mut self,
            _config: Value,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.bought = false;
            Ok(())
        }

        async fn on_market_data(
            &mut self,
            data: &MarketData,
        ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
            if !self.bought {
                self.bought = true;
                Ok(vec![Signal::entry(
                    "AlwaysBuy",
                    data.symbol.clone(),
                    Side::Buy,
                )])
            } else {
                Ok(vec![])
            }
        }

        async fn on_order_filled(
            &mut self,
            _order: &Order,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        async fn on_position_update(
            &mut self,
            _position: &Position,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        fn get_state(&self) -> Value {
            serde_json::json!({ "bought": self.bought })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use rust_decimal_macros::dec;
    use trader_core::{Symbol, Timeframe};

    fn create_test_klines(count: usize, start_price: Decimal, trend: Decimal) -> Vec<Kline> {
        let symbol = Symbol::crypto("BTC", "USDT");
        let base_time = Utc::now() - Duration::days(count as i64);

        (0..count)
            .map(|i| {
                let price = start_price + trend * Decimal::from(i);
                let high = price * dec!(1.01);
                let low = price * dec!(0.99);
                let open_time = base_time + Duration::hours(i as i64);
                let close_time = open_time + Duration::hours(1);

                Kline::new(
                    symbol.clone(),
                    Timeframe::H1,
                    open_time,
                    price,
                    high,
                    low,
                    price,
                    dec!(100),
                    close_time,
                )
            })
            .collect()
    }

    #[test]
    fn test_config_creation() {
        let config = BacktestConfig::new(dec!(10000));
        assert_eq!(config.initial_capital, dec!(10000));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation() {
        let config = BacktestConfig::new(dec!(-1000));
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_engine_creation() {
        let config = BacktestConfig::new(dec!(10000));
        let engine = BacktestEngine::new(config);
        assert_eq!(engine.balance(), dec!(10000));
        assert_eq!(engine.open_positions_count(), 0);
    }

    #[tokio::test]
    async fn test_backtest_empty_data() {
        let config = BacktestConfig::new(dec!(10000));
        let mut engine = BacktestEngine::new(config);
        let mut strategy = test_strategies::AlwaysBuyStrategy::new();

        let result = engine.run(&mut strategy, &[]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_backtest_always_buy() {
        let config = BacktestConfig::new(dec!(100000))
            .with_commission_rate(dec!(0.001))
            .with_slippage_rate(dec!(0.0005));

        let mut engine = BacktestEngine::new(config);
        let mut strategy = test_strategies::AlwaysBuyStrategy::new();

        // 상승 추세 데이터
        let klines = create_test_klines(10, dec!(50000), dec!(100));

        let result = engine.run(&mut strategy, &klines).await;
        assert!(result.is_ok());

        let report = result.unwrap();
        assert_eq!(report.data_points, 10);
        assert!(report.total_commission > Decimal::ZERO);
    }

    #[tokio::test]
    async fn test_backtest_sma_strategy() {
        let config = BacktestConfig::new(dec!(1000000))
            .with_commission_rate(dec!(0.001))
            .with_slippage_rate(dec!(0.0));

        let mut engine = BacktestEngine::new(config);
        let mut strategy = test_strategies::SimpleSmaStrategy::new(5, 20);

        // 상승 후 하락 데이터
        let mut klines = create_test_klines(30, dec!(50000), dec!(100));
        klines.extend(create_test_klines(30, dec!(53000), dec!(-100)));

        // 시간 조정
        let base_time = Utc::now() - Duration::days(60);
        for (i, k) in klines.iter_mut().enumerate() {
            k.open_time = base_time + Duration::hours(i as i64);
            k.close_time = k.open_time + Duration::hours(1);
        }

        let result = engine.run(&mut strategy, &klines).await;
        assert!(result.is_ok());

        let report = result.unwrap();
        assert_eq!(report.data_points, 60);
        println!("{}", report.summary());
    }

    #[tokio::test]
    async fn test_backtest_report() {
        let config = BacktestConfig::new(dec!(100000));
        let mut engine = BacktestEngine::new(config);
        let mut strategy = test_strategies::AlwaysBuyStrategy::new();

        let klines = create_test_klines(20, dec!(50000), dec!(50));

        let result = engine.run(&mut strategy, &klines).await.unwrap();

        // 리포트 확인
        assert!(!result.equity_curve.is_empty());
        assert!(!result.summary().is_empty());
    }

    #[test]
    fn test_default_config() {
        let config = BacktestConfig::default();
        assert_eq!(config.initial_capital, dec!(10000000));
        assert!(config.validate().is_ok());
    }
}
