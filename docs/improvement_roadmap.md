# ZeroQuant 개선 로드맵 (통합본)

> 작성일: 2026-01-31
> 버전: 5.0 (코드 리팩토링 Phase 7 추가)
> 대상 버전: v0.4.1+
> 기준 문서: **code_optimize_suggestion_improved2.md** (가장 최신)

---

## 📋 목차

1. [개요](#개요)
2. [⚠️ 에이전트 구현 가이드라인](#️-에이전트-구현-가이드라인-필독)
3. [완료된 개선사항](#완료된-개선사항)
4. [남은 개선사항](#남은-개선사항)
   - [🔴 Critical](#-critical-높은-효과-즉시-수행)
   - [🟡 High](#-high-중간-효과-1-2주-내)
   - [🟢 Medium](#-medium-낮은-효과-여유-있을-때)
   - [🔵 프론트엔드](#-프론트엔드-개선사항)
   - [🟣 운영 안정성](#-운영-안정성-신규)
   - [🟤 Rust API 최신 패턴](#-rust-api-최신-패턴-context7-검증)
5. [전략 등록 자동화](#전략-등록-자동화-신규)
6. [Repository 상세 설계](#repository-상세-설계)
7. [구현 로드맵](#구현-로드맵)
8. [권장하지 않는 개선](#권장하지-않는-개선)

---

## 개요

### 현재 프로젝트 상태 (v0.4.1)

| 항목 | 수치 |
|------|------|
| Rust 파일 | 170+ |
| Crate 수 | 10개 |
| 전략 수 | 27개 |
| API 라우트 | 17개 |
| 마이그레이션 | 14개 |

---

## ⚠️ 에이전트 구현 가이드라인 (필독)

> **중요**: 이 문서의 코드 예시는 **참조용**입니다. 실제 구현 시 반드시 아래 가이드라인을 준수하세요.

### 🚨 핵심 원칙: 학습 데이터 의존 금지

AI 에이전트(Claude, GPT 등)의 학습 데이터는 **과거 시점**의 정보를 포함합니다.
라이브러리 API는 지속적으로 변경되므로, **학습 데이터 기반 추측으로 코드를 작성하지 마세요**.

### ✅ 구현 전 필수 검증 절차

| 단계 | 작업 | 도구 |
|------|------|------|
| 1 | 대상 라이브러리의 현재 버전 확인 | `Cargo.toml`, `package.json` 확인 |
| 2 | 최신 API 문서 조회 | **Context7**, 공식 문서 |
| 3 | Breaking Changes 확인 | CHANGELOG, Migration Guide |
| 4 | 코드 예시 검증 | 공식 예제 저장소 |

### 📋 라이브러리별 검증 체크리스트

#### Rust (Backend)

```
□ Tokio
  - 현재 버전: Cargo.toml에서 확인
  - Context7 조회: "tokio async patterns" 또는 공식 docs.rs
  - 주의: select!, spawn, channel API 변경 빈번

□ Axum
  - 현재 버전: Cargo.toml에서 확인
  - Context7 조회: "axum middleware error handling"
  - 주의: 0.6 → 0.7에서 Router, State API 대폭 변경됨

□ SQLx
  - 현재 버전: Cargo.toml에서 확인
  - Context7 조회: "sqlx transaction query_as"
  - 주의: query!, query_as! 매크로 동작 변경 가능

□ Serde
  - 안정적이나, derive 매크로 속성 확인 필요
```

#### TypeScript/JavaScript (Frontend)

```
□ SolidJS
  - 현재 버전: package.json에서 확인
  - Context7 조회: "solidjs createStore createResource"
  - 주의: 1.x → 2.x 전환 시 reactivity 변경

□ Vite
  - 현재 버전: package.json에서 확인
  - 설정 파일 구조 변경 빈번
```

### ❌ 금지 사항

1. **버전 미확인 코드 작성**
   ```
   ❌ "tokio 1.x에서는 이렇게 합니다" (버전 미명시)
   ✅ "tokio 1.35 기준으로 Context7에서 확인한 패턴입니다"
   ```

2. **Deprecated API 사용**
   ```
   ❌ 학습 데이터에 있던 과거 API 사용
   ✅ 현재 권장 API를 Context7/공식 문서에서 확인 후 사용
   ```

3. **추측 기반 import 경로**
   ```
   ❌ use tokio::something::Maybe; // 존재 여부 불확실
   ✅ 실제 코드베이스 또는 docs.rs에서 import 경로 확인
   ```

4. **Feature flag 미확인 사용**
   ```
   ❌ tokio의 "full" feature에 포함되어 있을 것으로 가정
   ✅ Cargo.toml의 features 섹션 확인 후 사용
   ```

5. **주석은 한글로 작성**
   ```
   ❌ 주석을 작성할때 영문보다 한글을 이용합니다.
   ✅ 왠만하면 모든 주석은 한글로 작성합니다. 이미 영문이라면 한글로 변경합니다.
   ```

### 🔍 Context7 사용 가이드

```
# 구현 전 반드시 실행
1. resolve-library-id로 라이브러리 ID 획득
2. query-docs로 구체적인 API 패턴 조회

# 예시 쿼리
- "tokio select graceful shutdown pattern"
- "axum 0.7 HandleErrorLayer timeout middleware"
- "sqlx transaction rollback on error"
- "solidjs createStore nested update"
```

### 📝 코드 작성 시 주석 규칙

구현 시 다음 주석을 포함하세요:

```rust
// API 검증: Context7 조회 (2026-01-31)
// Tokio 1.35, Axum 0.7.4 기준
// 참조: https://docs.rs/tokio/latest/tokio/macro.select.html
tokio::select! {
    // ...
}
```

```typescript
// API 검증: Context7 조회 (2026-01-31)
// SolidJS 1.8 기준
// 참조: https://docs.solidjs.com/concepts/stores
const [state, setState] = createStore({ ... });
```

### ⚡ 빠른 검증 명령어

```bash
# Rust 의존성 버전 확인
cargo tree -p tokio
cargo tree -p axum
cargo tree -p sqlx

# Node.js 의존성 버전 확인
npm ls solid-js
npm ls vite

# Rust 문서 로컬 생성 (오프라인 참조)
cargo doc --open --no-deps
```

### 🎯 이 가이드라인의 목적

| 문제 | 해결책 |
|------|--------|
| 에이전트가 2023년 API로 코드 작성 | 구현 전 Context7 필수 조회 |
| Deprecated 함수 사용 | CHANGELOG/Migration Guide 확인 |
| 존재하지 않는 import 경로 | 실제 코드베이스 또는 docs.rs 확인 |
| Feature flag 누락으로 컴파일 실패 | Cargo.toml features 섹션 검증 |

---

## 완료된 개선사항 ✅

### v0.4.0 ~ v0.4.1에서 해결됨

#### 1. 백테스트 모듈 리팩토링 ✅
```
이전: backtest.rs (3,854줄)
현재: backtest/
  ├── mod.rs       (라우터)
  ├── engine.rs    (엔진)
  ├── loader.rs    (데이터 로더)
  ├── types.rs     (타입 정의)
  └── ui_schema.rs (UI 스키마)
```
**효과**: 유지보수성 향상, 모듈별 테스트 용이

#### 2. 유틸리티 모듈 통합 ✅
```
신규 추가: crates/trader-api/src/utils/
  ├── mod.rs
  ├── format.rs        (포맷팅 함수)
  ├── response.rs      (API 응답 헬퍼)
  └── serde_helpers.rs (Serde 헬퍼)
```
**효과**: 코드 중복 감소, 일관성 향상

#### 3. 전략 기본값 상수화 ✅
```
신규 추가: strategies/common/defaults.rs
- 지표 기본값 (RSI, SMA, Bollinger 등)
- 리스크 관리 기본값
```
**효과**: 기본값 한 곳에서 관리

#### 4. SDUI 전략 스키마 ✅
```
신규 추가: config/sdui/strategy_schemas.json (1,732줄)
- 27개 전략별 동적 폼 스키마
```
**효과**: 프론트엔드 동적 폼 렌더링

#### 5. Docker 구성 단순화 ✅
```
이전: 278줄, 9개 서비스, 11개 볼륨
현재: 105줄, 3개 서비스, 3개 볼륨
제거: Prometheus, Grafana, pgAdmin, Redis Commander, trader-api-dev
```
**효과**: 메모리 500MB+ 절감, 유지보수 부담 감소

#### 6. Dockerfile 간소화 ✅
```
이전: 184줄, 5단계 (sccache + mold)
현재: 89줄, 3단계 (cargo-chef만)
```
**효과**: 빌드 복잡도 감소

#### 7. N+1 쿼리 해결 (3곳) ✅
- `equity_history.rs`: 심볼별 루프 → 배치 쿼리
- `ohlcv.rs`: 개별 INSERT → UNNEST 배치
- `equity_history.rs`: 스냅샷 루프 → 배치 함수

#### 8. E2E 테스트 기반 ✅
```
신규 추가:
  - frontend/e2e/risk-management-ui.spec.ts
  - frontend/playwright.config.ts
  - tests/regression_baseline.json
```

#### 9. 심볼 검색 컴포넌트 ✅
```
신규 추가: frontend/src/components/SymbolSearch.tsx
```

#### 10. 에러 핸들링 개선 (Phase 1) ✅
```
신규 추가: crates/trader-api/src/error.rs
- ApiErrorResponse 통합 에러 타입 정의
- 일관된 에러 코드, 메시지, 타임스탬프 제공
- 기존 분산된 에러 타입들 통합
```
**효과**: API 에러 응답 표준화, 디버깅 용이성 향상

#### 11. 대형 파일 분리 ✅
```
완료:
- analytics.rs (2,678줄) → 7개 모듈 (charts, indicators, manager, performance, sync, types)
- credentials.rs (1,615줄) → 5개 모듈 (active_account, exchange, telegram, types)
- Dataset.tsx → SymbolPanel.tsx, format.ts, indicators.ts 분리
- Strategies.tsx → AddStrategyModal.tsx, EditStrategyModal.tsx 분리
```
**효과**: 유지보수성 향상, 컴파일 속도 개선

#### 12. Repository 패턴 확장 ✅
```
신규 추가:
- repository/portfolio.rs
- repository/orders.rs
- repository/positions.rs
- repository/equity_history.rs
- repository/backtest_results.rs
```
**효과**: 코드 재사용, 테스트 용이성, 데이터 접근 계층 표준화

#### 13. Docker → Podman 마이그레이션 ✅
```
변경:
- README.md: Podman 설치 및 사용법 추가
- docker-compose.yml: Podman 호환 주석 추가
```
**효과**: 메모리 ~80% 절감, 데몬리스 실행

---

## 남은 개선사항

### 🔴 Critical (높은 효과, 즉시 수행)

#### 1. 에러 핸들링 개선 (Phase 2) - unwrap() 제거
**현황**: `unwrap()` **159개** 사용 (code_optimize_suggestion_improved2.md 분석 기준)
> ✅ Phase 1 완료: ApiErrorResponse 타입 추가됨
> 🔄 Phase 2 진행 필요: 개별 unwrap() 제거 작업

**🎯 남은 수정 위치**:

| 파일 | 라인 | 현재 코드 | 문제 | 수정 방향 |
|------|------|----------|------|----------|
| `trader-exchange/src/connector/kis/client_kr.rs` | **59** | `.expect("Failed to create HTTP client")` | panic 위험 | `map_err()?` |
| `trader-exchange/src/connector/kis/auth.rs` | **105** | `.expect("Failed to create HTTP client")` | panic 위험 | `map_err()?` |

**예상 시간**: 4시간 (Phase 1 완료로 단축)
**효과**: 프로덕션 안정성 대폭 향상

---

### 🟡 High (중간 효과, 1-2주 내)

#### 3. 비동기 런타임 최적화 (락 홀드 시간)

**현재 문제**:
```rust
// 긴 락 홀드 - 동시성 저하
pub async fn list_backtest_strategies(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let engine = state.strategy_engine.read().await;  // 락 획득
    let all_statuses = engine.get_all_statuses().await;  // 락을 잡고 I/O 수행
    // 많은 계산...
}
```

**해결책 - 최소 락 홀드**:
```rust
pub async fn list_backtest_strategies(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // 1. 최소한의 데이터만 복사
    let statuses = {
        let engine = state.strategy_engine.read().await;
        engine.get_all_statuses().await  // 빠른 복사
    };  // 락 해제

    // 2. 락 없이 계산 수행
    let strategies: Vec<_> = statuses.into_iter()
        .map(|status| /* ... */)
        .collect();

    Json(strategies)
}
```

**예상 시간**: 4시간
**효과**: 동시 요청 처리 능력 향상

---

#### 4. 전략 공통 로직 추출

**현재 문제**:
- 27개 전략이 유사한 코드 패턴 반복 (리밸런싱, 모멘텀 계산 등)
- strategies/xaa.rs (1,103줄), strategies/haa.rs (917줄) 등 유사 로직 포함

**현재 common/ 모듈**:
```
strategies/common/
├── defaults.rs       ✅ 기본값
├── momentum.rs       ✅ 모멘텀 계산
├── rebalance.rs      ✅ 리밸런싱 (709줄)
└── serde_helpers.rs  ✅ 직렬화
```

**추가 권장**:
```
strategies/common/
├── position_sizing.rs    # 포지션 크기 계산
├── risk_checks.rs        # 공통 리스크 체크
└── signal_filters.rs     # 신호 필터링
```

**리팩토링 예시**:
```rust
// Before (각 전략에서 반복)
impl Strategy for XaaStrategy {
    async fn on_market_data(&mut self, data: &MarketData) -> Result<Vec<Signal>> {
        let momentum = self.calculate_momentum(data)?;  // 중복
        let size = self.calculate_position_size()?;     // 중복
        if !self.check_risk_limits()? { return Ok(vec![]); }  // 중복
        // ...
    }
}

// After (공통 컴포넌트 사용)
use trader_strategy::common::{MomentumCalculator, PositionSizer, RiskChecker};

impl Strategy for XaaStrategy {
    async fn on_market_data(&mut self, data: &MarketData) -> Result<Vec<Signal>> {
        let momentum = MomentumCalculator::calculate(&self.config, data)?;
        let size = PositionSizer::calculate(&self.risk_config, &self.portfolio)?;
        if !RiskChecker::validate(&self.risk_limits, &self.portfolio)? {
            return Ok(vec![]);
        }
        // 전략 고유 로직만 작성
        Ok(self.generate_signals(momentum, size)?)
    }
}
```

**예상 시간**: 12시간
**효과**: 코드 중복 제거, 새 전략 개발 속도 향상

---

#### 5. Repository 패턴 확장 ✅ (v0.4.3에서 완료)

**완료 상태**:
```
repository/
├── mod.rs
├── strategies.rs        ✅
├── execution_cache.rs   ✅
├── symbol_info.rs       ✅
├── portfolio.rs         ✅ 신규 추가
├── orders.rs            ✅ 신규 추가
├── positions.rs         ✅ 신규 추가
├── equity_history.rs    ✅ 신규 추가
└── backtest_results.rs  ✅ 신규 추가
```

**효과**: 코드 재사용, 테스트 용이성, 데이터 접근 계층 표준화

---

#### 6. 테스트 추가

**현재 커버리지**:
- 전략 테스트: 107개 ✅
- 통합 테스트: 2개 (제한적)
- API 엔드포인트 테스트: 없음

**추가 필요**:
```rust
// 1. 핵심 전략 단위 테스트
#[tokio::test]
async fn test_grid_buy_signal_at_lower_level() { ... }

// 2. API 엔드포인트 테스트
#[tokio::test]
async fn test_list_strategies_endpoint() { ... }

// 3. Repository 테스트
#[sqlx::test]
async fn test_create_strategy(pool: PgPool) { ... }
```

**목표**:
- 핵심 전략: Grid, RSI, Bollinger, VolatilityBreakout
- API: strategies, backtest, portfolio
- Repository: 새로 추가되는 것들

**예상 시간**: 16시간
**효과**: 리그레션 방지, 코드 신뢰성

---

#### 7. Redis 캐싱 전략

**현재**: Redis가 설정되어 있지만 제한적 사용

**제안 캐싱 대상**:

| 대상 | TTL | 이유 |
|------|-----|------|
| 전략 목록 | 5분 | 자주 조회, 드물게 변경 |
| 심볼 정보 | 1시간 | 거의 변경 없음 |
| 백테스트 결과 | 영구 | 동일 파라미터 재요청 |
| 실시간 시세 | 1초 | 빈번한 업데이트 |

**구현 예시**:
```rust
pub struct CacheLayer {
    redis: redis::Client,
}

impl CacheLayer {
    pub async fn get_or_fetch<T, F, Fut>(
        &self,
        key: &str,
        ttl: Duration,
        fetch: F,
    ) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T>>,
        T: Serialize + DeserializeOwned,
    {
        // 1. 캐시 확인
        if let Some(cached) = self.get::<T>(key).await? {
            return Ok(cached);
        }

        // 2. 없으면 가져오기
        let data = fetch().await?;

        // 3. 캐시 저장
        self.set(key, &data, ttl).await?;

        Ok(data)
    }
}
```

**예상 효과**:
- 전략 목록 조회: ~50ms → ~2ms
- 심볼 정보 조회: ~20ms → ~1ms

**예상 시간**: 8시간

---

### 🟢 Medium (낮은 효과, 여유 있을 때)

#### 8. OpenAPI/Swagger 문서화 ✅ 완료 (v0.4.4)

> **구현 완료** (2026-01-31):
> - `crates/trader-api/src/openapi.rs` 추가
> - Swagger UI: `/swagger-ui`
> - OpenAPI JSON: `/api-docs/openapi.json`
> - 14개 태그, 자동 스키마 생성

~~**현재**: `docs/api.md` 수동 관리~~

~~**제안**: utoipa + Swagger UI 통합~~

```rust
// 실제 구현 (openapi.rs)
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    info(title = "Trader API", version = "0.1.0"),
    tags(
        (name = "health"), (name = "strategies"), (name = "backtest"),
        // ... 14개 태그
    ),
    components(schemas(HealthResponse, StrategiesListResponse, ApiError)),
    paths(
        crate::routes::health::health_check,
        crate::routes::strategies::list_strategies,
    )
)]
pub struct ApiDoc;

pub fn swagger_ui_router<S>() -> Router<S> {
    SwaggerUi::new("/swagger-ui")
        .url("/api-docs/openapi.json", ApiDoc::openapi())
        .into()
}
```

**소요 시간**: ~4시간
**효과**: 자동 API 문서 생성, 인터랙티브 테스트

---

#### 9. 입력 검증 강화

**현재 문제**:
```rust
pub struct BacktestRunRequest {
    pub start_date: String,  // 임의의 문자열 허용
    pub initial_capital: f64, // 음수 가능
}
```

**제안**:
```rust
use validator::Validate;

#[derive(Deserialize, Validate)]
pub struct BacktestRunRequest {
    #[validate(custom(function = "validate_date"))]
    pub start_date: String,

    #[validate(range(min = 100, max = 1_000_000_000))]
    pub initial_capital: f64,
}
```

**예상 시간**: 4시간

---

#### 10. 타입 안전성 강화 ✅ 완료 (v0.4.4)

> **구현 완료** (2026-01-31):
> - `crates/trader-api/src/types/strategy_type.rs` 추가
> - 26개 StrategyType enum 정의
> - `FromStr`, `Display`, Serde 지원
> - 헬퍼: `is_single_asset()`, `is_asset_allocation()`, `display_name()`

~~**현재**:~~
~~```rust
fn run_backtest(strategy_id: &str) -> Result<...>  // 임의의 문자열
```~~

**실제 구현**:
```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategyType {
    Rsi, Grid, Bollinger, VolatilityBreakout, MagicSplit,
    Sma, CandlePattern, InfinityBot, MarketInterestDay,
    StockGugan, SectorVb, SimplePower, Haa, Xaa, Baa,
    AllWeather, Snow, StockRotation, MarketCapTop,
    Us3xLeverage, SectorMomentum, DualMomentum,
    SmallCapQuant, PensionBot, KospiBothside, KosdaqFireRain,
}

impl FromStr for StrategyType { /* 구현됨 */ }
```

**소요 시간**: ~6시간

---

#### 11. 병렬 백테스트

**현재**: 순차 실행
```rust
for strategy_id in strategy_ids {
    let result = run_backtest(strategy_id).await?;
}
```

**제안**: 병렬 실행
```rust
use futures::stream::{self, StreamExt};

let results: Vec<_> = stream::iter(strategy_ids)
    .map(|id| async move { run_backtest(id).await })
    .buffer_unordered(num_cpus::get())
    .collect()
    .await;
```

**예상 효과**: 10개 전략 기준 1,000초 → 125초 (8코어)

**예상 시간**: 4시간

---

#### 12. 민감 정보 로깅 방지 (보안)

**현재 위험**:
```rust
// 로그에 API 키 노출 가능성
tracing::debug!("Config: {:?}", config);  // config에 credentials 포함될 수 있음
```

**제안**:
```rust
use secrecy::{Secret, ExposeSecret};

#[derive(Debug)]
pub struct Credentials {
    pub api_key: Secret<String>,
    pub api_secret: Secret<String>,
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("api_key", &"***REDACTED***")
            .field("api_secret", &"***REDACTED***")
            .finish()
    }
}
```

**예상 시간**: 2시간
**효과**: 보안 로그 노출 방지

---

#### 13. Feature Flag 도입

**제안**:
```toml
# crates/trader-api/Cargo.toml
[dependencies]
trader-core = { path = "../trader-core" }
trader-strategy = { path = "../trader-strategy", optional = true }
trader-analytics = { path = "../trader-analytics", optional = true }

[features]
default = ["strategies", "analytics"]
strategies = ["trader-strategy"]
analytics = ["trader-analytics"]
ml = ["trader-analytics/ml", "ort"]
full = ["strategies", "analytics", "ml", "notifications"]
```

**효과**:
- 필요한 기능만 선택적 컴파일
- 빌드 시간 단축 (예상: 20-30%)
- 바이너리 크기 감소

**예상 시간**: 4시간

---

### 🔵 프론트엔드 개선사항 (SolidJS Best Practices 기반)

> **참고**: Context7에서 SolidJS 최신 문서 조회 (2026-01-31)

#### 14. createStore로 상태 통합

**현재 문제** (`Strategies.tsx` 라인 64-100):
```typescript
// ❌ 20개+ createSignal 분산 - 상태 관리 복잡
const [filter, setFilter] = createSignal<'all' | 'running' | 'stopped'>('all')
const [showAddModal, setShowAddModal] = createSignal(false)
const [modalStep, setModalStep] = createSignal<'select' | 'configure'>('select')
const [selectedStrategy, setSelectedStrategy] = createSignal<BacktestStrategy | null>(null)
const [strategyParams, setStrategyParams] = createSignal<Record<string, unknown>>({})
const [showDeleteModal, setShowDeleteModal] = createSignal(false)
const [deletingStrategy, setDeletingStrategy] = createSignal<Strategy | null>(null)
const [showEditModal, setShowEditModal] = createSignal(false)
const [editingStrategyId, setEditingStrategyId] = createSignal<string | null>(null)
// ... 10개 더
```

**제안 - createStore 사용** (SolidJS 공식 권장):
```typescript
// stores/strategyPageStore.ts
import { createStore, produce } from 'solid-js/store';

interface StrategyPageState {
  filter: 'all' | 'running' | 'stopped';
  search: string;

  // 모달 상태 통합
  modals: {
    add: { open: boolean; step: 'select' | 'configure' };
    edit: { open: boolean; strategyId: string | null };
    delete: { open: boolean; strategy: Strategy | null };
    clone: { open: boolean; strategy: Strategy | null; name: string };
  };

  // 폼 상태 통합
  form: {
    params: Record<string, unknown>;
    errors: Record<string, string>;
    loading: boolean;
  };

  // 캐시
  symbolNameCache: Map<string, string>;
}

export function createStrategyPageStore() {
  const [state, setState] = createStore<StrategyPageState>({
    filter: 'all',
    search: '',
    modals: {
      add: { open: false, step: 'select' },
      edit: { open: false, strategyId: null },
      delete: { open: false, strategy: null },
      clone: { open: false, strategy: null, name: '' },
    },
    form: { params: {}, errors: {}, loading: false },
    symbolNameCache: new Map(),
  });

  const actions = {
    // 모달 액션
    openAddModal: () => setState('modals', 'add', 'open', true),
    closeAddModal: () => setState('modals', 'add', { open: false, step: 'select' }),
    setAddStep: (step: 'select' | 'configure') =>
      setState('modals', 'add', 'step', step),

    openEditModal: (id: string) =>
      setState('modals', 'edit', { open: true, strategyId: id }),
    closeEditModal: () =>
      setState('modals', 'edit', { open: false, strategyId: null }),

    openDeleteModal: (strategy: Strategy) =>
      setState('modals', 'delete', { open: true, strategy }),
    closeDeleteModal: () =>
      setState('modals', 'delete', { open: false, strategy: null }),

    // 폼 액션
    setFormParams: (params: Record<string, unknown>) =>
      setState('form', 'params', params),
    setFormError: (field: string, error: string) =>
      setState('form', 'errors', field, error),
    clearFormErrors: () => setState('form', 'errors', {}),
    setFormLoading: (loading: boolean) => setState('form', 'loading', loading),

    // produce를 사용한 불변 업데이트
    updateFormParam: (key: string, value: unknown) =>
      setState('form', 'params', produce(p => { p[key] = value })),
  };

  return { state, ...actions };
}

// 컴포넌트에서 사용
function Strategies() {
  const store = createStrategyPageStore();

  return (
    <Show when={store.state.modals.add.open}>
      <AddStrategyModal
        step={store.state.modals.add.step}
        onClose={store.closeAddModal}
      />
    </Show>
  );
}
```

**예상 시간**: 8시간
**효과**: 상태 관리 복잡도 70% 감소

---

#### 15. createMemo로 계산 최적화

**현재 문제**: 필터링/정렬 계산이 매번 실행됨

**제안**:
```typescript
import { createMemo } from 'solid-js';

function Strategies() {
  const [strategies] = createResource(getStrategies);
  const [filter, setFilter] = createSignal<'all' | 'running' | 'stopped'>('all');
  const [search, setSearch] = createSignal('');

  // ✅ createMemo - 의존성 변경 시에만 재계산
  const filteredStrategies = createMemo(() => {
    const list = strategies() ?? [];
    const f = filter();
    const q = search().toLowerCase();

    return list
      .filter(s => {
        if (f === 'running') return s.status === 'Running';
        if (f === 'stopped') return s.status === 'Stopped';
        return true;
      })
      .filter(s =>
        s.name.toLowerCase().includes(q) ||
        s.strategyType.toLowerCase().includes(q)
      );
  });

  // ✅ 통계도 메모이제이션
  const stats = createMemo(() => {
    const list = strategies() ?? [];
    return {
      total: list.length,
      running: list.filter(s => s.status === 'Running').length,
      stopped: list.filter(s => s.status === 'Stopped').length,
      totalPnl: list.reduce((sum, s) => sum + s.pnl, 0),
    };
  });

  return (
    <div>
      <p>총 {stats().total}개 전략, {stats().running}개 실행 중</p>
      <For each={filteredStrategies()}>
        {strategy => <StrategyCard strategy={strategy} />}
      </For>
    </div>
  );
}
```

**예상 시간**: 3시간

---

#### 16. createResource 에러 처리 강화

**현재 문제**: error/loading 상태 활용 부족

**제안**:
```typescript
function Strategies() {
  const [strategies, { refetch, mutate }] = createResource(getStrategies);

  return (
    <>
      {/* 로딩 상태 */}
      <Show when={strategies.loading}>
        <LoadingSpinner />
      </Show>

      {/* 에러 상태 */}
      <Show when={strategies.error}>
        <ErrorBanner
          message={strategies.error.message}
          onRetry={refetch}
        />
      </Show>

      {/* 데이터 */}
      <Show when={strategies()}>
        <For each={strategies()}>
          {strategy => <StrategyCard strategy={strategy} />}
        </For>
      </Show>
    </>
  );
}

// ErrorBoundary 추가
import { ErrorBoundary } from 'solid-js';

function App() {
  return (
    <ErrorBoundary fallback={(err, reset) => (
      <div>
        <h1>오류 발생</h1>
        <p>{err.message}</p>
        <button onClick={reset}>다시 시도</button>
      </div>
    )}>
      <Strategies />
    </ErrorBoundary>
  );
}
```

**예상 시간**: 2시간

---

#### 17. Discriminated Union 타입 적용

**현재 문제** (`types/index.ts`):
```typescript
export interface Strategy {
  strategyType: string;  // ❌ 문자열 - 타입 안전성 없음
  // config 타입 불명확
}
```

**제안 - Discriminated Union 패턴** (TypeScript Best Practice):
```typescript
// types/strategy.ts

// 전략 타입 리터럴
export type StrategyType =
  | 'rsi' | 'rsi_mean_reversion'
  | 'grid' | 'grid_trading'
  | 'bollinger' | 'bollinger_bands'
  | 'volatility_breakout'
  | 'sma_crossover'
  | 'all_weather' | 'all_weather_kr' | 'all_weather_us'
  | 'haa' | 'xaa' | 'simple_power';

// 전략별 Config - Discriminated Union
export type StrategyConfig =
  | RsiConfig
  | GridConfig
  | BollingerConfig
  | VolatilityConfig
  | AllWeatherConfig;

export interface RsiConfig {
  type: 'rsi';
  period: number;
  oversold_threshold: number;
  overbought_threshold: number;
  amount: string;
}

export interface GridConfig {
  type: 'grid';
  grid_levels: number;
  lower_price: number;
  upper_price: number;
  amount: string;
}

export interface BollingerConfig {
  type: 'bollinger';
  period: number;
  std_dev: number;
  amount: string;
}

export interface VolatilityConfig {
  type: 'volatility_breakout';
  k_value: number;
  stop_loss_pct: number;
}

export interface AllWeatherConfig {
  type: 'all_weather';
  rebalance_threshold: number;
  assets: Record<string, number>;  // 자산별 비중
}

// 타입 가드 함수들
export function isRsiConfig(config: StrategyConfig): config is RsiConfig {
  return config.type === 'rsi';
}

export function isGridConfig(config: StrategyConfig): config is GridConfig {
  return config.type === 'grid';
}

export function isAllWeatherConfig(config: StrategyConfig): config is AllWeatherConfig {
  return config.type === 'all_weather';
}

// Result 타입 (에러 처리용)
export type Result<T, E = Error> =
  | { success: true; data: T }
  | { success: false; error: E };

// 사용 예시
function processConfig(config: StrategyConfig) {
  switch (config.type) {
    case 'rsi':
      // config은 자동으로 RsiConfig 타입으로 좁혀짐
      return `RSI Period: ${config.period}`;
    case 'grid':
      return `Grid Levels: ${config.grid_levels}`;
    case 'all_weather':
      return `Assets: ${Object.keys(config.assets).join(', ')}`;
    default:
      // exhaustive check
      const _exhaustive: never = config;
      return _exhaustive;
  }
}
```

**예상 시간**: 6시간
**효과**: 컴파일 타임 타입 검증, IDE 자동완성 향상

---

#### 18. 커스텀 훅 추출

**제안 - 재사용 가능한 훅들**:
```typescript
// hooks/useStrategies.ts
export function useStrategies() {
  const [strategies, { refetch, mutate }] = createResource(getStrategies);

  const findById = (id: string) =>
    strategies()?.find(s => s.id === id);

  const filterByStatus = (status: Strategy['status']) =>
    strategies()?.filter(s => s.status === status) ?? [];

  const start = async (id: string) => {
    await startStrategy(id);
    refetch();
  };

  const stop = async (id: string) => {
    await stopStrategy(id);
    refetch();
  };

  return {
    strategies,
    loading: () => strategies.loading,
    error: () => strategies.error,
    refetch,
    mutate,
    findById,
    filterByStatus,
    start,
    stop,
  };
}

// hooks/useBacktest.ts
export function useBacktest() {
  const [result, setResult] = createSignal<BacktestResult | null>(null);
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [progress, setProgress] = createSignal(0);

  const run = async (request: BacktestRequest) => {
    setLoading(true);
    setError(null);
    setProgress(0);

    try {
      const data = await runBacktest(request, (p) => setProgress(p));
      setResult(data);
      return { success: true, data } as const;
    } catch (e) {
      const message = e instanceof Error ? e.message : 'Unknown error';
      setError(message);
      return { success: false, error: message } as const;
    } finally {
      setLoading(false);
    }
  };

  const reset = () => {
    setResult(null);
    setError(null);
    setProgress(0);
  };

  return { result, loading, error, progress, run, reset };
}

// hooks/useModal.ts
export function useModal<T = void>() {
  const [isOpen, setIsOpen] = createSignal(false);
  const [data, setData] = createSignal<T | null>(null);

  const open = (initialData?: T) => {
    if (initialData) setData(() => initialData);
    setIsOpen(true);
  };

  const close = () => {
    setIsOpen(false);
    setData(null);
  };

  return { isOpen, data, open, close };
}

// hooks/useSymbolSearch.ts
export function useSymbolSearch() {
  const [query, setQuery] = createSignal('');
  const [results, setResults] = createSignal<SymbolInfo[]>([]);
  const [loading, setLoading] = createSignal(false);

  // 디바운스된 검색
  const debouncedQuery = createMemo(() => {
    const q = query();
    return q.length >= 2 ? q : '';
  });

  createEffect(async () => {
    const q = debouncedQuery();
    if (!q) {
      setResults([]);
      return;
    }

    setLoading(true);
    try {
      const data = await searchSymbols(q, 10);
      setResults(data);
    } finally {
      setLoading(false);
    }
  });

  return { query, setQuery, results, loading };
}
```

**예상 시간**: 8시간

---

#### 19. 컴포넌트 분리 구조

**제안 구조**:
```
frontend/src/
├── components/
│   ├── strategy/
│   │   ├── StrategyCard.tsx       # 전략 카드
│   │   ├── StrategyList.tsx       # 전략 목록
│   │   ├── StrategyStats.tsx      # 통계 요약
│   │   └── StrategyFilters.tsx    # 필터 UI
│   ├── modals/
│   │   ├── AddStrategyModal.tsx   # 전략 추가
│   │   ├── EditStrategyModal.tsx  # 전략 편집
│   │   ├── DeleteConfirmModal.tsx # 삭제 확인
│   │   └── CloneStrategyModal.tsx # 전략 복제
│   └── common/
│       ├── LoadingSpinner.tsx
│       ├── ErrorBanner.tsx
│       └── EmptyState.tsx
├── hooks/
│   ├── useStrategies.ts
│   ├── useBacktest.ts
│   ├── useModal.ts
│   └── useSymbolSearch.ts
├── stores/
│   ├── strategyStore.ts
│   └── uiStore.ts
├── types/
│   ├── strategy.ts            # 전략 관련 타입
│   ├── backtest.ts            # 백테스트 타입
│   └── index.ts               # re-export
└── pages/
    ├── Strategies.tsx         # 300줄 이하로 축소
    ├── Dashboard.tsx
    └── Backtest.tsx
```

**예상 시간**: 12시간

---

#### 20. Lazy Loading 적용

**제안**:
```typescript
import { lazy, Suspense } from 'solid-js';

// 페이지 레벨 lazy loading
const Strategies = lazy(() => import('./pages/Strategies'));
const Backtest = lazy(() => import('./pages/Backtest'));
const Dataset = lazy(() => import('./pages/Dataset'));
const Simulation = lazy(() => import('./pages/Simulation'));

// 라우터에서 사용
function App() {
  return (
    <Router>
      <Suspense fallback={<PageLoader />}>
        <Routes>
          <Route path="/strategies" component={Strategies} />
          <Route path="/backtest" component={Backtest} />
          <Route path="/dataset" component={Dataset} />
          <Route path="/simulation" component={Simulation} />
        </Routes>
      </Suspense>
    </Router>
  );
}

// 무거운 컴포넌트도 lazy loading
const HeavyChart = lazy(() => import('./components/charts/HeavyChart'));

function ChartPanel() {
  return (
    <Suspense fallback={<ChartSkeleton />}>
      <HeavyChart />
    </Suspense>
  );
}
```

**예상 시간**: 3시간
**효과**: 초기 번들 크기 30-40% 감소

---

### 🟣 운영 안정성 (신규)

#### 16. 의존성 버전 정책 수립

**현재 문제**:
```toml
# Cargo.toml - 너무 느슨한 버전 지정
tokio = { version = "1", features = ["full"] }  # 1.0 ~ 1.99 허용
axum = { version = "0.7", ... }  # breaking change 위험
```

**발견된 이슈**:
- `ahash` v0.7.8 + v0.8.12 중복 (빌드 시간 증가)
- `getrandom` v0.2.17 + v0.3.4 중복
- `cargo audit` 미사용 (보안 취약점 미점검)

**제안**:
```toml
# 틸다(~) 사용으로 패치 버전만 허용
tokio = { version = "~1.35", features = ["full"] }
axum = { version = "~0.7.4", features = ["ws", "macros"] }
```

```bash
# CI 또는 pre-commit에 추가
cargo audit --deny warnings
```

**예상 시간**: 2시간

---

#### 17. 설정 검증 추가

**현재 문제**: `AppConfig::load()`가 단순 역직렬화만 수행

**제안**:
```rust
use validator::Validate;

#[derive(Deserialize, Validate)]
pub struct RiskConfig {
    #[validate(range(min = 0.0, max = 100.0))]
    pub max_position_pct: Decimal,

    #[validate(range(min = 0.0))]
    pub max_daily_loss: Decimal,
}

impl AppConfig {
    pub fn load_and_validate<P: AsRef<Path>>(path: P) -> Result<Self> {
        let config = Self::load(path)?;
        config.validate()?;
        Ok(config)
    }
}
```

**예상 시간**: 3시간

---

#### 18. 재시도 로직 (Retry + Backoff)

**현재**: Circuit Breaker는 있지만 Retry 로직이 분리되지 않음

**제안**:
```rust
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub multiplier: f64,  // 지수 백오프
}

pub async fn with_retry<F, T, E>(
    config: &RetryConfig,
    operation: F,
) -> Result<T, E>
where
    F: Fn() -> Pin<Box<dyn Future<Output = Result<T, E>>>>,
{
    let mut backoff = config.initial_backoff_ms;

    for attempt in 0..config.max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(_) if attempt < config.max_retries - 1 => {
                tokio::time::sleep(Duration::from_millis(backoff)).await;
                backoff = (backoff as f64 * config.multiplier)
                    .min(config.max_backoff_ms as f64) as u64;
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}
```

**예상 시간**: 6시간

---

#### 19. rustfmt/clippy 설정 추가

**현재**: 프로젝트 루트에 설정 파일 없음

**생성 필요 - `.rustfmt.toml`**:
```toml
edition = "2021"
max_width = 100
hard_tabs = false
tab_spaces = 4
imports_granularity = "Crate"
reorder_imports = true
group_imports = "StdExternalCrate"
```

**생성 필요 - `clippy.toml`**:
```toml
cognitive-complexity-threshold = 25
too-many-arguments-threshold = 8
```

**`.cargo/config.toml` 추가**:
```toml
[alias]
lint = "clippy --all --all-features -- -D warnings -D clippy::unwrap_used"
```

**예상 시간**: 1시간

---

#### 20. 외부 호출 타임아웃 설정

**현재 문제**: 대부분의 외부 API 호출에 타임아웃 없음

**🎯 구체적인 수정 위치** (2026-01-31 코드 분석):

> **참고**: 클라이언트 레벨 타임아웃은 설정되어 있으나, **개별 API 호출 타임아웃이 누락**됨

| 파일 | 라인 범위 | API 함수 | 현재 상태 |
|------|----------|----------|----------|
| `trader-exchange/src/connector/kis/client_kr.rs` | **85-130** | `get_price()` | ❌ 타임아웃 없음 |
| `trader-exchange/src/connector/kis/client_kr.rs` | **136-179** | `get_orderbook()` | ❌ 타임아웃 없음 |
| `trader-exchange/src/connector/kis/client_kr.rs` | **192-250** | `get_balance()` | ❌ 타임아웃 없음 |
| `trader-exchange/src/connector/kis/client_kr.rs` | **255-303** | `place_order()` | ❌ 타임아웃 없음 |
| `trader-exchange/src/connector/kis/client_kr.rs` | **305-350** | `cancel_order()` | ❌ 타임아웃 없음 |

**제안 - 개별 호출 타임아웃**:
```rust
// 현재 (타임아웃 없음)
let response = self.client.get(&url).headers(headers).send().await?;

// 개선안 (5초 타임아웃)
let response = tokio::time::timeout(
    Duration::from_secs(5),
    self.client.get(&url).headers(headers).send()
)
.await
.map_err(|_| KisError::Timeout("get_price timed out after 5s".into()))?
.map_err(|e| KisError::Network(e.to_string()))?;
```

**제안 - 공통 헬퍼 함수**:
```rust
pub async fn fetch_with_timeout<F, T>(
    timeout_secs: u64,
    future: F,
) -> Result<T, AppError>
where
    F: Future<Output = Result<T, AppError>>,
{
    tokio::time::timeout(Duration::from_secs(timeout_secs), future)
        .await
        .map_err(|_| AppError::Timeout)?
}
```

**예상 시간**: 4시간 (10+ 곳 수정)

---

#### 21. WebSocket 세션 관리 강화

**현재 문제**:
- 클라이언트 단절 시 세션 상태 추적 없음
- 메시지 큐 없이 동기 전송 (느린 클라이언트 → 서버 블로킹)

**제안**:
```rust
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, SessionState>>>,
}

pub struct SessionState {
    id: String,
    last_heartbeat: Instant,
    subscriptions: Vec<String>,
    message_queue: mpsc::Sender<WsMessage>,  // 버퍼링
}

// Heartbeat 체크 태스크
tokio::spawn(async move {
    loop {
        tokio::time::sleep(Duration::from_secs(30)).await;
        cleanup_stale_sessions(&sessions).await;
    }
});
```

**예상 시간**: 4시간

---

#### 22. 마이그레이션 테스트 추가

**현재 문제**: DOWN 마이그레이션 없음, 테스트 없음

**제안**:
```rust
#[sqlx::test(migrations = "migrations")]
async fn test_all_migrations_apply(pool: PgPool) {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT table_name FROM information_schema.tables
         WHERE table_schema = 'public'"
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert!(tables.contains(&"strategies".to_string()));
    assert!(tables.contains(&"positions".to_string()));
    assert!(tables.contains(&"klines".to_string()));
}
```

**예상 시간**: 3시간

---

### 🟤 Rust API 최신 패턴 (Context7 검증)

> **참고**: Context7에서 Tokio, Axum, SQLx 최신 문서 조회 (2026-01-31)

#### 23. Tokio select! 활용한 Graceful Shutdown

**현재 문제**: 서버 종료 시 WebSocket 연결, 진행 중인 작업이 즉시 중단됨

**제안 - Tokio select! 패턴** (Tokio 공식 권장):
```rust
use tokio::signal;
use tokio::sync::broadcast;

/// Graceful shutdown 구현
pub async fn run_server_with_graceful_shutdown(
    app: Router,
    listener: TcpListener,
) {
    // 종료 신호 브로드캐스트 채널
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    // Ctrl+C 핸들러
    let shutdown_signal = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
        tracing::info!("Shutdown signal received, starting graceful shutdown...");
    };

    // 서버 실행 + 종료 신호 대기
    tokio::select! {
        result = axum::serve(listener, app) => {
            if let Err(e) = result {
                tracing::error!("Server error: {}", e);
            }
        }
        _ = shutdown_signal => {
            tracing::info!("Initiating graceful shutdown...");
            // 진행 중인 요청 완료 대기 (최대 30초)
            let _ = shutdown_tx.send(());
        }
    }

    // 정리 작업
    tracing::info!("Server shutdown complete");
}

/// WebSocket에서 shutdown 수신
async fn handle_websocket(
    ws: WebSocket,
    mut shutdown_rx: broadcast::Receiver<()>,
) {
    loop {
        tokio::select! {
            // 클라이언트 메시지 처리
            msg = ws.recv() => {
                match msg {
                    Some(Ok(msg)) => process_message(msg).await,
                    _ => break,
                }
            }
            // 종료 신호 수신
            _ = shutdown_rx.recv() => {
                tracing::info!("WebSocket closing due to shutdown");
                break;
            }
        }
    }
}
```

**적용 대상**:
- `trader-api/src/main.rs`: 서버 시작점
- `routes/ws.rs`: WebSocket 핸들러

**예상 시간**: 4시간
**효과**: 안전한 서버 종료, 데이터 손실 방지

---

#### 24. Axum HandleErrorLayer로 타임아웃 미들웨어

**현재 문제**: 외부 API 호출 타임아웃 시 에러 처리 불명확

**제안 - Axum Tower 미들웨어** (Axum 공식 권장):
```rust
use axum::{
    error_handling::HandleErrorLayer,
    http::StatusCode,
    BoxError, Router,
};
use tower::ServiceBuilder;
use tower_http::timeout::TimeoutLayer;
use std::time::Duration;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/backtest", post(run_backtest))
        .route("/api/v1/strategies", get(list_strategies))
        .layer(
            ServiceBuilder::new()
                // 에러 핸들러 (가장 바깥쪽)
                .layer(HandleErrorLayer::new(|err: BoxError| async move {
                    if err.is::<tower::timeout::error::Elapsed>() {
                        (
                            StatusCode::REQUEST_TIMEOUT,
                            "Request timed out".to_string(),
                        )
                    } else {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("Internal error: {}", err),
                        )
                    }
                }))
                // 전역 타임아웃 (60초)
                .layer(TimeoutLayer::new(Duration::from_secs(60)))
        )
        .with_state(state)
}

/// 엔드포인트별 커스텀 타임아웃
pub fn create_backtest_router(state: AppState) -> Router {
    Router::new()
        .route("/run", post(run_backtest))
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_backtest_error))
                // 백테스트는 긴 타임아웃 (10분)
                .layer(TimeoutLayer::new(Duration::from_secs(600)))
        )
        .with_state(state)
}

async fn handle_backtest_error(err: BoxError) -> (StatusCode, String) {
    if err.is::<tower::timeout::error::Elapsed>() {
        (
            StatusCode::REQUEST_TIMEOUT,
            "Backtest timeout - try with smaller date range".to_string(),
        )
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Backtest error: {}", err),
        )
    }
}
```

**적용 대상**:
- `trader-api/src/routes/mod.rs`: 라우터 설정
- `routes/backtest.rs`: 백테스트 라우터

**예상 시간**: 3시간
**효과**: 명확한 타임아웃 에러 응답

---

#### 25. SQLx 트랜잭션 패턴 개선

**현재 문제**: 트랜잭션 사용이 일관적이지 않음 (현재 **0건** 사용)

**🎯 트랜잭션 적용 필요 위치** (2026-01-31 코드 분석):

| 파일 | 함수 | 다중 쿼리 작업 | 트랜잭션 필요성 |
|------|------|--------------|----------------|
| `trader-api/src/repository/strategies.rs` | `create()` | INSERT 후 관련 테이블 업데이트 | ⚠️ 원자성 필요 |
| `trader-api/src/repository/strategies.rs` | `update()` | 전략 + 설정 동시 업데이트 | ⚠️ 원자성 필요 |
| `trader-api/src/repository/strategies.rs` | `delete()` | 전략 + 관련 데이터 삭제 | ⚠️ 원자성 필요 |
| `trader-api/src/routes/orders.rs` | 주문 처리 | 주문 생성 + 포지션 업데이트 | 🔴 필수 |
| `trader-api/src/routes/positions.rs` | 포지션 청산 | 포지션 + 주문 + 히스토리 | 🔴 필수 |

**제안 - SQLx 트랜잭션 Best Practice**:
```rust
use sqlx::{PgPool, Postgres, Transaction};

/// 트랜잭션 헬퍼 함수
pub async fn with_transaction<F, T, E>(
    pool: &PgPool,
    f: F,
) -> Result<T, E>
where
    F: for<'c> FnOnce(&'c mut Transaction<'_, Postgres>) -> BoxFuture<'c, Result<T, E>>,
    E: From<sqlx::Error>,
{
    let mut tx = pool.begin().await?;

    match f(&mut tx).await {
        Ok(result) => {
            tx.commit().await?;
            Ok(result)
        }
        Err(e) => {
            // 롤백은 Drop에서 자동 수행되지만 명시적으로 호출
            let _ = tx.rollback().await;
            Err(e)
        }
    }
}

/// 포지션 청산 예시 (여러 테이블 업데이트)
pub async fn close_position_with_order(
    pool: &PgPool,
    position_id: &str,
    close_price: Decimal,
    order: &CreateOrderInput,
) -> Result<(Position, Order), ApiError> {
    with_transaction(pool, |tx| {
        Box::pin(async move {
            // 1. 포지션 업데이트
            let position = sqlx::query_as!(
                Position,
                r#"
                UPDATE positions
                SET is_closed = true, close_price = $2, closed_at = NOW()
                WHERE id = $1
                RETURNING *
                "#,
                position_id,
                close_price
            )
            .fetch_one(&mut **tx)
            .await?;

            // 2. 주문 생성
            let order = sqlx::query_as!(
                Order,
                r#"
                INSERT INTO orders (strategy_id, symbol, side, quantity, price, status)
                VALUES ($1, $2, $3, $4, $5, 'filled')
                RETURNING *
                "#,
                order.strategy_id,
                order.symbol,
                order.side as _,
                order.quantity,
                close_price
            )
            .fetch_one(&mut **tx)
            .await?;

            // 3. 자산 히스토리 기록
            sqlx::query!(
                "INSERT INTO equity_history (strategy_id, equity, timestamp)
                 VALUES ($1, $2, NOW())",
                order.strategy_id,
                position.realized_pnl
            )
            .execute(&mut **tx)
            .await?;

            Ok((position, order))
        })
    })
    .await
}

/// close_event 핸들링 (연결 종료 감지)
pub async fn setup_db_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(3))
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                // 연결 후 설정 (예: search_path)
                sqlx::query("SET search_path TO public")
                    .execute(conn)
                    .await?;
                Ok(())
            })
        })
        .connect(database_url)
        .await?;

    // 연결 상태 모니터링
    tokio::spawn({
        let pool = pool.clone();
        async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                let size = pool.size();
                let idle = pool.num_idle();
                tracing::debug!("DB Pool: {} total, {} idle", size, idle);
            }
        }
    });

    Ok(pool)
}
```

**적용 대상**:
- `repository/*.rs`: 모든 Repository
- `routes/orders.rs`: 주문 처리
- `routes/positions.rs`: 포지션 관리

**예상 시간**: 6시간
**효과**: 데이터 일관성 보장, 연결 풀 모니터링

---

#### 26. Tokio spawn_blocking + mpsc 채널

**현재 문제**: CPU 집약적 작업이 async 런타임 블로킹

**제안 - spawn_blocking 패턴** (Tokio 공식 권장):
```rust
use tokio::sync::mpsc;
use tokio::task::spawn_blocking;

/// CPU 집약적 백테스트를 별도 스레드에서 실행
pub async fn run_cpu_intensive_backtest(
    request: BacktestRequest,
) -> Result<BacktestResult, ApiError> {
    // 결과 전달용 채널
    let (tx, mut rx) = mpsc::channel::<BacktestProgress>(100);

    // CPU 집약적 작업은 blocking 스레드에서 실행
    let handle = spawn_blocking(move || {
        // 동기 코드에서 진행 상황 전송
        let rt = tokio::runtime::Handle::current();

        let mut engine = BacktestEngine::new(request);

        for (i, candle) in engine.candles.iter().enumerate() {
            // 진행 상황 전송 (blocking_send 또는 try_send)
            let progress = BacktestProgress {
                current: i,
                total: engine.candles.len(),
                pnl: engine.current_pnl(),
            };

            // try_send로 논블로킹 전송 (버퍼 초과시 무시)
            let _ = tx.try_send(progress);

            // 백테스트 로직 실행
            engine.process_candle(candle);
        }

        engine.finalize()
    });

    // 진행 상황 수신 (선택적)
    tokio::spawn(async move {
        while let Some(progress) = rx.recv().await {
            tracing::debug!(
                "Backtest progress: {}/{} ({:.2}%)",
                progress.current,
                progress.total,
                (progress.current as f64 / progress.total as f64) * 100.0
            );
        }
    });

    // 결과 대기
    handle.await.map_err(|e| ApiError::Internal(e.to_string()))?
}

/// 기술적 지표 계산 (CPU 집약적)
pub async fn calculate_indicators_async(
    candles: Vec<Candle>,
) -> Result<IndicatorSet, ApiError> {
    spawn_blocking(move || {
        // 동기 코드로 지표 계산
        let rsi = calculate_rsi(&candles, 14);
        let macd = calculate_macd(&candles, 12, 26, 9);
        let bollinger = calculate_bollinger(&candles, 20, 2.0);

        IndicatorSet { rsi, macd, bollinger }
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}
```

**🎯 구체적인 수정 위치** (2026-01-31 코드 분석):

| 파일 | 라인 범위 | 함수 | CPU 작업 내용 |
|------|----------|------|--------------|
| `trader-api/src/routes/backtest/engine.rs` | **49-107** | `run_strategy_backtest()` | 전략 초기화 + 엔진 실행 |
| `trader-api/src/routes/backtest/engine.rs` | **102-108** | `engine.run()` | 캔들 순회 + 신호 생성 |

```rust
// 현재 (문제) - backtest/engine.rs:102-108
pub async fn run_strategy_backtest(...) -> Result<BacktestReport, String> {
    let mut engine = BacktestEngine::new(config);
    strategy.initialize(strategy_config).await?;  // async 컨텍스트에서 CPU 작업
    engine.run(&mut strategy, klines).await?      // 블로킹 위험
}

// 개선안 - spawn_blocking으로 분리
pub async fn run_strategy_backtest(...) -> Result<BacktestReport, String> {
    let config = config.clone();
    let klines = klines.to_vec();

    tokio::task::spawn_blocking(move || {
        let mut engine = BacktestEngine::new(config);
        engine.run_sync(&mut strategy, &klines)  // 동기 버전 호출
    })
    .await
    .map_err(|e| e.to_string())?
}
```

**적용 대상**:
- `routes/backtest/engine.rs`: 백테스트 실행
- `trader-analytics`: 지표 계산
- `trader-strategy`: 전략 신호 생성

**예상 시간**: 4시간
**효과**: async 런타임 블로킹 방지, 응답성 향상

---

#### 27. Tokio blocking_lock (sync Mutex)

**현재 문제**: 일부 코드에서 std::sync::Mutex와 async 혼용

**제안 - tokio::sync::Mutex::blocking_lock** (Tokio 권장):
```rust
use tokio::sync::Mutex;
use std::sync::Arc;

pub struct StrategyEngine {
    strategies: Arc<Mutex<HashMap<String, Box<dyn Strategy>>>>,
}

impl StrategyEngine {
    /// async 컨텍스트에서 사용
    pub async fn get_strategy(&self, id: &str) -> Option<StrategyStatus> {
        let strategies = self.strategies.lock().await;
        strategies.get(id).map(|s| s.get_status())
    }

    /// sync 컨텍스트에서 사용 (spawn_blocking 내부 등)
    pub fn get_strategy_sync(&self, id: &str) -> Option<StrategyStatus> {
        // blocking_lock()은 현재 스레드를 블로킹하지만
        // async 런타임을 블로킹하지 않음
        let strategies = self.strategies.blocking_lock();
        strategies.get(id).map(|s| s.get_status())
    }

    /// sync 컨텍스트에서 여러 전략 조회
    pub fn get_all_statuses_sync(&self) -> Vec<StrategyStatus> {
        let strategies = self.strategies.blocking_lock();
        strategies.values().map(|s| s.get_status()).collect()
    }
}

/// spawn_blocking에서 사용 예시
pub async fn heavy_computation_with_state(
    engine: Arc<StrategyEngine>,
) -> Result<ComputationResult, ApiError> {
    spawn_blocking(move || {
        // blocking_lock으로 동기적 접근
        let statuses = engine.get_all_statuses_sync();

        // CPU 집약적 계산
        compute_something_heavy(statuses)
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}
```

**주의사항**:
- `blocking_lock()`은 **async 컨텍스트가 아닌 곳**에서만 사용
- async 컨텍스트에서는 항상 `.lock().await` 사용
- `spawn_blocking` 내부에서는 `blocking_lock()` 사용 가능

**예상 시간**: 2시간

---

#### 28. Axum 에러 추출자 (Method, Uri)

**현재 문제**: 에러 응답에 요청 컨텍스트 정보 부족

**제안 - Axum Extractor 활용** (Axum 공식 패턴):
```rust
use axum::{
    extract::rejection::JsonRejection,
    http::{Method, Uri},
    response::{IntoResponse, Response},
    Json,
};

/// 커스텀 에러 타입 with 요청 컨텍스트
pub struct ApiError {
    pub kind: ApiErrorKind,
    pub method: Option<Method>,
    pub uri: Option<Uri>,
}

pub enum ApiErrorKind {
    NotFound(String),
    BadRequest(String),
    Internal(String),
    Timeout,
    JsonParse(JsonRejection),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self.kind {
            ApiErrorKind::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            ApiErrorKind::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiErrorKind::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ApiErrorKind::Timeout => (StatusCode::REQUEST_TIMEOUT, "Request timeout".into()),
            ApiErrorKind::JsonParse(e) => (StatusCode::BAD_REQUEST, e.to_string()),
        };

        let body = json!({
            "error": message,
            "method": self.method.map(|m| m.to_string()),
            "path": self.uri.map(|u| u.to_string()),
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        (status, Json(body)).into_response()
    }
}

/// 핸들러에서 요청 컨텍스트 캡처
pub async fn get_strategy(
    method: Method,
    uri: Uri,
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<Strategy>, ApiError> {
    state.strategy_repo
        .find_by_id(&id)
        .await
        .map_err(|e| ApiError {
            kind: ApiErrorKind::Internal(e.to_string()),
            method: Some(method.clone()),
            uri: Some(uri.clone()),
        })?
        .ok_or_else(|| ApiError {
            kind: ApiErrorKind::NotFound(format!("Strategy {} not found", id)),
            method: Some(method),
            uri: Some(uri),
        })
        .map(Json)
}
```

**예상 시간**: 3시간
**효과**: 디버깅 용이, 에러 추적 개선

---

## 전략 등록 자동화 (신규)

### 현재 문제점

새 전략을 추가할 때 **5곳 이상**을 수정해야 함:

| # | 파일 | 수정 내용 |
|---|------|----------|
| 1 | `strategies/mod.rs` | `pub mod`, `pub use` 추가 |
| 2 | `routes/strategies.rs` | 팩토리 함수 4개에 match arm 추가 |
| 3 | `routes/backtest/engine.rs` | import + match arm 추가 |
| 4 | `config/sdui/strategy_schemas.json` | UI 스키마 추가 (~50줄) |
| 5 | `frontend/src/pages/Strategies.tsx` | 타임프레임 매핑 추가 |

### 현재 수정 위치 상세

```
□ 1. crates/trader-strategy/src/strategies/mod.rs
  □ pub mod your_strategy;
  □ pub use your_strategy::*;

□ 2. crates/trader-api/src/routes/strategies.rs
  □ create_strategy_instance() - 전략 인스턴스 생성
  □ get_strategy_default_name() - 한글 이름
  □ get_strategy_default_timeframe() - 기본 타임프레임
  □ get_strategy_default_symbols() - 권장 심볼

□ 3. crates/trader-api/src/routes/backtest/engine.rs
  □ import 추가
  □ run_strategy_backtest() 또는 run_multi_strategy_backtest()

□ 4. config/sdui/strategy_schemas.json
  □ strategies 객체에 전략 스키마 추가

□ 5. frontend/src/pages/Strategies.tsx
  □ getDefaultTimeframe() switch 문에 case 추가
```

### 제안 1: 전략 레지스트리 패턴

**핵심 아이디어**: 전략 메타데이터를 한 곳에서 선언

```rust
// strategies/registry.rs (신규)
use inventory::collect;

/// 전략 메타데이터 (컴파일 타임 등록)
#[derive(Debug, Clone)]
pub struct StrategyMeta {
    pub id: &'static str,
    pub name: &'static str,           // 한글 이름
    pub description: &'static str,
    pub default_timeframe: &'static str,
    pub default_symbols: &'static [&'static str],
    pub category: StrategyCategory,
    pub factory: fn() -> Box<dyn Strategy>,
}

#[derive(Debug, Clone, Copy)]
pub enum StrategyCategory {
    Realtime,      // 1m - 그리드, 무한매수
    Intraday,      // 15m - RSI, 볼린저
    Daily,         // 1d - 변동성 돌파
    Monthly,       // 1M - 자산배분
}

// 매크로로 자동 등록
inventory::collect!(StrategyMeta);

/// 전략 정의 매크로
#[macro_export]
macro_rules! register_strategy {
    (
        id: $id:literal,
        name: $name:literal,
        description: $desc:literal,
        timeframe: $tf:literal,
        symbols: [$($sym:literal),*],
        category: $cat:ident,
        type: $type:ty
    ) => {
        inventory::submit! {
            StrategyMeta {
                id: $id,
                name: $name,
                description: $desc,
                default_timeframe: $tf,
                default_symbols: &[$($sym),*],
                category: StrategyCategory::$cat,
                factory: || Box::new(<$type>::new()),
            }
        }
    };
}
```

**전략 파일에서 사용**:
```rust
// strategies/rsi.rs
use crate::register_strategy;

register_strategy! {
    id: "rsi_mean_reversion",
    name: "RSI 평균회귀",
    description: "RSI 과매수/과매도 기반 평균회귀 전략",
    timeframe: "15m",
    symbols: [],
    category: Intraday,
    type: RsiStrategy
}

pub struct RsiStrategy { /* ... */ }
impl Strategy for RsiStrategy { /* ... */ }
```

**팩토리에서 자동 조회**:
```rust
// routes/strategies.rs
use trader_strategy::registry::{StrategyMeta, STRATEGIES};

fn create_strategy_instance(strategy_type: &str) -> Result<Box<dyn Strategy>, String> {
    for meta in inventory::iter::<StrategyMeta> {
        if meta.id == strategy_type {
            return Ok((meta.factory)());
        }
    }
    Err(format!("Unknown strategy: {}", strategy_type))
}

fn get_strategy_default_name(strategy_type: &str) -> &'static str {
    inventory::iter::<StrategyMeta>
        .find(|m| m.id == strategy_type)
        .map(|m| m.name)
        .unwrap_or("Unknown")
}
```

### 제안 2: SDUI 스키마 자동 생성

**전략 Config에서 스키마 파생**:
```rust
use schemars::JsonSchema;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[schemars(title = "RSI 평균회귀 설정")]
pub struct RsiConfig {
    /// RSI 계산 기간
    #[schemars(range(min = 2, max = 100))]
    pub period: usize,

    /// 과매도 임계값
    #[schemars(range(min = 0.0, max = 50.0))]
    pub oversold_threshold: f64,

    /// 과매수 임계값
    #[schemars(range(min = 50.0, max = 100.0))]
    pub overbought_threshold: f64,
}

// API 엔드포인트로 스키마 제공
async fn get_strategy_schema(
    Path(strategy_id): Path<String>,
) -> impl IntoResponse {
    match strategy_id.as_str() {
        "rsi" => Json(schemars::schema_for!(RsiConfig)),
        // ...
    }
}
```

### 제안 3: 프론트엔드 자동 동기화

**백엔드에서 메타데이터 제공**:
```rust
// GET /api/v1/strategies/meta
async fn get_all_strategy_meta() -> impl IntoResponse {
    let metas: Vec<_> = inventory::iter::<StrategyMeta>
        .map(|m| json!({
            "id": m.id,
            "name": m.name,
            "description": m.description,
            "defaultTimeframe": m.default_timeframe,
            "defaultSymbols": m.default_symbols,
            "category": format!("{:?}", m.category),
        }))
        .collect();

    Json(metas)
}
```

**프론트엔드에서 동적 사용**:
```typescript
// hooks/useStrategyMeta.ts
export function useStrategyMeta() {
  const [meta] = createResource(() => fetchStrategyMeta());

  const getDefaultTimeframe = (strategyId: string) => {
    return meta()?.find(m => m.id === strategyId)?.defaultTimeframe ?? '1d';
  };

  return { meta, getDefaultTimeframe };
}
```

### 자동화 후 전략 추가 체크리스트

```
□ 1. strategies/your_strategy.rs 생성
  □ register_strategy! 매크로 호출
  □ Strategy trait 구현
  □ Config 구조체 (JsonSchema derive)

✅ 완료! (나머지는 자동)
```

**예상 시간**: 16시간 (일회성 인프라 구축)
**효과**: 전략 추가 시간 2시간 → 30분

---

## Repository 상세 설계

### 현재 상태 (3개)

```
crates/trader-api/src/repository/
├── mod.rs
├── strategies.rs      ✅ 전략 CRUD
├── execution_cache.rs ✅ 실행 캐시
└── symbol_info.rs     ✅ 종목 정보
```

### 필요한 Repository (8개 추가)

#### 1. PortfolioRepository

```rust
// repository/portfolio.rs
pub struct PortfolioRepository;

impl PortfolioRepository {
    /// 현재 활성 포지션 조회
    pub async fn get_active_positions(
        pool: &PgPool,
        strategy_id: &str,
    ) -> Result<Vec<Position>, sqlx::Error>;

    /// 포트폴리오 요약 (총 자산, 수익률 등)
    pub async fn get_summary(
        pool: &PgPool,
        strategy_id: &str,
    ) -> Result<PortfolioSummary, sqlx::Error>;

    /// 포지션 비중 계산
    pub async fn get_weights(
        pool: &PgPool,
        strategy_id: &str,
    ) -> Result<HashMap<String, Decimal>, sqlx::Error>;
}
```

#### 2. OrdersRepository

```rust
// repository/orders.rs
pub struct OrdersRepository;

impl OrdersRepository {
    /// 주문 생성
    pub async fn create(
        pool: &PgPool,
        order: &CreateOrderInput,
    ) -> Result<Order, sqlx::Error>;

    /// 미체결 주문 조회
    pub async fn get_pending(
        pool: &PgPool,
        strategy_id: &str,
    ) -> Result<Vec<Order>, sqlx::Error>;

    /// 주문 상태 업데이트
    pub async fn update_status(
        pool: &PgPool,
        order_id: &str,
        status: OrderStatus,
        filled_qty: Option<Decimal>,
    ) -> Result<(), sqlx::Error>;

    /// 기간별 주문 이력
    pub async fn get_history(
        pool: &PgPool,
        strategy_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<Order>, sqlx::Error>;
}
```

#### 3. PositionsRepository

```rust
// repository/positions.rs
pub struct PositionsRepository;

impl PositionsRepository {
    /// 포지션 생성/업데이트 (UPSERT)
    pub async fn upsert(
        pool: &PgPool,
        position: &Position,
    ) -> Result<Position, sqlx::Error>;

    /// 포지션 청산
    pub async fn close(
        pool: &PgPool,
        position_id: &str,
        close_price: Decimal,
        close_reason: &str,
    ) -> Result<Position, sqlx::Error>;

    /// 심볼별 포지션 조회
    pub async fn get_by_symbol(
        pool: &PgPool,
        strategy_id: &str,
        symbol: &str,
    ) -> Result<Option<Position>, sqlx::Error>;
}
```

#### 4. EquityHistoryRepository

```rust
// repository/equity_history.rs
pub struct EquityHistoryRepository;

impl EquityHistoryRepository {
    /// 자산 스냅샷 저장 (배치)
    pub async fn save_snapshots(
        pool: &PgPool,
        snapshots: &[EquitySnapshot],
    ) -> Result<(), sqlx::Error>;

    /// 기간별 자산 곡선 조회
    pub async fn get_curve(
        pool: &PgPool,
        strategy_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        interval: &str,  // "1h", "1d", "1w"
    ) -> Result<Vec<EquityPoint>, sqlx::Error>;

    /// MDD 계산
    pub async fn calculate_mdd(
        pool: &PgPool,
        strategy_id: &str,
        period_days: i32,
    ) -> Result<Decimal, sqlx::Error>;
}
```

#### 5. BacktestResultsRepository

```rust
// repository/backtest_results.rs
pub struct BacktestResultsRepository;

impl BacktestResultsRepository {
    /// 백테스트 결과 저장
    pub async fn save(
        pool: &PgPool,
        result: &BacktestResult,
    ) -> Result<String, sqlx::Error>;  // 결과 ID 반환

    /// 결과 조회
    pub async fn get_by_id(
        pool: &PgPool,
        result_id: &str,
    ) -> Result<Option<BacktestResult>, sqlx::Error>;

    /// 전략별 결과 목록
    pub async fn list_by_strategy(
        pool: &PgPool,
        strategy_id: &str,
        limit: i32,
    ) -> Result<Vec<BacktestResultSummary>, sqlx::Error>;

    /// 결과 비교
    pub async fn compare(
        pool: &PgPool,
        result_ids: &[String],
    ) -> Result<Vec<BacktestComparison>, sqlx::Error>;
}
```

#### 6. KlinesRepository

```rust
// repository/klines.rs
pub struct KlinesRepository;

impl KlinesRepository {
    /// OHLCV 배치 저장 (UNNEST 최적화)
    pub async fn save_batch(
        pool: &PgPool,
        klines: &[Kline],
    ) -> Result<usize, sqlx::Error>;

    /// 기간별 조회 (타임프레임 지정)
    pub async fn get_range(
        pool: &PgPool,
        symbol: &str,
        timeframe: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<Kline>, sqlx::Error>;

    /// 최신 N개 조회
    pub async fn get_latest(
        pool: &PgPool,
        symbol: &str,
        timeframe: &str,
        count: i32,
    ) -> Result<Vec<Kline>, sqlx::Error>;

    /// 심볼 목록 조회
    pub async fn list_symbols(
        pool: &PgPool,
    ) -> Result<Vec<String>, sqlx::Error>;
}
```

#### 7. CredentialsRepository

```rust
// repository/credentials.rs
pub struct CredentialsRepository;

impl CredentialsRepository {
    /// 암호화된 자격증명 저장
    pub async fn save(
        pool: &PgPool,
        exchange: &str,
        credentials: &EncryptedCredentials,
    ) -> Result<(), sqlx::Error>;

    /// 자격증명 조회
    pub async fn get(
        pool: &PgPool,
        exchange: &str,
    ) -> Result<Option<EncryptedCredentials>, sqlx::Error>;

    /// 접근 로그 기록
    pub async fn log_access(
        pool: &PgPool,
        exchange: &str,
        action: &str,
    ) -> Result<(), sqlx::Error>;
}
```

#### 8. AlertsRepository

```rust
// repository/alerts.rs
pub struct AlertsRepository;

impl AlertsRepository {
    /// 알림 생성
    pub async fn create(
        pool: &PgPool,
        alert: &CreateAlertInput,
    ) -> Result<Alert, sqlx::Error>;

    /// 미확인 알림 조회
    pub async fn get_unread(
        pool: &PgPool,
        user_id: Option<&str>,
    ) -> Result<Vec<Alert>, sqlx::Error>;

    /// 알림 확인 처리
    pub async fn mark_read(
        pool: &PgPool,
        alert_ids: &[String],
    ) -> Result<(), sqlx::Error>;
}
```

### Repository 구조 요약

```
repository/
├── mod.rs                 # 모듈 export
├── strategies.rs          ✅ 기존
├── execution_cache.rs     ✅ 기존
├── symbol_info.rs         ✅ 기존
├── portfolio.rs           🆕 포트폴리오 요약
├── orders.rs              🆕 주문 CRUD
├── positions.rs           🆕 포지션 CRUD
├── equity_history.rs      🆕 자산 이력
├── backtest_results.rs    🆕 백테스트 결과
├── klines.rs              🆕 OHLCV 데이터
├── credentials.rs         🆕 자격증명
└── alerts.rs              🆕 알림
```

### Repository 공통 패턴

```rust
// repository/common.rs
use sqlx::PgPool;

/// 페이지네이션 옵션
#[derive(Debug, Default)]
pub struct Pagination {
    pub offset: i64,
    pub limit: i64,
}

/// 정렬 옵션
#[derive(Debug)]
pub struct Sort {
    pub field: String,
    pub direction: SortDirection,
}

/// Repository 기본 trait
#[async_trait]
pub trait Repository<T, Id> {
    async fn find_by_id(pool: &PgPool, id: Id) -> Result<Option<T>, sqlx::Error>;
    async fn find_all(pool: &PgPool, pagination: Pagination) -> Result<Vec<T>, sqlx::Error>;
    async fn delete(pool: &PgPool, id: Id) -> Result<bool, sqlx::Error>;
}
```

**총 예상 시간**: 16시간 (8개 Repository)
**효과**:
- 쿼리 로직 재사용
- 테스트 용이성 (Mock 가능)
- N+1 쿼리 방지
- 일관된 에러 처리

---

## 구현 로드맵

### Phase 1: Critical (1주)

| 일차 | 작업 | 예상 시간 |
|------|------|----------|
| Day 1-2 | unwrap() 159개 제거 (routes/*.rs) | 8시간 |
| Day 2 | 의존성 버전 정책 + cargo audit | 2시간 |
| Day 3-4 | analytics.rs 분리 (2,678줄 → 6파일) | 4시간 |
| Day 5 | Dataset.tsx 분리 (2,198줄 → 5컴포넌트) | 4시간 |
| Day 5 | rustfmt/clippy 설정 추가 | 1시간 |

**총 시간**: 19시간

### Phase 2: High (2주)

| 주차 | 작업 | 예상 시간 |
|------|------|----------|
| Week 1 | 비동기 락 홀드 최적화 | 4시간 |
| Week 1 | 전략 공통 로직 추출 | 12시간 |
| Week 1 | Repository 확장 (8개) ✅ | 16시간 |
| Week 2 | 핵심 테스트 추가 | 16시간 |
| Week 2 | Redis 캐싱 레이어 | 8시간 |
| Week 2 | 재시도 로직 (Retry + Backoff) | 6시간 |

**총 시간**: 62시간

### Phase 3: Medium (1개월)

| 항목 | 예상 시간 |
|------|----------|
| OpenAPI/Swagger 문서화 ✅ | ~~6시간~~ 4시간 |
| 입력 검증 강화 (validator) | 4시간 |
| StrategyType enum 타입 안전성 ✅ | ~~10시간~~ 6시간 |
| 병렬 백테스트 | 4시간 |
| 민감 정보 로깅 방지 | 2시간 |
| Feature Flag 도입 | 4시간 |
| 프론트엔드 훅 추출 | 6시간 |
| 프론트엔드 타입 강화 | 4시간 |

**총 시간**: 40시간

### Phase 4: 전략 자동화 인프라 (2주)

| 항목 | 예상 시간 |
|------|----------|
| 전략 레지스트리 패턴 구현 | 8시간 |
| register_strategy! 매크로 | 4시간 |
| SDUI 스키마 자동 생성 | 4시간 |
| 프론트엔드 메타 API 연동 | 4시간 |
| 기존 27개 전략 마이그레이션 | 8시간 |

**총 시간**: 28시간

### Phase 5: 운영 안정성 (여유 시)

| 항목 | 예상 시간 |
|------|----------|
| 설정 검증 추가 | 3시간 |
| 외부 호출 타임아웃 | 2시간 |
| WebSocket 세션 관리 | 4시간 |
| 마이그레이션 테스트 | 3시간 |

**총 시간**: 12시간

### Phase 6: Rust API 최신 패턴 (권장)

| 항목 | 예상 시간 |
|------|----------|
| Tokio select! Graceful Shutdown | 4시간 |
| Axum HandleErrorLayer 타임아웃 | 3시간 |
| SQLx 트랜잭션 패턴 개선 | 6시간 |
| spawn_blocking + mpsc 채널 | 4시간 |
| blocking_lock 적용 | 2시간 |
| Axum 에러 추출자 (Method, Uri) | 3시간 |

**총 시간**: 22시간

### Phase 7: 코드 리팩토링 (3-4주)

> **참고**: 2026-01-31 코드베이스 분석 결과

#### 7.1 코드 중복 제거 (DRY)

| 항목 | 파일 | 예상 시간 |
|------|------|----------|
| 에러 응답 타입 통합 | `BacktestApiError`, `SimulationApiError`, `ApiError`, `ErrorResponse` → 단일 `ApiErrorResponse` | 2시간 |
| 포매팅 함수 통합 | `Dashboard.tsx`, `Strategies.tsx`, `Simulation.tsx` → `utils/formatters.ts` | 1시간 |
| 기간 파싱 유틸리티 | `analytics.rs:2480` 등 → `utils/period.rs` | 1시간 |

**🎯 에러 응답 통합 상세**:

```rust
// 현재: 4개의 중복 타입
// routes/backtest/types.rs
pub struct BacktestApiError { pub code: String, pub message: String }
// routes/simulation.rs
pub struct SimulationApiError { pub code: String, pub message: String }
// routes/strategies.rs
pub struct ApiError { pub code: String, pub message: String }
// routes/ml.rs
pub struct ErrorResponse { pub error: String, pub message: String }

// 개선: 단일 통합 타입 (crates/trader-api/src/error.rs)
#[derive(Debug, Serialize)]
pub struct ApiErrorResponse {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
    pub timestamp: i64,
}

impl ApiErrorResponse {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}
```

**소계**: 4시간

---

#### 7.2 대형 파일 분리

| 파일 | 현재 | 분리 후 | 예상 시간 |
|------|------|---------|----------|
| `analytics.rs` | 2,678줄 | 6개 모듈 (각 ~450줄) | 8시간 |
| `Dataset.tsx` | 2,198줄 | 5개 컴포넌트 (각 ~440줄) | 6시간 |
| `credentials.rs` | 1,615줄 | 4개 모듈 (각 ~400줄) | 4시간 |
| `Strategies.tsx` | 1,384줄 | 4개 컴포넌트 (각 ~350줄) | 4시간 |

**🎯 analytics.rs 분리 구조**:

```
routes/analytics/
├── mod.rs              # 라우터 + re-export (100줄)
├── performance.rs      # 포트폴리오 성과 분석 (400줄)
├── charts.rs           # 차트 데이터 생성 (300줄)
└── indicators/
    ├── mod.rs          # 지표 라우터 (50줄)
    ├── sma.rs          # SMA (100줄)
    ├── ema.rs          # EMA (100줄)
    ├── rsi.rs          # RSI (100줄)
    ├── macd.rs         # MACD (150줄)
    ├── bollinger.rs    # Bollinger Bands (100줄)
    ├── stochastic.rs   # Stochastic (100줄)
    └── atr.rs          # ATR (100줄)
```

**🎯 Dataset.tsx 분리 구조**:

```
pages/Dataset/
├── index.tsx           # 메인 페이지 (300줄)
├── DatasetHeader.tsx   # 심볼 검색, 타임프레임 (200줄)
├── IndicatorPanel.tsx  # 지표 설정 UI (400줄)
├── DataTable.tsx       # OHLCV 데이터 테이블 (400줄)
└── ChartContainer.tsx  # 차트 영역 (300줄)
```

**소계**: 22시간

---

#### 7.3 타입 안전성 강화

| 항목 | 위치 | 예상 시간 |
|------|------|----------|
| `String` → `enum` (Rust) | `status`, `timeframe`, `side` 필드 | 4시간 |
| `any` 제거 (TypeScript) | `indicators.ts:247,253` 등 | 3시간 |
| WebSocket 타입 정의 | `types/index.ts:128-152` | 2시간 |

**🎯 Rust enum 정의**:

```rust
// crates/trader-core/src/types/enums.rs

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StrategyStatus {
    Running,
    Stopped,
    Error,
    Paused,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Timeframe {
    #[serde(rename = "1m")]  M1,
    #[serde(rename = "5m")]  M5,
    #[serde(rename = "15m")] M15,
    #[serde(rename = "1h")]  H1,
    #[serde(rename = "4h")]  H4,
    #[serde(rename = "1d")]  D1,
    #[serde(rename = "1w")]  W1,
    #[serde(rename = "1M")]  Mo1,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrderType {
    Market,
    Limit,
    StopLoss,
    TakeProfit,
}
```

**🎯 TypeScript 타입 강화**:

```typescript
// frontend/src/types/index.ts

// Before
interface WsOrderUpdate {
  status: string;      // ❌ 문자열
  side: string;        // ❌ 문자열
  order_type: string;  // ❌ 문자열
}

// After
type OrderStatus = 'pending' | 'partially_filled' | 'filled' | 'cancelled' | 'rejected';
type OrderSide = 'buy' | 'sell';
type OrderType = 'market' | 'limit' | 'stop_loss' | 'take_profit';

interface WsOrderUpdate {
  status: OrderStatus;     // ✅ 리터럴 타입
  side: OrderSide;         // ✅ 리터럴 타입
  order_type: OrderType;   // ✅ 리터럴 타입
}
```

**소계**: 9시간

---

#### 7.4 아키텍처 개선 (레이어 분리)

| 항목 | 현재 문제 | 예상 시간 |
|------|----------|----------|
| Routes → Repository 분리 | `analytics.rs`에서 직접 DB 쿼리 | 6시간 |
| Service 레이어 도입 | 비즈니스 로직 분리 | 4시간 |

**🎯 레이어 분리 상세**:

```
현재 (문제):
Routes (analytics.rs:655-670)
    ↓ 직접 쿼리 (레이어 위반)
Database

개선 후:
Routes (HTTP 핸들러)
    ↓
Services (비즈니스 로직)  ← 신규
    ↓
Repository (데이터 접근)
    ↓
Database
```

```rust
// 현재 (analytics.rs:655-670) - 레이어 위반
async fn get_position_metrics(pool: &PgPool, credential_id: &str) -> Result<...> {
    let positions = sqlx::query!(...).fetch_all(pool).await?;  // ❌ 직접 쿼리
}

// 개선 후 - Repository 사용
async fn get_position_metrics(
    State(state): State<AppState>,
    Path(credential_id): Path<String>,
) -> Result<Json<PositionMetrics>, ApiError> {
    let metrics = state.analytics_repo
        .get_position_metrics(&credential_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(metrics))
}
```

**소계**: 10시간

---

#### 7.5 Frontend 상태 관리 개선

| 항목 | 위치 | 예상 시간 |
|------|------|----------|
| Signal → Store 통합 | `Strategies.tsx:61-100` (30개+ Signal) | 4시간 |
| 모달 상태 객체화 | 각 페이지의 모달 상태 | 2시간 |

**🎯 상태 통합 상세**:

```typescript
// 현재 (Strategies.tsx:61-100) - 30개+ 분산된 Signal
const [showAddModal, setShowAddModal] = createSignal(false);
const [modalStep, setModalStep] = createSignal<'select' | 'configure'>('select');
const [selectedStrategy, setSelectedStrategy] = createSignal<BacktestStrategy | null>(null);
const [strategyParams, setStrategyParams] = createSignal<Record<string, unknown>>({});
const [formErrors, setFormErrors] = createSignal<Record<string, string>>({});
// ... 25개 더

// 개선 후 - createStore 사용
import { createStore } from 'solid-js/store';

interface StrategyPageState {
  filter: 'all' | 'running' | 'stopped';
  search: string;
  modals: {
    add: { open: boolean; step: 'select' | 'configure'; selected: BacktestStrategy | null };
    edit: { open: boolean; strategyId: string | null };
    delete: { open: boolean; strategy: Strategy | null };
    clone: { open: boolean; strategy: Strategy | null; name: string };
  };
  form: {
    params: Record<string, unknown>;
    errors: Record<string, string>;
    loading: boolean;
  };
}

const [state, setState] = createStore<StrategyPageState>({
  filter: 'all',
  search: '',
  modals: {
    add: { open: false, step: 'select', selected: null },
    edit: { open: false, strategyId: null },
    delete: { open: false, strategy: null },
    clone: { open: false, strategy: null, name: '' },
  },
  form: { params: {}, errors: {}, loading: false },
});

// 사용
setState('modals', 'add', 'open', true);
setState('modals', 'add', 'step', 'configure');
```

**소계**: 6시간

---

#### Phase 7 총 시간

| 카테고리 | 시간 |
|----------|------|
| 코드 중복 제거 | 4시간 |
| 대형 파일 분리 | 22시간 |
| 타입 안전성 강화 | 9시간 |
| 아키텍처 개선 | 10시간 |
| Frontend 상태 관리 | 6시간 |
| **소계** | **51시간** |

---

## 권장하지 않는 개선 ❌

### 1. 마이크로서비스 전환
- **이유**: 개인 프로젝트에 과도한 복잡성
- **대안**: 현재 모놀리스 유지

### 2. Kafka/RabbitMQ 도입
- **이유**: 운영 부담, 불필요한 인프라
- **대안**: 간단한 이벤트 로깅

### 3. 완벽한 테스트 커버리지
- **이유**: 시간 대비 효과 낮음
- **대안**: 핵심 기능만 테스트

### 4. clone() 대규모 최적화
- **이유**: 분석 결과 Copy trait 구현 어려움
- **대안**: 필요한 곳만 Arc 활용

### 5. 복잡한 CI/CD 파이프라인
- **이유**: 개인 사용에 불필요
- **대안**: Docker Compose 배포

---

## 예상 효과 요약

| 항목 | 개선 전 | 개선 후 | 비고 |
|------|---------|---------|------|
| **프로덕션 안정성** | 159개 unwrap() | 0개 | 에러 핸들링 |
| **API 응답 시간** | ~200ms | ~20ms | 캐싱 + 쿼리 최적화 |
| **백테스트 속도** | 1,000초 | 125초 | 병렬화 (8코어) |
| **테스트 커버리지** | ~10% | ~60% | 핵심 경로 |
| **대형 파일** | 4개 (2,000줄+) | 0개 | 모듈 분리 |
| **빌드 시간** | ~5분 | ~3.5분 | Feature flag |
| **동시 요청 처리** | 병목 발생 | 향상 | 락 홀드 최적화 |
| **코드 중복** | 전략간 중복 | 공통 모듈화 | 전략 공통 로직 |
| **전략 추가 시간** | 2시간 (5곳 수정) | 30분 (1곳) | 레지스트리 패턴 |
| **Repository** | 3개 | 11개 | 쿼리 재사용 |
| **외부 API 안정성** | 재시도 없음 | 지수 백오프 | Retry + Circuit Breaker |
| **의존성 보안** | 미점검 | 자동 점검 | cargo audit |
| **서버 종료** | 즉시 중단 | Graceful Shutdown | Tokio select! |
| **CPU 작업 처리** | 런타임 블로킹 | 별도 스레드 | spawn_blocking |
| **DB 트랜잭션** | 불일관적 | 헬퍼 함수 | SQLx 패턴 |
| **타임아웃 에러** | 불명확 | 명시적 응답 | HandleErrorLayer |
| **에러 응답 타입** | 4개 중복 | 1개 통합 | ApiErrorResponse |
| **대형 파일** | 4개 (2,000줄+) | 0개 | 모듈 분리 |
| **타입 안전성** | String/any 남용 | enum/리터럴 | 컴파일 타임 검증 |
| **Frontend 상태** | 30+ Signal 분산 | Store 통합 | createStore |

---

## 핵심 개선 포인트 (Top 15)

### 안정성 & 에러 처리
1. **에러 핸들링**: `unwrap()` 159개 제거 → `map_err()?` 패턴
2. **에러 응답 통합**: 4개 중복 타입 → 단일 `ApiErrorResponse`
3. **트랜잭션 안전성**: SQLx 트랜잭션 헬퍼로 데이터 일관성 보장

### 아키텍처 & 구조
4. **전략 자동화**: 레지스트리 패턴으로 전략 추가 시 1곳만 수정
5. **Repository 확장**: 3개 → 11개, 쿼리 로직 재사용 및 테스트 용이성
6. **대형 파일 분리**: 2,000줄+ 파일 4개 → 모듈화 (각 400줄 이하)
7. **레이어 분리**: Routes → Services → Repository 계층 명확화

### 성능 & 운영
8. **Graceful Shutdown**: Tokio select! 기반 안전한 서버 종료
9. **비동기 최적화**: spawn_blocking으로 CPU 집약 작업 분리
10. **운영 안정성**: 재시도 로직, 타임아웃, 의존성 보안 점검
11. **성능**: N+1 쿼리 해결, Redis 캐싱 레이어

### 타입 안전성 & 코드 품질
12. **Rust 타입 강화**: `String` → `enum` (StrategyStatus, Timeframe, OrderSide)
13. **TypeScript 타입 강화**: `any` 제거, 리터럴 타입 적용
14. **Frontend 상태 관리**: 30+ Signal → createStore 통합

### 문서화 & 테스트
15. **테스트 커버리지**: 핵심 로직 단위 테스트 추가

---

## 총 예상 시간 요약

| Phase | 내용 | 시간 |
|-------|------|------|
| Phase 1 | Critical (에러 핸들링, 파일 분리) | 19시간 |
| Phase 2 | High (Repository, 테스트, 캐싱) | 62시간 |
| Phase 3 | Medium (문서화, 타입 안전성) | 40시간 |
| Phase 4 | 전략 자동화 인프라 | 28시간 |
| Phase 5 | 운영 안정성 | 12시간 |
| Phase 6 | Rust API 최신 패턴 (Context7) | 22시간 |
| Phase 7 | 코드 리팩토링 (DRY, 분리, 타입) | 51시간 |
| **총계** | | **234시간** |

---

## 기존 문서 처리

이 문서가 다음 문서들을 대체합니다:
- ~~docs/code_optimize_suggestion.md~~ → 삭제 권장
- ~~docs/improve_suggestion.md~~ → 삭제 권장
- ~~docs/code_optimize_suggestion_improved.md~~ → 삭제 권장
- ~~docs/code_optimize_suggestion_improved2.md~~ → **기준 문서** (가장 최신)

---

*작성일: 2026-01-31*
*버전: 5.0 (코드 리팩토링 Phase 7 추가 - 총 234시간)*
*통합자: Claude Opus 4.5*
