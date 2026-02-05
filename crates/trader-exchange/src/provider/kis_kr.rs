//! 한국투자증권 국내 주식 ExchangeProvider 구현.

use crate::connector::kis::client_kr::KisKrClient;
use crate::connector::kis::config::KisAccountType;
use async_trait::async_trait;
use chrono::TimeZone;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};
use trader_core::domain::{
    ExchangeProvider, ExecutionHistoryRequest, ExecutionHistoryResponse, PendingOrder,
    ProviderError, Side, StrategyAccountInfo, StrategyPositionInfo, Trade,
};
use trader_core::types::{MarketType, Symbol};
use uuid::Uuid;

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

    /// ISA 계좌 여부 확인.
    ///
    /// ISA 계좌는 잔고 조회 API가 제한되어 체결 내역 기반으로 조회해야 합니다.
    fn is_isa_account(&self) -> bool {
        self.client.oauth().config().account_type == KisAccountType::RealIsa
    }

    /// 모든 체결 내역을 페이지네이션으로 조회.
    ///
    /// KIS API는 한 번에 최대 1년, 100건만 반환하므로 년도별로 분할하고 연속 조회 키를 사용합니다.
    /// Rate limiting과 무한 루프 방지 로직 포함.
    async fn fetch_all_order_history(
        &self,
    ) -> Result<Vec<crate::connector::kis::client_kr::KrOrderExecution>, ProviderError> {
        use crate::connector::kis::client_kr::KrOrderExecution;
        use chrono::{Datelike, Duration, NaiveDate};

        let mut all_executions: Vec<KrOrderExecution> = Vec::new();
        const MAX_PAGES_PER_RANGE: usize = 50; // 날짜 범위당 최대 페이지
        const API_DELAY_MS: u64 = 500; // Rate limiting (안전 마진)

        // 1년 단위로 날짜 범위 생성 (최근 10년)
        let today = chrono::Utc::now().date_naive();
        let mut date_ranges: Vec<(String, String)> = Vec::new();

        for years_ago in 0..10 {
            let end = if years_ago == 0 {
                today
            } else {
                NaiveDate::from_ymd_opt(today.year() - years_ago, today.month(), today.day())
                    .unwrap_or(today - Duration::days(365 * years_ago as i64))
            };
            let start = end - Duration::days(364); // 1년 미만으로 설정

            date_ranges.push((
                start.format("%Y%m%d").to_string(),
                end.format("%Y%m%d").to_string(),
            ));
        }

        debug!("체결 내역 조회: {} 개 날짜 범위", date_ranges.len());

        // 각 날짜 범위에 대해 조회
        for (range_idx, (start_str, end_str)) in date_ranges.iter().enumerate() {
            debug!(
                "날짜 범위 {}/{}: {} ~ {}",
                range_idx + 1,
                date_ranges.len(),
                start_str,
                end_str
            );

            let mut ctx_fk = String::new();
            let mut ctx_nk = String::new();
            let mut prev_ctx_nk = String::new();
            let mut page = 0;

            loop {
                // Rate Limiting
                if page > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(API_DELAY_MS)).await;
                }
                page += 1;

                if page > MAX_PAGES_PER_RANGE {
                    warn!("날짜 범위 {}의 최대 페이지 수 도달", range_idx + 1);
                    break;
                }

                let history = match self
                    .client
                    .get_order_history(start_str, end_str, "00", &ctx_fk, &ctx_nk)
                    .await
                {
                    Ok(h) => h,
                    Err(e) => {
                        let error_msg = e.to_string();
                        // Rate Limit 에러 시 재시도
                        if error_msg.contains("초당")
                            || error_msg.contains("건수")
                            || error_msg.contains("exceeded")
                        {
                            warn!("Rate limit hit, waiting 2 seconds...");
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            match self
                                .client
                                .get_order_history(start_str, end_str, "00", &ctx_fk, &ctx_nk)
                                .await
                            {
                                Ok(h) => h,
                                Err(_) => break, // 재시도 실패 시 다음 범위로
                            }
                        } else {
                            // 다른 에러는 다음 범위로 진행
                            warn!(
                                "날짜 범위 {}/{} ({} ~ {}) 조회 실패, 다음 범위로: {}",
                                range_idx + 1,
                                date_ranges.len(),
                                start_str,
                                end_str,
                                e
                            );
                            break;
                        }
                    }
                };

                let count = history.executions.len();
                all_executions.extend(history.executions);

                // 데이터가 있으면 해당 범위에서 거래 기록 발견 (debug 레벨로 출력)
                if count > 0 {
                    debug!(
                        "날짜 범위 {}/{} ({} ~ {}), 페이지 {}: {} 건 발견",
                        range_idx + 1,
                        date_ranges.len(),
                        start_str,
                        end_str,
                        page,
                        count
                    );
                }

                // 종료 조건들
                if !history.has_more {
                    break;
                }
                if prev_ctx_nk == history.ctx_area_nk100 && !prev_ctx_nk.is_empty() {
                    break;
                }
                if history.ctx_area_nk100.is_empty() {
                    break;
                }

                prev_ctx_nk = ctx_nk.clone();
                ctx_fk = history.ctx_area_fk100;
                ctx_nk = history.ctx_area_nk100;
            }

            // 데이터가 발견되면 더 오래된 범위는 조회할 필요 없을 수 있음
            // (단, 분할 매수 등이 있을 수 있으므로 모두 조회)
        }

        info!("전체 체결 내역 조회 완료: 총 {} 건", all_executions.len());
        Ok(all_executions)
    }

    /// 체결 내역 기반으로 계좌 정보 조회 (get_balance() 미지원 시 사용).
    ///
    /// ISA 계좌 등 get_balance()가 지원되지 않는 경우를 위한 대체 방법입니다.
    /// 계좌 개설 이후 모든 체결 내역을 페이지네이션으로 조회하여 현재 포지션을 계산합니다.
    async fn fetch_account_from_order_history(&self) -> Result<StrategyAccountInfo, ProviderError> {
        debug!("체결 내역 기반으로 계좌 정보 조회");

        // 모든 체결 내역 조회 (페이지네이션)
        let all_executions = self.fetch_all_order_history().await?;

        // 종목별 포지션 계산: (매수수량, 매도수량, 총매수금액)
        let mut positions: HashMap<String, (Decimal, Decimal, Decimal)> = HashMap::new();

        for execution in &all_executions {
            // 체결 수량이 0이면 스킵
            if execution.filled_qty <= Decimal::ZERO {
                continue;
            }

            let entry = positions.entry(execution.stock_code.clone()).or_insert((
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
            ));

            match execution.side_code.as_str() {
                "02" => {
                    // 매수: 수량 누적 + 총 매수 금액 누적
                    entry.0 += execution.filled_qty;
                    entry.2 += execution.filled_qty * execution.avg_price;
                }
                "01" => {
                    // 매도: 수량 누적
                    entry.1 += execution.filled_qty;
                }
                _ => {}
            }
        }

        // 총 평가액 및 손익 계산
        let mut total_eval = Decimal::ZERO;
        let mut total_cost = Decimal::ZERO;
        let mut holdings_count = 0;

        for (code, (buy_qty, sell_qty, total_buy_amount)) in &positions {
            let current_qty = *buy_qty - *sell_qty;
            if current_qty <= Decimal::ZERO {
                continue;
            }

            holdings_count += 1;

            // 평균 매입가 계산
            let avg_price = if *buy_qty > Decimal::ZERO {
                *total_buy_amount / *buy_qty
            } else {
                Decimal::ZERO
            };

            // 현재가 조회
            let current_price = match self.client.get_price(code).await {
                Ok(price_data) => price_data.current_price,
                Err(e) => {
                    warn!("현재가 조회 실패 ({}): {}", code, e);
                    avg_price // 실패 시 매입가로 대체
                }
            };

            total_eval += current_qty * current_price;
            total_cost += current_qty * avg_price;
        }

        let unrealized_pnl = total_eval - total_cost;

        info!(
            "체결 내역 기반 계좌 조회 완료: 종목 수 = {}, 총 평가액 = {}, 미실현손익 = {}",
            holdings_count, total_eval, unrealized_pnl
        );

        // ISA 계좌는 현금 잔고를 정확히 알 수 없으므로 0으로 설정
        Ok(StrategyAccountInfo {
            total_balance: total_eval,
            available_balance: Decimal::ZERO, // ISA는 현금 잔고 불명
            margin_used: Decimal::ZERO,
            unrealized_pnl,
            currency: "KRW".to_string(),
        })
    }

    /// 체결 내역 기반으로 포지션 조회 (get_balance() 미지원 시 사용).
    ///
    /// ISA 계좌 등 잔고 조회 API가 제한된 경우 체결 내역을 기반으로
    /// 현재 보유 종목, 수량, 평균 매입가를 계산합니다.
    async fn fetch_positions_from_order_history(
        &self,
    ) -> Result<Vec<StrategyPositionInfo>, ProviderError> {
        debug!("체결 내역 기반으로 포지션 조회");

        // 모든 체결 내역 조회 (페이지네이션)
        let all_executions = self.fetch_all_order_history().await?;

        // 종목별 포지션 계산: (매수수량, 매도수량, 총매수금액)
        let mut positions_map: HashMap<String, (Decimal, Decimal, Decimal)> = HashMap::new();

        for execution in &all_executions {
            if execution.filled_qty <= Decimal::ZERO {
                continue;
            }

            let entry = positions_map
                .entry(execution.stock_code.clone())
                .or_insert((Decimal::ZERO, Decimal::ZERO, Decimal::ZERO));

            match execution.side_code.as_str() {
                "02" => {
                    // 매수: 수량 누적 + 총 매수 금액 누적 (가중 평균 계산용)
                    entry.0 += execution.filled_qty;
                    entry.2 += execution.filled_qty * execution.avg_price;
                }
                "01" => {
                    // 매도: 매도 수량 누적
                    entry.1 += execution.filled_qty;
                }
                _ => {}
            }
        }

        // StrategyPositionInfo로 변환 (현재가 조회 포함)
        let mut positions = Vec::new();
        for (code, (buy_qty, sell_qty, total_buy_amount)) in positions_map {
            let current_qty = buy_qty - sell_qty;
            if current_qty <= Decimal::ZERO {
                continue; // 보유 수량 없으면 스킵
            }

            // 평균 매입가 계산 (가중 평균)
            let avg_entry_price = if buy_qty > Decimal::ZERO {
                total_buy_amount / buy_qty
            } else {
                Decimal::ZERO
            };

            // 현재가 조회
            let current_price = match self.client.get_price(&code).await {
                Ok(price_data) => price_data.current_price,
                Err(e) => {
                    warn!("현재가 조회 실패 ({}): {}", code, e);
                    avg_entry_price // 실패 시 매입가로 대체
                }
            };

            // 포지션 생성
            let mut position =
                StrategyPositionInfo::new(code.clone(), Side::Buy, current_qty, avg_entry_price);
            position.update_price(current_price);

            // 손익 계산
            let unrealized_pnl = (current_price - avg_entry_price) * current_qty;
            position.unrealized_pnl = unrealized_pnl;

            positions.push(position);
        }

        debug!("체결 내역 기반 포지션 조회 완료: {} 종목", positions.len());
        Ok(positions)
    }
}

#[async_trait]
impl ExchangeProvider for KisKrProvider {
    async fn fetch_account(&self) -> Result<StrategyAccountInfo, ProviderError> {
        // ISA 계좌는 잔고 조회 API 미지원 → 바로 체결 내역 기반 조회
        if self.is_isa_account() {
            debug!("ISA 계좌: 체결 내역 기반 계좌 조회");
            return self.fetch_account_from_order_history().await;
        }

        // 일반 계좌: get_balance() 시도
        match self.client.get_balance().await {
            Ok(balance) => {
                // 계좌 요약 정보가 없으면 체결 내역 기반으로 fallback
                let summary = match balance.summary {
                    Some(s) => s,
                    None => {
                        debug!("계좌 요약 정보 없음, 체결 내역 기반으로 전환");
                        return self.fetch_account_from_order_history().await;
                    }
                };

                // 보유 종목 평가액 합계
                let total_position_value: Decimal =
                    balance.holdings.iter().map(|h| h.eval_amount).sum();

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
            Err(e) => {
                // 일반 계좌에서 get_balance() 실패 시 경고 로그
                warn!("get_balance() 실패, 체결 내역 기반으로 전환: {}", e);
                self.fetch_account_from_order_history().await
            }
        }
    }

    async fn fetch_positions(&self) -> Result<Vec<StrategyPositionInfo>, ProviderError> {
        // ISA 계좌는 잔고 조회 API 미지원 → 바로 체결 내역 기반 조회
        if self.is_isa_account() {
            debug!("ISA 계좌: 체결 내역 기반 포지션 조회");
            return self.fetch_positions_from_order_history().await;
        }

        // 일반 계좌: get_balance() 시도
        match self.client.get_balance().await {
            Ok(balance) => {
                let mut positions = Vec::new();

                for holding in balance.holdings {
                    // 보유 수량이 없으면 스킵
                    if holding.quantity <= Decimal::ZERO {
                        continue;
                    }

                    // 포지션 생성 (현물은 매수만 가능)
                    let mut position = StrategyPositionInfo::new(
                        holding.stock_code.clone(),
                        Side::Buy,
                        holding.quantity,
                        holding.avg_price,
                    );

                    // 현재가 및 손익 업데이트
                    position.update_price(holding.current_price);

                    positions.push(position);
                }

                Ok(positions)
            }
            Err(e) => {
                // 일반 계좌에서 get_balance() 실패 시 경고 로그
                warn!("get_balance() 실패, 체결 내역 기반으로 전환: {}", e);
                self.fetch_positions_from_order_history().await
            }
        }
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
            let symbol = Symbol::new(&order.stock_code, "KRW", MarketType::Stock);

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
                ticker: symbol.to_string(),
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

    async fn fetch_execution_history(
        &self,
        request: &ExecutionHistoryRequest,
    ) -> Result<ExecutionHistoryResponse, ProviderError> {
        let is_isa = self.is_isa_account();
        info!(
            "fetch_execution_history 호출: is_isa={}, account_type={:?}",
            is_isa,
            self.client.oauth().config().account_type
        );

        // ISA 계좌는 전체 체결 내역 기반으로 조회 (페이징 포함)
        // ISA 계좌는 체결 기록 기반 자산 계산이 필수이므로 전체 내역 조회
        if is_isa {
            info!("ISA 계좌: 전체 체결 내역 기반 조회 시작");
            return self.fetch_execution_history_from_all_orders().await;
        }

        // 일반 계좌: 단일 페이지 조회
        // 커서 파싱 (format: "ctx_fk100|ctx_nk100")
        let (ctx_fk, ctx_nk) = if let Some(cursor) = &request.cursor {
            let parts: Vec<&str> = cursor.split('|').collect();
            if parts.len() == 2 {
                (parts[0].to_string(), parts[1].to_string())
            } else {
                (String::new(), String::new())
            }
        } else {
            (String::new(), String::new())
        };

        // side 파싱 (기본값: "00" = 전체)
        let side = request.side.as_deref().unwrap_or("00");

        // KIS API 호출
        let history = self
            .client
            .get_order_history(
                &request.start_date,
                &request.end_date,
                side,
                &ctx_fk,
                &ctx_nk,
            )
            .await
            .map_err(|e| ProviderError::Api(format!("KIS 체결 내역 조회 실패: {}", e)))?;

        // KIS 응답을 Trade로 변환
        let trades = self.convert_executions_to_trades(&history.executions);

        // 다음 페이지 커서 생성
        let next_cursor =
            if !history.ctx_area_fk100.is_empty() && !history.ctx_area_nk100.is_empty() {
                Some(format!(
                    "{}|{}",
                    history.ctx_area_fk100, history.ctx_area_nk100
                ))
            } else {
                None
            };

        Ok(ExecutionHistoryResponse {
            trades,
            next_cursor,
        })
    }
}

impl KisKrProvider {
    /// 전체 체결 내역을 조회하여 ExecutionHistoryResponse로 변환.
    ///
    /// ISA 계좌 등 체결 기록 기반 자산 계산이 필요한 경우 사용합니다.
    /// 페이징을 통해 계좌 개설 이후 모든 체결 내역을 조회합니다.
    async fn fetch_execution_history_from_all_orders(
        &self,
    ) -> Result<ExecutionHistoryResponse, ProviderError> {
        // 전체 체결 내역 조회 (페이징 포함)
        let all_executions = self.fetch_all_order_history().await?;

        info!(
            "ISA 계좌 전체 체결 내역 조회 완료: {} 건",
            all_executions.len()
        );

        // KIS 응답을 Trade로 변환
        let trades = self.convert_executions_to_trades(&all_executions);

        info!(
            "ISA 계좌 체결 내역 Trade 변환 완료: {} 건 (원본 {} 건)",
            trades.len(),
            all_executions.len()
        );

        // 전체 내역이므로 다음 페이지 없음
        Ok(ExecutionHistoryResponse {
            trades,
            next_cursor: None,
        })
    }

    /// KrOrderExecution 배열을 Trade 배열로 변환.
    fn convert_executions_to_trades(
        &self,
        executions: &[crate::connector::kis::client_kr::KrOrderExecution],
    ) -> Vec<Trade> {
        let mut trades = Vec::new();

        for execution in executions {
            // 체결 수량이 0이면 스킵
            if execution.filled_qty <= rust_decimal::Decimal::ZERO {
                continue;
            }

            // side 변환
            let trade_side = match execution.side_code.as_str() {
                "01" => Side::Sell,
                "02" => Side::Buy,
                _ => continue, // 알 수 없는 side는 스킵
            };

            // 심볼 생성
            let symbol = trader_core::types::Symbol::new(
                &execution.stock_code,
                "KRW",
                trader_core::types::MarketType::Stock,
            );

            // 체결 시각 파싱
            let executed_at = parse_kis_datetime(&execution.order_date, &execution.order_time)
                .unwrap_or_else(|_| chrono::Utc::now());

            // Trade 생성
            let trade = Trade {
                id: Uuid::new_v4(),
                order_id: Uuid::nil(), // KIS API는 내부 order_id를 제공하지 않음
                exchange: "KIS-KR".to_string(),
                exchange_trade_id: execution.order_no.clone(),
                ticker: symbol.to_string(),
                side: trade_side,
                quantity: execution.filled_qty,
                price: execution.avg_price,
                fee: Decimal::ZERO, // KIS API는 수수료를 제공하지 않음
                fee_currency: "KRW".to_string(),
                executed_at,
                is_maker: false, // KIS API는 메이커/테이커 정보를 제공하지 않음
                metadata: serde_json::json!({
                    "stock_name": execution.stock_name,
                    "order_no": execution.order_no,
                }),
            };

            trades.push(trade);
        }

        trades
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
