//! 거래소 정보 제공자 추상화.
//!
//! 다양한 거래소로부터 계좌 정보, 포지션, 미체결 주문을 조회하기 위한
//! 거래소 중립적인 인터페이스를 제공합니다.

use async_trait::async_trait;
use thiserror::Error;

use super::{PendingOrder, StrategyAccountInfo, StrategyPositionInfo};

// =============================================================================
// 에러 타입
// =============================================================================

/// ExchangeProvider 에러.
#[derive(Debug, Error)]
pub enum ProviderError {
    /// 네트워크 에러
    #[error("네트워크 에러: {0}")]
    Network(String),

    /// 인증 실패
    #[error("인증 실패: {0}")]
    Authentication(String),

    /// API 에러
    #[error("API 에러: {0}")]
    Api(String),

    /// 파싱 에러
    #[error("파싱 에러: {0}")]
    Parse(String),

    /// 지원하지 않는 기능
    #[error("지원하지 않는 기능: {0}")]
    Unsupported(String),

    /// 기타 에러
    #[error("기타 에러: {0}")]
    Other(String),
}

// =============================================================================
// ExchangeProvider Trait
// =============================================================================

/// 거래소 정보 제공자 trait.
///
/// 거래소로부터 실시간 계좌 정보, 포지션, 미체결 주문을 조회합니다.
/// 각 거래소별로 이 trait를 구현하여 거래소 중립적인 코드를 작성할 수 있습니다.
///
/// # 구현 예시
///
/// ```ignore
/// pub struct BinanceProvider {
///     client: Arc<BinanceClient>,
/// }
///
/// #[async_trait]
/// impl ExchangeProvider for BinanceProvider {
///     async fn fetch_account(&self) -> Result<AccountInfo, ProviderError> {
///         // Binance API 호출 및 변환
///     }
///
///     // ... 나머지 메서드 구현
/// }
/// ```
#[async_trait]
pub trait ExchangeProvider: Send + Sync {
    /// 계좌 정보 조회.
    ///
    /// 총 자산, 사용 가능 금액, 증거금, 미실현 손익 등을 조회합니다.
    ///
    /// # Errors
    ///
    /// - `ProviderError::Network`: 네트워크 연결 실패
    /// - `ProviderError::Authentication`: 인증 실패 (API 키 오류 등)
    /// - `ProviderError::Api`: 거래소 API 에러
    async fn fetch_account(&self) -> Result<StrategyAccountInfo, ProviderError>;

    /// 현재 보유 포지션 조회.
    ///
    /// 모든 보유 포지션의 상세 정보를 조회합니다.
    /// 현물 거래소의 경우 보유 자산을 포지션으로 변환합니다.
    ///
    /// # Returns
    ///
    /// 포지션 목록. 포지션이 없으면 빈 벡터 반환.
    ///
    /// # Errors
    ///
    /// - `ProviderError::Network`: 네트워크 연결 실패
    /// - `ProviderError::Authentication`: 인증 실패
    /// - `ProviderError::Api`: 거래소 API 에러
    async fn fetch_positions(&self) -> Result<Vec<StrategyPositionInfo>, ProviderError>;

    /// 미체결 주문 조회.
    ///
    /// 현재 대기 중이거나 부분 체결된 주문 목록을 조회합니다.
    ///
    /// # Returns
    ///
    /// 미체결 주문 목록. 미체결 주문이 없으면 빈 벡터 반환.
    ///
    /// # Errors
    ///
    /// - `ProviderError::Network`: 네트워크 연결 실패
    /// - `ProviderError::Authentication`: 인증 실패
    /// - `ProviderError::Api`: 거래소 API 에러
    async fn fetch_pending_orders(&self) -> Result<Vec<PendingOrder>, ProviderError>;

    /// 거래소 이름 반환.
    ///
    /// 로깅 및 디버깅 목적으로 사용됩니다.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let provider = BinanceProvider::new(client);
    /// assert_eq!(provider.exchange_name(), "Binance");
    /// ```
    fn exchange_name(&self) -> &str;
}

// =============================================================================
// 테스트
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::order::Side;
    use crate::types::{MarketType, Symbol};
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    /// 테스트용 MockProvider.
    struct MockProvider {
        name: String,
        should_fail: bool,
    }

    #[async_trait]
    impl ExchangeProvider for MockProvider {
        async fn fetch_account(&self) -> Result<StrategyAccountInfo, ProviderError> {
            if self.should_fail {
                return Err(ProviderError::Network("Mock network error".to_string()));
            }
            Ok(StrategyAccountInfo {
                total_balance: dec!(10000),
                available_balance: dec!(5000),
                margin_used: Decimal::ZERO,
                unrealized_pnl: dec!(100),
                currency: "USD".to_string(),
            })
        }

        async fn fetch_positions(&self) -> Result<Vec<StrategyPositionInfo>, ProviderError> {
            if self.should_fail {
                return Err(ProviderError::Api("Mock API error".to_string()));
            }
            let symbol = Symbol::new("BTC", "USDT", MarketType::Crypto);
            let pos = StrategyPositionInfo::new(symbol, Side::Buy, dec!(0.5), dec!(50000));
            Ok(vec![pos])
        }

        async fn fetch_pending_orders(&self) -> Result<Vec<PendingOrder>, ProviderError> {
            if self.should_fail {
                return Err(ProviderError::Authentication("Mock auth error".to_string()));
            }
            Ok(vec![])
        }

        fn exchange_name(&self) -> &str {
            &self.name
        }
    }

    #[tokio::test]
    async fn test_mock_provider_success() {
        let provider = MockProvider {
            name: "MockExchange".to_string(),
            should_fail: false,
        };

        // exchange_name 테스트
        assert_eq!(provider.exchange_name(), "MockExchange");

        // fetch_account 테스트
        let account = provider.fetch_account().await.unwrap();
        assert_eq!(account.total_balance, dec!(10000));
        assert_eq!(account.currency, "USD");

        // fetch_positions 테스트
        let positions = provider.fetch_positions().await.unwrap();
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].symbol.base, "BTC");

        // fetch_pending_orders 테스트
        let orders = provider.fetch_pending_orders().await.unwrap();
        assert_eq!(orders.len(), 0);
    }

    #[tokio::test]
    async fn test_mock_provider_errors() {
        let provider = MockProvider {
            name: "MockExchange".to_string(),
            should_fail: true,
        };

        // Network error
        let result = provider.fetch_account().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProviderError::Network(_)));

        // API error
        let result = provider.fetch_positions().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProviderError::Api(_)));

        // Authentication error
        let result = provider.fetch_pending_orders().await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProviderError::Authentication(_)
        ));
    }
}
