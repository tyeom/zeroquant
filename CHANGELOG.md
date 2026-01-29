# Changelog

프로젝트의 모든 주요 변경 사항을 기록합니다.

형식은 [Keep a Changelog](https://keepachangelog.com/ko/1.0.0/)를 따르며,
[Semantic Versioning](https://semver.org/lang/ko/)을 준수합니다.

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

### [0.2.0] - 예정
- 매매일지 (Trading Journal) 기능
- 다중 자산 백테스트 지원
- WebSocket 이벤트 브로드캐스트 완성

### [0.3.0] - 예정
- 추가 거래소 통합 (Coinbase, 키움증권)
- 성능 최적화 및 부하 테스트
