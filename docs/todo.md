
# 작업 규칙
- Context7과 Sequential Thinking, Shrimp Task Manager를 적극적으로 사용하세요.
- 모든 작업 수행시 UI와 API의 필드 매칭을 무조건 맞추고 진행 하세요.
- API는 무조건 호출하여 정상작동 하는지 테스트 합니다. 문제가 발생했을 때 수정 후 넘어가세요.
- UI는 playwright를 이용하여 항상 동작 확인을 수행합니다. 적당한 형태의 테스트 케이스를 만들어, 통과하도록 하세요.
- UI와 API가 모두 끝나야 작업이 끝나는 것입니다. API와 UI 테스트 도중 문제가 생기면 바로바로 해결하세요.
- docker 환경에서 반드시 테스트 할 것. 실제 환경은 docker를 사용합니다.
- 작업의 완료는 확인 해야할 모든 요소가 정상적일때 완료라고 합니다. 확인 해야 할 요소는 API, 구조, UI입니다.
---

# 코드베이스 검증 결과 (2026-01-30)

## 전체 통계
| 모듈 | 코드량 | 파일수 | 완료도 | 테스트 |
|------|--------|--------|--------|--------|
| Backend API Routes | 15,243줄 | 17개 | 95% | 부분 |
| Frontend Pages | 7,044줄 | 7개 | 95%+ | - |
| Frontend Components | 4,000+줄 | 15+개 | 100% | - |
| Strategies | 12,885줄 | 18개 | 100% | 107개 |
| Analytics/Backtest | 10,979줄 | 15개 | 95% | 108개 |
| Exchange Connectors | 11,025줄 | 24개 | 85-95% | 부분 |
| ML Module | 4,125줄 | 7개 | 95% | 43개 |
| Migrations | - | 12개 | 100% | - |

---

# 완료된 작업 (2026-01-30)

## PRD v2.0 재작성 및 코드베이스 검증
- [x] 전체 코드베이스 6개 서브에이전트 분석
- [x] Backend API Routes 분석 (15,243줄, 70+ 엔드포인트)
- [x] Frontend 전체 분석 (7,044줄 페이지 + 4,000줄 컴포넌트)
- [x] 전략 모듈 분석 (18개 전략, 107개 테스트)
- [x] 백테스트/분석 모듈 분석 (10,979줄, 108개 테스트)
- [x] 거래소 연동 모듈 분석 (Binance, KIS KR/US)
- [x] 마이그레이션 분석 (10개, 25+ 테이블)
- [x] PRD v2.0 업데이트 → synthetic-conjuring-peach.md

## [완료] Frontend 전체 ✅
- [x] Dashboard.tsx (603줄) - 포트폴리오, 실시간 알림
- [x] Backtest.tsx (849줄) - 단일/다중 백테스트, 결과 저장
- [x] Strategies.tsx (1,066줄) - SDUI 동적 폼, CRUD
- [x] Simulation.tsx (646줄) - 실시간 시뮬레이션
- [x] MLTraining.tsx (604줄) - 4가지 모델 훈련
- [x] Settings.tsx (1,383줄) - 거래소/텔레그램 설정
- [x] Dataset.tsx (1,893줄) - 차트, 지표, 무한 스크롤

## [완료] Frontend 컴포넌트 ✅
- [x] DynamicForm.tsx (723줄) - SDUI 폼 렌더러
- [x] Toast.tsx (183줄) - 알림 시스템
- [x] Layout.tsx (127줄) - 네비게이션
- [x] MultiPanelGrid.tsx (357줄) - 멀티 패널 레이아웃
- [x] 차트 컴포넌트 8개 (3,771줄) - 캔들, 자산곡선, 드로다운 등

## [완료] Backend API Routes ✅ (17개 파일, 15,243줄)
- [x] health.rs (237줄) - liveness/readiness probe
- [x] backtest.rs (3,323줄) - 백테스트 실행
- [x] backtest_results.rs (514줄) - 결과 저장/조회
- [x] strategies.rs (788줄) - 전략 CRUD
- [x] orders.rs (531줄) - 주문 관리
- [x] positions.rs (314줄) - 포지션 조회
- [x] portfolio.rs (893줄) - KIS 통합 잔고 조회
- [x] simulation.rs (868줄) - 시뮬레이션 모드
- [x] analytics.rs (2,325줄) - 성과 분석, 기술 지표
- [x] credentials.rs (1,615줄) - AES-256-GCM 암호화
- [x] notifications.rs (627줄) - 텔레그램 설정
- [x] ml.rs (606줄) - ML 훈련 관리
- [x] patterns.rs (496줄) - 패턴 인식
- [x] equity_history.rs (618줄) - 자산 곡선
- [x] dataset.rs (642줄) - 데이터셋 관리
- [x] market.rs (765줄) - 시장 상태

## [완료] 전략 모듈 ✅ (18개 전략, 12,885줄, 107개 테스트)

### 단일 자산 전략 (9개)
- [x] grid.rs (914줄) - 그리드 트레이딩
- [x] rsi.rs (932줄) - RSI 평균회귀
- [x] bollinger.rs (696줄) - 볼린저 밴드
- [x] volatility_breakout.rs (777줄) - 변동성 돌파
- [x] magic_split.rs (776줄) - 분할 매수
- [x] sma.rs (354줄) - 이동평균 크로스오버
- [x] trailing_stop.rs (489줄) - 트레일링 스탑
- [x] candle_pattern.rs (958줄) - 캔들 패턴 35종
- [x] infinity_bot.rs (674줄) - 무한매수봇

### 자산배분 전략 (9개)
- [x] simple_power.rs (758줄) - Simple Power
- [x] haa.rs (917줄) - HAA 계층적 자산배분
- [x] xaa.rs (1,103줄) - XAA 확장 자산배분
- [x] all_weather.rs (666줄) - 올웨더
- [x] snow.rs (531줄) - Snow 모멘텀
- [x] stock_rotation.rs (868줄) - 종목 갈아타기
- [x] market_cap_top.rs (707줄) - 시총 TOP N
- [x] market_interest_day.rs (698줄) - 시장관심 단타

### 미구현 전략 (기존 PRD 28개 중 13개 미구현)

**참고: Python 전략 폴더 인덱스**
- 75번 이후: 아이디어만 있음 (Python 코드 없음)
- 75번 이하: Python 코드 있음 (변환/참고 가능)

**시장 구분 설명**
- KR 또는 US: 해당 시장의 ETF를 대상으로 각각 운용 (시장 선택 필수)
- KR+US (복합): 두 시장의 자산을 동시에 운용하는 전략

**2차 구현 대상 (Python 코드 있음) - ✅ 모두 완료:**
- [x] Stock Gugan (41) - KR 또는 US, 주식 구간 매매 ✅
- [x] KOSDAQ Fire Rain (37) - KR, 코스닥 급등주 ✅
- [x] Sector VB (30) - KR, 섹터 변동성 돌파 ✅
- [x] US 3X Leverage (28) - US, 3배 레버리지 ETF ✅
- [x] KOSPI BothSide (26) - KR, 양방향 ✅

**3차 구현 대상 (Python 코드 있음):**
- [ ] SPAC No-Loss (56) - KR, 스팩 무손실
- [ ] All at Once ETF (14) - KR, ETF 일괄 투자
- [ ] Small Cap Quant (11) - KR, 소형주 퀀트
- [ ] BAA (7) - US, BAA 자산배분
- [ ] Rotation Savings (6) - KR, 회전 적금
- [ ] Sector Momentum (5) - KR+US, 섹터 모멘텀
- [ ] Pension Bot (3) - KR, 연금 자동화
- [ ] Dual KrStock UsBond (1) - KR+US, 한국주식+미국채권

**아이디어에서 직접 구현된 전략 (Python 코드 없었음):**
- [x] Stock Rotation (82) - 아이디어 → 직접 구현 ✅
- [x] Market Interest Day (75) - 아이디어 → 직접 구현 ✅

## [완료] ML 패턴 인식 모듈 ✅ (4,125줄, 43개 테스트)
- [x] pattern.rs (1,942줄) - 캔들스틱 26종 + 차트 패턴 22종
- [x] features.rs (636줄) - Feature Engineering 30개+ 지표
- [x] predictor.rs (476줄) - ONNX 추론 (GPU 가속)
- [x] service.rs (600줄) - 통합 ML 서비스

## [완료] 리스크 관리 모듈 ✅ (3,742줄, 7개 파일)
- [x] manager.rs (656줄) - 중앙 RiskManager, 검증 파이프라인
- [x] trailing_stop.rs (723줄) - 4가지 모드 (Fixed%, ATR, Step, Parabolic SAR)
- [x] stop_loss.rs (731줄) - Stop-Loss/Take-Profit 생성, 브라켓 주문
- [x] limits.rs (609줄) - 일일 손실 한도, UTC 자동 리셋, 70%/90% 경고
- [x] position_sizing.rs (586줄) - 포지션 사이징, Kelly Criterion
- [x] config.rs (398줄) - RiskConfig, 심볼별 설정 오버라이드

**구현된 기능:**
- [x] 스톱로스/테이크프로핏 - 자동 생성, ATR 기반, 브라켓 주문
- [x] 포지션 크기 제한 - 단일 포지션(10%), 총 노출(50%), 동시 포지션(10개)
- [x] 일일 손실 한도 - 기본 3%, UTC 자정 자동 리셋, 70%/90% 경고
- [x] 변동성 필터 - 5% 임계값 초과 시 거래 차단
- [x] 트레일링 스탑 - FixedPercentage, AtrBased, Step-Based, Parabolic SAR

**전략 레벨에서 구현된 패턴 (Python 전략에서 추출):**
- [x] 피라미드 분할 매수 - infinity_bot.rs (50라운드, 라운드별 진입 조건)
- [x] 물타기 (Water Dipping) - infinity_bot.rs (MA 변곡점 추가 매수)
- [x] 분할 매매 (Split Trading) - magic_split.rs (10차수 분할 매수/매도)

**🔴 [미구현] 전략별 리스크 설정 선택 ⭐ 필수**
현재: 실행 레이어에서 일괄 적용
요구사항: 각 전략마다 별도 리스크 모듈 선택 가능해야 함

### 기능 상세
1. **전략별 기본 리스크 모듈**
   - 각 전략은 생성 시 기본 리스크 설정이 포함됨
   - 예: Infinity Bot → 피라미드 분할 매수 + 50라운드 손절
   - 예: Grid Trading → 고정 그리드 스톱로스

2. **사용자 커스터마이징**
   - 기본 리스크 설정 수정 가능
   - 다른 리스크 모듈 선택 가능

3. **전략 복사 및 파생 전략 생성**
   - 등록된 전략 복사하여 새 전략 생성
   - 파라미터 및 리스크 설정 함께 복사
   - 파라미터 조작으로 새로운 파생 전략 생성
   - 예: "RSI 공격적" → RSI 전략 복사 + oversold=20, overbought=80

### 구현 작업
- [ ] 백엔드: StrategyConfig에 risk_config 필드 추가
- [ ] 백엔드: 전략별 기본 RiskConfig 정의
- [ ] 백엔드: 전략 복사 API (`POST /api/v1/strategies/{id}/clone`)
- [ ] 프론트: 전략 등록 시 리스크 설정 선택 UI
- [ ] 프론트: 전략 복사 버튼 및 모달
- [ ] DB: strategies 테이블에 risk_config JSON 컬럼 추가
- [ ] 수수료 조정 패턴 (profit * 0.8, loss * 1.2)
- [ ] 역추세 매매 (Reverse Trend) 모듈

## [완료] 전략 파라미터 튜닝 시스템 ✅
- [x] DynamicForm.tsx (723줄) - SDUI 폼 렌더러, JSON Schema 기반
- [x] strategies.rs API (788줄) - 전략 CRUD, 파라미터 저장
- [x] 대상 종목 (Symbol) 선택/입력
- [x] 기술적 지표 파라미터 (기간, 임계값 등)
- [x] 리스크 파라미터 (손절/익절 비율, 포지션 크기)
- [x] JSON Schema 기반 파라미터 스키마 정의
- [x] 파라미터 유효성 검증
- [x] 프리셋 저장/불러오기

## [완료] 백테스트/분석 모듈 ✅ (10,979줄, 108개 테스트)
- [x] 백테스트 엔진 - 슬리피지, 수수료, 포지션 관리
- [x] 성과 지표 14개 - Sharpe, Sortino, MDD, Calmar, 승률 등
- [x] 기술 지표 11개 - SMA, EMA, RSI, MACD, BB, ATR 등
- [x] 다중 자산 백테스트 지원

## [완료] 거래소 연동 ✅
- [x] Binance (95%) - REST API + WebSocket 구조
- [x] KIS 국내 (85%) - 시세, 주문, OAuth, WebSocket
- [x] KIS 해외 (80%) - 미국 주식, WebSocket
- [x] 시뮬레이션 (90%) - 매칭 엔진, 데이터 피드

## [완료] 데이터베이스 ✅ (12개 마이그레이션, 25+ 테이블)
- [x] TimescaleDB Hypertable 4개 (klines, trade_ticks, credential_access_logs, ohlcv)
- [x] AES-256-GCM 암호화 자격증명
- [x] 자산 곡선 히스토리 + 월별 수익률 뷰
- [x] 심볼 메타데이터 테이블 (012_symbol_info.sql) - 자동완성 검색 지원

## [완료] 심볼 관리 아키텍처 개선 ✅ (2026-01-30)
**핵심 변경**: 중립 심볼(canonical symbol) 기반 아키텍처로 리팩토링

### 구현 완료
- [x] **SymbolResolver 구현** (`trader-data/src/provider/symbol_info.rs`)
  - `normalize_symbol()`: 자동 형식 감지 및 정규화 (예: "005930.KS" → "005930")
  - `to_source_symbol()`: canonical → 데이터 소스별 심볼 변환
  - `to_canonical()`: 데이터 소스 심볼 → canonical 변환
  - `to_display_string()`: "티커(종목명)" 형식 표시 문자열 생성
  - `search()`: 티커, 종목명, Alias 통합 검색
  - `get_or_create_symbol_info()`: 캐시 → DB → 자동 생성 체인
  - `get_display_names_batch()`: 배치 조회로 성능 최적화

- [x] **인메모리 캐싱 구현**
  - `RwLock<HashMap>` 기반 스레드 안전 캐시
  - 최초 조회 시 DB에서 로드 → 이후 캐시 반환
  - 캐시 미스 시 자동 생성 후 DB 저장 (점진적 DB 구축)

- [x] **API 응답에 display_name 필드 추가**
  - `PositionResponse.display_name`: 포지션 목록/상세
  - `OrderResponse.display_name`: 주문 목록/상세
  - `HoldingInfo.display_name`: 보유종목 목록
  - `SimulationPosition.display_name`: 시뮬레이션 포지션
  - `SimulationTrade.display_name`: 시뮬레이션 거래
  - `DatasetSummary.display_name`: 데이터셋 목록

- [x] **AppState 헬퍼 메서드**
  - `get_display_names()`: 배치 조회
  - `get_display_name()`: 단일 조회

- [x] **프론트엔드 타입 업데이트**
  - `Position.displayName`: types/index.ts
  - `Order.displayName`: types/index.ts
  - `HoldingInfo.displayName`: api/client.ts
  - `SimulationPosition.displayName`: api/client.ts
  - `SimulationTrade.displayName`: api/client.ts
  - `DatasetSummary.displayName`: Dataset.tsx

- [x] **프론트엔드 컴포넌트 업데이트**
  - Dashboard.tsx: 보유종목 표시에 displayName 사용
  - Simulation.tsx: 포지션/거래 내역에 displayName 사용

- [x] **데이터 소스 종속 함수 제거**
  - `parse_yahoo_symbol()` 삭제 (ohlcv.rs)
  - `to_yahoo_symbol()` 삭제 (historical.rs)
  - 엔드포인트 메서드에서 특정 도메인 의존성 제거

### 아키텍처 설계
```
┌─────────────────────────────────────────────────────────────┐
│                   Canonical Symbol (중립 심볼)               │
│                 "005930", "AAPL", "BTC/USDT"                │
│                 시스템 전체에서 이 형식만 사용                 │
└─────────────────────────┬───────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                      SymbolResolver                          │
│         중립 심볼 ↔ 데이터 소스별 심볼 변환                   │
├─────────────────────────────────────────────────────────────┤
│  "005930" ───┬─→ Yahoo:   "005930.KS"                       │
│              ├─→ KIS:     "005930"                          │
│              └─→ KRX:     "A005930"                         │
│  "BTC/USDT" ─┬─→ Binance: "BTCUSDT"                         │
│              └─→ Yahoo:   "BTC-USD"                         │
└─────────────────────────────────────────────────────────────┘
```

### SymbolMetadata 구조
- `ticker`: 중립 심볼 ("005930")
- `name`: 종목명 ("삼성전자")
- `name_en`: 영문명 ("Samsung Electronics")
- `yahoo_symbol`: Yahoo 형식 Alias ("005930.KS")
- 표시 문자열: "005930(삼성전자)"

---

# 남은 작업 목록

## [최우선] 백테스트/시뮬레이션 UI 플로우 개선 ⭐
**핵심 변경사항**: 백테스트/시뮬레이션 페이지에서 "등록된 전략"만 테스트 가능하도록 변경

**새로운 워크플로우**:
1. 전략 페이지에서 전략 등록 (파라미터 포함)
2. 백테스트/시뮬레이션 페이지에서 등록된 전략 선택
3. 심볼/기간/초기자본만 입력하여 테스트 실행 (파라미터 입력 불필요)

**구현 작업**:
- [ ] 백엔드: `StrategyListItem`에 `strategy_type` 필드 추가
- [ ] 백엔드: `list_strategies` API에서 `strategy_type` 반환
- [ ] 백엔드: 백테스트 API에서 등록된 전략 ID로 실행 지원
- [ ] 프론트: Backtest.tsx에서 `getStrategies()` 사용 (등록된 전략만 표시)
- [ ] 프론트: 파라미터 입력 SDUI 폼 제거 (등록된 설정 사용)
- [ ] 프론트: Simulation.tsx 동일하게 수정
- [ ] 전략 페이지에서 모든 전략 등록 테스트
- [ ] 백테스트 페이지에서 등록된 전략 테스트

## 🔴 [버그] KIS API ISA 체결 내역 조회 ⭐

**문제**: Dashboard 자산 곡선 동기화 시 ISA 계좌에서 체결 내역 조회가 정상 동작하지 않음

**현재 상태**:
- `get_order_history()` 함수가 일반 계좌와 ISA 계좌를 동일한 tr_id로 처리
- `KR_ORDER_HISTORY_REAL`: "TTTC0081R" (일반 계좌용)
- ISA 계좌는 별도의 tr_id나 API 엔드포인트가 필요할 수 있음

**관련 파일**:
- `trader-exchange/src/connector/kis/client_kr.rs:712-804` - get_order_history 함수
- `trader-exchange/src/connector/kis/mod.rs:126-128` - tr_id 정의
- `trader-api/src/routes/analytics.rs:1069-1209` - sync_equity_curve 함수

**조사 결과 (2026-01-30)**:
- [x] KIS API 공식 GitHub 샘플에서도 ISA 계좌 특별 처리 없음
- [x] tr_id TTTC0081R은 실전/모의, 조회 기간으로만 분기 (계좌 유형 분기 없음)
- [x] 공식 계좌 상품 코드: "01"(종합), "22"(연금저축), "29"(퇴직연금) - ISA 코드 미명시
- [ ] **ISA 계좌 상품 코드 확인 필요** (01이 아닐 수 있음)
- [ ] **실제 API 응답 에러 메시지 확인 필요**
- [ ] KIS 고객센터 또는 API 챗봇 문의 권장

**참고 리소스**:
- [KIS Developers 공식 포털](https://apiportal.koreainvestment.com)
- [GitHub open-trading-api](https://github.com/koreainvestment/open-trading-api)
- [KIS API 챗봇](https://chatgpt.com/g/g-68b920ee7afc8191858d3dc05d429571)

---

## [완료] 데이터셋 페이지 추가 기능 ✅
- [x] 1시간 타임프레임 차트 문제 해결 (Unix timestamp 변환)
- [x] 테이블 무한 스크롤링 구현 (Intersection Observer API)
- [x] 심볼 자동완성 기능 추가 ✅
  - DB: 012_symbol_info.sql 마이그레이션 (symbol_info 테이블 + GIN 인덱스)
  - 백엔드: SymbolInfoRepository, GET /api/v1/strategies/symbols/search?q=
  - 프론트엔드: 디바운스 검색 (200ms), 자동완성 드롭다운 UI
- [x] 날짜 범위 다운로드 옵션 추가 ✅
  - 백엔드: FetchDatasetRequest에 start_date, end_date 옵션 추가
  - 백엔드: CachedHistoricalDataProvider.get_klines_range() - Yahoo Finance get_quote_history_interval() API 사용
  - 프론트엔드: 다운로드 폼에 "날짜 범위 사용" 체크박스 + 날짜 선택기 UI

## [진행중] 데이터셋 페이지 추가 기능 ⏳
- [ ] 전략별 타임프레임 설정 기능 추가
  - 백엔드: strategies 테이블에 timeframe 컬럼 추가
  - 프론트엔드: 전략 편집 모달에 타임프레임 선택 추가

## [미구현] 매매 일지 (Trading Journal) ⭐ 신규 요구사항
**핵심 기능**: 체결 내역 기반 종목별 매매 기록 관리

### 기능 상세
1. **거래소 체결 내역 동기화**
   - KIS/Binance 체결 내역 자동 동기화

2. **종목별 보유 현황 조회**
   - 보유 주식 수, 수량
   - 평균 매입가 (물타기 시 가중평균 자동 계산)
   - 투자 금액, 평가 금액
   - 보유 비중 (포트폴리오 내 비율)

3. **매매 이력 타임라인**
   - 매수/매도 시점, 가격, 수량
   - 물타기 기록 추적

4. **손익 분석**
   - 종목별 실현 손익
   - 미실현 손익 (현재 평가손익)
   - 기간별 수익률 추이

5. **투자 인사이트**
   - 매매 패턴 분석 (빈도, 성공률)
   - 종목별 평균 보유 기간
   - 리밸런싱 추천 (비중 조정)

6. **전략 수립 지원**
   - 목표 비중 설정 및 현재 비중 비교
   - 손익분기점 계산

### 백엔드 구현
- [ ] DB 스키마 설계
  - `trade_executions` - 체결 내역 저장
  - `position_snapshots` - 종목별 포지션 스냅샷
- [ ] 종목별 포지션 집계 서비스
  - 보유 수량, 평균 매입가 계산
  - 물타기 시 가중평균 계산
- [ ] 매매 일지 API 엔드포인트
  - `GET /api/v1/journal/positions` - 보유 현황
  - `GET /api/v1/journal/executions` - 체결 내역
  - `GET /api/v1/journal/pnl` - 손익 분석
  - `POST /api/v1/journal/sync` - 거래소 동기화

### 프론트엔드 구현
- [ ] TradingJournal.tsx 페이지 생성
- [ ] 보유 현황 테이블 (종목, 수량, 평단, 수익률)
- [ ] 체결 내역 타임라인 컴포넌트
- [ ] 포지션 비중 차트 (파이/도넛)
- [ ] 종목별 손익 분석 대시보드
- [ ] 목표 비중 설정 모달

---

# 코드 최적화 기회 (분석 결과)

## Backend API
- [ ] portfolio.rs:441 - 당일 손익 계산 TODO
- [ ] portfolio.rs:461 - 당일 수익률 계산 TODO
- [ ] OAuth 토큰 캐시 TTL 관리 로직 부재
- [ ] ML 모델 예측 기능 미완성 (훈련만 가능)

## 전략 모듈
- [ ] 큰 파일 리팩토링 기회 (900줄+ 파일들)
  - xaa.rs (1,103줄)
  - candle_pattern.rs (958줄)
  - rsi.rs (932줄)
- [ ] 캔들 패턴 매칭 성능 최적화 가능

## 거래소 모듈
- [ ] Binance WebSocket 구현 완성 (구조만 존재)
- [ ] KIS 선물/옵션 거래 미구현

## 백테스트/분석
- [ ] 틱 시뮬레이션 미구현 (설정만 존재)
- [ ] 마진 거래 검증 미구현
- [ ] 대규모 데이터셋 성능 테스트 필요

---

# 백테스트 테스트 현황

## 단일 자산 전략 (✅ 모두 동작)
| 전략 | 상태 | 거래 수 | 수익률 | 테스트 수 |
|-----|------|--------|--------|----------|
| RSI 평균회귀 | ✅ | 1회 | - | 4개 |
| 그리드 트레이딩 | ✅ | 17회 | +7.90% | 5개 |
| 볼린저 밴드 | ✅ | 3회 | -0.58% | 3개 |
| 변동성 돌파 | ✅ | 28회 | -2.60% | 3개 |
| Magic Split | ✅ | 13회 | -0.69% | 10개 |
| 이동평균 크로스오버 | ✅ | 6회 | +9.38% | 2개 |
| 트레일링 스탑 | ✅ | - | - | 3개 |
| 캔들 패턴 | ✅ | - | - | 3개 |
| 무한매수봇 | ✅ | - | - | 5개 |

## 자산배분 전략 (테스트 완료, 다중 심볼 백테스트 필요)
| 전략 | 테스트 수 | 필요 심볼 |
|-----|----------|-----------|
| Simple Power | 12개 | TQQQ, SCHD, PFIX, TMF |
| HAA | 14개 | TIP, SPY, IWM 등 10개 |
| XAA | 16개 | VWO, BND, SPY 등 9개 |
| Stock Rotation | 14개 | 005930, 000660 등 5개 |
| All Weather | 4개 | SPY, TLT, IEF, GLD 등 |
| Snow | 4개 | TIP, UPRO, TLT, BIL |
| Market Cap TOP | 5개 | AAPL, MSFT 등 10개 |

## 남은 백테스트 작업
- [ ] 다중 자산 백테스트 API 엔드포인트 구현 (/api/v1/backtest/run-multi)
- [ ] 다중 심볼 데이터 로딩 함수 구현

---

# 낮은 우선순위 작업

## 텔레그램 봇 명령어 ⏳
현재 상태: 푸시 알림만 구현됨, 명령어 수신 미구현

- [ ] 텔레그램 Bot API 명령어 핸들러 구현
  - getUpdates 또는 Webhook 방식 선택
  - 명령어 파싱 및 라우팅
- [ ] `/portfolio` - 현재 포트폴리오 조회
- [ ] `/performance` - 성과 지표 조회
- [ ] `/report` - 일일/주간 리포트 생성
- [ ] `/status` - 전략 실행 상태 조회
- [ ] `/stop <strategy_id>` - 전략 중지

## 추가 거래소 통합
- [ ] Coinbase 거래소
- [ ] Kraken 거래소
- [ ] Interactive Brokers
- [ ] 키움증권 (Windows COM)

## 인프라 & 모니터링
- [ ] Grafana 모니터링 대시보드
- [ ] 성능 및 부하 테스트

---

# 참고 문서
- PRD v2.0: [C:\Users\HP\.claude\plans\synthetic-conjuring-peach.md](C:\Users\HP\.claude\plans\synthetic-conjuring-peach.md)
- 전략 비교: [docs/STRATEGY_COMPARISON.md](docs/STRATEGY_COMPARISON.md)
- 기존 PRD (백업): [C:\Users\HP\.claude\plans\toasty-chasing-patterson.md](C:\Users\HP\.claude\plans\toasty-chasing-patterson.md)
