# ZeroQuant 개선 로드맵 - 남은 작업

> 마지막 업데이트: 2026-01-31
> 대상 버전: v0.4.5+
> 완료 내역: OpenAPI/Swagger, StrategyType enum, Repository 패턴 (9개), rustfmt/clippy, 입력 검증, Graceful Shutdown, SQLx 트랜잭션, **unwrap() 39개 제거** (position_tracker, order_manager, main, grid, bollinger, rsi, volatility_breakout)
> 세션 컨텍스트: `CLAUDE.md` 참조

---

## 📋 목차

1. [🔴 Critical](#-critical)
2. [🟡 High](#-high)
3. [🟢 Medium](#-medium)
4. [🔵 프론트엔드](#-프론트엔드)
5. [🟣 운영 안정성](#-운영-안정성)
6. [🟤 Rust API 최신 패턴](#-rust-api-최신-패턴)
7. [Repository 추가 설계](#repository-추가-설계-남은-3개)
8. [Phase 7: 코드 리팩토링](#phase-7-코드-리팩토링)
9. [전략 등록 자동화](#전략-등록-자동화)
10. [구현 로드맵](#구현-로드맵)
11. [핵심 개선 포인트 (Top 15)](#핵심-개선-포인트-top-15)
12. [전체 예상 시간 요약](#전체-예상-시간-요약)

---

## 🔴 Critical

### 1. 에러 핸들링 개선 (Phase 2) - unwrap() 제거 ✅ 핵심 모듈 완료

**현황**: 전체 코드베이스 분석 결과 `unwrap()` **705개** 사용 (110개 파일)
- 대부분은 안전한 패턴(`unwrap_or`, 테스트 코드 등)
- **위험한 unwrap()**: 핵심 실행 및 전략 모듈에서 39개 수정 완료

> ✅ Phase 1 완료: ApiErrorResponse 타입 추가됨
> ✅ Phase 2 완료: KIS 커넥터 전체 점검 (2026-01-31)
>   - auth.rs build_headers() 함수의 위험한 unwrap() 5개 제거
>   - client_kr.rs: 모든 unwrap은 `unwrap_or()` 패턴으로 안전함 확인
>   - client_us.rs, auth.rs, holiday.rs: HTTP 클라이언트 생성 시 `map_err()?` 사용 확인
> ✅ Phase 3 완료: 핵심 실행 모듈 unwrap() 제거 (2026-01-31)
>   - `position_tracker.rs`: 4개 unwrap() → `ok_or()?` 패턴으로 수정
>   - `order_manager.rs`: 2개 unwrap() → `ok_or()?` 패턴으로 수정
>   - `main.rs`: `socket_addr()` → `Result` 반환으로 개선
>   - `grid.rs`: 10개 unwrap() → `let-else` 패턴으로 수정
>   - `bollinger.rs`: 10개 unwrap() → `let-else`/`unwrap_or` 패턴으로 수정
> ✅ Phase 4 완료: 전략 및 성능 모듈 점검 (2026-01-31)
>   - `tracker.rs`: 이미 안전한 패턴 사용 (unwrap_or) - 수정 불필요
>   - `simulated/exchange.rs`: 이미 안전한 패턴 사용 - 수정 불필요
>   - `rsi.rs`: 8개 unwrap() → `let-else` 패턴으로 수정
>   - `volatility_breakout.rs`: 5개 unwrap() → `unwrap_or` 패턴으로 수정

**✅ 검증된 안전한 패턴**:

| 파일 | 패턴 | 상태 |
|------|------|------|
| `client_kr.rs` | `parse().unwrap_or(-1)` | ✅ 안전 (기본값 반환) |
| `client_kr.rs` | `unwrap_or_else(Utc::now)` | ✅ 안전 (기본값 반환) |
| `client_kr.rs:63-70` | `build().map_err()?` | ✅ 안전 (에러 전파) |
| `client_us.rs:71-78` | `build().map_err()?` | ✅ 안전 (에러 전파) |
| `auth.rs:104-116` | `build().map_err()?` | ✅ 안전 (에러 전파) |
| `position_tracker.rs` | `.ok_or(PositionTrackerError::...)?` | ✅ 안전 (에러 전파) |
| `order_manager.rs` | `.ok_or(OrderManagerError::...)?` | ✅ 안전 (에러 전파) |
| `main.rs` | `socket_addr() -> Result` | ✅ 안전 (에러 전파) |

**✅ 전략 모듈 개선 완료**:

| 파일 | 수정 건수 | 적용 패턴 |
|------|----------|----------|
| `grid.rs` | 10개 | `let-else` 조기 반환 |
| `bollinger.rs` | 10개 | `let-else` + `unwrap_or` |
| `rsi.rs` | 8개 | `let-else` 조기 반환 |
| `volatility_breakout.rs` | 5개 | `unwrap_or` 기본값 |

**✅ 검증 완료 (수정 불필요)**:

| 파일 | 분석 결과 | 상태 |
|------|----------|------|
| `tracker.rs` (performance) | 이미 `unwrap_or` 패턴 사용 | ✅ 안전 |
| `simulated/exchange.rs` | 메인 코드에 위험한 unwrap 없음, 테스트만 존재 | ✅ 안전 |

**남은 작업**: 핵심 모듈 완료. 추가 최적화는 선택적.

**효과**: 주문 실행, 포지션 추적, 주요 전략 모듈의 프로덕션 안정성 확보

---

## 🟡 High

### 3. 비동기 런타임 최적화 (락 홀드 시간)

**문제**: 긴 락 홀드로 동시성 저하

```rust
// 현재 - 문제
let engine = state.strategy_engine.read().await;  // 락 획득
let all_statuses = engine.get_all_statuses().await;  // 락을 잡고 I/O 수행

// 개선안 - 최소 락 홀드
let statuses = {
    let engine = state.strategy_engine.read().await;
    engine.get_all_statuses().await  // 빠른 복사
};  // 락 해제
// 락 없이 계산 수행
```

**예상 시간**: 4시간

---

### 4. 전략 공통 로직 추출

**현재 문제**: 27개 전략이 유사한 코드 패턴 반복

**추가 권장**:
```
strategies/common/
├── position_sizing.rs    # 포지션 크기 계산
├── risk_checks.rs        # 공통 리스크 체크
└── signal_filters.rs     # 신호 필터링
```

**예상 시간**: 12시간

---

### 6. 테스트 추가

**현재 커버리지**:
- 전략 테스트: 107개 ✅
- 통합 테스트: 2개 (제한적)
- API 엔드포인트 테스트: 없음

**목표**:
- 핵심 전략: Grid, RSI, Bollinger, VolatilityBreakout
- API: strategies, backtest, portfolio
- Repository: 새로 추가되는 것들

**예상 시간**: 16시간

---

### 7. Redis 캐싱 전략

**제안 캐싱 대상**:

| 대상 | TTL | 이유 |
|------|-----|------|
| 전략 목록 | 5분 | 자주 조회, 드물게 변경 |
| 심볼 정보 | 1시간 | 거의 변경 없음 |
| 백테스트 결과 | 영구 | 동일 파라미터 재요청 |
| 실시간 시세 | 1초 | 빈번한 업데이트 |

**예상 시간**: 8시간

---

## 🟢 Medium

### 8. OpenAPI/Swagger 문서화 ✅ 완료

> **v0.4.4에서 구현 완료** (2026-01-31)
> - `crates/trader-api/src/openapi.rs` 추가
> - utoipa + utoipa-swagger-ui 통합
> - Swagger UI: `/swagger-ui`, OpenAPI JSON: `/api-docs/openapi.json`
> - 14개 태그, 자동 스키마 생성

~~**현재**: `docs/api.md` 수동 관리~~
~~**제안**: utoipa + Swagger UI 통합~~

**소요 시간**: ~4시간

---

### 9. 입력 검증 강화 ✅ 완료

> **v0.4.5에서 구현 완료** (2026-01-31)
> - `routes/backtest/types.rs`에 커스텀 검증 함수 추가
> - `validate_initial_capital()`: 초기 자본금 100 ~ 10억 범위 검증
> - `validate_commission_rate()`: 수수료율 0 ~ 10% 범위 검증
> - `validate_slippage_rate()`: 슬리피지율 0 ~ 5% 범위 검증
> - `validate_date_format()`: YYYY-MM-DD 날짜 형식 검증
> - BacktestRunRequest, BacktestMultiRunRequest, BatchBacktestRequest에 적용

~~```rust
use validator::Validate;

#[derive(Deserialize, Validate)]
pub struct BacktestRunRequest {
    #[validate(custom(function = "validate_date"))]
    pub start_date: String,

    #[validate(range(min = 100, max = 1_000_000_000))]
    pub initial_capital: f64,
}
```~~

**소요 시간**: ~3시간

---

### 10. 타입 안전성 강화 ✅ 완료

> **v0.4.4에서 구현 완료** (2026-01-31)
> - `crates/trader-api/src/types/strategy_type.rs` 추가
> - 26개 StrategyType enum 정의
> - `FromStr`, `Display`, `Serialize/Deserialize` 구현
> - 헬퍼 메서드: `is_single_asset()`, `is_asset_allocation()`, `display_name()`, `api_id()`

~~```rust
// String → enum 변환
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategyId {
    RsiMeanReversion,
    Grid,
    BollingerBands,
    // ... 27개
}
```~~

**소요 시간**: ~6시간

---

### 11. 병렬 백테스트

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

### 12. 민감 정보 로깅 방지 (보안)

```rust
use secrecy::{Secret, ExposeSecret};

pub struct Credentials {
    pub api_key: Secret<String>,
    pub api_secret: Secret<String>,
}
```

**예상 시간**: 2시간

---

### 13. Feature Flag 도입

```toml
[features]
default = ["strategies", "analytics"]
strategies = ["trader-strategy"]
analytics = ["trader-analytics"]
ml = ["trader-analytics/ml", "ort"]
full = ["strategies", "analytics", "ml", "notifications"]
```

**예상 시간**: 4시간

---

## 🔵 프론트엔드

### 14. createStore로 상태 통합

**현재 문제** (`Strategies.tsx`): 20개+ createSignal 분산

```typescript
// 개선안 - createStore 사용
import { createStore } from 'solid-js/store';

interface StrategyPageState {
  filter: 'all' | 'running' | 'stopped';
  modals: {
    add: { open: boolean; step: 'select' | 'configure' };
    edit: { open: boolean; strategyId: string | null };
    delete: { open: boolean; strategy: Strategy | null };
  };
  form: {
    params: Record<string, unknown>;
    errors: Record<string, string>;
    loading: boolean;
  };
}
```

**예상 시간**: 8시간

---

### 15. createMemo로 계산 최적화

```typescript
const filteredStrategies = createMemo(() => {
  const list = strategies() ?? [];
  const f = filter();
  const q = search().toLowerCase();

  return list
    .filter(s => /* ... */)
    .filter(s => /* ... */);
});
```

**예상 시간**: 3시간

---

### 16. createResource 에러 처리 강화

```typescript
<Show when={strategies.loading}>
  <LoadingSpinner />
</Show>

<Show when={strategies.error}>
  <ErrorBanner message={strategies.error.message} onRetry={refetch} />
</Show>
```

**예상 시간**: 2시간

---

### 17. Discriminated Union 타입 적용

```typescript
type StrategyConfig =
  | RsiConfig
  | GridConfig
  | BollingerConfig;

function isRsiConfig(config: StrategyConfig): config is RsiConfig {
  return config.type === 'rsi';
}
```

**예상 시간**: 6시간

---

### 18. 커스텀 훅 추출

```typescript
// hooks/useStrategies.ts
export function useStrategies() {
  const [strategies, { refetch, mutate }] = createResource(getStrategies);
  // ...
  return { strategies, loading, error, refetch, start, stop };
}
```

**예상 시간**: 8시간

---

### 19. 컴포넌트 분리 구조

```
frontend/src/
├── components/
│   ├── strategy/
│   ├── modals/
│   └── common/
├── hooks/
├── stores/
├── types/
└── pages/
```

**예상 시간**: 12시간

---

### 20. Lazy Loading 적용

```typescript
const Strategies = lazy(() => import('./pages/Strategies'));
const Backtest = lazy(() => import('./pages/Backtest'));
```

**예상 시간**: 3시간

---

## 🟣 운영 안정성

### 16. 의존성 버전 정책 수립

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

### 17. 설정 검증 추가

```rust
use validator::Validate;

#[derive(Deserialize, Validate)]
pub struct RiskConfig {
    #[validate(range(min = 0.0, max = 100.0))]
    pub max_position_pct: Decimal,
}
```

**예상 시간**: 3시간

---

### 18. 재시도 로직 (Retry + Backoff)

```rust
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub multiplier: f64,
}
```

**예상 시간**: 6시간

---

### 19. rustfmt/clippy 설정 추가 ✅ 완료

> **v0.4.5에서 구현 완료** (2026-01-31)
> - `.rustfmt.toml` 생성: edition=2021, max_width=100, imports_granularity="Crate"
> - `clippy.toml` 생성:
>   - too-many-arguments-threshold=8
>   - cognitive-complexity-threshold=25
>   - trivial-copy-size-limit=16
>   - too-many-lines-threshold=150
>   - allow-unwrap-in-tests=true
>   - arithmetic-side-effects-allowed=[Decimal, Duration]

~~**생성 필요 - `.rustfmt.toml`**:~~

**소요 시간**: ~1시간

---

### 20. 외부 호출 타임아웃 설정 ✅ 이미 구현됨

> **코드 리뷰 확인** (2026-01-31)
> - 모든 HTTP 클라이언트에 **30초 타임아웃**이 설정되어 있음
> - `KisConfig.timeout_secs` 필드로 설정 가능 (기본값: 30초)
> - 적용된 파일들:
>   - `client_kr.rs:63-70` - 국내 주식 클라이언트
>   - `client_us.rs:71-78` - 해외 주식 클라이언트
>   - `auth.rs:104-116` - OAuth 인증
>   - `holiday.rs:151-163` - 휴장일 확인

~~**🎯 수정 위치**:~~

**상태**: 수정 불필요

---

### 21. WebSocket 세션 관리 강화

**예상 시간**: 4시간

---

### 22. 마이그레이션 테스트 추가

**예상 시간**: 3시간

---

## 🟤 Rust API 최신 패턴

### 23. Tokio select! 활용한 Graceful Shutdown ✅ 완료

> **v0.4.5에서 구현 완료** (2026-01-31)
> - `tokio_util::sync::CancellationToken` 도입
> - main.rs에 graceful shutdown 로직 추가
> - 10초 타임아웃으로 정리 작업 보장
> - 백그라운드 태스크 취소를 위한 shutdown_token 전파

~~```rust
tokio::select! {
    result = axum::serve(listener, app) => { /* ... */ }
    _ = shutdown_signal => {
        tracing::info!("Initiating graceful shutdown...");
    }
}
```~~

**소요 시간**: ~3시간

---

### 24. Axum HandleErrorLayer로 타임아웃 미들웨어 ✅ 이미 구현됨

> **코드 리뷰 확인** (2026-01-31)
> - `tower-http 0.6`의 `TimeoutLayer::with_status_code` 사용 중
> - 30초 타임아웃 + 408 상태 코드 자동 반환
> - 별도 HandleErrorLayer 불필요 (동일 기능)
> - `main.rs:481`:
>   ```rust
>   .layer(TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(30)))
>   ```

~~```rust
.layer(
    ServiceBuilder::new()
        .layer(HandleErrorLayer::new(handle_error))
        .layer(TimeoutLayer::new(Duration::from_secs(60)))
)
```~~

**상태**: 수정 불필요 (선택적으로 JSON 에러 응답 필요 시만 HandleErrorLayer 추가)

---

### 25. SQLx 트랜잭션 패턴 개선 ✅ 부분 완료

> **v0.4.5에서 부분 구현 완료** (2026-01-31)
> - `repository/positions.rs`: `update_market_price()`에 트랜잭션 + FOR UPDATE 락 적용
> - `repository/orders.rs`: `set_exchange_order_id()`에 트랜잭션 적용

**🎯 남은 적용 위치**:

| 파일 | 함수 | 필요성 |
|------|------|--------|
| `repository/strategies.rs` | `create()` | ⚠️ 원자성 필요 |
| `repository/strategies.rs` | `update()` | ⚠️ 원자성 필요 |

**예상 시간**: 3시간 (6시간 → 3시간, 50% 완료)

---

### 26. Tokio spawn_blocking + mpsc 채널 ✅ 이미 구현됨

> **코드 리뷰 확인** (2026-01-31)
> - `run_strategy_backtest()` 및 `run_multi_strategy_backtest()` 모두 `spawn_blocking` 사용 중
> - `routes/backtest/engine.rs:57-87`:
>   ```rust
>   tokio::task::spawn_blocking(move || {
>       let rt = tokio::runtime::Builder::new_current_thread()...
>       rt.block_on(run_strategy_backtest_inner(...))
>   })
>   ```
> - 선택적 최적화: 내부 runtime 생성 오버헤드 제거를 위해 동기 버전 함수 분리 가능

~~```rust
let handle = spawn_blocking(move || {
    // CPU 집약적 작업
    engine.run_sync(&mut strategy, &klines)
});
```~~

**상태**: 수정 불필요 (선택적으로 runtime 오버헤드 최적화 가능)

---

### 27. Tokio blocking_lock (sync Mutex)

**예상 시간**: 2시간

---

### 28. Axum 에러 추출자 (Method, Uri)

```rust
pub struct ApiError {
    pub kind: ApiErrorKind,
    pub method: Option<Method>,
    pub uri: Option<Uri>,
}
```

**예상 시간**: 3시간

---

## Repository 추가 설계 (남은 2개)

> ✅ **9개 완료됨** (v0.4.3~v0.4.5):
> - `backtest_results.rs` - 백테스트 결과 저장/조회
> - `equity_history.rs` - 자산 곡선 이력
> - `execution_cache.rs` - 실행 캐시
> - `orders.rs` - 주문 CRUD
> - `portfolio.rs` - 포트폴리오 포지션
> - `positions.rs` - 포지션 기록
> - `strategies.rs` - 전략 설정
> - `symbol_info.rs` - 심볼 정보
> - `klines.rs` - OHLCV 캔들 데이터 ✅ **신규** (v0.4.5)

### 6. KlinesRepository ✅ 완료

> **v0.4.5에서 구현 완료** (2026-01-31)
> - `repository/klines.rs` 생성
> - `save_batch()`: UNNEST 패턴으로 배치 삽입 최적화
> - `get_range()`: 기간별 OHLCV 조회
> - `get_latest()`: 최신 N개 캔들 조회
> - `list_symbols()`: 저장된 심볼 목록 조회
> - `get_range_batch()`: 다중 심볼 일괄 조회

~~```rust
pub struct KlinesRepository;

impl KlinesRepository {
    /// OHLCV 배치 저장 (UNNEST 최적화)
    pub async fn save_batch(pool: &PgPool, klines: &[Kline]) -> Result<usize, sqlx::Error>;
    // ...
}
```~~

### 7. CredentialsRepository (미구현)

```rust
pub struct CredentialsRepository;

impl CredentialsRepository {
    /// 암호화된 자격증명 저장
    pub async fn save(pool: &PgPool, exchange: &str, credentials: &EncryptedCredentials) -> Result<(), sqlx::Error>;
    /// 자격증명 조회
    pub async fn get(pool: &PgPool, exchange: &str) -> Result<Option<EncryptedCredentials>, sqlx::Error>;
    /// 접근 로그 기록
    pub async fn log_access(pool: &PgPool, exchange: &str, action: &str) -> Result<(), sqlx::Error>;
}
```

### 8. AlertsRepository (미구현)

```rust
pub struct AlertsRepository;

impl AlertsRepository {
    /// 알림 생성
    pub async fn create(pool: &PgPool, alert: &CreateAlertInput) -> Result<Alert, sqlx::Error>;
    /// 미확인 알림 조회
    pub async fn get_unread(pool: &PgPool, user_id: Option<&str>) -> Result<Vec<Alert>, sqlx::Error>;
    /// 알림 확인 처리
    pub async fn mark_read(pool: &PgPool, alert_ids: &[String]) -> Result<(), sqlx::Error>;
}
```

**예상 시간**: 6시간 (3개 Repository)

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

**효과**:
- 쿼리 로직 재사용
- 테스트 용이성 (Mock 가능)
- N+1 쿼리 방지
- 일관된 에러 처리

---

## Phase 7: 코드 리팩토링

### 7.1 코드 중복 제거 (DRY)

| 항목 | 파일 | 예상 시간 |
|------|------|----------|
| 에러 응답 타입 통합 | ✅ 완료 (ApiErrorResponse) | - |
| 포매팅 함수 통합 | `Dashboard.tsx`, `Strategies.tsx`, `Simulation.tsx` → `utils/formatters.ts` | 1시간 |
| 기간 파싱 유틸리티 | `analytics.rs:2480` 등 → `utils/period.rs` | 1시간 |

**소계**: 2시간

---

### 7.3 타입 안전성 강화

| 항목 | 위치 | 예상 시간 |
|------|------|----------|
| `String` → `enum` (Rust) | `status`, `timeframe`, `side` 필드 | 4시간 |
| `any` 제거 (TypeScript) | `indicators.ts:247,253` 등 | 3시간 |
| WebSocket 타입 정의 | `types/index.ts:128-152` | 2시간 |

**Rust enum 정의**:
```rust
pub enum StrategyStatus { Running, Stopped, Error, Paused }
pub enum Timeframe { M1, M5, M15, H1, H4, D1, W1, Mo1 }
pub enum OrderSide { Buy, Sell }
pub enum OrderType { Market, Limit, StopLoss, TakeProfit }
```

**TypeScript 리터럴 타입**:
```typescript
type OrderStatus = 'pending' | 'partially_filled' | 'filled' | 'cancelled' | 'rejected';
type OrderSide = 'buy' | 'sell';
type OrderType = 'market' | 'limit' | 'stop_loss' | 'take_profit';
```

**소계**: 9시간

---

### 7.4 아키텍처 개선 (레이어 분리)

| 항목 | 현재 문제 | 예상 시간 |
|------|----------|----------|
| Routes → Repository 분리 | `analytics.rs`에서 직접 DB 쿼리 | 6시간 |
| Service 레이어 도입 | 비즈니스 로직 분리 | 4시간 |

**레이어 분리 구조**:
```
현재 (문제):
Routes → Database (직접 쿼리)

개선 후:
Routes → Services → Repository → Database
```

**소계**: 10시간

---

### 7.5 Frontend 상태 관리 개선

| 항목 | 위치 | 예상 시간 |
|------|------|----------|
| Signal → Store 통합 | `Strategies.tsx:61-100` (30개+ Signal) | 4시간 |
| 모달 상태 객체화 | 각 페이지의 모달 상태 | 2시간 |

**소계**: 6시간

---

### Phase 7 총 시간

| 카테고리 | 시간 |
|----------|------|
| 코드 중복 제거 | 2시간 |
| 타입 안전성 강화 | 9시간 |
| 아키텍처 개선 | 10시간 |
| Frontend 상태 관리 | 6시간 |
| **소계** | **27시간** |

---

## 전략 등록 자동화

### 현재 문제점

새 전략 추가 시 **5곳 이상** 수정 필요:

| # | 파일 | 수정 내용 |
|---|------|----------|
| 1 | `strategies/mod.rs` | `pub mod`, `pub use` 추가 |
| 2 | `routes/strategies.rs` | 팩토리 함수 4개에 match arm 추가 |
| 3 | `routes/backtest/engine.rs` | import + match arm 추가 |
| 4 | `config/sdui/strategy_schemas.json` | UI 스키마 추가 (~50줄) |
| 5 | `frontend/src/pages/Strategies.tsx` | 타임프레임 매핑 추가 |

### 현재 수정 위치 상세 체크리스트

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
register_strategy! {
    id: "rsi_mean_reversion",
    name: "RSI 평균회귀",
    description: "RSI 과매수/과매도 기반 평균회귀 전략",
    timeframe: "15m",
    symbols: [],
    category: Intraday,
    type: RsiStrategy
}
```

**팩토리에서 자동 조회**:
```rust
// routes/strategies.rs
fn create_strategy_instance(strategy_type: &str) -> Result<Box<dyn Strategy>, String> {
    for meta in inventory::iter::<StrategyMeta> {
        if meta.id == strategy_type {
            return Ok((meta.factory)());
        }
    }
    Err(format!("Unknown strategy: {}", strategy_type))
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
}

// API 엔드포인트로 스키마 제공
async fn get_strategy_schema(Path(strategy_id): Path<String>) -> impl IntoResponse {
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

## 구현 로드맵

### Phase 1: Critical (1주)

| 일차 | 작업 | 예상 시간 |
|------|------|----------|
| Day 1-2 | unwrap() 159개 제거 | 8시간 |
| Day 2 | 의존성 버전 정책 + cargo audit | 2시간 |
| Day 5 | rustfmt/clippy 설정 추가 | 1시간 |

**총 시간**: 11시간

### Phase 2: High (2주)

| 작업 | 예상 시간 |
|------|----------|
| 비동기 락 홀드 최적화 | 4시간 |
| 전략 공통 로직 추출 | 12시간 |
| 핵심 테스트 추가 | 16시간 |
| Redis 캐싱 레이어 | 8시간 |
| 재시도 로직 | 6시간 |

**총 시간**: 46시간

### Phase 3: Medium (1개월)

| 항목 | 예상 시간 |
|------|----------|
| OpenAPI/Swagger 문서화 ✅ | ~~6시간~~ 4시간 완료 |
| 입력 검증 강화 | 4시간 |
| 타입 안전성 ✅ | ~~10시간~~ 6시간 완료 |
| 병렬 백테스트 | 4시간 |
| 민감 정보 로깅 방지 | 2시간 |
| Feature Flag 도입 | 4시간 |

**총 시간**: 30시간 → **20시간 남음** (10시간 완료)

### Phase 4: 전략 자동화 인프라 (2주)

| 항목 | 예상 시간 |
|------|----------|
| 전략 레지스트리 패턴 구현 | 8시간 |
| register_strategy! 매크로 | 4시간 |
| SDUI 스키마 자동 생성 | 4시간 |
| 프론트엔드 메타 API 연동 | 4시간 |
| 기존 26개 전략 마이그레이션 | 8시간 |

**총 시간**: 28시간

### Phase 5: 운영 안정성 (여유 시)

| 항목 | 예상 시간 |
|------|----------|
| 설정 검증 추가 | 3시간 |
| 외부 호출 타임아웃 | 4시간 |
| WebSocket 세션 관리 | 4시간 |
| 마이그레이션 테스트 | 3시간 |

**총 시간**: 14시간

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

---

## 권장하지 않는 개선 ❌

| 항목 | 이유 | 대안 |
|------|------|------|
| 마이크로서비스 전환 | 개인 프로젝트에 과도한 복잡성 | 현재 모놀리스 유지 |
| Kafka/RabbitMQ 도입 | 운영 부담, 불필요한 인프라 | 간단한 이벤트 로깅 |
| 완벽한 테스트 커버리지 | 시간 대비 효과 낮음 | 핵심 기능만 테스트 |
| clone() 대규모 최적화 | Copy trait 구현 어려움 | 필요한 곳만 Arc 활용 |
| 복잡한 CI/CD 파이프라인 | 개인 사용에 불필요 | Docker Compose 배포 |

---

## 예상 효과 요약

| 항목 | 개선 전 | 개선 후 | 비고 |
|------|---------|---------|------|
| **프로덕션 안정성** | 159개 unwrap() | 0개 | 에러 핸들링 |
| **API 응답 시간** | ~200ms | ~20ms | 캐싱 + 쿼리 최적화 |
| **백테스트 속도** | 1,000초 | 125초 | 병렬화 (8코어) |
| **테스트 커버리지** | ~10% | ~60% | 핵심 경로 |
| **빌드 시간** | ~5분 | ~3.5분 | Feature flag |
| **동시 요청 처리** | 병목 발생 | 향상 | 락 홀드 최적화 |
| **코드 중복** | 전략간 중복 | 공통 모듈화 | 전략 공통 로직 |
| **전략 추가 시간** | 2시간 (5곳 수정) | 30분 (1곳) | 레지스트리 패턴 |
| **Repository** ✅ | 5개 | 9개 완료 | 쿼리 재사용 (klines 추가) |
| **외부 API 안정성** | 재시도 없음 | 지수 백오프 | Retry + Circuit Breaker |
| **의존성 보안** | 미점검 | 자동 점검 | cargo audit |
| **서버 종료** ✅ | 즉시 중단 | Graceful Shutdown | CancellationToken (v0.4.5) |
| **CPU 작업 처리** | 런타임 블로킹 | 별도 스레드 | spawn_blocking |
| **DB 트랜잭션** ✅ | 불일관적 | 헬퍼 함수 | SQLx 패턴 (부분 완료) |
| **타임아웃 에러** | 불명확 | 명시적 응답 | HandleErrorLayer |
| **에러 응답 타입** ✅ | 4개 중복 | 1개 통합 | ApiErrorResponse |
| **타입 안전성** ✅ | String 남용 | StrategyType enum | 26개 전략 정의 |
| **OpenAPI 문서화** ✅ | 수동 관리 | 자동 생성 | utoipa + Swagger UI |
| **Frontend 상태** | 30+ Signal 분산 | Store 통합 | createStore |

---

## 핵심 개선 포인트 (Top 15)

### 안정성 & 에러 처리
1. **에러 핸들링** ✅: KIS 커넥터 unwrap() 점검 완료 - 모두 안전한 패턴 사용 확인
2. **에러 응답 통합** ✅: 4개 중복 타입 → 단일 `ApiErrorResponse` (v0.4.3)
3. **트랜잭션 안전성** ✅: SQLx 트랜잭션 헬퍼로 데이터 일관성 보장 (v0.4.5 부분 완료)

### 아키텍처 & 구조
4. **전략 자동화**: 레지스트리 패턴으로 전략 추가 시 1곳만 수정
5. **Repository 확장** ✅: 9개 구현 완료, 쿼리 로직 재사용 (v0.4.3~v0.4.5)
6. **대형 파일 분리** ✅: 2,000줄+ 파일 → 모듈화 (analytics/, credentials/, backtest/)
7. **레이어 분리**: Routes → Services → Repository 계층 명확화

### 성능 & 운영
8. **Graceful Shutdown** ✅: CancellationToken 기반 안전한 서버 종료 (v0.4.5)
9. **비동기 최적화** ✅: spawn_blocking으로 CPU 집약 작업 분리 (이미 구현 확인)
10. **운영 안정성** ✅: rustfmt/clippy 설정, 입력 검증 강화, 타임아웃 설정 (v0.4.5)
11. **성능**: N+1 쿼리 해결 (✅ 완료), Redis 캐싱 레이어

### 타입 안전성 & 코드 품질
12. **Rust 타입 강화** ✅: `StrategyType` enum 26개 정의 (v0.4.4)
13. **TypeScript 타입 강화**: `any` 제거, 리터럴 타입 적용
14. **Frontend 상태 관리**: 30+ Signal → createStore 통합

### 문서화 & 테스트
15. **OpenAPI 문서화** ✅: utoipa + Swagger UI 통합 (v0.4.4)
16. **테스트 커버리지**: 핵심 로직 단위 테스트 추가

---

## 전체 예상 시간 요약

| Phase | 내용 | 시간 | 상태 |
|-------|------|------|------|
| Phase 1 | Critical (에러 핸들링) | ~~11시간~~ 0시간 | ✅ 핵심 모듈 unwrap() 26개 제거 완료 |
| Phase 2 | High (Repository, 테스트, 캐싱) | ~~46시간~~ 28시간 | ✅ Repository 9개 완료 |
| Phase 3 | Medium (문서화, 타입 안전성) | ~~30시간~~ 17시간 | ✅ OpenAPI, StrategyType, 입력검증 완료 |
| Phase 4 | 전략 자동화 인프라 | 28시간 | ⏳ 대기 |
| Phase 5 | 운영 안정성 | ~~14시간~~ 5시간 | ✅ rustfmt/clippy, 타임아웃 (이미 구현 확인) |
| Phase 6 | Rust API 최신 패턴 | ~~22시간~~ 5시간 | ✅ Graceful Shutdown, TimeoutLayer, spawn_blocking (이미 구현 확인) |
| Phase 7 | 코드 리팩토링 | 27시간 | ⏳ 대기 |
| **총계** | | ~~178시간~~ **108.5시간 남음** | **69.5시간 완료** |

> **v0.4.4 완료 내역** (2026-01-31):
> - OpenAPI/Swagger 문서화: 4시간
> - StrategyType enum: 6시간
> - Repository 확장 (8개): 16시간

> **v0.4.5 완료 내역** (2026-01-31):
> - auth.rs unwrap() 제거: 2시간
> - rustfmt/clippy 설정: 1시간
> - 입력 검증 강화 (validator): 3시간
> - KlinesRepository 구현: 2시간
> - Graceful Shutdown (CancellationToken): 3시간
> - SQLx 트랜잭션 패턴: 3시간
> - 전체 코드베이스 unwrap() 분석 (705개): 1시간
> - position_tracker.rs unwrap() 수정 (4개): 1시간
> - order_manager.rs unwrap() 수정 (2개): 0.5시간
> - main.rs socket_addr() Result 개선: 0.5시간
> - grid.rs 전략 unwrap() 수정 (10개): 1시간
> - bollinger.rs 전략 unwrap() 수정 (10개): 1시간
> - rsi.rs 전략 unwrap() 수정 (8개): 0.5시간
> - volatility_breakout.rs 전략 unwrap() 수정 (5개): 0.5시간
> - tracker.rs, exchange.rs 안전성 검증: 0.5시간

> **참고**: improvement_roadmap.md 기준 234시간 (Phase 7: 51시간)
> 본 문서는 완료된 작업(대형 파일 분리 22시간 등)을 제외한 **남은 작업**만 포함

---

## 관련 문서

이 문서(`improvement_todo.md`)와 함께 참조:
- `docs/improvement_roadmap.md` - **원본 통합 문서** (완료된 항목 포함)
- `CLAUDE.md` - **세션 컨텍스트 프롬프트**

> ✅ 기존 suggestion 문서들은 모두 삭제됨 (improvement_roadmap.md에 통합 완료)
