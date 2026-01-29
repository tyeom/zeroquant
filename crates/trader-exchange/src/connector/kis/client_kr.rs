//! KIS 국내 주식 REST API 클라이언트.
//!
//! 이 모듈은 한국투자증권 API를 통해 국내 주식/ETF 거래를 위한
//! REST API 클라이언트를 제공합니다.
//!
//! # 지원 기능
//!
//! - 현재가 조회
//! - 호가 조회
//! - 현금 매수/매도 주문
//! - 주문 정정/취소
//! - 잔고 조회
//! - 매수가능금액 조회

use super::auth::KisOAuth;
use super::config::KisEnvironment;
use super::tr_id;
use crate::ExchangeError;
use reqwest::Client;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::sync::Arc;
use tracing::{debug, error, info};
use trader_core::{ExecutionRecord, ExecutionHistory, Side, OrderStatusType, Symbol};

/// KIS 국내 주식 REST API 클라이언트.
///
/// `KisOAuth`를 `Arc`로 공유하여 동일한 `app_key`를 사용하는 여러 클라이언트가
/// 토큰을 공유할 수 있습니다. KIS API는 토큰 발급을 1분에 1회로 제한하므로
/// 토큰 공유가 필수적입니다.
pub struct KisKrClient {
    oauth: Arc<KisOAuth>,
    client: Client,
}

impl KisKrClient {
    /// 새로운 국내 주식 클라이언트 생성 (소유권 이전).
    ///
    /// 단일 클라이언트만 사용하는 경우 이 메서드를 사용합니다.
    pub fn new(oauth: KisOAuth) -> Self {
        Self::with_shared_oauth(Arc::new(oauth))
    }

    /// 공유된 OAuth로 국내 주식 클라이언트 생성.
    ///
    /// 동일한 `app_key`를 사용하는 여러 클라이언트(국내/해외, 실계좌/모의투자)가
    /// 토큰을 공유하려면 이 메서드를 사용합니다.
    ///
    /// # 예시
    /// ```ignore
    /// let oauth = Arc::new(KisOAuth::new(config));
    /// let kr_client = KisKrClient::with_shared_oauth(Arc::clone(&oauth));
    /// let us_client = KisUsClient::with_shared_oauth(oauth);
    /// ```
    pub fn with_shared_oauth(oauth: Arc<KisOAuth>) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(oauth.config().timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self { oauth, client }
    }

    /// 내부 OAuth 참조 반환 (토큰 캐싱용).
    pub fn oauth(&self) -> &Arc<KisOAuth> {
        &self.oauth
    }

    /// 환경에 따른 적절한 tr_id 반환.
    fn get_tr_id<'a>(&self, real_id: &'a str, paper_id: &'a str) -> &'a str {
        match self.oauth.config().environment {
            KisEnvironment::Real => real_id,
            KisEnvironment::Paper => paper_id,
        }
    }

    // ========================================
    // Market Data APIs (시세 조회)
    // ========================================

    /// 주식현재가 시세 조회.
    ///
    /// # 인자
    /// * `stock_code` - 종목코드 (예: "005930" 삼성전자)
    pub async fn get_price(&self, stock_code: &str) -> Result<KrStockPrice, ExchangeError> {
        let tr_id = self.get_tr_id(tr_id::KR_PRICE_REAL, tr_id::KR_PRICE_PAPER);
        let url = format!(
            "{}/uapi/domestic-stock/v1/quotations/inquire-price",
            self.oauth.config().rest_base_url()
        );

        let headers = self.oauth.build_headers(tr_id, None).await?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .query(&[("FID_COND_MRKT_DIV_CODE", "J"), ("FID_INPUT_ISCD", stock_code)])
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            error!("KR price inquiry failed: {} - {}", status, body);
            return Err(ExchangeError::ApiError {
                code: status.as_u16() as i32,
                message: body,
            });
        }

        debug!("KR price response: {}", body);

        let resp: KisKrPriceResponse = serde_json::from_str(&body)
            .map_err(|e| ExchangeError::ParseError(format!("Failed to parse price response: {}", e)))?;

        if resp.rt_cd != "0" {
            return Err(ExchangeError::ApiError {
                code: resp.msg_cd.parse().unwrap_or(-1),
                message: resp.msg1,
            });
        }

        Ok(resp.output)
    }

    /// 주식현재가 호가 조회.
    ///
    /// # 인자
    /// * `stock_code` - 종목코드
    pub async fn get_orderbook(&self, stock_code: &str) -> Result<KrOrderBook, ExchangeError> {
        let tr_id = self.get_tr_id(tr_id::KR_ORDERBOOK_REAL, tr_id::KR_ORDERBOOK_PAPER);
        let url = format!(
            "{}/uapi/domestic-stock/v1/quotations/inquire-asking-price-exp-ccn",
            self.oauth.config().rest_base_url()
        );

        let headers = self.oauth.build_headers(tr_id, None).await?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .query(&[("FID_COND_MRKT_DIV_CODE", "J"), ("FID_INPUT_ISCD", stock_code)])
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            error!("KR orderbook inquiry failed: {} - {}", status, body);
            return Err(ExchangeError::ApiError {
                code: status.as_u16() as i32,
                message: body,
            });
        }

        let resp: KisKrOrderBookResponse = serde_json::from_str(&body)
            .map_err(|e| ExchangeError::ParseError(format!("Failed to parse orderbook response: {}", e)))?;

        if resp.rt_cd != "0" {
            return Err(ExchangeError::ApiError {
                code: resp.msg_cd.parse().unwrap_or(-1),
                message: resp.msg1,
            });
        }

        Ok(resp.output1)
    }

    // ========================================
    // Trading APIs (주문)
    // ========================================

    /// 현금 매수 주문.
    ///
    /// # 인자
    /// * `stock_code` - 종목코드
    /// * `quantity` - 주문 수량
    /// * `price` - 주문 가격 (시장가 주문의 경우 0)
    /// * `order_type` - 주문 유형 ("00" = 지정가, "01" = 시장가)
    pub async fn place_buy_order(
        &self,
        stock_code: &str,
        quantity: u32,
        price: Decimal,
        order_type: &str,
    ) -> Result<KrOrderResponse, ExchangeError> {
        self.place_order(stock_code, quantity, price, order_type, true).await
    }

    /// 현금 매도 주문.
    ///
    /// # 인자
    /// * `stock_code` - 종목코드
    /// * `quantity` - 주문 수량
    /// * `price` - 주문 가격 (시장가 주문의 경우 0)
    /// * `order_type` - 주문 유형 ("00" = 지정가, "01" = 시장가)
    pub async fn place_sell_order(
        &self,
        stock_code: &str,
        quantity: u32,
        price: Decimal,
        order_type: &str,
    ) -> Result<KrOrderResponse, ExchangeError> {
        self.place_order(stock_code, quantity, price, order_type, false).await
    }

    /// 내부 주문 실행.
    async fn place_order(
        &self,
        stock_code: &str,
        quantity: u32,
        price: Decimal,
        order_type: &str,
        is_buy: bool,
    ) -> Result<KrOrderResponse, ExchangeError> {
        let tr_id = if is_buy {
            self.get_tr_id(tr_id::KR_BUY_REAL, tr_id::KR_BUY_PAPER)
        } else {
            self.get_tr_id(tr_id::KR_SELL_REAL, tr_id::KR_SELL_PAPER)
        };

        let url = format!(
            "{}/uapi/domestic-stock/v1/trading/order-cash",
            self.oauth.config().rest_base_url()
        );

        // 요청 본문 구성
        let body = serde_json::json!({
            "CANO": self.oauth.config().cano(),
            "ACNT_PRDT_CD": self.oauth.config().acnt_prdt_cd(),
            "PDNO": stock_code,
            "ORD_DVSN": order_type,
            "ORD_QTY": quantity.to_string(),
            "ORD_UNPR": price.to_string(),
        });

        // POST 요청용 해시 키 생성
        let hashkey = self.oauth.generate_hashkey(&body).await?;
        let headers = self.oauth.build_headers(tr_id, Some(&hashkey)).await?;

        info!(
            "Placing KR {} order: {} x {} @ {} (type: {})",
            if is_buy { "BUY" } else { "SELL" },
            stock_code,
            quantity,
            price,
            order_type
        );

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        let status = response.status();
        let response_body = response
            .text()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            error!("KR order failed: {} - {}", status, response_body);
            return Err(ExchangeError::ApiError {
                code: status.as_u16() as i32,
                message: response_body,
            });
        }

        debug!("KR order response: {}", response_body);

        let resp: KisKrOrderApiResponse = serde_json::from_str(&response_body)
            .map_err(|e| ExchangeError::ParseError(format!("Failed to parse order response: {}", e)))?;

        if resp.rt_cd != "0" {
            return Err(ExchangeError::ApiError {
                code: resp.msg_cd.parse().unwrap_or(-1),
                message: resp.msg1,
            });
        }

        info!(
            "KR order placed successfully: order_no={}, stock={}",
            resp.output.odno, stock_code
        );

        Ok(resp.output)
    }

    /// 주문 취소.
    ///
    /// # 인자
    /// * `order_no` - 원주문번호
    /// * `stock_code` - 종목코드
    /// * `quantity` - 취소 수량 (0 = 전량)
    pub async fn cancel_order(
        &self,
        order_no: &str,
        stock_code: &str,
        quantity: u32,
    ) -> Result<KrOrderResponse, ExchangeError> {
        self.modify_or_cancel_order(order_no, stock_code, quantity, Decimal::ZERO, "02").await
    }

    /// 주문 정정.
    ///
    /// # 인자
    /// * `order_no` - 원주문번호
    /// * `stock_code` - 종목코드
    /// * `quantity` - 새로운 수량
    /// * `price` - 새로운 가격
    pub async fn modify_order(
        &self,
        order_no: &str,
        stock_code: &str,
        quantity: u32,
        price: Decimal,
    ) -> Result<KrOrderResponse, ExchangeError> {
        self.modify_or_cancel_order(order_no, stock_code, quantity, price, "01").await
    }

    /// 내부 정정/취소 주문.
    async fn modify_or_cancel_order(
        &self,
        order_no: &str,
        stock_code: &str,
        quantity: u32,
        price: Decimal,
        rvse_cncl_dvsn_cd: &str,
    ) -> Result<KrOrderResponse, ExchangeError> {
        let tr_id = self.get_tr_id(tr_id::KR_CANCEL_REAL, tr_id::KR_CANCEL_PAPER);

        let url = format!(
            "{}/uapi/domestic-stock/v1/trading/order-rvsecncl",
            self.oauth.config().rest_base_url()
        );

        let body = serde_json::json!({
            "CANO": self.oauth.config().cano(),
            "ACNT_PRDT_CD": self.oauth.config().acnt_prdt_cd(),
            "KRX_FWDG_ORD_ORGNO": "",
            "ORGN_ODNO": order_no,
            "ORD_DVSN": "00",
            "RVSE_CNCL_DVSN_CD": rvse_cncl_dvsn_cd,
            "ORD_QTY": quantity.to_string(),
            "ORD_UNPR": price.to_string(),
            "QTY_ALL_ORD_YN": if quantity == 0 { "Y" } else { "N" },
        });

        let hashkey = self.oauth.generate_hashkey(&body).await?;
        let headers = self.oauth.build_headers(tr_id, Some(&hashkey)).await?;

        info!(
            "KR order {}: order_no={}, stock={}",
            if rvse_cncl_dvsn_cd == "02" { "CANCEL" } else { "MODIFY" },
            order_no,
            stock_code
        );

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        let status = response.status();
        let response_body = response
            .text()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            error!("KR order modify/cancel failed: {} - {}", status, response_body);
            return Err(ExchangeError::ApiError {
                code: status.as_u16() as i32,
                message: response_body,
            });
        }

        let resp: KisKrOrderApiResponse = serde_json::from_str(&response_body)
            .map_err(|e| ExchangeError::ParseError(format!("Failed to parse response: {}", e)))?;

        if resp.rt_cd != "0" {
            return Err(ExchangeError::ApiError {
                code: resp.msg_cd.parse().unwrap_or(-1),
                message: resp.msg1,
            });
        }

        Ok(resp.output)
    }

    // ========================================
    // Account APIs (계좌)
    // ========================================

    /// 잔고 조회.
    pub async fn get_balance(&self) -> Result<KrBalance, ExchangeError> {
        let tr_id = self.get_tr_id(tr_id::KR_BALANCE_REAL, tr_id::KR_BALANCE_PAPER);
        let url = format!(
            "{}/uapi/domestic-stock/v1/trading/inquire-balance",
            self.oauth.config().rest_base_url()
        );

        let headers = self.oauth.build_headers(tr_id, None).await?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .query(&[
                ("CANO", self.oauth.config().cano()),
                ("ACNT_PRDT_CD", self.oauth.config().acnt_prdt_cd()),
                ("AFHR_FLPR_YN", "N"),
                ("OFL_YN", ""),
                ("INQR_DVSN", "02"),
                ("UNPR_DVSN", "01"),
                ("FUND_STTL_ICLD_YN", "N"),
                ("FNCG_AMT_AUTO_RDPT_YN", "N"),
                ("PRCS_DVSN", "00"),
                ("CTX_AREA_FK100", ""),
                ("CTX_AREA_NK100", ""),
            ])
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            error!("KR balance inquiry failed: {} - {}", status, body);
            return Err(ExchangeError::ApiError {
                code: status.as_u16() as i32,
                message: body,
            });
        }

        debug!("KR balance response: {}", body);

        let resp: KisKrBalanceResponse = serde_json::from_str(&body)
            .map_err(|e| ExchangeError::ParseError(format!("Failed to parse balance response: {}", e)))?;

        if resp.rt_cd != "0" {
            return Err(ExchangeError::ApiError {
                code: resp.msg_cd.parse().unwrap_or(-1),
                message: resp.msg1,
            });
        }

        Ok(KrBalance {
            holdings: resp.output1,
            summary: resp.output2.into_iter().next(),
        })
    }

    // ========================================
    // Chart Data APIs (차트/캔들 데이터)
    // ========================================

    /// 국내 주식 일/주/월/년봉 조회.
    ///
    /// # 인자
    /// * `stock_code` - 종목코드 (예: "005930" 삼성전자)
    /// * `period` - 기간 유형: "D" (일별), "W" (주별), "M" (월별), "Y" (년별)
    /// * `start_date` - 시작일 (YYYYMMDD)
    /// * `end_date` - 종료일 (YYYYMMDD)
    /// * `adj_price` - 수정주가 적용 여부 (true = 수정주가)
    pub async fn get_daily_price(
        &self,
        stock_code: &str,
        period: &str,
        start_date: &str,
        end_date: &str,
        adj_price: bool,
    ) -> Result<Vec<KrOhlcv>, ExchangeError> {
        let tr_id = self.get_tr_id(tr_id::KR_DAILY_PRICE_REAL, tr_id::KR_DAILY_PRICE_PAPER);
        let url = format!(
            "{}/uapi/domestic-stock/v1/quotations/inquire-daily-price",
            self.oauth.config().rest_base_url()
        );

        let headers = self.oauth.build_headers(tr_id, None).await?;

        // FID_PERIOD_DIV_CODE: D(일), W(주), M(월), Y(년)
        let adj_code = if adj_price { "0" } else { "1" }; // 0=수정주가, 1=원주가

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .query(&[
                ("FID_COND_MRKT_DIV_CODE", "J"),  // J=주식
                ("FID_INPUT_ISCD", stock_code),
                ("FID_INPUT_DATE_1", start_date),
                ("FID_INPUT_DATE_2", end_date),
                ("FID_PERIOD_DIV_CODE", period),
                ("FID_ORG_ADJ_PRC", adj_code),
            ])
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            error!("KR daily price inquiry failed: {} - {}", status, body);
            return Err(ExchangeError::ApiError {
                code: status.as_u16() as i32,
                message: body,
            });
        }

        debug!("KR daily price response: {}", body);

        let resp: KisKrDailyPriceResponse = serde_json::from_str(&body)
            .map_err(|e| ExchangeError::ParseError(format!("Failed to parse daily price response: {}", e)))?;

        if resp.rt_cd != "0" {
            return Err(ExchangeError::ApiError {
                code: resp.msg_cd.parse().unwrap_or(-1),
                message: resp.msg1,
            });
        }

        Ok(resp.output)
    }

    /// 국내 주식 분봉 조회.
    ///
    /// # 인자
    /// * `stock_code` - 종목코드
    /// * `time_unit` - 분 단위 (1, 3, 5, 10, 15, 30, 60)
    pub async fn get_minute_chart(
        &self,
        stock_code: &str,
        time_unit: u32,
    ) -> Result<Vec<KrMinuteOhlcv>, ExchangeError> {
        let tr_id = self.get_tr_id(tr_id::KR_MINUTE_PRICE_REAL, tr_id::KR_MINUTE_PRICE_PAPER);
        let url = format!(
            "{}/uapi/domestic-stock/v1/quotations/inquire-time-itemchartprice",
            self.oauth.config().rest_base_url()
        );

        let headers = self.oauth.build_headers(tr_id, None).await?;

        // 현재 시간 기준 조회 (HH:MM:SS 형식)
        let now = chrono::Utc::now() + chrono::Duration::hours(9); // KST
        let time_str = now.format("%H%M%S").to_string();

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .query(&[
                ("FID_ETC_CLS_CODE", ""),
                ("FID_COND_MRKT_DIV_CODE", "J"),
                ("FID_INPUT_ISCD", stock_code),
                ("FID_INPUT_HOUR_1", &time_str),
                ("FID_PW_DATA_INCU_YN", "Y"),  // 과거 데이터 포함
            ])
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            error!("KR minute chart inquiry failed: {} - {}", status, body);
            return Err(ExchangeError::ApiError {
                code: status.as_u16() as i32,
                message: body,
            });
        }

        debug!("KR minute chart response: {}", body);

        let resp: KisKrMinuteChartResponse = serde_json::from_str(&body)
            .map_err(|e| ExchangeError::ParseError(format!("Failed to parse minute chart response: {}", e)))?;

        if resp.rt_cd != "0" {
            return Err(ExchangeError::ApiError {
                code: resp.msg_cd.parse().unwrap_or(-1),
                message: resp.msg1,
            });
        }

        // time_unit에 따라 데이터 간격 조정 (API는 기본 1분봉 반환)
        let filtered: Vec<KrMinuteOhlcv> = if time_unit > 1 {
            resp.output2
                .into_iter()
                .enumerate()
                .filter(|(i, _)| i % (time_unit as usize) == 0)
                .map(|(_, v)| v)
                .collect()
        } else {
            resp.output2
        };

        Ok(filtered)
    }

    // ========================================
    // Account APIs (계좌)
    // ========================================

    /// 매수가능금액 조회.
    ///
    /// # 인자
    /// * `stock_code` - 종목코드
    /// * `price` - 계산을 위한 목표 가격
    pub async fn get_buy_power(
        &self,
        stock_code: &str,
        price: Decimal,
    ) -> Result<KrBuyPower, ExchangeError> {
        let tr_id = self.get_tr_id(tr_id::KR_BUYABLE_REAL, tr_id::KR_BUYABLE_PAPER);
        let url = format!(
            "{}/uapi/domestic-stock/v1/trading/inquire-psbl-order",
            self.oauth.config().rest_base_url()
        );

        let headers = self.oauth.build_headers(tr_id, None).await?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .query(&[
                ("CANO", self.oauth.config().cano()),
                ("ACNT_PRDT_CD", self.oauth.config().acnt_prdt_cd()),
                ("PDNO", stock_code),
                ("ORD_UNPR", &price.to_string()),
                ("ORD_DVSN", "00"),
                ("CMA_EVLU_AMT_ICLD_YN", "Y"),
                ("OVRS_ICLD_YN", "N"),
            ])
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            error!("KR buy power inquiry failed: {} - {}", status, body);
            return Err(ExchangeError::ApiError {
                code: status.as_u16() as i32,
                message: body,
            });
        }

        let resp: KisKrBuyPowerResponse = serde_json::from_str(&body)
            .map_err(|e| ExchangeError::ParseError(format!("Failed to parse buy power response: {}", e)))?;

        if resp.rt_cd != "0" {
            return Err(ExchangeError::ApiError {
                code: resp.msg_cd.parse().unwrap_or(-1),
                message: resp.msg1,
            });
        }

        Ok(resp.output)
    }

    // ========================================
    // Order History APIs (체결 내역)
    // ========================================

    /// 일별 주문체결 조회 (체결 내역).
    ///
    /// 특정 기간 동안의 주문 및 체결 내역을 조회합니다.
    ///
    /// # 인자
    /// * `start_date` - 시작일 (YYYYMMDD)
    /// * `end_date` - 종료일 (YYYYMMDD)
    /// * `side` - 매수/매도 구분: "00" (전체), "01" (매도), "02" (매수)
    /// * `ctx_area_fk100` - 연속조회키 (첫 조회시 빈 문자열)
    /// * `ctx_area_nk100` - 연속조회키 (첫 조회시 빈 문자열)
    pub async fn get_order_history(
        &self,
        start_date: &str,
        end_date: &str,
        side: &str,
        ctx_area_fk100: &str,
        ctx_area_nk100: &str,
    ) -> Result<KrOrderHistory, ExchangeError> {
        let tr_id = self.get_tr_id(tr_id::KR_ORDER_HISTORY_REAL, tr_id::KR_ORDER_HISTORY_PAPER);
        let url = format!(
            "{}/uapi/domestic-stock/v1/trading/inquire-daily-ccld",
            self.oauth.config().rest_base_url()
        );

        let headers = self.oauth.build_headers(tr_id, None).await?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .query(&[
                ("CANO", self.oauth.config().cano()),
                ("ACNT_PRDT_CD", self.oauth.config().acnt_prdt_cd()),
                ("INQR_STRT_DT", start_date),
                ("INQR_END_DT", end_date),
                ("SLL_BUY_DVSN_CD", side),
                ("INQR_DVSN", "00"),  // 00=역순
                ("PDNO", ""),  // 전 종목
                ("CCLD_DVSN", "00"),  // 00=전체, 01=체결, 02=미체결
                ("ORD_GNO_BRNO", ""),
                ("ODNO", ""),
                ("INQR_DVSN_3", "00"),  // 00=전체, 01=현금, 02=신용
                ("INQR_DVSN_1", ""),
                ("INQR_DVSN_2", ""),  // Python 레퍼런스에 있음
                ("CTX_AREA_FK100", ctx_area_fk100),
                ("CTX_AREA_NK100", ctx_area_nk100),
                ("EXCG_ID_DVSN_CD", "KRX"),  // 거래소 구분 코드 (Python 레퍼런스)
            ])
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            error!("KR order history inquiry failed: {} - {}", status, body);
            return Err(ExchangeError::ApiError {
                code: status.as_u16() as i32,
                message: body,
            });
        }

        // 디버깅용: 실제 API 응답 출력
        info!("KR order history response (first 2000 chars): {}", &body[..body.len().min(2000)]);

        let resp: KisKrOrderHistoryResponse = serde_json::from_str(&body)
            .map_err(|e| ExchangeError::ParseError(format!("Failed to parse order history response: {}", e)))?;

        if resp.rt_cd != "0" {
            return Err(ExchangeError::ApiError {
                code: resp.msg_cd.parse().unwrap_or(-1),
                message: resp.msg1,
            });
        }

        // 연속 조회 키 trim (Python 모듈과 동일하게 처리)
        // KIS API 응답에 공백 패딩이 포함되어 있으므로 제거 필요
        let ctx_fk = resp.ctx_area_fk100.trim().to_string();
        let ctx_nk = resp.ctx_area_nk100.trim().to_string();

        // 연속 조회 가능 여부 확인
        // Python 로직: NKKey가 비어있지 않고, 실제 데이터가 있으면 연속 조회
        let has_more = !ctx_nk.is_empty() && !resp.output1.is_empty();

        info!(
            "KR order history loaded: {} executions, has_more={}, ctx_fk_len={}, ctx_nk_len={}",
            resp.output1.len(),
            has_more,
            ctx_fk.len(),
            ctx_nk.len()
        );

        Ok(KrOrderHistory {
            executions: resp.output1,
            ctx_area_fk100: ctx_fk,
            ctx_area_nk100: ctx_nk,
            has_more,
        })
    }
}

// ========================================
// 응답 타입
// ========================================

/// 국내 주식 시세 데이터.
#[derive(Debug, Clone, Deserialize)]
pub struct KrStockPrice {
    /// 종목코드
    #[serde(rename = "stck_shrn_iscd")]
    pub stock_code: String,
    /// 현재가
    #[serde(rename = "stck_prpr", deserialize_with = "deserialize_decimal")]
    pub current_price: Decimal,
    /// 전일대비
    #[serde(rename = "prdy_vrss", deserialize_with = "deserialize_decimal")]
    pub price_change: Decimal,
    /// 등락률 (%)
    #[serde(rename = "prdy_ctrt", deserialize_with = "deserialize_decimal")]
    pub change_rate: Decimal,
    /// 누적거래량
    #[serde(rename = "acml_vol", deserialize_with = "deserialize_decimal")]
    pub volume: Decimal,
    /// 누적거래대금
    #[serde(rename = "acml_tr_pbmn", deserialize_with = "deserialize_decimal")]
    pub trading_value: Decimal,
    /// 당일 고가
    #[serde(rename = "stck_hgpr", deserialize_with = "deserialize_decimal")]
    pub high: Decimal,
    /// 당일 저가
    #[serde(rename = "stck_lwpr", deserialize_with = "deserialize_decimal")]
    pub low: Decimal,
    /// 당일 시가
    #[serde(rename = "stck_oprc", deserialize_with = "deserialize_decimal")]
    pub open: Decimal,
    /// 전일 종가
    #[serde(rename = "stck_sdpr", deserialize_with = "deserialize_decimal")]
    pub prev_close: Decimal,
    /// 상한가
    #[serde(rename = "stck_mxpr", deserialize_with = "deserialize_decimal")]
    pub upper_limit: Decimal,
    /// 하한가
    #[serde(rename = "stck_llam", deserialize_with = "deserialize_decimal")]
    pub lower_limit: Decimal,
}

/// 국내 주식 호가 데이터.
#[derive(Debug, Clone, Deserialize)]
pub struct KrOrderBook {
    /// 매도호가 1
    #[serde(rename = "askp1", deserialize_with = "deserialize_decimal")]
    pub ask_price_1: Decimal,
    /// 매도호가 잔량 1
    #[serde(rename = "askp_rsqn1", deserialize_with = "deserialize_decimal")]
    pub ask_qty_1: Decimal,
    /// 매수호가 1
    #[serde(rename = "bidp1", deserialize_with = "deserialize_decimal")]
    pub bid_price_1: Decimal,
    /// 매수호가 잔량 1
    #[serde(rename = "bidp_rsqn1", deserialize_with = "deserialize_decimal")]
    pub bid_qty_1: Decimal,
    /// 총 매도호가 잔량
    #[serde(rename = "total_askp_rsqn", deserialize_with = "deserialize_decimal")]
    pub total_ask_qty: Decimal,
    /// 총 매수호가 잔량
    #[serde(rename = "total_bidp_rsqn", deserialize_with = "deserialize_decimal")]
    pub total_bid_qty: Decimal,
}

/// 국내 주문 응답.
#[derive(Debug, Clone, Deserialize)]
pub struct KrOrderResponse {
    /// 주문번호
    #[serde(rename = "ODNO")]
    pub odno: String,
    /// 주문시간 (HHMMSS)
    #[serde(rename = "ORD_TMD")]
    pub order_time: String,
}

/// 국내 계좌 보유 종목.
#[derive(Debug, Clone, Deserialize)]
pub struct KrHolding {
    /// 종목코드
    #[serde(rename = "pdno")]
    pub stock_code: String,
    /// 종목명
    #[serde(rename = "prdt_name")]
    pub stock_name: String,
    /// 보유수량
    #[serde(rename = "hldg_qty", deserialize_with = "deserialize_decimal")]
    pub quantity: Decimal,
    /// 매도가능수량
    #[serde(rename = "ord_psbl_qty", deserialize_with = "deserialize_decimal")]
    pub sellable_qty: Decimal,
    /// 매입평균가격
    #[serde(rename = "pchs_avg_pric", deserialize_with = "deserialize_decimal")]
    pub avg_price: Decimal,
    /// 현재가
    #[serde(rename = "prpr", deserialize_with = "deserialize_decimal")]
    pub current_price: Decimal,
    /// 평가금액
    #[serde(rename = "evlu_amt", deserialize_with = "deserialize_decimal")]
    pub eval_amount: Decimal,
    /// 평가손익금액
    #[serde(rename = "evlu_pfls_amt", deserialize_with = "deserialize_decimal")]
    pub profit_loss: Decimal,
    /// 평가손익률 (%)
    #[serde(rename = "evlu_pfls_rt", deserialize_with = "deserialize_decimal")]
    pub profit_loss_rate: Decimal,
}

/// 국내 계좌 요약.
#[derive(Debug, Clone, Deserialize)]
pub struct KrAccountSummary {
    /// 예수금
    #[serde(rename = "dnca_tot_amt", deserialize_with = "deserialize_decimal")]
    pub cash_balance: Decimal,
    /// 총 평가금액
    #[serde(rename = "tot_evlu_amt", deserialize_with = "deserialize_decimal")]
    pub total_eval_amount: Decimal,
    /// 총 평가손익
    #[serde(rename = "evlu_pfls_smtl_amt", deserialize_with = "deserialize_decimal")]
    pub total_profit_loss: Decimal,
}

/// 국내 계좌 잔고.
#[derive(Debug, Clone)]
pub struct KrBalance {
    /// 보유 종목
    pub holdings: Vec<KrHolding>,
    /// 계좌 요약
    pub summary: Option<KrAccountSummary>,
}

/// 국내 주식 일/주/월/년봉 데이터.
#[derive(Debug, Clone, Deserialize)]
pub struct KrOhlcv {
    /// 영업일자 (YYYYMMDD)
    #[serde(rename = "stck_bsop_date")]
    pub date: String,
    /// 시가
    #[serde(rename = "stck_oprc", deserialize_with = "deserialize_decimal")]
    pub open: Decimal,
    /// 고가
    #[serde(rename = "stck_hgpr", deserialize_with = "deserialize_decimal")]
    pub high: Decimal,
    /// 저가
    #[serde(rename = "stck_lwpr", deserialize_with = "deserialize_decimal")]
    pub low: Decimal,
    /// 종가
    #[serde(rename = "stck_clpr", deserialize_with = "deserialize_decimal")]
    pub close: Decimal,
    /// 거래량
    #[serde(rename = "acml_vol", deserialize_with = "deserialize_decimal")]
    pub volume: Decimal,
    /// 거래대금 (일부 API 응답에서 누락될 수 있음)
    #[serde(rename = "acml_tr_pbmn", default, deserialize_with = "deserialize_decimal")]
    pub trading_value: Decimal,
    /// 전일 대비 (일부 API 응답에서 누락될 수 있음)
    #[serde(rename = "prdy_vrss", default, deserialize_with = "deserialize_decimal")]
    pub change: Decimal,
    /// 등락률 (%) (일부 API 응답에서 누락될 수 있음)
    #[serde(rename = "prdy_ctrt", default, deserialize_with = "deserialize_decimal")]
    pub change_rate: Decimal,
}

/// 국내 주식 분봉 데이터.
#[derive(Debug, Clone, Deserialize)]
pub struct KrMinuteOhlcv {
    /// 체결 시간 (HHMMSS)
    #[serde(rename = "stck_cntg_hour")]
    pub time: String,
    /// 시가
    #[serde(rename = "stck_oprc", deserialize_with = "deserialize_decimal")]
    pub open: Decimal,
    /// 고가
    #[serde(rename = "stck_hgpr", deserialize_with = "deserialize_decimal")]
    pub high: Decimal,
    /// 저가
    #[serde(rename = "stck_lwpr", deserialize_with = "deserialize_decimal")]
    pub low: Decimal,
    /// 현재가 (종가)
    #[serde(rename = "stck_prpr", deserialize_with = "deserialize_decimal")]
    pub close: Decimal,
    /// 거래량
    #[serde(rename = "cntg_vol", deserialize_with = "deserialize_decimal")]
    pub volume: Decimal,
}

/// 국내 매수가능금액.
#[derive(Debug, Clone, Deserialize)]
pub struct KrBuyPower {
    /// 최대 주문가능수량
    #[serde(rename = "ord_psbl_qty", deserialize_with = "deserialize_decimal")]
    pub max_quantity: Decimal,
    /// 현금 주문가능금액
    #[serde(rename = "ord_psbl_cash", deserialize_with = "deserialize_decimal")]
    pub orderable_cash: Decimal,
    /// 총 주문가능금액 (신용 포함)
    #[serde(rename = "ord_psbl_amt", deserialize_with = "deserialize_decimal")]
    pub orderable_amount: Decimal,
}

/// 국내 주식 체결 내역 (일별 주문체결조회).
#[derive(Debug, Clone, Deserialize)]
pub struct KrOrderExecution {
    /// 주문일자 (YYYYMMDD)
    #[serde(rename = "ord_dt")]
    pub order_date: String,
    /// 주문번호
    #[serde(rename = "odno")]
    pub order_no: String,
    /// 원주문번호
    #[serde(rename = "orgn_odno")]
    pub original_order_no: String,
    /// 주문시각 (HHMMSS)
    #[serde(rename = "ord_tmd")]
    pub order_time: String,
    /// 매도매수구분코드 (01=매도, 02=매수)
    #[serde(rename = "sll_buy_dvsn_cd")]
    pub side_code: String,
    /// 매도매수구분명
    #[serde(rename = "sll_buy_dvsn_cd_name")]
    pub side_name: String,
    /// 종목코드
    #[serde(rename = "pdno")]
    pub stock_code: String,
    /// 종목명
    #[serde(rename = "prdt_name")]
    pub stock_name: String,
    /// 주문수량
    #[serde(rename = "ord_qty", deserialize_with = "deserialize_decimal")]
    pub order_qty: Decimal,
    /// 주문단가
    #[serde(rename = "ord_unpr", deserialize_with = "deserialize_decimal")]
    pub order_price: Decimal,
    /// 총체결수량
    #[serde(rename = "tot_ccld_qty", deserialize_with = "deserialize_decimal")]
    pub filled_qty: Decimal,
    /// 체결평균가
    #[serde(rename = "avg_prvs", deserialize_with = "deserialize_decimal")]
    pub avg_price: Decimal,
    /// 총체결금액
    #[serde(rename = "tot_ccld_amt", deserialize_with = "deserialize_decimal")]
    pub filled_amount: Decimal,
    /// 주문구분명 (지정가, 시장가 등)
    #[serde(rename = "ord_dvsn_name")]
    pub order_type_name: String,
    /// 주문상태명 (접수, 체결 등)
    #[serde(rename = "ord_gno_brno", default)]
    pub order_branch: String,
    /// 취소수량
    #[serde(rename = "cncl_yn", default)]
    pub cancel_yn: String,
    /// 정정취소구분명
    #[serde(rename = "rvse_cncl_dvsn_name", default)]
    pub modify_cancel_name: String,
}

/// 체결 내역 조회 결과.
#[derive(Debug, Clone)]
pub struct KrOrderHistory {
    /// 체결 내역 목록
    pub executions: Vec<KrOrderExecution>,
    /// 연속 조회 키 (다음 페이지용)
    pub ctx_area_fk100: String,
    /// 연속 조회 키 (다음 페이지용)
    pub ctx_area_nk100: String,
    /// 추가 데이터 존재 여부
    pub has_more: bool,
}

impl KrOrderExecution {
    /// 거래소 중립적 ExecutionRecord로 변환.
    pub fn to_execution_record(&self) -> ExecutionRecord {
        // 매수/매도 구분 (01=매도, 02=매수)
        let side = match self.side_code.as_str() {
            "01" => Side::Sell,
            "02" => Side::Buy,
            _ => Side::Buy, // 기본값
        };

        // 주문 상태 결정
        let status = if self.filled_qty >= self.order_qty && self.order_qty > Decimal::ZERO {
            OrderStatusType::Filled
        } else if self.filled_qty > Decimal::ZERO {
            OrderStatusType::PartiallyFilled
        } else if self.cancel_yn == "Y" {
            OrderStatusType::Cancelled
        } else {
            OrderStatusType::Open
        };

        // 주문 일시 파싱 (YYYYMMDD + HHMMSS)
        let ordered_at = chrono::NaiveDateTime::parse_from_str(
            &format!("{} {}", self.order_date, self.order_time),
            "%Y%m%d %H%M%S"
        )
        .map(|ndt| {
            use chrono::{TimeZone, Utc};
            use chrono_tz::Asia::Seoul;
            // KST -> UTC 변환
            Seoul.from_local_datetime(&ndt)
                .single()
                .map(|kst| kst.with_timezone(&Utc))
                .unwrap_or_else(Utc::now)
        })
        .unwrap_or_else(|_| chrono::Utc::now());

        ExecutionRecord {
            exchange: "KIS".to_string(),
            order_id: self.order_no.clone(),
            original_order_id: if self.original_order_no.is_empty() {
                None
            } else {
                Some(self.original_order_no.clone())
            },
            symbol: Symbol::stock(&self.stock_code, "KRW"),
            asset_name: self.stock_name.clone(),
            side,
            order_type: self.order_type_name.clone(),
            order_qty: self.order_qty,
            order_price: self.order_price,
            filled_qty: self.filled_qty,
            filled_price: self.avg_price,
            filled_amount: self.filled_amount,
            status,
            is_cancelled: self.cancel_yn == "Y",
            modify_type: if self.modify_cancel_name.is_empty() {
                None
            } else {
                Some(self.modify_cancel_name.clone())
            },
            ordered_at,
            metadata: serde_json::json!({
                "order_branch": self.order_branch,
                "side_name": self.side_name,
            }),
        }
    }
}

impl KrOrderHistory {
    /// 거래소 중립적 ExecutionHistory로 변환.
    pub fn to_execution_history(&self) -> ExecutionHistory {
        ExecutionHistory {
            records: self.executions.iter().map(|e| e.to_execution_record()).collect(),
            has_more: self.has_more,
            next_cursor: if self.has_more {
                Some(format!("{}|{}", self.ctx_area_fk100, self.ctx_area_nk100))
            } else {
                None
            },
        }
    }
}

// ========================================
// API 응답 래퍼
// ========================================

#[derive(Debug, Deserialize)]
struct KisKrPriceResponse {
    rt_cd: String,
    msg_cd: String,
    msg1: String,
    output: KrStockPrice,
}

#[derive(Debug, Deserialize)]
struct KisKrOrderBookResponse {
    rt_cd: String,
    msg_cd: String,
    msg1: String,
    output1: KrOrderBook,
}

#[derive(Debug, Deserialize)]
struct KisKrOrderApiResponse {
    rt_cd: String,
    msg_cd: String,
    msg1: String,
    output: KrOrderResponse,
}

#[derive(Debug, Deserialize)]
struct KisKrBalanceResponse {
    rt_cd: String,
    msg_cd: String,
    msg1: String,
    output1: Vec<KrHolding>,
    output2: Vec<KrAccountSummary>,
}

#[derive(Debug, Deserialize)]
struct KisKrBuyPowerResponse {
    rt_cd: String,
    msg_cd: String,
    msg1: String,
    output: KrBuyPower,
}

#[derive(Debug, Deserialize)]
struct KisKrDailyPriceResponse {
    rt_cd: String,
    msg_cd: String,
    msg1: String,
    output: Vec<KrOhlcv>,
}

#[derive(Debug, Deserialize)]
struct KisKrMinuteChartResponse {
    rt_cd: String,
    msg_cd: String,
    msg1: String,
    #[serde(default)]
    output1: serde_json::Value,  // 종목 기본 정보 (필요시 사용)
    output2: Vec<KrMinuteOhlcv>,
}

#[derive(Debug, Deserialize)]
struct KisKrOrderHistoryResponse {
    rt_cd: String,
    msg_cd: String,
    msg1: String,
    #[serde(default)]
    ctx_area_fk100: String,
    #[serde(default)]
    ctx_area_nk100: String,
    #[serde(default)]
    output1: Vec<KrOrderExecution>,
}

// ========================================
// 유틸리티 함수
// ========================================

/// 문자열을 Decimal로 역직렬화.
fn deserialize_decimal<'de, D>(deserializer: D) -> Result<Decimal, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = String::deserialize(deserializer)?;
    if s.is_empty() || s == "-" {
        return Ok(Decimal::ZERO);
    }
    s.parse::<Decimal>()
        .map_err(|_| serde::de::Error::custom(format!("Invalid decimal: {}", s)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_decimal() {
        let json = r#"{"value": "12345.67"}"#;
        #[derive(Deserialize)]
        struct Test {
            #[serde(deserialize_with = "deserialize_decimal")]
            value: Decimal,
        }
        let result: Test = serde_json::from_str(json).unwrap();
        assert_eq!(result.value, Decimal::new(1234567, 2));
    }

    #[test]
    fn test_deserialize_empty_decimal() {
        let json = r#"{"value": ""}"#;
        #[derive(Deserialize)]
        struct Test {
            #[serde(deserialize_with = "deserialize_decimal")]
            value: Decimal,
        }
        let result: Test = serde_json::from_str(json).unwrap();
        assert_eq!(result.value, Decimal::ZERO);
    }
}
