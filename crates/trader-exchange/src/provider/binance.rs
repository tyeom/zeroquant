//! Binance ExchangeProvider 구현.

use crate::connector::binance::BinanceClient;
use crate::traits::Exchange;
use async_trait::async_trait;
use rust_decimal::Decimal;
use std::sync::Arc;
use trader_core::domain::{
    ExchangeProvider, PendingOrder, ProviderError, Side, StrategyAccountInfo, StrategyPositionInfo,
};
use trader_core::types::{MarketType, Symbol};

/// Binance ExchangeProvider 구현.
///
/// BinanceClient를 래핑하여 거래소 중립적인 ExchangeProvider 인터페이스를 제공합니다.
pub struct BinanceProvider {
    client: Arc<BinanceClient>,
}

impl BinanceProvider {
    /// 새 BinanceProvider 생성.
    pub fn new(client: Arc<BinanceClient>) -> Self {
        Self { client }
    }

    /// BinanceClient에서 생성.
    pub fn from_client(client: BinanceClient) -> Self {
        Self::new(Arc::new(client))
    }
}

#[async_trait]
impl ExchangeProvider for BinanceProvider {
    async fn fetch_account(&self) -> Result<StrategyAccountInfo, ProviderError> {
        // Exchange trait의 get_account 호출
        let account_info = self
            .client
            .get_account()
            .await
            .map_err(|e| ProviderError::Api(format!("Binance get_account failed: {}", e)))?;

        // 총 자산 계산 (USDT 기준)
        let mut total_balance = Decimal::ZERO;
        let mut available_balance = Decimal::ZERO;

        for balance in &account_info.balances {
            // USDT 또는 주요 통화만 계산 (간소화)
            if balance.asset == "USDT" {
                total_balance += balance.free + balance.locked;
                available_balance += balance.free;
            }
        }

        // Binance Spot은 증거금이 없음
        Ok(StrategyAccountInfo {
            total_balance,
            available_balance,
            margin_used: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO, // 현물은 미실현 손익 없음
            currency: "USDT".to_string(),
        })
    }

    async fn fetch_positions(&self) -> Result<Vec<StrategyPositionInfo>, ProviderError> {
        // Binance Spot은 포지션 개념이 없음, 보유 자산을 포지션으로 변환
        let account_info = self
            .client
            .get_account()
            .await
            .map_err(|e| ProviderError::Api(format!("Binance get_account failed: {}", e)))?;

        let mut positions = Vec::new();

        for balance in account_info.balances {
            // 잔고가 있는 자산만 포지션으로 변환
            if balance.free > Decimal::ZERO || balance.locked > Decimal::ZERO {
                let total_qty = balance.free + balance.locked;

                // USDT는 스킵 (기준 통화)
                if balance.asset == "USDT" {
                    continue;
                }

                // 심볼 생성 (예: BTC -> BTC/USDT)
                let symbol = Symbol::new(&balance.asset, "USDT", MarketType::Crypto);

                // 현재가 조회
                let ticker = self.client.get_ticker(&symbol).await.map_err(|e| {
                    ProviderError::Api(format!("Failed to get ticker for {}: {}", balance.asset, e))
                })?;

                // 포지션 생성 (현물은 매수만 가능)
                let mut position = StrategyPositionInfo::new(
                    symbol,
                    Side::Buy,
                    total_qty,
                    ticker.last, // 진입가는 현재가로 근사 (실제로는 평균 매수가 필요)
                );

                // 현재가 업데이트
                position.update_price(ticker.last);

                positions.push(position);
            }
        }

        Ok(positions)
    }

    async fn fetch_pending_orders(&self) -> Result<Vec<PendingOrder>, ProviderError> {
        // 모든 심볼의 미체결 주문 조회
        let orders =
            self.client.get_open_orders(None).await.map_err(|e| {
                ProviderError::Api(format!("Binance get_open_orders failed: {}", e))
            })?;

        let mut pending_orders = Vec::new();

        for order in orders {
            // 필수 정보가 없는 경우 스킵
            let Some(symbol) = order.symbol else {
                continue;
            };
            let Some(side) = order.side else {
                continue;
            };
            let Some(quantity) = order.quantity else {
                continue;
            };
            let Some(price) = order.price else {
                continue;
            };

            let pending = PendingOrder {
                order_id: order.order_id,
                symbol,
                side,
                price,
                quantity,
                filled_quantity: order.filled_quantity,
                status: order.status,
                created_at: order.updated_at,
            };

            pending_orders.push(pending);
        }

        Ok(pending_orders)
    }

    fn exchange_name(&self) -> &str {
        "Binance"
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_binance_provider_creation() {
        // BinanceClient는 실제 API 키가 필요하므로 구조만 테스트
        // 통합 테스트는 mock을 사용
    }
}
