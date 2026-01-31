# 작업 규칙

- Context7과 Sequential Thinking을 적극적으로 사용하세요.
- 모든 작업 수행시 UI와 API의 필드 매칭을 무조건 맞추고 진행 하세요.
- API는 무조건 호출하여 정상작동 하는지 테스트 합니다.
- UI는 playwright를 이용하여 항상 동작 확인을 수행합니다.
- 작업의 완료는 API, 구조, UI 모두 정상적일때 완료입니다.

---

## 실행 환경

### Docker 구성 (인프라 + ML만)
```bash
docker-compose up -d timescaledb redis  # 인프라
docker-compose --profile ml run --rm trader-ml python scripts/train_ml_model.py  # ML
```

### 로컬 실행
```bash
# API 서버
export DATABASE_URL=postgresql://trader:trader_secret@localhost:5432/trader
export REDIS_URL=redis://localhost:6379
cargo run --bin trader-api --features ml --release

# 프론트엔드
cd frontend && npm run dev
```

---

## 🔴 미구현 작업

### 1. 매매 일지 (Trading Journal) ⭐ 신규

**백엔드**
- [ ] DB 스키마: `trade_executions`, `position_snapshots`
- [ ] 종목별 포지션 집계 (평균 매입가, 가중평균)
- [ ] API 엔드포인트
  - `GET /api/v1/journal/positions`
  - `GET /api/v1/journal/executions`
  - `GET /api/v1/journal/pnl`
  - `POST /api/v1/journal/sync`

**프론트엔드**
- [ ] TradingJournal.tsx 페이지
- [ ] 보유 현황 테이블
- [ ] 체결 내역 타임라인
- [ ] 포지션 비중 차트
- [ ] 손익 분석 대시보드

### 2. 텔레그램 봇 명령어 (낮음)

현재: 푸시 알림만 구현됨

- [ ] Bot API 명령어 핸들러
- [ ] `/portfolio` - 포트폴리오 조회
- [ ] `/status` - 전략 실행 상태
- [ ] `/stop <id>` - 전략 중지
- [ ] `/report` - 리포트 생성

### 3. 미구현 전략 (4개, 선택적)

- [ ] SPAC No-Loss (KR)
- [ ] All at Once ETF (KR)
- [ ] Rotation Savings (KR)
- [ ] Dual KrStock UsBond (KR+US)

### 4. 종목 스크리닝 (Symbol Screening) ⭐ 진행중

**백엔드**
- [x] ScreeningRepository 구현
- [x] ScreeningFilter 조건 모델 정의
- [x] 프리셋 스크리닝 (value, dividend, growth, snowball, large_cap, near_52w_low)
- [x] 스크리닝 API 라우트 구현
  - `POST /api/v1/screening` - 커스텀 스크리닝 ✅
  - `GET /api/v1/screening/presets` - 프리셋 목록 ✅
  - `GET /api/v1/screening/presets/{preset}` - 프리셋 실행 ✅
  - `GET /api/v1/screening/momentum` - 모멘텀 스크리닝 ✅
- [x] 모멘텀 스크리닝 최적화 (OHLCV 기반 가격/거래량 분석)
- [ ] Fundamental 데이터 수집 (symbol_info → symbol_fundamental)

**전략 연계**
- [ ] 전략에서 스크리닝 결과 활용 인터페이스 정의
- [ ] 코스닥 급등주 전략: 스크리닝 연동
- [ ] 스노우볼 전략: 저PBR+고배당 스크리닝 연동
- [ ] 섹터 모멘텀 전략: 섹터별 상위 종목 스크리닝

**프론트엔드**
- [ ] Screening.tsx 페이지
- [ ] 필터 조건 입력 폼
- [ ] 프리셋 선택 UI
- [ ] 스크리닝 결과 테이블 (정렬, 페이지네이션)
- [ ] 종목 상세 모달 (Fundamental + 차트)

### 5. ML 예측 활용 (선택적)

- [ ] 전략에서 ML 예측 결과 사용
- [ ] Docker ML 훈련 End-to-End 테스트

---

## 🟡 코드 최적화 기회

### Backend API
- [ ] portfolio.rs:441 - 당일 손익 계산
- [ ] portfolio.rs:461 - 당일 수익률 계산
- [ ] OAuth 토큰 캐시 TTL 관리

### 전략 모듈
- [ ] 대형 파일 리팩토링 (xaa.rs, candle_pattern.rs, rsi.rs)
- [ ] 캔들 패턴 매칭 성능 최적화

### 거래소 모듈
- [ ] Binance WebSocket 완성
- [ ] KIS 선물/옵션 거래

### 백테스트/분석
- [ ] 틱 시뮬레이션
- [ ] 마진 거래 검증
- [ ] 대규모 데이터셋 성능 테스트

---

## 🟢 낮은 우선순위

### 추가 거래소
- [ ] Coinbase, Kraken
- [ ] Interactive Brokers
- [ ] 키움증권

### 인프라
- [ ] Grafana 모니터링
- [ ] 부하 테스트

---

## 🔵 핵심 워크플로우

```
전략 등록 (Strategies.tsx)
    ↓
백테스트 (Backtest.tsx) ← 데이터셋 자동 요청
    ↓
시뮬레이션 (Simulation.tsx)
    ↓
실전 운용 (Dashboard)
```

---

## ✅ 완료 현황 요약

| 모듈 | 상태 |
|------|------|
| Backend API (17개 라우트) | 95% |
| Frontend (7 페이지, 15+ 컴포넌트) | 95%+ |
| 전략 (25개 구현) | 100% |
| ML (훈련 + ONNX 추론) | 95% |
| 거래소 (Binance, KIS) | 85-95% |
| 테스트 (258개 단위 + 28개 통합) | 완료 |

> 상세 구조는 `CLAUDE.md` 참조

---

## 참고 문서

| 문서 | 위치 | 용도 |
|------|------|------|
| PRD | `docs/prd.md` | 제품 요구사항 정의서 |
| CLAUDE.md | 루트 | 프로젝트 구조, 에이전트 지침 |
| improvement_todo.md | `docs/` | 코드베이스 개선 로드맵 |
