<p align="center">
  <h1 align="center">ZeroQuant</h1>
  <p align="center">
    <strong>Rust 기반 고성능 다중 시장 자동화 트레이딩 시스템</strong>
  </p>
  <p align="center">
    <a href="#주요-기능">주요 기능</a> •
    <a href="#지원-전략">지원 전략</a> •
    <a href="#전략-개발-가이드">전략 개발</a> •
    <a href="#빠른-시작">빠른 시작</a> •
    <a href="#문서">문서</a>
  </p>
</p>

---

## 소개

ZeroQuant는 암호화폐와 주식 시장에서 **24/7 자동화된 거래**를 수행하는 트레이딩 시스템입니다.

검증된 **16가지 통합 전략**과 **50개 ML 패턴 인식** (캔들스틱 26개 + 차트 패턴 24개)을 통해 **그리드 트레이딩**, **자산배분**, **모멘텀** 등 다양한 투자 방법론을 지원합니다. 웹 대시보드에서 실시간 모니터링과 전략 제어가 가능하며, 리스크 관리 시스템이 자동으로 자산을 보호합니다.

> ⚠️ **v0.7.0 전략 리팩토링**: 기존 26개 전략이 16개로 통합되었습니다. 유사 기능을 가진 전략들이 하나의 모듈로 병합되어 유지보수성이 향상되었습니다.

## 주요 기능

### 🏦 다중 시장 지원
| 시장 | 거래소 | 기능 |
|------|--------|------|
| 암호화폐 | Binance | 현물 거래, WebSocket 실시간 시세 |
| 한국/미국 주식 | 한국투자증권 (KIS) | 국내/해외 주식, 모의투자 지원 |

### 📊 데이터 & 분석
- **다중 데이터 소스**: KRX OPEN API, 네이버 금융 (국내), Yahoo Finance (해외/암호화폐)
  - 데이터 프로바이더 토글 지원 (`PROVIDER_KRX_API_ENABLED`, `PROVIDER_YAHOO_ENABLED`, `NAVER_FUNDAMENTAL_ENABLED`)
  - 네이버 금융 크롤링으로 국내 펀더멘털 데이터 수집 속도 개선
- **다중 타임프레임 분석**: 여러 시간대 데이터 동시 분석 (1분~월봉)
  - Look-Ahead Bias 방지 자동 정렬
  - 크로스 타임프레임 시그널 결합
- **실시간 시세**: WebSocket 기반 실시간 가격/호가/체결
- **과거 데이터**: TimescaleDB 시계열 저장, 백테스팅 지원
- **데이터셋 관리**: Yahoo Finance 데이터 다운로드, 캔들 데이터 CRUD
- **백그라운드 수집**: 펀더멘털 데이터 자동 수집, 심볼 자동 동기화 (KRX/Binance/Yahoo)
- **ML 패턴 인식**: 캔들스틱 26개 + 차트 패턴 24개 (ONNX 추론)
- **ML 모델 훈련**: XGBoost, LightGBM, RandomForest, 앙상블 지원
- **성과 지표**: Sharpe Ratio, MDD, Win Rate, CAGR 등

### 🎯 고급 스코어링 시스템
- **Global Score Ranking**: 7개 팩터 기반 종합 종목 평가
  - VolumeQuality, Momentum, ValueFactor, RouteState 등
  - 페널티 시스템 (LiquidityGate, MarketRegime 필터)
- **7Factor Scoring**: 정규화된 다요인 분석 (0-100 점수)
  - Momentum, Value, Quality, Volatility, Liquidity, Growth, Sentiment
- **RouteState Calculator**: 진입 타이밍 자동 판단
  - ATTACK (진입 적기), ARMED (대기), WAIT (관찰), OVERHEAT (과열)
  - TTM Squeeze, 모멘텀, RSI, Range 종합 분석
- **Market Regime**: 5단계 추세 분류 (STRONG_UPTREND → DOWNTREND)
- **Reality Check**: 추천 종목 실제 성과 자동 검증
  - 전일 추천 → 익일 성과 자동 계산
  - 일별/소스별/랭크별 승률 통계

### 🤖 알림 & 모니터링
- **Telegram Bot**: 실시간 알림 및 모니터링
  - 포지션 현황 및 손익 업데이트
  - 거래 체결 알림
  - 전략 신호 알림
- **Signal System**: 백테스트/실거래 신호 저장
  - 신호 마커 (차트 표시용)
  - 알림 규칙 관리 (JSONB 필터)

### 🛡️ 리스크 관리
- 자동 스톱로스 / 테이크프로핏
- 포지션 크기 및 일일 손실 한도
- ATR 기반 변동성 필터
- Circuit Breaker 패턴 (에러 카테고리별 차등 임계치)
- API 재시도 시스템 (지수 백오프, Rate Limit 대응)

### 🖥️ 웹 대시보드
- 실시간 포트폴리오 모니터링
- 전략 등록/시작/중지/설정 (SDUI 동적 폼)
- 데이터셋 관리 (심볼 데이터 다운로드/조회/삭제)
- 백테스트 실행 및 결과 저장/비교
- ML 모델 훈련 및 관리
- 동기화된 멀티 차트 패널
- 거래소 API 키 관리 (AES-256-GCM 암호화)

### 📒 매매일지 (Trading Journal)
거래 내역을 체계적으로 관리하고 투자 성과를 분석합니다:
- **체결 내역 동기화**: 거래소 API에서 자동 수집
- **종목별 보유 현황**: 보유 수량, 평균 매입가, 투자 금액
- **물타기 자동 계산**: 추가 매수 시 가중평균 매입가 자동 갱신
- **FIFO 실현손익**: 선입선출 방식 실현손익 계산 (로트별 추적)
- **손익 분석**: 실현/미실현 손익, 기간별 수익률
- **매매 패턴 분석**: 빈도, 성공률, 평균 보유 기간
- **포트폴리오 비중**: 종목별 비중 시각화 및 리밸런싱 추천

## 지원 전략

### 통합 전략 (v0.7.0)

#### 📈 실시간/단기 전략
| 전략 | 설명 |
|------|------|
| **Day Trading** | 그리드 트레이딩 + 거래량 급증 종목 단타 (Grid, Market Interest Day 통합) |
| **Mean Reversion** | RSI/볼린저 밴드 기반 평균회귀 전략 (RSI, Bollinger 통합) |
| **Infinity Bot** | 무한매수봇 (50라운드, 트레일링 스탑) |
| **Candle Pattern** | 35개 캔들스틱 패턴 인식 |

#### 📊 모멘텀/로테이션 전략
| 전략 | 설명 |
|------|------|
| **Rotation** | 듀얼/섹터 모멘텀 + 시가총액 상위 로테이션 (Dual Momentum, Sector Momentum, Stock Rotation, Market Cap Top 통합) |
| **Compound Momentum** | 모멘텀 기반 공격/안전 자산 전환 (구 Simple Power) |
| **Momentum Power** | 모멘텀 기반 ETF 조합 + MA 필터 (구 Snow) |

#### 🏦 자산배분 전략
| 전략 | 설명 |
|------|------|
| **Asset Allocation** | 전천후 포트폴리오 (All Weather, HAA, XAA, BAA 통합) |
| **Pension Portfolio** | 연금 계좌 자동 운용 (MDD 최소화) |

#### 🎯 섹터/레버리지/국내 전략
| 전략 | 설명 |
|------|------|
| **Sector VB** | 섹터별 변동성 돌파 |
| **US 3X Leverage** | 미국 3배 레버리지 ETF (TQQQ/SOXL) |
| **Kosdaq Fire Rain** | 코스닥 급등 모멘텀 단타 |
| **Kospi Both Side** | KOSPI 롱숏 양방향 매매 |
| **Small Cap Factor** | 소형주 팩터 전략 |
| **Range Trading** | 박스권 구간별 분할 매매 (구 Stock Gugan) |
| **RSI Multi Timeframe** | 다중 타임프레임 RSI 전략 |

## 전략 개발 가이드

ZeroQuant는 플러그인 기반 전략 시스템을 지원합니다. `Strategy` trait를 구현하여 새로운 전략을 추가할 수 있습니다.

### Strategy Trait

```rust
#[async_trait]
pub trait Strategy: Send + Sync {
    /// 전략 이름
    fn name(&self) -> &str;

    /// 전략 버전
    fn version(&self) -> &str;

    /// 전략 설명
    fn description(&self) -> &str;

    /// 설정으로 전략 초기화
    async fn initialize(&mut self, config: Value) -> Result<()>;

    /// 새 시장 데이터 수신 시 호출 → 트레이딩 신호 반환
    async fn on_market_data(&mut self, data: &MarketData) -> Result<Vec<Signal>>;

    /// 주문 체결 시 호출
    async fn on_order_filled(&mut self, order: &Order) -> Result<()>;

    /// 포지션 업데이트 시 호출
    async fn on_position_update(&mut self, position: &Position) -> Result<()>;

    /// 전략 종료 및 리소스 정리
    async fn shutdown(&mut self) -> Result<()>;

    /// 현재 전략 상태 (모니터링용)
    fn get_state(&self) -> Value;
}
```

### 새 전략 추가 방법

1. **전략 파일 생성**: `crates/trader-strategy/src/strategies/my_strategy.rs`

```rust
use crate::Strategy;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MyStrategyConfig {
    pub symbol: String,
    pub parameter1: f64,
    // ... 전략 파라미터
}

pub struct MyStrategy {
    config: MyStrategyConfig,
    // ... 내부 상태
}

#[async_trait]
impl Strategy for MyStrategy {
    fn name(&self) -> &str { "my_strategy" }
    fn version(&self) -> &str { "1.0.0" }
    fn description(&self) -> &str { "나만의 전략 설명" }

    async fn on_market_data(&mut self, data: &MarketData) -> Result<Vec<Signal>> {
        // 시장 데이터 분석 후 매수/매도 신호 생성
        let signals = vec![];

        if /* 매수 조건 */ {
            signals.push(Signal::new(SignalType::Buy, ...));
        }

        Ok(signals)
    }
    // ... 나머지 구현
}
```

2. **모듈 등록**: `crates/trader-strategy/src/strategies/mod.rs`

```rust
pub mod my_strategy;
pub use my_strategy::*;
```

3. **엔진에 등록**: `crates/trader-strategy/src/engine.rs`의 전략 팩토리에 추가

### 전략 구조

```
crates/trader-strategy/
├── src/
│   ├── lib.rs              # 모듈 진입점
│   ├── traits.rs           # Strategy trait 정의
│   ├── engine.rs           # 전략 엔진 (로딩/실행)
│   ├── plugin/             # 동적 플러그인 로더
│   └── strategies/
│       ├── mod.rs          # 전략 모듈 목록
│       ├── common/         # 공통 유틸리티 (모멘텀, 리밸런스)
│       ├── grid.rs         # 그리드 트레이딩
│       ├── rsi.rs          # RSI 평균회귀
│       └── ...             # 기타 전략들
```

## 아키텍처

```
zeroquant/
├── crates/
│   ├── trader-core/         # 도메인 모델, 공통 유틸리티
│   ├── trader-exchange/     # 거래소 연동 (Binance, KIS)
│   ├── trader-strategy/     # 전략 엔진, 16개 통합 전략
│   ├── trader-risk/         # 리스크 관리
│   ├── trader-execution/    # 주문 실행 엔진
│   ├── trader-data/         # 데이터 수집/저장 (OHLCV)
│   ├── trader-analytics/    # ML 추론, 성과 분석, 패턴 인식
│   ├── trader-api/          # REST/WebSocket API
│   │   ├── repository/      # 데이터 접근 계층 (12개 Repository)
│   │   └── routes/          # 모듈화된 라우트 (analytics/, credentials/, backtest/, journal, screening)
│   ├── trader-cli/          # CLI 도구
│   └── trader-notification/ # 알림 (Telegram)
├── frontend/                # SolidJS + TypeScript + Vite
│   ├── src/pages/
│   │   ├── Dashboard.tsx    # 포트폴리오 모니터링
│   │   ├── Strategies.tsx   # 전략 등록/관리 (SDUI)
│   │   ├── Dataset.tsx      # 데이터셋 관리
│   │   ├── Backtest.tsx     # 백테스트 실행
│   │   ├── Simulation.tsx   # 시뮬레이션
│   │   ├── MLTraining.tsx   # ML 모델 훈련
│   │   ├── TradingJournal.tsx # 매매일지
│   │   └── Settings.tsx     # 설정 (API 키, 알림)
│   └── src/components/      # 재사용 컴포넌트 (15개+)
├── migrations/              # DB 마이그레이션 (7개 통합)
├── scripts/                 # ML 훈련 파이프라인
└── docs/                    # 프로젝트 문서
```

## 기술 스택

| 영역 | 기술 |
|------|------|
| Backend | Rust, Tokio, Axum |
| Database | PostgreSQL (TimescaleDB), Redis |
| Frontend | SolidJS, TypeScript, Vite |
| ML | ONNX Runtime, XGBoost, LightGBM, RandomForest |
| Testing | Playwright (E2E), pytest (ML) |
| Infrastructure | Podman/Docker, TimescaleDB, Redis |

## 빠른 시작

### 요구사항
- Rust 1.83+ (ONNX Runtime 호환)
- Node.js 18+
- **Podman** (권장) 또는 Docker & Docker Compose
- PostgreSQL 15+ (TimescaleDB) / Redis 7+

### 설치

```bash
# 저장소 클론
git clone https://github.com/berrzebb/zeroquant.git
cd zeroquant

# 환경 설정
cp .env.example .env
```

### Podman 설정 (Windows)

```bash
# Podman 설치
winget install RedHat.Podman

# Podman Machine 초기화 및 시작
podman machine init --cpus=2 --memory=2048 --disk-size=20
podman machine start
```

### 실행 (인프라 + 로컬 개발)

```bash
# 1. 인프라 시작 (DB, Redis) - Podman 또는 Docker 모두 지원
podman compose up -d    # Podman 사용 시
# docker-compose up -d  # Docker 사용 시

# 2. 백엔드 실행 (로컬)
export DATABASE_URL=postgresql://trader:trader_secret@localhost:5432/trader
export REDIS_URL=redis://localhost:6379
cargo run --bin trader-api --features ml --release  # ML 기능 포함

# 3. 프론트엔드 실행 (로컬)
cd frontend && npm install && npm run dev
```

### 명령어 매핑 (Docker ↔ Podman)

| Docker | Podman |
|--------|--------|
| `docker-compose up -d` | `podman compose up -d` |
| `docker-compose down` | `podman compose down` |
| `docker-compose logs -f` | `podman compose logs -f` |
| `docker exec -it` | `podman exec -it` |
| `docker ps` | `podman ps` |

### ML 모델 훈련

```bash
# Podman으로 ML 훈련 실행
podman compose --profile ml run --rm trader-ml \
  python scripts/train_ml_model.py --symbol SPY --model xgboost

# 사용 가능한 심볼 목록
podman compose --profile ml run --rm trader-ml \
  python scripts/train_ml_model.py --list-symbols
```

### E2E 테스트

```bash
# Playwright 설치
cd frontend && npx playwright install

# E2E 테스트 실행
npm run test:e2e

# 특정 테스트 실행
npx playwright test risk-management-ui.spec.ts
```

## 설정

### 환경 변수
```env
DATABASE_URL=postgresql://trader:password@localhost:5432/trader
REDIS_URL=redis://localhost:6379
JWT_SECRET=your-secret-key
ENCRYPTION_KEY=your-32-byte-key-base64
```

### 거래소 API 키
웹 대시보드 **설정 > API 키 관리**에서 등록합니다. 모든 키는 AES-256-GCM으로 암호화 저장됩니다.

## 문서

| 문서 | 설명 |
|------|------|
| [API 문서](docs/api.md) | REST/WebSocket API 레퍼런스 |
| [아키텍처](docs/architecture.md) | 시스템 아키텍처 상세 |
| [배포 가이드](docs/deployment.md) | 프로덕션 배포 방법 |
| [운영 가이드](docs/operations.md) | 일상 운영 및 관리 |
| [트러블슈팅](docs/troubleshooting.md) | 문제 해결 가이드 |
| [개발 규칙](docs/development_rules.md) | 코드 작성 규칙 및 가이드라인 |
| [전략 비교](docs/STRATEGY_COMPARISON.md) | 전략별 상세 파라미터 |
| [개선 로드맵](docs/improvement_todo.md) | 코드베이스 개선 계획 |
| [Claude 가이드](CLAUDE.md) | AI 세션 컨텍스트 |

## 라이선스

MIT License
