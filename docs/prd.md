# 트레이딩 봇 PRD 및 구현 계획

## 프로젝트 개요

**프로젝트명**: Multi-Market Trading Bot (Rust 기반)
**위치**: `d:\Trader`
**현재 상태**: Greenfield 프로젝트 (완전히 비어있음)
**주요 목표**: 24/7 자동화된 다중 시장 트레이딩 시스템 구축

## Product Requirements Document (PRD)

### 1. 제품 비전

여러 시장(암호화폐, 주식, 외환)에서 다양한 거래 전략을 동시에 실행하고, 웹 대시보드를 통해 실시간으로 모니터링 및 제어할 수 있는 자동화된 트레이딩 시스템을 구축합니다.

### 2. 핵심 요구사항

#### 2.1 기능 요구사항

**다중 시장 지원**
- 암호화폐 거래소 (Binance, Coinbase, Kraken)
- 주식 시장
  - 글로벌: Interactive Brokers
  - 한국: 키움증권(Kiwoom), 이베스트투자증권(eBEST/XingAPI), 한국투자증권(KIS Developers)
- 외환 시장 (Oanda)
- 거래소별 데이터 정규화 및 통합 인터페이스

**플러그인 기반 전략 시스템**
- 동적 전략 로딩 (cdylib)
- 여러 전략 동시 실행
- 전략 간 격리 보장
- 전략 설정 hot-reload
- 기본 제공 전략:
  - **그리드 트레이딩 (Grid Trading)** ⭐ 최우선
    - 단순 그리드 (고정 간격)
    - 동적 그리드 (ATR 기반 간격 조정)
    - 트렌드 필터 그리드 (트렌드 방향에 따라 활성화)
  - 이동평균 크로스오버 (트렌드 추종)
  - RSI 평균회귀
  - 추가 전략 개발 가능

**웹 대시보드**
- 실시간 시장 데이터 시각화
- 포지션 및 주문 관리
- 성과 차트 및 지표
- 전략 시작/중지/설정
- 수동 거래 기능
- 알림 및 경고

**자동화 및 효율성**
- 24/7 무인 운영
- 자동 주문 실행
- 거래 기회 자동 포착
- 재연결 및 에러 복구
- Graceful shutdown

#### 2.2 리스크 관리 요구사항 (모두 필수)

**스톱로스/테이크프로핏**
- 포지션 오픈 시 자동 보호 주문 생성
- 설정 가능한 기본 비율
- 전략별 커스텀 설정 가능

**포지션 크기 제한**
- 거래당 최대 크기 (계좌 대비 %)
- 총 투자금 대비 최대 노출 제한
- 심볼별 개별 한도 설정

**일일 손실 한도**
- 일일 최대 손실액 설정
- 한도 도달 시 자동 거래 중단
- 다음 거래일 자동 리셋

**변동성 필터**
- ATR 기반 변동성 측정
- 임계값 초과 시 거래 중단 또는 포지션 축소
- 시장별 별도 설정

#### 2.3 데이터 및 분석 요구사항 (모두 필수)

**실시간 시장 데이터**
- WebSocket을 통한 실시간 가격 피드
- 호가창 (Order Book) 데이터
- 체결 내역 (Trades)
- OHLCV 캔들스틱 (1m, 5m, 15m, 1h, 4h, 1d)

**과거 데이터 저장**
- TimescaleDB에 시계열 데이터 저장
- 백테스팅을 위한 과거 데이터 import
- 데이터 압축 및 파티셔닝
- 데이터 갭 탐지 및 복구

**성능 지표 추적**
- 샤프 비율 (Sharpe Ratio)
- 최대 낙폭 (Maximum Drawdown)
- 승률 (Win Rate)
- Profit Factor
- 평균 수익/손실
- 실시간 PnL 추적

**ML/AI 기능**
- ONNX Runtime을 통한 ML 모델 추론
- 가격 예측 모델
- 패턴 인식 (캔들스틱 패턴, 차트 패턴)
- 피처 엔지니어링 파이프라인

#### 2.4 비기능 요구사항

**성능**
- 주문 검증 지연 < 10ms
- 데이터 저장 지연 < 100ms
- WebSocket 재연결 < 5초
- API 응답 시간 < 200ms (P95)

**확장성**
- 10개 이상의 전략 동시 실행
- 100개 이상의 심볼 동시 모니터링
- 수평 확장 가능한 아키텍처

**신뢰성**
- 99.9% 가용성 목표
- 자동 에러 복구
- Circuit breaker 패턴
- 포괄적인 에러 처리

**보안**
- API 키 암호화 저장
- JWT 기반 인증
- 역할 기반 접근 제어 (RBAC)
- 감사 로그
- Rate limiting

### 3. 사용자 스토리

**전략 개발자 (Strategy Developer)**
- 새로운 거래 전략을 Rust로 작성하여 플러그인으로 추가할 수 있다
- 과거 데이터로 전략을 백테스트하고 성과를 분석할 수 있다
- 기술적 지표 라이브러리를 사용하여 복잡한 로직을 구현할 수 있다

**트레이더 (Trader)**
- 웹 대시보드에서 실시간 시장 상황을 모니터링할 수 있다
- 전략을 시작/중지하고 설정을 조정할 수 있다
- 포지션과 주문을 확인하고 필요시 수동으로 개입할 수 있다
- 성과 지표와 차트를 통해 전략의 효과를 평가할 수 있다

**시스템 관리자 (Admin)**
- 시스템 상태와 리소스 사용량을 모니터링할 수 있다
- 로그를 확인하고 문제를 진단할 수 있다
- 리스크 한도를 설정하고 거래를 중단시킬 수 있다
- 사용자 계정과 권한을 관리할 수 있다

### 4. 제외 사항 (Out of Scope)

- 모바일 앱 (웹 대시보드만 제공)
- 소셜 트레이딩 (다른 트레이더 팔로우)
- 페이퍼 트레이딩 전용 모드 (Phase 1에서는 제외, 추후 추가 가능)
- 자동 전략 최적화 (수동 백테스트만 지원)

---

## 기술 아키텍처

### 시스템 구성도

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

### 기술 스택

**백엔드**
- **언어**: Rust (stable)
- **비동기 런타임**: Tokio
- **웹 프레임워크**: Axum
- **데이터베이스**: TimescaleDB (PostgreSQL 확장)
- **캐시**: Redis
- **ORM**: SQLx

**프론트엔드**
- **프레임워크**: SolidJS
- **스타일링**: TailwindCSS
- **차트**: LightweightCharts
- **빌드 도구**: Vite

**데이터 및 분석**
- **데이터 처리**: Polars
- **기술적 지표**: ta-rs
- **ML 추론**: ONNX Runtime

**인프라**
- **컨테이너**: Docker + Docker Compose
- **모니터링**: Prometheus + Grafana
- **로깅**: tracing + tracing-subscriber

### 프로젝트 구조

```
d:\Trader\
├── Cargo.toml                 # Workspace 루트
├── .env.example
├── docker-compose.yml
│
├── crates/
│   ├── trader-core/           # 도메인 모델
│   ├── trader-exchange/       # 거래소 연동
│   │   ├── binance/          # Binance 커넥터
│   │   ├── coinbase/         # Coinbase 커넥터
│   │   ├── kraken/           # Kraken 커넥터
│   │   ├── interactive_brokers/ # IB 커넥터
│   │   ├── kiwoom/           # 키움증권 커넥터 (프록시 서비스)
│   │   ├── ebest/            # 이베스트 커넥터
│   │   ├── korea_investment/ # 한국투자증권 커넥터
│   │   └── oanda/            # Oanda 커넥터
│   ├── trader-strategy/       # 전략 엔진
│   ├── trader-risk/          # 리스크 관리
│   ├── trader-execution/     # 주문 실행
│   ├── trader-data/          # 데이터 관리
│   ├── trader-analytics/     # 분석 엔진
│   ├── trader-api/           # REST API 서버
│   └── trader-cli/           # CLI 도구
│
├── migrations/                # DB 마이그레이션
├── config/                    # 설정 파일
├── frontend/                  # 웹 대시보드
├── tests/                     # 통합 테스트
└── docs/                      # 문서
```

---

## 구현 계획

### Phase 1: 기반 구조 설정 (우선순위: 최고)

**목표**: 프로젝트 초기화 및 핵심 인프라 구축

**작업 항목**:
1. Cargo workspace 생성 및 크레이트 구조 설정
2. Docker Compose로 로컬 개발 환경 구축
   - PostgreSQL (TimescaleDB)
   - Redis
3. 핵심 도메인 모델 정의 (`trader-core`)
   - Order, Position, Trade, Symbol 등
   - 공통 타입 및 에러 처리
4. 데이터베이스 스키마 및 마이그레이션
5. 설정 관리 시스템 (config + .env)
6. 로깅 인프라 (tracing)

**완료 기준**:
- [ ] `cargo build` 성공
- [ ] Docker Compose로 DB 실행 가능
- [ ] 기본 도메인 타입 컴파일
- [ ] 마이그레이션 실행 성공
- [ ] 구조적 로깅 출력 확인

**핵심 파일**:
- [Cargo.toml](d:\Trader\Cargo.toml) - Workspace 정의
- [crates/trader-core/src/lib.rs](d:\Trader\crates\trader-core\src\lib.rs) - 도메인 모델
- [crates/trader-core/src/domain/order.rs](d:\Trader\crates\trader-core\src\domain\order.rs)
- [crates/trader-core/src/domain/position.rs](d:\Trader\crates\trader-core\src\domain\position.rs)
- [migrations/001_initial_schema.sql](d:\Trader\migrations\001_initial_schema.sql)
- [docker-compose.yml](d:\Trader\docker-compose.yml)

### Phase 2: 거래소 연동 (우선순위: 최고)

**목표**: 실시간 시장 데이터 수신 및 주문 실행 기능

**우선 타겟: Binance (24/7 그리드 트레이딩용)**

그리드 트레이딩 전략은 24시간 거래가 가능한 암호화폐 시장에서 최적의 성과를 발휘합니다. Binance는 세계 1위 거래소로 안정적인 API, 낮은 수수료(0.1%), 높은 유동성을 제공하여 MVP 단계의 최우선 목표로 설정합니다.

**작업 항목**:
1. Exchange trait 정의
2. **Binance 통합 구현** ⭐ 최우선
   - REST API 클라이언트 (reqwest)
     - Spot 거래 (현물)
     - 계좌 정보, 잔고 조회
     - 주문 생성/취소/조회
   - WebSocket 스트림 (tokio-tungstenite)
     - 실시간 가격 데이터 (ticker)
     - 캔들스틱 스트림 (1m, 5m, 15m)
     - 개인 계정 스트림 (주문 체결 알림)
   - 인증 및 서명 (HMAC-SHA256)
   - Rate limiting 처리
3. 데이터 정규화 레이어
4. 통합 테스트 (mockito 사용)

**완료 기준**:
- [ ] Binance Spot API 연결 성공
- [ ] BTC/USDT, ETH/USDT 실시간 데이터 수신
- [ ] 주문 생성/취소/조회 작동 (Testnet)
- [ ] WebSocket 재연결 로직 작동 (3회 재시도)
- [ ] 통합 테스트 통과
- [ ] Rate limit 준수 (1200 req/min)

**핵심 파일**:
- [crates/trader-exchange/src/traits.rs](d:\Trader\crates\trader-exchange\src\traits.rs)
- [crates/trader-exchange/src/connector/binance.rs](d:\Trader\crates\trader-exchange\src\connector\binance.rs)
- [crates/trader-exchange/src/websocket/stream.rs](d:\Trader\crates\trader-exchange\src\websocket\stream.rs)

### Phase 3: 데이터 관리 (우선순위: 높음)

**목표**: 실시간 및 과거 데이터 저장/조회

**작업 항목**:
1. DataManager 구현
2. 실시간 데이터 수집 및 저장
3. TimescaleDB 최적화 (hypertable, 압축)
4. Redis 캐싱 레이어
5. 과거 데이터 import CLI

**완료 기준**:
- [ ] OHLCV 데이터 자동 저장
- [ ] 과거 데이터 조회 API
- [ ] 캐시 히트율 >80%
- [ ] 데이터 저장 지연 <100ms

**핵심 파일**:
- [crates/trader-data/src/manager.rs](d:\Trader\crates\trader-data\src\manager.rs)
- [crates/trader-data/src/storage/timescale.rs](d:\Trader\crates\trader-data\src\storage\timescale.rs)
- [crates/trader-data/src/storage/redis.rs](d:\Trader\crates\trader-data\src\storage\redis.rs)

### Phase 4: 전략 엔진 (우선순위: 최고)

**목표**: 플러그인 기반 전략 실행 시스템

**작업 항목**:
1. Strategy trait 정의
2. 전략 실행 엔진 구현
3. 플러그인 로더 시스템 (libloading)
4. 기술적 지표 라이브러리 (ta-rs 통합)
5. 샘플 전략 2개 구현
   - 이동평균 크로스오버
   - RSI 평균회귀
6. 백테스팅 프레임워크

**완료 기준**:
- [ ] 전략 동적 로딩 작동
- [ ] 여러 전략 동시 실행
- [ ] 샘플 전략 백테스트 성공
- [ ] 전략 설정 hot-reload

**핵심 파일**:
- [crates/trader-strategy/src/engine.rs](d:\Trader\crates\trader-strategy\src\engine.rs)
- [crates/trader-strategy/src/plugin/loader.rs](d:\Trader\crates\trader-strategy\src\plugin\loader.rs)
- [crates/trader-strategy/src/strategies/trend_following.rs](d:\Trader\crates\trader-strategy\src\strategies\trend_following.rs)

### Phase 5: 리스크 관리 (우선순위: 최고)

**목표**: 포괄적인 리스크 관리 시스템

**작업 항목**:
1. RiskManager 구현
2. 주문 검증 로직
   - 포지션 크기 제한
   - 일일 손실 한도
   - 변동성 필터
3. 스톱로스/테이크프로핏 자동 주문
4. 위험 경고 시스템

**완료 기준**:
- [ ] 모든 주문 리스크 검증 통과
- [ ] 일일 한도 도달 시 거래 중단
- [ ] 보호 주문 자동 생성
- [ ] 변동성 필터 작동

**핵심 파일**:
- [crates/trader-risk/src/manager.rs](d:\Trader\crates\trader-risk\src\manager.rs)
- [crates/trader-risk/src/position_sizing.rs](d:\Trader\crates\trader-risk\src\position_sizing.rs)
- [crates/trader-risk/src/stop_loss.rs](d:\Trader\crates\trader-risk\src\stop_loss.rs)

### Phase 6: 주문 실행 (우선순위: 최고)

**목표**: 신뢰성 있는 주문 실행 및 포지션 관리

**작업 항목**:
1. OrderExecutor 구현
2. 신호 → 주문 변환 로직
3. 주문 상태 추적
4. 포지션 트래커
5. 에러 복구 및 재시도

**완료 기준**:
- [ ] 전략 신호 자동 실행
- [ ] 주문 체결 100% 추적
- [ ] 포지션 실시간 업데이트
- [ ] 네트워크 에러 복구

**핵심 파일**:
- [crates/trader-execution/src/executor.rs](d:\Trader\crates\trader-execution\src\executor.rs)
- [crates/trader-execution/src/order_manager.rs](d:\Trader\crates\trader-execution\src\order_manager.rs)
- [crates/trader-execution/src/position_tracker.rs](d:\Trader\crates\trader-execution\src\position_tracker.rs)

### Phase 7: 성과 분석 (우선순위: 중간)

**목표**: 거래 성과 측정 및 분석

**작업 항목**:
1. PerformanceTracker 구현
2. 주요 지표 계산
   - 샤프 비율
   - 최대 낙폭
   - 승률, Profit Factor
3. 실시간 PnL 추적
4. 백테스팅 리포트 생성

**완료 기준**:
- [ ] 모든 지표 실시간 계산
- [ ] 백테스트 리포트 생성
- [ ] 성과 스냅샷 자동 저장

**핵심 파일**:
- [crates/trader-analytics/src/performance/metrics.rs](d:\Trader\crates\trader-analytics\src\performance\metrics.rs)
- [crates/trader-analytics/src/backtest/engine.rs](d:\Trader\crates\trader-analytics\src\backtest\engine.rs)

### Phase 8: Web API & 대시보드 (우선순위: 중간)

**목표**: 실시간 모니터링 및 제어 UI

**작업 항목**:
1. REST API 구현 (Axum)
2. WebSocket 서버
3. JWT 인증/인가
4. 프론트엔드 대시보드 (SolidJS)
   - 실시간 시장 데이터
   - 포지션/주문 관리
   - 성과 차트
   - 전략 제어
5. API 문서화 (OpenAPI)

**완료 기준**:
- [ ] 모든 API 엔드포인트 작동
- [ ] WebSocket 실시간 업데이트
- [ ] JWT 인증 구현
- [ ] 대시보드 반응형 UI

**핵심 파일**:
- [crates/trader-api/src/main.rs](d:\Trader\crates\trader-api\src\main.rs)
- [crates/trader-api/src/routes/strategies.rs](d:\Trader\crates\trader-api\src\routes\strategies.rs)
- [crates/trader-api/src/websocket/handler.rs](d:\Trader\crates\trader-api\src\websocket\handler.rs)
- [frontend/src/App.tsx](d:\Trader\frontend\src\App.tsx)

### Phase 9: ML/AI 기능 (우선순위: 낮음)

**목표**: 가격 예측 및 패턴 인식

**작업 항목**:
1. ONNX Runtime 통합
2. 피처 엔지니어링 파이프라인
3. 가격 예측 모델 학습 (별도 Python 스크립트)
4. 모델 추론 서비스
5. 패턴 인식 알고리즘

**완료 기준**:
- [ ] ONNX 모델 추론 작동
- [ ] 예측 신호 전략 통합
- [ ] 피처 계산 지연 <50ms

**핵심 파일**:
- [crates/trader-analytics/src/ml/predictor.rs](d:\Trader\crates\trader-analytics\src\ml\predictor.rs)
- [crates/trader-analytics/src/ml/pattern.rs](d:\Trader\crates\trader-analytics\src\ml\pattern.rs)

### Phase 10: 추가 거래소 통합 (우선순위: 낮음)

**목표**: 다중 시장 지원 확대

**작업 항목**:
1. **암호화폐 거래소**
   - Coinbase 통합
   - Kraken 통합

2. **글로벌 주식 시장**
   - Interactive Brokers 통합

3. **한국 주식 시장** (우선순위: 한국투자증권 > 이베스트 > 키움)
   - **한국투자증권 (KIS Developers API) 통합** ⭐ 최우선
     - REST API 기반 (가장 현대적)
     - OAuth 2.0 인증
     - 실시간 시세: WebSocket
     - 주식 현재가, 호가, 체결, 주문/잔고 조회, 매매
     - API 문서: https://apiportal.koreainvestment.com/
   - 이베스트투자증권 (eBEST XingAPI) 통합
     - REST API 및 WebSocket 지원
   - 키움증권 (Kiwoom OpenAPI) 통합
     - Windows 전용 ActiveX/COM 인터페이스
     - Rust FFI 또는 별도 서비스 프로세스로 통합

4. **외환 시장**
   - Oanda 통합

**완료 기준**:
- [ ] 각 거래소 기본 기능 작동
- [ ] 통합 테스트 통과
- [ ] 한국 브로커 인증 및 주문 실행 작동
- [ ] 거래소별 특수 요구사항 문서화

**한국 브로커 특이사항**:
- **키움증권**: Windows COM/ActiveX 기반이므로 별도 프록시 서비스 필요 (Python/C++ 브리지)
- **이베스트투자증권**: REST API 지원으로 상대적으로 통합 용이
- **한국투자증권**: 최신 REST API, OAuth 인증으로 가장 현대적인 인터페이스
- 한국 시장 거래 시간: 09:00-15:30 (KST)
- 호가 단위 및 가격 제한폭 규칙 준수 필요

### Phase 11: 프로덕션 준비 (우선순위: 중간)

**목표**: 안정성, 보안, 배포

**작업 항목**:
1. 포괄적인 에러 처리
2. Circuit breaker 패턴
3. 보안 감사
4. 성능 최적화
5. Docker 이미지 생성
6. 모니터링 (Prometheus + Grafana)
7. 운영 문서 작성

**완료 기준**:
- [ ] 모든 에러 경로 테스트
- [ ] 보안 취약점 수정
- [ ] 부하 테스트 통과
- [ ] Docker Compose 작동
- [ ] 운영 문서 완성

---

## 핵심 구현 파일 목록

### 최우선 파일 (Phase 1-6)

1. **[d:\Trader\Cargo.toml](d:\Trader\Cargo.toml)**
   - Workspace 루트 설정
   - 모든 크레이트 정의
   - 공통 의존성 관리

2. **[d:\Trader\crates\trader-core\src\lib.rs](d:\Trader\crates\trader-core\src\lib.rs)**
   - 핵심 도메인 모델
   - Order, Position, Trade, Symbol 타입
   - 에러 처리

3. **[d:\Trader\crates\trader-exchange\src\traits.rs](d:\Trader\crates\trader-exchange\src\traits.rs)**
   - Exchange trait 정의
   - 모든 거래소 연동의 기반 인터페이스

4. **[d:\Trader\crates\trader-strategy\src\engine.rs](d:\Trader\crates\trader-strategy\src\engine.rs)**
   - 전략 실행 엔진
   - 플러그인 로딩
   - 신호 생성

5. **[d:\Trader\migrations\001_initial_schema.sql](d:\Trader\migrations\001_initial_schema.sql)**
   - 데이터베이스 스키마
   - TimescaleDB 설정

6. **[d:\Trader\crates\trader-risk\src\manager.rs](d:\Trader\crates\trader-risk\src\manager.rs)**
   - 리스크 관리 핵심 로직
   - 주문 검증
   - 보호 주문 생성

7. **[d:\Trader\crates\trader-execution\src\executor.rs](d:\Trader\crates\trader-execution\src\executor.rs)**
   - 주문 실행 로직
   - 포지션 관리

### 중요 설정 파일

8. **[d:\Trader\docker-compose.yml](d:\Trader\docker-compose.yml)**
   - 로컬 개발 환경
   - PostgreSQL, Redis 설정

9. **[d:\Trader\.env.example](d:\Trader\.env.example)**
   - 환경변수 템플릿
   - API 키 등

10. **[d:\Trader\config\default.toml](d:\Trader\config\default.toml)**
    - 애플리케이션 기본 설정

---

## 검증 계획

### 단위 테스트
- 각 크레이트별 `cargo test` 실행
- 테스트 커버리지 >80% 목표

### 통합 테스트
- Mockito로 API 호출 모킹
- 전체 플로우 테스트
  1. 시장 데이터 수신
  2. 전략 신호 생성
  3. 리스크 검증
  4. 주문 실행
  5. 포지션 업데이트

### 백테스팅
- 샘플 전략으로 과거 데이터 백테스트
- 성과 지표 확인
- 리포트 생성

### 시스템 테스트
1. Docker Compose로 전체 시스템 실행
2. 웹 대시보드 접속
3. 전략 시작 및 실시간 모니터링
4. 수동 주문 실행
5. 성과 차트 확인

### 보안 테스트
- API 키 암호화 확인
- JWT 인증 테스트
- 권한 검증
- SQL 인젝션 방어 확인

### 성능 테스트
- Criterion 벤치마크
- 부하 테스트 (1000 req/s)
- 메모리 프로파일링
- CPU 사용률 모니터링

---

## 리스크 및 대응 방안

### 기술적 리스크

**R1: Rust 학습 곡선**
- 완화: 단계별 학습, 커뮤니티 활용
- 대응: 필요시 일부 기능 Node.js로 프로토타입

**R2: 거래소 API 변경**
- 완화: 추상화 레이어로 격리
- 대응: 버전별 어댑터 패턴

**R3: 데이터베이스 성능**
- 완화: TimescaleDB 최적화, 인덱싱
- 대응: 샤딩, 읽기 전용 복제본

### 운영 리스크

**R4: 네트워크 장애**
- 완화: 재연결 로직, Circuit breaker
- 대응: 다중 연결, Fallback 메커니즘

**R5: 버그로 인한 손실**
- 완화: 포괄적인 테스트, 리스크 한도
- 대응: Kill switch, 실시간 모니터링

**R6: API 키 유출**
- 완화: 암호화 저장, 최소 권한
- 대응: 즉시 키 교체, 감사 로그

### 비즈니스 리스크

**R7: 전략 성과 부진**
- 완화: 백테스팅, 다양한 전략
- 대응: A/B 테스트, 전략 교체

**R8: 시장 변동성**
- 완화: 변동성 필터, 포지션 제한
- 대응: 자동 거래 중단

---

## 성공 지표

### MVP 출시 기준 (Phase 1-6 완료)
- [ ] Binance에서 실시간 데이터 수신
- [ ] 샘플 전략 1개 이상 작동
- [ ] 리스크 관리 모든 기능 작동
- [ ] 주문 실행 및 추적 100% 작동
- [ ] 과거 데이터 저장 및 조회
- [ ] 백테스트 성공

### 베타 출시 기준 (Phase 1-8 완료)
- [ ] 웹 대시보드 작동
- [ ] 실시간 모니터링
- [ ] 전략 제어 가능
- [ ] 성과 지표 표시
- [ ] JWT 인증

### 정식 출시 기준 (Phase 1-11 완료)
- [ ] 3개 이상 거래소 지원
- [ ] ML 예측 모델 통합
- [ ] 99.9% 가용성
- [ ] 보안 감사 완료
- [ ] 운영 문서 완성
- [ ] 모니터링 대시보드

### 성과 지표 (출시 후)
- 시스템 가동률 >99.9%
- 평균 API 응답시간 <200ms
- 주문 체결률 >99.5%
- WebSocket 재연결 횟수 <10/일
- 테스트 커버리지 >80%

---

## 다음 단계

구현을 시작하려면 다음 순서로 진행합니다:

1. **Cargo workspace 초기화**
   - 루트 `Cargo.toml` 생성
   - 각 크레이트 디렉토리 생성

2. **Docker 환경 구축**
   - `docker-compose.yml` 작성
   - PostgreSQL + Redis 실행

3. **핵심 도메인 모델 작성**
   - `trader-core` 크레이트
   - Order, Position, Trade 타입

4. **데이터베이스 스키마**
   - 마이그레이션 파일 작성
   - TimescaleDB 설정

5. **Binance 연동**
   - Exchange trait
   - REST + WebSocket 클라이언트

이후 Phase별로 순차적으로 구현을 진행합니다.

---

## 전략 및 시장 선택 요약

### 🎯 최종 결정사항

**주 전략**: 그리드 트레이딩 (Grid Trading)
- 이유: 소규모 자본, 안정적 수익, 높은 승률, 24/7 운영

**주 시장**: 암호화폐 (Binance)
- 이유: 24시간 거래, 높은 변동성, 즉시 체결, 낮은 진입장벽
- 추천 코인: BTC/USDT (최우선), ETH/USDT

**보조 시장**: 한국 주식 (한국투자증권 API)
- 장 시간 한정 (09:00-15:30)
- 테마주/중소형주 그리드 전략

**초기 자본 배분 (1,000만원 기준):**
- 암호화폐 (Binance): 600만원 (60%)
  - BTC/USDT: 400만원
  - ETH/USDT: 200만원
- 한국 주식: 300만원 (30%)
- 현금 예비: 100만원 (10%)

### 📈 예상 수익

**월평균 목표:**
- 암호화폐: 18-24만원 (3-4%)
- 한국 주식: 6-9만원 (2-3%)
- 합계: **24-33만원/월 (2.4-3.3%)**

**연간 목표:**
- 복리 효과: **약 30-40% 수익률**
- 안전 여유분 고려: **실제 25-35%**

---

## 전략 플러그인 개발 가이드

### 플러그인 아키텍처 개요

전략은 독립적인 동적 라이브러리(cdylib)로 개발되며, 런타임에 로드됩니다. 각 전략은 `Strategy` trait을 구현하고, 시장 데이터를 받아 거래 신호를 생성합니다.

### 1. Strategy Trait 정의

```rust
// trader-strategy/src/lib.rs
use async_trait::async_trait;
use trader_core::{MarketData, Order, Position, Signal};
use std::collections::HashMap;
use serde_json::Value;

#[async_trait]
pub trait Strategy: Send + Sync {
    /// 전략 고유 이름
    fn name(&self) -> &str;

    /// 전략 버전
    fn version(&self) -> &str;

    /// 전략 설명
    fn description(&self) -> &str;

    /// 전략 초기화
    /// config: 전략별 설정 (JSON)
    async fn initialize(&mut self, config: Value) -> Result<(), Box<dyn std::error::Error>>;

    /// 시장 데이터 수신 시 호출
    /// 반환: 생성된 거래 신호 리스트
    async fn on_market_data(
        &mut self,
        data: &MarketData,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error>>;

    /// 주문 체결 시 호출
    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error>>;

    /// 포지션 업데이트 시 호출
    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error>>;

    /// 전략 종료 (리소스 정리)
    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>>;

    /// 전략 상태를 JSON으로 반환 (디버깅/모니터링용)
    fn get_state(&self) -> Value;
}
```

### 2. 샘플 전략 구현: RSI 평균회귀

```rust
// my-rsi-strategy/src/lib.rs
use async_trait::async_trait;
use trader_strategy::Strategy;
use trader_core::{MarketData, MarketDataType, Order, Position, Signal, SignalType, Side, Symbol};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;
use rust_decimal::Decimal;

/// RSI 평균회귀 전략 설정
#[derive(Debug, Clone, Deserialize)]
struct RsiConfig {
    /// RSI 기간
    period: usize,
    /// 과매도 기준 (예: 30)
    oversold_threshold: f64,
    /// 과매수 기준 (예: 70)
    overbought_threshold: f64,
    /// 거래할 심볼
    symbol: String,
}

/// RSI 평균회귀 전략
pub struct RsiMeanReversionStrategy {
    config: Option<RsiConfig>,
    /// 최근 종가 저장 (RSI 계산용)
    price_history: VecDeque<Decimal>,
    /// 현재 포지션 여부
    has_position: bool,
}

impl RsiMeanReversionStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            price_history: VecDeque::new(),
            has_position: false,
        }
    }

    /// RSI 계산
    fn calculate_rsi(&self) -> Option<f64> {
        let config = self.config.as_ref()?;
        if self.price_history.len() < config.period + 1 {
            return None;
        }

        let mut gains = Vec::new();
        let mut losses = Vec::new();

        for i in 0..config.period {
            let diff = self.price_history[i + 1] - self.price_history[i];
            if diff > Decimal::ZERO {
                gains.push(diff);
                losses.push(Decimal::ZERO);
            } else {
                gains.push(Decimal::ZERO);
                losses.push(diff.abs());
            }
        }

        let avg_gain: Decimal = gains.iter().sum::<Decimal>() / Decimal::from(config.period);
        let avg_loss: Decimal = losses.iter().sum::<Decimal>() / Decimal::from(config.period);

        if avg_loss.is_zero() {
            return Some(100.0);
        }

        let rs = avg_gain / avg_loss;
        let rsi = 100.0 - (100.0 / (1.0 + rs.to_f64().unwrap()));

        Some(rsi)
    }
}

#[async_trait]
impl Strategy for RsiMeanReversionStrategy {
    fn name(&self) -> &str {
        "RSI Mean Reversion"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "RSI 기반 평균회귀 전략. RSI < 30이면 매수, RSI > 70이면 매도."
    }

    async fn initialize(&mut self, config: Value) -> Result<(), Box<dyn std::error::Error>> {
        self.config = Some(serde_json::from_value(config)?);
        self.price_history.clear();
        self.has_position = false;

        println!("[{}] 전략 초기화 완료", self.name());
        Ok(())
    }

    async fn on_market_data(
        &mut self,
        data: &MarketData,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error>> {
        let config = self.config.as_ref()
            .ok_or("전략이 초기화되지 않았습니다")?;

        // 설정된 심볼이 아니면 무시
        if data.symbol.to_string() != config.symbol {
            return Ok(Vec::new());
        }

        // 캔들스틱 데이터만 처리
        let close_price = match &data.data {
            MarketDataType::Kline(kline) => kline.close,
            _ => return Ok(Vec::new()),
        };

        // 가격 히스토리 업데이트
        self.price_history.push_front(close_price);
        if self.price_history.len() > config.period + 1 {
            self.price_history.pop_back();
        }

        // RSI 계산
        let rsi = match self.calculate_rsi() {
            Some(r) => r,
            None => return Ok(Vec::new()), // 데이터 부족
        };

        println!("[{}] RSI: {:.2}", self.name(), rsi);

        let mut signals = Vec::new();

        // 매수 신호: RSI < 30 && 포지션 없음
        if rsi < config.oversold_threshold && !self.has_position {
            signals.push(Signal {
                strategy_id: self.name().to_string(),
                symbol: data.symbol.clone(),
                side: Side::Buy,
                signal_type: SignalType::Entry,
                strength: (config.oversold_threshold - rsi) / config.oversold_threshold,
                metadata: json!({
                    "rsi": rsi,
                    "reason": "oversold"
                }).as_object().unwrap().clone(),
            });

            println!("[{}] 매수 신호 생성 (RSI: {:.2})", self.name(), rsi);
        }

        // 매도 신호: RSI > 70 && 포지션 있음
        if rsi > config.overbought_threshold && self.has_position {
            signals.push(Signal {
                strategy_id: self.name().to_string(),
                symbol: data.symbol.clone(),
                side: Side::Sell,
                signal_type: SignalType::Exit,
                strength: (rsi - config.overbought_threshold) / (100.0 - config.overbought_threshold),
                metadata: json!({
                    "rsi": rsi,
                    "reason": "overbought"
                }).as_object().unwrap().clone(),
            });

            println!("[{}] 매도 신호 생성 (RSI: {:.2})", self.name(), rsi);
        }

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("[{}] 주문 체결: {:?} {} @ {}",
            self.name(), order.side, order.quantity, order.average_fill_price.unwrap());
        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.has_position = position.quantity > Decimal::ZERO;
        println!("[{}] 포지션 업데이트: {} (PnL: {})",
            self.name(), position.quantity, position.unrealized_pnl);
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("[{}] 전략 종료", self.name());
        self.price_history.clear();
        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "has_position": self.has_position,
            "price_history_length": self.price_history.len(),
            "latest_rsi": self.calculate_rsi(),
        })
    }
}

// 플러그인 진입점 - 전략 인스턴스 생성
#[no_mangle]
pub extern "C" fn create_strategy() -> *mut dyn Strategy {
    let strategy = Box::new(RsiMeanReversionStrategy::new());
    Box::into_raw(strategy)
}

// 플러그인 해제 - 메모리 정리
#[no_mangle]
pub unsafe extern "C" fn destroy_strategy(ptr: *mut dyn Strategy) {
    if !ptr.is_null() {
        drop(Box::from_raw(ptr));
    }
}
```

### 3. 플러그인 Cargo.toml 설정

```toml
# my-rsi-strategy/Cargo.toml
[package]
name = "my-rsi-strategy"
version = "1.0.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]  # 동적 라이브러리로 빌드

[dependencies]
trader-core = { path = "../trader/crates/trader-core" }
trader-strategy = { path = "../trader/crates/trader-strategy" }
async-trait = "0.1"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
rust_decimal = "1.33"
tokio = { version = "1", features = ["full"] }
```

### 4. 플러그인 빌드

```bash
# 전략 디렉토리에서
cd my-rsi-strategy

# 릴리즈 빌드 (최적화)
cargo build --release

# 생성된 파일 위치:
# Windows: target/release/my_rsi_strategy.dll
# Linux: target/release/libmy_rsi_strategy.so
# macOS: target/release/libmy_rsi_strategy.dylib
```

### 5. 전략 설정 파일

```toml
# config/strategies/rsi_mean_reversion.toml
name = "RSI Mean Reversion"
plugin_path = "./plugins/my_rsi_strategy.dll"  # Windows
# plugin_path = "./plugins/libmy_rsi_strategy.so"  # Linux
enabled = true

[parameters]
period = 14
oversold_threshold = 30.0
overbought_threshold = 70.0
symbol = "BTC/USDT"

[risk_limits]
max_position_size = "1000.0"  # USDT
max_daily_loss = "100.0"      # USDT
stop_loss_pct = "2.0"         # 2%
take_profit_pct = "5.0"       # 5%
```

### 6. 전략 로딩 및 실행

```rust
// trader-strategy/src/plugin/loader.rs
use libloading::{Library, Symbol};
use std::path::Path;

pub struct StrategyPlugin {
    _lib: Library,
    create_fn: extern "C" fn() -> *mut dyn Strategy,
    destroy_fn: unsafe extern "C" fn(*mut dyn Strategy),
}

impl StrategyPlugin {
    pub unsafe fn load<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let lib = Library::new(path.as_ref())?;

        let create_fn: Symbol<extern "C" fn() -> *mut dyn Strategy> =
            lib.get(b"create_strategy")?;
        let destroy_fn: Symbol<unsafe extern "C" fn(*mut dyn Strategy)> =
            lib.get(b"destroy_strategy")?;

        Ok(Self {
            _lib: lib,
            create_fn: *create_fn,
            destroy_fn: *destroy_fn,
        })
    }

    pub fn create_instance(&self) -> Box<dyn Strategy> {
        unsafe {
            let raw = (self.create_fn)();
            Box::from_raw(raw)
        }
    }
}

// 전략 엔진에서 사용
pub async fn load_and_run_strategy(config_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    // 설정 로드
    let config = load_strategy_config(config_path)?;

    // 플러그인 로드
    let plugin = unsafe { StrategyPlugin::load(&config.plugin_path)? };
    let mut strategy = plugin.create_instance();

    // 전략 초기화
    strategy.initialize(config.parameters).await?;

    // 시장 데이터 스트림 구독
    let mut market_data_rx = subscribe_market_data().await?;

    // 메인 루프
    loop {
        tokio::select! {
            Some(data) = market_data_rx.recv() => {
                // 전략에 데이터 전달
                let signals = strategy.on_market_data(&data).await?;

                // 신호 처리
                for signal in signals {
                    handle_signal(signal).await?;
                }
            }
            _ = tokio::signal::ctrl_c() => {
                break;
            }
        }
    }

    // 전략 종료
    strategy.shutdown().await?;

    Ok(())
}
```

### 7. 전략 백테스팅

```rust
// trader-analytics/src/backtest/engine.rs
use trader_strategy::Strategy;
use trader_data::DataManager;

pub struct BacktestEngine {
    strategy: Box<dyn Strategy>,
    data_manager: DataManager,
    initial_capital: Decimal,
}

impl BacktestEngine {
    pub async fn run(
        &mut self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<BacktestReport, Box<dyn std::error::Error>> {
        // 전략 초기화
        self.strategy.initialize(/* config */).await?;

        // 과거 데이터 로드
        let klines = self.data_manager
            .get_historical_klines(/* symbol, timeframe, start, end */)
            .await?;

        let mut portfolio = Portfolio::new(self.initial_capital);

        // 각 캔들에 대해 전략 실행
        for kline in klines {
            let market_data = MarketData::from(kline);

            // 전략 신호 생성
            let signals = self.strategy.on_market_data(&market_data).await?;

            // 신호 실행 (시뮬레이션)
            for signal in signals {
                let order = self.simulate_order(signal, &market_data)?;
                portfolio.apply_order(order);
            }
        }

        // 리포트 생성
        Ok(BacktestReport::from_portfolio(portfolio))
    }
}
```

### 8. 전략 개발 체크리스트

**구현 단계:**
- [ ] `Strategy` trait 구현
- [ ] 전략 로직 작성 (기술적 지표, 신호 생성)
- [ ] `create_strategy()` 진입점 함수 작성
- [ ] `Cargo.toml`에 `crate-type = ["cdylib"]` 설정
- [ ] 로컬 빌드 및 테스트

**테스트 단계:**
- [ ] 단위 테스트 작성 (RSI 계산 등)
- [ ] 백테스팅으로 과거 성과 확인
- [ ] 설정 파일 작성
- [ ] 플러그인 로딩 테스트

**배포 단계:**
- [ ] 릴리즈 빌드 (`cargo build --release`)
- [ ] 플러그인 파일을 `plugins/` 디렉토리에 복사
- [ ] 설정 파일을 `config/strategies/`에 복사
- [ ] 전략 활성화 (`enabled = true`)

### 9. 고급 기능

**상태 저장 및 복원:**
```rust
impl Strategy for MyStrategy {
    async fn save_state(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let state = bincode::serialize(&self.internal_state)?;
        Ok(state)
    }

    async fn load_state(&mut self, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        self.internal_state = bincode::deserialize(data)?;
        Ok(())
    }
}
```

**다중 타임프레임 전략:**
```rust
async fn on_market_data(&mut self, data: &MarketData) -> Result<Vec<Signal>, Box<dyn std::error::Error>> {
    match &data.data {
        MarketDataType::Kline(kline) => {
            match kline.timeframe {
                Timeframe::M5 => self.update_short_term(kline),
                Timeframe::H1 => self.update_medium_term(kline),
                Timeframe::D1 => self.update_long_term(kline),
                _ => {}
            }

            // 여러 타임프레임 분석 결과를 종합
            self.generate_signals()
        }
        _ => Ok(Vec::new())
    }
}
```

**의존성 주입 (데이터 접근):**
```rust
pub trait Strategy {
    // 데이터 매니저 주입
    fn set_data_manager(&mut self, data_manager: Arc<DataManager>);
}

// 전략 내부에서 사용
async fn on_market_data(&mut self, data: &MarketData) -> Result<Vec<Signal>, Box<dyn std::error::Error>> {
    // 추가 데이터 조회
    let historical = self.data_manager
        .get_recent_klines(symbol, timeframe, 100)
        .await?;

    // 분석 로직...
}
```

### 10. 참고 자료

**기술적 지표 라이브러리:**
- `ta` crate: https://docs.rs/ta/
- 주요 지표: SMA, EMA, RSI, MACD, Bollinger Bands, ATR 등

**전략 아이디어:**
- 트렌드 추종: 이동평균 크로스오버, 돌파 전략
- 평균회귀: RSI, 볼린저 밴드 반전
- 모멘텀: MACD, Stochastic
- 변동성: ATR 기반 포지션 사이징

**주의사항:**
- 전략은 순수 함수형으로 작성 (부작용 최소화)
- 과최적화(overfitting) 주의
- 리스크 관리 필수 (스톱로스, 포지션 크기)
- 백테스팅 결과와 실전 결과 차이 고려 (슬리피지, 수수료)

---

## 그리드 트레이딩 전략 상세 가이드

### 전략 선택 근거

프로젝트의 주요 전략으로 **그리드 트레이딩**을 우선 구현하는 이유:

1. **소규모 자본에 적합**: 1천만원 미만 자본으로도 효과적 운영
2. **높은 승률**: 70-80% 승률로 심리적 안정감
3. **안정적 수익**: 빈번한 소액 수익으로 꾸준한 자본 증식
4. **혼합형 시장**: 트렌드와 횡보 모두에서 수익 가능
5. **자동화 용이**: 명확한 규칙으로 24/7 무인 운영

### 그리드 트레이딩 개념

**기본 원리:**
1. 기준 가격을 중심으로 일정 간격으로 매수/매도 주문 배치
2. 가격이 하락하면 자동 매수, 상승하면 자동 매도
3. 가격 변동마다 작은 수익 실현

**예시 (기준가 100원, 간격 1%):**
```
106원 ─── 매도 대기
105원 ─── 매도 대기
104원 ─── 매도 대기
103원 ─── 매도 대기
102원 ─── 매도 대기
101원 ─── 매도 대기
──────── 기준가 (100원) ────────
99원  ─── 매수 대기
98원  ─── 매수 대기
97원  ─── 매수 대기
96원  ─── 매수 대기
95원  ─── 매수 대기
94원  ─── 매수 대기
```

### 1. 단순 그리드 전략 구현

```rust
// strategies/grid-trading/src/lib.rs
use async_trait::async_trait;
use trader_strategy::Strategy;
use trader_core::{MarketData, MarketDataType, Order, Position, Signal, SignalType, Side, Symbol};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
struct GridConfig {
    /// 거래 심볼
    symbol: String,
    /// 그리드 중심 가격 (현재가 or 지정가)
    center_price: Option<Decimal>,
    /// 그리드 간격 (%)
    grid_spacing_pct: f64,
    /// 그리드 레벨 수 (위/아래 각각)
    grid_levels: usize,
    /// 레벨당 투자 금액
    amount_per_level: Decimal,
    /// 상한/하한 가격 (선택)
    upper_limit: Option<Decimal>,
    lower_limit: Option<Decimal>,
}

struct GridLevel {
    price: Decimal,
    side: Side,
    executed: bool,
    order_id: Option<String>,
}

pub struct GridTradingStrategy {
    config: Option<GridConfig>,
    grid_levels: Vec<GridLevel>,
    current_price: Option<Decimal>,
}

impl GridTradingStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            grid_levels: Vec::new(),
            current_price: None,
        }
    }

    /// 그리드 레벨 초기화
    fn initialize_grid(&mut self, center_price: Decimal) {
        let config = self.config.as_ref().unwrap();
        self.grid_levels.clear();

        let spacing = center_price * Decimal::from_f64(config.grid_spacing_pct / 100.0).unwrap();

        // 매수 그리드 (기준가 아래)
        for i in 1..=config.grid_levels {
            let price = center_price - (spacing * Decimal::from(i));

            // 하한가 체크
            if let Some(lower) = config.lower_limit {
                if price < lower {
                    continue;
                }
            }

            self.grid_levels.push(GridLevel {
                price,
                side: Side::Buy,
                executed: false,
                order_id: None,
            });
        }

        // 매도 그리드 (기준가 위)
        for i in 1..=config.grid_levels {
            let price = center_price + (spacing * Decimal::from(i));

            // 상한가 체크
            if let Some(upper) = config.upper_limit {
                if price > upper {
                    continue;
                }
            }

            self.grid_levels.push(GridLevel {
                price,
                side: Side::Sell,
                executed: false,
                order_id: None,
            });
        }

        // 가격순 정렬
        self.grid_levels.sort_by(|a, b| a.price.cmp(&b.price));

        println!("[그리드] {} 레벨 생성 완료 (중심가: {})",
            self.grid_levels.len(), center_price);
    }

    /// 현재 가격에서 실행할 신호 생성
    fn generate_grid_signals(&mut self, current_price: Decimal) -> Vec<Signal> {
        let mut signals = Vec::new();
        let config = self.config.as_ref().unwrap();

        for level in &mut self.grid_levels {
            // 이미 실행된 레벨은 스킵
            if level.executed {
                continue;
            }

            // 매수 그리드: 현재가가 레벨 가격 이하로 떨어짐
            if level.side == Side::Buy && current_price <= level.price {
                signals.push(Signal {
                    strategy_id: "grid_trading".to_string(),
                    symbol: Symbol::from_string(&config.symbol),
                    side: Side::Buy,
                    signal_type: SignalType::Entry,
                    strength: 1.0,
                    metadata: json!({
                        "grid_price": level.price,
                        "grid_type": "buy",
                        "amount": config.amount_per_level
                    }).as_object().unwrap().clone(),
                });

                level.executed = true;
                println!("[그리드] 매수 신호: {} @ {}", config.amount_per_level, level.price);
            }

            // 매도 그리드: 현재가가 레벨 가격 이상으로 상승
            if level.side == Side::Sell && current_price >= level.price {
                // 매도는 보유 포지션이 있을 때만
                // (실제로는 포지션 확인 필요)
                signals.push(Signal {
                    strategy_id: "grid_trading".to_string(),
                    symbol: Symbol::from_string(&config.symbol),
                    side: Side::Sell,
                    signal_type: SignalType::Exit,
                    strength: 1.0,
                    metadata: json!({
                        "grid_price": level.price,
                        "grid_type": "sell",
                        "amount": config.amount_per_level
                    }).as_object().unwrap().clone(),
                });

                level.executed = true;
                println!("[그리드] 매도 신호: {} @ {}", config.amount_per_level, level.price);
            }
        }

        // 실행된 레벨을 리셋 (반대 방향 움직임 대비)
        // 예: 매수 후 가격이 다시 떨어지면 재매수
        self.reset_executed_levels(current_price);

        signals
    }

    /// 실행된 레벨 리셋 (가격이 반대로 움직였을 때)
    fn reset_executed_levels(&mut self, current_price: Decimal) {
        for level in &mut self.grid_levels {
            if !level.executed {
                continue;
            }

            // 매수 레벨: 가격이 다시 레벨 위로 올라가면 리셋
            if level.side == Side::Buy && current_price > level.price * dec!(1.005) {
                level.executed = false;
            }

            // 매도 레벨: 가격이 다시 레벨 아래로 내려가면 리셋
            if level.side == Side::Sell && current_price < level.price * dec!(0.995) {
                level.executed = false;
            }
        }
    }
}

#[async_trait]
impl Strategy for GridTradingStrategy {
    fn name(&self) -> &str {
        "Grid Trading"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "단순 그리드 트레이딩 전략. 일정 간격으로 매수/매도 주문을 배치하여 변동성에서 수익 실현."
    }

    async fn initialize(&mut self, config: Value) -> Result<(), Box<dyn std::error::Error>> {
        self.config = Some(serde_json::from_value(config)?);
        println!("[그리드] 전략 초기화");
        Ok(())
    }

    async fn on_market_data(
        &mut self,
        data: &MarketData,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error>> {
        let config = self.config.as_ref()
            .ok_or("전략이 초기화되지 않았습니다")?;

        if data.symbol.to_string() != config.symbol {
            return Ok(Vec::new());
        }

        // 현재가 업데이트
        let current_price = match &data.data {
            MarketDataType::Ticker(ticker) => ticker.last_price,
            MarketDataType::Kline(kline) => kline.close,
            _ => return Ok(Vec::new()),
        };

        // 첫 실행: 그리드 초기화
        if self.grid_levels.is_empty() {
            let center = config.center_price.unwrap_or(current_price);
            self.initialize_grid(center);
        }

        self.current_price = Some(current_price);

        // 신호 생성
        Ok(self.generate_grid_signals(current_price))
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("[그리드] 주문 체결: {:?} {} @ {}",
            order.side, order.quantity, order.average_fill_price.unwrap());
        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("[그리드] 포지션: {} (PnL: {})",
            position.quantity, position.unrealized_pnl);
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("[그리드] 전략 종료");
        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "grid_levels": self.grid_levels.len(),
            "executed_levels": self.grid_levels.iter().filter(|l| l.executed).count(),
            "current_price": self.current_price,
        })
    }
}

#[no_mangle]
pub extern "C" fn create_strategy() -> *mut dyn Strategy {
    Box::into_raw(Box::new(GridTradingStrategy::new()))
}
```

### 2. 그리드 전략 설정 파일

```toml
# config/strategies/grid_btc.toml
name = "BTC Grid Trading"
plugin_path = "./plugins/grid_trading.dll"
enabled = true

[parameters]
symbol = "BTC/USDT"
center_price = null  # null이면 현재가 사용
grid_spacing_pct = 1.0  # 1% 간격
grid_levels = 10  # 위/아래 각 10개 (총 20개)
amount_per_level = "50000"  # 레벨당 5만원 (USDT)
upper_limit = null  # 상한 없음
lower_limit = null  # 하한 없음

[risk_limits]
max_position_size = "1000000"  # 최대 100만원
max_daily_loss = "50000"  # 일일 최대 손실 5만원
```

### 3. 동적 그리드 전략 (ATR 기반)

```rust
// 변동성에 따라 그리드 간격 자동 조정
fn calculate_dynamic_spacing(&self, atr: Decimal, current_price: Decimal) -> Decimal {
    let atr_pct = (atr / current_price) * dec!(100);

    // ATR 비율에 따라 간격 조정
    let spacing_pct = if atr_pct > dec!(5.0) {
        dec!(2.0)  // 변동성 높음 → 넓은 간격
    } else if atr_pct > dec!(2.0) {
        dec!(1.0)  // 중간 변동성
    } else {
        dec!(0.5)  // 낮은 변동성 → 좁은 간격
    };

    current_price * spacing_pct / dec!(100)
}
```

### 4. 트렌드 필터 그리드

```rust
// 트렌드 방향에 따라 그리드 활성화
fn should_activate_grid(&self, trend: TrendDirection) -> (bool, bool) {
    match trend {
        TrendDirection::StrongUp => (true, false),   // 매수 그리드만
        TrendDirection::StrongDown => (false, true),  // 매도 그리드만
        TrendDirection::Sideways => (true, true),     // 양방향
        TrendDirection::Uncertain => (false, false),  // 거래 중단
    }
}
```

### 5. 백테스팅 예상 결과

**시뮬레이션 조건:**
- 자본: 1,000만원
- 시장: BTC/USDT
- 기간: 2024년 1월 ~ 12월
- 그리드 간격: 1%
- 레벨: 20개

**예상 성과:**
- **승률**: 75-80%
- **월평균 수익률**: 3-5%
- **최대 낙폭**: -15%
- **샤프 비율**: 1.5-2.0
- **거래 횟수**: 월 50-100회

### 6. 실전 운영 가이드

**초기 설정:**
1. 소액으로 시작 (100만원)
2. 변동성 높은 종목 선택 (BTC, 테마주)
3. 그리드 간격 1-2%로 설정
4. 5-10개 레벨로 시작

**모니터링:**
- 일일 수익률 확인
- 그리드 실행률 (executed ratio)
- 포지션 집중도
- 수수료 비용

**최적화:**
- 2주마다 간격 조정
- 변동성 변화 반영
- 수수료 대비 수익 분석
- 레벨 수 조정

**리스크 관리:**
- 총 포지션 한도: 자본의 70%
- 일일 손실 한도: 자본의 3%
- 강한 트렌드 시 거래 중단
- Stop-loss: 중심가 대비 -20%

### 7. 소규모 자본 최적 설정

**1,000만원 이하 권장 설정:**

```toml
# 보수적 (안정 지향)
grid_spacing_pct = 1.5
grid_levels = 5
amount_per_level = "100000"  # 10만원
총 필요 자본: 약 500만원

# 균형형 (추천)
grid_spacing_pct = 1.0
grid_levels = 8
amount_per_level = "60000"  # 6만원
총 필요 자본: 약 480만원

# 공격적 (수익 극대화)
grid_spacing_pct = 0.7
grid_levels = 10
amount_per_level = "40000"  # 4만원
총 필요 자본: 약 400만원
```

### 8. 성공을 위한 체크리스트

- [ ] 변동성 3% 이상인 종목 선택
- [ ] 거래량 충분한 종목 (슬리피지 최소화)
- [ ] 수수료 저렴한 거래소 (0.1% 이하)
- [ ] 그리드 간격 = 일일 변동성의 1/3
- [ ] 백테스팅 3개월 이상 데이터
- [ ] 실전 전 데모 계좌로 1주일 테스트
- [ ] 일일 모니터링 루틴 확립
- [ ] 자본 30% 현금 여유 유지
