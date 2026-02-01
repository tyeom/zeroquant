# ZeroQuant Trading Bot - PRD (Product Requirements Document)

> 버전: 6.0 | 마지막 업데이트: 2026-02-01

---

## 1. 제품 개요

### 1.1 목적
Rust 기반 고성능 다중 시장 자동화 트레이딩 시스템. 국내/해외 주식 및 암호화폐 시장에서 다양한 전략을 자동으로 실행하고 관리한다.

### 1.2 대상 사용자
- 개인 투자자 (개인 프로젝트)
- 퀀트 트레이딩 학습자

### 1.3 핵심 가치
- **자동화**: 전략 기반 자동 매매로 감정 개입 배제
- **다양성**: 25+ 전략, 다중 거래소/시장 지원
- **안전성**: 리스크 관리, 시뮬레이션 검증 후 실전 운용
- **학습**: 백테스트를 통한 전략 성과 분석 및 개선

---

## 2. 기능 요구사항

### 2.1 전략 관리

#### 2.1.1 전략 등록
- 사용자는 제공된 기본 전략(27개) 중 선택하여 새로운 전략 인스턴스를 생성한다
- 전략 유형:
  - **단일 자산 전략**: 하나의 심볼에 대해 매매 신호 생성 (Grid, RSI, Bollinger 등)
  - **자산배분 전략**: 여러 심볼로 구성된 포트폴리오 리밸런싱 (HAA, XAA, All Weather 등)
- 전략 인스턴스는 고유한 이름으로 저장되며, 동일 기본 전략에서 여러 인스턴스 생성 가능

#### 2.1.2 파라미터 설정 (SDUI 자동 생성)

##### 기본 동작
- 각 전략은 SDUI(Server-Driven UI) 스키마를 통해 동적 파라미터 폼을 렌더링한다
- **전략 Config 구조체에서 UI 스키마가 자동 생성**되어 수동 스키마 작성 불필요
- 파라미터 유형:
  - **심볼**: 대상 종목 (자동완성 검색 지원)
  - **기술적 지표**: RSI 기간, 볼린저 밴드 표준편차, 이동평균 기간 등
  - **거래 조건**: 진입/청산 임계값, 포지션 크기 비율
  - **타임프레임**: 1분, 5분, 15분, 30분, 1시간, 4시간, 일봉
- 파라미터 유효성 검증:
  - 숫자 범위 제한 (min/max)
  - 필수 값 검증
  - 타입 검증 (정수, 실수, 문자열, 배열)

##### SDUI Fragment 시스템
- **Schema Fragment**: 재사용 가능한 UI 스키마 조각
  - 카테고리: Indicator, Filter, RiskManagement, PositionSizing, Timing, Asset
  - 예: `indicator.rsi`, `filter.route_state`, `risk.trailing_stop`
- **FragmentRegistry**: 빌트인 Fragment 관리 및 조회
- **SchemaComposer**: Fragment + 커스텀 필드 → 완성된 SDUI JSON 조합

##### 자동 생성 흐름
1. 전략 Config에 `#[derive(StrategyConfig)]` 매크로 적용
2. 사용할 Fragment를 `#[fragment("id")]` 속성으로 지정
3. 커스텀 필드는 `#[schema(label, min, max)]` 속성으로 메타데이터 정의
4. 런타임에 `SchemaComposer`가 완성된 SDUI JSON 반환
5. 프론트엔드 `SDUIRenderer`가 JSON 기반으로 폼 렌더링

##### 조건부 필드 표시
- 특정 필드 값에 따라 다른 필드 표시/숨김 가능
- 예: "트레일링 스탑 활성화" 체크 시에만 관련 설정 표시
- `condition` 속성: `"enabled == true"`, `"mode == 'advanced'"` 등

##### API 엔드포인트
- `GET /api/v1/strategies/meta`: 전략 목록 및 기본 정보
- `GET /api/v1/strategies/{id}/schema`: 해당 전략의 완성된 SDUI JSON
- `GET /api/v1/schema/fragments`: 사용 가능한 Fragment 카탈로그

#### 2.1.3 리스크 설정
- 전략별 리스크 파라미터:
  - **손절가 (Stop Loss)**: 진입가 대비 손실 허용 비율 (기본 3%)
  - **익절가 (Take Profit)**: 진입가 대비 목표 수익 비율 (기본 5%)
  - **트레일링 스탑**: 고점 대비 하락 허용 비율 (기본 2%)
  - **포지션 크기**: 총 자본 대비 단일 포지션 비율 (최대 10%)
- 리스크 설정은 전략별 기본값이 SDUI 스키마에 정의되며, 사용자가 수정 가능
- 일일 손실 한도 설정 (기본 3%, UTC 자정 자동 리셋)

#### 2.1.4 전략 CRUD
- **생성**: 기본 전략 선택 → 파라미터 입력 → 저장
- **조회**: 등록된 전략 목록, 상세 정보 조회
- **수정**: 파라미터 변경 (전략 유형 변경 불가)
- **삭제**: 전략 인스턴스 삭제 (관련 백테스트 결과 보존)
- **복사**: 기존 전략 복사하여 새 인스턴스 생성 (이름만 변경)

---

### 2.2 백테스트

#### 2.2.1 백테스트 실행
- 입력 조건:
  - **전략**: 등록된 전략 인스턴스 선택
  - **기간**: 시작일 ~ 종료일 (과거 데이터)
  - **초기 자본**: 시뮬레이션 시작 금액
- 데이터 요구사항:
  - 해당 심볼/기간의 OHLCV 데이터 필요
  - 캐시에 없을 경우 Yahoo Finance에서 자동 다운로드
- 시뮬레이션 옵션:
  - **슬리피지**: 체결가 오차 (기본 0.1%)
  - **수수료**: 거래 수수료 (기본 0.1%)
  - **마진**: 레버리지 설정 (선물/암호화폐)

#### 2.2.2 성과 지표
- 수익률 지표:
  - 총 수익률, 연환산 수익률 (CAGR)
  - 월별/연도별 수익률
- 리스크 지표:
  - **MDD** (Maximum Drawdown): 최대 낙폭
  - **변동성**: 수익률 표준편차
- 위험조정 수익률:
  - **Sharpe Ratio**: (수익률 - 무위험수익률) / 변동성
  - **Sortino Ratio**: 하방 변동성만 고려
  - **Calmar Ratio**: CAGR / MDD
- 거래 통계:
  - 총 거래 수, 승률
  - 평균 수익/손실, 손익비
  - 최장 연승/연패

#### 2.2.3 결과 시각화
- **자산 곡선**: 일별 포트폴리오 가치 추이
- **드로다운 차트**: 고점 대비 하락률 추이
- **월별 수익률 히트맵**: 연-월 매트릭스 색상 표시
- **거래 목록**: 진입/청산 시점, 가격, 손익

#### 2.2.4 결과 저장
- 백테스트 결과 DB 저장
- 저장 항목: 전략 ID, 기간, 파라미터 스냅샷, 성과 지표, 거래 내역
- 히스토리 조회 및 비교 기능

---

### 2.2.5 신호 기록 (SignalMarker)

**목적**: 전략이 생성한 모든 신호를 DB에 저장하여 분석 및 시각화에 활용

**핵심 기능**:
- 진입/청산 신호, 신호 강도, 지표 값 기록
- 백테스트와 실거래에서 동일 형식 사용
- UnifiedTrade trait으로 타입 통합

**저장 정보**:
- 신호 유형 (Entry, Exit, Alert)
- 발생 시점 지표 값 (RSI, MACD, BB 등)
- RouteState, 전략 정보
- 실행 여부 (체결/미체결)

**예상 구현**: v0.6.0 (TODO Phase 1-5)

#### 2.2.6 신호 시각화 (캔들 차트 오버레이)

**SignalMarker 오버레이**:
- 매수 신호: 초록색 위 화살표 ▲
- 매도 신호: 빨간색 아래 화살표 ▼
- 알림 신호: 노란색 점 ●

**IndicatorFilter 패널**:
- RSI 범위 슬라이더
- MACD 크로스 유형 선택
- RouteState 필터
- 전략 선택 드롭다운

**통합 화면**:
- 백테스트 결과 페이지
- 종목 상세 페이지
- 전략 디버깅 페이지

**예상 구현**: v0.6.0 (TODO Phase 2-4)

---

### 2.3 시뮬레이션 (Paper Trading)

#### 2.3.1 시뮬레이션 실행
- 실시간 시장 데이터 기반 가상 거래
- 실제 자금 사용 없이 전략 검증
- 실행 모드:
  - **실시간 모드**: WebSocket으로 틱/분봉 데이터 수신
  - **가속 모드**: 과거 데이터를 빠르게 재생 (선택적)

#### 2.3.2 포지션 관리
- 가상 포지션 추적:
  - 보유 종목, 수량, 진입가
  - 미실현 손익 (현재가 기준)
- 가상 주문 실행:
  - 지정가/시장가 주문
  - 주문 체결 시뮬레이션 (호가창 기반)

#### 2.3.3 성과 모니터링
- 실시간 대시보드:
  - 현재 포트폴리오 가치
  - 일별/누적 수익률
  - 활성 포지션 목록
- 거래 내역 로깅

---

### 2.4 실전 운용 (Live Trading)

#### 2.4.1 거래소 연동
- 지원 거래소:
  - **Binance**: 암호화폐 현물/선물
  - **KIS (한국투자증권)**: 국내 주식, 해외 주식 (미국)
- 연동 기능:
  - OAuth/API Key 인증
  - 잔고 조회, 주문 실행, 체결 내역 조회
  - 실시간 시세 WebSocket

#### 2.4.2 자동 매매
- 전략 신호 기반 자동 주문:
  - 매수/매도 신호 발생 시 자동 주문 전송
  - 손절/익절 조건 충족 시 자동 청산
- 주문 유형:
  - 시장가 (Market)
  - 지정가 (Limit)
  - 스탑 주문 (Stop-Loss, Take-Profit)
- 주문 검증:
  - 최소 주문 수량 확인
  - 잔고 충분 여부 확인
  - 일일 거래 한도 확인

#### 2.4.3 포트폴리오 관리
- 통합 잔고 조회:
  - 여러 거래소/계좌 잔고 통합
  - 자산 배분 현황 (비중)
- 보유 종목 현황:
  - 종목별 수량, 평균 매입가
  - 평가 금액, 수익률

#### 2.4.4 알림 시스템
- **텔레그램 푸시 알림**:
  - 주문 체결 알림
  - 손익 임계값 도달 알림
  - 시스템 오류 알림
- **텔레그램 봇 명령어** (양방향 통신):
  - `/portfolio`: 현재 포트폴리오 조회
  - `/status`: 전략 실행 상태 조회
  - `/stop <id>`: 특정 전략 중지
  - `/report`: 일일/주간 성과 리포트

---

### 2.5 데이터 관리

#### 2.5.1 시장 데이터 수집
- **OHLCV 데이터**:
  - Open, High, Low, Close, Volume
  - 지원 타임프레임: 1m, 5m, 15m, 30m, 1h, 4h, 1d
- **데이터 소스**:
  - Yahoo Finance (주식, ETF)
  - Binance API (암호화폐)
  - KIS API (국내 주식 실시간)
- **자동 다운로드**:
  - 백테스트 실행 시 필요 데이터 자동 요청
  - 캐시 (TimescaleDB)에 저장하여 재사용

#### 2.5.2 데이터셋 관리
- 데이터셋 목록 조회:
  - 보유 심볼, 기간, 데이터 포인트 수
- 차트 시각화:
  - 캔들스틱 차트
  - 기술적 지표 오버레이 (SMA, EMA, RSI, MACD, Bollinger)
- 데이터 품질 검증:
  - 누락 구간 감지
  - 이상치 표시

#### 2.5.3 심볼 검색
- 자동완성 검색:
  - 티커, 종목명, 영문명으로 검색
  - 시장별 필터링 (KR, US, Crypto)
- 심볼 정보:
  - 정규화된 심볼 (canonical)
  - 거래소별 심볼 매핑 (Yahoo, KIS, Binance)
  - 표시 이름: "티커(종목명)" 형식

#### 2.5.4 심볼 자동 동기화
- **목적**: 스크리닝 수집기 가동 시 자동으로 전체 종목 목록을 수집하여 symbol_info 테이블에 등록
- **데이터 소스**:
  - **KRX (한국거래소)**: KOSPI/KOSDAQ 전 종목 (~2,500개)
  - **Binance**: USDT 거래 페어 활성 종목 (~300개)
  - **Yahoo Finance**: 미국 주식 주요 지수 구성종목 (S&P 500, NASDAQ 등)
- **동기화 트리거**:
  - 서버 시작 시 심볼 수가 최소 기준 이하면 자동 실행
  - Fundamental 배치 수집 전 자동 호출
- **환경변수**:
  | 변수 | 기본값 | 설명 |
  |------|--------|------|
  | `SYMBOL_SYNC_KRX` | true | KRX 동기화 활성화 |
  | `SYMBOL_SYNC_BINANCE` | false | Binance 동기화 활성화 |
  | `SYMBOL_SYNC_YAHOO` | true | Yahoo Finance 동기화 활성화 |
  | `SYMBOL_SYNC_YAHOO_MAX` | 500 | Yahoo 최대 수집 수 |
  | `SYMBOL_SYNC_MIN_COUNT` | 100 | 최소 심볼 수 기준 |

#### 2.5.5 Fundamental 데이터 백그라운드 수집
- **목적**: 서버 실행 중 백그라운드에서 Fundamental 데이터를 주기적으로 배치 수집
- **수집 지표**:
  - 시가총액, 발행주식수, 52주 고저가
  - PER, PBR, ROE, ROA
  - 배당수익률, 배당성향
  - 영업이익률, 순이익률
  - 부채비율, 유동비율
- **수집 방식**:
  - Yahoo Finance API 연동
  - Rate Limiting 적용 (요청 간 2초 딜레이)
  - 7일 이상 경과한 데이터 자동 갱신
- **OHLCV 증분 업데이트**:
  - Fundamental 수집 시 동일 API 호출로 1년치 일봉 OHLCV도 함께 저장
  - ON CONFLICT DO UPDATE로 중복 없이 병합
- **환경변수**:
  | 변수 | 기본값 | 설명 |
  |------|--------|------|
  | `FUNDAMENTAL_COLLECT_ENABLED` | true | 수집기 활성화 |
  | `FUNDAMENTAL_COLLECT_INTERVAL_SECS` | 3600 | 수집 주기 (초) |
  | `FUNDAMENTAL_STALE_DAYS` | 7 | 갱신 기준 (일) |
  | `FUNDAMENTAL_BATCH_SIZE` | 50 | 배치당 처리 심볼 수 |
  | `FUNDAMENTAL_REQUEST_DELAY_MS` | 2000 | API 요청 간 딜레이 |
  | `FUNDAMENTAL_UPDATE_OHLCV` | true | OHLCV 증분 업데이트 |
  | `FUNDAMENTAL_AUTO_SYNC_SYMBOLS` | true | 심볼 자동 동기화 |

---

### 2.6 매매 일지 (Trading Journal)

#### 2.6.1 체결 내역 동기화
- 거래소에서 체결 내역 자동 동기화:
  - KIS: 국내/해외 체결 내역
  - Binance: 현물/선물 체결 내역
- 동기화 주기: 수동 또는 자동 (설정 가능)

#### 2.6.2 종목별 보유 현황
- 보유 종목 상세 정보:
  - 보유 수량
  - 평균 매입가 (물타기 시 가중평균 자동 계산)
  - 투자 금액 (총 매입가)
  - 평가 금액 (현재가 × 수량)
  - 포트폴리오 내 비중 (%)

#### 2.6.3 매매 이력 타임라인
- 종목별 거래 히스토리:
  - 매수/매도 시점, 가격, 수량
  - 물타기/분할매도 기록
- 시간순 타임라인 뷰

#### 2.6.4 손익 분석
- **실현 손익**: 청산된 거래의 확정 손익
- **미실현 손익**: 보유 중인 포지션의 평가손익
- **기간별 수익률**:
  - 일별, 주별, 월별, 연도별
  - 누적 수익률 곡선

#### 2.6.5 투자 인사이트
- 매매 패턴 분석:
  - 평균 보유 기간
  - 승률, 손익비
- 리밸런싱 추천:
  - 목표 비중 대비 현재 비중 비교
  - 조정 필요 종목 표시

---

### 2.7 ML 예측

#### 2.7.1 모델 훈련
- 지원 알고리즘:
  - XGBoost
  - LightGBM
  - RandomForest
- 훈련 데이터:
  - OHLCV 기반 특징 추출 (22개 기술 지표)
  - **구조적 피처** (6개): 저점 추세, 거래량 질, 박스권 위치, MA 이격도, BB 폭, RSI
  - 레이블: 다음 기간 수익률 방향 (상승/하락)
- 훈련 환경:
  - ONNX 형식으로 저장 후 Rust Runtime에서 추론

#### 2.7.4 구조적 피처 (Structural Features)
- **목적**: "살아있는 횡보"와 "죽은 횡보"를 구분하여 돌파 가능성 예측
- **피처 목록**:
  | 피처 | 설명 | 의미 |
  |------|------|------|
  | `low_trend` | 저점 상승 강도 (Higher Low) | 양수면 저점이 올라가는 중 |
  | `vol_quality` | 양봉/음봉 거래량 비율 | 1 초과면 매수세 우위 |
  | `range_pos` | 박스권 내 위치 (0~1) | 0.8 이상이면 돌파 임박 |
  | `dist_ma20` | MA20 이격도 | 0 근처가 눌림목 구간 |
  | `bb_width` | 볼린저 밴드 폭 | 낮을수록 에너지 응축 |
  | `rsi` | RSI 14일 | 과매수/과매도 필터링 |
- **활용**:
  - ML 모델 입력 피처로 추가
  - 스크리닝 필터 조건으로 활용
  - RouteState 판정 로직에 반영

#### 2.7.5 TTM Squeeze 지표 (John Carter)

**목적**: Bollinger Band가 Keltner Channel 내부로 들어가면 에너지 응축 상태

**계산 방식**:
1. **Bollinger Band** (BB): 20일 SMA ± 2σ
2. **Keltner Channel** (KC): 20일 EMA ± 1.5 × ATR(20)
3. **Squeeze 판정**: BB_upper < KC_upper AND BB_lower > KC_lower
4. **Release 판정**: 이전 봉은 Squeeze, 현재 봉은 Squeeze 해제

**출력 형식**:
```rust
pub struct TtmSqueeze {
    pub is_squeeze: bool,        // 현재 스퀴즈 상태
    pub squeeze_count: u32,      // 연속 스퀴즈 일수
    pub momentum: Decimal,       // 스퀴즈 모멘텀 (방향)
    pub released: bool,          // 이번 봉에서 해제되었는가?
}
```

**활용**:
- RouteState ATTACK 판정 (Release + Momentum > 0)
- TRIGGER 시스템에 +30점 기여
- 변동성 돌파 전략 필터링

**DB 저장**:
- `symbol_fundamental` 테이블에 컬럼 추가:
  - `ttm_squeeze`: BOOLEAN
  - `ttm_squeeze_cnt`: INTEGER (연속 일수)

**예상 구현**: v0.6.0 (TODO Phase 1-2.3)

#### 2.7.6 추가 기술적 지표

**목적**: 분석 정확도 향상을 위한 고급 지표

**4개 신규 지표**:
| 지표 | 설명 | 용도 |
|------|------|------|
| **HMA** | Hull Moving Average | 빠른 반응, 낮은 휩소 |
| **OBV** | On-Balance Volume | 스마트 머니 추적 |
| **SuperTrend** | 추세 추종 지표 | 트렌드 방향 판정 |
| **CandlePattern** | 캔들 패턴 감지 | 망치형, 장악형 등 |

**구현 위치**:
```
trader-analytics/src/indicators/
├── hma.rs         // Hull Moving Average
├── obv.rs         // On-Balance Volume
├── supertrend.rs  // SuperTrend
└── candle_patterns.rs // 캔들 패턴 감지
```

**활용**:
- TRIGGER 시스템에 캔들 패턴 연동
- 전략 신호 생성에 활용
- 구조적 피처 확장

**예상 구현**: v0.6.0 (TODO Phase 1-2.6)

#### 2.7.2 모델 관리
- 모델 등록 API:
  - ONNX 파일 경로, 메타데이터
  - 훈련 심볼, 정확도 지표
- 모델 버전 관리:
  - 심볼별 최신 모델 관리
  - 모델 배포/롤백

#### 2.7.3 예측 활용
- 전략에서 ML 예측 결과 사용:
  - 진입 신호 필터링 (ML이 상승 예측 시만 매수)
  - 예측 확률 기반 포지션 크기 조절
- 패턴 인식 통합:
  - 26개 캔들스틱 패턴
  - 24개 차트 패턴

---

## 3. 비기능 요구사항

### 3.1 성능
| 항목 | 요구사항 |
|------|---------|
| API 응답 시간 | 일반 조회 < 200ms, 백테스트 < 5초 (1년 데이터) |
| 동시 전략 | 10개 이상 동시 실행 |
| 데이터 처리 | 100만 캔들 백테스트 < 30초 |
| WebSocket | 틱 데이터 지연 < 100ms |

### 3.2 보안
| 항목 | 요구사항 |
|------|---------|
| API Key 저장 | AES-256-GCM 암호화 |
| 환경 변수 | 민감 정보 환경 변수로 관리 |
| 접근 제어 | 로컬 실행 (외부 접근 차단) |

### 3.3 가용성
| 항목 | 요구사항 |
|------|---------|
| 실행 환경 | 로컬 PC (Windows/Linux/macOS) |
| 데이터베이스 | TimescaleDB (PostgreSQL 확장) |
| 캐시 | Redis |
| 컨테이너 | Docker/Podman |

### 3.4 확장성
| 항목 | 요구사항 |
|------|---------|
| 전략 추가 | 새로운 전략 플러그인 구조 |
| 거래소 추가 | Exchange trait 구현으로 확장 |
| 지표 추가 | Indicator trait 구현으로 확장 |

---

## 4. 기술 스택

| 계층 | 기술 | 용도 |
|------|------|------|
| Backend | Rust, Tokio, Axum | 고성능 비동기 API 서버 |
| Database | PostgreSQL + TimescaleDB | 시계열 데이터 저장 |
| Cache | Redis | 세션, 실시간 데이터 캐시 |
| Frontend | SolidJS, TypeScript, Vite | 반응형 SPA |
| ML | ONNX Runtime, Python | 모델 추론, 훈련 |
| Infra | Podman/Docker | 컨테이너화된 인프라 |

---

## 5. 지원 거래소

### 5.1 Binance
- **시장**: 암호화폐 현물, 선물
- **기능**: 잔고 조회, 주문 실행, 체결 내역, WebSocket 실시간 시세
- **인증**: API Key + Secret

### 5.2 KIS (한국투자증권)
- **시장**: 국내 주식, 해외 주식 (미국)
- **기능**: 잔고 조회, 주문 실행, 체결 내역, WebSocket 실시간 시세
- **인증**: OAuth 2.0 (App Key, App Secret, 계좌번호)
- **계좌 유형**: 일반, ISA, 연금

### 5.3 추가 거래소 (선택적 확장)
- Coinbase, Kraken (암호화폐)
- Interactive Brokers, 키움증권 (주식)

---

## 6. 전략 목록

### 6.1 단일 자산 전략 (9개)

| 전략 | 설명 | 주요 파라미터 |
|------|------|-------------|
| Grid Trading | 가격 구간별 매수/매도 주문 | 그리드 수, 가격 범위 |
| RSI Mean Reversion | RSI 과매도/과매수 기반 매매 | RSI 기간, 과매도/과매수 임계값 |
| Bollinger Bands | 볼린저 밴드 이탈 시 평균회귀 | 기간, 표준편차 배수 |
| Volatility Breakout | 전일 변동성 돌파 시 진입 | K 계수 |
| Magic Split | 분할 매수/매도 | 분할 횟수, 간격 비율 |
| SMA Crossover | 이동평균 골든/데드 크로스 | 단기/장기 이동평균 기간 |
| Trailing Stop | 트레일링 스탑 기반 청산 | 트레일링 비율 |
| Candle Pattern | 캔들 패턴 인식 매매 | 패턴 유형, 확인 캔들 수 |
| Infinity Bot | 무한 분할 매수 (물타기) | 라운드 수, 매수 간격 |

### 6.2 자산배분 전략 (16개+)

| 전략 | 설명 | 대상 시장 |
|------|------|---------|
| Simple Power | 심플 파워 모멘텀 | US ETF |
| HAA | 계층적 자산배분 | Global |
| XAA | 확장 자산배분 | Global |
| All Weather | 레이 달리오 올웨더 포트폴리오 | US ETF |
| Snow | 스노우 모멘텀 | US ETF |
| Stock Rotation | 종목 갈아타기 | KR/US |
| Market Cap Top | 시총 상위 N종목 | KR/US |
| Market Interest Day | 시장 관심 단타 | KR |
| Dual Momentum | 듀얼 모멘텀 | US |
| BAA | Bold Asset Allocation | US |
| US 3X Leverage | 3배 레버리지 ETF 전략 | US ETF |
| Stock Gugan | 주식 구간 매매 | KR/US |
| KOSDAQ Fire Rain | 코스닥 급등주 | KR |
| Sector VB | 섹터 변동성 돌파 | KR |
| KOSPI BothSide | 코스피 양방향 | KR |
| Small Cap Quant | 소형주 퀀트 | KR |
| Sector Momentum | 섹터 모멘텀 | KR/US |

### 6.3 추가 전략 (선택적)

| 전략 | 설명 | 대상 시장 |
|------|------|---------|
| SPAC No-Loss | 스팩 무손실 전략 | KR |
| All at Once ETF | ETF 일괄 투자 | KR |
| Rotation Savings | 회전 적금 | KR |
| Dual KrStock UsBond | 한국주식+미국채권 | KR+US |

---

### 2.8 종목 스크리닝 (Symbol Screening)

#### 2.8.1 스크리닝 개요
- **목적**: 전체 시장에서 특정 조건을 만족하는 종목을 필터링하여 전략에 활용
- **데이터 소스**:
  - Fundamental 데이터 (PER, PBR, ROE, 시가총액 등)
  - OHLCV 데이터 (가격 변동률, 거래량 등)
  - 심볼 정보 (시장, 거래소, 섹터)
- **활용**:
  - 전략에서 스크리닝 결과를 유니버스로 사용
  - 사용자 정의 스크리닝 조건으로 종목 탐색
  - 프리셋 스크리닝 (가치주, 고배당주, 성장주 등)

#### 2.8.2 Fundamental 필터
- **밸류에이션 지표**:
  - PER (Price to Earnings Ratio): 주가수익비율
  - PBR (Price to Book Ratio): 주가순자산비율
  - PSR (Price to Sales Ratio): 주가매출비율
  - EV/EBITDA: 기업가치 대비 EBITDA
- **수익성 지표**:
  - ROE (Return on Equity): 자기자본이익률
  - ROA (Return on Assets): 총자산이익률
  - Operating Margin: 영업이익률
  - Net Profit Margin: 순이익률
- **배당 지표**:
  - Dividend Yield: 배당수익률
  - Dividend Payout Ratio: 배당성향
- **안정성 지표**:
  - Debt Ratio: 부채비율
  - Current Ratio: 유동비율
  - Quick Ratio: 당좌비율
- **성장성 지표**:
  - Revenue Growth (YoY): 매출 성장률
  - Earnings Growth (YoY): 이익 성장률
  - Revenue Growth (3Y CAGR): 3년 매출 성장률
- **규모 지표**:
  - Market Cap: 시가총액
  - 52주 최고가/최저가

#### 2.8.3 기술적 필터 (OHLCV 기반)
- **가격 변동률**:
  - 1일 변동률 (당일 대비 전일)
  - 5일 변동률 (5거래일 전 대비)
  - 20일 변동률 (한 달 전 대비)
- **거래량 지표**:
  - Volume Ratio: 평균 거래량 대비 현재 거래량 배율
  - 최소 평균 거래량 필터
- **52주 고저가 대비**:
  - 52주 고가 대비 하락률 (예: 고가 대비 10% 이내)
  - 52주 저가 대비 상승률 (예: 저가 대비 20% 이상)

#### 2.8.4 프리셋 스크리닝
| 프리셋 | 설명 | 주요 조건 |
|--------|------|----------|
| 가치주 (Value) | 저평가 우량주 | PER ≤ 15, PBR ≤ 1.0, ROE ≥ 5% |
| 고배당주 (Dividend) | 안정적 고배당 | 배당수익률 ≥ 3%, ROE ≥ 5%, 부채비율 ≤ 100% |
| 성장주 (Growth) | 고성장 기업 | 매출성장률 ≥ 20%, 이익성장률 ≥ 15%, ROE ≥ 10% |
| 스노우볼 (Snowball) | 저PBR + 고배당 | PBR ≤ 1.0, 배당수익률 ≥ 3%, 부채비율 ≤ 80%, ROE ≥ 8% |
| 대형주 (Large Cap) | 시총 상위 | 시가총액 ≥ 10조원 |
| 52주 신저가 근접 | 바닥 매수 전략 | 52주 고가 대비 ≥ 50% 하락, ROE ≥ 5% |

#### 2.8.5 전략 연계
- **코스닥 급등주 (KOSDAQ Fire Rain)**:
  - 거래량 급증 (평균 대비 3배 이상)
  - 가격 상승 (전일 대비 5% 이상)
  - 시가총액 필터 (소형주 중심)
- **스노우볼 전략**:
  - 저PBR + 고배당 스크리닝 결과를 유니버스로 사용
  - 매월 리밸런싱 시 스크리닝 재실행
- **섹터 모멘텀**:
  - 섹터별 상위 모멘텀 종목 스크리닝
  - OHLCV 기반 가격 변동률 정렬

#### 2.8.6 API 엔드포인트
| 엔드포인트 | 메서드 | 설명 |
|-----------|--------|------|
| `/api/v1/screening` | POST | 커스텀 스크리닝 실행 |
| `/api/v1/screening/presets` | GET | 사용 가능한 프리셋 목록 |
| `/api/v1/screening/presets/{preset}` | GET | 프리셋 스크리닝 실행 |
| `/api/v1/screening/momentum` | GET | 모멘텀 기반 스크리닝 |

#### 2.8.7 응답 데이터
- 심볼 기본 정보 (티커, 종목명, 시장, 거래소, 섹터)
- Fundamental 지표 (PER, PBR, ROE, 시가총액, 배당수익률 등)
- 기술적 지표 (현재가, 변동률, 거래량 비율, 52주 고저가 대비)
- 정렬 및 페이지네이션 지원

---

#### 2.8.8 거시 환경 필터 (MacroFilter)

**목적**: USD/KRW 환율, 나스닥 지수 모니터링으로 시장 위험도 평가 및 동적 진입 기준 조정

**3단계 리스크 레벨**:
| 레벨 | 조건 | 조치 |
|------|------|------|
| **Critical** | 환율 ≥ 1400원 OR 나스닥 -2% 이상 | EBS +1, 추천 3개로 제한 |
| **High** | 환율 +0.5% 급등 | EBS +1, 추천 5개로 제한 |
| **Normal** | 기본 상태 | EBS 4, 추천 10개 |

**출력 형식**:
```rust
pub struct MacroEnvironment {
    pub risk_level: MacroRisk,
    pub usd_krw: Decimal,
    pub usd_change_pct: f64,
    pub nasdaq_change_pct: f64,
    pub adjusted_ebs: u8,          // 조정된 EBS 기준
    pub recommendation_limit: usize, // 추천 종목 수 제한
}
```

**데이터 소스**:
- USD/KRW: Yahoo Finance `KRW=X`
- 나스닥: Yahoo Finance `^IXIC`
- 갱신 주기: 1시간

**활용**:
- 전략 진입 차단 (Critical 시 신규 진입 중지)
- Global Score EBS 기준 동적 조정
- 텔레그램 알림 (리스크 상승 시)

**API 엔드포인트**:
- `GET /api/v1/market/macro`: 현재 거시 환경 조회
- 스크리닝 응답에 `macro_risk` 필드 포함

**예상 구현**: v0.6.0 (TODO Phase 1-2.4)

#### 2.8.9 시장 온도 지표 (MarketBreadth)

**목적**: 20일선 상회 종목 비율로 시장 전체 건강 상태 측정

**3단계 온도**:
| 온도 | Above_MA20 비율 | 의미 |
|------|----------------|------|
| Overheat 🔥 | ≥ 65% | 과열 (조정 임박) |
| Neutral 🌤 | 35~65% | 중립 (정상) |
| Cold 🧊 | ≤ 35% | 냉각 (반등 대기) |

**출력 형식**:
```rust
pub struct MarketBreadth {
    pub all: f64,
    pub kospi: f64,
    pub kosdaq: f64,
    pub temperature: MarketTemperature,
}
```

**계산 방식**:
- 전체 종목 중 종가 > SMA(20) 비율
- 시장별 개별 계산 (KOSPI, KOSDAQ)

**활용**:
- 시장 타이밍 (Overheat 시 신규 진입 신중)
- 대시보드 위젯 (시장 온도 게이지)
- 전략 필터링

**API 엔드포인트**:
- `GET /api/v1/market/breadth`: 현재 시장 온도 조회

**예상 구현**: v0.6.0 (TODO Phase 1-2.5)

#### 2.8.10 섹터 분석 (SectorRS)

**목적**: 시장 대비 초과수익(Relative Strength)으로 진짜 주도 섹터 발굴

**계산 방식**:
- `rel_20d_%`: 20일 전 대비 수익률
- `sector_rs`: 섹터 평균 `rel_20d_%`
- `market_rs`: 전체 시장 평균 `rel_20d_%`
- `excess_return`: `sector_rs - market_rs`

**종합 섹터 점수**:
```
score = RS × 0.6 + 단순수익 × 0.4
```

**출력 형식**:
- 스크리닝 응답에 `sector_rs`, `sector_rank` 필드 추가
- 섹터별 순위 (1~11)

**11개 섹터 분류 (GICS)**:
- 에너지, 소재, 산업재, 경기소비재
- 필수소비재, 헬스케어, 금융, IT
- 커뮤니케이션, 유틸리티, 부동산

**활용**:
- 섹터 모멘텀 전략 (상위 3개 섹터 집중)
- 섹터 로테이션 전략
- 대시보드 섹터 히트맵

**API 엔드포인트**:
- `GET /api/v1/market/sectors`: 섹터별 RS 조회

**예상 구현**: v0.6.0 (TODO Phase 1-2.7)

---

### 2.9 종목 랭킹 시스템 (Global Score)

#### 2.9.1 개요
- **목적**: 모든 기술적 지표를 단일 점수(GLOBAL_SCORE 0~100)로 종합하여 종목 순위 산출
- **활용**: 스크리닝 결과 정렬, TOP N 종목 추천, 전략 유니버스 선정

#### 2.9.2 스코어링 팩터 (가중치 합계 = 1.0)
| 팩터 | 가중치 | 설명 |
|------|--------|------|
| Risk/Reward | 0.25 | 목표가 대비 손절가 비율 |
| Target Room | 0.18 | 현재가 대비 목표가 여유율 |
| Stop Room | 0.12 | 현재가 대비 손절가 여유율 |
| Entry Proximity | 0.12 | 추천 진입가 근접도 |
| Momentum | 0.10 | ERS + MACD 기울기 + RSI 중심 보너스 |
| Liquidity | 0.13 | 거래대금 퍼센타일 |
| Technical Balance | 0.10 | 변동성(VolZ) 스윗스팟 + 이격도 안정성 |

#### 2.9.3 페널티 시스템 (점수 차감)
| 조건 | 페널티 | 설명 |
|------|--------|------|
| 5일 과열 | -6점 | 5일 수익률 +10% 초과 시 |
| 10일 과열 | -6점 | 10일 수익률 +20% 초과 시 |
| RSI 이탈 | -4점 | RSI 45~65 밴드 이탈 |
| MACD 음수 | -4점 | MACD 기울기 음수 |
| 진입 괴리 | -4점 | 추천가 대비 현재가 괴리 과다 |
| 저유동성 | -4점 | 거래대금 하위 20% |
| 변동성 스파이크 | -2점 | VolZ > 3 |

#### 2.9.4 유동성 게이트 (시장별)
| 시장 | 최소 거래대금 | 완화 기준 |
|------|--------------|----------|
| KR-KOSPI | 200억원 | 150억원 |
| KR-KOSDAQ | 100억원 | 80억원 |
| US-NYSE/NASDAQ | $100M | $50M |
| JP-TSE | ¥10B | ¥5B |

#### 2.9.5 품질 게이트
- **EBS (Entry Balance Score)**: 진입 조건 균형 점수
- 기본 통과 기준: EBS ≥ 4
- 후보 부족 시 자동 완화: EBS ≥ 3

#### 2.9.6 API 엔드포인트
| 엔드포인트 | 메서드 | 설명 |
|-----------|--------|------|
| `/api/v1/ranking/global` | POST | 글로벌 랭킹 조회 |
| `/api/v1/ranking/top` | GET | TOP N 종목 조회 |

---

#### 2.9.7 추천 검증 (RealityCheck)

**목적**: 전일 추천 종목의 익일 실제 성과 자동 검증

**2개 신규 테이블 (TimescaleDB Hypertable)**:

**price_snapshot 테이블**:
```sql
CREATE TABLE price_snapshot (
    snapshot_date DATE NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    close_price DECIMAL(18,4),
    volume BIGINT,
    global_score DECIMAL(5,2),
    route_state VARCHAR(20),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (snapshot_date, symbol)
);
SELECT create_hypertable('price_snapshot', 'snapshot_date');
```

**reality_check 테이블**:
```sql
CREATE TABLE reality_check (
    check_date DATE NOT NULL,
    recommend_date DATE NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    recommend_rank INT,
    recommend_score DECIMAL(5,2),
    entry_price DECIMAL(18,4),
    next_close DECIMAL(18,4),
    return_pct DECIMAL(8,4),
    is_win BOOLEAN,
    holding_days INT DEFAULT 1,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (check_date, symbol)
);
SELECT create_hypertable('reality_check', 'check_date');
```

**워크플로우**:
1. 매일 종가 시점에 TOP 10 스냅샷 저장 (`price_snapshot`)
2. 익일 종가에 전일 스냅샷과 비교 (`reality_check`)
3. 승률, 평균 수익률 계산

**출력 지표**:
- 추천 종목 승률 (전체, 7일, 30일)
- 평균 수익률
- 최고/최저 수익률
- 레짐별 성과 (MarketRegime 연동)

**활용**:
- 전략 신뢰도 측정
- 백테스트 vs 실거래 괴리 분석
- 파라미터 튜닝 피드백
- 대시보드 성과 위젯

**API 엔드포인트**:
- `GET /api/v1/reality-check/stats`: 통계 조회
- `GET /api/v1/reality-check/history?days=30`: 이력 조회

**예상 구현**: v0.6.0 (TODO Phase 1-2.8)

#### 2.9.8 대시보드 위젯

**시장 심리 지표**:
- `FearGreedGauge`: RSI + Disparity 기반 0~100 게이지
- `MarketBreadthWidget`: 20일선 상회 비율 게이지
- `MacroRiskPanel`: 환율, 나스닥 상태 표시

**팩터 분석 차트**:
- `RadarChart7Factor`: 7개 팩터 레이더 차트
- `ScoreWaterfall`: 점수 기여도 워터폴
- `KellyVisualization`: 켈리 자금관리 바

**포트폴리오 분석**:
- `CorrelationHeatmap`: TOP 10 상관관계 히트맵
- `VolumeProfile`: 매물대 가로 막대 오버레이
- `OpportunityMap`: TOTAL vs TRIGGER 산점도

**상태 관리 UI**:
- `KanbanBoard`: ATTACK/ARMED/WATCH 3열 칸반
- `SurvivalBadge`: 생존일 뱃지 (연속 상위권 일수)
- `RegimeSummaryTable`: 레짐별 평균 성과

**섹터 시각화**:
- `SectorTreemap`: 거래대금 기반 트리맵
- `SectorMomentumBar`: 5일 수익률 Top 10

**예상 구현**: v0.6.0 (TODO Phase 2-5)

---

### 2.10 종목 상태 관리 (RouteState)

#### 2.10.1 상태 정의
| 상태 | 설명 | 액션 |
|------|------|------|
| `ATTACK` | 공략 - 진입 시그널 발생 | 매수 검토 |
| `ARMED` | 임박 - 발사 준비 완료 | 모니터링 강화 |
| `WAIT` | 대기 - 추세 양호, 타점 대기 | 관찰 유지 |
| `OVERHEAT` | 과열 - 단기 급등 | 익절/주의 |
| `NEUTRAL` | 중립 - 특별 신호 없음 | 기본 관찰 |

#### 2.10.2 상태 판정 기준
- **ATTACK**: TTM Squeeze 해제 + 모멘텀 상승 + RSI 적정대
- **ARMED**: 박스권 상단 + 거래량 증가 + 저점 상승
- **WAIT**: 정배열 + MA 지지 + 눌림목
- **OVERHEAT**: 5일 수익률 > 15% 또는 RSI > 70
- **NEUTRAL**: 위 조건 미충족

#### 2.10.3 활용
- 스크리닝 결과에 상태 표시
- 전략에서 상태 기반 필터링
- 알림 시스템 연동 (ATTACK 상태 시 푸시 알림)

---

#### 2.10.4 시장 추세 분류 (MarketRegime)

**목적**: 종목의 추세 단계를 5단계로 분류하여 매매 타이밍 판단

**5단계 레짐**:
| 레짐 | 조건 | 의미 |
|------|------|------|
| StrongUptrend | rel_60d > 10% + slope > 0 + RSI 50~70 | ① 강한 상승 추세 |
| Correction | rel_60d > 5% + slope ≤ 0 | ② 상승 후 조정 |
| Sideways | -5% ≤ rel_60d ≤ 5% | ③ 박스 / 중립 |
| BottomBounce | rel_60d ≤ -5% + slope > 0 | ④ 바닥 반등 시도 |
| Downtrend | rel_60d < -5% + slope < 0 | ⑤ 하락 / 약세 |

**계산 지표**:
- `rel_60d_%`: 60일 전 종가 대비 현재 수익률
- `slope`: 60일 선형 회귀 기울기
- `RSI`: 14일 RSI

**활용**:
- RouteState 판정에 활용 (Downtrend → NEUTRAL 고정)
- 전략 필터링 (Downtrend 종목 진입 차단)
- 스크리닝 API에 `regime` 필드 추가

**API 엔드포인트**:
- `GET /api/v1/market/regime/{symbol}`: 종목별 레짐 조회
- 스크리닝 응답에 `market_regime` 필드 포함

**예상 구현**: v0.6.0 (TODO Phase 1-2.1)

#### 2.10.5 진입 신호 강도 (TRIGGER)

**목적**: 여러 기술적 조건을 종합하여 진입 신호 강도(0~100점) 산출

**6가지 트리거 유형**:
| 트리거 | 점수 | 조건 |
|--------|------|------|
| SqueezeBreak | +30점 | TTM Squeeze 해제 |
| BoxBreakout | +25점 | 박스권 상단 돌파 (Range_Pos ≥ 0.85) |
| VolumeSpike | +20점 | 거래량 평균 대비 3배 이상 |
| MomentumUp | +15점 | MACD 기울기 > 0 |
| HammerCandle | +10점 | 망치형 캔들 패턴 |
| Engulfing | +10점 | 장악형 캔들 패턴 |

**출력 형식**:
```rust
pub struct TriggerResult {
    pub score: f64,              // 0~100 (중복 가능)
    pub triggers: Vec<TriggerType>,
    pub label: String,           // "🚀급등시동, 📦박스돌파"
}
```

**활용**:
- RouteState ATTACK 판정 (TRIGGER ≥ 50점)
- Global Score 모멘텀 팩터에 반영
- 스크리닝 정렬 기준
- 텔레그램 알림 (고강도 신호 발생 시)

**API 엔드포인트**:
- 스크리닝 응답에 `trigger_score`, `trigger_label` 필드 포함

**예상 구현**: v0.6.0 (TODO Phase 1-2.2)

---

### 2.11 호가 단위 관리 (Tick Size)

#### 2.11.1 거래소별 틱 사이즈
| 거래소 | 규칙 | 예시 |
|--------|------|------|
| **KRX** | 가격대별 7단계 | 50,000원 → 100원 틱 |
| **NYSE/NASDAQ** | 고정 $0.01 | 페니 틱 |
| **TSE (일본)** | 가격대별 변동 | ¥3,000 이하 1円 |
| **HKEX** | 가격대별 변동 | HK$0.25~5,000 |

#### 2.11.2 KRX 호가 단위 (7단계)
| 가격대 | 호가 단위 |
|--------|----------|
| 2,000원 미만 | 1원 |
| 2,000원 ~ 5,000원 미만 | 5원 |
| 5,000원 ~ 20,000원 미만 | 10원 |
| 20,000원 ~ 50,000원 미만 | 50원 |
| 50,000원 ~ 200,000원 미만 | 100원 |
| 200,000원 ~ 500,000원 미만 | 500원 |
| 500,000원 이상 | 1,000원 |

#### 2.11.3 활용
- 주문 가격 유효성 검증
- 목표가/손절가 자동 반올림
- 슬리피지 계산

---

## 7. 핵심 워크플로우

### 7.1 전략 개발 워크플로우

```
[1] 전략 등록 (Strategies.tsx)
    - 기본 전략 선택
    - 파라미터 커스터마이징
    - 리스크 설정
         ↓
[2] 백테스트 (Backtest.tsx)
    - 과거 데이터로 전략 검증
    - 성과 지표 분석
    - (필요시 파라미터 조정 → 1번 반복)
         ↓
[3] 시뮬레이션 (Simulation.tsx)
    - 실시간 데이터로 모의 거래
    - 실제 시장 환경 검증
         ↓
[4] 실전 운용 (Dashboard)
    - 검증된 전략 활성화
    - 실제 거래 실행
    - 포트폴리오 모니터링
```

### 7.2 데이터 흐름

```
Yahoo Finance / Binance / KIS
         ↓
    [데이터 수집]
         ↓
    TimescaleDB (OHLCV 저장)
         ↓
    [전략 엔진] ← 실시간 시세 (WebSocket)
         ↓
    [주문 실행] → 거래소 API
         ↓
    [알림] → Telegram
```

---

## 8. 참고 문서

| 문서 | 위치 | 용도 |
|------|------|------|
| CLAUDE.md | 프로젝트 루트 | 프로젝트 구조, API 검증 가이드, 에이전트 지침 |
| todo.md | docs/ | 작업 관리, 진행 상황 추적 |
| improvement_todo.md | docs/ | 코드베이스 개선 로드맵 |

---

*버전 이력: v1.0 → v2.0 → v2.1 → v3.0 → v4.0 → v4.1 → v5.0 → v5.1 → v5.2 → v6.0*

**v6.0 변경사항:**
- SignalMarker (신호 기록) 요구사항 추가 (2.2.5)
- 신호 시각화 (캔들 차트 오버레이) 요구사항 추가 (2.2.6)
- TTM Squeeze 지표 요구사항 추가 (2.7.5)
- 추가 기술적 지표 (HMA, OBV, SuperTrend, CandlePattern) 요구사항 추가 (2.7.6)
- MacroFilter (거시 환경 필터) 요구사항 추가 (2.8.8)
- MarketBreadth (시장 온도 지표) 요구사항 추가 (2.8.9)
- SectorRS (섹터 분석) 요구사항 추가 (2.8.10)
- RealityCheck (추천 검증) 요구사항 추가 (2.9.7)
- 대시보드 위젯 요구사항 추가 (2.9.8)
- MarketRegime (시장 추세 분류) 요구사항 추가 (2.10.4)
- TRIGGER (진입 신호 강도) 요구사항 추가 (2.10.5)

**v5.2 변경사항:**
- 종목 랭킹 시스템 (Global Score) 요구사항 추가
- 종목 상태 관리 (RouteState) 요구사항 추가
- 호가 단위 관리 (Tick Size) 요구사항 추가
- ML 구조적 피처 (Structural Features) 요구사항 추가

**v5.1 변경사항:**
- 심볼 자동 동기화 기능 추가 (KRX, Binance, Yahoo Finance)
- Fundamental 데이터 백그라운드 수집 기능 추가
- OHLCV 증분 업데이트 기능 추가
