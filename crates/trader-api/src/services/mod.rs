//! 백그라운드 서비스 모듈.
//!
//! 전략 실행, 컨텍스트 동기화 등 백그라운드에서 실행되는 서비스들을 제공합니다.

pub mod context_sync;
pub mod signal_alert;

pub use context_sync::start_context_sync_service;
pub use signal_alert::{SignalAlertFilter, SignalAlertService};
