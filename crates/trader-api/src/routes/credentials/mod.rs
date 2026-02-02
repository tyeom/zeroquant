//! 자격증명 관리 API.
//!
//! 거래소 API 키, 텔레그램 설정 등 민감한 자격증명을
//! 암호화하여 데이터베이스에 저장/관리하는 엔드포인트.
//!
//! # 보안
//! - 모든 자격증명은 AES-256-GCM으로 암호화
//! - API 키 값은 응답에 마스킹하여 반환
//! - 모든 접근은 감사 로그에 기록
//!
//! # 엔드포인트
//!
//! ## 활성 계정 관리
//! - `GET /api/v1/credentials/active` - 활성 계정 조회
//! - `PUT /api/v1/credentials/active` - 활성 계정 설정
//!
//! ## 거래소 자격증명
//! - `GET /api/v1/credentials/exchanges` - 지원 거래소 목록
//! - `GET /api/v1/credentials/exchanges/list` - 등록된 자격증명 목록
//! - `POST /api/v1/credentials/exchanges` - 자격증명 등록
//! - `PUT /api/v1/credentials/exchanges/:id` - 자격증명 수정
//! - `DELETE /api/v1/credentials/exchanges/:id` - 자격증명 삭제
//! - `POST /api/v1/credentials/exchanges/:id/test` - 연결 테스트
//! - `POST /api/v1/credentials/exchanges/test` - 새 자격증명 테스트
//!
//! ## 텔레그램 설정
//! - `GET /api/v1/credentials/telegram` - 텔레그램 설정 조회
//! - `POST /api/v1/credentials/telegram` - 텔레그램 설정 저장
//! - `DELETE /api/v1/credentials/telegram` - 텔레그램 설정 삭제
//! - `POST /api/v1/credentials/telegram/test` - 연결 테스트

mod active_account;
mod exchange;
mod telegram;
pub mod types;

// Re-export types for external use
pub use types::{
    ActiveAccountResponse, CreateExchangeCredentialRequest, CredentialField, EncryptedCredentials,
    ExchangeCredentialResponse, ExchangeCredentialsListResponse, ExchangeTestResponse,
    SaveTelegramSettingsRequest, SetActiveAccountRequest, SupportedExchange,
    SupportedExchangesResponse, TelegramNotificationSettings, TelegramSettingsResponse,
    TestNewCredentialRequest, UpdateExchangeCredentialRequest,
};

use axum::{
    routing::{delete, get, post, put},
    Router,
};
use std::sync::Arc;

use crate::state::AppState;

use active_account::{get_active_account, set_active_account};
use exchange::{
    create_exchange_credential, delete_exchange_credential, get_supported_exchanges,
    list_exchange_credentials, test_exchange_credential, test_new_exchange_credential,
    update_exchange_credential,
};
use telegram::{
    delete_telegram_settings, get_telegram_settings, save_telegram_settings, test_telegram_settings,
};

/// 자격증명 관리 라우터.
pub fn credentials_router() -> Router<Arc<AppState>> {
    Router::new()
        // 활성 계정 관리
        .route("/active", get(get_active_account))
        .route("/active", put(set_active_account))
        // 지원 거래소 목록
        .route("/exchanges", get(get_supported_exchanges))
        // 거래소 자격증명 CRUD
        .route("/exchanges/list", get(list_exchange_credentials))
        .route("/exchanges", post(create_exchange_credential))
        .route("/exchanges/test", post(test_new_exchange_credential))
        .route("/exchanges/{id}", put(update_exchange_credential))
        .route("/exchanges/{id}", delete(delete_exchange_credential))
        .route("/exchanges/{id}/test", post(test_exchange_credential))
        // 텔레그램 설정
        .route("/telegram", get(get_telegram_settings))
        .route("/telegram", post(save_telegram_settings))
        .route("/telegram", delete(delete_telegram_settings))
        .route("/telegram/test", post(test_telegram_settings))
}
