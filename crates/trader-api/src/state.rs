//! 모든 핸들러에서 공유되는 애플리케이션 상태.
//!
//! AppState는 모든 API 핸들러에서 공유되는 상태를 관리합니다.
//! Arc로 래핑되어 여러 요청 간에 안전하게 공유됩니다.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use trader_strategy::StrategyEngine;
use trader_risk::RiskManager;
use trader_execution::OrderExecutor;
use trader_exchange::connector::kis::{KisKrClient, KisUsClient, KisOAuth};
use trader_core::crypto::CredentialEncryptor;

use crate::websocket::{SharedSubscriptionManager, ServerMessage};

/// KIS 국내/해외 클라이언트 쌍.
///
/// 토큰 재사용을 위해 credential_id별로 캐싱됩니다.
pub struct KisClientPair {
    pub kr: Arc<KisKrClient>,
    pub us: Arc<KisUsClient>,
}

impl KisClientPair {
    /// 새 클라이언트 쌍 생성.
    pub fn new(kr: KisKrClient, us: KisUsClient) -> Self {
        Self {
            kr: Arc::new(kr),
            us: Arc::new(us),
        }
    }
}

/// 애플리케이션 공유 상태.
///
/// 이 구조체는 모든 API 핸들러에서 접근할 수 있는 공유 리소스를 포함합니다.
/// Axum의 State extractor를 통해 핸들러에 주입됩니다.
#[derive(Clone)]
pub struct AppState {
    /// 전략 실행 엔진 - 전략 등록, 시작/중지, 신호 처리
    pub strategy_engine: Arc<RwLock<StrategyEngine>>,

    /// 리스크 매니저 - 주문 검증, 일일 손실 한도, 변동성 필터
    pub risk_manager: Arc<RwLock<RiskManager>>,

    /// 주문 실행기 - 신호→주문 변환, 포지션 추적
    pub executor: Arc<RwLock<OrderExecutor>>,

    /// 데이터베이스 연결 풀 (TimescaleDB/PostgreSQL)
    pub db_pool: Option<sqlx::PgPool>,

    /// Redis 연결 (캐싱 및 세션 관리용)
    pub redis: Option<redis::Client>,

    /// KIS 국내 주식 클라이언트 (한국투자증권 API)
    pub kis_kr_client: Option<Arc<KisKrClient>>,

    /// KIS 해외 주식 클라이언트 (한국투자증권 API - 미국 등)
    pub kis_us_client: Option<Arc<KisUsClient>>,

    /// credential_id별 KIS 클라이언트 캐시.
    ///
    /// 매 요청마다 새 클라이언트를 생성하면 토큰 발급 제한(1분 1회)에 걸리므로,
    /// 캐시된 클라이언트를 재사용합니다.
    pub kis_clients_cache: Arc<RwLock<HashMap<Uuid, Arc<KisClientPair>>>>,

    /// app_key 기반 KisOAuth 캐시 (토큰 공유).
    ///
    /// 동일한 app_key를 사용하는 모든 클라이언트(국내/해외, 실계좌/모의투자)가
    /// OAuth를 공유하여 토큰 발급 rate limit (1분 1회)을 우회합니다.
    ///
    /// 키: "app_key:environment" (예: "PSxxxxxx:real" 또는 "PSxxxxxx:paper")
    pub kis_oauth_cache: Arc<RwLock<HashMap<String, Arc<KisOAuth>>>>,

    /// 자격증명 암호화 관리자 (AES-256-GCM)
    pub encryptor: Option<Arc<CredentialEncryptor>>,

    /// WebSocket 구독 관리자 - 실시간 이벤트 브로드캐스트
    pub subscriptions: Option<SharedSubscriptionManager>,

    /// 서버 시작 시간 (업타임 계산용)
    pub started_at: chrono::DateTime<chrono::Utc>,

    /// API 버전
    pub version: String,
}

impl AppState {
    /// 새로운 AppState 생성.
    ///
    /// # 인자
    /// * `strategy_engine` - 전략 실행 엔진
    /// * `risk_manager` - 리스크 매니저
    /// * `executor` - 주문 실행기
    pub fn new(
        strategy_engine: StrategyEngine,
        risk_manager: RiskManager,
        executor: OrderExecutor,
    ) -> Self {
        // 환경변수에서 암호화 마스터 키 로드 시도
        let encryptor = std::env::var("ENCRYPTION_MASTER_KEY")
            .ok()
            .and_then(|key| CredentialEncryptor::new(&key).ok())
            .map(Arc::new);

        Self {
            strategy_engine: Arc::new(RwLock::new(strategy_engine)),
            risk_manager: Arc::new(RwLock::new(risk_manager)),
            executor: Arc::new(RwLock::new(executor)),
            db_pool: None,
            redis: None,
            kis_kr_client: None,
            kis_us_client: None,
            kis_clients_cache: Arc::new(RwLock::new(HashMap::new())),
            kis_oauth_cache: Arc::new(RwLock::new(HashMap::new())),
            encryptor,
            subscriptions: None,
            started_at: chrono::Utc::now(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// 자격증명 암호화 관리자 설정.
    pub fn with_encryptor(mut self, encryptor: CredentialEncryptor) -> Self {
        self.encryptor = Some(Arc::new(encryptor));
        self
    }

    /// 암호화 관리자 설정 여부 확인.
    pub fn has_encryptor(&self) -> bool {
        self.encryptor.is_some()
    }

    /// 데이터베이스 연결 설정.
    pub fn with_db_pool(mut self, pool: sqlx::PgPool) -> Self {
        self.db_pool = Some(pool);
        self
    }

    /// Redis 연결 설정.
    pub fn with_redis(mut self, client: redis::Client) -> Self {
        self.redis = Some(client);
        self
    }

    /// KIS 국내 주식 클라이언트 설정.
    ///
    /// 한국투자증권 API를 통해 국내 주식/ETF 거래를 가능하게 합니다.
    pub fn with_kis_kr_client(mut self, client: KisKrClient) -> Self {
        self.kis_kr_client = Some(Arc::new(client));
        self
    }

    /// KIS 해외 주식 클라이언트 설정.
    ///
    /// 한국투자증권 API를 통해 해외(미국) 주식/ETF 거래를 가능하게 합니다.
    pub fn with_kis_us_client(mut self, client: KisUsClient) -> Self {
        self.kis_us_client = Some(Arc::new(client));
        self
    }

    /// WebSocket 구독 관리자 설정.
    ///
    /// REST API에서 실시간 이벤트를 브로드캐스트할 수 있게 합니다.
    pub fn with_subscriptions(mut self, subscriptions: SharedSubscriptionManager) -> Self {
        self.subscriptions = Some(subscriptions);
        self
    }

    /// WebSocket 메시지 브로드캐스트.
    ///
    /// 연결된 모든 클라이언트에게 메시지를 전송합니다.
    /// subscriptions가 설정되지 않은 경우 무시됩니다.
    pub fn broadcast(&self, message: ServerMessage) {
        if let Some(ref subs) = self.subscriptions {
            let _ = subs.broadcast(message);
        }
    }

    /// WebSocket 구독 관리자 설정 여부 확인.
    pub fn has_subscriptions(&self) -> bool {
        self.subscriptions.is_some()
    }

    /// 서버 업타임(초) 반환.
    pub fn uptime_secs(&self) -> i64 {
        chrono::Utc::now()
            .signed_duration_since(self.started_at)
            .num_seconds()
    }

    /// 데이터베이스 연결 상태 확인.
    pub async fn is_db_healthy(&self) -> bool {
        if let Some(pool) = &self.db_pool {
            sqlx::query("SELECT 1")
                .fetch_one(pool)
                .await
                .is_ok()
        } else {
            false
        }
    }

    /// Redis 연결 상태 확인.
    pub async fn is_redis_healthy(&self) -> bool {
        if let Some(client) = &self.redis {
            if let Ok(mut conn) = client.get_multiplexed_async_connection().await {
                redis::cmd("PING")
                    .query_async::<String>(&mut conn)
                    .await
                    .is_ok()
            } else {
                false
            }
        } else {
            false
        }
    }

    /// KIS 국내 클라이언트 설정 여부 확인.
    pub fn has_kis_kr_client(&self) -> bool {
        self.kis_kr_client.is_some()
    }

    /// KIS 해외 클라이언트 설정 여부 확인.
    pub fn has_kis_us_client(&self) -> bool {
        self.kis_us_client.is_some()
    }

    /// KIS 클라이언트 전체 설정 여부 확인.
    ///
    /// 국내 또는 해외 클라이언트 중 하나라도 설정되어 있으면 true 반환.
    pub fn has_kis_client(&self) -> bool {
        self.has_kis_kr_client() || self.has_kis_us_client()
    }
}

/// 테스트용 AppState 생성 헬퍼.
///
/// 실제 DB 연결 없이 테스트할 수 있는 최소한의 상태를 생성합니다.
#[cfg(any(test, feature = "test-utils"))]
pub fn create_test_state() -> AppState {
    use rust_decimal_macros::dec;
    use trader_strategy::EngineConfig;
    use trader_risk::RiskConfig;
    use trader_execution::ConversionConfig;

    let strategy_engine = StrategyEngine::new(EngineConfig::default());
    let risk_manager = RiskManager::new(RiskConfig::default(), dec!(10000));
    let executor = OrderExecutor::new_complete(
        RiskManager::new(RiskConfig::default(), dec!(10000)),
        "test_exchange",
        ConversionConfig::default(),
    );

    AppState::new(strategy_engine, risk_manager, executor)
}
