//! 전략을 위한 공통 유틸리티 및 계산기.
//!
//! 이 모듈은 트레이딩 전략 구축을 위한 재사용 가능한 컴포넌트를 제공합니다:
//!
//! - **모멘텀**: 자산 배분 전략을 위한 다기간 모멘텀 스코어링
//! - **리밸런싱**: 포트폴리오 리밸런싱 계산
//! - **serde_helpers**: SDUI와 전략 설정 간 타입 변환
//! - **시그널**: 공통 시그널 생성 패턴 (추후 지원 예정)

pub mod momentum;
pub mod rebalance;
pub mod serde_helpers;

pub use momentum::{
    MomentumCalculator, MomentumConfig, MomentumResult, MomentumScore,
    WeightedMomentumConfig,
};

pub use rebalance::{
    RebalanceCalculator, RebalanceConfig, RebalanceOrder, RebalanceOrderSide,
    RebalanceResult, PortfolioPosition, TargetAllocation,
};

pub use serde_helpers::{deserialize_symbol, deserialize_symbol_opt};
