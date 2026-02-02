//! 한국투자증권 국내 주식 ExchangeProvider 구현.

use crate::connector::kis::client_kr::KisKrClient;
use async_trait::async_trait;
use chrono::TimeZone;
use rust_decimal::Decimal;
use std::sync::Arc;
use trader_core::domain::{
    ExchangeProvider, PendingOrder, ProviderError, Side, StrategyAccountInfo, StrategyPositionInfo,
};
use trader_core::types::{MarketType, Symbol};

/// KIS 날짜/시간 파싱 (YYYYMMDD + HHMMSS → DateTime<Utc>).
fn parse_kis_datetime(date: &str, time: &str) -> Result<chrono::DateTime<chrono::Utc>, String> {
    // 시간이 비어있으면 현재 시각 반환
    if time.is_empty() {
        return Ok(chrono::Utc::now());
    }

    let datetime_str = format!("{} {}", date, time);
    let naive = chrono::NaiveDateTime::parse_from_str(&datetime_str, "%Y%m%d %H%M%S")
        .map_err(|e| format!("날짜 파싱 실패: {}", e))?;

    // KST → UTC 변환
    let kst_offset = chrono::FixedOffset::east_opt(9 * 3600).ok_or("KST offset 생성 실패")?;
    let kst_datetime = kst_offset
        .from_local_datetime(&naive)
        .single()
        .ok_or("KST datetime 변환 실패")?;

    Ok(kst_datetime.with_timezone(&chrono::Utc))
}

/// 한국투자증권 국내 주식 ExchangeProvider 구현.
///
/// KisKrClient를 래핑하여 거래소 중립적인 ExchangeProvider 인터페이스를 제공합니다.
pub struct KisKrProvider {
    client: Arc<KisKrClient>,
}

impl KisKrProvider {
    /// 새 Provider 생성 (Arc로 래핑).
    pub fn new(client: Arc<KisKrClient>) -> Self {
        Self { client }
    }

    /// KisKrClient로부터 직접 생성 (Arc로 자동 래핑).
    pub fn from_client(client: KisKrClient) -> Self {
        Self::new(Arc::new(client))
    }
}

#[async_trait]
impl ExchangeProvider for KisKrProvider {
    async fn fetch_account(&self) -> Result<StrategyAccountInfo, ProviderError> {
        // 잔고 조회 (보유 종목 + 계좌 요약)
        let balance = self
            .client
            .get_balance()
            .await
            .map_err(|e| ProviderError::Api(format!("KIS 잔고 조회 실패: {}", e)))?;

        // 계좌 요약 정보가 없으면 기본값 사용
        let summary = balance
            .summary
            .ok_or_else(|| ProviderError::Parse("KIS 계좌 요약 정보 없음".to_string()))?;

        // 보유 종목 평가액 합계
        let total_position_value: Decimal = balance.holdings.iter().map(|h| h.eval_amount).sum();

        // 총 자산 = 예수금 + 보유 종목 평가액
        let total_balance = summary.cash_balance + total_position_value;

        // 미실현 손익 = 보유 종목 평가손익 합계
        let unrealized_pnl: Decimal = balance.holdings.iter().map(|h| h.profit_loss).sum();

        Ok(StrategyAccountInfo {
            total_balance,
            available_balance: summary.cash_balance,
            margin_used: Decimal::ZERO, // 현물 거래는 증거금 없음
            unrealized_pnl,
            currency: "KRW".to_string(),
        })
    }

    async fn fetch_positions(&self) -> Result<Vec<StrategyPositionInfo>, ProviderError> {
        // 잔고 조회
        let balance = self
            .client
            .get_balance()
            .await
            .map_err(|e| ProviderError::Api(format!("KIS 잔고 조회 실패: {}", e)))?;

        let mut positions = Vec::new();

        for holding in balance.holdings {
            // 보유 수량이 없으면 스킵
            if holding.quantity <= Decimal::ZERO {
                continue;
            }

            // 심볼 생성 (KRX 국내 주식)
            let symbol = Symbol::new(&holding.stock_code, "KRW", MarketType::KrStock);

            // 포지션 생성 (현물은 매수만 가능)
            let mut position =
                StrategyPositionInfo::new(symbol, Side::Buy, holding.quantity, holding.avg_price);

            // 현재가 및 손익 업데이트
            position.update_price(holding.current_price);

            positions.push(position);
        }

        Ok(positions)
    }

    async fn fetch_pending_orders(&self) -> Result<Vec<PendingOrder>, ProviderError> {
        // KIS API로 당일 미체결 주문 조회
        let pending_orders = self
            .client
            .get_pending_orders()
            .await
            .map_err(|e| ProviderError::Api(format!("KIS 미체결 주문 조회 실패: {}", e)))?;

        let mut result = Vec::new();

        for order in pending_orders {
            // 매수/매도 구분 변환 (01=매도, 02=매수)
            let side = match order.side_code.as_str() {
                "01" => Side::Sell,
                "02" => Side::Buy,
                _ => continue, // 알 수 없는 side는 스킵
            };

            // 심볼 생성
            let symbol = Symbol::new(&order.stock_code, "KRW", MarketType::KrStock);

            // 주문 상태 결정
            let status = if order.filled_qty > Decimal::ZERO {
                trader_core::OrderStatusType::PartiallyFilled
            } else {
                trader_core::OrderStatusType::Open
            };

            // 주문 시각 파싱 (YYYYMMDD + HHMMSS -> DateTime)
            let created_at = parse_kis_datetime(&order.order_date, &order.order_time)
                .unwrap_or_else(|_| chrono::Utc::now());

            let pending = PendingOrder {
                order_id: order.order_no,
                symbol,
                side,
                price: order.order_price,
                quantity: order.order_qty,
                filled_quantity: order.filled_qty,
                status,
                created_at,
            };

            result.push(pending);
        }

        Ok(result)
    }

    fn exchange_name(&self) -> &str {
        "KIS-KR"
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_kis_kr_provider_creation() {
        // KisKrClient는 실제 API 키가 필요하므로 구조만 테스트
        // 통합 테스트는 mock을 사용
    }
}
