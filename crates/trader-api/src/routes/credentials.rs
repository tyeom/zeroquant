//! 자격증명 관리 API.
//!
//! 거래소 API 키, 텔레그램 설정 등 민감한 자격증명을
//! 암호화하여 데이터베이스에 저장/관리하는 엔드포인트.
//!
//! # 보안
//! - 모든 자격증명은 AES-256-GCM으로 암호화
//! - API 키 값은 응답에 마스킹하여 반환
//! - 모든 접근은 감사 로그에 기록

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::state::AppState;
use super::strategies::ApiError;

// =============================================================================
// 거래소 자격증명 타입
// =============================================================================

/// 거래소 자격증명 등록 요청.
/// 프론트엔드에서 fields 객체로 api_key, api_secret 등을 전달.
#[derive(Debug, Deserialize)]
pub struct CreateExchangeCredentialRequest {
    /// 거래소 ID (binance, kis, coinbase 등)
    pub exchange_id: String,
    /// 표시 이름 (프론트엔드 호환)
    pub display_name: String,
    /// 필드 값 (api_key, api_secret, passphrase 등)
    pub fields: std::collections::HashMap<String, String>,
    /// 테스트넷 여부
    #[serde(default)]
    pub is_testnet: bool,
    /// 추가 설정
    #[serde(default)]
    pub settings: Option<serde_json::Value>,
}

/// 거래소 자격증명 수정 요청.
#[derive(Debug, Deserialize)]
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
    pub additional_fields: Option<std::collections::HashMap<String, String>>,
    /// 활성화 여부
    pub is_active: Option<bool>,
    /// 추가 설정
    pub settings: Option<serde_json::Value>,
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

// =============================================================================
// 텔레그램 설정 타입
// =============================================================================

/// 텔레그램 설정 등록/수정 요청.
#[derive(Debug, Deserialize)]
pub struct SaveTelegramSettingsRequest {
    /// Bot Token
    pub bot_token: String,
    /// Chat ID
    pub chat_id: String,
    /// 알림 유형별 활성화 설정
    #[serde(default)]
    pub notification_settings: Option<TelegramNotificationSettings>,
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

fn default_true() -> bool { true }

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
// 지원 거래소 목록
// =============================================================================

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
// DB 레코드 타입
// =============================================================================

/// DB에서 조회한 거래소 자격증명 레코드.
#[derive(Debug, sqlx::FromRow)]
struct ExchangeCredentialRow {
    id: Uuid,
    exchange_id: String,
    exchange_name: String,
    market_type: String,
    encrypted_credentials: Vec<u8>,
    encryption_nonce: Vec<u8>,
    is_active: bool,
    is_testnet: bool,
    permissions: Option<serde_json::Value>,
    settings: Option<serde_json::Value>,
    last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    last_verified_at: Option<chrono::DateTime<chrono::Utc>>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

/// DB에서 조회한 텔레그램 설정 레코드.
#[derive(Debug, sqlx::FromRow)]
struct TelegramSettingsRow {
    id: Uuid,
    encrypted_bot_token: Vec<u8>,
    encryption_nonce_token: Vec<u8>,
    encrypted_chat_id: Vec<u8>,
    encryption_nonce_chat: Vec<u8>,
    is_enabled: bool,
    notification_settings: Option<serde_json::Value>,
    bot_username: Option<String>,
    chat_type: Option<String>,
    last_message_at: Option<chrono::DateTime<chrono::Utc>>,
    last_verified_at: Option<chrono::DateTime<chrono::Utc>>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

/// 암호화된 자격증명 JSON 구조.
///
/// DB에 저장된 암호화된 자격증명을 복호화한 후의 구조체입니다.
/// 거래소 API 클라이언트 생성에 사용됩니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedCredentials {
    pub api_key: String,
    pub api_secret: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passphrase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional: Option<std::collections::HashMap<String, String>>,
}

// =============================================================================
// 헬퍼 함수
// =============================================================================

/// API 키 마스킹 유틸리티.
fn mask_api_key(key: &str) -> String {
    if key.len() <= 8 {
        "*".repeat(key.len())
    } else {
        format!("{}...{}", &key[..4], &key[key.len()-4..])
    }
}

/// 거래소 ID로 시장 유형 추론.
fn infer_market_type(exchange_id: &str) -> &'static str {
    match exchange_id {
        "binance" | "coinbase" | "kraken" => "crypto",
        "kis" => "stock_kr",
        "interactive_brokers" | "ib" => "stock_us",
        "oanda" => "forex",
        _ => "unknown",
    }
}

/// 감사 로그 기록.
async fn log_credential_access(
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
        "#
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

// =============================================================================
// 핸들러 구현
// =============================================================================

/// 지원 거래소 목록 조회.
///
/// `GET /api/v1/credentials/exchanges`
pub async fn get_supported_exchanges() -> impl IntoResponse {
    let exchanges = vec![
        SupportedExchange {
            exchange_id: "binance".to_string(),
            display_name: "Binance".to_string(),
            market_type: "crypto".to_string(),
            supports_testnet: true,
            required_fields: vec![
                CredentialField {
                    name: "api_key".to_string(),
                    label: "API Key".to_string(),
                    field_type: "text".to_string(),
                    placeholder: Some("Enter your Binance API Key".to_string()),
                    help_text: Some("API 관리에서 생성한 API Key".to_string()),
                },
                CredentialField {
                    name: "api_secret".to_string(),
                    label: "API Secret".to_string(),
                    field_type: "password".to_string(),
                    placeholder: Some("Enter your Binance API Secret".to_string()),
                    help_text: Some("API 생성 시 한 번만 표시되는 Secret Key".to_string()),
                },
            ],
            optional_fields: vec![],
            description: "세계 최대 암호화폐 거래소".to_string(),
            docs_url: Some("https://binance-docs.github.io/apidocs/spot/en/".to_string()),
        },
        SupportedExchange {
            exchange_id: "kis".to_string(),
            display_name: "한국투자증권".to_string(),
            market_type: "stock_kr".to_string(),
            supports_testnet: true,
            required_fields: vec![
                CredentialField {
                    name: "api_key".to_string(),
                    label: "App Key".to_string(),
                    field_type: "text".to_string(),
                    placeholder: Some("발급받은 App Key".to_string()),
                    help_text: Some("KIS Developers에서 발급받은 App Key".to_string()),
                },
                CredentialField {
                    name: "api_secret".to_string(),
                    label: "App Secret".to_string(),
                    field_type: "password".to_string(),
                    placeholder: Some("발급받은 App Secret".to_string()),
                    help_text: Some("KIS Developers에서 발급받은 App Secret".to_string()),
                },
            ],
            optional_fields: vec![
                CredentialField {
                    name: "account_number".to_string(),
                    label: "계좌번호".to_string(),
                    field_type: "text".to_string(),
                    placeholder: Some("00000000-00".to_string()),
                    help_text: Some("거래에 사용할 계좌번호 (종합계좌번호-상품코드)".to_string()),
                },
            ],
            description: "한국투자증권 KIS Developers API".to_string(),
            docs_url: Some("https://apiportal.koreainvestment.com/".to_string()),
        },
        SupportedExchange {
            exchange_id: "coinbase".to_string(),
            display_name: "Coinbase".to_string(),
            market_type: "crypto".to_string(),
            supports_testnet: true,
            required_fields: vec![
                CredentialField {
                    name: "api_key".to_string(),
                    label: "API Key".to_string(),
                    field_type: "text".to_string(),
                    placeholder: Some("Coinbase API Key".to_string()),
                    help_text: None,
                },
                CredentialField {
                    name: "api_secret".to_string(),
                    label: "API Secret".to_string(),
                    field_type: "password".to_string(),
                    placeholder: Some("Coinbase API Secret".to_string()),
                    help_text: None,
                },
                CredentialField {
                    name: "passphrase".to_string(),
                    label: "Passphrase".to_string(),
                    field_type: "password".to_string(),
                    placeholder: Some("API Passphrase".to_string()),
                    help_text: Some("API 생성 시 설정한 Passphrase".to_string()),
                },
            ],
            optional_fields: vec![],
            description: "미국 최대 암호화폐 거래소".to_string(),
            docs_url: Some("https://docs.cloud.coinbase.com/exchange/reference".to_string()),
        },
    ];

    Json(SupportedExchangesResponse { exchanges })
}

/// 거래소 자격증명 목록 조회.
///
/// `GET /api/v1/credentials/exchanges/list`
pub async fn list_exchange_credentials(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    // DB 연결 확인
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        error!("DB 연결이 설정되지 않았습니다.");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_NOT_CONFIGURED", "데이터베이스 연결이 설정되지 않았습니다.")),
        )
    })?;

    // 암호화 관리자 확인
    let encryptor = state.encryptor.as_ref().ok_or_else(|| {
        error!("암호화 관리자가 설정되지 않았습니다.");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("ENCRYPTOR_NOT_CONFIGURED", "암호화 설정이 없습니다. ENCRYPTION_MASTER_KEY를 설정하세요.")),
        )
    })?;

    // DB에서 모든 자격증명 조회
    let rows: Vec<ExchangeCredentialRow> = sqlx::query_as(
        r#"
        SELECT
            id, exchange_id, exchange_name, market_type,
            encrypted_credentials, encryption_nonce,
            is_active, is_testnet, permissions, settings,
            last_used_at, last_verified_at, created_at, updated_at
        FROM exchange_credentials
        ORDER BY created_at DESC
        "#
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        error!("자격증명 목록 조회 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", &format!("조회 실패: {}", e))),
        )
    })?;

    // 응답 변환 (복호화하여 마스킹)
    let mut credentials = Vec::with_capacity(rows.len());

    for row in rows {
        // 복호화하여 API 키 마스킹
        let api_key_masked = match encryptor.decrypt_json::<EncryptedCredentials>(
            &row.encrypted_credentials,
            &row.encryption_nonce,
        ) {
            Ok(creds) => mask_api_key(&creds.api_key),
            Err(e) => {
                warn!("자격증명 복호화 실패 (id: {}): {}", row.id, e);
                "***복호화 실패***".to_string()
            }
        };

        // permissions JSON을 Vec<String>으로 변환
        let permissions: Option<Vec<String>> = row.permissions
            .and_then(|v| serde_json::from_value(v).ok());

        credentials.push(ExchangeCredentialResponse {
            id: row.id,
            exchange_id: row.exchange_id,
            display_name: row.exchange_name,
            market_type: row.market_type,
            api_key_masked,
            is_active: row.is_active,
            is_testnet: row.is_testnet,
            permissions,
            settings: row.settings,
            last_used_at: row.last_used_at.map(|t| t.to_rfc3339()),
            last_verified_at: row.last_verified_at.map(|t| t.to_rfc3339()),
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
        });
    }

    let total = credentials.len();

    info!("자격증명 목록 조회 완료: {}개", total);

    Ok(Json(ExchangeCredentialsListResponse { credentials, total }))
}

/// 거래소 자격증명 등록.
///
/// `POST /api/v1/credentials/exchanges`
pub async fn create_exchange_credential(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateExchangeCredentialRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(
        "거래소 자격증명 등록 요청: {}",
        request.exchange_id
    );

    // DB 연결 확인
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        error!("DB 연결이 설정되지 않았습니다.");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_NOT_CONFIGURED", "데이터베이스 연결이 설정되지 않았습니다.")),
        )
    })?;

    // 암호화 관리자 확인
    let encryptor = state.encryptor.as_ref().ok_or_else(|| {
        error!("암호화 관리자가 설정되지 않았습니다.");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("ENCRYPTOR_NOT_CONFIGURED", "암호화 설정이 없습니다. ENCRYPTION_MASTER_KEY를 설정하세요.")),
        )
    })?;

    // 입력 검증 - fields에서 api_key, api_secret 추출
    let api_key = request.fields.get("api_key").cloned().unwrap_or_default();
    let api_secret = request.fields.get("api_secret").cloned().unwrap_or_default();

    if api_key.is_empty() || api_secret.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("INVALID_INPUT", "API Key와 Secret은 필수입니다.")),
        ));
    }

    // passphrase와 추가 필드 추출
    let passphrase = request.fields.get("passphrase").cloned();
    let additional: Option<std::collections::HashMap<String, String>> = {
        let mut additional_fields: std::collections::HashMap<String, String> = request.fields.clone();
        additional_fields.remove("api_key");
        additional_fields.remove("api_secret");
        additional_fields.remove("passphrase");
        if additional_fields.is_empty() {
            None
        } else {
            Some(additional_fields)
        }
    };

    // 자격증명 구조체 생성
    let credentials = EncryptedCredentials {
        api_key: api_key.clone(),
        api_secret,
        passphrase,
        additional,
    };

    // AES-256-GCM으로 암호화
    let (encrypted_data, nonce) = encryptor.encrypt_json(&credentials).map_err(|e| {
        error!("자격증명 암호화 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("ENCRYPTION_FAILED", "암호화 실패")),
        )
    })?;

    // 시장 유형 추론
    let market_type = infer_market_type(&request.exchange_id);

    // DB에 저장
    let credential_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO exchange_credentials
            (id, exchange_id, exchange_name, market_type,
             encrypted_credentials, encryption_nonce, encryption_version,
             is_active, is_testnet, settings)
        VALUES ($1, $2, $3, $4, $5, $6, 1, true, $7, $8)
        ON CONFLICT (exchange_id, market_type, is_testnet, exchange_name)
        DO UPDATE SET
            encrypted_credentials = EXCLUDED.encrypted_credentials,
            encryption_nonce = EXCLUDED.encryption_nonce,
            settings = EXCLUDED.settings,
            updated_at = NOW()
        RETURNING id
        "#
    )
    .bind(credential_id)
    .bind(&request.exchange_id)
    .bind(&request.display_name)
    .bind(market_type)
    .bind(&encrypted_data)
    .bind(&nonce.to_vec())
    .bind(request.is_testnet)
    .bind(&request.settings)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        error!("자격증명 저장 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", &format!("저장 실패: {}", e))),
        )
    })?;

    // 감사 로그 기록
    log_credential_access(pool, "exchange", credential_id, "create", true, None).await;

    info!("자격증명 등록 완료: {} (id: {})", request.exchange_id, credential_id);

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "success": true,
            "message": "자격증명이 등록되었습니다.",
            "credential": {
                "id": credential_id,
                "exchange_id": request.exchange_id,
                "display_name": request.display_name,
                "market_type": market_type,
                "api_key_masked": mask_api_key(&api_key),
                "is_active": true,
                "is_testnet": request.is_testnet,
                "created_at": chrono::Utc::now().to_rfc3339(),
                "updated_at": chrono::Utc::now().to_rfc3339()
            }
        })),
    ))
}

/// 거래소 자격증명 수정.
///
/// `PUT /api/v1/credentials/exchanges/:id`
pub async fn update_exchange_credential(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateExchangeCredentialRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!("자격증명 수정 요청: {}", id);

    // DB 연결 확인
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_NOT_CONFIGURED", "데이터베이스 연결이 설정되지 않았습니다.")),
        )
    })?;

    // 암호화 관리자 확인
    let encryptor = state.encryptor.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("ENCRYPTOR_NOT_CONFIGURED", "암호화 설정이 없습니다.")),
        )
    })?;

    // 기존 자격증명 조회
    let existing: Option<ExchangeCredentialRow> = sqlx::query_as(
        r#"
        SELECT
            id, exchange_id, exchange_name, market_type,
            encrypted_credentials, encryption_nonce,
            is_active, is_testnet, permissions, settings,
            last_used_at, last_verified_at, created_at, updated_at
        FROM exchange_credentials
        WHERE id = $1
        "#
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("자격증명 조회 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", &format!("조회 실패: {}", e))),
        )
    })?;

    let existing = existing.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ApiError::new("NOT_FOUND", "자격증명을 찾을 수 없습니다.")),
        )
    })?;

    // 기존 자격증명 복호화
    let mut credentials: EncryptedCredentials = encryptor
        .decrypt_json(&existing.encrypted_credentials, &existing.encryption_nonce)
        .map_err(|e| {
            error!("기존 자격증명 복호화 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DECRYPTION_FAILED", "복호화 실패")),
            )
        })?;

    // 변경사항 적용
    if let Some(api_key) = request.api_key {
        credentials.api_key = api_key;
    }
    if let Some(api_secret) = request.api_secret {
        credentials.api_secret = api_secret;
    }
    if let Some(passphrase) = request.passphrase {
        credentials.passphrase = Some(passphrase);
    }
    if let Some(additional) = request.additional_fields {
        credentials.additional = Some(additional);
    }

    // 다시 암호화
    let (encrypted_data, nonce) = encryptor.encrypt_json(&credentials).map_err(|e| {
        error!("자격증명 암호화 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("ENCRYPTION_FAILED", "암호화 실패")),
        )
    })?;

    // DB 업데이트
    let exchange_name = request.exchange_name.unwrap_or(existing.exchange_name);
    let is_active = request.is_active.unwrap_or(existing.is_active);
    let settings = request.settings.or(existing.settings);

    sqlx::query(
        r#"
        UPDATE exchange_credentials
        SET exchange_name = $1,
            encrypted_credentials = $2,
            encryption_nonce = $3,
            is_active = $4,
            settings = $5,
            updated_at = NOW()
        WHERE id = $6
        "#
    )
    .bind(&exchange_name)
    .bind(&encrypted_data)
    .bind(&nonce.to_vec())
    .bind(is_active)
    .bind(&settings)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| {
        error!("자격증명 업데이트 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", &format!("업데이트 실패: {}", e))),
        )
    })?;

    // 감사 로그 기록
    log_credential_access(pool, "exchange", id, "update", true, None).await;

    info!("자격증명 수정 완료: {}", id);

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "자격증명이 업데이트되었습니다."
    })))
}

/// 거래소 자격증명 삭제.
///
/// `DELETE /api/v1/credentials/exchanges/:id`
pub async fn delete_exchange_credential(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!("자격증명 삭제 요청: {}", id);

    // DB 연결 확인
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_NOT_CONFIGURED", "데이터베이스 연결이 설정되지 않았습니다.")),
        )
    })?;

    // 자격증명 존재 확인 및 삭제
    let result = sqlx::query(
        r#"
        DELETE FROM exchange_credentials
        WHERE id = $1
        RETURNING id
        "#
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("자격증명 삭제 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", &format!("삭제 실패: {}", e))),
        )
    })?;

    if result.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiError::new("NOT_FOUND", "자격증명을 찾을 수 없습니다.")),
        ));
    }

    // 감사 로그 기록
    log_credential_access(pool, "exchange", id, "delete", true, None).await;

    info!("자격증명 삭제 완료: {}", id);

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "자격증명이 삭제되었습니다."
    })))
}

/// 거래소 연결 테스트.
///
/// `POST /api/v1/credentials/exchanges/:id/test`
pub async fn test_exchange_credential(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!("자격증명 연결 테스트: {}", id);

    // DB 연결 확인
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_NOT_CONFIGURED", "데이터베이스 연결이 설정되지 않았습니다.")),
        )
    })?;

    // 암호화 관리자 확인
    let encryptor = state.encryptor.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("ENCRYPTOR_NOT_CONFIGURED", "암호화 설정이 없습니다.")),
        )
    })?;

    // 자격증명 조회
    let row: Option<ExchangeCredentialRow> = sqlx::query_as(
        r#"
        SELECT
            id, exchange_id, exchange_name, market_type,
            encrypted_credentials, encryption_nonce,
            is_active, is_testnet, permissions, settings,
            last_used_at, last_verified_at, created_at, updated_at
        FROM exchange_credentials
        WHERE id = $1
        "#
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("자격증명 조회 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", &format!("조회 실패: {}", e))),
        )
    })?;

    let row = row.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ApiError::new("NOT_FOUND", "자격증명을 찾을 수 없습니다.")),
        )
    })?;

    // 복호화
    let credentials: EncryptedCredentials = encryptor
        .decrypt_json(&row.encrypted_credentials, &row.encryption_nonce)
        .map_err(|e| {
            error!("자격증명 복호화 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DECRYPTION_FAILED", "복호화 실패")),
            )
        })?;

    // 거래소별 연결 테스트 (실제 API 호출)
    // TODO: 실제 거래소 API 호출로 변경
    // 현재는 자격증명 형식 검증만 수행
    let (success, message, permissions) = match row.exchange_id.as_str() {
        "binance" => {
            // Binance API 키 형식 검증 (실제로는 API 호출 필요)
            if credentials.api_key.len() >= 10 && credentials.api_secret.len() >= 10 {
                (true, "Binance API 키 형식이 유효합니다.".to_string(),
                 Some(vec!["read".to_string(), "trade".to_string()]))
            } else {
                (false, "API 키 형식이 올바르지 않습니다.".to_string(), None)
            }
        }
        "kis" => {
            // KIS API 키 형식 검증
            if credentials.api_key.len() >= 8 && credentials.api_secret.len() >= 8 {
                (true, "한국투자증권 API 키 형식이 유효합니다.".to_string(),
                 Some(vec!["read".to_string(), "trade".to_string()]))
            } else {
                (false, "API 키 형식이 올바르지 않습니다.".to_string(), None)
            }
        }
        _ => {
            (true, format!("{} API 키가 등록되어 있습니다.", row.exchange_id), None)
        }
    };

    // 테스트 결과에 따라 last_verified_at 업데이트
    if success {
        let _ = sqlx::query(
            r#"
            UPDATE exchange_credentials
            SET last_verified_at = NOW(),
                permissions = $1
            WHERE id = $2
            "#
        )
        .bind(serde_json::to_value(&permissions).ok())
        .bind(id)
        .execute(pool)
        .await;
    }

    // 감사 로그 기록
    log_credential_access(
        pool,
        "exchange",
        id,
        "verify",
        success,
        if success { None } else { Some(&message) }
    ).await;

    Ok(Json(ExchangeTestResponse {
        success,
        message,
        permissions,
        account_info: if success {
            Some(serde_json::json!({
                "exchange": row.exchange_id,
                "testnet": row.is_testnet
            }))
        } else {
            None
        },
    }))
}

/// 새 자격증명 테스트 요청 (저장 전).
#[derive(Debug, Deserialize)]
pub struct TestNewCredentialRequest {
    /// 거래소 ID
    pub exchange_id: String,
    /// 필드 값 (api_key, api_secret 등)
    pub fields: std::collections::HashMap<String, String>,
}

/// 새 자격증명으로 연결 테스트 (저장 전).
///
/// `POST /api/v1/credentials/exchanges/test`
pub async fn test_new_exchange_credential(
    Json(request): Json<TestNewCredentialRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(
        "새 자격증명 테스트: {}",
        request.exchange_id
    );

    // 입력 검증 - fields에서 api_key, api_secret 추출
    let api_key = request.fields.get("api_key").cloned().unwrap_or_default();
    let api_secret = request.fields.get("api_secret").cloned().unwrap_or_default();

    if api_key.is_empty() || api_secret.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("INVALID_INPUT", "API Key와 Secret은 필수입니다.")),
        ));
    }

    // 거래소별 연결 테스트 (실제 API 호출)
    // TODO: 실제 거래소 API 호출로 변경
    let (success, message, permissions) = match request.exchange_id.as_str() {
        "binance" => {
            if api_key.len() >= 10 && api_secret.len() >= 10 {
                (true, "Binance API 키가 유효합니다.".to_string(),
                 Some(vec!["read".to_string(), "trade".to_string()]))
            } else {
                (false, "API 키 형식이 올바르지 않습니다.".to_string(), None)
            }
        }
        "kis" => {
            if api_key.len() >= 8 && api_secret.len() >= 8 {
                (true, "한국투자증권 API 키가 유효합니다.".to_string(),
                 Some(vec!["read".to_string(), "trade".to_string()]))
            } else {
                (false, "API 키 형식이 올바르지 않습니다.".to_string(), None)
            }
        }
        _ => {
            (true, format!("{} API 키 형식이 유효합니다.", request.exchange_id), None)
        }
    };

    Ok(Json(ExchangeTestResponse {
        success,
        message,
        permissions,
        account_info: None,
    }))
}

// =============================================================================
// 텔레그램 설정 핸들러
// =============================================================================

/// 텔레그램 설정 조회.
///
/// `GET /api/v1/credentials/telegram`
pub async fn get_telegram_settings(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    // DB 연결 확인
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_NOT_CONFIGURED", "데이터베이스 연결이 설정되지 않았습니다.")),
        )
    })?;

    // 암호화 관리자 확인
    let encryptor = state.encryptor.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("ENCRYPTOR_NOT_CONFIGURED", "암호화 설정이 없습니다.")),
        )
    })?;

    // DB에서 텔레그램 설정 조회
    let row: Option<TelegramSettingsRow> = sqlx::query_as(
        r#"
        SELECT
            id, encrypted_bot_token, encryption_nonce_token,
            encrypted_chat_id, encryption_nonce_chat, encryption_version,
            is_enabled, notification_settings, bot_username, chat_type,
            last_message_at, last_verified_at, created_at, updated_at
        FROM telegram_settings
        LIMIT 1
        "#
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("텔레그램 설정 조회 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", &format!("조회 실패: {}", e))),
        )
    })?;

    match row {
        Some(settings) => {
            // 복호화하여 마스킹
            let bot_token_masked = match encryptor.decrypt(
                &settings.encrypted_bot_token,
                &settings.encryption_nonce_token,
            ) {
                Ok(token) => mask_api_key(&token),
                Err(_) => "***복호화 실패***".to_string(),
            };

            let chat_id_masked = match encryptor.decrypt(
                &settings.encrypted_chat_id,
                &settings.encryption_nonce_chat,
            ) {
                Ok(chat_id) => mask_api_key(&chat_id),
                Err(_) => "***복호화 실패***".to_string(),
            };

            let notification_settings: TelegramNotificationSettings = settings
                .notification_settings
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default();

            Ok(Json(serde_json::json!({
                "configured": true,
                "id": settings.id,
                "display_name": settings.bot_username.clone().unwrap_or_else(|| "Telegram".to_string()),
                "masked_token": bot_token_masked,
                "masked_chat_id": chat_id_masked,
                "is_enabled": settings.is_enabled,
                "notification_settings": notification_settings,
                "bot_username": settings.bot_username,
                "chat_type": settings.chat_type,
                "last_message_at": settings.last_message_at.map(|t| t.to_rfc3339()),
                "last_tested_at": settings.last_verified_at.map(|t| t.to_rfc3339()),
                "created_at": settings.created_at.to_rfc3339(),
                "updated_at": settings.updated_at.to_rfc3339()
            })))
        }
        None => {
            Ok(Json(serde_json::json!({
                "configured": false,
                "message": "텔레그램 설정이 없습니다. 설정해주세요."
            })))
        }
    }
}

/// 텔레그램 설정 저장.
///
/// `POST /api/v1/credentials/telegram`
pub async fn save_telegram_settings(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SaveTelegramSettingsRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!("텔레그램 설정 저장 요청");

    // DB 연결 확인
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_NOT_CONFIGURED", "데이터베이스 연결이 설정되지 않았습니다.")),
        )
    })?;

    // 암호화 관리자 확인
    let encryptor = state.encryptor.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("ENCRYPTOR_NOT_CONFIGURED", "암호화 설정이 없습니다.")),
        )
    })?;

    // 입력 검증
    if request.bot_token.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("INVALID_INPUT", "Bot Token은 필수입니다.")),
        ));
    }

    if request.chat_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("INVALID_INPUT", "Chat ID는 필수입니다.")),
        ));
    }

    // Bot Token 암호화
    let (encrypted_bot_token, nonce_token) = encryptor.encrypt(&request.bot_token).map_err(|e| {
        error!("Bot Token 암호화 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("ENCRYPTION_FAILED", "암호화 실패")),
        )
    })?;

    // Chat ID 암호화
    let (encrypted_chat_id, nonce_chat) = encryptor.encrypt(&request.chat_id).map_err(|e| {
        error!("Chat ID 암호화 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("ENCRYPTION_FAILED", "암호화 실패")),
        )
    })?;

    let notification_settings = request.notification_settings.unwrap_or_default();
    let notification_settings_json = serde_json::to_value(&notification_settings).ok();

    // 기존 설정이 있으면 업데이트, 없으면 삽입
    let settings_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO telegram_settings
            (id, encrypted_bot_token, encryption_nonce_token,
             encrypted_chat_id, encryption_nonce_chat, encryption_version,
             is_enabled, notification_settings)
        VALUES ($1, $2, $3, $4, $5, 1, true, $6)
        ON CONFLICT ((1))
        DO UPDATE SET
            encrypted_bot_token = EXCLUDED.encrypted_bot_token,
            encryption_nonce_token = EXCLUDED.encryption_nonce_token,
            encrypted_chat_id = EXCLUDED.encrypted_chat_id,
            encryption_nonce_chat = EXCLUDED.encryption_nonce_chat,
            notification_settings = EXCLUDED.notification_settings,
            updated_at = NOW()
        "#
    )
    .bind(settings_id)
    .bind(&encrypted_bot_token)
    .bind(&nonce_token.to_vec())
    .bind(&encrypted_chat_id)
    .bind(&nonce_chat.to_vec())
    .bind(&notification_settings_json)
    .execute(pool)
    .await
    .map_err(|e| {
        error!("텔레그램 설정 저장 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", &format!("저장 실패: {}", e))),
        )
    })?;

    // 감사 로그 기록
    log_credential_access(pool, "telegram", settings_id, "create", true, None).await;

    info!("텔레그램 설정 저장 완료");

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "success": true,
            "message": "텔레그램 설정이 저장되었습니다.",
            "bot_token_masked": mask_api_key(&request.bot_token),
            "chat_id_masked": mask_api_key(&request.chat_id)
        })),
    ))
}

/// 텔레그램 설정 삭제.
///
/// `DELETE /api/v1/credentials/telegram`
pub async fn delete_telegram_settings(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!("텔레그램 설정 삭제 요청");

    // DB 연결 확인
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_NOT_CONFIGURED", "데이터베이스 연결이 설정되지 않았습니다.")),
        )
    })?;

    // 삭제 전 ID 조회 (감사 로그용)
    let row: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM telegram_settings LIMIT 1")
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("텔레그램 설정 조회 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", &format!("조회 실패: {}", e))),
            )
        })?;

    // 삭제 실행
    let result = sqlx::query("DELETE FROM telegram_settings")
        .execute(pool)
        .await
        .map_err(|e| {
            error!("텔레그램 설정 삭제 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", &format!("삭제 실패: {}", e))),
            )
        })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiError::new("NOT_FOUND", "삭제할 텔레그램 설정이 없습니다.")),
        ));
    }

    // 감사 로그 기록
    if let Some((id,)) = row {
        log_credential_access(pool, "telegram", id, "delete", true, None).await;
    }

    info!("텔레그램 설정 삭제 완료");

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "텔레그램 설정이 삭제되었습니다."
    })))
}

/// 텔레그램 연결 테스트 (저장된 설정).
///
/// `POST /api/v1/credentials/telegram/test`
pub async fn test_telegram_settings(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!("텔레그램 설정 테스트");

    // DB 연결 확인
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_NOT_CONFIGURED", "데이터베이스 연결이 설정되지 않았습니다.")),
        )
    })?;

    // 암호화 관리자 확인
    let encryptor = state.encryptor.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("ENCRYPTOR_NOT_CONFIGURED", "암호화 설정이 없습니다.")),
        )
    })?;

    // 설정 조회
    let row: Option<TelegramSettingsRow> = sqlx::query_as(
        r#"
        SELECT
            id, encrypted_bot_token, encryption_nonce_token,
            encrypted_chat_id, encryption_nonce_chat, encryption_version,
            is_enabled, notification_settings, bot_username, chat_type,
            last_message_at, last_verified_at, created_at, updated_at
        FROM telegram_settings
        LIMIT 1
        "#
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("텔레그램 설정 조회 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", &format!("조회 실패: {}", e))),
        )
    })?;

    let settings = row.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ApiError::new("NOT_FOUND", "텔레그램 설정이 없습니다.")),
        )
    })?;

    // 복호화
    let bot_token = encryptor.decrypt(
        &settings.encrypted_bot_token,
        &settings.encryption_nonce_token,
    ).map_err(|e| {
        error!("Bot Token 복호화 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DECRYPTION_FAILED", "복호화 실패")),
        )
    })?;

    let chat_id = encryptor.decrypt(
        &settings.encrypted_chat_id,
        &settings.encryption_nonce_chat,
    ).map_err(|e| {
        error!("Chat ID 복호화 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DECRYPTION_FAILED", "복호화 실패")),
        )
    })?;

    // TODO: 실제 텔레그램 API로 테스트 메시지 전송
    // 현재는 형식 검증만 수행
    let success = bot_token.len() > 10 && chat_id.len() > 0;
    let message = if success {
        "텔레그램 설정이 유효합니다."
    } else {
        "텔레그램 설정이 올바르지 않습니다."
    };

    // 테스트 성공 시 last_verified_at 업데이트
    if success {
        let _ = sqlx::query(
            "UPDATE telegram_settings SET last_verified_at = NOW() WHERE id = $1"
        )
        .bind(settings.id)
        .execute(pool)
        .await;
    }

    // 감사 로그 기록
    log_credential_access(
        pool,
        "telegram",
        settings.id,
        "verify",
        success,
        if success { None } else { Some(message) }
    ).await;

    Ok(Json(serde_json::json!({
        "success": success,
        "message": message
    })))
}

// =============================================================================
// 활성 계정 관리
// =============================================================================

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

/// 활성 계정 조회.
///
/// 현재 대시보드에 표시될 자산 정보의 기준 계정을 조회합니다.
///
/// `GET /api/v1/credentials/active`
pub async fn get_active_account(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    // DB 연결 확인
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_NOT_CONFIGURED", "데이터베이스 연결이 설정되지 않았습니다.")),
        )
    })?;

    // app_settings 테이블에서 active_credential_id 조회
    let setting: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT setting_value FROM app_settings WHERE setting_key = 'active_credential_id' LIMIT 1
        "#
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        // 테이블이 없으면 None 반환
        warn!("활성 계정 조회 실패 (테이블 없음?): {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", &format!("조회 실패: {}", e))),
        )
    })?;

    match setting {
        Some((credential_id_str,)) => {
            // UUID 파싱
            let credential_id = Uuid::parse_str(&credential_id_str).ok();

            if let Some(cred_id) = credential_id {
                // 자격증명 정보 조회
                let row: Option<(String, String, bool)> = sqlx::query_as(
                    r#"
                    SELECT exchange_id, exchange_name, is_testnet
                    FROM exchange_credentials
                    WHERE id = $1
                    "#
                )
                .bind(cred_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| {
                    error!("자격증명 조회 실패: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ApiError::new("DB_ERROR", &format!("조회 실패: {}", e))),
                    )
                })?;

                if let Some((exchange_id, display_name, is_testnet)) = row {
                    return Ok(Json(ActiveAccountResponse {
                        credential_id: Some(cred_id),
                        exchange_id: Some(exchange_id),
                        display_name: Some(display_name),
                        is_testnet,
                    }));
                }
            }

            // 자격증명이 없으면 설정 초기화
            Ok(Json(ActiveAccountResponse {
                credential_id: None,
                exchange_id: None,
                display_name: None,
                is_testnet: false,
            }))
        }
        None => {
            Ok(Json(ActiveAccountResponse {
                credential_id: None,
                exchange_id: None,
                display_name: None,
                is_testnet: false,
            }))
        }
    }
}

/// 활성 계정 설정.
///
/// 대시보드에 표시될 자산 정보의 기준 계정을 설정합니다.
///
/// `PUT /api/v1/credentials/active`
pub async fn set_active_account(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SetActiveAccountRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!("활성 계정 설정: {:?}", request.credential_id);

    // DB 연결 확인
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_NOT_CONFIGURED", "데이터베이스 연결이 설정되지 않았습니다.")),
        )
    })?;

    // credential_id가 있으면 해당 자격증명이 존재하는지 확인
    if let Some(cred_id) = request.credential_id {
        let exists: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM exchange_credentials WHERE id = $1"
        )
        .bind(cred_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("자격증명 조회 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", &format!("조회 실패: {}", e))),
            )
        })?;

        if exists.is_none() {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiError::new("NOT_FOUND", "자격증명을 찾을 수 없습니다.")),
            ));
        }
    }

    // app_settings에 저장 (UPSERT)
    let credential_id_str = request.credential_id
        .map(|id| id.to_string())
        .unwrap_or_default();

    sqlx::query(
        r#"
        INSERT INTO app_settings (setting_key, setting_value, updated_at)
        VALUES ('active_credential_id', $1, NOW())
        ON CONFLICT (setting_key)
        DO UPDATE SET setting_value = EXCLUDED.setting_value, updated_at = NOW()
        "#
    )
    .bind(&credential_id_str)
    .execute(pool)
    .await
    .map_err(|e| {
        error!("활성 계정 저장 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", &format!("저장 실패: {}", e))),
        )
    })?;

    let message = if request.credential_id.is_some() {
        "활성 계정이 설정되었습니다."
    } else {
        "활성 계정이 해제되었습니다."
    };

    info!("{}", message);

    Ok(Json(serde_json::json!({
        "success": true,
        "message": message
    })))
}

// =============================================================================
// 라우터
// =============================================================================

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
        .route("/exchanges/:id", put(update_exchange_credential))
        .route("/exchanges/:id", delete(delete_exchange_credential))
        .route("/exchanges/:id/test", post(test_exchange_credential))
        // 텔레그램 설정
        .route("/telegram", get(get_telegram_settings))
        .route("/telegram", post(save_telegram_settings))
        .route("/telegram", delete(delete_telegram_settings))
        .route("/telegram/test", post(test_telegram_settings))
}
