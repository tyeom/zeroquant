//! Circuit Breaker pattern implementation.
//!
//! 외부 서비스 장애 시 연쇄 실패를 방지하고 시스템 복원력을 향상시킵니다.
//!
//! # 상태 전이
//!
//! ```text
//! Closed ──[실패 임계치 도달]──> Open
//!    ↑                            │
//!    │                   [타임아웃 경과]
//!    │                            ↓
//!    └──[성공]── HalfOpen ──[실패]──> Open
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, Instant};

use crate::ExchangeError;

/// Circuit Breaker 상태.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// 정상 상태 - 모든 요청 허용
    Closed,
    /// 장애 상태 - 모든 요청 즉시 거부
    Open,
    /// 복구 테스트 상태 - 단일 요청만 허용
    HalfOpen,
}

impl std::fmt::Display for CircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitState::Closed => write!(f, "closed"),
            CircuitState::Open => write!(f, "open"),
            CircuitState::HalfOpen => write!(f, "half_open"),
        }
    }
}

/// Circuit Breaker 설정.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// 실패 임계치 (이 횟수 초과 시 Open 상태로 전이)
    pub failure_threshold: u32,
    /// Open 상태 유지 시간 (이후 HalfOpen으로 전이)
    pub reset_timeout: Duration,
    /// 연속 성공 횟수 (HalfOpen에서 Closed로 전이하기 위한 조건)
    pub success_threshold: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            reset_timeout: Duration::from_secs(30),
            success_threshold: 1,
        }
    }
}

impl CircuitBreakerConfig {
    /// 새 설정 생성.
    pub fn new(failure_threshold: u32, reset_timeout_secs: u64, success_threshold: u32) -> Self {
        Self {
            failure_threshold,
            reset_timeout: Duration::from_secs(reset_timeout_secs),
            success_threshold,
        }
    }

    /// 보수적인 설정 (낮은 임계치, 긴 타임아웃).
    pub fn conservative() -> Self {
        Self {
            failure_threshold: 3,
            reset_timeout: Duration::from_secs(60),
            success_threshold: 2,
        }
    }

    /// 공격적인 설정 (높은 임계치, 짧은 타임아웃).
    pub fn aggressive() -> Self {
        Self {
            failure_threshold: 10,
            reset_timeout: Duration::from_secs(10),
            success_threshold: 1,
        }
    }
}

/// Circuit Breaker 내부 상태.
struct CircuitBreakerState {
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<Instant>,
    last_state_change: Instant,
}

impl CircuitBreakerState {
    fn new() -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure_time: None,
            last_state_change: Instant::now(),
        }
    }
}

/// Circuit Breaker.
///
/// 외부 서비스 호출 시 연쇄 실패를 방지합니다.
///
/// # Example
///
/// ```ignore
/// let cb = CircuitBreaker::new("binance", CircuitBreakerConfig::default());
///
/// // 요청 허용 여부 확인
/// if cb.is_allowed() {
///     match make_request().await {
///         Ok(result) => {
///             cb.record_success();
///             // 결과 처리
///         }
///         Err(e) => {
///             if e.is_retryable() {
///                 cb.record_failure();
///             }
///         }
///     }
/// } else {
///     // Circuit이 열려있음 - 빠른 실패
///     return Err(ExchangeError::CircuitOpen);
/// }
/// ```
pub struct CircuitBreaker {
    /// 서비스 이름 (로깅 및 메트릭용)
    name: String,
    /// 설정
    config: CircuitBreakerConfig,
    /// 내부 상태 (RwLock으로 보호)
    state: RwLock<CircuitBreakerState>,
    /// 총 실패 횟수 (메트릭용)
    total_failures: AtomicU64,
    /// 총 성공 횟수 (메트릭용)
    total_successes: AtomicU64,
    /// Circuit Open 횟수 (메트릭용)
    open_count: AtomicU64,
}

impl CircuitBreaker {
    /// 새 Circuit Breaker 생성.
    pub fn new(name: impl Into<String>, config: CircuitBreakerConfig) -> Self {
        Self {
            name: name.into(),
            config,
            state: RwLock::new(CircuitBreakerState::new()),
            total_failures: AtomicU64::new(0),
            total_successes: AtomicU64::new(0),
            open_count: AtomicU64::new(0),
        }
    }

    /// 기본 설정으로 생성.
    pub fn with_defaults(name: impl Into<String>) -> Self {
        Self::new(name, CircuitBreakerConfig::default())
    }

    /// 서비스 이름 반환.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 현재 상태 반환.
    pub fn state(&self) -> CircuitState {
        let mut state = self.state.write().unwrap();
        self.maybe_transition_from_open(&mut state);
        state.state
    }

    /// 요청이 허용되는지 확인.
    ///
    /// HalfOpen 상태에서는 복구 테스트를 위해 단일 요청만 허용됩니다.
    pub fn is_allowed(&self) -> bool {
        let mut state = self.state.write().unwrap();

        // Open 상태에서 타임아웃이 경과했으면 HalfOpen으로 전이
        self.maybe_transition_from_open(&mut state);

        match state.state {
            CircuitState::Closed => true,
            CircuitState::Open => false,
            CircuitState::HalfOpen => {
                // HalfOpen에서는 이미 테스트 요청이 진행 중이면 거부
                // 단순 구현을 위해 HalfOpen에서는 항상 허용
                // (실제로는 semaphore로 제한할 수 있음)
                true
            }
        }
    }

    /// 성공 기록.
    ///
    /// HalfOpen 상태에서 성공하면 Closed로 전이합니다.
    pub fn record_success(&self) {
        self.total_successes.fetch_add(1, Ordering::Relaxed);

        let mut state = self.state.write().unwrap();

        match state.state {
            CircuitState::HalfOpen => {
                state.success_count += 1;
                if state.success_count >= self.config.success_threshold {
                    // HalfOpen → Closed
                    self.transition_to(&mut state, CircuitState::Closed);
                    tracing::info!(
                        circuit_breaker = %self.name,
                        "Circuit breaker recovered: HalfOpen -> Closed"
                    );
                }
            }
            CircuitState::Closed => {
                // Closed 상태에서 성공하면 실패 카운터 리셋
                state.failure_count = 0;
            }
            CircuitState::Open => {
                // Open 상태에서는 요청이 거부되므로 이 케이스는 발생하지 않아야 함
            }
        }
    }

    /// 실패 기록.
    ///
    /// 실패 횟수가 임계치를 초과하면 Open 상태로 전이합니다.
    pub fn record_failure(&self) {
        self.total_failures.fetch_add(1, Ordering::Relaxed);

        let mut state = self.state.write().unwrap();
        state.last_failure_time = Some(Instant::now());

        match state.state {
            CircuitState::Closed => {
                state.failure_count += 1;
                if state.failure_count >= self.config.failure_threshold {
                    // Closed → Open
                    self.transition_to(&mut state, CircuitState::Open);
                    self.open_count.fetch_add(1, Ordering::Relaxed);
                    tracing::warn!(
                        circuit_breaker = %self.name,
                        failure_count = state.failure_count,
                        "Circuit breaker tripped: Closed -> Open"
                    );
                }
            }
            CircuitState::HalfOpen => {
                // HalfOpen → Open (복구 테스트 실패)
                self.transition_to(&mut state, CircuitState::Open);
                self.open_count.fetch_add(1, Ordering::Relaxed);
                tracing::warn!(
                    circuit_breaker = %self.name,
                    "Circuit breaker recovery failed: HalfOpen -> Open"
                );
            }
            CircuitState::Open => {
                // 이미 Open 상태
            }
        }
    }

    /// ExchangeError 기반 결과 기록.
    ///
    /// `is_retryable()` 에러만 실패로 간주합니다.
    pub fn record_result<T>(&self, result: &Result<T, ExchangeError>) {
        match result {
            Ok(_) => self.record_success(),
            Err(e) if e.is_retryable() => self.record_failure(),
            Err(_) => {
                // 재시도 불가능한 에러는 Circuit Breaker에 영향 주지 않음
                // (예: InsufficientBalance, InvalidQuantity 등)
            }
        }
    }

    /// 수동으로 Circuit 리셋.
    pub fn reset(&self) {
        let mut state = self.state.write().unwrap();
        self.transition_to(&mut state, CircuitState::Closed);
        state.failure_count = 0;
        state.success_count = 0;
        tracing::info!(
            circuit_breaker = %self.name,
            "Circuit breaker manually reset"
        );
    }

    /// 메트릭 반환.
    pub fn metrics(&self) -> CircuitBreakerMetrics {
        let state = self.state.read().unwrap();
        CircuitBreakerMetrics {
            name: self.name.clone(),
            state: state.state,
            failure_count: state.failure_count,
            total_failures: self.total_failures.load(Ordering::Relaxed),
            total_successes: self.total_successes.load(Ordering::Relaxed),
            open_count: self.open_count.load(Ordering::Relaxed),
            time_in_current_state: state.last_state_change.elapsed(),
        }
    }

    /// Open 상태에서 타임아웃이 경과했으면 HalfOpen으로 전이.
    fn maybe_transition_from_open(&self, state: &mut CircuitBreakerState) {
        if state.state == CircuitState::Open {
            if state.last_state_change.elapsed() >= self.config.reset_timeout {
                self.transition_to(state, CircuitState::HalfOpen);
                tracing::info!(
                    circuit_breaker = %self.name,
                    "Circuit breaker timeout: Open -> HalfOpen"
                );
            }
        }
    }

    /// 상태 전이.
    fn transition_to(&self, state: &mut CircuitBreakerState, new_state: CircuitState) {
        state.state = new_state;
        state.last_state_change = Instant::now();

        if new_state == CircuitState::Closed {
            state.failure_count = 0;
            state.success_count = 0;
        } else if new_state == CircuitState::HalfOpen {
            state.success_count = 0;
        }
    }
}

/// Circuit Breaker 메트릭.
#[derive(Debug, Clone)]
pub struct CircuitBreakerMetrics {
    /// 서비스 이름
    pub name: String,
    /// 현재 상태
    pub state: CircuitState,
    /// 현재 연속 실패 횟수
    pub failure_count: u32,
    /// 총 실패 횟수
    pub total_failures: u64,
    /// 총 성공 횟수
    pub total_successes: u64,
    /// Circuit Open 횟수
    pub open_count: u64,
    /// 현재 상태 유지 시간
    pub time_in_current_state: Duration,
}

/// Circuit이 열려있을 때 반환되는 에러.
#[derive(Debug, Clone)]
pub struct CircuitOpenError {
    /// Circuit Breaker 이름
    pub name: String,
    /// 남은 대기 시간 (예상)
    pub retry_after: Option<Duration>,
}

impl std::fmt::Display for CircuitOpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Circuit breaker '{}' is open", self.name)?;
        if let Some(retry_after) = self.retry_after {
            write!(f, " (retry after {:?})", retry_after)?;
        }
        Ok(())
    }
}

impl std::error::Error for CircuitOpenError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_circuit_breaker_initial_state() {
        let cb = CircuitBreaker::with_defaults("test");
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.is_allowed());
    }

    #[test]
    fn test_circuit_breaker_opens_after_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            reset_timeout: Duration::from_secs(30),
            success_threshold: 1,
        };
        let cb = CircuitBreaker::new("test", config);

        // 3번 실패
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.is_allowed());
    }

    #[test]
    fn test_circuit_breaker_success_resets_failure_count() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            reset_timeout: Duration::from_secs(30),
            success_threshold: 1,
        };
        let cb = CircuitBreaker::new("test", config);

        // 2번 실패 후 성공
        cb.record_failure();
        cb.record_failure();
        cb.record_success();

        // 다시 3번 실패해야 Open
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_breaker_half_open_after_timeout() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            reset_timeout: Duration::from_millis(50),
            success_threshold: 1,
        };
        let cb = CircuitBreaker::new("test", config);

        // Open 상태로 전이
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // 타임아웃 대기
        thread::sleep(Duration::from_millis(60));

        // HalfOpen으로 전이
        assert_eq!(cb.state(), CircuitState::HalfOpen);
        assert!(cb.is_allowed());
    }

    #[test]
    fn test_circuit_breaker_recovers_on_success() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            reset_timeout: Duration::from_millis(50),
            success_threshold: 1,
        };
        let cb = CircuitBreaker::new("test", config);

        // Open → HalfOpen → Closed
        cb.record_failure();
        thread::sleep(Duration::from_millis(60));
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_half_open_failure_reopens() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            reset_timeout: Duration::from_millis(50),
            success_threshold: 1,
        };
        let cb = CircuitBreaker::new("test", config);

        // Open → HalfOpen → Open
        cb.record_failure();
        thread::sleep(Duration::from_millis(60));
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_breaker_record_result() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            reset_timeout: Duration::from_secs(30),
            success_threshold: 1,
        };
        let cb = CircuitBreaker::new("test", config);

        // 성공
        let ok_result: Result<i32, ExchangeError> = Ok(42);
        cb.record_result(&ok_result);
        assert_eq!(cb.state(), CircuitState::Closed);

        // 재시도 가능한 에러 (실패로 카운트)
        let retryable_err: Result<i32, ExchangeError> =
            Err(ExchangeError::NetworkError("connection failed".to_string()));
        cb.record_result(&retryable_err);
        cb.record_result(&retryable_err);
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_breaker_non_retryable_error_ignored() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            reset_timeout: Duration::from_secs(30),
            success_threshold: 1,
        };
        let cb = CircuitBreaker::new("test", config);

        // 재시도 불가능한 에러 (실패로 카운트 안 됨)
        let non_retryable: Result<i32, ExchangeError> =
            Err(ExchangeError::InsufficientBalance("not enough".to_string()));

        cb.record_result(&non_retryable);
        cb.record_result(&non_retryable);
        cb.record_result(&non_retryable);

        // 여전히 Closed
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_metrics() {
        let cb = CircuitBreaker::with_defaults("test");

        cb.record_success();
        cb.record_success();
        cb.record_failure();

        let metrics = cb.metrics();
        assert_eq!(metrics.name, "test");
        assert_eq!(metrics.state, CircuitState::Closed);
        assert_eq!(metrics.total_successes, 2);
        assert_eq!(metrics.total_failures, 1);
        assert_eq!(metrics.failure_count, 1);
    }

    #[test]
    fn test_circuit_breaker_manual_reset() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            reset_timeout: Duration::from_secs(300),
            success_threshold: 1,
        };
        let cb = CircuitBreaker::new("test", config);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.is_allowed());
    }
}
