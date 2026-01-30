# Changelog

프로젝트의 모든 주요 변경 사항을 기록합니다.

형식은 [Keep a Changelog](https://keepachangelog.com/ko/1.0.0/)를 따르며,
[Semantic Versioning](https://semver.org/lang/ko/)을 준수합니다.

## [0.4.0] - 2026-01-31

### Added

#### ML 훈련 파이프라인
- **Python ML 훈련 스크립트** (`scripts/train_ml_model.py`)
  - XGBoost, LightGBM, RandomForest 모델 지원
  - DB에서 OHLCV 데이터 자동 로드
  - 기술적 지표 기반 피처 엔지니어링 (30+ 피처)
  - ONNX 포맷으로 모델 내보내기
- **ML 모듈 구조** (`scripts/ml/`)
  - `data_fetcher.py`: TimescaleDB에서 데이터 가져오기
  - `feature_engineering.py`: RSI, MACD, Bollinger, ATR 등 피처 생성
  - `model_trainer.py`: 하이퍼파라미터 튜닝, 교차 검증
- **ML Docker 이미지** (`Dockerfile.ml`)
  - Python 3.11 + 과학 계산 라이브러리
  - `docker-compose --profile ml` 로 실행
- **Python 프로젝트 설정** (`pyproject.toml`)
  - uv 패키지 매니저 지원
  - 의존성: pandas, scikit-learn, xgboost, lightgbm, onnx

#### ML API 확장
- **ML 서비스 레이어** (`ml/service.rs`): 예측 로직 분리
- **ML API 엔드포인트** (`routes/ml.rs`): 모델 목록, 예측 API 확장
- **예측기 개선** (`predictor.rs`): 다중 모델 지원

#### Execution Cache
- **실행 캐시 Repository** (`execution_cache.rs`): 전략 실행 상태 캐싱

### Changed
- `Dataset.tsx`: 데이터셋 페이지 UI/UX 개선
- `MultiPanelGrid.tsx`: 차트 패널 레이아웃 개선
- `patterns.rs`: 패턴 인식 API 개선
- `state.rs`: AppState ML 서비스 통합

---

## [0.3.0] - 2026-01-30

### Added

#### 10개 신규 전략 추가 (총 27개)
- **BAA** (Bold Asset Allocation): 카나리아 자산 기반 공격/수비 모드 전환
- **Dual Momentum**: 절대/상대 모멘텀 기반 자산 배분 (Gary Antonacci)
- **Kosdaq Fire Rain** (코스닥 불비): 코스닥 단타 변동성 돌파
- **KOSPI Bothside** (코스피 양방향): 롱숏 양방향 매매
- **Pension Bot** (연금봇): 연금 계좌 자동 운용 (MDD 최소화)
- **Sector Momentum**: 섹터 ETF 로테이션 전략
- **Sector VB**: 섹터별 변동성 돌파
- **Small Cap Quant**: 소형주 퀀트 팩터 전략
- **Stock Gugan** (주식 구간): 구간별 분할 매매
- **US 3X Leverage**: 미국 3배 레버리지 ETF 전략 (TQQQ/SOXL)

#### Symbol Info Provider
- **종목 정보 캐싱** (`symbol_info.rs`): KIS API 종목 정보 조회/캐싱
- 종목명, 시장 구분, 가격 정보, 거래 단위 등 메타데이터 관리
- DB 마이그레이션: `012_symbol_info.sql`

#### Docker 빌드 최적화
- **sccache**: Rust 증분 빌드 캐시 (재빌드 시 50-80% 시간 단축)
- **mold 링커**: lld보다 2-3배 빠른 링킹
- Crate 수정 빈도별 빌드 순서 최적화
- 개발 스크립트 추가: `scripts/dev-build.ps1`, `scripts/docker-build.ps1`

#### 아키텍처 문서
- **architecture.md**: 시스템 아키텍처 상세 문서화
- Crate 간 의존성, 데이터 흐름, 배포 구조 설명

#### 테스트 자동화
- **전략 테스트 스크립트** (`scripts/test_all_strategies.py`)
- 모든 전략 백테스트 자동 검증

### Changed

#### API 개선
- `analytics.rs`: 성과 분석 API 확장 (기간별 통계, 상세 메트릭)
- `backtest.rs`: 결과 저장/조회 API 개선
- `dataset.rs`: 다중 심볼 지원, 배치 다운로드
- `equity_history.rs`: 자산 이력 조회 API 추가

#### 프론트엔드
- `Dataset.tsx`: 다중 심볼 관리, 배치 작업 UI
- `MultiPanelGrid.tsx`: 차트 패널 레이아웃 개선
- `PortfolioEquityChart.tsx`: 성과 차트 시각화 개선
- `Strategies.tsx`: 신규 전략 지원

#### 데이터 레이어
- `historical.rs`: 캐시 효율성 개선
- `ohlcv.rs`: 저장소 최적화

### Database Migrations
- `011_execution_cache.sql`: 실행 캐시 테이블
- `012_symbol_info.sql`: 종목 정보 테이블
- `013_strategy_timeframe.sql`: 전략 타임프레임 설정

---

## [0.2.0] - 2026-01-30

### Added

#### 데이터셋 관리 시스템
- **데이터셋 페이지** (`Dataset.tsx`): OHLCV 데이터 조회/다운로드/관리 UI
  - Yahoo Finance에서 심볼 데이터 다운로드
  - 캔들 수 또는 날짜 범위 지정 다운로드
  - 무한 스크롤링 테이블 (Intersection Observer API)
  - 실시간 차트 시각화 (멀티 타임프레임 지원)
- **데이터셋 API** (`dataset.rs`): OHLCV 데이터 CRUD 엔드포인트
- **OHLCV 저장소 리팩토링**: `yahoo_cache.rs` → `ohlcv.rs`로 이름 변경

#### 백테스트 결과 저장
- **백테스트 결과 API** (`backtest_results.rs`): 백테스트 결과 저장/조회
- **DB 마이그레이션**: `010_backtest_results.sql` - 결과 테이블 추가
- 과거 백테스트 결과 조회 및 비교 기능

#### 전략 워크플로우 개선
- **등록된 전략 기반 백테스트**: 전략 페이지에서 먼저 등록 → 백테스트/시뮬레이션에서 선택
- **전략 Repository 패턴** (`repository/strategies.rs`): 데이터 접근 계층 분리
- **전략 자동 로드**: 서버 시작 시 DB에서 저장된 전략 자동 로드
- **strategy_type 필드 추가**: 전략 타입 구분 (`volatility_breakout`, `grid` 등)
- **symbols 필드 추가**: 전략별 대상 심볼 목록 저장

#### 차트 시스템 개선
- **동기화된 차트 패널** (`SyncedChartPanel.tsx`): 다중 차트 동기화 지원
- **멀티 패널 그리드** (`MultiPanelGrid.tsx`): 차트 패널 레이아웃 관리
- **PriceChart 개선**: 1시간 타임프레임 Unix timestamp 변환 수정

### Changed

#### 프론트엔드
- `Backtest.tsx`: 등록된 전략 선택 방식으로 전환, 파라미터 입력 폼 제거
- `Simulation.tsx`: 동일한 전략 선택 방식 적용
- `Strategies.tsx`: strategy_type, symbols 필드 지원
- `App.tsx`: Dataset 페이지 라우트 추가
- `Layout.tsx`: 데이터셋 메뉴 추가

#### 백엔드
- `backtest.rs`: 등록된 전략 ID 기반 실행 지원
- `historical.rs`: 지표 계산에 isDailyOrHigher 파라미터 추가
- `volatility_breakout.rs`: is_new_period 날짜 비교 로직 개선

### Removed
- `docs/prd.md`: 불필요한 대용량 PRD 문서 제거 (38,000+ 토큰)
- docker-compose.yml에서 불필요한 설정 제거

### Database Migrations
- `008_strategies_type_and_symbols.sql`: 전략 타입/심볼 컬럼 추가
- `009_rename_candle_cache.sql`: 테이블명 리네이밍
- `010_backtest_results.sql`: 백테스트 결과 테이블

---

## [0.1.0] - 2026-01-30

### Added

#### 핵심 시스템
- Rust 기반 모듈형 아키텍처 구축 (10개 crate)
- 비동기 런타임 (Tokio) 기반 고성능 처리
- PostgreSQL (TimescaleDB) + Redis 데이터 저장소

#### 거래소 연동
- **Binance**: 현물 거래, WebSocket 실시간 시세
- **한국투자증권 (KIS)**:
  - OAuth 2.0 인증 (자동 토큰 갱신)
  - 국내/해외 주식 주문 (매수/매도/정정/취소)
  - WebSocket 실시간 연동 (국내/해외)
  - 모의투자 계좌 지원
  - 휴장일 관리 시스템
- Yahoo Finance 데이터 연동

#### 전략 시스템 (17개 전략)
- **실시간 전략**: Grid Trading, RSI, Bollinger Bands, Magic Split, Infinity Bot, Trailing Stop
- **일간 전략**: Volatility Breakout, SMA Crossover, Snow, Stock Rotation, Market Interest Day, Candle Pattern
- **월간 자산배분**: All Weather, HAA, XAA, Simple Power, Market Cap Top
- 플러그인 기반 동적 전략 로딩
- Strategy trait 기반 확장 가능한 구조

#### 백테스트 시스템
- 단일 자산 전략 백테스트 (6종 검증 완료)
- 시뮬레이션 거래소 (매칭 엔진)
- 성과 지표 계산 (Sharpe Ratio, MDD, Win Rate 등)

#### ML/AI 기능
- 패턴 인식 엔진 (47가지: 캔들스틱 25개 + 차트 22개)
- 피처 엔지니어링 (25-30개 기술 지표)
- ONNX Runtime 추론 시스템
- Python 훈련 파이프라인 (XGBoost, LightGBM, RandomForest)

#### 리스크 관리
- 자동 스톱로스/테이크프로핏
- 포지션 크기 제한
- 일일 손실 한도
- ATR 기반 변동성 필터
- Circuit Breaker 패턴

#### Web API & 대시보드
- Axum 기반 REST API
- WebSocket 실시간 통신
- SolidJS + TypeScript 프론트엔드
- 실시간 포트폴리오 모니터링
- 전략 관리 UI (시작/중지/설정)
- 백테스트 실행 및 결과 시각화
- 설정 화면 (API 키, 텔레그램, 리스크)
- 포트폴리오 분석 차트 (Equity Curve, Drawdown)

#### 알림 시스템
- Telegram 알림 연동
- 체결/신호/리스크 경고 알림

#### 인프라
- Docker / Docker Compose 지원
- Prometheus / Grafana 모니터링 설정
- 데이터베이스 마이그레이션 시스템

### Security
- API 키 AES-256-GCM 암호화 저장
- JWT 기반 인증
- CORS 설정

---

## 로드맵

### [0.4.0] - 예정
- 매매일지 (Trading Journal) 기능 완성
- 다중 자산 백테스트 지원 (다중 심볼 전략)
- WebSocket 이벤트 브로드캐스트 완성

### [0.5.0] - 예정
- 추가 거래소 통합 (Coinbase, 키움증권)
- 성능 최적화 및 부하 테스트
- Grafana 대시보드 사전 설정
