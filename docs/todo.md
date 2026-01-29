
# 작업 규칙
- Context7과 Sequential Thinking, Shrimp Task Manager를 적극적으로 사용하세요.
- 모든 작업 수행시 UI와 API의 필드 매칭을 무조건 맞추고 진행 하세요.
- API는 무조건 호출하여 정상작동 하는지 테스트 합니다. 문제가 발생했을 때 수정 후 넘어가세요. 
- UI는 playwright를 이용하여 항상 동작 확인을 수행합니다. 적당한 형태의 테스트 케이스를 만들어, 통과하도록 하세요.
- UI와 API가 모두 끝나야 작업이 끝나는 것입니다. API와 UI 테스트 도중 문제가 생기면 바로바로 해결하세요.
- docker 환경에서 반드시 테스트 할 것. 실제 환경은 docker를 사용합니다.
- 작업의 완료는 확인 해야할 모든 요소가 정상적일때 완료라고 합니다. 확인 해야 할 요소는 API, 구조, UI입니다.
---

# 완료된 작업 (2026-01-29)

## UI/UX 작업
- [x] 전략 편집 모달 UI 구현
- [x] 토스트 알림 컴포넌트 구현 (Toast.tsx)
- [x] Strategies 페이지에 토스트 적용
- [x] Docker 빌드 문제 해결 (Rust 1.93 + ort 2.0.0-rc.11)

---

# 우선순위 작업 목록

## [최우선] ML 패턴 인식 고도화
- [ ] 기존 ML 모듈 분석 (trader-analytics/src/ml/)
- [ ] 패턴 인식 알고리즘 개선
- [ ] ONNX 모델 추론 테스트

## [최우선] KIS API 연동 완성 및 테스트
- [ ] OAuth 2.0 인증 구현
- [ ] 국내 주식/ETF 시세 조회
- [ ] 주문 실행 (매수/매도)
- [ ] 실시간 WebSocket 연동 (국내)
- [ ] 해외 주식 API 연동 (미국 ETF)
- [ ] 모의투자 계좌 테스트

## [최우선] Frontend 실시간 알림 UI
- [ ] WebSocket 클라이언트 훅 구현
- [ ] 실시간 주문 상태 알림
- [ ] 전략 이벤트 알림
- [ ] 시스템 경고 알림

---

# 중간 우선순위 작업

## Frontend 거래소 자격증명 관리 UI
- [ ] 거래소 API 키 등록/수정/삭제 UI
- [ ] 연결 테스트 기능

## Frontend 텔레그램 자격증명 관리 UI
- [ ] 봇 토큰 및 Chat ID 등록 UI
- [ ] 연결 테스트 메시지 전송

## Frontend 관심 종목 등록/삭제 UI
- [ ] 종목 검색 기능
- [ ] 드래그앤드롭 순서 변경

## Frontend 전략 파라미터 설정 폼
- [x] DynamicForm 컴포넌트 완료
- [x] 전략 편집 모달 완료

---

# 백테스트 테스트 현황 (2026-01-29)

## 완료된 작업
- [x] SMA 크로스오버 전략 생성 (sma.rs)
- [x] Magic Split 전략 설정 수정 (levels 필드 추가)
- [x] 복잡한 전략 Config Default 적용 (SimplePowerConfig, HaaConfig, XaaConfig, StockRotationConfig)
- [x] 프론트엔드 최종 자본 표시 수정
- [x] 볼린저 밴드 파라미터 수정 (std_dev → std_multiplier, RSI 비활성화)
- [x] 변동성 돌파 is_new_period 날짜 비교 로직 추가

## 단일 자산 전략 테스트 결과 (✅ 모두 동작)
| 전략 | 상태 | 거래 수 | 수익률 | 비고 |
|-----|------|--------|--------|-----|
| RSI 평균회귀 | ✅ 동작 | 1회 | - | 005930 테스트 |
| 그리드 트레이딩 | ✅ 동작 | 17회 | +7.90% | 횡보장 최적 |
| 볼린저 밴드 | ✅ 동작 | 3회 | -0.58% | std_multiplier: 1.5 |
| 변동성 돌파 | ✅ 동작 | 28회 | -2.60% | k_factor: 0.3, ATR 사용 |
| Magic Split | ✅ 동작 | 13회 | -0.69% | 305540 테스트 |
| 이동평균 크로스오버 | ✅ 동작 | 6회 | +9.38% | 추세 추종 |

## 다중 자산 전략 (미지원 - 백테스트 엔진 수정 필요)
| 전략 | 필요 심볼 | 상태 |
|-----|---------|------|
| Simple Power | TQQQ, SCHD, PFIX, TMF | ⏳ 다중 심볼 지원 필요 |
| HAA | TIP, SPY, IWM, VEA, VWO, TLT, IEF, PDBC, VNQ, BIL | ⏳ 다중 심볼 지원 필요 |
| XAA | VWO, BND, SPY, EFA, EEM, TLT, IEF, LQD, BIL | ⏳ 다중 심볼 지원 필요 |
| 종목 갈아타기 | 005930, 000660, 035420, 051910, 006400 | ⏳ 다중 심볼 지원 필요 |

## 남은 백테스트 작업
- [ ] 다중 자산 백테스트 API 엔드포인트 구현 (/api/v1/backtest/run-multi)
- [ ] 다중 심볼 데이터 로딩 함수 구현

---

# 낮은 우선순위 작업

## 추가 거래소 통합
- [ ] Coinbase 거래소
- [ ] Kraken 거래소
- [ ] Interactive Brokers
- [ ] Oanda (외환)
- [ ] 키움증권 (Windows COM)
- [ ] 이베스트투자증권

## 인프라 & 모니터링
- [ ] Grafana 모니터링 대시보드
- [ ] 성능 및 부하 테스트

## Python 전략 변환
- [ ] 22개 미구현 전략

---

# 참고 문서
- 테스트 시나리오: [docs/backtest-test-scenarios.md](docs/backtest-test-scenarios.md)
- PRD: [plan file](C:\Users\HP\.claude\plans\toasty-chasing-patterson.md)
