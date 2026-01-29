//! Strategy trait 정의.

use async_trait::async_trait;
use serde_json::Value;
use trader_core::{MarketData, Order, Position, Signal};

/// 트레이딩 전략 구현을 위한 Strategy trait.
///
/// 모든 전략은 전략 엔진에서 로드되기 위해 이 trait를 구현해야 합니다.
#[async_trait]
pub trait Strategy: Send + Sync {
    /// 전략 이름 반환.
    fn name(&self) -> &str;

    /// 전략 버전 반환.
    fn version(&self) -> &str;

    /// 전략 설명 반환.
    fn description(&self) -> &str;

    /// 설정으로 전략 초기화.
    async fn initialize(&mut self, config: Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// 새 시장 데이터 수신 시 호출.
    /// 트레이딩 신호가 있으면 반환.
    async fn on_market_data(
        &mut self,
        data: &MarketData,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>>;

    /// 주문 체결 시 호출.
    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// 포지션 업데이트 시 호출.
    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// 전략 종료 및 리소스 정리.
    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// 현재 전략 상태를 JSON으로 반환 (디버깅/모니터링용).
    fn get_state(&self) -> Value;

    /// 영속성을 위해 전략 상태 저장.
    fn save_state(&self) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![])
    }

    /// 영속성에서 전략 상태 로드.
    fn load_state(&mut self, _data: &[u8]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

/// 등록을 위한 전략 메타데이터.
#[derive(Debug, Clone)]
pub struct StrategyMetadata {
    /// 전략 이름
    pub name: String,
    /// 전략 버전
    pub version: String,
    /// 전략 설명
    pub description: String,
    /// 필수 설정 키
    pub required_config: Vec<String>,
    /// 지원 심볼 (빈 값 = 전체)
    pub supported_symbols: Vec<String>,
}
