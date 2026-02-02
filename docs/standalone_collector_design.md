# Standalone Collector 설계 문서

> **작성일**: 2026-02-03
> **버전**: v1.0
> **목적**: API 서버와 독립적으로 데이터 수집을 수행하는 standalone 바이너리 설계

---

## 📋 목차

1. [개요](#1-개요)
2. [요구사항](#2-요구사항)
3. [아키텍처 설계](#3-아키텍처-설계)
4. [재사용 가능 컴포넌트](#4-재사용-가능-컴포넌트)
5. [새로운 Crate 구조](#5-새로운-crate-구조)
6. [구현 가이드](#6-구현-가이드)
7. [실행 시나리오](#7-실행-시나리오)
8. [배포 전략](#8-배포-전략)

---

## 1. 개요

### 1.1 배경

현재 ZeroQuant의 데이터 수집 로직은 **trader-api 서버 내부의 백그라운드 태스크**로 실행됩니다:
- Fundamental 데이터 수집기 (`src/tasks/fundamental.rs`)
- 심볼 동기화 (`src/tasks/symbol_sync.rs`)
- CSV 동기화 (KRX, EOD)

**문제점:**
- API 서버 재시작 시 데이터 수집 중단
- 데이터 수집의 높은 I/O 부하가 API 응답 성능에 영향
- 스케줄링 유연성 부족 (Cron/systemd로 독립 실행 불가)
- 리소스 격리 불가 (별도 머신/컨테이너 배포 어려움)

### 1.2 목표

**독립 실행 가능한 collector 바이너리**를 구축하여:
1. ✅ API 서버와 완전히 독립적으로 동작
2. ✅ Cron/systemd로 주기적 실행 가능
3. ✅ 심볼 데이터 갱신, 상폐 종목 처리, OHLCV/Fundamental 수집 통합
4. ✅ 리소스 격리 및 독립 배포 지원

---

## 2. 요구사항

### 2.1 기능 요구사항

| 기능 | 설명 | 우선순위 |
|------|------|---------|
| **심볼 동기화** | KRX, Binance, Yahoo Finance에서 종목 목록 갱신 | 🔴 필수 |
| **상폐 종목 처리** | 권위 있는 소스에 없는 종목 자동 비활성화 | 🔴 필수 |
| **OHLCV 수집** | 일봉 데이터 수집 (KRX, Yahoo Finance) | 🔴 필수 |
| **Fundamental 수집** | 재무 지표 수집 (Yahoo Finance) | 🟡 중요 |
| **증분 업데이트** | 마지막 수집 시간 이후만 갱신 | 🟢 선택 |
| **CSV 임포트** | KRX/EOD CSV 파일 일괄 임포트 | 🟢 선택 |
| **배치 처리** | 대량 심볼 동시 처리 (청크 단위) | 🟡 중요 |
| **Rate Limiting** | API 차단 방지 (요청 간 딜레이) | 🔴 필수 |

### 2.2 비기능 요구사항

| 항목 | 요구사항 | 기준 |
|------|---------|------|
| **성능** | 1000개 심볼 처리 30분 이내 | Rate limit 2초 기준 |
| **안정성** | 단일 심볼 실패가 전체 배치 중단하지 않음 | 개별 에러 핸들링 |
| **가관측성** | 진행률, 성공/실패 통계 로깅 | tracing 활용 |
| **설정** | 환경변수 기반 설정 | .env 파일 지원 |
| **의존성** | 최소 의존성 (trader-data 중심) | API 서버 코드 제외 |

---

## 3. 아키텍처 설계

### 3.1 시스템 아키텍처

```
┌─────────────────────────────────────────────────────────────┐
│                    Standalone Collector                      │
│                   (trader-collector crate)                   │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌───────────────┐  ┌───────────────┐  ┌──────────────┐    │
│  │ Symbol Sync   │  │ OHLCV Collect │  │ Fundamental  │    │
│  │   Module      │  │    Module     │  │   Collect    │    │
│  └───────┬───────┘  └───────┬───────┘  └──────┬───────┘    │
│          │                   │                  │            │
│          └───────────────────┼──────────────────┘            │
│                              │                               │
│                    ┌─────────▼─────────┐                     │
│                    │  Core Orchestrator│                     │
│                    │  (batch processor)│                     │
│                    └─────────┬─────────┘                     │
│                              │                               │
├──────────────────────────────┼───────────────────────────────┤
│          Reusable Components (trader-data)                   │
├──────────────────────────────┼───────────────────────────────┤
│  ┌──────────────┐  ┌─────────▼──────┐  ┌─────────────┐     │
│  │KrxDataSource │  │SymbolResolver  │  │OhlcvCache   │     │
│  └──────────────┘  └────────────────┘  └─────────────┘     │
│  ┌──────────────┐  ┌────────────────┐  ┌─────────────┐     │
│  │Fundamental   │  │SymbolInfo      │  │Kline        │     │
│  │Fetcher       │  │Provider        │  │Repository   │     │
│  └──────────────┘  └────────────────┘  └─────────────┘     │
└─────────────────────────────────────────────────────────────┘
                              │
                    ┌─────────▼─────────┐
                    │   PostgreSQL      │
                    │  (TimescaleDB)    │
                    └───────────────────┘
```

### 3.2 데이터 흐름

```
[Cron/systemd 트리거]
    │
    ├─→ [심볼 동기화 모드]
    │   1. KRX API → 종목 목록 조회
    │   2. Binance API → USDT 페어 조회
    │   3. DB 업서트 (symbol_info 테이블)
    │   4. 권위 소스에 없는 종목 비활성화
    │
    ├─→ [OHLCV 수집 모드]
    │   1. 오래된 심볼 조회 (last_cached_time 기준)
    │   2. KRX/Yahoo Finance API 호출
    │   3. DB 저장 (ohlcv 테이블, 배치 UNNEST)
    │   4. 메타데이터 업데이트 (ohlcv_metadata)
    │
    └─→ [Fundamental 수집 모드]
        1. 오래된 심볼 조회 (fetched_at < 7일)
        2. Yahoo Finance API 호출
        3. DB 저장 (symbol_fundamental 테이블)
        4. OHLCV 함께 저장 (옵션)
        5. 실패 카운트 관리 (3회 초과 시 비활성화)
```

### 3.3 권위 있는 소스 원칙

| 시장 | 권위 있는 소스 | 동작 | 비활성화 조건 |
|------|--------------|------|-------------|
| **KR** | KRX | KRX에만 존재하는 종목 활성화 | KRX 조회 실패 (상폐 추정) |
| **CRYPTO** | Binance | USDT 페어만 활성화 | Binance에 없음 |
| **US/기타** | Yahoo Finance | Yahoo 조회 성공한 종목만 활성화 | 3회 연속 실패 |

---

## 4. 재사용 가능 컴포넌트

### 4.1 trader-data에서 가져올 컴포넌트

| 컴포넌트 | 위치 | 기능 | 재사용성 |
|---------|------|------|---------|
| **KrxDataSource** | `storage/krx.rs` | KRX API 일봉 조회 | ✅ 100% |
| **FundamentalFetcher** | `cache/fundamental.rs` | Yahoo Fundamental + OHLCV | ✅ 100% |
| **OhlcvCache** | `storage/ohlcv.rs` | OHLCV 테이블 배치 저장 | ✅ 100% |
| **SymbolResolver** | `provider/symbol_info.rs` | 심볼 정규화 및 변환 | ✅ 100% |
| **KlineRepository** | `storage/timescale.rs` | Kline 배치 INSERT | ✅ 90% |
| **SymbolInfoProvider** | `provider/symbol_info.rs` | KRX/Binance/Yahoo 종목 조회 | ✅ 100% |

### 4.2 trader-api에서 참고할 패턴

| 파일 | 학습할 패턴 | 라인 수 |
|------|-----------|---------|
| `tasks/fundamental.rs` | 배치 처리, Rate Limiting, 오류 복구 | 1000+ |
| `tasks/symbol_sync.rs` | 권위 소스 원칙, 비활성화 로직 | 800+ |
| `repository/symbol_info.rs` | DB 업서트, 실패 카운트 관리 | 400+ |

**재작성 vs 복사:**
- ✅ **재작성**: 로직은 유사하지만 API 서버 의존성 제거
- ❌ **복사**: 코드 중복 방지, trader-data 공개 API 활용

---

## 5. 새로운 Crate 구조

### 5.1 Crate 개요

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

# 데이터베이스
sqlx = { workspace = true }
rust_decimal = { workspace = true }

# 비동기 런타임
tokio = { workspace = true }

# 로깅
tracing = { workspace = true }
tracing-subscriber = { workspace = true }

# 설정
dotenvy = "0.15"
serde = { workspace = true }

# 날짜/시간
chrono = { workspace = true }

# CLI
clap = { version = "4", features = ["derive"] }
```

### 5.2 디렉토리 구조

```
crates/trader-collector/
├── Cargo.toml
├── src/
│   ├── main.rs                    # CLI 엔트리포인트
│   ├── lib.rs                     # 라이브러리 루트
│   │
│   ├── config.rs                  # 환경변수 기반 설정
│   ├── error.rs                   # CollectorError 타입
│   │
│   ├── modules/
│   │   ├── mod.rs
│   │   ├── symbol_sync.rs         # 심볼 동기화 모듈
│   │   ├── ohlcv_collect.rs       # OHLCV 수집 모듈
│   │   └── fundamental_collect.rs # Fundamental 수집 모듈
│   │
│   ├── orchestrator.rs            # 배치 처리 오케스트레이터
│   └── stats.rs                   # 수집 통계 구조체
│
├── .env.example                   # 환경변수 예제
└── README.md                      # 사용 가이드
```

---

## 6. 구현 가이드

### 6.1 Config 모듈 (config.rs)

```rust
use std::time::Duration;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct CollectorConfig {
    pub database_url: String,
    pub symbol_sync: SymbolSyncConfig,
    pub ohlcv_collect: OhlcvCollectConfig,
    pub fundamental_collect: FundamentalCollectConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SymbolSyncConfig {
    pub min_symbol_count: i64,        // 최소 심볼 수
    pub enable_krx: bool,             // KRX 동기화 활성화
    pub enable_binance: bool,         // Binance 동기화 활성화
    pub enable_yahoo: bool,           // Yahoo 동기화 활성화
    pub yahoo_max_symbols: usize,     // Yahoo 최대 수집 종목
}

#[derive(Debug, Clone, Deserialize)]
pub struct OhlcvCollectConfig {
    pub batch_size: i64,              // 배치당 심볼 수
    pub stale_days: i64,              // 갱신 기준 일수
    pub request_delay_ms: u64,        // API 요청 간 딜레이
    pub start_date: Option<String>,   // 시작 날짜 (YYYYMMDD)
    pub end_date: Option<String>,     // 종료 날짜 (YYYYMMDD)
}

#[derive(Debug, Clone, Deserialize)]
pub struct FundamentalCollectConfig {
    pub batch_size: i64,              // 배치당 심볼 수
    pub stale_days: i64,              // 갱신 기준 일수 (기본: 7일)
    pub request_delay_ms: u64,        // API 요청 간 딜레이
    pub include_ohlcv: bool,          // OHLCV 함께 수집 여부
}

impl CollectorConfig {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        dotenvy::dotenv().ok();

        let database_url = std::env::var("DATABASE_URL")?;

        Ok(Self {
            database_url,
            symbol_sync: SymbolSyncConfig {
                min_symbol_count: std::env::var("SYMBOL_SYNC_MIN_COUNT")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(100),
                enable_krx: std::env::var("SYMBOL_SYNC_KRX")
                    .map(|v| v == "true" || v == "1")
                    .unwrap_or(true),
                enable_binance: std::env::var("SYMBOL_SYNC_BINANCE")
                    .map(|v| v == "true" || v == "1")
                    .unwrap_or(false),
                enable_yahoo: std::env::var("SYMBOL_SYNC_YAHOO")
                    .map(|v| v == "true" || v == "1")
                    .unwrap_or(true),
                yahoo_max_symbols: std::env::var("SYMBOL_SYNC_YAHOO_MAX")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(500),
            },
            ohlcv_collect: OhlcvCollectConfig {
                batch_size: std::env::var("OHLCV_BATCH_SIZE")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(50),
                stale_days: std::env::var("OHLCV_STALE_DAYS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1),
                request_delay_ms: std::env::var("OHLCV_REQUEST_DELAY_MS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(500),
                start_date: std::env::var("OHLCV_START_DATE").ok(),
                end_date: std::env::var("OHLCV_END_DATE").ok(),
            },
            fundamental_collect: FundamentalCollectConfig {
                batch_size: std::env::var("FUNDAMENTAL_BATCH_SIZE")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(50),
                stale_days: std::env::var("FUNDAMENTAL_STALE_DAYS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(7),
                request_delay_ms: std::env::var("FUNDAMENTAL_REQUEST_DELAY_MS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(2000),
                include_ohlcv: std::env::var("FUNDAMENTAL_INCLUDE_OHLCV")
                    .map(|v| v == "true" || v == "1")
                    .unwrap_or(true),
            },
        })
    }
}
```

### 6.2 Stats 모듈 (stats.rs)

```rust
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CollectionStats {
    pub total: usize,           // 총 시도 횟수
    pub success: usize,         // 성공 횟수
    pub errors: usize,          // 에러 횟수
    pub skipped: usize,         // 건너뛴 횟수 (이미 최신)
    pub empty: usize,           // 빈 데이터 (조회 성공, 데이터 없음)
    pub total_klines: usize,    // 저장된 총 캔들 수
    pub elapsed: Duration,      // 소요 시간
}

impl CollectionStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn success_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.success as f64 / self.total as f64) * 100.0
        }
    }

    pub fn log_summary(&self, operation: &str) {
        tracing::info!(
            operation = operation,
            total = self.total,
            success = self.success,
            errors = self.errors,
            skipped = self.skipped,
            empty = self.empty,
            total_klines = self.total_klines,
            success_rate = format!("{:.1}%", self.success_rate()),
            elapsed = format!("{:.1}s", self.elapsed.as_secs_f64()),
            "수집 완료"
        );
    }
}
```

### 6.3 Symbol Sync 모듈 (modules/symbol_sync.rs)

```rust
use sqlx::PgPool;
use trader_data::provider::{
    KrxSymbolProvider, BinanceSymbolProvider, YahooSymbolProvider,
    SymbolInfoProvider, SymbolMetadata,
};
use crate::{CollectorConfig, CollectionStats};
use std::collections::HashSet;
use std::time::Instant;

pub async fn sync_symbols(
    pool: &PgPool,
    config: &CollectorConfig,
) -> Result<CollectionStats, Box<dyn std::error::Error + Send + Sync>> {
    let start = Instant::now();
    let mut stats = CollectionStats::new();

    // 1. 현재 심볼 수 확인
    let current_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM symbol_info")
        .fetch_one(pool)
        .await?;

    tracing::info!(current_count, min = config.symbol_sync.min_symbol_count, "심볼 수 확인");

    if current_count >= config.symbol_sync.min_symbol_count {
        tracing::info!("심볼 수 충분, 동기화 건너뛰기");
        return Ok(stats);
    }

    // 2. KRX 동기화
    if config.symbol_sync.enable_krx {
        match sync_krx_symbols(pool).await {
            Ok(count) => {
                stats.success += count;
                tracing::info!(count, "KRX 심볼 동기화 완료");
            }
            Err(e) => {
                stats.errors += 1;
                tracing::error!(error = %e, "KRX 동기화 실패");
            }
        }
    }

    // 3. Binance 동기화
    if config.symbol_sync.enable_binance {
        match sync_binance_symbols(pool).await {
            Ok(count) => {
                stats.success += count;
                tracing::info!(count, "Binance 심볼 동기화 완료");
            }
            Err(e) => {
                stats.errors += 1;
                tracing::error!(error = %e, "Binance 동기화 실패");
            }
        }
    }

    // 4. Yahoo 동기화 (선택)
    if config.symbol_sync.enable_yahoo {
        match sync_yahoo_symbols(pool, config.symbol_sync.yahoo_max_symbols).await {
            Ok(count) => {
                stats.success += count;
                tracing::info!(count, "Yahoo 심볼 동기화 완료");
            }
            Err(e) => {
                stats.errors += 1;
                tracing::error!(error = %e, "Yahoo 동기화 실패");
            }
        }
    }

    stats.elapsed = start.elapsed();
    stats.total = stats.success + stats.errors;

    Ok(stats)
}

async fn sync_krx_symbols(
    pool: &PgPool,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    let provider = KrxSymbolProvider::new()?;
    let symbols = provider.fetch_all().await?;

    // TODO: SymbolInfoRepository::upsert_batch() 활용
    // TODO: deactivate_missing_symbols() 구현

    Ok(symbols.len())
}

// 나머지 함수들도 유사한 패턴으로 구현...
```

### 6.4 CLI 엔트리포인트 (main.rs)

```rust
use clap::{Parser, Subcommand};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use trader_collector::{CollectorConfig, modules};

#[derive(Parser)]
#[command(name = "trader-collector")]
#[command(about = "ZeroQuant Standalone Data Collector", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// 로그 레벨 (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[derive(Subcommand)]
enum Commands {
    /// 심볼 정보 동기화 (KRX, Binance, Yahoo)
    SyncSymbols,

    /// OHLCV 데이터 수집 (일봉)
    CollectOhlcv {
        /// 특정 심볼만 수집 (쉼표로 구분)
        #[arg(long)]
        symbols: Option<String>,
    },

    /// Fundamental 데이터 수집
    CollectFundamental {
        /// 특정 심볼만 수집 (쉼표로 구분)
        #[arg(long)]
        symbols: Option<String>,
    },

    /// 전체 워크플로우 실행 (심볼 동기화 → OHLCV → Fundamental)
    RunAll,

    /// 데몬 모드: 주기적으로 전체 워크플로우 자동 실행
    Daemon,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // 로깅 초기화
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| cli.log_level.into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 설정 로드
    let config = CollectorConfig::from_env()?;

    // DB 연결
    let pool = sqlx::PgPool::connect(&config.database_url).await?;

    tracing::info!("데이터베이스 연결 성공");

    // 명령 실행
    match cli.command {
        Commands::SyncSymbols => {
            let stats = modules::symbol_sync::sync_symbols(&pool, &config).await?;
            stats.log_summary("심볼 동기화");
        }
        Commands::CollectOhlcv { symbols } => {
            let stats = modules::ohlcv_collect::collect_ohlcv(&pool, &config, symbols).await?;
            stats.log_summary("OHLCV 수집");
        }
        Commands::CollectFundamental { symbols } => {
            let stats = modules::fundamental_collect::collect_fundamental(&pool, &config, symbols).await?;
            stats.log_summary("Fundamental 수집");
        }
        Commands::RunAll => {
            tracing::info!("전체 워크플로우 시작");

            let sync_stats = modules::symbol_sync::sync_symbols(&pool, &config).await?;
            sync_stats.log_summary("심볼 동기화");

            let ohlcv_stats = modules::ohlcv_collect::collect_ohlcv(&pool, &config, None).await?;
            ohlcv_stats.log_summary("OHLCV 수집");

            let fund_stats = modules::fundamental_collect::collect_fundamental(&pool, &config, None).await?;
            fund_stats.log_summary("Fundamental 수집");

            tracing::info!("전체 워크플로우 완료");
        }
    }

    Ok(())
}
```

---

## 7. 실행 시나리오

### 7.1 수동 실행

```bash
# 환경변수 설정
export DATABASE_URL="postgresql://trader:trader_secret@localhost:5432/trader"
export SYMBOL_SYNC_KRX=true
export OHLCV_BATCH_SIZE=100

# 심볼 동기화만 실행
./target/release/trader-collector sync-symbols

# OHLCV 수집만 실행
./target/release/trader-collector collect-ohlcv

# 특정 심볼만 수집
./target/release/trader-collector collect-ohlcv --symbols "005930,000660,035420"

# 전체 워크플로우 실행
./target/release/trader-collector run-all

# 데몬 모드 (주기적 자동 실행)
./target/release/trader-collector daemon
```

### 7.2 데몬 모드 (권장)

**실시간 자동 수집을 위한 가장 간단한 방법**

```bash
# 기본 설정 (60분 주기)
./target/release/trader-collector daemon

# 주기 변경 (환경변수)
export DAEMON_INTERVAL_MINUTES=30
./target/release/trader-collector daemon

# 백그라운드 실행
nohup ./target/release/trader-collector daemon > collector.log 2>&1 &

# systemd 서비스로 실행 (자동 재시작 지원)
sudo systemctl start trader-collector-daemon
sudo systemctl enable trader-collector-daemon  # 부팅 시 자동 시작
```

**데몬 모드 systemd 서비스**:

```ini
# /etc/systemd/system/trader-collector-daemon.service

[Unit]
Description=ZeroQuant Data Collector Daemon
After=postgresql.service
Wants=postgresql.service

[Service]
Type=simple
User=trader
WorkingDirectory=/opt/zeroquant
EnvironmentFile=/opt/zeroquant/.env
ExecStart=/opt/zeroquant/bin/trader-collector daemon
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

```bash
# 활성화
sudo systemctl daemon-reload
sudo systemctl enable trader-collector-daemon
sudo systemctl start trader-collector-daemon

# 상태 확인
sudo systemctl status trader-collector-daemon
sudo journalctl -u trader-collector-daemon -f  # 로그 실시간 확인
```

### 7.3 Cron 스케줄링

```bash
# /etc/cron.d/trader-collector

# 매일 오전 9시: 심볼 동기화 (상폐 종목 체크)
0 9 * * * trader cd /app && ./trader-collector sync-symbols >> /var/log/trader/sync.log 2>&1

# 매일 오후 6시: OHLCV 수집 (장 마감 후)
0 18 * * * trader cd /app && ./trader-collector collect-ohlcv >> /var/log/trader/ohlcv.log 2>&1

# 매주 일요일 오전 2시: Fundamental 수집
0 2 * * 0 trader cd /app && ./trader-collector collect-fundamental >> /var/log/trader/fundamental.log 2>&1
```

### 7.4 systemd 서비스 (One-shot)

```ini
# /etc/systemd/system/trader-collector-ohlcv.service

[Unit]
Description=ZeroQuant OHLCV Collector
After=postgresql.service

[Service]
Type=oneshot
User=trader
WorkingDirectory=/opt/zeroquant
EnvironmentFile=/opt/zeroquant/.env
ExecStart=/opt/zeroquant/bin/trader-collector collect-ohlcv
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

```ini
# /etc/systemd/system/trader-collector-ohlcv.timer

[Unit]
Description=Daily OHLCV Collection at 18:00

[Timer]
OnCalendar=daily
OnCalendar=18:00
Persistent=true

[Install]
WantedBy=timers.target
```

```bash
# 활성화
sudo systemctl daemon-reload
sudo systemctl enable trader-collector-ohlcv.timer
sudo systemctl start trader-collector-ohlcv.timer

# 상태 확인
sudo systemctl status trader-collector-ohlcv.timer
```

---

## 8. 배포 전략

### 8.1 Docker 컨테이너

```dockerfile
# Dockerfile.collector

FROM rust:1.75 AS builder

WORKDIR /app
COPY . .
RUN cargo build --release --bin trader-collector

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/trader-collector /usr/local/bin/

ENTRYPOINT ["trader-collector"]
CMD ["run-all"]
```

```yaml
# docker-compose.collector.yml

services:
  trader-collector:
    build:
      context: .
      dockerfile: Dockerfile.collector
    environment:
      DATABASE_URL: postgresql://trader:trader_secret@timescaledb:5432/trader
      SYMBOL_SYNC_KRX: "true"
      OHLCV_BATCH_SIZE: "100"
    depends_on:
      - timescaledb
    restart: "no"  # Cron으로 실행
```

### 8.2 Kubernetes CronJob

```yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: trader-collector-ohlcv
spec:
  schedule: "0 18 * * *"  # 매일 18:00
  jobTemplate:
    spec:
      template:
        spec:
          containers:
          - name: collector
            image: zeroquant/trader-collector:latest
            args: ["collect-ohlcv"]
            env:
            - name: DATABASE_URL
              valueFrom:
                secretKeyRef:
                  name: trader-db-secret
                  key: url
            - name: OHLCV_BATCH_SIZE
              value: "100"
          restartPolicy: OnFailure
```

### 8.3 별도 머신 배포

```bash
# 빌드 서버에서
cargo build --release --bin trader-collector

# 수집 전용 머신으로 복사
scp target/release/trader-collector trader@collector-server:/opt/zeroquant/bin/

# 수집 서버에서
cd /opt/zeroquant
chmod +x bin/trader-collector

# .env 파일 생성
cat > .env <<EOF
DATABASE_URL=postgresql://trader:trader_secret@db-server:5432/trader
SYMBOL_SYNC_KRX=true
OHLCV_BATCH_SIZE=100
EOF

# Cron 등록
crontab -e
0 18 * * * cd /opt/zeroquant && ./bin/trader-collector collect-ohlcv
```

---

## 9. 테스트 전략

### 9.1 단위 테스트

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_env() {
        std::env::set_var("DATABASE_URL", "postgresql://test");
        std::env::set_var("SYMBOL_SYNC_MIN_COUNT", "50");

        let config = CollectorConfig::from_env().unwrap();
        assert_eq!(config.symbol_sync.min_symbol_count, 50);
    }

    #[tokio::test]
    async fn test_symbol_sync_krx() {
        // Mock DB 사용
        let pool = create_test_pool().await;

        let result = sync_krx_symbols(&pool).await;
        assert!(result.is_ok());
        assert!(result.unwrap() > 0);
    }
}
```

### 9.2 통합 테스트

```bash
# 테스트 DB 준비
docker run -d --name test-timescaledb \
  -e POSTGRES_USER=test \
  -e POSTGRES_PASSWORD=test \
  -e POSTGRES_DB=test_trader \
  -p 5433:5432 \
  timescale/timescaledb:latest-pg16

# 마이그레이션 실행
export DATABASE_URL="postgresql://test:test@localhost:5433/test_trader"
sqlx migrate run

# 통합 테스트 실행
cargo test --test integration
```

---

## 10. 모니터링 및 알림

### 10.1 로그 수집

```rust
// tracing_subscriber 설정 (main.rs)
use tracing_subscriber::fmt::format::FmtSpan;

tracing_subscriber::fmt()
    .with_span_events(FmtSpan::CLOSE)
    .json()  // JSON 형식으로 출력 (Elasticsearch 연동)
    .init();
```

### 10.2 메트릭 수집

```rust
// stats.rs에 메트릭 추가
impl CollectionStats {
    pub fn to_prometheus_metrics(&self) -> String {
        format!(
            "# HELP collector_total Total collection attempts\n\
             # TYPE collector_total counter\n\
             collector_total{{operation=\"ohlcv\"}} {}\n\
             # HELP collector_success Successful collections\n\
             # TYPE collector_success counter\n\
             collector_success{{operation=\"ohlcv\"}} {}\n\
             # HELP collector_errors Collection errors\n\
             # TYPE collector_errors counter\n\
             collector_errors{{operation=\"ohlcv\"}} {}\n",
            self.total, self.success, self.errors
        )
    }
}
```

### 10.3 실패 알림 (Telegram)

```bash
# Cron에서 실패 시 알림
0 18 * * * cd /app && ./trader-collector collect-ohlcv || \
  curl -X POST "https://api.telegram.org/bot$BOT_TOKEN/sendMessage" \
       -d chat_id=$CHAT_ID \
       -d text="⚠️ OHLCV 수집 실패: $(date)"
```

---

## 11. 마이그레이션 계획

### 11.1 단계적 전환

| 단계 | 작업 | 기간 |
|------|------|------|
| **1단계** | Standalone collector 개발 및 테스트 | 2주 |
| **2단계** | 별도 서버에서 병렬 실행 (API 서버와 동시) | 1주 |
| **3단계** | API 서버의 백그라운드 태스크 비활성화 | 1일 |
| **4단계** | API 서버 코드에서 수집 로직 제거 | 1주 |

### 11.2 롤백 계획

- Standalone collector 실패 시 API 서버의 백그라운드 태스크 재활성화
- 환경변수 `FUNDAMENTAL_COLLECT_ENABLED=true`로 즉시 복구

---

## 12. 향후 확장

### 12.1 추가 기능 후보

- [ ] 웹훅 알림 (Discord, Slack)
- [ ] 진행률 대시보드 (웹 UI)
- [ ] 분봉/시간봉 수집 지원
- [ ] 다중 거래소 확장 (Upbit, Bithumb)
- [ ] 재시도 메커니즘 개선 (지수 백오프)
- [ ] 분산 수집 (여러 worker 병렬)

### 12.2 성능 최적화

- [ ] 심볼별 동시 수집 (tokio::spawn 활용)
- [ ] Connection Pool 크기 최적화
- [ ] 배치 크기 자동 조정 (시스템 부하 기반)

---

## 13. 참고 자료

### 13.1 기존 구현 분석

- `crates/trader-api/src/tasks/fundamental.rs` (1000+ 줄) - 배치 처리 패턴
- `crates/trader-api/src/tasks/symbol_sync.rs` (800+ 줄) - 권위 소스 원칙
- `crates/trader-data/src/cache/historical.rs` - 증분 업데이트 로직

### 13.2 외부 문서

- [TimescaleDB Best Practices](https://docs.timescale.com/timescaledb/latest/how-to-guides/hypertables/)
- [SQLx Migration Guide](https://github.com/launchbadge/sqlx/blob/main/sqlx-cli/README.md)
- [systemd Timer 가이드](https://www.freedesktop.org/software/systemd/man/systemd.timer.html)

---

## 변경 이력

| 날짜 | 버전 | 변경 내용 |
|------|------|----------|
| 2026-02-03 | v1.0 | 초안 작성 |

