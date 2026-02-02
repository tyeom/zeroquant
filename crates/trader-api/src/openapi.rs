//! OpenAPI 문서화 설정.
//!
//! utoipa를 사용하여 REST API의 OpenAPI 3.0 스펙을 생성합니다.
//! Swagger UI는 `/swagger-ui` 경로에서 사용 가능합니다.
//!
//! # 자동 생성 구조
//!
//! 각 라우트 모듈은 자체 스키마를 정의하고, 중앙 `ApiDoc`에서 자동으로 집계합니다.
//! 새로운 엔드포인트를 추가할 때:
//!
//! 1. 응답/요청 타입에 `#[derive(ToSchema)]` 추가
//! 2. 핸들러에 `#[utoipa::path(...)]` 어노테이션 추가
//! 3. 이 파일의 `components(schemas(...))` 및 `paths(...)` 섹션에 추가
//!
//! # 외부 타입 처리
//!
//! 외부 크레이트의 타입은 두 가지 방법으로 처리:
//! - 해당 크레이트에 `ToSchema` 구현 추가
//! - 또는 `#[schema(value_type = Object)]` 사용하여 JSON 객체로 처리

use axum::Router;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

// ==================== 각 모듈에서 스키마 Import ====================

use crate::routes::{
    // Strategies 모듈
    strategies::{ApiError, StrategyListItem},
    // Health 모듈
    ComponentHealth,
    ComponentStatus,
    // Monitoring 모듈
    ErrorRecordDto,
    ErrorsResponse,
    HealthResponse,
    // Screening 모듈
    MomentumResponse,
    ScreeningRequest,
    ScreeningResponse,
    StatsResponse,
    StrategiesListResponse,
};

// ==================== OpenAPI 문서 정의 ====================

/// Trader API 문서.
///
/// 모든 엔드포인트와 스키마를 포함하는 OpenAPI 3.0 스펙입니다.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "ZeroQuant Trading API",
        version = "0.4.4",
        description = r#"
# ZeroQuant 트레이딩 봇 REST API

전략 관리, 백테스트, 포트폴리오 분석을 위한 REST API입니다.

## 주요 기능

- **전략 관리**: 트레이딩 전략 생성, 조회, 시작/중지
- **백테스트**: 과거 데이터 기반 전략 성과 분석
- **포트폴리오**: 실시간 포트폴리오 상태 조회
- **시장 데이터**: 실시간 시세 및 차트 데이터
- **모니터링**: 에러 추적 및 시스템 상태 모니터링
- **스크리닝**: 종목 스크리닝 및 필터링
- **ML**: 머신러닝 모델 훈련 및 배포

## 인증

대부분의 엔드포인트는 JWT Bearer 토큰 인증이 필요합니다.
`Authorization: Bearer <token>` 헤더를 포함하세요.

## 심볼 동기화

- **KRX**: `POST /api/v1/dataset/sync/krx` - 한국 거래소 심볼
- **EODData**: `POST /api/v1/dataset/sync/eod` - 해외 거래소 심볼
"#,
        license(name = "MIT", url = "https://opensource.org/licenses/MIT"),
        contact(
            name = "ZeroQuant Team",
            url = "https://github.com/user/zeroquant"
        )
    ),
    servers(
        (url = "http://localhost:3000", description = "로컬 개발 서버"),
    ),
    tags(
        (name = "health", description = "헬스 체크 - 서버 상태 확인"),
        (name = "strategies", description = "전략 관리 - 트레이딩 전략 CRUD"),
        (name = "orders", description = "주문 관리 - 주문 생성/조회/취소"),
        (name = "positions", description = "포지션 - 현재 보유 포지션 조회"),
        (name = "portfolio", description = "포트폴리오 - 계좌 잔고 및 요약"),
        (name = "backtest", description = "백테스트 - 전략 과거 성과 분석"),
        (name = "analytics", description = "분석 - 성과 지표 및 차트"),
        (name = "patterns", description = "패턴 - 캔들/차트 패턴 인식"),
        (name = "market", description = "시장 - 시장 상태 및 시세"),
        (name = "credentials", description = "자격증명 - API 키 관리"),
        (name = "notifications", description = "알림 - 텔레그램 등 알림 설정"),
        (name = "ml", description = "ML - 머신러닝 모델 훈련"),
        (name = "dataset", description = "데이터셋 - 심볼 동기화 및 데이터 관리"),
        (name = "journal", description = "매매일지 - 체결 내역 및 손익 분석"),
        (name = "screening", description = "스크리닝 - 종목 필터링"),
        (name = "simulation", description = "시뮬레이션 - 모의 거래"),
        (name = "monitoring", description = "모니터링 - 에러 추적 및 시스템 상태")
    ),
    // ==================== 스키마 등록 ====================
    components(
        schemas(
            // ===== Health =====
            HealthResponse,
            ComponentHealth,
            ComponentStatus,

            // ===== Common =====
            ApiError,

            // ===== Strategies =====
            StrategiesListResponse,
            StrategyListItem,

            // ===== Monitoring =====
            ErrorsResponse,
            ErrorRecordDto,
            StatsResponse,

            // ===== Screening =====
            ScreeningRequest,
            ScreeningResponse,
            MomentumResponse,
        )
    ),
    // ==================== 경로 등록 ====================
    paths(
        // ===== Health =====
        crate::routes::health::health_check,
        crate::routes::health::health_ready,

        // ===== Strategies =====
        crate::routes::strategies::list_strategies,

        // ===== Monitoring =====
        crate::routes::monitoring::list_errors,
        crate::routes::monitoring::list_critical_errors,
        crate::routes::monitoring::get_error_by_id,
        crate::routes::monitoring::get_stats,
        crate::routes::monitoring::reset_stats,
        crate::routes::monitoring::clear_errors,
        crate::routes::monitoring::get_summary,

        // ===== Screening =====
        crate::routes::screening::run_screening,
        crate::routes::screening::list_presets,
        crate::routes::screening::run_preset_screening,
        crate::routes::screening::run_momentum_screening,
    )
)]
pub struct ApiDoc;

// ==================== Swagger UI 라우터 ====================

/// Swagger UI 라우터 생성.
///
/// 다음 경로에 문서 UI를 마운트합니다:
/// - `/swagger-ui` - Swagger UI 대화형 문서
/// - `/api-docs/openapi.json` - OpenAPI JSON 스펙
pub fn swagger_ui_router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    SwaggerUi::new("/swagger-ui")
        .url("/api-docs/openapi.json", ApiDoc::openapi())
        .into()
}

// ==================== 테스트 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openapi_spec_valid() {
        let spec = ApiDoc::openapi();
        let json = serde_json::to_string_pretty(&spec).unwrap();

        // 기본 정보 확인
        assert!(json.contains("ZeroQuant Trading API"));
        assert!(json.contains("0.4.4"));

        // 태그 확인
        assert!(json.contains("health"));
        assert!(json.contains("strategies"));
        assert!(json.contains("monitoring"));
        assert!(json.contains("screening"));

        // 경로 확인
        assert!(json.contains("/health"));
        assert!(json.contains("/health/ready"));
        assert!(json.contains("/api/v1/monitoring/errors"));
        assert!(json.contains("/api/v1/screening"));
    }

    #[test]
    fn test_swagger_ui_router_creates() {
        let _router: Router<()> = swagger_ui_router();
    }

    #[test]
    fn test_openapi_contains_schemas() {
        let spec = ApiDoc::openapi();
        let json = serde_json::to_string(&spec).unwrap();

        // 스키마 확인
        assert!(json.contains("HealthResponse"));
        assert!(json.contains("ErrorsResponse"));
        assert!(json.contains("ScreeningRequest"));
        assert!(json.contains("ApiError"));
    }
}
