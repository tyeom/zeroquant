# ZeroQuant - Claude 세션 컨텍스트

> 이 문서를 세션 시작 시 복사하여 Claude에게 컨텍스트를 제공하세요.
> 마지막 업데이트: 2026-02-01 | 버전: v0.5.3

---

## 🚀 작업 시작 전 확인사항

세션 시작 시 아래 문서를 확인하여 현재 작업 상태를 파악하세요:

| 문서 | 위치 | 용도 |
|------|------|------|
| **TODO** | `docs/todo.md` | 현재 진행 중/남은 작업, 작업 규칙, 실행 환경 |
| **PRD** | `docs/prd.md` | 제품 요구사항 문서 (PRD v5.0) |
| **개발 규칙** | `docs/development_rules.md` | 신규 기능 추가 시 필수 규칙 |
| **개선사항** | `docs/improvement_todo.md` | 코드베이스 개선 로드맵 |

---

## 📊 프로젝트 개요

**ZeroQuant**: Rust 기반 고성능 다중 시장 자동화 트레이딩 시스템

| 항목 | 수치 |
|------|------|
| Rust 파일 | 180+ |
| Crate 수 | 10개 |
| 전략 수 | 26개 |
| API 라우트 | 24개 |
| 마이그레이션 | 21개 |

### 기술 스택
- **Backend**: Rust, Tokio, Axum
- **Database**: PostgreSQL (TimescaleDB), Redis
- **Frontend**: SolidJS, TypeScript, Vite
- **ML**: ONNX Runtime
- **Infrastructure**: Podman, TimescaleDB

---

## 🐳 인프라 환경 (중요)

> ⚠️ **PostgreSQL과 Redis는 Podman/Docker 컨테이너에서 실행됩니다.**
> 로컬 `psql` 또는 `redis-cli` 명령어를 직접 사용하지 마세요.

### 컨테이너 정보

| 서비스 | 컨테이너명 | 포트 | 이미지 |
|--------|------------|------|--------|
| PostgreSQL | `trader-timescaledb` | 5432 | timescale/timescaledb:latest-pg15 |
| Redis | `trader-redis` | 6379 | redis:7-alpine |

### 접속 정보

```bash
# 환경 변수 (.env)
DATABASE_URL=postgresql://trader:trader_secret@localhost:5432/trader
REDIS_URL=redis://localhost:6379
```

| 항목 | 값 |
|------|-----|
| DB 사용자 | `trader` |
| DB 비밀번호 | `trader_secret` |
| DB 이름 | `trader` |

### 인프라 명령어

```bash
# 인프라 시작/중지
podman compose up -d          # 시작
podman compose down           # 중지
podman compose logs -f        # 로그 확인

# PostgreSQL 접속 (컨테이너 내부)
podman exec -it trader-timescaledb psql -U trader -d trader

# Redis 접속 (컨테이너 내부)
podman exec -it trader-redis redis-cli

# 컨테이너 상태 확인
podman ps
```

### 자주 사용하는 DB 쿼리

```bash
# 컨테이너 내부에서 SQL 실행
podman exec -it trader-timescaledb psql -U trader -d trader -c "SELECT COUNT(*) FROM symbol_info;"

# 테이블 목록 확인
podman exec -it trader-timescaledb psql -U trader -d trader -c "\dt"

# 마이그레이션 상태 확인
podman exec -it trader-timescaledb psql -U trader -d trader -c "SELECT * FROM _sqlx_migrations ORDER BY installed_on DESC LIMIT 5;"
```

### ❌ 잘못된 사용 예시

```bash
# ❌ 로컬 psql 직접 사용 (설치되어 있지 않거나 연결 실패)
psql -U trader -d trader

# ❌ 로컬 redis-cli 직접 사용
redis-cli
```

### ✅ 올바른 사용 예시

```bash
# ✅ 컨테이너를 통한 접속
podman exec -it trader-timescaledb psql -U trader -d trader
podman exec -it trader-redis redis-cli
```

---

## ⚠️ 에이전트 구현 가이드라인 (필독)

> **중요**: 코드 예시는 **참조용**입니다. 실제 구현 시 반드시 아래 가이드라인을 준수하세요.

### 🚨 핵심 원칙: 학습 데이터 의존 금지

AI 에이전트의 학습 데이터는 **과거 시점**의 정보입니다.
라이브러리 API는 지속적으로 변경되므로, **학습 데이터 기반 추측으로 코드를 작성하지 마세요**.

### ✅ 구현 전 필수 검증 절차

| 단계 | 작업 | 도구 |
|------|------|------|
| 1 | 대상 라이브러리의 현재 버전 확인 | `Cargo.toml`, `package.json` |
| 2 | 최신 API 문서 조회 | **Context7**, 공식 문서 |
| 3 | Breaking Changes 확인 | CHANGELOG, Migration Guide |
| 4 | 코드 예시 검증 | 공식 예제 저장소 |

### 📋 주요 라이브러리 검증 체크리스트

**Rust (Backend)**
- **Tokio**: select!, spawn, channel API 변경 빈번
- **Axum**: 0.6 → 0.7에서 Router, State API 대폭 변경됨
- **SQLx**: query!, query_as! 매크로 동작 변경 가능
- **Serde**: 안정적이나, derive 매크로 속성 확인 필요

**TypeScript/JavaScript (Frontend)**
- **SolidJS**: 1.x → 2.x 전환 시 reactivity 변경
- **Vite**: 설정 파일 구조 변경 빈번

### ❌ 금지 사항

1. **버전 미확인 코드 작성**
   - ❌ "tokio 1.x에서는 이렇게 합니다" (버전 미명시)
   - ✅ "tokio 1.35 기준으로 Context7에서 확인한 패턴입니다"

2. **Deprecated API 사용**
   - ❌ 학습 데이터에 있던 과거 API 사용
   - ✅ 현재 권장 API를 Context7/공식 문서에서 확인 후 사용

3. **추측 기반 import 경로**
   - ❌ `use tokio::something::Maybe;` (존재 여부 불확실)
   - ✅ 실제 코드베이스 또는 docs.rs에서 import 경로 확인

4. **Feature flag 미확인 사용**
   - ❌ tokio의 "full" feature에 포함되어 있을 것으로 가정
   - ✅ Cargo.toml의 features 섹션 확인 후 사용

5. **주석은 한글로 작성**
   - ✅ 모든 주석은 한글로 작성합니다
   - ✅ 이미 영문이라면 한글로 변경합니다

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

---

## 📁 프로젝트 구조

```
zeroquant/
├── crates/
│   ├── trader-core/         # 도메인 모델, 공통 유틸리티
│   ├── trader-exchange/     # 거래소 연동 (Binance, KIS)
│   ├── trader-strategy/     # 전략 엔진, 26개 전략
│   ├── trader-risk/         # 리스크 관리
│   ├── trader-execution/    # 주문 실행 엔진
│   ├── trader-data/         # 데이터 수집/저장 (OHLCV)
│   ├── trader-analytics/    # ML 추론, 성과 분석
│   ├── trader-api/          # REST/WebSocket API
│   │   ├── monitoring/      # 에러 추적 및 시스템 모니터링
│   │   ├── repository/      # 데이터 접근 계층 (Repository 패턴)
│   │   └── tasks/           # 백그라운드 작업 (심볼 동기화, 데이터 수집)
│   ├── trader-cli/          # CLI 도구
│   └── trader-notification/ # 알림 (Telegram)
├── data/                    # 정적 데이터 (KRX 종목코드, 섹터 매핑)
├── frontend/                # SolidJS + TypeScript + Vite
├── migrations/              # DB 마이그레이션 (21개)
└── scripts/                 # ML 훈련 파이프라인, 스크래퍼
```

---

## 🔄 최근 완료된 개선사항 (v0.5.3)

- ✅ **모니터링 에러 추적 시스템**: AI 디버깅용 구조화된 에러 로깅
- ✅ **CSV 심볼 동기화**: KRX/EOD 해외 거래소 종목 자동 동기화
- ✅ 매매일지 (Trading Journal) 기능
- ✅ 종목 스크리닝 API 및 프론트엔드
- ✅ OpenAPI/Swagger 문서화 (utoipa)
- ✅ Repository 패턴 12개 완료
- ✅ Graceful Shutdown (CancellationToken)

---

## 🔧 주요 시스템 사용 가이드

### 🔍 모니터링 에러 추적 시스템

에러 발생 시 구조화된 로그를 수집하고 AI 디버깅에 활용합니다.

```rust
use trader_api::monitoring::{global_tracker, ErrorRecordBuilder, ErrorSeverity, ErrorCategory};

// 에러 기록
let record = ErrorRecordBuilder::new("데이터베이스 쿼리 실패")
    .severity(ErrorSeverity::Error)
    .category(ErrorCategory::Database)
    .entity("AAPL")  // 관련 티커/ID
    .with_context("query", "SELECT * FROM ...")
    .raw_error(&e)
    .build();

global_tracker().record(record);

// 최근 에러 조회
let recent_errors = global_tracker().get_recent(10);
let stats = global_tracker().get_stats();
```

**모니터링 API 엔드포인트:**
| 엔드포인트 | 설명 |
|------------|------|
| `GET /api/v1/monitoring/errors` | 에러 목록 (필터: severity, category) |
| `GET /api/v1/monitoring/errors/critical` | Critical 에러만 조회 |
| `GET /api/v1/monitoring/stats` | 에러 통계 (심각도별/카테고리별) |
| `GET /api/v1/monitoring/summary` | 시스템 요약 (디버깅용) |

### 📊 CSV 심볼 동기화

정적 CSV 파일에서 종목 정보를 DB에 동기화합니다.

```rust
use trader_api::tasks::{krx_csv_sync, eod_csv_sync};

// KRX 종목 동기화
let result = krx_csv_sync::sync_krx_from_csv(pool, "data/krx_codes.csv").await?;
let sector_result = krx_csv_sync::update_sectors_from_csv(pool, "data/krx_sector_map.csv").await?;

// 해외 거래소 동기화 (EODData)
let result = eod_csv_sync::sync_eod_exchange(pool, "NYSE", "data/eod_nyse.csv").await?;
let all_results = eod_csv_sync::sync_eod_all(pool, "data/").await?;
```

**데이터 파일 위치:**
- `data/krx_codes.csv` - KRX 종목코드 (KOSPI/KOSDAQ)
- `data/krx_sector_map.csv` - KRX 업종 매핑
- `data/eod_*.csv` - 해외 거래소별 종목 (NYSE, NASDAQ 등)

---

## 📌 개선사항 참조

남은 개선사항은 `docs/improvement_todo.md`를 참조하세요.
