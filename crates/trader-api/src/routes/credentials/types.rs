//! 자격증명 타입 정의.
//!
//! 거래소 API 키, 텔레그램 설정 등 민감한 자격증명 관련 타입들을 정의합니다.
//!
//! # 구조
//! - 거래소 자격증명 요청/응답 타입
//! - 텔레그램 설정 요청/응답 타입
//! - DB 레코드 타입 (내부용)
//! - 헬퍼 함수

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use tracing::warn;
use uuid::Uuid;

// =============================================================================
// 거래소 자격증명 타입
// =============================================================================

/// 거래소 자격증명 등록 요청.
///
/// 프론트엔드에서 fields 객체로 api_key, api_secret 등을 전달합니다.
///
/// # 보안
/// - `Debug` 구현은 민감 필드를 마스킹합니다.
#[derive(Deserialize)]
pub struct CreateExchangeCredentialRequest {
    /// 거래소 ID (binance, kis, coinbase 등)
    pub exchange_id: String,
    /// 표시 이름 (프론트엔드 호환)
    pub display_name: String,
    /// 필드 값 (api_key, api_secret, passphrase 등)
    pub fields: HashMap<String, String>,
    /// 테스트넷 여부
    #[serde(default)]
    pub is_testnet: bool,
    /// 추가 설정
    #[serde(default)]
    pub settings: Option<serde_json::Value>,
}

impl fmt::Debug for CreateExchangeCredentialRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CreateExchangeCredentialRequest")
            .field("exchange_id", &self.exchange_id)
            .field("display_name", &self.display_name)
            .field(
                "fields",
                &format!("[{} redacted fields]", self.fields.len()),
            )
            .field("is_testnet", &self.is_testnet)
            .field("settings", &self.settings)
            .finish()
    }
}

/// 거래소 자격증명 수정 요청.
///
/// # 보안
/// - `Debug` 구현은 민감 필드를 마스킹합니다.
#[derive(Deserialize)]
pub struct UpdateExchangeCredentialRequest {
    /// 거래소 표시 이름
    pub exchange_name: Option<String>,
    /// API Key (변경 시)
    pub api_key: Option<String>,
    /// API Secret (변경 시)
    pub api_secret: Option<String>,
    /// Passphrase (변경 시)
    pub passphrase: Option<String>,
    /// 추가 필드
    pub additional_fields: Option<HashMap<String, String>>,
    /// 활성화 여부
    pub is_active: Option<bool>,
    /// 추가 설정
    pub settings: Option<serde_json::Value>,
}

impl fmt::Debug for UpdateExchangeCredentialRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UpdateExchangeCredentialRequest")
            .field("exchange_name", &self.exchange_name)
            .field("api_key", &self.api_key.as_ref().map(|_| "***REDACTED***"))
            .field(
                "api_secret",
                &self.api_secret.as_ref().map(|_| "***REDACTED***"),
            )
            .field(
                "passphrase",
                &self.passphrase.as_ref().map(|_| "***REDACTED***"),
            )
            .field(
                "additional_fields",
                &self
                    .additional_fields
                    .as_ref()
                    .map(|m| format!("[{} fields]", m.len())),
            )
            .field("is_active", &self.is_active)
            .field("settings", &self.settings)
            .finish()
    }
}

/// 거래소 자격증명 응답 (마스킹됨).
#[derive(Debug, Serialize)]
pub struct ExchangeCredentialResponse {
    pub id: Uuid,
    pub exchange_id: String,
    /// 표시 이름 (프론트엔드 호환)
    pub display_name: String,
    pub market_type: String,
    /// 마스킹된 API Key (예: "abc...xyz")
    pub api_key_masked: String,
    pub is_active: bool,
    pub is_testnet: bool,
    pub permissions: Option<Vec<String>>,
    pub settings: Option<serde_json::Value>,
    pub last_used_at: Option<String>,
    /// 마지막 테스트 시간 (프론트엔드 호환: last_tested_at)
    #[serde(rename = "last_tested_at")]
    pub last_verified_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 거래소 목록 응답.
#[derive(Debug, Serialize)]
pub struct ExchangeCredentialsListResponse {
    pub credentials: Vec<ExchangeCredentialResponse>,
    pub total: usize,
}

/// 거래소 연결 테스트 응답.
#[derive(Debug, Serialize)]
pub struct ExchangeTestResponse {
    pub success: bool,
    pub message: String,
    pub permissions: Option<Vec<String>>,
    pub account_info: Option<serde_json::Value>,
}

/// 지원 거래소 정보.
#[derive(Debug, Serialize)]
pub struct SupportedExchange {
    /// 거래소 ID (프론트엔드 호환)
    pub exchange_id: String,
    /// 거래소 표시 이름 (프론트엔드 호환)
    pub display_name: String,
    pub market_type: String,
    pub supports_testnet: bool,
    pub required_fields: Vec<CredentialField>,
    pub optional_fields: Vec<CredentialField>,
    pub description: String,
    pub docs_url: Option<String>,
}

/// 자격증명 필드 정보.
#[derive(Debug, Serialize)]
pub struct CredentialField {
    pub name: String,
    pub label: String,
    pub field_type: String,
    pub placeholder: Option<String>,
    pub help_text: Option<String>,
}

/// 지원 거래소 목록 응답.
#[derive(Debug, Serialize)]
pub struct SupportedExchangesResponse {
    pub exchanges: Vec<SupportedExchange>,
}

// =============================================================================
// 텔레그램 설정 타입
// =============================================================================

/// 텔레그램 설정 등록/수정 요청.
///
/// # 보안
/// - `Debug` 구현은 민감 필드를 마스킹합니다.
#[derive(Deserialize)]
pub struct SaveTelegramSettingsRequest {
    /// Bot Token
    pub bot_token: String,
    /// Chat ID
    pub chat_id: String,
    /// 알림 유형별 활성화 설정
    #[serde(default)]
    pub notification_settings: Option<TelegramNotificationSettings>,
}

impl fmt::Debug for SaveTelegramSettingsRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SaveTelegramSettingsRequest")
            .field("bot_token", &"***REDACTED***")
            .field("chat_id", &mask_api_key(&self.chat_id))
            .field("notification_settings", &self.notification_settings)
            .finish()
    }
}

/// 텔레그램 알림 설정.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TelegramNotificationSettings {
    #[serde(default = "default_true")]
    pub trade_executed: bool,
    #[serde(default = "default_true")]
    pub order_filled: bool,
    #[serde(default = "default_true")]
    pub position_opened: bool,
    #[serde(default = "default_true")]
    pub position_closed: bool,
    #[serde(default = "default_true")]
    pub stop_loss_triggered: bool,
    #[serde(default = "default_true")]
    pub take_profit_triggered: bool,
    #[serde(default = "default_true")]
    pub daily_summary: bool,
    #[serde(default = "default_true")]
    pub error_alerts: bool,
    #[serde(default = "default_true")]
    pub risk_warnings: bool,
}

/// serde default 함수: true 반환.
fn default_true() -> bool {
    true
}

/// 텔레그램 설정 응답 (마스킹됨).
#[derive(Debug, Serialize)]
pub struct TelegramSettingsResponse {
    pub id: Uuid,
    /// 마스킹된 Bot Token
    pub bot_token_masked: String,
    /// 마스킹된 Chat ID
    pub chat_id_masked: String,
    pub is_enabled: bool,
    pub notification_settings: TelegramNotificationSettings,
    pub bot_username: Option<String>,
    pub chat_type: Option<String>,
    pub last_message_at: Option<String>,
    pub last_verified_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// =============================================================================
// DB 레코드 타입
// =============================================================================

/// DB에서 조회한 거래소 자격증명 레코드.
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct ExchangeCredentialRow {
    pub id: Uuid,
    pub exchange_id: String,
    pub exchange_name: String,
    pub market_type: String,
    pub encrypted_credentials: Vec<u8>,
    pub encryption_nonce: Vec<u8>,
    pub is_active: bool,
    pub is_testnet: bool,
    pub permissions: Option<serde_json::Value>,
    pub settings: Option<serde_json::Value>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_verified_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// DB에서 조회한 텔레그램 설정 레코드.
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct TelegramSettingsRow {
    pub id: Uuid,
    pub encrypted_bot_token: Vec<u8>,
    pub encryption_nonce_token: Vec<u8>,
    pub encrypted_chat_id: Vec<u8>,
    pub encryption_nonce_chat: Vec<u8>,
    pub is_enabled: bool,
    pub notification_settings: Option<serde_json::Value>,
    pub bot_username: Option<String>,
    pub chat_type: Option<String>,
    pub last_message_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_verified_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

// =============================================================================
// 공개 타입
// =============================================================================

/// 암호화된 자격증명 JSON 구조.
///
/// DB에 저장된 암호화된 자격증명을 복호화한 후의 구조체입니다.
/// 거래소 API 클라이언트 생성에 사용됩니다.
///
/// # 보안
/// - `Debug` 구현은 민감 정보를 마스킹합니다.
/// - 로그에 출력해도 실제 값이 노출되지 않습니다.
#[derive(Clone, Serialize, Deserialize)]
pub struct EncryptedCredentials {
    pub api_key: String,
    pub api_secret: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passphrase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional: Option<HashMap<String, String>>,
}

impl fmt::Debug for EncryptedCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EncryptedCredentials")
            .field("api_key", &"***REDACTED***")
            .field("api_secret", &"***REDACTED***")
            .field(
                "passphrase",
                &self.passphrase.as_ref().map(|_| "***REDACTED***"),
            )
            .field(
                "additional",
                &self
                    .additional
                    .as_ref()
                    .map(|m| m.keys().map(|k| format!("{}=***", k)).collect::<Vec<_>>()),
            )
            .finish()
    }
}

/// 활성 계정 응답.
#[derive(Debug, Serialize)]
pub struct ActiveAccountResponse {
    pub credential_id: Option<Uuid>,
    pub exchange_id: Option<String>,
    pub display_name: Option<String>,
    pub is_testnet: bool,
}

/// 활성 계정 설정 요청.
#[derive(Debug, Deserialize)]
pub struct SetActiveAccountRequest {
    pub credential_id: Option<Uuid>,
}

/// 새 자격증명 테스트 요청 (저장 전).
///
/// # 보안
/// - `Debug` 구현은 민감 필드를 마스킹합니다.
#[derive(Deserialize)]
pub struct TestNewCredentialRequest {
    /// 거래소 ID
    pub exchange_id: String,
    /// 필드 값 (api_key, api_secret 등)
    pub fields: HashMap<String, String>,
}

impl fmt::Debug for TestNewCredentialRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TestNewCredentialRequest")
            .field("exchange_id", &self.exchange_id)
            .field(
                "fields",
                &format!("[{} redacted fields]", self.fields.len()),
            )
            .finish()
    }
}

// =============================================================================
// 헬퍼 함수
// =============================================================================

/// API 키 마스킹 유틸리티.
///
/// 8자 이하의 키는 전체를 `*`로 마스킹하고,
/// 그 이상은 앞 4자와 뒤 4자만 표시합니다.
///
/// # Examples
/// ```ignore
/// assert_eq!(mask_api_key("abcd1234efgh5678"), "abcd...5678");
/// assert_eq!(mask_api_key("short"), "*****");
/// ```
pub(crate) fn mask_api_key(key: &str) -> String {
    if key.len() <= 8 {
        "*".repeat(key.len())
    } else {
        format!("{}...{}", &key[..4], &key[key.len() - 4..])
    }
}

/// 거래소 ID로 시장 유형 추론.
///
/// 거래소 ID를 기반으로 해당 거래소의 시장 유형을 반환합니다.
///
/// # Returns
/// - `"crypto"`: 암호화폐 거래소 (binance, coinbase, kraken)
/// - `"stock_kr"`: 한국 주식 (kis)
/// - `"stock_us"`: 미국 주식 (interactive_brokers, ib)
/// - `"forex"`: 외환 (oanda)
/// - `"unknown"`: 알 수 없는 거래소
pub(crate) fn infer_market_type(exchange_id: &str) -> &'static str {
    match exchange_id {
        "binance" | "coinbase" | "kraken" => "crypto",
        "kis" => "stock_kr",
        "interactive_brokers" | "ib" => "stock_us",
        "oanda" => "forex",
        _ => "unknown",
    }
}

/// 감사 로그 기록.
///
/// 자격증명에 대한 접근(생성, 수정, 삭제, 검증)을 로그에 기록합니다.
/// 로그 기록 실패 시에도 에러를 반환하지 않고 경고 로그만 출력합니다.
///
/// # Arguments
/// * `pool` - PostgreSQL 연결 풀
/// * `credential_type` - 자격증명 유형 ("exchange" 또는 "telegram")
/// * `credential_id` - 자격증명 UUID
/// * `action` - 수행된 작업 ("create", "update", "delete", "verify")
/// * `success` - 작업 성공 여부
/// * `error_message` - 실패 시 에러 메시지
pub(crate) async fn log_credential_access(
    pool: &sqlx::PgPool,
    credential_type: &str,
    credential_id: Uuid,
    action: &str,
    success: bool,
    error_message: Option<&str>,
) {
    let result = sqlx::query(
        r#"
        INSERT INTO credential_access_logs
            (credential_type, credential_id, action, success, error_message)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(credential_type)
    .bind(credential_id)
    .bind(action)
    .bind(success)
    .bind(error_message)
    .execute(pool)
    .await;

    if let Err(e) = result {
        warn!("감사 로그 기록 실패: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_api_key_short() {
        assert_eq!(mask_api_key("abc"), "***");
        assert_eq!(mask_api_key("12345678"), "********");
    }

    #[test]
    fn test_mask_api_key_long() {
        assert_eq!(mask_api_key("abcdefghij"), "abcd...ghij");
        assert_eq!(mask_api_key("123456789012345"), "1234...2345");
    }

    #[test]
    fn test_infer_market_type() {
        assert_eq!(infer_market_type("binance"), "crypto");
        assert_eq!(infer_market_type("coinbase"), "crypto");
        assert_eq!(infer_market_type("kraken"), "crypto");
        assert_eq!(infer_market_type("kis"), "stock_kr");
        assert_eq!(infer_market_type("interactive_brokers"), "stock_us");
        assert_eq!(infer_market_type("ib"), "stock_us");
        assert_eq!(infer_market_type("oanda"), "forex");
        assert_eq!(infer_market_type("unknown_exchange"), "unknown");
    }

    #[test]
    fn test_telegram_notification_settings_default() {
        let settings = TelegramNotificationSettings::default();
        // Default trait implementation sets all to false
        assert!(!settings.trade_executed);
        assert!(!settings.order_filled);
    }

    #[test]
    fn test_telegram_notification_settings_deserialize_with_defaults() {
        let json = r#"{}"#;
        let settings: TelegramNotificationSettings = serde_json::from_str(json).unwrap();
        // When deserializing, serde default functions should set to true
        assert!(settings.trade_executed);
        assert!(settings.order_filled);
        assert!(settings.position_opened);
        assert!(settings.position_closed);
        assert!(settings.stop_loss_triggered);
        assert!(settings.take_profit_triggered);
        assert!(settings.daily_summary);
        assert!(settings.error_alerts);
        assert!(settings.risk_warnings);
    }
}
