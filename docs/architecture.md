# ZeroQuant Trading Bot - 기술 아키텍처

> 작성일: 2026-01-30
> 버전: 2.0

---

## 시스템 구성도

```
┌─────────────────────────────────────────────────────────────┐
│                  Web Dashboard (Frontend)                    │
│                 SolidJS + TailwindCSS                        │
└─────────────────────┬───────────────────────────────────────┘
                      │ WebSocket + REST API
┌─────────────────────▼───────────────────────────────────────┐
│                   API Gateway (Axum)                         │
│          Authentication & Authorization Layer                │
└─────────┬────────────────────────────────────┬──────────────┘
          │                                    │
┌─────────▼────────────┐          ┌───────────▼──────────────┐
│  Strategy Engine     │          │    Risk Manager          │
│  (Plugin System)     │◄─────────┤  (Real-time Monitor)     │
└─────────┬────────────┘          └───────────┬──────────────┘
          │                                    │
┌─────────▼─────────▼────────────────────────────────────────┐
│                 Order Executor                              │
│       (Position Management, Order Routing)                  │
└─────────┬───────────────────────────────────┬───────────────┘
          │                                   │
┌─────────▼──────────┐          ┌────────────▼──────────────┐
│ Exchange Connector │          │     Data Manager          │
│  (Multi-Exchange)  │          │ (Real-time + Historical)  │
└─────────┬──────────┘          └────────────┬──────────────┘
          │                                   │
          └───────────────┬───────────────────┘
                          │
          ┌───────────────▼───────────────────────────┐
          │      Database Layer                       │
          │ PostgreSQL (Timescale) + Redis            │
          └───────────────────────────────────────────┘
```

---

## 기술 스택

### 백엔드
| 기술 | 버전 | 용도 |
|------|------|------|
| Rust | stable (1.93+) | 시스템 프로그래밍 언어 |
| Tokio | 최신 | 비동기 런타임 |
| Axum | 0.7+ | 웹 프레임워크 |
| SQLx | 0.8+ | 데이터베이스 드라이버 (async, compile-time checked) |
| TimescaleDB | 2.x | 시계열 데이터베이스 (PostgreSQL 15 확장) |
| Redis | 7.x | 캐시, 세션, 실시간 데이터 |

### 프론트엔드
| 기술 | 버전 | 용도 |
|------|------|------|
| SolidJS | 1.8+ | 반응형 UI 프레임워크 |
| TailwindCSS | 3.x | 유틸리티 CSS |
| Lightweight Charts | 4.x | 금융 차트 라이브러리 |
| TanStack Query | 5.x | 서버 상태 관리 |
| Vite | 5.x | 빌드 도구 |

### 데이터 및 분석
| 기술 | 용도 |
|------|------|
| Polars | 고성능 데이터프레임 처리 |
| ta-rs | 기술적 지표 라이브러리 |
| ONNX Runtime | ML 모델 추론 (GPU 가속) |

### 인프라
| 기술 | 용도 |
|------|------|
| Docker | 컨테이너화 |
| Docker Compose | 멀티 컨테이너 오케스트레이션 |
| Prometheus | 메트릭 수집 |
| Grafana | 모니터링 대시보드 |
| tracing | 구조화된 로깅 |

---

## 프로젝트 구조

```
d:\Trader\
├── Cargo.toml                 # Workspace 루트
├── .env.example               # 환경 변수 템플릿
├── docker-compose.yml         # Docker 서비스 정의
│
├── crates/                    # Rust 크레이트 (백엔드)
│   ├── trader-core/           # 도메인 모델 (2,917줄)
│   │   ├── order.rs           # 주문 타입 정의
│   │   ├── position.rs        # 포지션 타입 정의
│   │   ├── trade.rs           # 거래 기록
│   │   ├── signal.rs          # 전략 신호
│   │   └── symbol.rs          # 심볼 정의
│   │
│   ├── trader-api/            # REST API 서버 (19,588줄)
│   │   ├── routes/            # 17개 API 라우트
│   │   │   ├── backtest.rs    # 백테스트 실행
│   │   │   ├── strategies.rs  # 전략 CRUD
│   │   │   ├── portfolio.rs   # 포트폴리오 조회
│   │   │   └── ...
│   │   ├── state.rs           # 앱 상태 관리
│   │   ├── websocket.rs       # 실시간 통신
│   │   └── main.rs            # 서버 엔트리포인트
│   │
│   ├── trader-strategy/       # 전략 엔진 (15,842줄)
│   │   ├── strategies/        # 18개 전략 구현
│   │   │   ├── rsi.rs         # RSI 평균회귀
│   │   │   ├── grid.rs        # 그리드 트레이딩
│   │   │   ├── haa.rs         # HAA 자산배분
│   │   │   └── ...
│   │   ├── engine.rs          # 전략 실행 엔진
│   │   └── registry.rs        # 전략 레지스트리
│   │
│   ├── trader-risk/           # 리스크 관리 (3,742줄)
│   │   ├── manager.rs         # 중앙 RiskManager
│   │   ├── position_sizing.rs # 포지션 사이징
│   │   ├── stop_loss.rs       # 스톱로스/테이크프로핏
│   │   ├── limits.rs          # 일일 손실 한도
│   │   ├── trailing_stop.rs   # 트레일링 스탑 (4가지 모드)
│   │   └── config.rs          # 리스크 설정
│   │
│   ├── trader-execution/      # 주문 실행 (2,889줄)
│   │   ├── executor.rs        # 주문 실행기
│   │   ├── order_manager.rs   # 주문 관리
│   │   └── position_tracker.rs# 포지션 추적
│   │
│   ├── trader-exchange/       # 거래소 연동 (11,025줄)
│   │   ├── binance/           # Binance 커넥터
│   │   ├── kis_kr/            # 한국투자증권 (국내)
│   │   ├── kis_us/            # 한국투자증권 (해외)
│   │   ├── yahoo/             # Yahoo Finance 데이터
│   │   └── simulation/        # 시뮬레이션 모드
│   │
│   ├── trader-data/           # 데이터 관리 (4,070줄)
│   │   ├── storage/           # TimescaleDB 저장소
│   │   ├── cache/             # Redis 캐시
│   │   └── krx.rs             # KRX API 연동
│   │
│   ├── trader-analytics/      # 분석 엔진 (11,039줄)
│   │   ├── backtest/          # 백테스트 엔진
│   │   ├── metrics.rs         # 성과 지표 14개
│   │   ├── indicators.rs      # 기술 지표 11개
│   │   └── ml/                # ML 패턴 인식 (4,125줄)
│   │       ├── pattern.rs     # 캔들/차트 패턴 48종
│   │       ├── predictor.rs   # ONNX 추론
│   │       └── features.rs    # Feature Engineering
│   │
│   ├── trader-cli/            # CLI 도구 (1,981줄)
│   │   ├── download.rs        # 데이터 다운로드
│   │   ├── backtest.rs        # CLI 백테스트
│   │   └── import.rs          # 데이터 임포트
│   │
│   └── trader-notification/   # 알림 서비스 (690줄)
│       ├── telegram.rs        # 텔레그램 봇
│       └── discord.rs         # Discord 웹훅
│
├── migrations/                # DB 마이그레이션 (10개)
│   ├── 001_initial_schema.sql
│   ├── 002_encrypted_credentials.sql
│   └── ...
│
├── frontend/                  # 웹 대시보드
│   ├── src/
│   │   ├── pages/             # 7개 페이지 (7,044줄)
│   │   │   ├── Dashboard.tsx
│   │   │   ├── Backtest.tsx
│   │   │   ├── Strategies.tsx
│   │   │   └── ...
│   │   ├── components/        # UI 컴포넌트 (4,000+줄)
│   │   │   ├── DynamicForm.tsx# SDUI 폼 렌더러
│   │   │   ├── charts/        # 차트 컴포넌트 8개
│   │   │   └── ...
│   │   ├── api/               # API 클라이언트
│   │   └── hooks/             # React 훅
│   ├── package.json
│   └── vite.config.ts
│
├── config/                    # 설정 파일
├── tests/                     # 통합 테스트
└── docs/                      # 문서
    ├── architecture.md        # (이 문서)
    ├── api.md                 # API 문서
    ├── STRATEGY_COMPARISON.md # 전략 비교
    └── todo.md                # TODO 목록
```

---

## 크레이트 의존성 그래프

```
                    ┌────────────────┐
                    │  trader-api    │
                    │  (Entry Point) │
                    └───────┬────────┘
                            │
        ┌───────────────────┼───────────────────┐
        │                   │                   │
        ▼                   ▼                   ▼
┌───────────────┐  ┌────────────────┐  ┌───────────────┐
│trader-strategy│  │ trader-risk    │  │trader-exchange│
│   (Signals)   │  │ (Validation)   │  │   (Market)    │
└───────┬───────┘  └───────┬────────┘  └───────┬───────┘
        │                  │                   │
        └──────────────────┼───────────────────┘
                           │
                    ┌──────▼──────┐
                    │trader-exec  │
                    │(Order Flow) │
                    └──────┬──────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
              ▼            ▼            ▼
      ┌────────────┐ ┌──────────┐ ┌────────────┐
      │trader-data │ │trader-   │ │trader-     │
      │ (Storage)  │ │analytics │ │notification│
      └──────┬─────┘ └────┬─────┘ └────────────┘
             │            │
             ▼            ▼
      ┌────────────────────────┐
      │     trader-core        │
      │   (Domain Models)      │
      └────────────────────────┘
```

---

## 데이터 흐름

### 1. 백테스트 플로우

```
Frontend (Backtest.tsx)
    │
    │ POST /api/v1/backtest/run
    ▼
API Layer (backtest.rs)
    │
    │ 1. 파라미터 검증
    │ 2. 히스토리컬 데이터 로드
    ▼
Data Layer (trader-data)
    │
    │ TimescaleDB에서 OHLCV 조회
    ▼
Strategy Engine (trader-strategy)
    │
    │ 전략 실행, 신호 생성
    ▼
Backtest Engine (trader-analytics)
    │
    │ 1. 주문 시뮬레이션
    │ 2. 슬리피지/수수료 적용
    │ 3. 포지션 관리
    │ 4. 성과 지표 계산
    ▼
API Layer
    │
    │ BacktestResult 반환
    ▼
Frontend
    │
    │ 차트 및 통계 렌더링
    ▼
```

### 2. 실시간 트레이딩 플로우

```
Exchange WebSocket
    │
    │ 실시간 시세 수신
    ▼
Data Layer (캐시)
    │
    │ Redis에 틱 데이터 저장
    ▼
Strategy Engine
    │
    │ 전략 평가, 신호 생성
    ▼
Risk Manager                    ◄── 검증 실패 시 거부
    │
    │ 1. 포지션 크기 검증
    │ 2. 일일 손실 한도 확인
    │ 3. 변동성 필터 적용
    ▼
Order Executor
    │
    │ 1. 주문 생성
    │ 2. 스톱로스/테이크프로핏 자동 생성
    ▼
Exchange Connector
    │
    │ 거래소 API 호출
    ▼
Notification Service
    │
    │ 텔레그램/Discord 알림
    ▼
```

---

## 리스크 관리 아키텍처

### 검증 파이프라인

```
Signal (from Strategy)
    │
    ▼
┌────────────────────────────────────────────────┐
│              RiskManager.validate_order()       │
├────────────────────────────────────────────────┤
│ 1. 일일 손실 한도 확인                          │
│    - can_trade() → false면 거부                │
├────────────────────────────────────────────────┤
│ 2. 심볼 활성화 확인                             │
│    - 비활성 심볼이면 거부                       │
├────────────────────────────────────────────────┤
│ 3. 변동성 필터                                  │
│    - volatility > threshold → 거부/경고        │
├────────────────────────────────────────────────┤
│ 4. 포지션 사이징 검증                           │
│    - 단일 포지션 한도 (10%)                    │
│    - 총 노출 한도 (50%)                        │
│    - 동시 포지션 제한 (10개)                   │
│    - 최소 주문 크기                            │
├────────────────────────────────────────────────┤
│ 5. 일일 손실 경고                               │
│    - 70%+ 경고, 90%+ 위험                      │
└────────────────────────────────────────────────┘
    │
    ▼
Order Execution (if valid)
```

### 트레일링 스탑 모드

| 모드 | 동작 |
|------|------|
| FixedPercentage | 고정 비율로 가격 추적 (기본 1.5%) |
| AtrBased | ATR × 배수로 변동성 기반 추적 |
| Step-Based | 수익률 구간별 다른 추적 비율 |
| Parabolic SAR | 가속 계수 기반 포물선 추적 |

---

## 데이터베이스 스키마

### TimescaleDB Hypertables

| 테이블 | 파티션 키 | 용도 |
|--------|----------|------|
| klines | timestamp | 분봉/일봉 OHLCV |
| ohlcv | timestamp | Yahoo Finance 캐시 |
| trade_ticks | timestamp | 틱 데이터 |
| credential_access_logs | accessed_at | 접근 로그 |

### 주요 테이블

| 테이블 | 설명 |
|--------|------|
| symbols | 심볼 정의 |
| orders | 주문 기록 |
| trades | 체결 기록 |
| positions | 포지션 |
| strategies | 등록된 전략 |
| exchange_credentials | 암호화된 API 키 |
| telegram_settings | 텔레그램 설정 |
| backtest_results | 백테스트 결과 |
| portfolio_equity_history | 자산 곡선 |

---

## Docker 서비스

| 서비스 | 포트 | 프로필 | 설명 |
|--------|------|--------|------|
| timescaledb | 5432 | 기본 | TimescaleDB (PostgreSQL 15) |
| redis | 6379 | 기본 | Redis 7 |
| trader-api | 3000 | 기본 | Rust API 서버 |
| prometheus | 9090 | monitoring | 메트릭 수집 |
| grafana | 3001 | monitoring | 모니터링 대시보드 |
| redis-commander | 8081 | dev | Redis GUI |
| pgadmin | 5050 | dev | PostgreSQL GUI |
| frontend-dev | 5173 | dev | Vite 개발 서버 |

### 실행 방법

```bash
# 기본 서비스 실행
docker compose up -d

# 개발 환경 포함
docker compose --profile dev up -d

# 모니터링 포함
docker compose --profile monitoring up -d

# 전체 실행
docker compose --profile dev --profile monitoring up -d
```

---

## API 엔드포인트 요약

| 엔드포인트 | 메서드 | 설명 |
|-----------|--------|------|
| `/health` | GET | 헬스 체크 |
| `/api/v1/strategies` | GET, POST, PUT, DELETE | 전략 CRUD |
| `/api/v1/backtest/run` | POST | 백테스트 실행 |
| `/api/v1/backtest/results` | GET, POST | 결과 저장/조회 |
| `/api/v1/orders` | GET, POST | 주문 관리 |
| `/api/v1/positions` | GET | 포지션 조회 |
| `/api/v1/portfolio` | GET | 포트폴리오 조회 |
| `/api/v1/analytics/*` | GET | 성과 분석 |
| `/api/v1/credentials` | GET, POST, DELETE | API 키 관리 |
| `/api/v1/notifications/*` | GET, POST | 알림 설정 |
| `/api/v1/ml/*` | GET, POST | ML 훈련 |
| `/api/v1/dataset/*` | GET, POST | 데이터셋 관리 |
| `/api/v1/simulation/*` | POST | 시뮬레이션 제어 |
| `/ws` | WebSocket | 실시간 스트림 |

---

## 보안

### 자격증명 암호화
- **알고리즘**: AES-256-GCM
- **키 관리**: 환경 변수 또는 Docker Secret
- **저장**: `exchange_credentials` 테이블 (암호화된 상태)

### API 보안
- Rate Limiting (향후 구현)
- CORS 설정
- 입력 유효성 검증 (Validator)

---

## 성능 최적화

### TimescaleDB
- Hypertable 자동 파티셔닝
- 압축 정책 (7일 이상 데이터)
- 연속 집계 (continuous aggregates)

### Redis 캐싱
- 실시간 시세 데이터
- 세션 관리
- Rate Limit 카운터

### Rust 최적화
- async/await 비동기 처리
- Zero-copy 직렬화
- 컴파일 타임 SQL 검증 (SQLx)

---

## 참고 문서

- [API 문서](./api.md)
- [전략 비교](./STRATEGY_COMPARISON.md)
- [TODO 목록](./todo.md)
- [PRD v2.0](C:\Users\HP\.claude\plans\synthetic-conjuring-peach.md)

---

*문서 생성일: 2026-01-30*
