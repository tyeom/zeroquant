//! 백테스팅 모듈
//!
//! 과거 데이터로 트레이딩 전략을 시뮬레이션하고 성과를 분석합니다.
//!
//! # 주요 구성요소
//!
//! - [`BacktestConfig`]: 백테스트 설정 (초기 자본, 수수료, 슬리피지 등)
//! - [`BacktestEngine`]: 백테스트 실행 엔진
//! - [`BacktestReport`]: 백테스트 결과 리포트

pub mod engine;

pub use engine::{BacktestConfig, BacktestEngine, BacktestError, BacktestReport, BacktestResult};
