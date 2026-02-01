# Changelog

프로젝트의 모든 주요 변경 사항을 기록합니다.

형식은 [Keep a Changelog](https://keepachangelog.com/ko/1.0.0/)를 따르며,
[Semantic Versioning](https://semver.org/lang/ko/)을 준수합니다.

## [0.5.3] - 2026-02-01

### Added

#### 🔍 모니터링 및 에러 추적 시스템
- **ErrorTracker** (`monitoring/error_tracker.rs`)
  - AI 디버깅을 위한 구조화된 에러 로그 수집
  - 에러 심각도별 분류 (Warning, Error, Critical)
  - 에러 카테고리별 분류 (Database, ExternalApi, DataConversion, Authentication, Network, BusinessLogic, System)
  - 메모리 기반 에러 히스토리 보관 (최대 1000개)
  - 에러 발생 위치, 컨텍스트, 스택 트레이스 자동 수집
  - Critical 에러 발생 시 Telegram 알림 지원

- **모니터링 API** (`routes/monitoring.rs`)
  - `GET /api/v1/monitoring/errors` - 에러 목록 조회 (심각도/카테고리 필터)
  - `GET /api/v1/monitoring/errors/critical` - Critical 에러 조회
  - `GET /api/v1/monitoring/errors/:id` - 특정 에러 상세
  - `GET /api/v1/monitoring/stats` - 에러 통계 (심각도별/카테고리별 집계)
  - `GET /api/v1/monitoring/summary` - 시스템 모니터링 요약
  - `POST /api/v1/monitoring/stats/reset` - 통계 초기화
  - `DELETE /api/v1/monitoring/errors` - 에러 히스토리 삭제

#### 📊 CSV 기반 심볼 동기화
- **KRX CSV 동기화** (`tasks/krx_csv_sync.rs`)
  - `data/krx_codes.csv`에서 종목 코드 동기화
  - `data/krx_sector_map.csv`에서 업종 정보 업데이트
  - KOSPI/KOSDAQ 자동 판별 (0으로 시작: KOSPI, 1~4로 시작: KOSDAQ)
  - Yahoo Finance 심볼 자동 생성 (.KS/.KQ 접미사)

- **EODData CSV 동기화** (`tasks/eod_csv_sync.rs`)
  - NYSE, NASDAQ, AMEX, LSE, TSX, ASX, HKEX, SGX 등 해외 거래소 지원
  - 거래소별 Market 코드 자동 매핑 (US, GB, CA, AU, HK, SG 등)
  - 배치 upsert로 대량 심볼 동기화

- **데이터 파일**
  - `data/krx_codes.csv` - KRX 종목 코드 (KOSPI/KOSDAQ)
  - `data/krx_sector_map.csv` - KRX 업종 매핑

#### 🛠️ Python 스크래퍼
- `scripts/scrape_eoddata_symbols.py` - EODData 심볼 스크래핑 도구
- `scripts/requirements-scraper.txt` - 스크래퍼 의존성

#### 📄 문서
- `docs/fulltest_workflow.md` - 전체 테스트 워크플로우 가이드
- `docs/improvement_roadmap.md` - 코드베이스 개선 로드맵
- `docs/improvement_todo.md` - 개선사항 TODO 목록

### Changed

#### Fundamental 캐시 개선
- `cache/fundamental.rs`: 데이터 변환 로직 개선

### Database

- `migrations/021_fix_fundamental_decimal_precision.sql`
  - Decimal 정밀도 확장: `DECIMAL(8,4)` → `DECIMAL(12,4)`
  - 극단적 성장률 지원 (스타트업/바이오텍: 21,000%+ 성장률)
  - 영향 컬럼: ROE, ROA, 영업이익률, 순이익률, 매출/이익 성장률, 배당 관련

---

## [0.5.2] - 2026-01-31

### Added

#### 🔄 백그라운드 데이터 수집 시스템
- **FundamentalCollector** (`tasks/fundamental.rs`)
  - Yahoo Finance에서 펀더멘털 데이터 자동 수집
  - 설정 가능한 수집 주기 및 배치 처리
  - Rate limiting 기반 API 요청 관리
  - OHLCV 캔들 데이터 증분 업데이트 지원
- **SymbolSyncTask** (`tasks/symbol_sync.rs`)
  - KRX (KOSPI/KOSDAQ) 종목 자동 동기화
  - Binance USDT 거래 페어 동기화
  - Yahoo Finance 주요 지수 종목 동기화
  - 최소 심볼 수 기반 자동 실행 조건

#### 📊 프론트엔드 스크리닝 페이지
- **Screening.tsx** - 종목 스크리닝 UI 구현
  - 프리셋 스크리닝 (가치주, 고배당, 성장주 등)
  - 커스텀 필터 조합
  - 결과 테이블 및 정렬

#### 🛠️ 환경 변수 확장 (.env.example)
- `FUNDAMENTAL_COLLECT_*`: 펀더멘털 수집 설정 (활성화, 주기, 배치 크기)
- `SYMBOL_SYNC_*`: 심볼 동기화 설정 (KRX, Binance, Yahoo)

### Changed

#### 브랜딩 통일
- Web UI 타이틀을 "ZeroQuant │ 퀀트 트레이딩 플랫폼"으로 통일
- 사이드바 로고 텍스트 "Zero Quant" → "ZeroQuant"로 변경

#### 데이터 캐시 확장
- **FundamentalCache** (`cache/fundamental.rs`) - 펀더멘털 데이터 캐싱
- **SymbolInfoProvider** 확장 - 심볼 정보 조회 기능 강화

---

## [0.5.1] - 2026-01-31

### Added

#### 🔍 종목 스크리닝 (Symbol Screening) - 백엔드 API
- **ScreeningRepository** (`repository/screening.rs`, 592줄)
  - Fundamental + OHLCV 기반 종목 필터링
  - 다양한 조건 조합 지원 (시가총액, PER, PBR, ROE, 배당수익률 등)
- **스크리닝 API** (`routes/screening.rs`, 574줄)
  - `POST /api/v1/screening` - 커스텀 스크리닝 실행
  - `GET /api/v1/screening/presets` - 프리셋 목록 조회
  - `GET /api/v1/screening/presets/{preset}` - 프리셋 스크리닝 실행
  - `GET /api/v1/screening/momentum` - 모멘텀 기반 스크리닝
- **사전 정의 프리셋 6종**
  - `value`: 저PER + 저PBR 가치주
  - `dividend`: 고배당주 (배당수익률 3%+)
  - `growth`: 고ROE 성장주 (ROE 15%+)
  - `snowball`: 스노우볼 전략 (저PBR + 고배당)
  - `large_cap`: 대형주 (시가총액 상위)
  - `near_52w_low`: 52주 신저가 근접 종목

#### Symbol Fundamental 확장
- **SymbolFundamentalRepository** (`repository/symbol_fundamental.rs`, 459줄)
  - 종목 기본정보 CRUD
  - 섹터별/시장별 조회
- **SymbolInfoRepository 확장** (439줄 추가)
  - 시장 정보, 섹터 정보 조회
  - 종목 검색 기능 강화

### Changed

#### 전략 개선
- `kosdaq_fire_rain.rs`: 조건 로직 개선
- `kospi_bothside.rs`: 양방향 매매 조건 정밀화
- `sector_vb.rs`: 섹터별 변동성 돌파 조건 개선
- `us_3x_leverage.rs`: 레버리지 조건 최적화

#### 백테스트/분석 개선
- `analytics/charts.rs`: 차트 데이터 생성 개선
- `analytics/performance.rs`: 성과 지표 계산 확장
- `backtest/loader.rs`, `backtest/mod.rs`: 데이터 로딩 최적화

#### 프론트엔드 개선
- `Backtest.tsx`: 백테스트 UI 개선
- `PortfolioEquityChart.tsx`: 차트 렌더링 최적화
- `Dashboard.tsx`: 대시보드 개선

#### 코드 품질
- `.rustfmt.toml`: Rust 코드 포맷팅 규칙 추가
  - `max_width = 100`
  - `use_small_heuristics = "Max"`
  - `imports_granularity = "Crate"`

---

## [0.5.0] - 2026-01-31

### Added

#### 📒 매매일지 (Trading Journal) - 신규 기능
- **체결 내역 관리** (`routes/journal.rs`, `repository/journal.rs`)
  - 거래소 API에서 체결 내역 자동 동기화
  - 기간별 조회 (일별/주별/월별/전체)
  - 종목별/전략별 필터링
- **손익 분석 (PnL Analysis)**
  - 실현/미실현 손익 계산
  - 누적 손익 차트 (`PnLBarChart.tsx`)
  - 종목별 손익 분석 (`SymbolPnLTable.tsx`)
- **포지션 추적**
  - 보유 현황 대시보드 (`PositionsTable.tsx`)
  - 물타기 자동 계산 (평균 매입가 갱신)
  - 포지션 이력 조회
- **전략 인사이트** (`StrategyInsightsPanel.tsx`)
  - 전략별 성과 분석
  - 매매 패턴 분석 (빈도, 성공률, 평균 보유 기간)
- **DB 마이그레이션 6개 추가**
  - `015_trading_journal.sql`: 매매일지 기본 테이블
  - `016_positions_credential_id.sql`: 포지션-계정 연결
  - `017_journal_views.sql`: 분석용 뷰
  - `018_journal_period_views.sql`: 기간별 분석 뷰
  - `019_fix_cumulative_pnl_types.sql`: 타입 수정
  - `020_symbol_fundamental.sql`: 종목 기본정보

#### Repository 패턴 확장
- **JournalRepository** (`repository/journal.rs`, 993줄)
  - 체결 내역 CRUD
  - 손익 집계 쿼리
  - 기간별 통계 조회
- **KlinesRepository** (`repository/klines.rs`, 481줄)
  - OHLCV 데이터 접근 계층
  - 시계열 쿼리 최적화

#### 프론트엔드 컴포넌트
- **TradingJournal.tsx** (344줄): 매매일지 메인 페이지
- **SymbolDisplay.tsx** (203줄): 종목 표시 컴포넌트
- **PnLBarChart.tsx** (167줄): 손익 막대 차트
- **ExecutionsTable.tsx** (208줄): 체결 내역 테이블
- **PnLAnalysisPanel.tsx** (216줄): 손익 분석 패널
- **StrategyInsightsPanel.tsx** (242줄): 전략 인사이트 패널

#### 문서화
- **development_rules.md** (561줄): 개발 규칙 문서 신규 작성
  - Context7 API 검증 절차
  - unwrap() 안전 패턴
  - Repository 패턴 가이드
  - 전략 추가 체크리스트
- **prd.md**: PRD 문서 위치 이동 및 업데이트
- **docs/*.md**: 운영/배포/모니터링 문서 현행화

### Changed

#### 전략 개선
- **bollinger.rs**: 밴드 계산 로직 개선
- **grid.rs**: 그리드 간격 계산 최적화
- **rsi.rs**: RSI 신호 생성 로직 개선
- **volatility_breakout.rs**: 돌파 조건 정밀화

#### 백엔드 개선
- `routes/portfolio.rs`: 포트폴리오 조회 API 확장
- `repository/positions.rs`: 포지션 Repository 확장 (239줄 추가)
- `repository/orders.rs`: 주문 Repository 개선
- `main.rs`: Journal 라우트 등록

#### 프론트엔드 개선
- `App.tsx`: Trading Journal 라우트 추가
- `Layout.tsx`: 매매일지 메뉴 추가
- `client.ts`: Journal API 클라이언트 추가 (357줄 추가)
- `format.ts`: 포맷팅 유틸리티 확장 (80줄 추가)

#### KIS 거래소 연동
- `kis/auth.rs`: 인증 로직 개선 (40줄 변경)

### Database

- 마이그레이션 14개 → 20개 (6개 추가)
- 매매일지 관련 테이블 및 뷰 추가

---

## [0.4.4] - 2026-01-31

### Added

#### OpenAPI/Swagger 문서화
- **utoipa 통합**: REST API 자동 문서화
  - `openapi.rs`: OpenAPI 3.0 스펙 중앙 집계
  - Swagger UI (`/swagger-ui`) 경로에서 인터랙티브 문서 제공
  - 모든 주요 엔드포인트 태그 분류 (strategies, backtest, portfolio 등)
- **응답/요청 스키마**: ToSchema derive로 타입 자동 문서화
  - `HealthResponse`, `ComponentHealth`, `StrategiesListResponse` 등
  - 에러 응답 스키마 표준화 (`ApiError`)

#### 타입 안전성 강화
- **StrategyType enum** (`types/strategy_type.rs`): 전략 타입 열거형 추가
  - 26개 전략 타입 정의 (rsi_mean_reversion, grid, bollinger_bands 등)
  - serde 직렬화/역직렬화 지원
  - OpenAPI 스키마 자동 생성

#### 백테스트 API 개선
- **OpenAPI 어노테이션**: 백테스트 엔드포인트 문서화
  - `run_backtest`, `get_backtest_strategies` 등 핸들러
  - 요청/응답 타입 스키마 정의

### Changed

#### API 구조 개선
- `routes/mod.rs`: OpenAPI 스키마 타입 re-export
- `routes/health.rs`: 헬스 체크 OpenAPI 어노테이션 추가
- `routes/strategies.rs`: 전략 목록 API 문서화
- `routes/credentials/types.rs`: 자격증명 타입 OpenAPI 스키마

#### 거래소 커넥터
- `binance.rs`: 타임아웃 설정 개선
- `kis/config.rs`: 설정 타입 강화

### Dependencies

#### 신규 추가
- `utoipa = "5.3"`: OpenAPI 스키마 생성
- `utoipa-swagger-ui = "9.0"`: Swagger UI 서빙
- `utoipa-axum = "0.2"`: Axum 라우터 통합

---

## [0.4.3] - 2026-01-31

### Added

#### 통합 에러 핸들링 시스템
- **ApiErrorResponse** (`error.rs`): 모든 API 엔드포인트의 에러 응답 통합
  - 일관된 에러 코드, 메시지, 타임스탬프 제공
  - 기존 분산된 에러 타입들 통합 (strategies, backtest, simulation, ml)
  - 에러 상세 정보 및 요청 컨텍스트 포함

#### Repository 패턴 확장
- **신규 Repository 모듈 5개 추가**:
  - `repository/portfolio.rs`: 포트폴리오 데이터 접근
  - `repository/orders.rs`: 주문 이력 관리
  - `repository/positions.rs`: 포지션 데이터 관리
  - `repository/equity_history.rs`: 자산 이력 조회
  - `repository/backtest_results.rs`: 백테스트 결과 저장/조회

#### 프론트엔드 컴포넌트 분리
- **AddStrategyModal.tsx**: 전략 추가 모달 분리
- **EditStrategyModal.tsx**: 전략 편집 모달 분리
- **SymbolPanel.tsx**: 심볼 패널 컴포넌트
- **format.ts**: 포맷팅 유틸리티
- **indicators.ts**: 기술적 지표 계산 유틸리티

### Changed

#### 대형 파일 모듈화
- **analytics.rs (2,678줄) → 7개 모듈로 분리**:
  ```
  routes/analytics/
  ├── mod.rs        (라우터)
  ├── charts.rs     (차트 데이터)
  ├── indicators.rs (지표 계산)
  ├── manager.rs    (매니저)
  ├── performance.rs(성과 분석)
  ├── sync.rs       (동기화)
  └── types.rs      (타입 정의)
  ```

- **credentials.rs (1,615줄) → 5개 모듈로 분리**:
  ```
  routes/credentials/
  ├── mod.rs           (라우터)
  ├── active_account.rs(활성 계정)
  ├── exchange.rs      (거래소 자격증명)
  ├── telegram.rs      (텔레그램 설정)
  └── types.rs         (타입 정의)
  ```

- **Dataset.tsx, Strategies.tsx**: 컴포넌트 분리로 1,400+ 줄 감소

#### 모듈 재배치
- **trailing_stop.rs**: `trader-strategy` → `trader-risk` 크레이트로 이동
  - 리스크 관리 로직의 올바른 위치 배치

#### 인프라 개선
- **Docker → Podman 마이그레이션 지원**
  - README.md: Podman 설치 및 사용법 추가
  - docker-compose.yml: Podman 호환 주석 추가
  - 명령어 매핑 테이블 제공

### Improved

#### 코드 품질
- 에러 처리 일관성 향상 (unwrap() 사용 감소)
- 모듈별 관심사 분리로 유지보수성 향상
- Repository 패턴으로 데이터 접근 계층 표준화

---

## [0.4.2] - 2026-01-31

### Fixed

#### 다중 자산 전략 심볼 비교 버그 수정
- **심볼 비교 로직 통일**: `data.symbol.base.clone()` → `data.symbol.to_string()`
  - 영향 받은 전략 (10개):
    - `all_weather.rs`: All Weather 포트폴리오
    - `baa.rs`: Bold Asset Allocation
    - `dual_momentum.rs`: Dual Momentum
    - `kosdaq_fire_rain.rs`: 코스닥 불비
    - `kospi_bothside.rs`: KOSPI 양방향
    - `market_cap_top.rs`: 시가총액 상위
    - `sector_momentum.rs`: 섹터 모멘텀
    - `sector_vb.rs`: 섹터 변동성 돌파
    - `snow.rs`: Snow 전략
    - `us_3x_leverage.rs`: 미국 3배 레버리지

#### 백테스트 가격 매칭 버그 수정
- **다중 자산 가격 데이터 매칭**: `engine.rs`에서 현재 심볼에 맞는 가격 데이터만 필터링
- 이전: 모든 심볼 데이터에서 첫 번째 데이터 사용 (잘못된 가격)
- 이후: 심볼별 정확한 가격 데이터 매칭

### Added

#### 전략 통합 테스트
- **strategy_integration.rs**: 28개 전략 통합 테스트 (1,753줄)
  - 모든 백테스트 대상 전략 자동 검증
  - 다중 심볼 전략 테스트 커버리지 추가
  - 실행 시간: ~15분 (병렬 실행)

#### 차트 컴포넌트
- **SyncedChartPanel.tsx**: 동기화된 차트 패널 개선
  - 다중 심볼 동시 표시 지원
  - 줌/팬 동기화 기능

### Changed

#### 프론트엔드
- `Backtest.tsx`: 다중 자산 전략 결과 표시 개선
- `Simulation.tsx`: 전략 선택 UI/UX 개선
- `Strategies.tsx`: 전략 목록 필터링 및 정렬 개선
- `client.ts`: API 클라이언트 타입 안전성 강화

#### 백엔드
- `backtest/engine.rs`: 다중 자산 백테스트 엔진 로직 개선
- `backtest/loader.rs`: 데이터 로딩 최적화
- `strategies.rs`: 전략 Repository 쿼리 개선
- `simulation.rs`: 시뮬레이션 라우트 리팩토링

---

## [0.4.1] - 2026-01-31

### Added

#### SDUI (Server-Driven UI) 전략 스키마
- **전략 UI 스키마** (`config/sdui/strategy_schemas.json`)
  - 27개 전략별 동적 폼 스키마 정의
  - 필드 타입, 검증 규칙, 기본값 포함
  - 프론트엔드에서 서버 스키마 기반 동적 폼 렌더링

#### 유틸리티 모듈 (`utils/`)
- `format.rs`: 숫자, 날짜, 통화 포맷팅 함수
- `response.rs`: API 응답 헬퍼 (성공/에러 응답 표준화)
- `serde_helpers.rs`: Serde 직렬화 헬퍼 함수

#### 전략 기본값
- **defaults.rs**: 전략별 기본 파라미터 정의
- 신규 전략 생성 시 합리적인 기본값 제공

#### 심볼 검색 컴포넌트
- **SymbolSearch.tsx**: 실시간 심볼 검색 UI
- 자동완성, 최근 검색 기록, 시장 필터

#### E2E 테스트
- **risk-management-ui.spec.ts**: 리스크 관리 UI Playwright 테스트
- **playwright.config.ts**: E2E 테스트 설정
- **regression_baseline.json**: 회귀 테스트 베이스라인

#### DB 마이그레이션
- `014_strategy_risk_capital.sql`: 전략 리스크/자본 설정 컬럼 추가

### Changed

#### 백테스트 모듈 리팩토링
- **모듈 분리**: `backtest.rs` (3,854줄) → 5개 모듈로 분리
  - `backtest/mod.rs`: 라우터 및 핸들러
  - `backtest/engine.rs`: 백테스트 실행 엔진
  - `backtest/loader.rs`: 데이터 로더
  - `backtest/types.rs`: 타입 정의
  - `backtest/ui_schema.rs`: UI 스키마 생성
- 코드 가독성 및 유지보수성 향상

#### 프론트엔드 개선
- `Backtest.tsx`: SDUI 스키마 기반 동적 폼 통합
- `Simulation.tsx`: 심볼 검색 컴포넌트 통합
- `Strategies.tsx`: 전략 생성/편집 UI 개선
- `DynamicForm.tsx`: 스키마 기반 폼 렌더링 개선

#### API 개선
- `strategies.rs`: 전략 CRUD API 확장 (리스크/자본 설정)
- `equity_history.rs`: N+1 쿼리 최적화 (배치 쿼리)

---

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

### [0.6.0] - 예정
- 추가 거래소 통합 (Coinbase, 키움증권)
- WebSocket 이벤트 브로드캐스트 완성
- 성능 최적화 및 부하 테스트

### [0.7.0] - 예정
- 실시간 알림 대시보드
- 포트폴리오 리밸런싱 자동화
- 다중 계좌 지원
