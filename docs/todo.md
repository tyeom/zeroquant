# ZeroQuant TODO - 통합 로드맵

> **마지막 업데이트**: 2026-02-01
> **현재 버전**: v0.5.5
> **참조 문서**: `python_strategy_modules.md`, `improvement_todo.md`

---

## 📋 목차

1. [⚙️ Phase 0 - 기반 작업 (Foundation)](#️-phase-0---기반-작업-foundation)
2. [🔴 Phase 1 - 핵심 기능 (Core Features)](#-phase-1---핵심-기능-core-features)
3. [🟡 Phase 2 - 프론트엔드 UI](#-phase-2---프론트엔드-ui)
4. [🟢 Phase 3 - 품질/성능 개선](#-phase-3---품질성능-개선)
5. [🟣 Phase 4 - 선택적/낮은 우선순위](#-phase-4---선택적낮은-우선순위)
6. [✅ 완료 현황](#-완료-현황)

---

## 📊 의존성 다이어그램

```
┌──────────────────────────────────────────────────────────────────────┐
│                    Phase 0: Foundation (3주)                          │
│                                                                       │
│  ┌────────────────┐  ┌────────────────┐  ┌──────────────────┐       │
│  │ 전략 레지스트리  │  │ 공통 로직 추출  │  │ StrategyContext  │       │
│  │ (자동등록)      │  │ (26개 전략)    │  │ (거래소 컨텍스트) │       │
│  └───────┬────────┘  └───────┬────────┘  └────────┬─────────┘       │
│          │                   │                    │                  │
│          │                   │           ┌───────┴───────┐          │
│          ▼                   │           ▼               ▼          │
│  ┌────────────────────┐      │    ┌────────────┐  ┌────────────┐    │
│  │ SDUI 자동 생성 ⭐   │      │    │TickSize   │  │ 포지션 공유 │    │
│  │ ┌────────────────┐ │      │    │Provider   │  │ 충돌 방지  │    │
│  │ │FragmentRegistry│ │      │    └────────────┘  └────────────┘    │
│  │ │SchemaComposer  │ │      │                                       │
│  │ │#[derive(Config)]│      │                                       │
│  │ └────────────────┘ │      │                                       │
│  └────────────────────┘      │                                       │
│          │                   │                                       │
│  ┌───────┴───────────────────┴─────────────────────────────────┐    │
│  │            Journal-Backtest 공통 모듈 ⭐ 신규                 │    │
│  │  ┌──────────────┐  ┌──────────────┐  ┌────────────────────┐ │    │
│  │  │ calculations │  │ statistics   │  │ UnifiedTrade trait │ │    │
│  │  │ (P&L 계산)   │  │ (승률,PF 등) │  │ (타입 통합)        │ │    │
│  │  └──────────────┘  └──────────────┘  └────────────────────┘ │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                                                                       │
└──────────┬───────────────────┬───────────────────────────────────────┘
           │                   │
           ▼                   ▼
┌──────────────────────────────────────────────────────────────────────┐
│                    Phase 1: Core Features (2.5주)                     │
│                                                                       │
│  ┌─────────────────────┐                                             │
│  │ StructuralFeatures  │ ← 공통 로직에서 피처 계산 재사용            │
│  └──────────┬──────────┘                                             │
│             ▼                                                         │
│  ┌─────────────────────┐                                             │
│  │     RouteState      │ ← StructuralFeatures 기반 상태 판정         │
│  └──────────┬──────────┘                                             │
│             ▼                                                         │
│  ┌─────────────────────┐     ┌─────────────────────────┐             │
│  │    Global Score     │     │  SignalMarker ⭐ 신규   │             │
│  │ (RouteState+TickSize│     │  (기술 신호 저장)       │             │
│  └──────────┬──────────┘     │  - indicators 값 기록   │             │
│             │                │  - 백테스트/실거래 공용 │             │
│             ▼                └───────────┬─────────────┘             │
│  ┌─────────────────────┐                 │                           │
│  │    전략 연계        │ ←───────────────┘                           │
│  │ (스크리닝 + 포지션) │   ↑ 공통 통계 모듈 재사용                   │
│  └─────────────────────┘                                             │
└──────────────────────────────────────────────────────────────────────┘
           │
           ▼
┌──────────────────────────────────────────────────────────────────────┐
│                    Phase 2: Frontend UI (3.5주)                       │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐              │
│  │ Journal UI  │  │ Screening UI│  │ Global Ranking  │              │
│  │             │  │             │  │                 │              │
│  │ 공통 통계   │  │             │  │                 │              │
│  │ 모듈 재사용 │  │             │  │                 │              │
│  └──────┬──────┘  └─────────────┘  └─────────────────┘              │
│         │                                                            │
│         ▼                                                            │
│  ┌────────────────────────────────────────────┐                     │
│  │   캔들 차트 신호 시각화 ⭐ 신규             │                     │
│  │  ┌────────────────┐  ┌────────────────┐   │                     │
│  │  │SignalOverlay   │  │IndicatorFilter │   │                     │
│  │  │(진입/청산 표시)│  │(RSI,MACD필터) │   │                     │
│  │  └────────────────┘  └────────────────┘   │                     │
│  └────────────────────────────────────────────┘                     │
└──────────────────────────────────────────────────────────────────────┘
```

### 🔑 StrategyContext의 핵심 역할

```
┌──────────────────────────────────────────────────────────────────────┐
│                         데이터 소스                                   │
│  ┌────────────────┐              ┌────────────────────────────────┐  │
│  │  거래소 API    │              │      분석 엔진                  │  │
│  │  (Binance,KIS) │              │  (GlobalScorer, RouteState)    │  │
│  └───────┬────────┘              └───────────────┬────────────────┘  │
│          │                                       │                   │
│          ▼                                       ▼                   │
│  ┌────────────────┐              ┌────────────────────────────────┐  │
│  │ExchangeProvider│              │     AnalyticsProvider          │  │
│  │ - 계좌 정보    │              │ - Global Score                 │  │
│  │ - 포지션       │              │ - RouteState                   │  │
│  │ - 미체결 주문  │              │ - Screening 결과               │  │
│  └───────┬────────┘              │ - StructuralFeatures           │  │
│          │                       └───────────────┬────────────────┘  │
│          │                                       │                   │
│          └───────────────┬───────────────────────┘                   │
│                          ▼                                           │
└──────────────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌──────────────────────────────────────────────────────────────────────┐
│                      StrategyContext                                  │
│        (전략 간 공유되는 통합 컨텍스트 - Arc<RwLock<>>)               │
├──────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  ┌─────────────────────────┐      ┌─────────────────────────────┐   │
│  │  거래소 정보 (1~5초)     │      │  분석 결과 (1~10분)          │   │
│  │  - AccountInfo          │      │  - global_scores            │   │
│  │  - positions            │      │  - route_states             │   │
│  │  - pending_orders       │      │  - screening_results        │   │
│  │  - exchange_constraints │      │  - structural_features      │   │
│  └────────────┬────────────┘      └──────────────┬──────────────┘   │
│               │                                   │                  │
│               └─────────────┬─────────────────────┘                  │
│                             ▼                                        │
│              ┌──────────────────────────────┐                        │
│              │       충돌 방지 + 의사결정    │                        │
│              │  - 중복 주문 차단             │                        │
│              │  - 잔고/포지션 한도 체크      │                        │
│              │  - Global Score 기반 종목 선택│                        │
│              │  - RouteState 기반 진입/청산  │                        │
│              └──────────────────────────────┘                        │
│                             │                                        │
│         ┌───────────────────┼───────────────────┐                    │
│         ▼                   ▼                   ▼                    │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐            │
│  │ 전략 A      │     │ 전략 B      │     │ 전략 C      │            │
│  │ (RSI)       │     │ (Grid)      │     │ (Momentum)  │            │
│  │             │     │             │     │             │            │
│  │ ctx.route_  │     │ ctx.account │     │ ctx.global_ │            │
│  │ states 활용 │     │ .available  │     │ scores 활용 │            │
│  └─────────────┘     └─────────────┘     └─────────────┘            │
│                                                                       │
└──────────────────────────────────────────────────────────────────────┘
```

---

## ⚙️ Phase 0 - 기반 작업 (Foundation)

> **🎯 핵심 원칙**: 합칠 수 있는 기능은 합치고, 재활용할 수 있는 코드는 재활용한다.
>
> **왜 먼저?** 이 작업들이 완료되면 이후 모든 기능 구현이 훨씬 수월해집니다.
> - 공통 로직 추출 → 새 전략/기능 추가 시 보일러플레이트 80% 감소
> - Journal-Backtest 통합 → P&L 계산 로직 1곳에서 관리, 버그 수정 범위 축소
> - 레지스트리 패턴 → 모든 전략에 새 기능(RouteState, GlobalScore) 일괄 적용 가능
>
> **예상 시간**: 3주 (96시간) - SDUI 시스템 포함
> **핵심 효과**: 코드 중복 40-50% 감소, 사이드 이펙트 최소화, 유지보수 용이성 증대, UI 자동 생성

### 1. 전략 레지스트리 패턴 ⭐ 최우선

**현재 문제**: 새 전략 추가 시 **5곳 이상 수정** 필요
- `strategies/mod.rs` - pub mod, pub use
- `routes/strategies.rs` - 팩토리 함수 4개
- `routes/backtest/engine.rs` - match arm
- `config/sdui/strategy_schemas.json` - UI 스키마
- `frontend/Strategies.tsx` - 타임프레임 매핑

**개선 후**: 전략 파일 **1곳만 수정**

**구현 항목**
- [ ] `inventory` crate 도입 (컴파일 타임 등록)
- [ ] `StrategyMeta` 구조체 정의
  ```rust
  pub struct StrategyMeta {
      pub id: &'static str,
      pub name: &'static str,           // 한글 이름
      pub description: &'static str,
      pub default_timeframe: &'static str,
      pub default_symbols: &'static [&'static str],
      pub category: StrategyCategory,   // Realtime/Intraday/Daily/Monthly
      pub factory: fn() -> Box<dyn Strategy>,
  }
  ```
- [ ] `register_strategy!` 매크로 구현
  ```rust
  register_strategy! {
      id: "rsi_mean_reversion",
      name: "RSI 평균회귀",
      timeframe: "15m",
      category: Intraday,
      type: RsiStrategy
  }
  ```
- [ ] 팩토리 함수 자동화 (`create_strategy_instance()` 등)
- [ ] `GET /api/v1/strategies/meta` API (프론트엔드 동적 조회)
- [ ] 기존 26개 전략 마이그레이션

**효과**:
- 전략 추가 시간: 2시간 → 30분
- Global Score, RouteState를 전략에 쉽게 연동 가능
- 새 피처(StructuralFeatures) 모든 전략에 일괄 적용 가능

**예상 시간**: 28시간 (3.5일)

---

### 2. 전략 공통 로직 추출

**현재 문제**: 26개 전략이 유사한 코드 패턴 반복

**개선 구조**
```
strategies/common/
├── mod.rs
├── position_sizing.rs    # 켈리, 고정비율, ATR 기반 사이징
├── risk_checks.rs        # 최대 포지션, 일일 손실 한도
├── signal_filters.rs     # 노이즈 필터, 확인 신호
├── entry_exit.rs         # 진입/청산 공통 로직
├── indicators.rs         # 기술적 지표 계산 (공용)
└── position_sync.rs      # ✅ 구현 완료 (v0.5.5)
```

**구현 항목**
- [ ] `PositionSizer` trait 및 구현체
  ```rust
  pub trait PositionSizer {
      fn calculate_size(&self, capital: Decimal, risk: &RiskParams) -> Decimal;
  }
  pub struct KellyPositionSizer { /* ... */ }
  pub struct FixedRatioSizer { /* ... */ }
  ```
- [ ] `RiskChecker` trait 및 공통 체크
- [ ] `SignalFilter` trait (노이즈 필터링)
- [ ] 공용 지표 계산 함수 (RSI, MACD, BB 등)

**효과**:
- StructuralFeatures 계산 로직을 공통 모듈에서 재사용
- 새 전략 개발 시 보일러플레이트 80% 감소
- 버그 수정 시 한 곳만 수정

**예상 시간**: 12시간 (1.5일)

---

### 3. StrategyContext (전략 실행 컨텍스트) ⭐ 신규

**목적**: 전략이 거래소 정보와 현재 포지션 상태를 실시간으로 조회하여 의사결정에 활용

**현재 문제**:
- 각 전략이 포지션을 독립적으로 관리 → 전략 간 포지션 정보 공유 불가
- 거래소 실시간 잔고 조회 기능 부재 → 실제 매수 가능 금액 알 수 없음
- 미체결 주문 상태 모름 → 중복 주문 위험

**구현 항목**
- [ ] `StrategyContext` 구조체 정의
  ```rust
  pub struct StrategyContext {
      // ===== 거래소 실시간 정보 =====
      /// 계좌 정보 (거래소에서 실시간 조회)
      pub account: AccountInfo,
      /// 현재 보유 포지션 (전략 간 공유)
      pub positions: HashMap<Symbol, PositionInfo>,
      /// 미체결 주문 목록
      pub pending_orders: Vec<PendingOrder>,
      /// 거래소 제약 조건
      pub exchange_constraints: ExchangeConstraints,

      // ===== 외부 분석 결과 (주기적 갱신) =====
      /// Global Score 랭킹 결과
      pub global_scores: HashMap<Symbol, GlobalScoreResult>,
      /// RouteState 상태 정보
      pub route_states: HashMap<Symbol, RouteState>,
      /// 스크리닝 결과 (프리셋별)
      pub screening_results: HashMap<ScreeningPreset, Vec<ScreeningResult>>,
      /// 구조적 피처 캐시
      pub structural_features: HashMap<Symbol, StructuralFeatures>,

      // ===== 메타 정보 =====
      /// 마지막 거래소 동기화 시간
      pub last_exchange_sync: DateTime<Utc>,
      /// 마지막 분석 갱신 시간
      pub last_analysis_sync: DateTime<Utc>,
  }
  ```
- [ ] `AccountInfo` - 실시간 계좌 정보
  ```rust
  pub struct AccountInfo {
      pub total_balance: Decimal,       // 총 자산
      pub available_balance: Decimal,   // 매수 가능 금액
      pub margin_used: Decimal,         // 사용 중인 증거금
      pub unrealized_pnl: Decimal,      // 미실현 손익 합계
  }
  ```
- [ ] `PositionInfo` - 포지션 상세 정보
  ```rust
  pub struct PositionInfo {
      pub symbol: Symbol,
      pub side: Side,
      pub quantity: Decimal,
      pub avg_entry_price: Decimal,
      pub current_price: Decimal,       // 실시간 시세
      pub unrealized_pnl: Decimal,
      pub unrealized_pnl_pct: Decimal,  // 수익률 %
      pub liquidation_price: Option<Decimal>,  // 청산가 (레버리지)
  }
  ```
- [ ] `ExchangeConstraints` - 거래소 제약
  ```rust
  pub struct ExchangeConstraints {
      pub tick_size: TickSizeProvider,
      pub min_order_qty: Decimal,
      pub max_leverage: Option<Decimal>,
      pub trading_hours: Option<TradingHours>,
  }
  ```
- [ ] `ExchangeProvider` trait (거래소별 구현)
  ```rust
  #[async_trait]
  pub trait ExchangeProvider: Send + Sync {
      async fn fetch_account(&self) -> Result<AccountInfo>;
      async fn fetch_positions(&self) -> Result<Vec<PositionInfo>>;
      async fn fetch_pending_orders(&self) -> Result<Vec<PendingOrder>>;
  }
  ```
- [ ] `AnalyticsProvider` trait (분석 결과 주입)
  ```rust
  #[async_trait]
  pub trait AnalyticsProvider: Send + Sync {
      /// Global Score 조회 (시장별)
      async fn fetch_global_scores(&self, market: Market) -> Result<Vec<GlobalScoreResult>>;
      /// RouteState 조회
      async fn fetch_route_states(&self, symbols: &[Symbol]) -> Result<HashMap<Symbol, RouteState>>;
      /// 스크리닝 결과 조회
      async fn fetch_screening(&self, preset: ScreeningPreset) -> Result<Vec<ScreeningResult>>;
      /// 구조적 피처 조회
      async fn fetch_features(&self, symbols: &[Symbol]) -> Result<HashMap<Symbol, StructuralFeatures>>;
  }
  ```
- [ ] `ContextSyncService` - 주기적 동기화 서비스
  ```rust
  pub struct ContextSyncService {
      exchange_provider: Box<dyn ExchangeProvider>,
      analytics_provider: Box<dyn AnalyticsProvider>,
      context: Arc<RwLock<StrategyContext>>,
      exchange_sync_interval: Duration,  // 1~5초
      analytics_sync_interval: Duration, // 1~10분
  }

  impl ContextSyncService {
      pub async fn run(&self, shutdown: CancellationToken) {
          loop {
              tokio::select! {
                  _ = tokio::time::sleep(self.exchange_sync_interval) => {
                      self.sync_exchange().await;
                  }
                  _ = shutdown.cancelled() => break,
              }
          }
      }
  }
  ```

**Strategy trait 확장**
```rust
pub trait Strategy: Send + Sync {
    // 기존 메서드들...

    /// 컨텍스트 주입 (엔진에서 호출)
    fn set_context(&mut self, ctx: Arc<RwLock<StrategyContext>>);

    /// 포지션 기반 의사결정 (선택적 구현)
    fn should_adjust_position(&self, position: &PositionInfo) -> Option<PositionAdjustment> {
        None  // 기본: 조정 안 함
    }
}
```

**활용 예시**:

```rust
// 예시 1: 물타기 전략 (포지션 기반)
fn should_adjust_position(&self, pos: &PositionInfo) -> Option<PositionAdjustment> {
    if pos.unrealized_pnl_pct < dec!(-5) {  // -5% 손실 시
        Some(PositionAdjustment::AddToPosition {
            quantity: pos.quantity * dec!(0.5),  // 50% 추가 매수
            reason: "물타기".to_string(),
        })
    } else {
        None
    }
}

// 예시 2: Global Score 기반 종목 선택
fn select_targets(&self, ctx: &StrategyContext) -> Vec<Symbol> {
    ctx.global_scores.iter()
        .filter(|(_, score)| score.global_score >= 80.0)  // 80점 이상
        .filter(|(symbol, _)| {
            // RouteState가 ATTACK 또는 ARMED인 종목만
            matches!(
                ctx.route_states.get(*symbol),
                Some(RouteState::Attack) | Some(RouteState::Armed)
            )
        })
        .map(|(symbol, _)| symbol.clone())
        .take(10)  // TOP 10
        .collect()
}

// 예시 3: 스크리닝 결과 기반 진입 (코스닥 급등주 전략)
fn generate_signals(&mut self) -> Vec<Signal> {
    let ctx = self.context.read().await;

    // 스크리닝 결과에서 모멘텀 상위 종목 조회
    let candidates = ctx.screening_results
        .get(&ScreeningPreset::Momentum)
        .unwrap_or(&vec![]);

    candidates.iter()
        .filter(|r| {
            // ATTACK 상태 + 이미 보유하지 않은 종목
            ctx.route_states.get(&r.symbol) == Some(&RouteState::Attack)
                && !ctx.positions.contains_key(&r.symbol)
        })
        .map(|r| Signal::buy(&r.symbol, r.current_price))
        .collect()
}

// 예시 4: OVERHEAT 상태 자동 익절
fn check_overheat_exit(&self, ctx: &StrategyContext) -> Vec<Signal> {
    ctx.positions.iter()
        .filter(|(symbol, _)| {
            ctx.route_states.get(*symbol) == Some(&RouteState::Overheat)
        })
        .map(|(symbol, pos)| Signal::sell(symbol, pos.current_price))
        .collect()
}
```

**효과**:

| 카테고리 | 효과 |
|----------|------|
| **거래소 연동** | 실시간 잔고/포지션으로 유효한 주문만 생성 |
| **충돌 방지** | 전략 간 포지션 공유로 중복 주문/반대 포지션 차단 |
| **포지션 관리** | 물타기, 부분 익절, 리밸런싱 등 동적 조절 가능 |
| **분석 결과 활용** | Global Score 기반 자동 종목 선택 |
| **상태 기반 매매** | RouteState(ATTACK/OVERHEAT)로 진입/청산 자동화 |
| **스크리닝 연동** | 스크리닝 결과를 전략에서 직접 조회하여 활용 |

**예상 시간**: 20시간 (2.5일) - AnalyticsProvider 포함

---

### 4. TickSizeProvider trait

**목적**: 거래소별 호가 단위 통합 관리 (StrategyContext.exchange_constraints에서 활용)

**구현 항목**
- [ ] `TickSizeProvider` trait 정의 (trader-core)
  ```rust
  pub trait TickSizeProvider: Send + Sync {
      fn tick_size(&self, price: Decimal) -> Decimal;
      fn round_to_tick(&self, price: Decimal, method: RoundMethod) -> Decimal;
  }
  ```
- [ ] 거래소별 구현
  - [ ] `KrxTickSize`: 7단계 호가 단위
  - [ ] `UsEquityTickSize`: 고정 $0.01
  - [ ] `BinanceTickSize`: 심볼별 설정
- [ ] `round_to_tick()` 유틸리티 함수
- [ ] 팩토리 함수 `get_tick_provider(exchange: Exchange)`

**효과**:
- 백테스트 정확도 향상 (실제 호가 단위 반영)
- 주문 실행 시 가격 유효성 자동 검증
- Global Score의 목표가/손절가 계산에 활용

**예상 시간**: 4시간 (0.5일)

---

### 5. SDUI 스키마 자동 생성 시스템 ⭐ 확장

**목적**: 전략 Config에서 UI 스키마를 자동 생성하고, 재사용 가능한 Fragment로 동적 UI 조합

**현재 문제**:
- 전략마다 수동으로 SDUI JSON 스키마 작성 필요
- 동일한 지표/필터 설정이 여러 전략에 중복 정의
- 전략 추가 시 프론트엔드 코드 수정 필요

#### 5.1 Schema Fragment 시스템

**구현 항목**
- [ ] `SchemaFragment` 구조체 정의 (trader-core)
  ```rust
  /// 재사용 가능한 UI 스키마 조각
  pub struct SchemaFragment {
      pub id: String,           // "indicator.rsi", "filter.route_state"
      pub name: String,         // "RSI 설정"
      pub description: Option<String>,
      pub category: FragmentCategory,
      pub fields: Vec<FieldSchema>,
      pub dependencies: Vec<String>,  // 다른 Fragment 의존성
  }

  pub enum FragmentCategory {
      Indicator,    // 기술적 지표 (RSI, MACD, BB 등)
      Filter,       // 필터 조건 (RouteState, MarketRegime 등)
      RiskManagement,  // 리스크 관리 (손절, 익절, 트레일링)
      PositionSizing,  // 포지션 크기 (고정, 켈리, ATR 기반)
      Timing,       // 타이밍 (리밸런싱 주기, 거래 시간)
      Asset,        // 자산 선택 (심볼, 유니버스)
  }
  ```

- [ ] 기본 Fragment 정의 (26개 전략 공통 요소)
  ```rust
  // 지표 Fragment
  pub static RSI_FRAGMENT: SchemaFragment = fragment! {
      id: "indicator.rsi",
      name: "RSI 설정",
      category: Indicator,
      fields: [
          { name: "period", type: "integer", default: 14, min: 2, max: 100, label: "RSI 기간" },
          { name: "overbought", type: "number", default: 70.0, min: 50, max: 100, label: "과매수 임계값" },
          { name: "oversold", type: "number", default: 30.0, min: 0, max: 50, label: "과매도 임계값" },
      ]
  };

  // 필터 Fragment
  pub static ROUTE_STATE_FILTER: SchemaFragment = fragment! {
      id: "filter.route_state",
      name: "RouteState 필터",
      category: Filter,
      fields: [
          { name: "enabled", type: "boolean", default: false, label: "RouteState 필터 활성화" },
          { name: "allowed_states", type: "multi_select",
            options: ["Attack", "Armed", "Wait", "Overheat", "Neutral"],
            default: ["Attack", "Armed"], label: "허용 상태" },
      ]
  };

  // 리스크 Fragment
  pub static TRAILING_STOP_FRAGMENT: SchemaFragment = fragment! {
      id: "risk.trailing_stop",
      name: "트레일링 스탑",
      category: RiskManagement,
      fields: [
          { name: "enabled", type: "boolean", default: false, label: "트레일링 스탑 활성화" },
          { name: "trigger_pct", type: "number", default: 2.0, min: 0.1, max: 20,
            label: "활성화 수익률 (%)", condition: "enabled == true" },
          { name: "trail_pct", type: "number", default: 1.0, min: 0.1, max: 10,
            label: "추적 비율 (%)", condition: "enabled == true" },
      ]
  };
  ```

#### 5.2 FragmentRegistry (Fragment 관리)

- [ ] `FragmentRegistry` 구현
  ```rust
  pub struct FragmentRegistry {
      fragments: HashMap<String, SchemaFragment>,
  }

  impl FragmentRegistry {
      /// 빌트인 Fragment 자동 등록
      pub fn with_builtins() -> Self;

      /// Fragment 조회
      pub fn get(&self, id: &str) -> Option<&SchemaFragment>;

      /// 카테고리별 Fragment 목록
      pub fn list_by_category(&self, category: FragmentCategory) -> Vec<&SchemaFragment>;

      /// 의존성 포함 전체 Fragment 수집
      pub fn resolve_with_dependencies(&self, ids: &[&str]) -> Vec<&SchemaFragment>;
  }
  ```

- [ ] 빌트인 Fragment 카탈로그
  | 카테고리 | Fragment ID | 설명 |
  |----------|-------------|------|
  | Indicator | `indicator.rsi` | RSI 설정 |
  | Indicator | `indicator.macd` | MACD 설정 |
  | Indicator | `indicator.bollinger` | 볼린저 밴드 설정 |
  | Indicator | `indicator.ma` | 이동평균 설정 (SMA/EMA) |
  | Indicator | `indicator.atr` | ATR 설정 |
  | Filter | `filter.route_state` | RouteState 필터 |
  | Filter | `filter.market_regime` | MarketRegime 필터 |
  | Filter | `filter.volume` | 거래량 필터 |
  | RiskManagement | `risk.stop_loss` | 손절 설정 |
  | RiskManagement | `risk.take_profit` | 익절 설정 |
  | RiskManagement | `risk.trailing_stop` | 트레일링 스탑 |
  | PositionSizing | `sizing.fixed_ratio` | 고정 비율 |
  | PositionSizing | `sizing.kelly` | 켈리 기준 |
  | Timing | `timing.rebalance` | 리밸런싱 주기 |
  | Asset | `asset.single` | 단일 심볼 |
  | Asset | `asset.universe` | 심볼 유니버스 |

#### 5.3 StrategyConfig Derive 매크로

- [ ] `#[derive(StrategyConfig)]` 프로시저 매크로
  ```rust
  use trader_strategy_macro::StrategyConfig;

  #[derive(StrategyConfig)]
  #[strategy(
      id = "rsi_mean_reversion",
      name = "RSI 평균회귀",
      description = "RSI 과매수/과매도 구간에서 평균회귀 매매",
      category = "single_asset"
  )]
  pub struct RsiConfig {
      // 기본 Fragment 사용
      #[fragment("indicator.rsi")]
      pub rsi: RsiIndicatorConfig,

      // 선택적 Fragment
      #[fragment("filter.route_state", optional)]
      pub route_filter: Option<RouteStateFilterConfig>,

      // 커스텀 필드
      #[schema(label = "쿨다운 캔들 수", min = 0, max = 100)]
      pub cooldown_candles: usize,
  }
  ```

- [ ] 매크로가 생성하는 코드
  ```rust
  impl RsiConfig {
      /// 전체 UI 스키마 생성
      pub fn ui_schema() -> StrategyUISchema {
          StrategyUISchema {
              id: "rsi_mean_reversion".to_string(),
              name: "RSI 평균회귀".to_string(),
              description: Some("RSI 과매수/과매도 구간에서 평균회귀 매매".to_string()),
              category: "single_asset".to_string(),
              fragments: vec![
                  FragmentRef { id: "indicator.rsi", required: true },
                  FragmentRef { id: "filter.route_state", required: false },
              ],
              custom_fields: vec![
                  FieldSchema {
                      name: "cooldown_candles".to_string(),
                      field_type: FieldType::Integer,
                      label: "쿨다운 캔들 수".to_string(),
                      min: Some(0.0), max: Some(100.0),
                      ..Default::default()
                  }
              ],
          }
      }
  }
  ```

#### 5.4 SchemaComposer (스키마 조합기)

- [ ] `SchemaComposer` 구현
  ```rust
  pub struct SchemaComposer {
      registry: Arc<FragmentRegistry>,
  }

  impl SchemaComposer {
      /// 전략 스키마 + Fragment → 완성된 SDUI JSON
      pub fn compose(&self, strategy_schema: &StrategyUISchema) -> serde_json::Value {
          let mut sections = vec![];

          // Fragment 섹션 추가
          for frag_ref in &strategy_schema.fragments {
              if let Some(fragment) = self.registry.get(&frag_ref.id) {
                  sections.push(self.fragment_to_section(fragment, frag_ref.required));
              }
          }

          // 커스텀 필드 섹션
          if !strategy_schema.custom_fields.is_empty() {
              sections.push(self.custom_fields_section(&strategy_schema.custom_fields));
          }

          json!({
              "strategy_id": strategy_schema.id,
              "name": strategy_schema.name,
              "description": strategy_schema.description,
              "sections": sections
          })
      }

      fn fragment_to_section(&self, fragment: &SchemaFragment, required: bool) -> serde_json::Value {
          json!({
              "id": fragment.id,
              "name": fragment.name,
              "required": required,
              "collapsible": !required,
              "fields": fragment.fields.iter().map(|f| self.field_to_json(f)).collect::<Vec<_>>()
          })
      }
  }
  ```

#### 5.5 API 엔드포인트

- [ ] `GET /api/v1/strategies/meta` - 전략 목록 + 기본 메타데이터
- [ ] `GET /api/v1/strategies/{id}/schema` - 완성된 SDUI JSON 스키마
- [ ] `GET /api/v1/schema/fragments` - 사용 가능한 Fragment 목록
- [ ] `GET /api/v1/schema/fragments/{category}` - 카테고리별 Fragment

#### 5.6 프론트엔드 통합

- [ ] `SDUIRenderer` 컴포넌트 (SolidJS)
  - Fragment 기반 섹션 자동 렌더링
  - 조건부 필드 표시/숨김 (`condition` 속성 처리)
  - 실시간 유효성 검증

**의존성**: 전략 레지스트리 패턴 (1번 항목)

**효과**:
| 항목 | 개선 |
|------|------|
| 전략 추가 UI 작업 | 2시간 → 0분 (자동 생성) |
| Fragment 재사용 | 26개 전략에서 공통 설정 통합 |
| 프론트엔드 수정 | 새 전략 추가 시 코드 변경 불필요 |
| 일관성 | 모든 전략이 동일한 UI 패턴 사용 |

**예상 시간**: 20시간 (2.5일)
- FragmentRegistry + 빌트인: 8시간
- Derive 매크로: 6시간
- SchemaComposer + API: 4시간
- 프론트엔드 통합: 2시간

---

### 6. Journal-Backtest 공통 모듈 ⭐ 신규

**목적**: 매매일지와 백테스트에서 중복되는 로직을 통합하여 일관성 확보

**현재 문제**:
- P&L 계산이 `journal.rs`와 `engine.rs`에서 각각 독립 구현됨
- 승률, Profit Factor 등 통계 로직이 분산됨
- `TradeExecutionRecord`(Journal)와 `RoundTrip`(Backtest) 타입이 별도 정의
- 버그 수정 시 양쪽 모두 수정 필요

**구현 항목**
- [ ] `trader-core/domain/calculations.rs` - 공유 계산 함수
  ```rust
  pub mod calculations {
      /// 비용기준 계산 (FIFO, 가중평균, 최종평가 지원)
      pub fn cost_basis(entries: &[TradeEntry], method: CostMethod) -> Decimal;

      /// 실현손익 계산
      pub fn realized_pnl(entry: Decimal, exit: Decimal, qty: Decimal, side: Side) -> Decimal;

      /// 수익률 계산
      pub fn return_pct(pnl: Decimal, cost_basis: Decimal) -> Decimal;

      /// 미실현손익 계산
      pub fn unrealized_pnl(entry: Decimal, current: Decimal, qty: Decimal, side: Side) -> Decimal;
  }
  ```
- [ ] `trader-core/domain/statistics.rs` - 통합 통계 모듈
  ```rust
  pub struct TradeStatistics {
      pub total_trades: usize,
      pub winning_trades: usize,
      pub losing_trades: usize,
      pub win_rate_pct: Decimal,
      pub profit_factor: Decimal,
      pub avg_win: Decimal,
      pub avg_loss: Decimal,
      pub largest_win: Decimal,
      pub largest_loss: Decimal,
      pub avg_holding_period: Duration,
      pub expectancy: Decimal,  // 기대값 = 승률*평균이익 - 패률*평균손실
  }

  impl TradeStatistics {
      pub fn from_round_trips(trades: &[RoundTrip]) -> Self;
      pub fn from_journal_trades(trades: &[TradeExecutionRecord]) -> Self;
  }
  ```
- [ ] `UnifiedTrade` trait 정의 (두 타입 간 변환)
  ```rust
  pub trait UnifiedTrade {
      fn symbol(&self) -> &str;
      fn side(&self) -> Side;
      fn entry_price(&self) -> Decimal;
      fn exit_price(&self) -> Option<Decimal>;
      fn quantity(&self) -> Decimal;
      fn pnl(&self) -> Option<Decimal>;
      fn entry_time(&self) -> DateTime<Utc>;
      fn exit_time(&self) -> Option<DateTime<Utc>>;
  }

  impl UnifiedTrade for RoundTrip { /* ... */ }
  impl UnifiedTrade for TradeExecutionRecord { /* ... */ }
  ```
- [ ] 백테스트에서 Journal 통계 재사용
  ```rust
  // 백테스트 결과를 Journal 형식으로 내보내기
  pub fn export_to_journal(report: &BacktestReport) -> Vec<TradeExecutionRecord>;

  // Journal 데이터로 백테스트 비교 분석
  pub fn compare_with_actual(backtest: &BacktestReport, journal: &[TradeExecutionRecord]) -> ComparisonReport;
  ```

**효과**:
| 항목 | 개선 |
|------|------|
| 코드 중복 | 40-50% 감소 |
| 버그 수정 범위 | 1곳으로 통합 |
| 새 지표 추가 | 양쪽 자동 적용 |
| 백테스트-실거래 비교 | 동일 기준으로 분석 가능 |

**예상 시간**: 12시간 (1.5일)

---

## 🔴 Phase 1 - 핵심 기능 (Core Features)

> **의존성**: Phase 0 완료 후 시작
> **예상 시간**: 2주

### 1. 구조적 피처 (Structural Features)

**의존성**: `strategies/common/indicators.rs` 활용

**목적**: "살아있는 횡보"와 "죽은 횡보"를 구분하여 돌파 가능성 예측

**구현 항목**
- [ ] `StructuralFeatures` 구조체 정의 (trader-analytics)
  ```rust
  pub struct StructuralFeatures {
      pub low_trend: f64,      // Higher Low 강도
      pub vol_quality: f64,    // 매집/이탈 판별
      pub range_pos: f64,      // 박스권 위치 (0~1)
      pub dist_ma20: f64,      // MA20 이격도
      pub bb_width: f64,       // 볼린저 밴드 폭
      pub rsi: f64,            // RSI 14일
  }
  ```
- [ ] `from_candles()` 계산 로직 (공통 지표 모듈 활용)
- [ ] 피처 캐싱 (Redis, 동일 OHLCV 재계산 방지)
- [ ] 스크리닝 필터 조건으로 활용

**예상 시간**: 1주

---

### 2. RouteState 상태 관리

**의존성**: StructuralFeatures 완료 후

**목적**: 종목의 현재 매매 단계를 5단계로 분류

**구현 항목**
- [ ] `RouteState` enum 정의 (trader-core)
  ```rust
  pub enum RouteState {
      Attack,    // TTM Squeeze 해제 + 모멘텀 상승 + RSI 45~65 + Range_Pos >= 0.8
      Armed,     // Squeeze 중 + MA20 위 또는 Vol_Quality >= 2.0
      Wait,      // 정배열 + MA 지지 + Low_Trend > 0
      Overheat,  // 5일 수익률 > 20% 또는 RSI >= 75
      Neutral,   // 위 조건 미충족
  }
  ```
- [ ] `RouteStateCalculator` 구현 (StructuralFeatures 활용)
- [ ] `symbol_fundamental` 테이블에 `route_state` 컬럼 추가
- [ ] 스크리닝 응답에 `route_state` 포함
- [ ] ATTACK 상태 전환 시 텔레그램 알림

**전략 연동**:
- 레지스트리 패턴으로 등록된 모든 전략에서 RouteState 조회 가능
- 진입/청산 조건에 RouteState 활용

**예상 시간**: 0.5주

---

### 2.1 MarketRegime 시장 레짐 ⭐ 신규

**목적**: 종목의 추세 단계를 5단계로 분류하여 매매 타이밍 판단

**구현 항목**
- [ ] `MarketRegime` enum 정의 (trader-core)
  ```rust
  pub enum MarketRegime {
      StrongUptrend,  // ① 강한 상승 추세 (rel_60d > 10 + slope > 0 + RSI 50~70)
      Correction,     // ② 상승 후 조정 (rel_60d > 5 + slope <= 0)
      Sideways,       // ③ 박스 / 중립 (-5 <= rel_60d <= 5)
      BottomBounce,   // ④ 바닥 반등 시도 (rel_60d <= -5 + slope > 0)
      Downtrend,      // ⑤ 하락 / 약세
  }
  ```
- [ ] 60일 상대강도(`rel_60d_%`) 계산 로직
- [ ] 스크리닝 응답에 `regime` 필드 추가

**예상 시간**: 4시간

---

### 2.2 TRIGGER 진입 트리거 시스템 ⭐ 신규

**목적**: 여러 기술적 조건을 종합하여 진입 신호 강도와 트리거 라벨 생성

**구현 항목**
- [ ] `TriggerResult` 구조체 정의
  ```rust
  pub struct TriggerResult {
      pub score: f64,              // 0~100
      pub triggers: Vec<TriggerType>,
      pub label: String,           // "🚀급등시동, 📦박스돌파"
  }

  pub enum TriggerType {
      SqueezeBreak,   // TTM Squeeze 해제 (+30점)
      BoxBreakout,    // 박스권 돌파 (+25점)
      VolumeSpike,    // 거래량 폭증 (+20점)
      MomentumUp,     // 모멘텀 상승 (+15점)
      HammerCandle,   // 망치형 캔들 (+10점)
      Engulfing,      // 장악형 캔들 (+10점)
  }
  ```
- [ ] 캔들 패턴 감지 로직 (망치형, 장악형)
- [ ] 스크리닝 응답에 `trigger_score`, `trigger_label` 추가

**예상 시간**: 8시간

---

### 2.3 TTM Squeeze 상세 구현 ⭐ 신규

**목적**: John Carter의 TTM Squeeze - BB가 KC 내부로 들어가면 에너지 응축 상태

**구현 항목**
- [ ] `TtmSqueeze` 구조체 정의
  ```rust
  pub struct TtmSqueeze {
      pub is_squeeze: bool,        // 현재 스퀴즈 상태
      pub squeeze_count: u32,      // 연속 스퀴즈 일수
      pub momentum: Decimal,       // 스퀴즈 모멘텀 (방향)
      pub released: bool,          // 이번 봉에서 해제되었는가?
  }
  ```
- [ ] Keltner Channel 계산 (KC = MA ± 1.5 * ATR)
- [ ] BB vs KC 비교 로직
- [ ] `symbol_fundamental` 테이블에 `ttm_squeeze`, `ttm_squeeze_cnt` 컬럼 추가

**예상 시간**: 6시간

---

### 2.4 Macro Filter 매크로 환경 필터 ⭐ 신규

**목적**: USD/KRW 환율, 나스닥 지수 모니터링으로 시장 위험도 평가 및 동적 진입 기준 조정

**구현 항목**
- [ ] `MacroEnvironment` 구조체 정의
  ```rust
  pub struct MacroEnvironment {
      pub risk_level: MacroRisk,
      pub usd_krw: Decimal,
      pub usd_change_pct: f64,
      pub nasdaq_change_pct: f64,
      pub adjusted_ebs: u8,          // 조정된 EBS 기준
      pub recommendation_limit: usize, // 추천 종목 수 제한
  }

  pub enum MacroRisk {
      Critical,  // 환율 1400+ or 나스닥 -2% → EBS +1, 추천 3개
      High,      // 환율 +0.5% 급등 → EBS +1, 추천 5개
      Normal,    // 기본값
  }
  ```
- [ ] 환율/지수 데이터 수집 (Yahoo Finance API)
- [ ] 스크리닝 API 응답에 `macro_risk` 필드 추가
- [ ] 텔레그램 알림에 매크로 상태 포함

**예상 시간**: 6시간

---

### 2.5 Market Breadth 시장 온도 ⭐ 신규

**목적**: 20일선 상회 종목 비율로 시장 전체 건강 상태 측정

**구현 항목**
- [ ] `MarketBreadth` 구조체 정의
  ```rust
  pub struct MarketBreadth {
      pub all: f64,
      pub kospi: f64,
      pub kosdaq: f64,
      pub temperature: MarketTemperature,
  }

  pub enum MarketTemperature {
      Overheat,   // >= 65% 🔥
      Neutral,    // 35~65% 🌤
      Cold,       // <= 35% 🧊
  }
  ```
- [ ] 시장별 Above_MA20 비율 계산
- [ ] 대시보드에 시장 온도 위젯 추가

**예상 시간**: 4시간

---

### 2.6 추가 기술적 지표 ⭐ 신규

**목적**: 분석 정확도 향상을 위한 추가 지표

**구현 항목**
- [ ] `HMA` (Hull Moving Average) - 빠른 반응, 낮은 휩소
- [ ] `OBV` (On-Balance Volume) - 스마트 머니 추적
- [ ] `SuperTrend` - 추세 추종 지표
- [ ] `CandlePattern` 감지 - 망치형, 장악형

```rust
// trader-analytics/src/indicators/
pub mod hma;         // Hull Moving Average
pub mod obv;         // On-Balance Volume
pub mod supertrend;  // SuperTrend
pub mod candle_patterns; // 캔들 패턴 감지
```

**예상 시간**: 8시간

---

### 2.7 Sector RS 섹터 상대강도 ⭐ 신규

**목적**: 시장 대비 초과수익(Relative Strength)으로 진짜 주도 섹터 발굴

**구현 항목**
- [ ] 섹터별 RS 계산 (rel_20d_% 평균)
- [ ] 종합 섹터 점수 = RS * 0.6 + 단순수익 * 0.4
- [ ] 스크리닝에 `sector_rs`, `sector_rank` 필드 추가

**예상 시간**: 4시간

---

### 2.8 Reality Check 추천 검증 ⭐ 신규

**목적**: 전일 추천 종목의 익일 실제 성과 자동 검증

**구현 항목**
- [ ] `price_snapshot` 테이블 (TimescaleDB hypertable)
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
- [ ] `reality_check` 테이블 (TimescaleDB hypertable)
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
- [ ] 전일 추천 vs 금일 종가 비교 로직
- [ ] `RealityCheckRepository` 구현
- [ ] 통계 대시보드 API (`GET /api/v1/reality-check/stats`)

**활용**:
- 전략 신뢰도 측정
- 백테스트 vs 실거래 괴리 분석
- 파라미터 튜닝 피드백
- 시계열 쿼리로 기간별 성과 추이 분석

**예상 시간**: 8시간

---

### 3. Global Score 시스템

**의존성**: RouteState + StructuralFeatures + TickSizeProvider 완료 후

**목적**: 모든 기술적 지표를 단일 점수(0~100)로 종합

**구현 항목**
- [ ] `GlobalScorer` 구현 (trader-analytics)
  - [ ] 7개 팩터 가중치 (RR 0.25, T1 0.18, SL 0.12, NEAR 0.12, MOM 0.10, LIQ 0.13, TEC 0.10)
  - [ ] 페널티 시스템 7개
  - [ ] 정규화 유틸리티
- [ ] `LiquidityGate` 시장별 설정
- [ ] `ERS (Entry Ready Score)` 계산

**API**
- [ ] `POST /api/v1/ranking/global` - 글로벌 랭킹 조회
- [ ] `GET /api/v1/ranking/top?market=KR&n=10` - TOP N 조회
- [ ] 스크리닝 API에 `global_score` 필드 추가

**전략 연동**:
- 레지스트리 패턴으로 Global Score 기반 종목 자동 선택
- 점수 기반 포지션 사이징 (공통 로직 모듈 활용)

**예상 시간**: 1주

---

### 4. 전략 연계 (스크리닝 활용)

**의존성**: 위 3개 완료 후

**구현 항목**
- [ ] 전략에서 스크리닝 결과 활용 인터페이스 정의
  ```rust
  pub trait ScreeningAware {
      fn set_screening_results(&mut self, results: Vec<ScreeningResult>);
      fn filter_by_route_state(&self, state: RouteState) -> Vec<&ScreeningResult>;
  }
  ```
- [ ] 코스닥 급등주 전략: ATTACK 상태 종목만 진입
- [ ] 스노우볼 전략: 저PBR+고배당 + Global Score 상위
- [ ] 섹터 모멘텀 전략: 섹터별 TOP 5 자동 선택

**예상 시간**: 8시간

---

### 5. 기술 신호 저장 시스템 (SignalMarker) ⭐ 신규

**목적**: 백테스트와 실거래에서 발생한 기술 신호를 저장하여 분석 및 시각화에 활용

**현재 문제**:
- 백테스트에서 신호 발생 시점과 지표값이 기록되지 않음
- 전략 디버깅 시 "왜 이 시점에 진입/청산했는가" 추적 불가
- 과거 데이터에서 특정 패턴(골든크로스, RSI 과매도 등) 검색 불가

**구현 항목**
- [ ] `SignalMarker` 구조체 정의 (trader-core)
  ```rust
  /// 기술 신호 마커 - 캔들 차트에 표시할 신호 정보
  pub struct SignalMarker {
      pub id: Uuid,
      pub symbol: Symbol,
      pub timestamp: DateTime<Utc>,
      pub signal_type: SignalType,       // Entry, Exit, Alert
      pub side: Option<Side>,            // Buy, Sell
      pub price: Decimal,                // 신호 발생 시점 가격
      pub strength: f64,                 // 신호 강도 (0~1)

      /// 신호 생성에 사용된 지표 값들
      pub indicators: SignalIndicators,

      /// 신호 생성 이유 (사람이 읽을 수 있는 형태)
      pub reason: String,

      /// 전략 정보
      pub strategy_id: String,
      pub strategy_name: String,

      /// 실행 여부 (백테스트에서 실제 체결되었는지)
      pub executed: bool,

      /// 메타데이터 (확장용)
      pub metadata: HashMap<String, Value>,
  }

  /// 신호 생성에 사용된 기술적 지표 값들
  pub struct SignalIndicators {
      // 추세 지표
      pub sma_short: Option<Decimal>,
      pub sma_long: Option<Decimal>,
      pub ema_short: Option<Decimal>,
      pub ema_long: Option<Decimal>,

      // 모멘텀 지표
      pub rsi: Option<f64>,
      pub macd: Option<Decimal>,
      pub macd_signal: Option<Decimal>,
      pub macd_histogram: Option<Decimal>,

      // 변동성 지표
      pub bb_upper: Option<Decimal>,
      pub bb_middle: Option<Decimal>,
      pub bb_lower: Option<Decimal>,
      pub atr: Option<Decimal>,

      // TTM Squeeze
      pub squeeze_on: Option<bool>,
      pub squeeze_momentum: Option<Decimal>,

      // 구조적 피처 (StructuralFeatures 연동)
      pub route_state: Option<RouteState>,
      pub range_pos: Option<f64>,
      pub vol_quality: Option<f64>,
  }
  ```
- [ ] `SignalMarkerRepository` 구현 (저장/조회)
  ```rust
  #[async_trait]
  pub trait SignalMarkerRepository {
      /// 신호 마커 저장
      async fn save(&self, marker: &SignalMarker) -> Result<()>;

      /// 심볼+기간으로 조회
      async fn find_by_symbol(
          &self,
          symbol: &Symbol,
          start: DateTime<Utc>,
          end: DateTime<Utc>,
      ) -> Result<Vec<SignalMarker>>;

      /// 전략별 조회
      async fn find_by_strategy(
          &self,
          strategy_id: &str,
          limit: usize,
      ) -> Result<Vec<SignalMarker>>;

      /// 특정 지표 조건으로 검색 (예: RSI < 30인 신호)
      async fn search_by_indicator(
          &self,
          filter: IndicatorFilter,
      ) -> Result<Vec<SignalMarker>>;
  }
  ```
- [ ] 백테스트 엔진에서 SignalMarker 자동 기록
  ```rust
  // engine.rs에서 신호 발생 시 마커 생성
  fn process_signal(&mut self, signal: &Signal, kline: &Kline) {
      let marker = SignalMarker::from_signal(signal, kline, &self.indicators);
      self.signal_markers.push(marker);
      // ... 기존 로직
  }
  ```
- [ ] 지표 패턴 검색 API
  ```rust
  // POST /api/v1/signals/search
  #[derive(Deserialize)]
  pub struct SignalSearchRequest {
      pub symbol: Option<String>,
      pub start_date: DateTime<Utc>,
      pub end_date: DateTime<Utc>,
      pub filters: Vec<IndicatorFilter>,  // RSI < 30, MACD 크로스 등
      pub strategy_id: Option<String>,
  }
  ```

**API 엔드포인트**
- [ ] `GET /api/v1/signals/markers/{symbol}` - 심볼별 신호 마커 조회
- [ ] `GET /api/v1/signals/markers/backtest/{id}` - 백테스트 결과의 신호 목록
- [ ] `POST /api/v1/signals/search` - 지표 조건 검색

**텔레그램 알림 연동**
- [ ] `SignalAlertService` - 신호 발생 시 텔레그램 알림
  ```rust
  pub struct SignalAlertService {
      telegram: TelegramNotifier,
      alert_rules: Vec<AlertRule>,
  }

  /// 알림 규칙 정의
  pub struct AlertRule {
      pub name: String,
      pub conditions: AlertConditions,
      pub enabled: bool,
  }

  pub struct AlertConditions {
      pub signal_types: Vec<SignalType>,      // Entry, Exit 등
      pub min_strength: Option<f64>,          // 최소 신호 강도
      pub route_states: Vec<RouteState>,      // ATTACK, ARMED 등
      pub symbols: Option<Vec<String>>,       // 특정 심볼만 (None=전체)
      pub strategies: Option<Vec<String>>,    // 특정 전략만
      pub indicator_filters: Vec<IndicatorFilter>,  // RSI < 30 등
  }

  impl SignalAlertService {
      /// 신호 발생 시 규칙 검사 후 알림 전송
      pub async fn on_signal(&self, marker: &SignalMarker) -> Result<()> {
          for rule in &self.alert_rules {
              if rule.matches(marker) {
                  self.send_alert(rule, marker).await?;
              }
          }
          Ok(())
      }

      /// 텔레그램 메시지 포맷
      fn format_message(&self, marker: &SignalMarker) -> String {
          format!(
              "🚨 *{} 신호*\n\
               종목: `{}`\n\
               유형: {} (강도: {:.0}%)\n\
               가격: {}\n\
               상태: {:?}\n\
               ─────────────\n\
               RSI: {:.1} | MACD: {}\n\
               전략: {}",
              marker.side.map(|s| s.to_string()).unwrap_or_default(),
              marker.symbol,
              marker.signal_type,
              marker.strength * 100.0,
              marker.price,
              marker.indicators.route_state,
              marker.indicators.rsi.unwrap_or(0.0),
              marker.indicators.macd.map(|m| m.to_string()).unwrap_or("-".into()),
              marker.strategy_name,
          )
      }
  }
  ```
- [ ] 알림 규칙 설정 API
  - [ ] `GET /api/v1/alerts/rules` - 알림 규칙 목록
  - [ ] `POST /api/v1/alerts/rules` - 규칙 생성
  - [ ] `PUT /api/v1/alerts/rules/{id}` - 규칙 수정
  - [ ] `DELETE /api/v1/alerts/rules/{id}` - 규칙 삭제
- [ ] 기본 제공 알림 규칙
  - ATTACK 상태 진입 시 알림
  - 고강도(strength > 0.8) 진입 신호
  - RSI 극단값(< 25 또는 > 75) 신호

**활용 시나리오**:
1. **전략 디버깅**: "왜 이 시점에 매수했는가?" → 당시 RSI=28, MACD 골든크로스 확인
2. **패턴 학습**: RSI 30 이하에서 진입한 신호들의 성과 분석
3. **백테스트 시각화**: 차트에 진입/청산 포인트와 지표값 표시
4. **실거래 검증**: 백테스트 신호 vs 실제 신호 비교
5. **실시간 알림**: ATTACK 상태 진입, 고강도 신호 발생 시 즉시 텔레그램 알림

**예상 시간**: 20시간 (2.5일) - 텔레그램 알림 포함

---

## 🟡 Phase 2 - 프론트엔드 UI

> **의존성**: Phase 1 완료 후 (백엔드 API 필요)
> **예상 시간**: 3주

### 1. Trading Journal UI ⭐ (백엔드 완료)

**페이지**: `TradingJournal.tsx`
- [ ] 보유 현황 테이블 (FIFO 원가, 평가손익)
- [ ] 체결 내역 타임라인
- [ ] 포지션 비중 차트 (파이/도넛)
- [ ] 손익 분석 대시보드 (일별/주별/월별/연도별)

**예상 시간**: 1주

---

### 2. Screening UI (백엔드 완료)

**페이지**: `Screening.tsx`
- [ ] 필터 조건 입력 폼
- [ ] 프리셋 선택 UI
- [ ] 스크리닝 결과 테이블 (정렬, 페이지네이션)
- [ ] **RouteState 뱃지 컴포넌트** (Phase 1 연동)
- [ ] 종목 상세 모달 (Fundamental + 미니 차트)

**예상 시간**: 1주

---

### 3. Global Ranking UI

**페이지**: `GlobalRanking.tsx`
- [ ] TOP 10 대시보드 위젯
- [ ] 시장별 필터 (KR-KOSPI, KR-KOSDAQ, US)
- [ ] **점수 구성 요소 시각화** (레이더 차트)
- [ ] **RouteState별 필터링**

**예상 시간**: 0.5주

---

### 4. 캔들 차트 신호 시각화 ⭐ 신규

**의존성**: Phase 1 SignalMarker API 완료 후

**목적**: 과거 캔들 데이터에서 기술 신호 발생 지점을 시각적으로 표시

**구현 항목**
- [ ] `SignalMarkerOverlay` 컴포넌트
  ```tsx
  interface SignalMarkerOverlayProps {
    markers: SignalMarker[];
    onMarkerClick?: (marker: SignalMarker) => void;
  }

  // 차트에 마커 아이콘 표시
  // - 매수 신호: 초록색 위 화살표 ▲
  // - 매도 신호: 빨간색 아래 화살표 ▼
  // - 알림 신호: 노란색 점 ●
  ```
- [ ] `SignalDetailPopup` - 마커 클릭 시 상세 정보
  ```tsx
  // 표시 내용:
  // - 신호 유형, 강도
  // - 발생 시점 지표 값 (RSI, MACD 등)
  // - RouteState
  // - 전략 이름
  // - 실행 여부 (체결/미체결)
  ```
- [ ] `IndicatorFilterPanel` - 신호 필터링 UI
  ```tsx
  // 필터 조건:
  // - RSI 범위 (예: 30 이하만)
  // - MACD 크로스 유형
  // - RouteState (ATTACK, ARMED 등)
  // - 전략 선택
  ```
- [ ] 백테스트 결과 페이지에 차트+신호 통합
  ```tsx
  // BacktestResult.tsx
  <CandlestickChart data={candles}>
    <SignalMarkerOverlay markers={backtest.signal_markers} />
    <EquityCurveOverlay data={backtest.equity_curve} />
  </CandlestickChart>
  ```

**활용 화면**:
1. **백테스트 결과 분석**: 진입/청산 지점 시각적 확인
2. **종목 상세 페이지**: 과거 신호 발생 이력 조회
3. **전략 디버깅**: 특정 조건의 신호만 필터링하여 분석

**예상 시간**: 1주

---

### 5. 대시보드 고급 시각화 ⭐ 신규

**의존성**: Phase 1 핵심 기능 완료 후

**목적**: 고급 시각화 기능을 프론트엔드에 구현

#### 5.1 시장 심리 지표
- [ ] `FearGreedGauge` 컴포넌트
  - RSI + Disparity 기반 0~100 게이지
  - 5단계 색상 구분 (극단적 공포 → 극단적 탐욕)
- [ ] `MarketBreadthWidget` - 20일선 상회 비율

#### 5.2 팩터 분석 차트
- [ ] `RadarChart7Factor` - 7개 팩터 레이더 (NORM_*)
- [ ] `ScoreWaterfall` - 점수 기여도 워터폴
- [ ] `KellyVisualization` - 켈리 자금관리 바

#### 5.3 포트폴리오 분석
- [ ] `CorrelationHeatmap` - TOP 10 상관관계 히트맵
- [ ] `VolumeProfile` - 매물대 가로 막대 오버레이
- [ ] `OpportunityMap` - TOTAL vs TRIGGER 산점도

#### 5.4 상태 관리 UI
- [ ] `KanbanBoard` - ATTACK/ARMED/WATCH 3열 칸반
- [ ] `SurvivalBadge` - 생존일 뱃지 (연속 상위권 일수)
- [ ] `RegimeSummaryTable` - 레짐별 평균 성과

#### 5.5 섹터 시각화
- [ ] `SectorTreemap` - 거래대금 기반 트리맵
- [ ] `SectorMomentumBar` - 5일 수익률 Top 10

**예상 시간**: 1.5주 (46시간)

---

### 6. 프론트엔드 공통 개선

**상태 관리 리팩토링**
- [ ] `createSignal` → `createStore` 통합
- [ ] `createMemo`로 파생 상태 최적화

**컴포넌트 구조화**
```
frontend/src/
├── components/
│   ├── strategy/
│   │   └── SDUIRenderer/    # ⭐ 신규: SDUI 자동 생성
│   │       ├── SDUIRenderer.tsx
│   │       ├── SDUISection.tsx
│   │       ├── SDUIField.tsx
│   │       └── SDUIValidation.ts
│   ├── journal/
│   ├── screening/
│   ├── charts/        # ⭐ 신규: 시각화 컴포넌트
│   │   ├── FearGreedGauge.tsx
│   │   ├── RadarChart7Factor.tsx
│   │   ├── ScoreWaterfall.tsx
│   │   ├── CorrelationHeatmap.tsx
│   │   ├── OpportunityMap.tsx
│   │   └── KanbanBoard.tsx
│   └── common/
├── hooks/
│   ├── useStrategies.ts
│   ├── useStrategySchema.ts  # ⭐ 신규: SDUI 스키마 조회
│   ├── useJournal.ts
│   ├── useScreening.ts
│   └── useMarketSentiment.ts  # ⭐ 신규
└── stores/
```

**SDUIRenderer 시스템** (Phase 0 SDUI 자동 생성 연동)
- [ ] `SDUIRenderer` 메인 컴포넌트
  ```tsx
  interface SDUIRendererProps {
    strategyId: string;
    initialValues?: Record<string, any>;
    onChange?: (values: Record<string, any>) => void;
  }

  // API에서 스키마 조회 → Fragment 기반 섹션 자동 렌더링
  ```
- [ ] `SDUISection` - Fragment 섹션 렌더링 (접힘 지원)
- [ ] `SDUIField` - 필드 타입별 입력 컴포넌트 자동 선택
  - integer/number → NumberInput
  - boolean → Switch
  - select → Dropdown
  - multi_select → Checkboxes
  - symbol → SymbolAutocomplete
- [ ] `SDUIValidation` - 실시간 유효성 검증 (min/max, required)
- [ ] 조건부 필드 표시/숨김 (`condition` 속성 처리)
- [ ] `useStrategySchema` 훅 - 스키마 캐싱 및 조회

- [ ] 커스텀 훅 추출
- [ ] Lazy Loading 적용

**예상 시간**: 1주 (SDUIRenderer 포함)

---

## 🟢 Phase 3 - 품질/성능 개선

> 시스템 안정성 및 성능 개선 (Phase 1/2와 병행 가능)

### 성능 최적화
- [ ] 비동기 락 홀드 최적화 (4시간)
- [ ] Redis 캐싱 전략 (8시간)
- [ ] 병렬 백테스트 (4시간)

### 테스트
- [ ] 핵심 전략 테스트: Grid, RSI, Bollinger (8시간)
- [ ] API 테스트: strategies, backtest, journal (8시간)

### 인프라
- [ ] `CredentialsRepository` 구현 (3시간)
- [ ] `AlertsRepository` 구현 (3시간)
- [ ] SQLx 트랜잭션 패턴 완료 (3시간)

### 아키텍처
- [ ] Service 레이어 도입 (10시간)
- [ ] `analytics.rs` → Repository 이동

**총 예상 시간**: 51시간

---

## 🟣 Phase 4 - 선택적/낮은 우선순위

### 외부 데이터 연동
- [ ] `NewsProvider` trait + Finnhub API
- [ ] `DisclosureProvider` trait + SEC EDGAR
- [ ] LLM 분석 (공시/뉴스 감성 분석)

### 텔레그램 봇 명령어
- [ ] `/portfolio`, `/status`, `/stop`, `/report`, `/attack`

### 미구현 전략 (4개)
- [ ] SPAC No-Loss, All at Once ETF, Rotation Savings, Dual KrStock UsBond

### 추가 거래소
- [ ] Coinbase, Kraken, Interactive Brokers, 키움증권

### ML 예측 활용
- [ ] 전략에서 ML 예측 결과 사용
- [ ] 구조적 피처 기반 모델 재훈련

---

## ✅ 완료 현황

### v0.5.5 완료 (2026-02-01)

| 모듈 | 상태 | 비고 |
|------|:----:|------|
| Backend API (24개 라우트) | 98% | Journal 14개 API 포함 |
| Frontend (7 페이지, 15+ 컴포넌트) | 95%+ | |
| 전략 (26개 구현) | 100% | |
| ML (훈련 + ONNX 추론) | 95% | |
| 거래소 (Binance, KIS) | 90-95% | |
| 테스트 (258개 단위 + 28개 통합) | ✅ | |

### v0.5.5 신규 구현

| 기능 | 상태 |
|------|:----:|
| Trading Journal 백엔드 (14개 API) | ✅ |
| FIFO 원가 추적 (CostBasisTracker) | ✅ |
| API Retry 시스템 (지수 백오프, 지터) | ✅ |
| Circuit Breaker 에러 분류 (4개 카테고리) | ✅ |
| 동적 슬리피지 모델 (4개 모델) | ✅ |
| 브라켓 주문 (스탑/익절 OCO) | ✅ |
| 포지션 동기화 (PositionSync) | ✅ |
| SQL Injection 방지 | ✅ |
| 시간대별 거래 제한 (TradingTimezone) | ✅ |

### v0.4.x 완료

| 기능 | 버전 |
|------|------|
| OpenAPI/Swagger 문서화 | v0.4.4 |
| StrategyType enum (26개) | v0.4.4 |
| Repository 9개 구현 | v0.4.3~v0.4.5 |
| Graceful Shutdown | v0.4.5 |
| rustfmt/clippy 설정 | v0.4.5 |
| 입력 검증 강화 | v0.4.5 |
| unwrap() 39개 제거 | v0.4.5 |

---

## 📊 예상 시간 요약

| Phase | 카테고리 | 예상 시간 | 의존성 |
|:-----:|----------|----------:|:------:|
| ⚙️ 0 | **기반 작업** (레지스트리, 공통 로직, StrategyContext, TickSize, **공통 모듈**) | **2.5주** | - |
| 🔴 1 | 핵심 기능 (Features, RouteState, **REGIME**, **TRIGGER**, **TTM**, Global Score, **SignalMarker**, 전략 연계) | **4주** | Phase 0 |
| 🟡 2 | 프론트엔드 UI (Journal, Screening, Ranking, **신호 시각화**) | **3.5주** | Phase 1 |
| 🟢 3 | 품질/성능 개선 | **51시간** | 병행 가능 |
| 🟣 4 | 선택적 | - | - |

**v0.6.0 목표 (Phase 0 + 1 + 2)**: ~10주

### Phase 0 상세 시간 (기반 작업 - 코드 재사용의 핵심)

| 항목 | 예상 시간 | 효과 |
|------|----------:|------|
| 전략 레지스트리 | 28시간 | 전략 추가 2시간→30분, 모든 전략에 일괄 기능 적용 |
| 공통 로직 추출 | 12시간 | 보일러플레이트 80% 감소 |
| **StrategyContext** | **20시간** | **거래소 정보 + 분석 결과 통합, 충돌 방지** |
| TickSizeProvider | 4시간 | 백테스트/주문 정확도 향상 |
| **Journal-Backtest 공통 모듈** | **12시간** | **P&L/통계 로직 통합, 코드 중복 40-50% 감소** |
| **총계** | **76시간 (2.5주)** | |

### Phase 1 상세 시간

| 항목 | 예상 시간 | 효과 |
|------|----------:|------|
| StructuralFeatures | 1주 | 구조적 피처 6개, 공통 모듈 재사용 |
| RouteState | 0.5주 | 5단계 상태 판정 |
| **MarketRegime** | **4시간** | 5단계 추세 분류 |
| **TRIGGER 시스템** | **8시간** | 진입 트리거 + 캔들 패턴 |
| **TTM Squeeze 상세** | **6시간** | KC vs BB 로직, 연속일수 |
| **Macro Filter** | **6시간** | USD/KRW, 나스닥 모니터링 |
| **Market Breadth** | **4시간** | 시장 온도, Above_MA20 비율 |
| **추가 기술적 지표** | **8시간** | HMA, OBV, SuperTrend, 캔들패턴 |
| **Sector RS** | **4시간** | 섹터 상대강도 |
| **Reality Check** | **6시간** | 추천 검증 시스템 |
| Global Score | 1주 | 7개 팩터 + 페널티 시스템 |
| **SignalMarker + 알림** | **20시간** | **기술 신호 저장 + 텔레그램 알림 연동** |
| 전략 연계 | 8시간 | 스크리닝+포지션 연동 |
| **총계** | **~4주** | |

### Phase 2 상세 시간

| 항목 | 예상 시간 | 효과 |
|------|----------:|------|
| Trading Journal UI | 1주 | 보유현황, 체결내역, 손익분석 |
| Screening UI | 1주 | 필터, 프리셋, RouteState 뱃지 |
| Global Ranking UI | 0.5주 | TOP 10, 점수 시각화 |
| **캔들 차트 신호 시각화** | **1주** | **신호 마커, 지표 필터링** |
| 프론트엔드 공통 개선 | 0.5주 | 상태 관리, 컴포넌트 구조화 |

---

## 🔵 핵심 워크플로우 (v0.6.0)

```
┌─────────────────────────────────────────────────────────────┐
│  Phase 0 완료 후                                            │
│  ┌─────────────┐                                            │
│  │ 전략 등록   │ ← register_strategy! 매크로로 1줄 등록    │
│  └──────┬──────┘                                            │
│         ▼                                                    │
│  ┌─────────────┐     ┌─────────────┐                        │
│  │ 스크리닝    │ ──▶ │ RouteState  │ ATTACK 종목 필터      │
│  └──────┬──────┘     └──────┬──────┘                        │
│         │                   │                               │
│         ▼                   ▼                               │
│  ┌─────────────┐     ┌─────────────┐                        │
│  │ Global Score│ ──▶ │ TOP 10     │ 자동 포지션 사이징    │
│  └──────┬──────┘     └──────┬──────┘                        │
│         │                   │                               │
│         ▼                   ▼                               │
│  ┌─────────────┐     ┌─────────────┐                        │
│  │ 백테스트    │ ──▶ │ 시뮬레이션  │ TickSize 반영        │
│  └──────┬──────┘     └─────────────┘                        │
│         │                                                    │
│         ▼                                                    │
│  ┌─────────────┐     ┌─────────────┐                        │
│  │ 실전 운용   │ ──▶ │ 매매 일지   │ FIFO 손익 추적       │
│  └─────────────┘     └─────────────┘                        │
└─────────────────────────────────────────────────────────────┘
```

---

## 📚 참조 문서

| 문서 | 위치 | 용도 |
|------|------|------|
| PRD | `docs/prd.md` | 제품 요구사항 정의서 |
| Python 전략 모듈 | `docs/python_strategy_modules.md` | Global Score, RouteState 상세 스펙 |
| 개선 로드맵 | `docs/improvement_todo.md` | 코드베이스 개선 상세 |
| CLAUDE.md | 루트 | 프로젝트 구조, 에이전트 지침 |
