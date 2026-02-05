//! Credential 관리 Repository (거래소 중립)
//!
//! Single Source of Truth for credential 복호화 및 거래소 클라이언트 생성.
//!
//! # 설계 원칙
//! - 모든 credential 관련 로직은 이 모듈을 통해서만 처리
//! - OAuth 토큰은 DB에 캐싱하여 rate limit 대응 (1분당 1회 제한)
//! - **거래소 중립**: 특정 거래소에 의존하지 않음

use super::kis_token::KisTokenRepository;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};
use trader_core::CredentialEncryptor;
use trader_core::ExchangeProvider;
use trader_exchange::connector::kis::{KisAccountType, KisKrClient, KisUsClient};
use trader_exchange::connector::{KisConfig, KisOAuth};
use trader_exchange::provider::{KisKrProvider, KisUsProvider};
use uuid::Uuid;

/// 암호화된 credential 구조
///
/// # 보안 설계
/// DB에는 이 구조체 전체가 암호화되어 저장됩니다.
/// APP_KEY, APP_SECRET, ACCOUNT_NUMBER 모두 암호화됩니다.
///
/// # Backward Compatibility
/// account_number는 Optional로, additional 맵에서 fallback 읽기 지원
#[derive(Debug, serde::Deserialize)]
struct EncryptedCredentials {
    api_key: String,
    api_secret: String,
    /// 계좌번호 (최상위 필드, 없으면 additional에서 읽음)
    #[serde(default)]
    account_number: Option<String>,
    #[serde(default)]
    additional: Option<HashMap<String, String>>,
}

impl EncryptedCredentials {
    /// 계좌번호 가져오기 (최상위 필드 우선, 없으면 additional에서)
    fn get_account_number(&self) -> Result<String, String> {
        // 1. 최상위 필드 확인
        if let Some(ref acc) = self.account_number {
            if !acc.is_empty() {
                return Ok(acc.clone());
            }
        }

        // 2. additional 맵에서 확인
        if let Some(ref additional) = self.additional {
            if let Some(acc) = additional.get("account_number") {
                if !acc.is_empty() {
                    return Ok(acc.clone());
                }
            }
        }

        Err("account_number가 없습니다. DB credential을 확인하세요.".to_string())
    }
}

/// DB에서 조회한 credential row
#[derive(sqlx::FromRow)]
struct CredentialRow {
    encrypted_credentials: Vec<u8>,
    encryption_nonce: Vec<u8>,
    is_testnet: bool,
    settings: Option<serde_json::Value>,
    exchange_name: String,
}

/// 거래소 Provider 쌍 (거래소 중립)
pub struct ExchangeProviderPair {
    pub kr: Arc<dyn trader_core::ExchangeProvider>,
    pub us: Arc<dyn trader_core::ExchangeProvider>,
}

/// ISA 계좌 여부 판단
fn is_isa_account(settings: &Option<serde_json::Value>, exchange_name: &str) -> bool {
    // settings에 account_type 필드 확인
    if let Some(settings) = settings {
        if let Some(account_type) = settings.get("account_type").and_then(|v| v.as_str()) {
            if account_type == "isa" {
                return true;
            }
        }
    }

    // exchange_name에 "ISA" 포함 여부 확인
    exchange_name.to_uppercase().contains("ISA")
}

/// Active credential ID 조회
///
/// app_settings 테이블에서 active_credential_id를 조회합니다.
pub async fn get_active_credential_id(pool: &PgPool) -> Result<Uuid, String> {
    let row: (String,) = sqlx::query_as(
        "SELECT setting_value FROM app_settings WHERE setting_key = 'active_credential_id' LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Active credential 조회 실패: {}", e))?
    .ok_or_else(|| "Active credential이 설정되지 않았습니다.".to_string())?;

    Uuid::parse_str(&row.0).map_err(|e| format!("Invalid credential UUID: {}", e))
}

/// 거래소 Provider 생성 (거래소 중립)
///
/// # Single Source of Truth for Exchange Integration
///
/// 이 함수는 credential로부터 ExchangeProvider를 생성하는 **유일한 원천**입니다.
/// 거래소 특정 타입(KisKrClient 등)을 직접 사용하지 마세요.
///
/// # Arguments
///
/// * `pool` - DB 연결 풀
/// * `encryptor` - Credential 암호화/복호화 관리자
/// * `credential_id` - Credential UUID
/// * `cached_oauth` - 캐시된 OAuth (선택적)
///
/// # Returns
///
/// 거래소 Provider 쌍 (KR, US)
pub async fn create_exchange_providers_from_credential(
    pool: &PgPool,
    encryptor: &CredentialEncryptor,
    credential_id: Uuid,
    cached_oauth: Option<Arc<KisOAuth>>,
) -> Result<ExchangeProviderPair, String> {
    // 1. Credential 조회
    let row: CredentialRow = sqlx::query_as(
        r#"
        SELECT encrypted_credentials, encryption_nonce, is_testnet, settings, exchange_name
        FROM exchange_credentials
        WHERE id = $1 AND exchange_id = 'kis' AND is_active = true
        "#,
    )
    .bind(credential_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Credential 조회 실패: {}", e))?
    .ok_or_else(|| "해당 credential을 찾을 수 없습니다.".to_string())?;

    info!(
        "거래소 계좌 로드: id={}, name={}, is_testnet={}, is_isa={}",
        credential_id,
        row.exchange_name,
        row.is_testnet,
        is_isa_account(&row.settings, &row.exchange_name)
    );

    // 2. Credential 복호화
    let credentials: EncryptedCredentials = encryptor
        .decrypt_json(&row.encrypted_credentials, &row.encryption_nonce)
        .map_err(|e| format!("Credential 복호화 실패: {}", e))?;

    // 3. 계좌번호 추출 (최상위 필드 또는 additional에서 fallback)
    let account_number = credentials.get_account_number()?;

    // 4. 계좌 유형 결정
    let account_type = if row.is_testnet {
        KisAccountType::Paper
    } else if is_isa_account(&row.settings, &row.exchange_name) {
        KisAccountType::RealIsa
    } else {
        KisAccountType::RealGeneral
    };

    info!(
        "거래소 클라이언트 생성: credential_id={}, account_type={:?}, account={}***",
        credential_id,
        account_type,
        if account_number.len() > 4 {
            &account_number[..4]
        } else {
            &account_number
        }
    );

    // 6. KisConfig 생성
    let config = KisConfig::new(
        credentials.api_key.clone(),
        credentials.api_secret.clone(),
        account_number.clone(),
        account_type,
    );

    // 7. OAuth 생성 (캐시된 것이 있으면 재사용)
    let oauth = if let Some(cached) = cached_oauth {
        info!("OAuth 캐시 재사용: credential_id={}", credential_id);
        cached
    } else {
        let new_oauth =
            Arc::new(KisOAuth::new(config.clone()).map_err(|e| format!("OAuth 생성 실패: {}", e))?);

        // DB에서 유효한 토큰 조회 (rate limit 대응)
        let environment = if row.is_testnet { "paper" } else { "real" };
        if let Some(cached_token) =
            KisTokenRepository::load_valid_token(pool, credential_id, environment).await
        {
            // DB에 유효한 토큰이 있으면 OAuth에 설정
            debug!("DB 캐시된 토큰 사용: credential_id={}", credential_id);
            new_oauth.set_cached_token(cached_token).await;
        } else {
            // DB에 유효한 토큰이 없으면 새로 발급
            info!(
                "DB에 유효한 토큰 없음, 새로 발급: credential_id={}",
                credential_id
            );
            let token = new_oauth
                .refresh_and_get_token()
                .await
                .map_err(|e| format!("OAuth 토큰 획득 실패: {}", e))?;

            // 발급받은 토큰을 DB에 저장
            if let Err(e) =
                KisTokenRepository::save_token(pool, credential_id, environment, &token).await
            {
                warn!("토큰 DB 저장 실패 (계속 진행): {}", e);
            }
        }

        new_oauth
    };

    // 8. 클라이언트 생성
    let kr_client = Arc::new(
        KisKrClient::with_shared_oauth(Arc::clone(&oauth))
            .map_err(|e| format!("KR 클라이언트 생성 실패: {}", e))?,
    );

    let us_client = Arc::new(
        KisUsClient::with_shared_oauth(oauth)
            .map_err(|e| format!("US 클라이언트 생성 실패: {}", e))?,
    );

    // 9. ExchangeProvider로 래핑 (거래소 중립)
    let kr_provider: Arc<dyn ExchangeProvider> = Arc::new(KisKrProvider::new(kr_client));
    let us_provider: Arc<dyn ExchangeProvider> = Arc::new(KisUsProvider::new(us_client));

    Ok(ExchangeProviderPair {
        kr: kr_provider,
        us: us_provider,
    })
}

/// KIS-specific API를 위한 내부 헬퍼 (체결 내역 조회 등)
///
/// # 용도
///
/// ExchangeProvider trait에 없는 거래소 특화 API (체결 내역 조회 등)를 위한 헬퍼입니다.
/// 가능한 한 `create_exchange_providers_from_credential()`를 사용하고,
/// 정말 필요한 경우에만 이 함수를 사용하세요.
///
/// # TODO
///
/// 향후 ExchangeProvider trait에 체결 내역 조회 메서드를 추가하면 이 함수는 제거됩니다.
pub async fn create_kis_kr_client_from_credential(
    pool: &PgPool,
    encryptor: &CredentialEncryptor,
    credential_id: Uuid,
) -> Result<Arc<KisKrClient>, String> {
    // create_exchange_providers_from_credential()와 동일한 로직
    let row: CredentialRow = sqlx::query_as(
        r#"
        SELECT encrypted_credentials, encryption_nonce, is_testnet, settings, exchange_name
        FROM exchange_credentials
        WHERE id = $1 AND exchange_id = 'kis' AND is_active = true
        "#,
    )
    .bind(credential_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Credential 조회 실패: {}", e))?
    .ok_or_else(|| "해당 credential을 찾을 수 없습니다.".to_string())?;

    let credentials: EncryptedCredentials = encryptor
        .decrypt_json(&row.encrypted_credentials, &row.encryption_nonce)
        .map_err(|e| format!("Credential 복호화 실패: {}", e))?;

    let account_number = credentials.get_account_number()?;

    let account_type = if row.is_testnet {
        KisAccountType::Paper
    } else if is_isa_account(&row.settings, &row.exchange_name) {
        KisAccountType::RealIsa
    } else {
        KisAccountType::RealGeneral
    };

    let config = KisConfig::new(
        credentials.api_key,
        credentials.api_secret,
        account_number,
        account_type,
    );

    let oauth =
        Arc::new(KisOAuth::new(config.clone()).map_err(|e| format!("OAuth 생성 실패: {}", e))?);

    // DB에서 유효한 토큰 조회 (rate limit 대응)
    let environment = if row.is_testnet { "paper" } else { "real" };
    if let Some(cached_token) =
        KisTokenRepository::load_valid_token(pool, credential_id, environment).await
    {
        // DB에 유효한 토큰이 있으면 OAuth에 설정
        debug!("DB 캐시된 토큰 사용: credential_id={}", credential_id);
        oauth.set_cached_token(cached_token).await;
    } else {
        // DB에 유효한 토큰이 없으면 새로 발급
        info!(
            "DB에 유효한 토큰 없음, 새로 발급: credential_id={}",
            credential_id
        );
        let token = oauth
            .refresh_and_get_token()
            .await
            .map_err(|e| format!("OAuth 토큰 획득 실패: {}", e))?;

        // 발급받은 토큰을 DB에 저장
        if let Err(e) =
            KisTokenRepository::save_token(pool, credential_id, environment, &token).await
        {
            warn!("토큰 DB 저장 실패 (계속 진행): {}", e);
        }
    }

    Ok(Arc::new(
        KisKrClient::with_shared_oauth(oauth)
            .map_err(|e| format!("KR 클라이언트 생성 실패: {}", e))?,
    ))
}
