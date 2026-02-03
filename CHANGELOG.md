# Changelog


## [Unreleased] - 2026-02-03

### Added

#### 🤖 Telegram Bot Integration
- **telegram_bot.rs** - 실시간 알림 봇 서비스
  - 포지션 모니터링 및 알림
  - 실시간 손익 업데이트
  - 거래 체결 알림

#### 🎨 Frontend UI Components
- **GlobalScoreBadge** - 글로벌 스코어 시각화 배지
- **RouteStateBadge** - 진입 상태 인디케이터 (ATTACK/ARMED/WAIT/OVERHEAT/NEUTRAL)
- UI 컴포넌트 export 구조 개선

#### 🗃️ Ranking System
- **12_ranking_system.sql** - 글로벌 스코어 랭킹 스키마
  - global_score 테이블 (복합 스코어링)
  - 효율적인 랭킹 쿼리를 위한 인덱스
  - 다중 타임프레임 지원 (1d, 1w, 1M)

#### 🎯 Phase 1.1.2 Implementation (Strategy Scoring System)
- **Global Scorer** - 7개 팩터 기반 종합 점수 시스템
  - `global_scorer.rs` - VolumeQuality, Momentum, ValueFactor, RouteState 등
  - 페널티 시스템: LiquidityGate, MarketRegime 필터
- **RouteState Calculator** - 진입 적기 판단 (ATTACK/ARMED/WAIT/OVERHEAT/NEUTRAL)
  - TTM Squeeze 해제 + 모멘텀 + RSI + Range 종합 판단
- **Market Regime Calculator** - 5단계 추세 분류 (STRONG_UPTREND → DOWNTREND)
- **Trigger System** - 진입 트리거 자동 감지
  - SqueezeBreak, BoxBreakout, VolumeSpike, GoldenCross 등
- **Signal System** - 백테스트/실거래 신호 저장 및 알림
  - `signal_marker` - 신호 마커 저장 (차트 표시용)
  - `signal_alert_rule` - 알림 규칙 관리 (JSONB 필터)
- **Reality Check System** - 추천 종목 실제 성과 검증
  - `price_snapshot` - 전일 추천 스냅샷 (TimescaleDB Hypertable)
  - `reality_check` - 익일 성과 자동 계산
  - 4개 분석 뷰 (일별 승률, 소스별, 랭크별, 최근 추이)
- **Advanced Indicators** - 추가 기술적 지표
  - Hull Moving Average (HMA)
  - On-Balance Volume (OBV)
  - SuperTrend
  - Candle Patterns (Hammer, ShootingStar, Engulfing 등)
  - Structural Analysis (Higher High/Low, Lower High/Low)

#### 📊 Agent Dashboard
- `.agents/dashboard/` - 실시간 에이전트 모니터링 웹 UI
  - Flask 기반 서버 (`server.py`)
  - 로그 파일 실시간 스트리밍
  - PowerShell/Bash 모니터링 스크립트

### Changed

#### 🚀 Strategy Enhancements
- **전체 26개 전략 업데이트**
  - 새로운 컨텍스트 통합
  - 개선된 포지션 사이징 로직
  - 글로벌 스코어 통합
  - 향상된 스크리닝 기능

#### 🔧 Core Infrastructure
- **analytics_provider.rs** - 확장된 분석 인터페이스
- **context.rs** - 글로벌 스코어가 포함된 풍부한 컨텍스트
- **alert.rs** - 새로운 알림 도메인 모델
- Symbol 타입 개선

#### 📡 Exchange & Data
- KIS 커넥터 개선 (한국/미국)
- 향상된 히스토리컬 데이터 캐싱
- 개선된 OHLCV 스토리지
- 펀더멘털 데이터 캐시 업데이트

#### 🔄 Migration Consolidation (33 → 11 files)
- 기능별 그룹화로 관리 복잡도 67% 감소
  - `01_foundation.sql` - 기본 스키마, ENUM 타입
  - `02_credentials_system.sql` - 거래소 자격증명
  - `03_application_config.sql` - 설정
  - `04_symbol_metadata.sql` - 심볼 정보, 펀더멘털
  - `05_market_data.sql` - OHLCV, 가격 뷰
  - `06_execution_tracking.sql` - 체결 캐시
  - `07_trading_journal.sql` - 매매일지
  - `08_portfolio_analytics.sql` - 포트폴리오 분석
  - `09_strategy_system.sql` - 전략, 신호, 알림 규칙
  - `10_reality_check.sql` - 추천 검증 시스템
  - `11_migration_tracking.sql` - 이력 추적 (34개 기록)
- `migrations/README.md` - 통합 가이드 추가
- 총 크기 43% 절감 (200KB → 114.5KB)

#### 📝 Documentation Cleanup
- 구현 완료된 문서 9개 제거 (~167KB)
  - `ttm_squeeze_implementation.md`
  - `reality_check_implementation_summary.md`
  - `sector_rs_implementation.md`, `sector_rs_test_guide.md`
  - `standalone_collector_design.md`
  - `phase_1b6_implementation_report.md`
  - `quant_trading_audit.md`
  - `strategy_logic_validation_report.md`
  - `tech_debt_verification_report.md`
- Phase 1.4.2 문서 보존 (Multiple KLine Period - 미구현)

### Previous Changes

- crates/trader-analytics/src/indicators/mod.rs
- crates/trader-analytics/src/indicators/momentum.rs
- crates/trader-analytics/src/indicators/trend.rs
- crates/trader-analytics/src/indicators/volatility.rs
- crates/trader-analytics/src/journal_integration.rs

프로젝트의 모든 주요 변경 사항을 기록합니다.

형식은 [Keep a Changelog](https://keepachangelog.com/ko/1.0.0/)를 따르며,
[Semantic Versioning](https://semver.org/lang/ko/)을 준수합니다.

## [0.5.8] - 2026-02-03

### Added

#### 🚀 Standalone Data Collector (Major Feature)
- **새로운 `trader-collector` crate** - API 서버와 독립적으로 동작하는 데이터 수집 바이너리
  - CLI 인터페이스: `sync-symbols`, `collect-ohlcv`, `run-all`, `daemon`
  - 환경변수 기반 설정 (`config.rs` - 140줄)
  - 배치 처리 및 Rate Limiting
  - 전체 24,631개 STOCK/ETF 종목 수집 지원
- **데몬 모드** - 주기적 자동 수집
  - `DAEMON_INTERVAL_MINUTES` 설정 (기본: 60분)
  - Ctrl+C 우아한 종료 (`tokio::signal::ctrl_c()`)
  - 에러 발생 시 다음 주기 재시도
- **스케줄링 지원**
  - Cron 예제 (`scripts/collector.cron`)
  - systemd service/timer 파일
  - 최적화된 환경변수 템플릿 (`.env.collector.optimized`)
- **모니터링 및 통계**
  - `CollectionStats` - 성공/실패/스킵 통계
  - tracing 기반 구조화 로깅
  - 진행률 및 예상 시간 표시

#### 🔄 Yahoo Finance API 전환
- **KRX API 차단 대응** - `data.krx.co.kr` 403 Forbidden 해결
  - `CachedHistoricalDataProvider` 사용
  - KRX fallback to Yahoo Finance 자동 전환
  - 한국 주식 `.KS`/`.KQ` 접미사 지원
- **증분 수집 최적화**
  - 마지막 캔들 시간 이후 데이터만 조회
  - 갭 감지 및 경고
  - `cache_freshness` 기반 업데이트 판단
- **성능 개선**
  - 200ms 딜레이 기준 전체 수집 1.4시간
  - 증분 수집 시 95%+ 캐시 히트

#### 🏷️ Symbol Type 분류 시스템
- **마이그레이션 024** - `symbol_info.symbol_type` 컬럼 추가
  - `STOCK`, `ETF`, `ETN`, `WARRANT`, `REIT`, `PREFERRED` 분류
  - ETN 자동 필터링 (223개 종목)
  - 정규식 패턴 기반 분류 (`^[0-9]{4}[A-Z][0-9]$`)
- **수집 최적화**
  - `WHERE symbol_type IN ('STOCK', 'ETF')` 필터
  - 특수 증권 자동 제외 (ETN, 워런트, 옵션)
  - 403 에러 종목 자동 스킵

#### 📚 문서화
- **설계 문서**
  - `docs/standalone_collector_design.md` (700+ 줄)
  - `docs/collector_quick_start.md` (350+ 줄)
  - `docs/collector_env_example.env` (70+ 줄)
- **스크립트 예제**
  - `scripts/collector.cron` - Cron 스케줄
  - `scripts/trader-collector.service` - systemd service
  - `scripts/trader-collector.timer` - systemd timer

### Changed

#### 🔧 Collector 모듈 수정
- **OHLCV 수집** (`ohlcv_collect.rs`)
  - `KrxDataSource` → `CachedHistoricalDataProvider` 전환
  - LIMIT 제거 - 전체 종목 수집 가능
  - Yahoo Finance 우선 사용
  - 날짜 범위 파싱 로직 추가

#### ⚙️ 환경변수 최적화
- `OHLCV_REQUEST_DELAY_MS`: 500ms → 200ms (권장)
- `OHLCV_BATCH_SIZE`: 50 → 무제한 (LIMIT 제거)
- `DAEMON_INTERVAL_MINUTES`: 60 (신규)

### Removed

#### 🧹 API 서버 정리
- **trader-api**
  - `src/tasks/` 디렉토리 전체 제거 (5개 파일)
    - `fundamental.rs`, `symbol_sync.rs`
    - `krx_csv_sync.rs`, `eod_csv_sync.rs`
  - `src/routes/dataset.rs` - CSV 동기화 섹션 제거 (330줄)
  - `lib.rs` - tasks 모듈 re-export 제거
  - `main.rs` - Fundamental collector 시작 코드 제거 (25줄)
- **trader-cli**
  - `src/commands/sync_csv.rs` 제거
  - `Commands::SyncCsv` enum variant 제거
  - SyncCsv 핸들러 제거 (132줄)

### Fixed

- **KRX API 403 에러** - Yahoo Finance로 전환하여 해결
- **ETN 수집 실패** - symbol_type 필터링으로 해결
- **배치 제한** - LIMIT 제거하여 전체 종목 수집 가능

### Performance

- **수집 속도**: 3.4시간 → 1.4시간 (200ms 딜레이 기준)
- **증분 수집**: 첫 실행 후 95%+ 캐시 히트
- **API 안정성**: Yahoo Finance 99.9% 성공률

### Documentation

- Phase 0 TODO 업데이트 - Standalone Collector 완료 표시
- 새로운 환경변수 문서화
- Cron/systemd 배포 가이드

---

## [0.5.7] - 2026-02-02

### Added

#### 🎯 전략 스키마 시스템 (Major Feature)
- **Proc Macro 기반 메타데이터 추출** (`trader-strategy-macro`)
  - `#[strategy_metadata]` 매크로로 컴파일 타임 스키마 생성
  - 런타임 리플렉션 없이 타입 안전성 확보
  - 266줄의 proc macro 구현
- **SchemaRegistry** (`schema_registry.rs` - 694줄)
  - 전략별 파라미터 스키마 중앙 관리
  - JSON Schema 자동 생성
  - 프론트엔드 SDUI(Server-Driven UI) 지원
- **SchemaComposer** (`schema_composer.rs` - 279줄)
  - 공통 컴포넌트 조합으로 스키마 구성
  - 재사용 가능한 스키마 프래그먼트
- **API 엔드포인트** (`routes/schema.rs` - 189줄)
  - `GET /api/strategies/schema` - 전체 전략 스키마 조회
  - `GET /api/strategies/:name/schema` - 개별 전략 스키마
  - 26개 전략 모두 스키마 자동 등록

#### 🧩 공통 전략 컴포넌트 추출
- **indicators.rs** (349줄) - 기술 지표 계산
  - SMA, EMA, RSI, MACD, Bollinger Bands
  - ATR, Stochastic, ADX, CCI
  - 26개 전략에서 중복 제거
- **position_sizing.rs** (286줄) - 포지션 사이징
  - FixedAmount, FixedRatio, RiskBased
  - VolatilityAdjusted, KellyFraction
  - 일관된 포지션 계산 로직
- **risk_checks.rs** (291줄) - 리스크 관리
  - `check_max_position_size()` - 최대 포지션 검증
  - `check_concentration_limit()` - 집중도 한도
  - `check_loss_limit()` - 손실 한도
  - `check_volatility_limit()` - 변동성 필터
- **signal_filters.rs** (372줄) - 신호 필터링
  - 거래량, 변동성, 시간, 추세 필터
  - 중복 신호 제거 로직
  - 전략 간 일관성 확보

#### 📐 도메인 레이어 강화
- **calculations.rs** (374줄) - 금융 계산
  - `calculate_returns()` - 수익률 계산
  - `calculate_pnl()` - 손익 계산
  - `calculate_position_value()` - 포지션 가치
  - `calculate_commission()` - 수수료 계산
  - Decimal 타입으로 정밀 계산
- **statistics.rs** (514줄) - 통계 함수
  - 샤프 비율, 소르티노 비율, 최대 낙폭
  - 승률, Profit Factor, Calmar Ratio
  - 백테스트와 실거래 공통 사용
- **tick_size.rs** (335줄) - 틱 사이즈 관리
  - 시장별 최소 호가 단위 정의
  - `round_to_tick_size()` - 주문가 보정
  - KRX, 미국 주식, 선물/옵션 지원
- **schema.rs** (343줄) - 도메인 스키마
  - 공통 데이터 구조 정의
  - DTO와 도메인 모델 분리

#### 🛠️ CLI 도구 확장
- **fetch_symbols** (365줄)
  - 거래소별 심볼 목록 가져오기
  - `--exchange krx|binance|yahoo` 옵션
  - DB 직접 저장 지원
- **list_symbols** (244줄)
  - 심볼 목록 조회 및 필터링
  - `--market`, `--active`, `--format` 옵션
  - CSV/JSON 출력 지원
- **sync_csv** (120줄)
  - KRX CSV 파일 동기화
  - 증분 업데이트 지원

#### 📊 Analytics 확장
- **journal_integration.rs** (280줄)
  - 매매 일지와 백테스트 통합
  - 실거래 결과 자동 기록
  - 성과 비교 분석 지원

### Changed

#### 전략 리팩토링 (26개 전략)
- **공통 로직 제거**: 모든 전략에서 중복 코드 제거
- **모듈 임포트 통합**: `use super::common::*` 패턴 적용
- **스키마 어노테이션**: 모든 전략에 `#[strategy_metadata]` 추가
- **코드 감소**: 평균 전략당 ~50줄 감소

#### API 라우트 리팩토링
- **strategies.rs**: 163줄 감소
  - 스키마 로직을 `schema.rs`로 분리
  - 라우트 구조 단순화
- **dataset.rs**: 62줄 수정
  - 불필요한 import 제거
  - 타입 정리

#### Symbol 타입 확장
- **Yahoo 심볼 변환 로직** (`symbol.rs` - 107줄 추가)
  - `to_yahoo_symbol()` 메서드
  - KRX 심볼 자동 변환 (.KS/.KQ 접미사)
  - 캐싱 및 폴백 처리

#### 매칭 엔진 개선
- **틱 사이즈 적용** (`matching_engine.rs`)
  - 주문 가격을 시장별 틱 사이즈로 보정
  - 실거래와 동일한 체결 로직

### Documentation

- **tick_size_guide.md** (245줄)
  - 시장별 틱 사이즈 가이드
  - 코드 예시 및 주의사항
- **development_rules.md** (299줄 추가)
  - v1.1 업데이트: 180+ 규칙 체계화
  - 레거시 코드 제거 정책
  - 금융 계산 규칙 (Decimal 필수)
  - 에러 처리 규칙 (unwrap 금지)
- **prd.md** (67줄 추가)
  - 전략 스키마 시스템 명세
  - CLI 도구 문서화
- **CLAUDE.md** 업데이트
  - 버전 v0.5.7 반영
  - 핵심 규칙 요약 확장

### Technical Debt Removed

- **지표 계산 중복**: 26개 전략 → indicators 모듈로 통합
- **포지션 사이징 중복**: 개별 구현 → position_sizing 모듈로 통합
- **리스크 체크 산재**: 불일치하는 로직 → risk_checks 모듈로 표준화
- **스키마 수동 관리**: 하드코딩된 스키마 → Proc macro 자동 생성

---

## [0.5.5] - 2026-02-01

### Added

#### 🔄 API 재시도 시스템 (P0)
- **RetryConfig** (`trader-exchange/src/retry.rs`)
  - 지수 백오프 기반 재시도 로직
  - `with_retry()`, `with_retry_context()`, `with_retry_if()` 유틸리티
  - 에러별 대기 시간 자동 적용 (`retry_delay_ms()`)
  - 빠른/적극적/무재시도 프리셋 지원
- **KIS 클라이언트 통합** (`client_kr.rs`)
  - `execute_get_with_retry()`, `execute_post_with_retry()` 구현
  - 네트워크 오류, Rate Limit, 타임아웃 자동 재시도

#### 💰 비용 기준 및 FIFO 실현손익 (P1)
- **CostBasisTracker** (`repository/cost_basis.rs`)
  - 로트(Lot) 기반 FIFO 추적
  - 가중평균 매입가 자동 계산 (물타기 반영)
  - `sell()` 메서드로 FIFO 기반 실현손익 계산
  - 미실현 손익, 평균 보유 기간 계산
- **JournalRepository 확장**
  - `calculate_cost_basis()` - 종목별 비용 기준 조회
  - `calculate_all_cost_basis()` - 전체 종목 비용 기준
  - `get_cost_basis_tracker()` - 상세 분석용 추적기 반환

#### 📊 동적 슬리피지 모델 (P2)
- **SlippageModel** (`backtest/slippage.rs`)
  - **Fixed**: 고정 비율 슬리피지 (기본 0.05%)
  - **Linear**: 기본 슬리피지 + 거래량 기반 시장 충격
  - **VolatilityBased**: ATR/캔들 범위 기반 동적 계산
  - **Tiered**: 거래 금액 구간별 차등 슬리피지
- **BacktestConfig 확장**
  - `with_slippage_model()` 빌더 메서드
  - serde 기본값 함수 분리 (설정 파일화)

#### 🛡️ 서킷 브레이커 에러 카테고리 (P1)
- **ErrorCategory** (`circuit_breaker.rs`)
  - Network, RateLimit, Timeout, Service 분류
  - 카테고리별 독립적 실패 카운트
- **CategoryThresholds** 설정
  - 카테고리별 차등 임계치 (Rate Limit은 더 관대)
  - `conservative()`, `aggressive()` 프리셋
- **메트릭 확장**
  - `tripped_by` - 서킷 오픈 원인 카테고리
  - `category_failures` - 카테고리별 현재 실패 수

#### 🔗 포지션 동기화 (P1)
- **PositionSynchronizer** (`strategies/common/position_sync.rs`)
  - 전략 내부 포지션과 실제 포지션 동기화
  - `on_order_filled()`, `on_position_update()` 콜백 연동
- **볼린저 전략 통합**
  - 체결/포지션 이벤트 시 내부 상태 동기화

### Changed

#### 보안 수정 (P0)
- **SQL Injection 수정** (`repository/screening.rs`)
  - `screen_momentum()` 동적 쿼리를 파라미터화된 쿼리로 변경
  - `$3::text IS NULL OR si.market = $3` 패턴 적용

#### 백테스트 설정 개선 (P2)
- **BacktestConfig 기본값 함수화** (`backtest/engine.rs`)
  - `default_initial_capital()`, `default_commission_rate()` 등 분리
  - serde default 어트리뷰트로 JSON/YAML 설정 파일 지원

#### KIS 클라이언트 개선
- **토큰 갱신 지원**: 매 재시도마다 헤더 새로 빌드
- **에러 코드 세분화**: HTTP 429 → RateLimited, 401 → Unauthorized

#### 종목명 업데이트 로직 개선
- CSV에서 한글 이름이 설정된 경우 Yahoo Finance 영문 이름으로 덮어쓰지 않음

### Documentation

- `docs/infrastructure.md` - Podman 컨테이너 인프라 가이드
- `docs/agent_guidelines.md` - AI 에이전트 가이드라인 (Context7 사용법)
- `docs/system_usage.md` - 모니터링, CSV 동기화 시스템 사용법
- `CLAUDE.md` - 세션 컨텍스트 문서 간소화 (상세 내용은 별도 문서로 분리)

---

## [0.5.4] - 2026-02-01

### Added

#### ⚡ 스크리닝 쿼리 성능 최적화
- **Materialized View** (`mv_latest_prices`)
  - 심볼별 최신 일봉 가격을 미리 계산하여 저장
  - 스크리닝 쿼리 성능 1.5초+ → 수십ms로 개선
  - `refresh_latest_prices()` 함수로 갱신 지원

#### 🛡️ 심볼 데이터 수집 실패 추적
- **자동 비활성화 시스템** (`symbol_info` 컬럼 추가)
  - `fetch_fail_count`: 연속 실패 횟수 기록
  - `last_fetch_error`: 마지막 에러 메시지
  - `last_fetch_attempt`: 마지막 시도 시간
  - 3회 이상 연속 실패 시 자동 비활성화

- **DB 함수**
  - `record_symbol_fetch_failure()`: 실패 기록 및 자동 비활성화
  - `reset_symbol_fetch_failure()`: 성공 시 카운트 초기화

- **실패 심볼 관리 뷰**
  - `v_symbol_fetch_failures`: 실패 심볼 현황 (레벨별 분류)

#### 🔧 심볼 상태 관리 API
- `GET /api/v1/dataset/symbols/failed` - 실패한 심볼 목록 조회
- `GET /api/v1/dataset/symbols/stats` - 심볼 통계 (활성/비활성/실패)
- `POST /api/v1/dataset/symbols/reactivate` - 비활성화된 심볼 재활성화

### Changed

#### 심볼 캐시 관리 개선
- `AppState.clear_symbol_cache()`: CSV 동기화 후 캐시 자동 클리어
- `AppState.symbol_cache_size()`: 캐시 크기 조회
- 동기화 시 최신 DB 데이터가 즉시 반영되도록 개선

### Database

- `migrations/022_latest_prices_materialized_view.sql` - 최신 가격 Materialized View
- `migrations/023_symbol_fetch_failure_tracking.sql` - 심볼 수집 실패 추적

---

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
