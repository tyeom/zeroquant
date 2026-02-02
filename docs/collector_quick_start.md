# Standalone Collector 빠른 시작 가이드

> **요약**: API 서버와 독립적으로 데이터를 수집하는 바이너리 구현 가이드

---

## 📋 전제 조건

- ✅ Rust 1.75+ 설치
- ✅ PostgreSQL (TimescaleDB) 실행 중
- ✅ `trader-data` crate 의존성 이해

---

## 🚀 빠른 시작 (3단계)

### 1단계: Crate 생성

```bash
cd crates
cargo new --bin trader-collector
cd trader-collector
```

### 2단계: Cargo.toml 설정

```toml
[package]
name = "trader-collector"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "trader-collector"
path = "src/main.rs"

[dependencies]
trader-core = { path = "../trader-core", features = ["sqlx-support"] }
trader-data = { path = "../trader-data" }

sqlx = { workspace = true }
rust_decimal = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
dotenvy = "0.15"
serde = { workspace = true }
chrono = { workspace = true }
clap = { version = "4", features = ["derive"] }
```

### 3단계: 최소 구현

**src/main.rs:**

```rust
use clap::{Parser, Subcommand};
use sqlx::PgPool;
use tracing_subscriber;

#[derive(Parser)]
#[command(name = "trader-collector")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// OHLCV 데이터 수집
    CollectOhlcv,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL")?;
    let pool = PgPool::connect(&database_url).await?;

    tracing::info!("데이터베이스 연결 성공");

    let cli = Cli::parse();

    match cli.command {
        Commands::CollectOhlcv => {
            collect_ohlcv(&pool).await?;
        }
    }

    Ok(())
}

async fn collect_ohlcv(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    use trader_data::storage::krx::KrxDataSource;

    let krx = KrxDataSource::new();
    let klines = krx.get_ohlcv("005930", "20260101", "20260203").await?;

    tracing::info!(count = klines.len(), "삼성전자 캔들 수집 완료");

    // TODO: DB 저장 로직 추가

    Ok(())
}
```

**실행:**

```bash
cargo run --bin trader-collector collect-ohlcv
```

---

## 📚 구현 단계별 가이드

### Phase 1: 기본 수집 (1주)

- [ ] CLI 인터페이스 (clap)
- [ ] 환경변수 설정 로더
- [ ] KRX 데이터 수집 + DB 저장
- [ ] 기본 로깅 및 에러 핸들링

### Phase 2: 배치 처리 (1주)

- [ ] 여러 심볼 배치 처리
- [ ] Rate Limiting (요청 간 딜레이)
- [ ] 진행률 로깅
- [ ] 통계 수집 (성공/실패/건너뛰기)

### Phase 3: 심볼 동기화 (1주)

- [ ] KRX 종목 목록 동기화
- [ ] 권위 있는 소스 원칙 구현
- [ ] 상폐 종목 비활성화

### Phase 4: Fundamental 수집 (1주)

- [ ] Yahoo Finance 연동
- [ ] Fundamental + OHLCV 통합 수집
- [ ] 실패 카운트 관리

### Phase 5: 운영 최적화 (1주)

- [ ] Cron/systemd 통합
- [ ] Docker 이미지 빌드
- [ ] 모니터링 및 알림

---

## 🛠️ 핵심 컴포넌트 사용 예제

### 1. KRX 데이터 수집

```rust
use trader_data::storage::krx::KrxDataSource;

let krx = KrxDataSource::new();
let klines = krx.get_ohlcv("005930", "20260101", "20260131").await?;

// klines: Vec<Kline>
for kline in klines {
    println!("{:?}", kline);
}
```

### 2. Yahoo Finance Fundamental 수집

```rust
use trader_data::cache::fundamental::FundamentalFetcher;

let mut fetcher = FundamentalFetcher::new()?;
let result = fetcher.fetch_with_ohlcv("005930.KS", "005930", "KR").await?;

// result.fundamental: FundamentalData
// result.klines: Vec<Kline>
// result.name: String (종목명)
```

### 3. DB 배치 저장

```rust
use trader_data::storage::ohlcv::OhlcvCache;
use trader_core::Timeframe;

let cache = OhlcvCache::new(pool.clone());
cache.save_klines("005930", Timeframe::D1, &klines).await?;
```

### 4. 심볼 정규화

```rust
use trader_data::provider::symbol_info::SymbolResolver;

let resolver = SymbolResolver::new(pool.clone());

// "005930.KS" → "005930"
let canonical = SymbolResolver::normalize_symbol("005930.KS");

// Canonical → Yahoo 심볼
let yahoo_symbol = resolver.to_source_symbol(&canonical, "yahoo").await?;
// Some("005930.KS")
```

---

## 🔍 트러블슈팅

### 컴파일 에러: "trader-data not found"

```bash
# workspace Cargo.toml에 추가
[workspace]
members = [
    "crates/trader-collector",  # 추가
    # ...
]
```

### DB 연결 실패

```bash
# Podman 컨테이너 상태 확인
podman ps | grep timescaledb

# 로그 확인
podman logs trader-timescaledb
```

### KRX API 에러

```rust
// 한국 주식 코드는 6자리 숫자여야 함
let klines = krx.get_ohlcv("005930", ...).await?;  // ✅
let klines = krx.get_ohlcv("삼성전자", ...).await?; // ❌
```

---

## 📊 예상 성능

| 항목 | 값 | 비고 |
|------|-----|------|
| **KRX 전체 수집** | ~20분 | 2500개 종목, Rate limit 500ms |
| **Fundamental 수집** | ~30분 | 1000개 종목, Rate limit 2초 |
| **메모리 사용량** | < 100MB | 배치 크기 50 기준 |
| **DB 저장 속도** | ~5000 klines/sec | UNNEST 최적화 |

---

## 📖 참고 문서

- **상세 설계**: `docs/standalone_collector_design.md`
- **환경변수**: `docs/collector_env_example.env`
- **기존 구현**: `crates/trader-api/src/tasks/fundamental.rs`

---

## ✅ 체크리스트

**개발 전 확인:**
- [ ] `standalone_collector_design.md` 읽음
- [ ] Podman 컨테이너 (PostgreSQL) 실행 중
- [ ] `.env` 파일 설정 완료
- [ ] `trader-data` crate 의존성 이해

**개발 중 확인:**
- [ ] Rate Limiting 구현 (API 차단 방지)
- [ ] 에러 핸들링 (단일 실패가 전체 중단하지 않음)
- [ ] 로깅 (tracing 활용)
- [ ] 배치 처리 (한 번에 대량 저장)

**배포 전 확인:**
- [ ] 통합 테스트 통과
- [ ] 로그 레벨 조정 (info 권장)
- [ ] Cron/systemd 스크립트 작성
- [ ] 모니터링 알림 설정

---

**Next Steps:**
1. Phase 1 구현 시작 (KRX 수집 + DB 저장)
2. 기존 `trader-api/src/tasks/fundamental.rs` 코드 참고
3. 단위 테스트 작성
4. 통합 테스트 (테스트 DB)
5. Production 배포
