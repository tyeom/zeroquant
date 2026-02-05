최종 업데이트: 2026-02-03

# ZeroQuant TODO - 통합 로드맵

> **마지막 업데이트**: 2026-02-02
> **현재 버전**: v0.5.7
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
> **예상 시간**: 3주 (188시간) - SDUI 시스템 포함
> **핵심 효과**: 코드 중복 40-50% 감소, 사이드 이펙트 최소화, 유지보수 용이성 증대, UI 자동 생성

### 1. 전략 레지스트리 패턴 ⭐ 최우선
**[병렬 가능: P0.1]**

**현재 문제**: 새 전략 추가 시 **5곳 이상 수정** 필요
- `strategies/mod.rs` - pub mod, pub use
- `routes/strategies.rs` - 팩토리 함수 4개
- `routes/backtest/engine.rs` - match arm
- `config/sdui/strategy_schemas.json` - UI 스키마
- `frontend/Strategies.tsx` - 타임프레임 매핑

**개선 후**: 전략 파일 **1곳만 수정**

**구현 항목**
- [x] `inventory` crate 도입 (컴파일 타임 등록) ✅ v0.5.7
- [x] `StrategyMeta` 구조체 정의 ✅ v0.5.7
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
- [x] `register_strategy!` 매크로 구현 ✅ v0.5.7 (Proc macro로 구현, 266줄)
  ```rust
  register_strategy! {
      id: "rsi_mean_reversion",
      name: "RSI 평균회귀",
      timeframe: "15m",
      category: Intraday,
      type: RsiStrategy
  }
  ```
- [x] 팩토리 함수 자동화 (`create_strategy_instance()` 등) ✅ v0.5.7
- [x] `GET /api/v1/strategies/meta` API (프론트엔드 동적 조회) ✅ v0.5.7 (routes/schema.rs, 189줄)
- [x] 기존 26개 전략 마이그레이션 ✅ v0.5.7

**효과**:
- 전략 추가 시간: 2시간 → 30분
- Global Score, RouteState를 전략에 쉽게 연동 가능
- 새 피처(StructuralFeatures) 모든 전략에 일괄 적용 가능

**예상 시간**: 28시간 (3.5일)

---

### 2. TickSizeProvider (호가 단위 계산)

**[병렬 가능: P0.1]**

**목적**: 거래소별 호가 단위 통합 관리 (StrategyContext.exchange_constraints에서 활용)

**구현 항목**
- [x] `TickSizeProvider` trait 정의 (trader-core) ✅ v0.5.7 (tick_size.rs, 335줄)
  ```rust
  pub trait TickSizeProvider: Send + Sync {
      fn tick_size(&self, price: Decimal) -> Decimal;
      fn round_to_tick(&self, price: Decimal, method: RoundMethod) -> Decimal;
  }
  ```
- [x] 거래소별 구현 ✅ v0.5.7
  - [x] `KrxTickSize`: 7단계 호가 단위
  - [x] `UsEquityTickSize`: 고정 $0.01
  - [x] `BinanceTickSize`: 심볼별 설정
- [x] `round_to_tick()` 유틸리티 함수 ✅ v0.5.7
- [x] 팩토리 함수 `get_tick_provider(exchange: Exchange)` ✅ v0.5.7

**효과**:
- 백테스트 정확도 향상 (실제 호가 단위 반영)
- 주문 실행 시 가격 유효성 자동 검증
- Global Score의 목표가/손절가 계산에 활용

**예상 시간**: 4시간 (0.5일)

---

### 3. 전략 공통 로직 추출

**[의존성: P0.1 완료 후]**

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
- [x] `PositionSizer` trait 및 구현체 ✅ v0.5.7 (position_sizing.rs, 286줄)
  ```rust
  pub trait PositionSizer {
      fn calculate_size(&self, capital: Decimal, risk: &RiskParams) -> Decimal;
  }
  pub struct KellyPositionSizer { /* ... */ }
  pub struct FixedRatioSizer { /* ... */ }
  ```
- [x] `RiskChecker` trait 및 공통 체크 ✅ v0.5.7 (risk_checks.rs, 291줄)
- [x] `SignalFilter` trait (노이즈 필터링) ✅ v0.5.7 (signal_filters.rs, 372줄)
- [x] 공용 지표 계산 함수 (RSI, MACD, BB 등) ✅ v0.5.7 (indicators.rs, 349줄)

**효과**:
- StructuralFeatures 계산 로직을 공통 모듈에서 재사용
- 새 전략 개발 시 보일러플레이트 80% 감소
- 버그 수정 시 한 곳만 수정

**예상 시간**: 25시간 (3일)

---
### 4. SDUI 스키마 자동 생성 시스템 ⭐ 확장

**[병렬 가능: P0.2]**

**목적**: 전략 Config에서 UI 스키마를 자동 생성하고, 재사용 가능한 Fragment로 동적 UI 조합

**현재 문제**:
- 전략마다 수동으로 SDUI JSON 스키마 작성 필요
- 동일한 지표/필터 설정이 여러 전략에 중복 정의
- 전략 추가 시 프론트엔드 코드 수정 필요

#### 4.1 Schema Fragment 시스템 ✅ 완료

**구현 항목**
- [x] `SchemaFragment` 구조체 정의 (trader-core) → [schema.rs](crates/trader-core/src/domain/schema.rs)
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

- [x] 기본 Fragment 정의 (26개 전략 공통 요소) → [schema_registry.rs](crates/trader-strategy/src/schema_registry.rs)
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

#### 4.2 FragmentRegistry (Fragment 관리) ✅ 완료

- [x] `FragmentRegistry` 구현 → [schema_registry.rs](crates/trader-strategy/src/schema_registry.rs)
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

- [x] 빌트인 Fragment 카탈로그 (17개 Fragment 구현)
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

#### 4.3 StrategyConfig Derive 매크로 ✅ 완료

- [x] `#[derive(StrategyConfig)]` 프로시저 매크로 → [trader-strategy-macro/src/lib.rs](crates/trader-strategy-macro/src/lib.rs)
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

- [x] 매크로가 생성하는 코드 (`ui_schema()` 메서드 생성)
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

#### 4.4 SchemaComposer (스키마 조합기)

- [x] `SchemaComposer` 구현 ✅ v0.5.7 (schema_composer.rs, 279줄)
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

#### 4.5 API 엔드포인트

- [x] `GET /api/v1/strategies/meta` - 전략 목록 + 기본 메타데이터 ✅ v0.5.7 (routes/schema.rs, 189줄)
- [x] `GET /api/v1/strategies/{id}/schema` - 완성된 SDUI JSON 스키마 ✅ v0.5.7
- [ ] `GET /api/v1/schema/fragments` - 사용 가능한 Fragment 목록
- [ ] `GET /api/v1/schema/fragments/{category}` - 카테고리별 Fragment

#### 4.6 프론트엔드 통합

- [ ] `SDUIRenderer` 컴포넌트 (SolidJS)
  - Fragment 기반 섹션 자동 렌더링
  - 조건부 필드 표시/숨김 (`condition` 속성 처리)
  - 실시간 유효성 검증

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

### 5. Journal-Backtest 공통 모듈 ⭐ 신규

**[병렬 가능: P0.4]**

**목적**: 매매일지와 백테스트에서 중복되는 로직을 통합하여 일관성 확보

**현재 문제**:
- P&L 계산이 `journal.rs`와 `engine.rs`에서 각각 독립 구현됨
- 승률, Profit Factor 등 통계 로직이 분산됨
- `TradeExecutionRecord`(Journal)와 `RoundTrip`(Backtest) 타입이 별도 정의
- 버그 수정 시 양쪽 모두 수정 필요

**구현 항목**
- [x] `trader-core/domain/calculations.rs` - 공유 계산 함수 ✅ v0.5.7 (374줄)
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
- [x] `trader-core/domain/statistics.rs` - 통합 통계 모듈 ✅ v0.5.7 (514줄)
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
- [x] `UnifiedTrade` trait 정의 (두 타입 간 변환) ✅ v0.5.7
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
- [x] 백테스트에서 Journal 통계 재사용 ✅ v0.5.7 (journal_integration.rs, 280줄)
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

**예상 시간**: 15시간 (2일)

---

### 6. StrategyContext (전략 실행 컨텍스트) ⭐ 신규

**[의존성: P0.4, P0.5 완료 후]**

**목적**: 전략이 거래소 정보와 현재 포지션 상태를 실시간으로 조회하여 의사결정에 활용

**현재 문제**:
- 각 전략이 포지션을 독립적으로 관리 → 전략 간 포지션 정보 공유 불가
- 거래소 실시간 잔고 조회 기능 부재 → 실제 매수 가능 금액 알 수 없음
- 미체결 주문 상태 모름 → 중복 주문 위험

**구현 항목**
- [x] `StrategyContext` 구조체 정의 ✅ (trader-core/domain/context.rs)
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
- [x] `AccountInfo` - 실시간 계좌 정보 ✅ (`StrategyAccountInfo`로 구현)
  ```rust
  pub struct AccountInfo {
      pub total_balance: Decimal,       // 총 자산
      pub available_balance: Decimal,   // 매수 가능 금액
      pub margin_used: Decimal,         // 사용 중인 증거금
      pub unrealized_pnl: Decimal,      // 미실현 손익 합계
  }
  ```
- [x] `PositionInfo` - 포지션 상세 정보 ✅ (`StrategyPositionInfo`로 구현)
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
- [x] `ExchangeConstraints` - 거래소 제약 ✅ (trader-core/domain/context.rs)
  ```rust
  pub struct ExchangeConstraints {
      pub tick_size: TickSizeProvider,
      pub min_order_qty: Decimal,
      pub max_leverage: Option<Decimal>,
      pub trading_hours: Option<TradingHours>,
  }
  ```
- [x] `ExchangeProvider` trait (거래소별 구현) ✅ (trader-core/domain/exchange_provider.rs)
  ```rust
  #[async_trait]
  pub trait ExchangeProvider: Send + Sync {
      async fn fetch_account(&self) -> Result<AccountInfo>;
      async fn fetch_positions(&self) -> Result<Vec<PositionInfo>>;
      async fn fetch_pending_orders(&self) -> Result<Vec<PendingOrder>>;
  }
  ```
- [x] `AnalyticsProvider` trait (분석 결과 주입) ✅ (trader-core/domain/analytics_provider.rs)
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
- [x] `ContextSyncService` - 주기적 동기화 서비스 ✅ (trader-api/services/context_sync.rs)
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
- [x] `set_context` 메서드 ✅ (trader-strategy/traits.rs:58)
- [x] `PositionAdjustable` trait ✅ *2026-02-03 구현* (trader-core/domain/position.rs)
  - `should_adjust_position(&self, position: &Position) -> PositionAdjustment`
- [x] `PositionAdjustment` struct ✅ *2026-02-03 구현* (trader-core/domain/position.rs)
  - `AdjustmentType` enum: Add, Reduce, Close, StopLoss, TakeProfit, Rebalance, None

```rust
pub trait Strategy: Send + Sync {
    // 기존 메서드들...

    /// 컨텍스트 주입 (엔진에서 호출)
    fn set_context(&mut self, ctx: Arc<RwLock<StrategyContext>>);

    /// 포지션 기반 의사결정 (선택적 구현) - 미구현
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

**예상 시간**: 50시간 (6일) (AnalyticsProvider 포함, 가장 복잡한 작업)

---

```
순서 | 작업 | 시간 | 병렬 가능 여부 | 의존성
-----|------|------|--------------|--------
1    | 전략 레지스트리 | 28h | [P0.1] | -
2    | TickSizeProvider | 4h | [P0.1] | -
3    | 공통 로직 추출 | 25h | - | P0.1 완료 후
4    | SDUI 자동 생성 | 50h | [P0.2] | -
5    | Journal-Backtest | 15h | [P0.4] | -
6    | StrategyContext | 50h | - | P0.4, P0.5 완료 후
```

**예상 시간**: 4시간 (0.5일)
**총 예상 시간**: 172h → **228h** (56h 증가, Standalone Collector 추가)

---

### 7. Standalone Data Collector ⭐ 신규

**[병렬 가능: P0.7]**

**목적**: API 서버와 독립적으로 데이터를 수집하는 standalone 바이너리 구축

**현재 문제**:
- 데이터 수집이 API 서버 내부 백그라운드 태스크로 실행됨
- API 서버 재시작 시 데이터 수집 중단
- 높은 I/O 부하가 API 응답 성능에 영향
- Cron/systemd로 독립 실행 불가
- 리소스 격리 불가 (별도 머신/컨테이너 배포 어려움)

**구현 항목**
- [x] 새로운 `trader-collector` crate 생성 ✅
  ```rust
  // CLI 인터페이스
  pub enum Commands {
      SyncSymbols,           // 심볼 동기화 (KRX, Binance, Yahoo)
      CollectOhlcv,          // OHLCV 수집 (일봉)
      CollectFundamental,    // Fundamental 수집
      RunAll,                // 전체 워크플로우
      Daemon,                // 데몬 모드 (주기적 실행) ⭐
  }
  ```
- [x] 모듈 구조 ✅
  ```
  trader-collector/
  ├── src/
  │   ├── main.rs           // CLI 엔트리포인트
  │   ├── config.rs         // 환경변수 기반 설정
  │   ├── modules/
  │   │   ├── symbol_sync.rs      // 심볼 동기화
  │   │   ├── ohlcv_collect.rs    // OHLCV 수집
  │   ├── error.rs          // 에러 타입
  │   └── stats.rs          // 수집 통계
  ```
- [x] trader-data 컴포넌트 재사용 ✅
  - `CachedHistoricalDataProvider` - Yahoo Finance (KRX fallback) 🔄
  - `SymbolResolver` - 심볼 정규화 및 변환
  - `SymbolInfoProvider` - KRX/Binance/Yahoo 종목 조회
- [x] Yahoo Finance로 전환 ✅ (KRX API 차단 대응)
  - KRX data.krx.co.kr → 403 Forbidden
  - Yahoo Finance 자동 fallback 내장
  - 증분 수집 지원 (마지막 시간 이후만)
- [x] 배치 처리 및 Rate Limiting ✅
  - 전체 종목 수집 (LIMIT 제거)
  - Rate limit: 200ms~500ms (설정 가능)
  - 개별 실패가 전체 중단하지 않도록 에러 핸들링
- [x] 스케줄링 지원 ✅
  - Cron 스크립트 예제 제공 (`scripts/collector.cron`)
  - systemd timer/service 파일 제공
  - 데몬 모드 추가 (DAEMON_INTERVAL_MINUTES)
- [x] 모니터링 및 로깅 ✅
  - tracing 기반 구조화 로깅
  - 진행률, 성공/실패 통계 출력
  - CollectionStats 구조체
- [x] 추가 구현 ⭐
  - symbol_type 마이그레이션 (024_add_symbol_type.sql)
  - ETN 자동 필터링 (223개)
  - 우선주/특수증권 대응
  - 최적화된 환경변수 예제 (.env.collector.optimized)

**기대 효과**:
| 항목 | 개선 |
|------|------|
| **서비스 분리** | API 서버와 완전 독립 운영 |
| **스케줄링** | Cron/systemd로 유연한 주기 설정 |
| **리소스 격리** | 별도 머신/컨테이너 배포 가능 |
| **안정성** | API 서버 장애가 데이터 수집에 영향 없음 |
| **성능** | 데이터 수집 부하가 API 응답에 영향 없음 |

**참조 문서**:
- `docs/standalone_collector_design.md` - 상세 설계 문서 (100+ 섹션)
- `docs/collector_quick_start.md` - 빠른 시작 가이드
- `docs/collector_env_example.env` - 환경변수 예제

**예상 시간**: 40시간 (5일)
- CLI + 기본 구조: 8시간
- 심볼 동기화 모듈: 10시간
- OHLCV 수집 모듈: 10시간
- Fundamental 수집 모듈: 8시간
- 배포 설정 (Docker, systemd): 4시간

---

## 🔴 Phase 1 - 핵심 기능 (Core Features)

> **의존성**: Phase 0 완료 후 시작
> **예상 시간**: 2주

### Phase 1-A: 분석 엔진(1.5주, 선형 의존)

#### 1.1.1 구조적 피처 (Structural Features)
**[의존성: P0.3 공통 로직]**

**목적**: "살아있는 횡보"와 "죽은 횡보"를 구분하여 돌파 가능성 예측

**구현 항목**
- [x] `StructuralFeatures` 구조체 정의 (trader-analytics) ✅
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
- [x] `from_candles()` 계산 로직 (공통 지표 모듈 활용) ✅
- [x] 피처 캐싱 (Redis, 동일 OHLCV 재계산 방지) ✅ (trader-api/cache/structural.rs)
- [x] 스크리닝 필터 조건으로 활용 ✅ (screening_integration.rs)

**예상 시간**: 1주

---

#### 1.1.2 RouteState 상태 관리
**[의존성: P1-A.1.1 완료 후]**

**목적**: 종목의 현재 매매 단계를 5단계로 분류

**구현 항목**
- [x] `RouteState` enum 정의 (trader-core) ✅
  ```rust
  pub enum RouteState {
      Attack,    // TTM Squeeze 해제 + 모멘텀 상승 + RSI 45~65 + Range_Pos >= 0.8
      Armed,     // Squeeze 중 + MA20 위 또는 Vol_Quality >= 2.0
      Wait,      // 정배열 + MA 지지 + Low_Trend > 0
      Overheat,  // 5일 수익률 > 20% 또는 RSI >= 75
      Neutral,   // 위 조건 미충족
  }
  ```
- [x] `RouteStateCalculator` 구현 (StructuralFeatures 활용) ✅
- [x] `symbol_fundamental` 테이블에 `route_state` 컬럼 추가 ✅ (09_strategy_system.sql)
- [x] 스크리닝 응답에 `route_state` 포함 ✅ (ScreeningResult.route_state)
- [ ] ATTACK 상태 전환 시 텔레그램 알림

**전략 연동**:
- 레지스트리 패턴으로 등록된 모든 전략에서 RouteState 조회 가능
- 진입/청산 조건에 RouteState 활용

**예상 시간**: 0.5주

---
### Phase 1-B: 환경 분석 (0.5주, 병렬 가능)

> **병렬 실행**: Phase 1-A 완료 후, 아래 항목들은 서로 독립적이므로 동시 진행 가능

#### 1.2.1 MarketRegime 시장 레짐 ⭐ 신규

**목적**: 종목의 추세 단계를 5단계로 분류하여 매매 타이밍 판단

**구현 항목**
- [x] `MarketRegime` enum 정의 (trader-core) ✅
  ```rust
  pub enum MarketRegime {
      StrongUptrend,  // ① 강한 상승 추세 (rel_60d > 10 + slope > 0 + RSI 50~70)
      Correction,     // ② 상승 후 조정 (rel_60d > 5 + slope <= 0)
      Sideways,       // ③ 박스 / 중립 (-5 <= rel_60d <= 5)
      BottomBounce,   // ④ 바닥 반등 시도 (rel_60d <= -5 + slope > 0)
      Downtrend,      // ⑤ 하락 / 약세
  }
  ```
- [x] 60일 상대강도(`rel_60d_%`) 계산 로직 ✅ (calculate_relative_strength_60d)
- [x] 스크리닝 응답에 `regime` 필드 추가 ✅ (ScreeningResult.regime)

**예상 시간**: 4시간

---

#### 1.2.2 TRIGGER 진입 트리거 시스템 ✅ 완료

**목적**: 여러 기술적 조건을 종합하여 진입 신호 강도와 트리거 라벨 생성

**구현 항목**
- [x] `TriggerResult` 구조체 정의 → [trigger.rs](crates/trader-core/src/domain/trigger.rs)
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
- [x] 캔들 패턴 감지 로직 (망치형, 장악형) → [candle_patterns.rs](crates/trader-analytics/src/indicators/candle_patterns.rs)
- [x] 스크리닝 응답에 `trigger_score`, `trigger_label` 추가 → [screening.rs](crates/trader-api/src/routes/screening.rs)

**예상 시간**: 8시간

---

#### 1.2.3 TTM Squeeze 상세 구현 ✅ 완료

**목적**: John Carter의 TTM Squeeze - BB가 KC 내부로 들어가면 에너지 응축 상태

**구현 항목**
- [x] `TtmSqueezeResult` 구조체 정의 → [volatility.rs](crates/trader-analytics/src/indicators/volatility.rs)
  ```rust
  pub struct TtmSqueezeResult {
      pub is_squeeze: bool,
      pub squeeze_count: u32,
      pub momentum: Decimal,
      pub released: bool,
  }
  ```
- [x] Keltner Channel 계산 → `KeltnerChannelResult`
- [x] BB vs KC 비교 로직 → `VolatilityIndicators::ttm_squeeze()`
- [x] `symbol_fundamental` 테이블에 `ttm_squeeze`, `ttm_squeeze_cnt` 컬럼 추가

**예상 시간**: 6시간

---

#### 1.2.4 Macro Filter 매크로 환경 필터 ✅ 완료

**목적**: USD/KRW 환율, 나스닥 지수 모니터링으로 시장 위험도 평가 및 동적 진입 기준 조정

**구현 항목**
- [x] `MacroEnvironment` 구조체 정의 → [macro_environment.rs](crates/trader-core/src/domain/macro_environment.rs)
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

#### 1.2.5 Market Breadth 시장 온도 ✅ 완료

**목적**: 20일선 상회 종목 비율로 시장 전체 건강 상태 측정

**구현 항목**
- [x] `MarketBreadth` 구조체 정의 → [market_breadth.rs](crates/trader-core/src/domain/market_breadth.rs)
- [x] `MarketTemperature` enum 정의
- [x] 시장별 Above_MA20 비율 계산
- [ ] 대시보드에 시장 온도 위젯 추가

**예상 시간**: 4시간

---

#### 1.2.6 추가 기술적 지표 ✅ 완료

**목적**: 분석 정확도 향상을 위한 추가 지표

**구현 항목**
- [x] `HMA` (Hull Moving Average) → [hma.rs](crates/trader-analytics/src/indicators/hma.rs)
- [x] `OBV` (On-Balance Volume) → [obv.rs](crates/trader-analytics/src/indicators/obv.rs)
- [x] `SuperTrend` → [supertrend.rs](crates/trader-analytics/src/indicators/supertrend.rs)
- [x] `CandlePattern` 감지 → [candle_patterns.rs](crates/trader-analytics/src/indicators/candle_patterns.rs)

**예상 시간**: 8시간

---

#### 1.2.7 Sector RS 섹터 상대강도 ✅ 완료

**목적**: 시장 대비 초과수익(Relative Strength)으로 진짜 주도 섹터 발굴

**구현 항목**
- [x] 섹터별 RS 계산 → [screening.rs](crates/trader-api/src/repository/screening.rs)
- [x] 종합 섹터 점수 계산 로직
- [x] 스크리닝에 `sector_rs`, `sector_rank` 필드 추가 → [screening_integration.rs](crates/trader-strategy/src/strategies/common/screening_integration.rs)

**예상 시간**: 4시간

---

#### 1.2.8 Reality Check 추천 검증 ✅ 완료

**목적**: 전일 추천 종목의 익일 실제 성과 자동 검증

**구현 항목**
- [x] `price_snapshot` 테이블 (TimescaleDB hypertable) → [10_reality_check.sql](migrations/10_reality_check.sql)
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
- [x] `reality_check` 테이블 (TimescaleDB hypertable) → [10_reality_check.sql](migrations/10_reality_check.sql)
- [x] 전일 추천 vs 금일 종가 비교 로직 → [reality_check.rs](crates/trader-api/src/repository/reality_check.rs)
- [x] `RealityCheckRepository` 구현 → [reality_check.rs](crates/trader-api/src/repository/reality_check.rs)
- [x] 통계 대시보드 API → [reality_check.rs](crates/trader-api/src/routes/reality_check.rs)
  - `GET /api/v1/reality-check/stats` - 통계 조회
  - `GET /api/v1/reality-check/results` - 검증 결과 목록
  - `GET /api/v1/reality-check/snapshots` - 스냅샷 목록
  - `POST /api/v1/reality-check/snapshots` - 스냅샷 저장
  - `POST /api/v1/reality-check/calculate` - Reality Check 계산

**활용**:
- 전략 신뢰도 측정
- 백테스트 vs 실거래 괴리 분석
- 파라미터 튜닝 피드백
- 시계열 쿼리로 기간별 성과 추이 분석

**예상 시간**: 8시간

---

### Phase 1-C: 신호 시스템 (0.5주, 순차)

#### 1.3.1 기술 신호 저장 시스템 (SignalMarker) ⭐ 신규

**목적**: 백테스트와 실거래에서 발생한 기술 신호를 저장하여 분석 및 시각화에 활용

**현재 문제**:
- 백테스트에서 신호 발생 시점과 지표값이 기록되지 않음
- 전략 디버깅 시 "왜 이 시점에 진입/청산했는가" 추적 불가
- 과거 데이터에서 특정 패턴(골든크로스, RSI 과매도 등) 검색 불가

**구현 항목**
- [x] ✅ `SignalMarker` 구조체 정의 (trader-core) → [signal.rs:196-234](crates/trader-core/src/domain/signal.rs#L196-L234)
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
- [x] ✅ `SignalMarkerRepository` 구현 (저장/조회) → [signal_marker.rs](crates/trader-api/src/repository/signal_marker.rs)
- [x] ✅ 백테스트 엔진에서 SignalMarker 자동 기록 → [engine.rs:533](crates/trader-analytics/src/backtest/engine.rs#L533)
  ```rust
  // engine.rs에서 신호 발생 시 마커 생성
  fn process_signal(&mut self, signal: &Signal, kline: &Kline) {
      let marker = SignalMarker::from_signal(signal, kline, &self.indicators);
      self.signal_markers.push(marker);
      // ... 기존 로직
  }
  ```
- [x] ✅ 지표 패턴 검색 API → [signals.rs:184](crates/trader-api/src/routes/signals.rs#L184)

**API 엔드포인트**
- [x] ✅ `GET /api/v1/signals/by-symbol` - 심볼별 신호 마커 조회 → [signals.rs:226](crates/trader-api/src/routes/signals.rs#L226)
- [x] ✅ `GET /api/v1/signals/markers/backtest/{id}` - 백테스트 결과의 신호 목록 *2026-02-03 구현* → [signals.rs:330](crates/trader-api/src/routes/signals.rs#L330)
- [x] ✅ `POST /api/v1/signals/search` - 지표 조건 검색 → [signals.rs:184](crates/trader-api/src/routes/signals.rs#L184)
- [x] ✅ `GET /api/v1/signals/by-strategy` - 전략별 신호 조회 → [signals.rs:270](crates/trader-api/src/routes/signals.rs#L270)

**텔레그램 알림 연동**
- [x] ✅ `SignalAlertService` 기본 구조체 → [signal_alert.rs:96](crates/trader-api/src/services/signal_alert.rs#L96)
- [x] ✅ `AlertRule` 구조체 *2026-02-03 구현* → [alert.rs](crates/trader-core/src/domain/alert.rs)
- [x] ✅ `AlertCondition` enum *2026-02-03 구현* → [alert.rs](crates/trader-core/src/domain/alert.rs)
  - Indicator, Price, RouteStateChange, GlobalScore, And, Or
- [x] ✅ `IndicatorFilter` 구조체 *2026-02-03 구현* → [alert.rs](crates/trader-core/src/domain/alert.rs)
- [x] ✅ `ComparisonOperator` enum *2026-02-03 구현* (Eq, Ne, Gt, Gte, Lt, Lte, Between, CrossAbove, CrossBelow)
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
- [ ] ❌ 알림 규칙 설정 API **미구현**
  - [ ] `GET /api/v1/alerts/rules` - 알림 규칙 목록
  - [ ] `POST /api/v1/alerts/rules` - 규칙 생성
  - [ ] `PUT /api/v1/alerts/rules/{id}` - 규칙 수정
  - [ ] `DELETE /api/v1/alerts/rules/{id}` - 규칙 삭제
- [ ] ❌ 기본 제공 알림 규칙 **미구현**
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

### Phase 1-D: 검증 및 통합 (0.5주, 순차)

#### 1.4.1. Global Score 시스템

**[의존성: P1-A 완료 후]**

**목적**: 모든 기술적 지표를 단일 점수(0~100)로 종합

**구현 항목**
- [x] ✅ `GlobalScorer` 구현 (trader-analytics) → [global_scorer.rs](crates/trader-analytics/src/global_scorer.rs)
  - [x] ✅ 7개 팩터 가중치 (RR 0.25, T1 0.18, SL 0.12, NEAR 0.12, MOM 0.10, LIQ 0.13, TEC 0.10) → [global_scorer.rs:56-79](crates/trader-analytics/src/global_scorer.rs#L56-L79)
  - [x] ✅ 페널티 시스템 7개 → [global_scorer.rs:17-23](crates/trader-analytics/src/global_scorer.rs#L17-L23)
  - [x] ✅ 정규화 유틸리티 (GlobalScorerParams) → [global_scorer.rs:82-126](crates/trader-analytics/src/global_scorer.rs#L82-L126)
- [x] ✅ `LiquidityGate` 시장별 설정 → [liquidity_gate.rs](crates/trader-analytics/src/liquidity_gate.rs)
- [x] ✅ `ERS (Entry Ready Score)` 계산 → GlobalScorer::calculate의 momentum 팩터에 포함

**API**
- [x] ✅ `POST /api/v1/ranking/global` - 글로벌 랭킹 조회 → [ranking.rs:calculate_global](crates/trader-api/src/routes/ranking.rs)
- [x] ✅ `GET /api/v1/ranking/top?market=KR&n=10` - TOP N 조회 → [ranking.rs:get_top_ranked](crates/trader-api/src/routes/ranking.rs)
- [ ] 스크리닝 API에 `global_score` 필드 추가

**전략 연동**:
- 레지스트리 패턴으로 Global Score 기반 종목 자동 선택
- 점수 기반 포지션 사이징 (공통 로직 모듈 활용)

**예상 시간**: 1주

---

#### 1.4.2 Multiple KLine Period (다중 타임프레임) ⭐ 신규

**[병렬 가능: P1-C 완료 후]**

**목적**: 단일 전략에서 여러 타임프레임의 캔들 데이터를 동시에 활용하여 더 정교한 매매 신호 생성

**참조 문서**: `docs/multiple_kline_period_requirements.md` (상세 요구사항 및 구현 방법론)

**현재 한계**:
- 전략은 생성 시 지정한 단일 Timeframe만 사용 가능
- 멀티 타임프레임 분석(MTF Analysis) 불가능
- 장기 추세 + 단기 진입 타이밍 조합 불가

**구현 단계** (총 6 Phase, 7주):

##### Phase 1: 데이터 모델 확장 (1주)
- [x] `MultiTimeframeConfig` 구조체 정의 ✅ *2026-02-03 구현*
  ```rust
  // crates/trader-core/src/domain/context.rs
  pub struct MultiTimeframeConfig {
      pub timeframes: HashMap<Timeframe, usize>,  // TF별 캔들 개수
      pub primary_timeframe: Option<Timeframe>,   // 기본 타임프레임
      pub auto_sync: bool,                        // 자동 동기화 여부
  }
  ```
- [ ] `StrategyConfig`에 `multi_timeframe` 필드 추가
- [ ] DB 스키마 확장 (`strategies.secondary_timeframes` 컬럼)
- [ ] 유효성 검증 (Secondary는 Primary보다 큰 TF만 허용)

##### Phase 2: 데이터 조회 API (1주)
- [x] `AnalyticsProviderImpl::fetch_multi_timeframe_klines()` 구현 ✅ *2026-02-03 구현*
  ```rust
  // crates/trader-analytics/src/analytics_provider_impl.rs
  pub async fn fetch_multi_timeframe_klines(
      &self,
      ticker: &str,
      config: &MultiTimeframeConfig,
  ) -> Result<Vec<(Timeframe, Vec<Kline>)>, AnalyticsError>

  pub async fn fetch_multi_timeframe_klines_batch(
      &self,
      tickers: &[&str],
      config: &MultiTimeframeConfig,
  ) -> Result<HashMap<String, Vec<(Timeframe, Vec<Kline>)>>, AnalyticsError>
  ```
- [ ] Redis 멀티키 조회 최적화 (병렬 GET)
- [ ] PostgreSQL 단일 쿼리 최적화 (UNION ALL)
- [ ] 타임프레임별 차등 TTL 설정
- [ ] 성능 테스트 (목표: 3 TF 조회 < 50ms)

##### Phase 3: Context Layer 통합 (1주)
- [x] `StrategyContext`에 `klines_by_timeframe` 필드 추가 ✅ *2026-02-03 구현*
  ```rust
  // crates/trader-core/src/domain/context.rs
  pub struct StrategyContext {
      pub klines_by_timeframe: HashMap<String, HashMap<Timeframe, Vec<Kline>>>,
      // ticker → (timeframe → klines)
  }

  impl StrategyContext {
      pub fn get_klines(&self, ticker: &str, tf: Timeframe) -> &[Kline];
      pub fn get_multi_timeframe_klines(&self, ticker: &str, tfs: &[Timeframe]) -> Vec<(Timeframe, &[Kline])>;
      pub fn get_available_timeframes(&self, ticker: &str) -> Vec<(Timeframe, usize)>;
      pub fn update_klines(&mut self, ticker: &str, tf: Timeframe, klines: Vec<Kline>);
      pub fn update_multi_timeframe_klines(&mut self, ticker: &str, data: Vec<(Timeframe, Vec<Kline>)>);
  }
  ```
- [ ] Timeframe Alignment 로직 (미래 데이터 누출 방지)
- [ ] `StrategyExecutor`에서 멀티 데이터 자동 로드

##### Phase 4: 전략 예제 작성 (1주)
- [ ] `RsiMultiTimeframeStrategy` 구현
  - 일봉 RSI > 50 (상승 추세 확인)
  - 1시간봉 RSI < 30 (과매도 진입)
  - 5분봉 RSI 반등 (실제 진입 타이밍)
- [ ] `MovingAverageCascadeStrategy` 구현
  - 주봉 200MA, 일봉 50MA, 1시간 20MA 계층 분석
- [ ] 헬퍼 함수 작성 (`analyze_trend`, `combine_signals` 등)
- [ ] 유닛/통합 테스트

##### Phase 5: SDUI 및 API (1.5주)
- [ ] SDUI 스키마에 멀티 타임프레임 선택 UI 추가
  ```json
  {
    "type": "multi-select",
    "id": "secondary_timeframes",
    "label": "보조 타임프레임 (최대 2개)",
    "validation": "larger_than_primary"
  }
  ```
- [ ] API 엔드포인트 수정
  - `POST /api/v1/strategies`: `multi_timeframe_config` 필드
  - `GET /api/v1/strategies/{id}/timeframes`: TF 설정 조회
  - `GET /api/v1/klines/multi`: 멀티 TF 데이터 조회 (디버깅용)
- [ ] 프론트엔드 `MultiTimeframeSelector.tsx` 컴포넌트

##### Phase 6: 백테스트/실시간 통합 (1.5주)
- [ ] 백테스트 엔진에서 타임스탬프별 Secondary 데이터 정렬
- [ ] 히스토리 캐싱으로 성능 최적화
- [ ] WebSocket 멀티 스트림 구독
  ```rust
  let streams = vec![
      format!("{}@kline_5m", symbol),
      format!("{}@kline_1h", symbol),
      format!("{}@kline_1d", symbol),
  ];
  ```
- [ ] Primary TF 완료 시에만 전략 재평가
- [ ] 통합 테스트 및 부하 테스트

**사용 예시**:
```rust
// RSI 멀티 타임프레임 전략
impl Strategy for RsiMultiTimeframeStrategy {
    async fn analyze(&self, ctx: &StrategyContext) -> Result<Signal> {
        // Primary (5분)
        let klines_5m = ctx.primary_klines()?;
        let rsi_5m = calculate_rsi(klines_5m, 14);
        
        // Secondary (1시간)
        let klines_1h = ctx.get_klines(Timeframe::H1)?;
        let rsi_1h = calculate_rsi(klines_1h, 14);
        
        // Secondary (일봉)
        let klines_1d = ctx.get_klines(Timeframe::D1)?;
        let rsi_1d = calculate_rsi(klines_1d, 14);
        
        // 계층적 필터링
        if rsi_1d > 50.0 && rsi_1h < 30.0 && rsi_5m < 30.0 {
            return Ok(Signal::Buy);  // 일봉 상승 + 시간/분봉 과매도
        }
        
        Ok(Signal::Hold)
    }
}
```

**성능 목표**:
- 3개 타임프레임 조회: < 50ms (캐시 히트)
- 메모리 사용: < 10MB/전략
- 백테스트 정확도: 100% (실시간과 일치)

**효과**:
- 신호 신뢰도 향상 (장기 추세 + 단기 타이밍)
- 허위 신호 필터링 (멀티 TF 합의 필요)
- 전문적인 MTF 분석 기법 적용
- 전략 다양성 확대

**예상 시간**: 7주 (Phase 1-4: 4주 MVP, Phase 5-6: 3주 개선)

---

### 1.4.3. 전략 연계 (스크리닝 활용)

**[의존성: P1-A,P1-B,P1-C 완료 후]**

**구현 항목**
- [x] ✅ 전략에서 스크리닝 결과 활용 인터페이스 정의 → [screening_integration.rs](crates/trader-strategy/src/strategies/common/screening_integration.rs)
  - ⚠️ **미연동**: 전략에서 실제 호출하지 않음 (테스트에서만 사용)
- [ ] 코스닥 급등주 전략: ATTACK 상태 종목만 진입 ← **미연동**
- [ ] 스노우볼 전략: 저PBR+고배당 + Global Score 상위 ← **미연동**
- [ ] 섹터 모멘텀 전략: 섹터별 TOP 5 자동 선택 ← **미연동**
- [x] ✅ 참고 구현: `grid.rs`의 `can_enter()` 패턴 → [grid.rs:218-264](crates/trader-strategy/src/strategies/grid.rs#L218-L264)

**예상 시간**: 8시간 (전략 연동 작업)

---

## Phase 2:  프론트엔드 UI (병렬 가능)

**[의존성: P1 완료 후]**

> **병렬 실행**: Phase 1 완료 후, 아래 항목들은 서로 독립적이므로 동시 진행 가능

> **예상 시간**: 3주

### 2.1. Trading Journal UI ⭐ (백엔드 완료)

**페이지**: `TradingJournal.tsx`
- [ ] 보유 현황 테이블 (FIFO 원가, 평가손익)
- [ ] 체결 내역 타임라인
- [ ] 포지션 비중 차트 (파이/도넛)
- [ ] 손익 분석 대시보드 (일별/주별/월별/연도별)

**예상 시간**: 1주

---

### 2.2. Screening UI (백엔드 완료)

**페이지**: `Screening.tsx`
- [ ] 필터 조건 입력 폼
- [ ] 프리셋 선택 UI
- [ ] 스크리닝 결과 테이블 (정렬, 페이지네이션)
- [ ] **RouteState 뱃지 컴포넌트** (Phase 1 연동)
- [ ] 종목 상세 모달 (Fundamental + 미니 차트)

**예상 시간**: 1주

---

### 2.3. Global Ranking UI

**페이지**: `GlobalRanking.tsx`
- [ ] TOP 10 대시보드 위젯
- [ ] 시장별 필터 (KR-KOSPI, KR-KOSDAQ, US)
- [ ] **점수 구성 요소 시각화** (레이더 차트)
- [ ] **RouteState별 필터링**

**예상 시간**: 0.5주

---

### 2.4. 캔들 차트 신호 시각화 ⭐ 신규

**[의존성: P1-C1.1 완료 후]**

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

### 2.5. 대시보드 고급 시각화 ⭐ 신규

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

### 2.6. 프론트엔드 공통 개선

> **📋 프론트엔드 전체 작업 목록** (Phase 0 ~ Phase 2)

---

#### 6.1 UI 공통 컴포넌트 리팩토링 ⭐ 진행 중

> 참조 구현: `GlobalRanking.tsx`
> 공통 컴포넌트: `components/ui/` (Card, StatCard, PageHeader, EmptyState, ErrorState, Button 등)

---

##### 6.1.1 완료된 페이지

| 페이지 | 줄 수 | 적용 컴포넌트 | 감소량 |
|--------|------:|--------------|-------:|
| ✅ GlobalRanking.tsx | 400줄 | Card, StatCard, EmptyState, ErrorState | 참조 |
| ✅ Simulation.tsx | 963줄 | Card(6), StatCardGrid(6), EmptyState, ErrorState, PageHeader | ~100줄 |
| ✅ Backtest.tsx | 1148줄 | Card, EmptyState, Button | ~80줄 |
| ✅ Settings.tsx | 1384줄 | Card(5), Button (secondary, danger) | ~150줄 |

---

##### 6.1.2 대기 중인 페이지 상세

**TradingJournal.tsx** (345줄)

체크리스트:
- [ ] `PageHeader` 적용
  - title: "매매일지", icon: "📘"
  - actions: 새로고침, 필터 버튼
- [ ] `StatCardGrid` 적용 (4열)
  - 총 실현손익, 총 거래 수, 승률, 총 수수료
- [ ] 인라인 카드 → `Card`, `CardHeader`, `CardContent`
- [ ] 빈 상태 → `EmptyState`
- [ ] 버튼 → `Button` 컴포넌트
- [ ] `formatCurrency`, `getPnLColor` 유틸 사용

**Dashboard.tsx** (635줄)

체크리스트:
- [ ] `PageHeader` 적용
  - title: "대시보드", icon: "📊"
  - actions: 연결 상태 표시, 새로고침
- [ ] `StatCardGrid` 적용 (4열)
  - 총 자산, 일일 손익, 총 손익, 현금 잔고
- [ ] `PageLoader` 적용 (로딩 상태)
- [ ] `ErrorState` 적용 (API 에러)
- [ ] 인라인 카드 → `Card` 컴포넌트

**Strategies.tsx** (587줄)

체크리스트:
- [ ] `PageHeader` 적용
  - title: "전략 관리", icon: "⚙️"
  - actions: 새 전략 추가 버튼
- [ ] `FilterPanel` 적용 (카테고리 필터)
- [ ] `EmptyState` 적용 ("등록된 전략이 없습니다")
- [ ] `ErrorState` 적용
- [ ] 전략 카드 → `Card` 컴포넌트
- [ ] 버튼 → `Button` (primary, secondary, danger)

**Dataset.tsx** (777줄)

체크리스트:
- [ ] `PageHeader` 적용
  - title: "데이터셋", icon: "📁"
- [ ] `StatCardGrid` 적용 (4열)
  - 총 심볼 수, 데이터 기간, 마지막 업데이트, 데이터 크기
- [ ] `FilterPanel` 적용 (시장, 기간 필터)
- [ ] 인라인 카드 → `Card` 컴포넌트
- [ ] `DataTable` 적용 (심볼 목록)

**Screening.tsx** (907줄)

체크리스트:
- [ ] `PageHeader` 적용
  - title: "스크리닝", icon: "🔍"
  - actions: 프리셋 드롭다운
- [ ] `FilterPanel` 적용 (필터 조건)
- [ ] `DataTable` 적용 (결과 테이블)
- [ ] `EmptyState` 적용 ("조건에 맞는 종목이 없습니다")
- [ ] `ErrorState` 적용

**MLTraining.tsx** (642줄)

체크리스트:
- [ ] `PageHeader` 적용
  - title: "ML 학습", icon: "🤖"
  - actions: 학습 시작 버튼
- [ ] 인라인 카드 → `Card` 컴포넌트
- [ ] `EmptyState` 적용 ("학습된 모델이 없습니다")
- [ ] `ErrorState` 적용
- [ ] 프로그레스 → 커스텀 프로그레스 바

---

##### 6.1.3 공통 컴포넌트 목록

**위치**: `frontend/src/components/ui/`

| 컴포넌트 | 파일 | 용도 |
|----------|------|------|
| Card, CardHeader, CardContent | Card.tsx | 섹션 컨테이너 |
| StatCard, StatCardGrid | StatCard.tsx | 통계 표시 |
| PageHeader | PageHeader.tsx | 페이지 헤더 |
| EmptyState | StateDisplay.tsx | 빈 상태 표시 |
| ErrorState | StateDisplay.tsx | 에러 상태 표시 |
| PageLoader, Spinner | Loading.tsx | 로딩 상태 |
| Button | Form.tsx | 버튼 (primary, secondary, danger) |
| FilterPanel | Form.tsx | 필터 패널 |
| Select, Input | Form.tsx | 폼 요소 |
| DataTable | DataTable.tsx | 데이터 테이블 |

**유틸리티** (`components/ui/ChartUtils.tsx`):
- `formatNumber()`, `formatCurrency()`, `formatPercent()`
- `getPnLColor()`, `getPnLBgColor()`
- `chartColors`

**예상 시간**: 2일 (16시간)
| 페이지 | 시간 |
|--------|-----:|
| TradingJournal.tsx | 2h |
| Dashboard.tsx | 3h |
| Strategies.tsx | 3h |
| Dataset.tsx | 3h |
| Screening.tsx | 3h |
| MLTraining.tsx | 2h |
| **총계** | **16h** |

---

#### 6.2 SDUIRenderer 시스템 ⭐ (Phase 0 연동) - 🟢 핵심 구현 완료

> **[의존성: P0.4 SDUI 자동 생성 완료 ✅]**
> **목적**: 백엔드 스키마 기반 전략 설정 UI 자동 생성
> **참조**: `crates/trader-core/src/domain/schema.rs`
> **상태**: 핵심 컴포넌트 구현 완료 (v0.6.4), SymbolAutocomplete 및 통합 테스트 잔여

---

##### 6.2.1 백엔드 스키마 타입 (참조용)

```rust
// crates/trader-core/src/domain/schema.rs
pub struct StrategyUISchema {
    pub id: String,           // 전략 ID (예: "grid", "rsi")
    pub name: String,         // 표시 이름 (예: "그리드 전략")
    pub description: String,  // 설명
    pub category: StrategyCategory,
    pub fragments: Vec<FragmentRef>,  // 포함된 Fragment 목록
    pub custom_fields: Vec<FieldSchema>,  // 전략 고유 필드
    pub defaults: HashMap<String, Value>,  // 기본값
}

pub struct FragmentRef {
    pub id: String,      // Fragment ID (예: "base_config")
    pub required: bool,  // 필수 여부
}

pub struct FieldSchema {
    pub name: String,          // 필드 키 (예: "upper_limit")
    pub field_type: FieldType, // 타입
    pub label: String,         // 라벨 (예: "상한가")
    pub description: Option<String>,
    pub default: Option<Value>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub options: Option<Vec<SelectOption>>,
    pub condition: Option<String>,  // 조건부 표시
    pub required: bool,
}

pub enum FieldType {
    Integer, Number, Boolean, String,
    Select, MultiSelect, Symbol, Symbols
}
```

---

##### 6.2.2 프론트엔드 타입 정의

**파일**: `frontend/src/types/sdui.ts`

```typescript
// 백엔드 StrategyUISchema와 1:1 매핑
export interface StrategyUISchema {
  id: string;
  name: string;
  description: string;
  category: StrategyCategory;
  fragments: FragmentRef[];
  custom_fields: FieldSchema[];
  defaults: Record<string, unknown>;
}

export interface FragmentRef {
  id: string;
  required: boolean;
}

export interface FieldSchema {
  name: string;
  field_type: FieldType;
  label: string;
  description?: string;
  default?: unknown;
  min?: number;
  max?: number;
  options?: SelectOption[];
  condition?: string;  // 조건식 (예: "position_sizing_method == 'kelly'")
  required: boolean;
}

export type FieldType =
  | 'integer' | 'number' | 'boolean' | 'string'
  | 'select' | 'multi_select' | 'symbol' | 'symbols';

export interface SelectOption {
  value: string;
  label: string;
}

export type StrategyCategory = 'trend' | 'mean_reversion' | 'momentum' | 'hybrid' | 'ml';
```

- [x] `frontend/src/types/sdui.ts` 파일 생성 ✅ v0.6.4
- [x] 백엔드 스키마와 동기화 확인 ✅ v0.6.4

---

##### 6.2.3 useStrategySchema 훅

**파일**: `frontend/src/hooks/useStrategySchema.ts`

```typescript
export function useStrategySchema(strategyId: string) {
  const [schema, setSchema] = createSignal<StrategyUISchema | null>(null);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);

  // API 호출 + 캐싱 로직

  return { schema, loading, error, refetch };
}
```

체크리스트:
- [x] 훅 기본 구조 작성 ✅ v0.6.4
- [x] `GET /api/v1/strategies/{id}/schema` API 호출 ✅ v0.6.4
- [x] 스키마 캐싱 (Map 또는 createStore) ✅ v0.6.4 (5분 TTL 캐싱)
- [x] 로딩 상태 관리 (createSignal) ✅ v0.6.4
- [x] 에러 상태 관리 (createSignal) ✅ v0.6.4
- [x] `refetch()` 함수 (강제 재조회) ✅ v0.6.4
- [x] 타입 안전성 확보 (TypeScript) ✅ v0.6.4

---

##### 6.2.4 SDUIRenderer 메인 컴포넌트 ✅ 완료

**파일**: `frontend/src/components/strategy/SDUIRenderer/SDUIRenderer.tsx`

```typescript
interface SDUIRendererProps {
  strategyId: string;
  initialValues?: Record<string, unknown>;
  onChange?: (values: Record<string, unknown>) => void;
  onSubmit?: (values: Record<string, unknown>) => void;
  readOnly?: boolean;
}

export const SDUIRenderer: Component<SDUIRendererProps> = (props) => {
  const { schema, loading, error } = useStrategySchema(props.strategyId);
  const [values, setValues] = createStore<Record<string, unknown>>({});
  const [errors, setErrors] = createStore<Record<string, string>>({});

  // 초기값 병합 (defaults + initialValues)
  // Fragment별 SDUISection 렌더링
  // custom_fields SDUISection 렌더링
  // 유효성 검증 + 제출
};
```

체크리스트:
- [x] 컴포넌트 기본 구조 작성 ✅ v0.6.4
- [x] `useStrategySchema` 훅 연동 ✅ v0.6.4
- [x] 로딩 상태 표시 (Spinner) ✅ v0.6.4
- [x] 에러 상태 표시 (ErrorState) ✅ v0.6.4
- [x] 초기값 병합 로직 ✅ v0.6.4
  - [x] `schema.defaults` 기본값 적용 ✅ v0.6.4
  - [x] `props.initialValues` 우선 적용 ✅ v0.6.4
- [x] Fragment 목록 순회 → `SDUISection` 렌더링 ✅ v0.6.4
- [x] `custom_fields` → `SDUISection` 렌더링 ✅ v0.6.4
- [x] `values` 상태 관리 (createStore) ✅ v0.6.4
- [x] `errors` 상태 관리 (createStore) ✅ v0.6.4
- [x] `onChange` 콜백 호출 ✅ v0.6.4
- [x] `onSubmit` 콜백 + 전체 유효성 검증 ✅ v0.6.4
- [x] `readOnly` 모드 지원 ✅ v0.6.4

---

##### 6.2.5 SDUISection 컴포넌트 ✅ 완료

**파일**: `frontend/src/components/strategy/SDUIRenderer/SDUISection.tsx`

```typescript
interface SDUISectionProps {
  fragment: FragmentRef;
  fields: FieldSchema[];
  values: Record<string, unknown>;
  errors: Record<string, string>;
  onChange: (name: string, value: unknown) => void;
  readOnly?: boolean;
}

export const SDUISection: Component<SDUISectionProps> = (props) => {
  const [collapsed, setCollapsed] = createSignal(!props.fragment.required);

  // Card + CardHeader + CardContent 구조
  // 접힘/펼침 토글 버튼
  // 필드 목록 렌더링 (SDUIField)
};
```

체크리스트:
- [x] 컴포넌트 기본 구조 작성 ✅ v0.6.4
- [x] `Card`, `CardHeader`, `CardContent` 사용 ✅ v0.6.4
- [x] 섹션 제목 표시 (Fragment name) ✅ v0.6.4
- [x] 섹션 설명 표시 (Fragment description) ✅ v0.6.4
- [x] 접힘/펼침 토글 버튼 ✅ v0.6.4
  - [x] 필수 섹션: 기본 펼침 ✅ v0.6.4
  - [x] 선택 섹션: 기본 접힘, 토글 가능 ✅ v0.6.4
- [x] 필드 목록 순회 → `SDUIField` 렌더링 ✅ v0.6.4
- [x] 조건부 필드 필터링 (`condition` 평가) ✅ v0.6.4
- [x] 필수 마크 (*) 표시 ✅ v0.6.4

---

##### 6.2.6 SDUIField 컴포넌트 ✅ 완료

**파일**: `frontend/src/components/strategy/SDUIRenderer/SDUIField.tsx`

```typescript
interface SDUIFieldProps {
  field: FieldSchema;
  value: unknown;
  error?: string;
  onChange: (value: unknown) => void;
  readOnly?: boolean;
}

export const SDUIField: Component<SDUIFieldProps> = (props) => {
  // field.field_type에 따라 적절한 입력 컴포넌트 렌더링
  return (
    <div class="mb-4">
      <label>{props.field.label}</label>
      <Switch>
        <Match when={props.field.field_type === 'integer'}>
          <NumberInput step={1} ... />
        </Match>
        <Match when={props.field.field_type === 'number'}>
          <NumberInput step={0.01} ... />
        </Match>
        {/* 나머지 타입들 */}
      </Switch>
      <Show when={props.error}>
        <span class="text-red-500">{props.error}</span>
      </Show>
    </div>
  );
};
```

체크리스트:
- [x] 컴포넌트 기본 구조 작성 ✅ v0.6.4
- [x] 라벨 + 필수 마크 표시 ✅ v0.6.4
- [x] 설명 텍스트 표시 (description) ✅ v0.6.4
- [x] 에러 메시지 표시 ✅ v0.6.4
- [x] **필드 타입별 입력 컴포넌트**: ✅ v0.6.4
  - [x] `integer` → `<input type="number" step="1" />` ✅ v0.6.4
  - [x] `number` → `<input type="number" step="0.01" />` ✅ v0.6.4
  - [x] `boolean` → `<ToggleSwitch />` 컴포넌트 ✅ v0.6.4
  - [x] `string` → `<input type="text" />` ✅ v0.6.4
  - [x] `select` → `<SelectInput />` 컴포넌트 ✅ v0.6.4
  - [x] `multi_select` → `<MultiSelectInput />` 컴포넌트 ✅ v0.6.4
  - [ ] `symbol` → `<SymbolAutocomplete />` (6.2.8) - 기본 TextInput 사용 중
  - [x] `symbols` → `<MultiSymbolInput />` (6.2.9) ✅ v0.6.4 (자동완성 미적용)
- [x] `min`, `max` 속성 적용 (number 타입) ✅ v0.6.4
- [x] `readOnly` 모드 지원 ✅ v0.6.4
- [x] 범위 힌트 표시 (최소/최대) ✅ v0.6.4

---

##### 6.2.7 SDUIValidation 유틸리티 ✅ 완료

**파일**: `frontend/src/components/strategy/SDUIRenderer/SDUIValidation.ts`

```typescript
export function validateField(
  field: FieldSchema,
  value: unknown
): string | null {
  // required 검증
  // min/max 검증
  // 타입별 추가 검증
  return null; // 또는 에러 메시지
}

export function validateAllFields(
  fields: FieldSchema[],
  values: Record<string, unknown>
): Record<string, string> {
  // 모든 필드 검증 후 에러 맵 반환
}

export function evaluateCondition(
  condition: string,
  values: Record<string, unknown>
): boolean {
  // 조건식 평가 (예: "position_sizing_method == 'kelly'")
}
```

체크리스트:
- [x] `validateField()` 함수 ✅ v0.6.4
  - [x] `required` 검증 (빈 값 체크) ✅ v0.6.4
  - [x] `min` 검증 (number/integer) ✅ v0.6.4
  - [x] `max` 검증 (number/integer) ✅ v0.6.4
  - [x] `options` 검증 (select/multi_select) ✅ v0.6.4
- [x] `validateAllFields()` 함수 ✅ v0.6.4
- [x] `evaluateCondition()` 함수 ✅ v0.6.4
  - [x] 간단한 비교 연산 파싱 (`==`, `!=`, `>`, `<`, `>=`, `<=`) ✅ v0.6.4
  - [x] 필드값 참조 ✅ v0.6.4
- [x] 에러 메시지 한글화 ✅ v0.6.4
- [x] `getDefaultValueForType()` 유틸리티 ✅ v0.6.4
- [x] `coerceValue()` 타입 변환 유틸리티 ✅ v0.6.4

---

##### 6.2.8 SymbolAutocomplete 컴포넌트 ✅ 완료

**파일**: `frontend/src/components/strategy/SDUIRenderer/fields/SymbolAutocomplete.tsx`

```typescript
interface SymbolAutocompleteProps {
  value: string;
  onChange: (symbol: string) => void;
  market?: 'KR' | 'US' | 'CRYPTO' | 'ALL';
  readOnly?: boolean;
}
```

체크리스트:
- [x] 컴포넌트 기본 구조 작성 ✅ v0.6.4
- [x] 입력 필드 + 드롭다운 구조 ✅ v0.6.4
- [x] 심볼 검색 API 연동 (`GET /api/v1/dataset/search`) ✅ v0.6.4
- [x] 디바운스 적용 (300ms) ✅ v0.6.4
- [x] 검색 결과 목록 표시 ✅ v0.6.4
- [x] 시장별 필터링 (market prop) ✅ v0.6.4
- [x] 키보드 네비게이션 (↑↓ Enter Escape) ✅ v0.6.4
- [x] 선택 시 심볼명 + 종목명 표시 ✅ v0.6.4

---

##### 6.2.9 MultiSymbolInput 컴포넌트 ✅ 완료

**파일**: `frontend/src/components/strategy/SDUIRenderer/SDUIField.tsx` (내부 컴포넌트)

```typescript
interface MultiSymbolInputProps {
  value: string[];
  onChange: (symbols: string[]) => void;
  readOnly?: boolean;
}
```

체크리스트:
- [x] 컴포넌트 기본 구조 작성 ✅ v0.6.4
- [x] 선택된 심볼 태그 표시 ✅ v0.6.4
- [x] 태그 삭제 버튼 (X) ✅ v0.6.4
- [x] `SymbolAutocomplete` 재사용 (추가용) ✅ v0.6.4
- [x] 중복 추가 방지 ✅ v0.6.4
- [ ] 최대 개수 제한 (`maxCount`) - 선택사항
- [ ] 드래그 앤 드롭 순서 변경 - 선택사항

---

##### 6.2.10 API 함수

**파일**: `frontend/src/api/schema.ts`

```typescript
export async function getStrategySchema(
  strategyId: string
): Promise<StrategyUISchema> {
  const response = await fetch(`/api/v1/strategies/${strategyId}/schema`);
  if (!response.ok) throw new Error('Failed to fetch schema');
  return response.json();
}

export async function getFragments(
  category?: string
): Promise<SchemaFragment[]> {
  const url = category
    ? `/api/v1/schema/fragments/${category}`
    : '/api/v1/schema/fragments';
  const response = await fetch(url);
  if (!response.ok) throw new Error('Failed to fetch fragments');
  return response.json();
}
```

체크리스트:
- [x] `getStrategySchema()` 함수 ✅ v0.6.4
- [x] `getFragments()` 함수 ✅ v0.6.4
- [x] `getFragmentDetail()` 함수 ✅ v0.6.4
- [x] `getFragmentDetails()` 함수 (배치 조회) ✅ v0.6.4
- [x] 에러 처리 (HTTP 상태 코드) ✅ v0.6.4
- [x] 타입 안전성 확보 ✅ v0.6.4

---

##### 6.2.11 통합 및 테스트 ✅ 완료

체크리스트:
- [x] 백엔드 스키마 API 엔드포인트 추가 ✅ v0.6.4
  - `GET /api/v1/strategies/meta` - 전략 메타데이터 목록
  - `GET /api/v1/strategies/{id}/schema` - 전략 SDUI 스키마
- [x] `SDUIEditModal` 컴포넌트 생성 ✅ v0.6.4
  - SDUIRenderer 기반 전략 편집 모달
- [x] `Strategies.tsx` 페이지에서 SDUIEditModal 활성화 ✅ v0.6.4
  - EditStrategyModal → SDUIEditModal 교체 완료
- [ ] 전략 추가 모달에 적용 (선택사항)
- [ ] 백테스트 설정에 적용 (선택사항)
- [ ] 스키마 없는 전략 fallback UI (필요시)
- [ ] 브라우저 테스트 (Chrome, Firefox, Safari)
- [ ] 반응형 레이아웃 확인

---

**파일 구조 (현재)**:
```
frontend/src/
├── types/
│   └── sdui.ts                    # SDUI 타입 정의 ✅
├── api/
│   └── schema.ts                  # 스키마 API 함수 ✅
├── hooks/
│   └── useStrategySchema.ts       # 스키마 조회 훅 ✅
├── components/
│   └── SDUIEditModal.tsx          # SDUI 기반 편집 모달 ✅
└── components/strategy/SDUIRenderer/
    ├── index.ts                   # export ✅
    ├── SDUIRenderer.tsx           # 메인 컨테이너 ✅
    ├── SDUISection.tsx            # 섹션 렌더링 ✅
    ├── SDUIField.tsx              # 필드 렌더링 ✅
    ├── SDUIValidation.ts          # 유효성 검증 ✅
    └── fields/
        ├── index.ts               # 필드 컴포넌트 export ✅
        └── SymbolAutocomplete.tsx # 심볼 자동완성 ✅
    ├── SDUISection.tsx            # 섹션 렌더링
    ├── SDUIField.tsx              # 필드 렌더링
    ├── SDUIValidation.ts          # 유효성 검증
    └── fields/
        ├── SymbolAutocomplete.tsx # 심볼 자동완성
        └── MultiSymbolInput.tsx   # 다중 심볼 입력
```

**예상 시간**: 3일 (24시간)
| 항목 | 시간 |
|------|-----:|
| 타입 정의 + API 함수 | 2h |
| useStrategySchema 훅 | 2h |
| SDUIRenderer 메인 | 4h |
| SDUISection | 3h |
| SDUIField (8개 타입) | 6h |
| SDUIValidation | 2h |
| SymbolAutocomplete | 3h |
| MultiSymbolInput | 2h |
| **총계** | **24h** |

---

#### 6.3 Trading Journal UI 기능 (2.1 연동)

> **[의존성: 백엔드 API 완료 ✅]**
> **페이지**: `frontend/src/pages/TradingJournal.tsx`

---

##### 6.3.1 UI 리팩토링 (공통 컴포넌트 적용)

**파일**: `TradingJournal.tsx` (현재 345줄)

체크리스트:
- [ ] `PageHeader` 적용
  - title: "매매일지"
  - icon: "📘"
  - description: "체결 내역과 손익 분석"
  - actions: 새로고침, 필터 버튼
- [ ] `StatCardGrid` 적용 (4열)
  - [ ] 총 실현손익 (formatCurrency, getPnLColor)
  - [ ] 총 거래 수
  - [ ] 승률 (%)
  - [ ] 총 수수료
- [ ] 인라인 카드 스타일 → `Card`, `CardHeader`, `CardContent`
- [ ] 빈 상태 → `EmptyState` ("거래 내역이 없습니다")
- [ ] 에러 상태 → `ErrorState`
- [ ] 버튼 → `Button` 컴포넌트

---

##### 6.3.2 보유 현황 테이블

체크리스트:
- [ ] `DataTable` 컴포넌트 사용
- [ ] 컬럼 정의:
  | 컬럼 | 키 | 정렬 | 포맷 |
  |------|-----|:----:|------|
  | 종목명 | symbol | ✅ | - |
  | 수량 | quantity | ✅ | 천 단위 콤마 |
  | 평균 단가 | avg_price | ✅ | 통화 |
  | 현재가 | current_price | - | 통화 |
  | 평가 금액 | market_value | ✅ | 통화 |
  | 평가 손익 | unrealized_pnl | ✅ | 통화 + 색상 |
  | 수익률 | return_rate | ✅ | % + 색상 |
- [ ] FIFO 원가 표시 (CostBasisTracker 연동)
- [ ] 행 클릭 → 상세 모달
- [ ] 포지션 비중 막대 표시

---

##### 6.3.3 체결 내역 타임라인

체크리스트:
- [ ] 날짜별 그룹핑
- [ ] 타임라인 UI (세로 선 + 노드)
- [ ] 각 체결 노드:
  - 시간 (HH:mm)
  - 종목명
  - 매수/매도 구분 (색상)
  - 수량 x 가격
  - 체결 금액
- [ ] 무한 스크롤 (페이지네이션)
- [ ] 필터: 날짜 범위, 종목, 매수/매도

---

##### 6.3.4 포지션 비중 차트 (파이/도넛)

체크리스트:
- [ ] 차트 라이브러리 선택 (Chart.js / D3 / uPlot)
- [ ] 도넛 차트 컴포넌트
- [ ] 데이터: 종목별 평가 금액 비중
- [ ] 호버 시 툴팁 (종목명, 금액, 비중%)
- [ ] 범례 (상위 10개 + 기타)
- [ ] 클릭 시 해당 종목 상세

---

##### 6.3.5 손익 분석 대시보드

체크리스트:
- [ ] 기간 선택 탭 (일/주/월/연도)
- [ ] 손익 바 차트 (기간별 실현 손익)
- [ ] 누적 손익 라인 차트
- [ ] 통계 테이블:
  | 지표 | 설명 |
  |------|------|
  | 총 거래 수 | - |
  | 승/패 | - |
  | 승률 | % |
  | 평균 수익 | 원 |
  | 평균 손실 | 원 |
  | Profit Factor | 총이익/총손실 |
  | 최대 연속 승 | - |
  | 최대 연속 패 | - |
  | Max Drawdown | % |
- [ ] `statistics.rs` 함수 활용 (백엔드)

**예상 시간**: 1주 (40시간)
| 항목 | 시간 |
|------|-----:|
| UI 리팩토링 | 4h |
| 보유 현황 테이블 | 8h |
| 체결 내역 타임라인 | 10h |
| 포지션 비중 차트 | 8h |
| 손익 분석 대시보드 | 10h |
| **총계** | **40h** |

---

#### 6.4 Screening UI 기능 (2.2 연동)

> **[의존성: 백엔드 API 완료 ✅]**
> **페이지**: `frontend/src/pages/Screening.tsx` (현재 907줄)

---

##### 6.4.1 UI 리팩토링 (공통 컴포넌트 적용)

체크리스트:
- [ ] `PageHeader` 적용
  - title: "스크리닝"
  - icon: "🔍"
  - description: "조건 기반 종목 필터링"
- [ ] `FilterPanel` 적용
- [ ] `DataTable` 적용
- [ ] 빈 상태 → `EmptyState` ("조건에 맞는 종목이 없습니다")
- [ ] 에러 상태 → `ErrorState`

---

##### 6.4.2 필터 조건 입력 폼

**컴포넌트**: `ScreeningFilterForm.tsx`

체크리스트:
- [ ] 기본 필터:
  | 필터 | 타입 | UI |
  |------|------|-----|
  | 시장 | select | KR-KOSPI, KR-KOSDAQ, US |
  | 섹터 | multi_select | 체크박스 그룹 |
  | 시가총액 | range | min~max 슬라이더 |
  | 거래대금 | range | min~max 슬라이더 |
- [ ] 기술 지표 필터:
  | 필터 | 타입 | 범위 |
  |------|------|------|
  | RSI | range | 0~100 |
  | MACD | select | 골든크로스/데드크로스/전체 |
  | 이격도 | range | 80~120 |
  | 20일선 위치 | select | 상회/하회/전체 |
- [ ] 조건 추가/삭제 버튼
- [ ] 조건 AND/OR 토글
- [ ] 필터 초기화 버튼
- [ ] 검색 버튼 (+ 단축키 Enter)

---

##### 6.4.3 프리셋 선택 UI

**컴포넌트**: `ScreeningPresets.tsx`

체크리스트:
- [ ] 프리셋 드롭다운 (저장된 필터 목록)
- [ ] 프리셋 적용 시 필터 폼에 반영
- [ ] 새 프리셋 저장 버튼 → 모달
  - 프리셋 이름 입력
  - 현재 필터 조건 저장
- [ ] 프리셋 삭제 (확인 다이얼로그)
- [ ] 기본 프리셋 (시스템 제공):
  - "과매도 종목" (RSI < 30)
  - "돌파 임박" (볼린저 밴드 하단 근접)
  - "거래량 급증" (전일 대비 200%+)

---

##### 6.4.4 스크리닝 결과 테이블

체크리스트:
- [ ] `DataTable` 컴포넌트 사용
- [ ] 컬럼 정의:
  | 컬럼 | 키 | 정렬 | 포맷 |
  |------|-----|:----:|------|
  | 종목코드 | symbol | ✅ | - |
  | 종목명 | name | ✅ | - |
  | 현재가 | price | ✅ | 통화 |
  | 전일비 | change_rate | ✅ | % + 색상 |
  | 거래량 | volume | ✅ | 축약 (K, M) |
  | RSI | rsi | ✅ | 소수점 1자리 |
  | RouteState | route_state | ✅ | 뱃지 |
  | Global Score | total_score | ✅ | 소수점 2자리 |
- [ ] 정렬 기능 (서버 사이드)
- [ ] 페이지네이션 (20/50/100개)
- [ ] 행 클릭 → 종목 상세 모달
- [ ] 체크박스 선택 (일괄 작업용)
- [ ] 컬럼 표시/숨김 설정

---

##### 6.4.5 RouteState 뱃지 컴포넌트

**파일**: `frontend/src/components/ui/RouteStateBadge.tsx`

```typescript
interface RouteStateBadgeProps {
  state: 'ATTACK' | 'ARMED' | 'WATCH' | 'NONE';
  size?: 'sm' | 'md';
}
```

체크리스트:
- [ ] 컴포넌트 생성
- [ ] 상태별 스타일:
  | 상태 | 배경색 | 텍스트 |
  |------|--------|--------|
  | ATTACK | bg-red-500 | 공격 |
  | ARMED | bg-orange-500 | 대기 |
  | WATCH | bg-yellow-500 | 관찰 |
  | NONE | bg-gray-400 | - |
- [ ] 크기 변형 (sm, md)
- [ ] 툴팁 (상태 설명)
- [ ] `components/ui/index.ts`에 export 추가

---

##### 6.4.6 종목 상세 모달

**컴포넌트**: `SymbolDetailModal.tsx`

체크리스트:
- [ ] 모달 기본 구조 (Header, Body, Footer)
- [ ] 탭 구성:
  - [ ] **개요** 탭
    - 종목명, 시장, 섹터
    - 현재가, 전일비, 거래량
    - 52주 최고/최저
  - [ ] **지표** 탭
    - RSI, MACD, 볼린저 밴드
    - 이동평균선 (5, 20, 60, 120)
    - Global Score 구성 요소
  - [ ] **차트** 탭
    - 미니 캔들 차트 (최근 60일)
    - 거래량 바 차트
- [ ] 액션 버튼: 관심종목 추가, 전략 연결

**예상 시간**: 1주 (40시간)
| 항목 | 시간 |
|------|-----:|
| UI 리팩토링 | 4h |
| 필터 조건 입력 폼 | 10h |
| 프리셋 선택 UI | 6h |
| 결과 테이블 | 8h |
| RouteState 뱃지 | 2h |
| 종목 상세 모달 | 10h |
| **총계** | **40h** |

---

#### 6.5 Global Ranking UI 기능 (2.3 연동)

> **페이지**: `frontend/src/pages/GlobalRanking.tsx` (참조 구현 완료 ✅)
> **상태**: 기본 UI 완료, 고급 기능 추가 필요

---

##### 6.5.1 현재 완료 상태

- [x] `Card`, `CardHeader`, `CardContent` 적용 ✅
- [x] `StatCard`, `StatCardGrid` 적용 ✅
- [x] `EmptyState`, `ErrorState` 적용 ✅
- [x] 기본 랭킹 테이블 표시 ✅

---

##### 6.5.2 TOP 10 대시보드 위젯

**컴포넌트**: `RankingWidget.tsx` (대시보드용 소형 위젯)

체크리스트:
- [ ] 컴포넌트 생성 (`components/ranking/RankingWidget.tsx`)
- [ ] TOP 10 종목 목록 (순위, 종목명, 점수)
- [ ] 축약 표시 (컴팩트 모드)
- [ ] "더 보기" 링크 → GlobalRanking 페이지
- [ ] 자동 갱신 (옵션, 30초 간격)
- [ ] Dashboard.tsx에 통합

---

##### 6.5.3 시장별 필터

체크리스트:
- [ ] 필터 버튼 그룹:
  | 필터 | 값 |
  |------|-----|
  | 전체 | ALL |
  | 한국 | KR |
  | ├ KOSPI | KR-KOSPI |
  | └ KOSDAQ | KR-KOSDAQ |
  | 미국 | US |
  | 암호화폐 | CRYPTO |
- [ ] 다중 선택 가능
- [ ] URL 쿼리 파라미터 동기화
- [ ] 선택 상태 유지 (localStorage)

---

##### 6.5.4 점수 구성 요소 시각화 (RadarChart7Factor)

**컴포넌트**: `frontend/src/components/charts/RadarChart7Factor.tsx`

> ✅ **기본 RadarChart 구현 완료** (2026-02-03)
> - 5축 버전 구현 (technical, momentum, trend, volume, volatility)
> - 파일: `frontend/src/components/ui/RadarChart.tsx`
> - TopRankCard 및 GlobalRanking 페이지에 통합됨

```typescript
interface RadarChart7FactorProps {
  data: {
    norm_momentum: number;
    norm_value: number;
    norm_quality: number;
    norm_volatility: number;
    norm_liquidity: number;
    norm_growth: number;
    norm_sentiment: number;
  };
  size?: 'sm' | 'md' | 'lg';
}
```

체크리스트:
- [x] 기본 RadarChart 구현 ✅ (5축 버전)
- [ ] 7개 축 레이더 차트 확장 (백엔드 7Factor 데이터 필요)
  | 축 | 필드 | 라벨 |
  |-----|------|------|
  | 1 | norm_momentum | 모멘텀 |
  | 2 | norm_value | 가치 |
  | 3 | norm_quality | 품질 |
  | 4 | norm_volatility | 변동성 |
  | 5 | norm_liquidity | 유동성 |
  | 6 | norm_growth | 성장 |
  | 7 | norm_sentiment | 심리 |
- [ ] 0~100 범위 정규화 표시
- [ ] 각 축 라벨 + 값 표시
- [ ] 평균선 (50) 참조선
- [ ] 크기 변형 (sm: 120px, md: 200px, lg: 300px)
- [ ] 랭킹 테이블 행 클릭 시 팝업 표시

---

##### 6.5.5 RouteState별 필터링

> ✅ **기본 필터링 구현 완료** (2026-02-03)
> - 단일 선택 드롭다운 구현
> - 백엔드 API 연동 (`route_state=ATTACK`)
> - 실시간 RouteState 계산 로직 추가

체크리스트:
- [x] 필터 UI (Select 드롭다운) ✅
- [x] API 쿼리 파라미터 연동 ✅
- [x] 백엔드 실시간 계산 로직 ✅
- [ ] 필터 버튼 그룹으로 변경 (RouteStateBadge 재사용)
- [ ] 다중 선택 가능 (`route_states=ATTACK,ARMED`)
- [ ] 선택된 RouteState 카운트 표시

---

##### 6.5.6 추가 기능

체크리스트:
- [ ] 순위 변동 표시 (↑↓ 화살표 + 변동폭)
- [ ] 종목 즐겨찾기 토글
- [ ] Excel 내보내기 버튼
- [ ] 자동 갱신 토글 (30초/1분/5분)

**예상 시간**: 0.5주 (20시간)
| 항목 | 시간 |
|------|-----:|
| TOP 10 위젯 | 4h |
| 시장별 필터 | 3h |
| RadarChart7Factor | 8h |
| RouteState 필터 | 3h |
| 추가 기능 | 2h |
| **총계** | **20h** |

---

#### 6.6 캔들 차트 신호 시각화 (2.4)

> **[의존성: P1-C1.1 SignalMarker 완료 후]**
> **목적**: 과거 캔들 데이터에서 기술 신호 발생 지점을 시각적으로 표시

---

##### 6.6.1 SignalMarkerOverlay 컴포넌트

**파일**: `frontend/src/components/charts/SignalMarkerOverlay.tsx`

```typescript
interface SignalMarker {
  timestamp: number;
  price: number;
  signal_type: 'buy' | 'sell' | 'alert';
  strength: number;  // 0.0 ~ 1.0
  indicator: string;  // 'RSI', 'MACD', etc.
  strategy_id?: string;
  route_state?: RouteState;
  metadata?: Record<string, unknown>;
}

interface SignalMarkerOverlayProps {
  markers: SignalMarker[];
  chartRef: RefObject<ChartInstance>;
  onMarkerClick?: (marker: SignalMarker) => void;
  visibleTypes?: ('buy' | 'sell' | 'alert')[];
}
```

체크리스트:
- [ ] 컴포넌트 기본 구조
- [ ] 차트 좌표계와 동기화 (x: timestamp, y: price)
- [ ] 마커 아이콘 렌더링:
  | 타입 | 아이콘 | 색상 | 위치 |
  |------|--------|------|------|
  | buy | ▲ (위 화살표) | #10B981 (초록) | 캔들 아래 |
  | sell | ▼ (아래 화살표) | #EF4444 (빨강) | 캔들 위 |
  | alert | ● (원) | #F59E0B (노랑) | 캔들 위 |
- [ ] 강도에 따른 크기 조절 (strength)
- [ ] 마커 호버 시 하이라이트
- [ ] 마커 클릭 이벤트 (`onMarkerClick`)
- [ ] 줌/팬 시 마커 위치 업데이트
- [ ] 대량 마커 최적화 (가상화)

---

##### 6.6.2 SignalDetailPopup 컴포넌트

**파일**: `frontend/src/components/charts/SignalDetailPopup.tsx`

```typescript
interface SignalDetailPopupProps {
  marker: SignalMarker;
  position: { x: number; y: number };
  onClose: () => void;
}
```

체크리스트:
- [ ] 팝업 기본 구조 (Card 스타일)
- [ ] 표시 내용:
  | 항목 | 설명 |
  |------|------|
  | 신호 유형 | 매수/매도/알림 + 뱃지 |
  | 발생 시간 | YYYY-MM-DD HH:mm |
  | 가격 | 해당 시점 가격 |
  | 강도 | 0~100% (프로그레스 바) |
  | 지표 | RSI, MACD 등 |
  | 지표 값 | 해당 시점 지표 값 |
  | RouteState | 뱃지 표시 |
  | 전략 | 전략 이름 (있는 경우) |
  | 실행 여부 | 체결됨/미체결 |
- [ ] 외부 클릭 시 닫기
- [ ] ESC 키 닫기
- [ ] 화면 경계 자동 조정 (팝업 위치)

---

##### 6.6.3 IndicatorFilterPanel 컴포넌트

**파일**: `frontend/src/components/charts/IndicatorFilterPanel.tsx`

```typescript
interface IndicatorFilterPanelProps {
  filters: IndicatorFilters;
  onChange: (filters: IndicatorFilters) => void;
}

interface IndicatorFilters {
  signal_types: ('buy' | 'sell' | 'alert')[];
  indicators: string[];  // 'RSI', 'MACD', etc.
  rsi_range?: [number, number];  // [0, 100]
  macd_type?: 'golden' | 'dead' | 'all';
  route_states?: RouteState[];
  strategies?: string[];
  date_range?: [Date, Date];
}
```

체크리스트:
- [ ] 접힘 가능한 필터 패널
- [ ] 신호 타입 체크박스 (매수/매도/알림)
- [ ] 지표 선택 (다중):
  | 지표 | 추가 필터 |
  |------|----------|
  | RSI | 범위 슬라이더 (0~100) |
  | MACD | 크로스 타입 (골든/데드/전체) |
  | Bollinger | 위치 (상단/하단/전체) |
  | Volume | 급증 배율 (1x~5x) |
- [ ] RouteState 필터 (뱃지 버튼)
- [ ] 전략 필터 (드롭다운)
- [ ] 날짜 범위 필터 (DatePicker)
- [ ] 필터 초기화 버튼
- [ ] 필터 프리셋 저장/불러오기

---

##### 6.6.4 백테스트 결과 차트 통합

**파일**: `frontend/src/pages/Backtest.tsx` (결과 섹션)

체크리스트:
- [ ] `CandlestickChart` 컴포넌트에 `SignalMarkerOverlay` 통합
  ```tsx
  <CandlestickChart data={candles}>
    <SignalMarkerOverlay
      markers={backtest.signal_markers}
      onMarkerClick={handleMarkerClick}
    />
    <EquityCurveOverlay data={backtest.equity_curve} />
  </CandlestickChart>
  ```
- [ ] 진입/청산 포인트 연결선 (선택적)
- [ ] 손익 구간 배경색 표시
- [ ] 마커 필터 패널 통합
- [ ] 마커 상세 팝업 연동

---

##### 6.6.5 종목 상세 페이지 통합

체크리스트:
- [ ] 종목 상세 페이지에 과거 신호 차트 추가
- [ ] 최근 N일 신호 목록 테이블
- [ ] 신호 발생 통계 (타입별 카운트)
- [ ] 신호→실제 수익률 상관관계 표시

**예상 시간**: 1주 (40시간)
| 항목 | 시간 |
|------|-----:|
| SignalMarkerOverlay | 12h |
| SignalDetailPopup | 6h |
| IndicatorFilterPanel | 10h |
| 백테스트 차트 통합 | 8h |
| 종목 상세 통합 | 4h |
| **총계** | **40h** |

---

#### 6.7 대시보드 고급 시각화 컴포넌트 (2.5)

> **디렉토리**: `frontend/src/components/charts/`
> **목적**: 고급 시각화 기능을 프론트엔드에 구현

---

##### 6.7.1 시장 심리 지표

**FearGreedGauge 컴포넌트**

**파일**: `frontend/src/components/charts/FearGreedGauge.tsx`

```typescript
interface FearGreedGaugeProps {
  value: number;  // 0~100
  size?: 'sm' | 'md' | 'lg';
  showLabel?: boolean;
}
```

체크리스트:
- [ ] 반원형 게이지 UI (D3.js 또는 SVG)
- [ ] 5단계 색상 구간:
  | 범위 | 라벨 | 색상 |
  |------|------|------|
  | 0~20 | 극단적 공포 | #EF4444 |
  | 21~40 | 공포 | #F97316 |
  | 41~60 | 중립 | #FCD34D |
  | 61~80 | 탐욕 | #84CC16 |
  | 81~100 | 극단적 탐욕 | #22C55E |
- [ ] 바늘(needle) 애니메이션
- [ ] 현재 값 + 라벨 표시
- [ ] 전일 대비 변화 표시 (↑↓)
- [ ] 크기 변형 (sm: 100px, md: 150px, lg: 200px)

**MarketBreadthWidget 컴포넌트**

**파일**: `frontend/src/components/charts/MarketBreadthWidget.tsx`

```typescript
interface MarketBreadthWidgetProps {
  aboveSma20: number;  // 20일선 상회 종목 수
  total: number;        // 전체 종목 수
  market?: string;
}
```

체크리스트:
- [ ] 프로그레스 바 스타일 표시
- [ ] 비율 계산 + 표시 (예: "65% (1,234 / 1,899)")
- [ ] 색상 구간 (30% 미만: 빨강, 70% 초과: 초록)
- [ ] 시장별 필터 지원
- [ ] 히스토리 미니 차트 (최근 20일)

---

##### 6.7.2 팩터 분석 차트

**RadarChart7Factor 컴포넌트** → 6.5.4에서 상세화 완료

**ScoreWaterfall 컴포넌트**

**파일**: `frontend/src/components/charts/ScoreWaterfall.tsx`

```typescript
interface ScoreWaterfallProps {
  symbol: string;
  factors: {
    name: string;
    contribution: number;  // 양수/음수
  }[];
  totalScore: number;
}
```

체크리스트:
- [ ] 워터폴 차트 구현 (막대 차트 변형)
- [ ] 각 팩터 기여도 막대 표시:
  | 팩터 | 기여도 예시 |
  |------|------------|
  | 모멘텀 | +15 |
  | 가치 | +8 |
  | 품질 | +12 |
  | 변동성 | -5 |
  | 유동성 | +3 |
  | 성장 | +10 |
  | 심리 | -2 |
  | **합계** | **41** |
- [ ] 양수: 초록색, 음수: 빨간색
- [ ] 누적 막대 연결선
- [ ] 최종 점수 강조 표시
- [ ] 호버 시 상세 툴팁

**KellyVisualization 컴포넌트**

**파일**: `frontend/src/components/charts/KellyVisualization.tsx`

```typescript
interface KellyVisualizationProps {
  kellyFraction: number;  // 0.0 ~ 1.0
  currentAllocation: number;  // 0.0 ~ 1.0
  maxRisk?: number;  // 제한값
}
```

체크리스트:
- [ ] 수평 바 차트 (0% ~ 100%)
- [ ] 켈리 비율 마커 (이론적 최적)
- [ ] 현재 배분 비율 마커
- [ ] 위험 한도 영역 표시
- [ ] 과대/과소 배분 경고 색상
- [ ] 툴팁: 켈리 공식 설명

---

##### 6.7.3 포트폴리오 분석

**CorrelationHeatmap 컴포넌트**

**파일**: `frontend/src/components/charts/CorrelationHeatmap.tsx`

```typescript
interface CorrelationHeatmapProps {
  symbols: string[];
  correlations: number[][];  // N x N 행렬
  onCellClick?: (i: number, j: number) => void;
}
```

체크리스트:
- [ ] N x N 히트맵 그리드
- [ ] 색상 스케일: -1 (빨강) ~ 0 (흰색) ~ +1 (파랑)
- [ ] 셀 호버 시 값 표시
- [ ] 대각선 (자기 상관) 구분 표시
- [ ] 심볼 라벨 (축)
- [ ] 클러스터링 정렬 (선택적)
- [ ] 셀 클릭 시 상세 (상관관계 차트)

**VolumeProfile 컴포넌트**

**파일**: `frontend/src/components/charts/VolumeProfile.tsx`

```typescript
interface VolumeProfileProps {
  priceVolumes: { price: number; volume: number }[];
  currentPrice: number;
  chartHeight: number;
}
```

체크리스트:
- [ ] 가로 막대 차트 (가격대별 거래량)
- [ ] 캔들 차트 Y축과 동기화
- [ ] POC (Point of Control) 강조
- [ ] Value Area 표시 (70% 거래량)
- [ ] 현재가 위치 라인
- [ ] 매물대 밀집 구간 하이라이트

**OpportunityMap 컴포넌트**

**파일**: `frontend/src/components/charts/OpportunityMap.tsx`

```typescript
interface OpportunityMapProps {
  symbols: {
    symbol: string;
    totalScore: number;
    triggerScore: number;
    routeState: RouteState;
  }[];
  onSymbolClick?: (symbol: string) => void;
}
```

체크리스트:
- [ ] 산점도 (X: TOTAL, Y: TRIGGER)
- [ ] 점 색상: RouteState 기반
- [ ] 점 크기: 시가총액 또는 거래량 기반
- [ ] 4분면 라벨 표시
- [ ] 호버 시 종목 정보 툴팁
- [ ] 클릭 시 종목 상세
- [ ] 줌/팬 기능

---

##### 6.7.4 상태 관리 UI

**KanbanBoard 컴포넌트**

**파일**: `frontend/src/components/charts/KanbanBoard.tsx`

```typescript
interface KanbanBoardProps {
  symbols: {
    symbol: string;
    name: string;
    routeState: RouteState;
    score: number;
  }[];
  onStateChange?: (symbol: string, newState: RouteState) => void;
}
```

체크리스트:
- [ ] 3열 칸반 레이아웃 (ATTACK / ARMED / WATCH)
- [ ] 각 열 헤더 + 카운트 배지
- [ ] 종목 카드:
  - 종목명
  - 현재가 + 등락률
  - 점수
  - 미니 스파크라인
- [ ] 드래그 앤 드롭 (상태 변경)
- [ ] 정렬: 점수 순
- [ ] 빈 열 표시 처리

**SurvivalBadge 컴포넌트**

**파일**: `frontend/src/components/charts/SurvivalBadge.tsx`

```typescript
interface SurvivalBadgeProps {
  days: number;  // 연속 상위권 일수
  tier?: 'bronze' | 'silver' | 'gold' | 'platinum';
}
```

체크리스트:
- [ ] 뱃지 스타일 컴포넌트
- [ ] 티어별 색상:
  | 일수 | 티어 | 색상 |
  |------|------|------|
  | 1~6 | Bronze | #CD7F32 |
  | 7~13 | Silver | #C0C0C0 |
  | 14~29 | Gold | #FFD700 |
  | 30+ | Platinum | #E5E4E2 |
- [ ] 일수 표시 + 아이콘
- [ ] 툴팁: 연속 상위권 기록

**RegimeSummaryTable 컴포넌트**

**파일**: `frontend/src/components/charts/RegimeSummaryTable.tsx`

체크리스트:
- [ ] 테이블 레이아웃
- [ ] 컬럼:
  | 레짐 | 기간 | 평균 수익률 | 변동성 | 최대 DD |
  |------|------|------------|--------|---------|
  | Bull | 45일 | +2.3% | 15% | -8% |
  | Bear | 30일 | -1.5% | 22% | -15% |
  | Sideways | 25일 | +0.3% | 10% | -5% |
- [ ] 현재 레짐 하이라이트
- [ ] 레짐 전환 히스토리 차트

---

##### 6.7.5 섹터 시각화

**SectorTreemap 컴포넌트**

**파일**: `frontend/src/components/charts/SectorTreemap.tsx`

```typescript
interface SectorTreemapProps {
  sectors: {
    name: string;
    value: number;  // 거래대금
    changeRate: number;
    symbols?: { symbol: string; value: number }[];
  }[];
  onSectorClick?: (sector: string) => void;
}
```

체크리스트:
- [ ] 트리맵 레이아웃 (D3 또는 recharts)
- [ ] 크기: 거래대금 비례
- [ ] 색상: 등락률 기반 (초록/빨강 그라데이션)
- [ ] 섹터명 + 등락률 라벨
- [ ] 클릭 시 섹터 드릴다운 (개별 종목)
- [ ] 호버 시 상세 툴팁

**SectorMomentumBar 컴포넌트**

**파일**: `frontend/src/components/charts/SectorMomentumBar.tsx`

```typescript
interface SectorMomentumBarProps {
  sectors: {
    name: string;
    return5d: number;  // 5일 수익률
  }[];
  limit?: number;  // 표시 개수 (기본 10)
}
```

체크리스트:
- [ ] 수평 막대 차트
- [ ] TOP 10 / BOTTOM 10 탭
- [ ] 색상: 양수 초록, 음수 빨강
- [ ] 정렬: 수익률 순
- [ ] 클릭 시 섹터 상세

**예상 시간**: 1.5주 (60시간)
| 항목 | 시간 |
|------|-----:|
| FearGreedGauge | 4h |
| MarketBreadthWidget | 3h |
| ScoreWaterfall | 6h |
| KellyVisualization | 3h |
| CorrelationHeatmap | 8h |
| VolumeProfile | 6h |
| OpportunityMap | 6h |
| KanbanBoard | 8h |
| SurvivalBadge | 2h |
| RegimeSummaryTable | 4h |
| SectorTreemap | 6h |
| SectorMomentumBar | 4h |
| **총계** | **60h** |

---

#### 6.8 Multi Timeframe UI (Phase 5 연동)

> **[의존성: 멀티 타임프레임 백엔드 완료 후]**
> **참조**: `docs/todo.md` 멀티 타임프레임 섹션 (Phase 1~6)

---

##### 6.8.1 MultiTimeframeSelector 컴포넌트

**파일**: `frontend/src/components/strategy/MultiTimeframeSelector.tsx`

```typescript
interface MultiTimeframeSelectorProps {
  primaryTimeframe: Timeframe;
  secondaryTimeframes: Timeframe[];
  onPrimaryChange: (tf: Timeframe) => void;
  onSecondaryChange: (tfs: Timeframe[]) => void;
  maxSecondary?: number;  // 기본 3
}

type Timeframe = '1m' | '5m' | '15m' | '30m' | '1h' | '4h' | '1d' | '1w';
```

체크리스트:
- [ ] 컴포넌트 기본 구조
- [ ] Primary TF 선택 드롭다운
  | 타임프레임 | 라벨 |
  |-----------|------|
  | 1m | 1분 |
  | 5m | 5분 |
  | 15m | 15분 |
  | 30m | 30분 |
  | 1h | 1시간 |
  | 4h | 4시간 |
  | 1d | 일봉 |
  | 1w | 주봉 |
- [ ] Secondary TF 다중 선택 (체크박스 그룹)
- [ ] 제약 조건 검증:
  - Secondary는 Primary보다 큰 TF만 선택 가능
  - 최대 3개 Secondary 선택
- [ ] 선택 불가 TF 비활성화 + 툴팁 설명
- [ ] 선택된 TF 요약 표시

---

##### 6.8.2 멀티 TF 차트 동기화

**파일**: `frontend/src/components/charts/MultiTimeframeChart.tsx`

체크리스트:
- [ ] 메인 차트 (Primary TF)
- [ ] 서브 차트 패널 (Secondary TF별)
- [ ] 차트 간 시간축 동기화
- [ ] 크로스헤어 동기화 (한 차트에서 이동 시 다른 차트도 연동)
- [ ] 줌/팬 동기화
- [ ] 차트 패널 접힘/펼침
- [ ] 레이아웃 옵션 (세로/가로 분할)

---

##### 6.8.3 API 연동

**파일**: `frontend/src/api/klines.ts`

```typescript
export async function fetchMultiTimeframeKlines(
  symbol: string,
  timeframes: Timeframe[],
  limit?: number
): Promise<Record<Timeframe, Kline[]>> {
  // GET /api/v1/klines/multi?symbol=...&timeframes=1h,4h,1d
}
```

체크리스트:
- [ ] `fetchMultiTimeframeKlines()` API 함수
- [ ] 타임프레임별 캐싱
- [ ] 로딩 상태 관리
- [ ] 에러 처리 (부분 실패 시)

---

##### 6.8.4 전략 설정 통합

체크리스트:
- [ ] SDUIRenderer에 `MultiTimeframeSelector` 통합
  - `field_type: 'multi_timeframe'` 지원
- [ ] 전략 생성/수정 시 TF 설정 저장
- [ ] 백테스트 설정에서 TF 선택

**예상 시간**: 0.5주 (20시간)
| 항목 | 시간 |
|------|-----:|
| MultiTimeframeSelector | 6h |
| 멀티 TF 차트 동기화 | 8h |
| API 연동 | 3h |
| 전략 설정 통합 | 3h |
| **총계** | **20h** |

---

#### 6.9 상태 관리 및 아키텍처 개선

> **목적**: 프론트엔드 코드 품질 및 성능 개선

---

##### 6.9.1 상태 관리 리팩토링

**createSignal → createStore 통합**

체크리스트:
- [ ] 복잡한 상태 객체 → `createStore` 변환
  | 페이지 | 변환 대상 |
  |--------|----------|
  | Strategies.tsx | `strategies`, `filters`, `selectedId` |
  | TradingJournal.tsx | `positions`, `trades`, `statistics` |
  | Screening.tsx | `filters`, `results`, `presets` |
  | Dashboard.tsx | `metrics`, `positions`, `orders` |
  | Backtest.tsx | `config`, `results`, `charts` |
- [ ] `produce` 함수로 불변 업데이트 패턴 적용
- [ ] 중첩 상태 접근 최적화

**createMemo 파생 상태 최적화**

체크리스트:
- [ ] 필터링된 목록 → `createMemo`
  ```typescript
  const filteredStrategies = createMemo(() =>
    strategies().filter(s => s.category === selectedCategory())
  );
  ```
- [ ] 계산된 통계 → `createMemo`
- [ ] 정렬된 데이터 → `createMemo`
- [ ] 불필요한 재계산 제거 (deps 최적화)

---

##### 6.9.2 커스텀 훅 추출

**useStrategies 훅**

**파일**: `frontend/src/hooks/useStrategies.ts`

```typescript
export function useStrategies() {
  const [strategies, setStrategies] = createSignal<Strategy[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);

  const fetchAll = async () => { ... };
  const create = async (data: CreateStrategy) => { ... };
  const update = async (id: string, data: UpdateStrategy) => { ... };
  const remove = async (id: string) => { ... };
  const toggle = async (id: string, enabled: boolean) => { ... };

  return { strategies, loading, error, fetchAll, create, update, remove, toggle };
}
```

체크리스트:
- [ ] `fetchAll()` - 전략 목록 조회
- [ ] `create()` - 전략 생성
- [ ] `update()` - 전략 수정
- [ ] `remove()` - 전략 삭제
- [ ] `toggle()` - 활성화/비활성화
- [ ] 낙관적 업데이트 (UI 즉시 반영)
- [ ] 에러 롤백

**useStrategySchema 훅** → 6.2.3에서 상세화 완료

**useJournal 훅**

**파일**: `frontend/src/hooks/useJournal.ts`

체크리스트:
- [ ] `positions` - 보유 포지션 조회
- [ ] `trades` - 체결 내역 조회 (페이지네이션)
- [ ] `statistics` - 손익 통계 조회
- [ ] `refresh()` - 데이터 새로고침
- [ ] 자동 갱신 (WebSocket 또는 폴링)

**useScreening 훅**

**파일**: `frontend/src/hooks/useScreening.ts`

체크리스트:
- [ ] `filters` - 필터 상태 관리
- [ ] `results` - 스크리닝 결과
- [ ] `presets` - 프리셋 CRUD
- [ ] `search()` - 스크리닝 실행
- [ ] `savePreset()` / `loadPreset()` / `deletePreset()`

**useMarketSentiment 훅**

**파일**: `frontend/src/hooks/useMarketSentiment.ts`

체크리스트:
- [ ] `fearGreedIndex` - 공포탐욕 지수
- [ ] `marketBreadth` - 20일선 상회 비율
- [ ] `sectorMomentum` - 섹터별 모멘텀
- [ ] 자동 갱신 (5분 간격)

---

##### 6.9.3 성능 최적화

**Lazy Loading 적용**

체크리스트:
- [ ] 페이지 레벨 Lazy Loading
  ```typescript
  const Dashboard = lazy(() => import('./pages/Dashboard'));
  const Strategies = lazy(() => import('./pages/Strategies'));
  const Backtest = lazy(() => import('./pages/Backtest'));
  // ...
  ```
- [ ] `Suspense` fallback UI (Spinner)
- [ ] 라우트별 코드 스플리팅

**컴포넌트 코드 스플리팅**

체크리스트:
- [ ] 차트 컴포넌트 Lazy Loading (번들 크기 큼)
  ```typescript
  const CandlestickChart = lazy(() => import('./charts/CandlestickChart'));
  const RadarChart7Factor = lazy(() => import('./charts/RadarChart7Factor'));
  ```
- [ ] 모달 컴포넌트 Lazy Loading
- [ ] 조건부 렌더링 컴포넌트 Lazy Loading

**가상 스크롤 (대용량 테이블)**

체크리스트:
- [ ] `@tanstack/solid-virtual` 또는 유사 라이브러리 도입
- [ ] 대용량 테이블에 적용:
  | 페이지 | 테이블 | 예상 행 수 |
  |--------|--------|-----------|
  | Screening | 결과 테이블 | 1,000+ |
  | GlobalRanking | 랭킹 테이블 | 500+ |
  | TradingJournal | 체결 내역 | 10,000+ |
- [ ] 스크롤 성능 테스트 (60fps 유지)
- [ ] 행 높이 고정 vs 가변 처리

**기타 최적화**

체크리스트:
- [ ] 이미지 Lazy Loading (`loading="lazy"`)
- [ ] API 응답 캐싱 (stale-while-revalidate)
- [ ] 디바운스 적용 (검색, 필터 입력)
- [ ] 번들 분석 (`vite-bundle-visualizer`)
- [ ] 불필요한 리렌더링 제거 (React DevTools Profiler)

**예상 시간**: 1주 (40시간)
| 항목 | 시간 |
|------|-----:|
| createStore 리팩토링 | 8h |
| createMemo 최적화 | 4h |
| useStrategies 훅 | 4h |
| useJournal 훅 | 4h |
| useScreening 훅 | 4h |
| useMarketSentiment 훅 | 3h |
| Lazy Loading | 6h |
| 가상 스크롤 | 5h |
| 기타 최적화 | 2h |
| **총계** | **40h** |

---

#### 📊 프론트엔드 작업 요약

| 카테고리 | 체크리스트 | 예상 시간 | 상태 | 우선순위 |
|----------|----------:|---------:|:----:|:--------:|
| UI 리팩토링 (6.1) | 36개 | 16h (2일) | 🟡 진행중 | P0 |
| SDUIRenderer (6.2) | 52개 | 24h (3일) | 🔴 대기 | P0 |
| Journal UI (6.3) | 28개 | 40h (1주) | 🔴 대기 | P1 |
| Screening UI (6.4) | 38개 | 40h (1주) | 🔴 대기 | P1 |
| Ranking UI (6.5) | 24개 | 20h (2.5일) | 🔴 대기 | P1 |
| 신호 시각화 (6.6) | 32개 | 40h (1주) | 🔴 대기 | P2 |
| 고급 시각화 (6.7) | 48개 | 60h (1.5주) | 🔴 대기 | P2 |
| Multi TF (6.8) | 16개 | 20h (2.5일) | 🔴 대기 | P3 |
| 아키텍처 (6.9) | 40개 | 40h (1주) | 🔴 대기 | P3 |
| **총계** | **314개** | **300h (~7.5주)** | | |

**우선순위 설명**:
- **P0**: 즉시 시작 (의존성 없음)
- **P1**: Phase 1 백엔드 완료 후
- **P2**: Phase 1-C SignalMarker 완료 후
- **P3**: 선택적/나중에

**권장 진행 순서**:
1. ✅ UI 리팩토링 (6.1) - 남은 6개 페이지 완료
2. 🎯 SDUIRenderer (6.2) - 백엔드 스키마 연동
3. Journal UI (6.3) + Screening UI (6.4) + Ranking UI (6.5) - 병렬 진행 가능
4. 신호 시각화 (6.6) + 고급 시각화 (6.7) - 병렬 진행 가능
5. Multi TF (6.8) + 아키텍처 개선 (6.9) - 마무리

**예상 총 시간**: 7.5주 (프론트엔드 전체, 300시간)

---

## Phase 3 - 품질/성능 개선

> **병렬 실행**: 시스템 안정성 및 성능 개선 Phase 1/2와 병행 가능

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

## Phase 4 : 선택적/낮은 우선순위

### 외부 데이터 연동
- [ ] `NewsProvider` trait + Finnhub API
- [ ] `DisclosureProvider` trait + SEC EDGAR
- [ ] LLM 분석 (공시/뉴스 감성 분석)

### 텔레그램 봇 명령어 ✅ 완료
- [x] `/portfolio`, `/status`, `/stop`, `/report`, `/attack` → [bot_handler.rs](crates/trader-notification/src/bot_handler.rs), [telegram_bot.rs](crates/trader-api/src/services/telegram_bot.rs)

### 미구현 전략 (4개)
- [ ] SPAC No-Loss, All at Once ETF, Rotation Savings, Dual KrStock UsBond

### 추가 거래소
- [ ] Coinbase, Kraken, Interactive Brokers, 키움증권

### ML 예측 활용
- [ ] 전략에서 ML 예측 결과 사용
- [ ] 구조적 피처 기반 모델 재훈련

---

## ✅ 완료 현황

### v0.6.5 완료 (2026-02-03) - 프론트엔드 UI 개선

| 기능 | 상태 | 비고 |
|------|:----:|------|
| **Screening UI 개선** | ✅ | RouteState/Grade/Score 표시 |
| **Global Ranking RadarChart** | ✅ | 5축 레이더 차트 통합 |
| **RouteState 필터링** | ✅ | 백엔드+프론트엔드 구현 |

#### 상세 내역

**1. Screening 페이지**
- RouteState 뱃지 컬럼 추가 (ATTACK/ARMED/WATCH/REST)
- Grade 뱃지 컬럼 추가 (S/A/B/C/D/F)
- Overall Score 컬럼 추가 (색상 코딩)
- 파일: `frontend/src/pages/Screening.tsx`

**2. RadarChart 컴포넌트**
- 신규 생성: `frontend/src/components/ui/RadarChart.tsx`
- SVG 기반 5축 레이더 차트 (technical, momentum, trend, volume, volatility)
- 등급별 색상 (80+:초록, 60+:파랑, 40+:노랑, 그 외:빨강)
- TopRankCard에 통합

**3. RouteState 필터링**
- 백엔드: `crates/trader-api/src/repository/global_score.rs`
  - RankedSymbol에 route_state 필드 추가
  - 실시간 RouteState 계산 로직 (RouteStateCalculator 사용)
- 프론트엔드: `frontend/src/pages/GlobalRanking.tsx`
  - 필터 드롭다운 추가 (ATTACK/ARMED/WATCH/REST)
  - API 연동 완료

---

### v0.5.6 완료 (2026-02-02)

| 기능 | 상태 | 비고 |
|------|:----:|------|
| **종목 데이터 관리 시스템** | ✅ | CLI 도구 완성 |
| CSV 변환 스크립트 | ✅ | KRX 원본 → 표준 형식 (21,968개 종목) |
| sync-csv 명령 | ✅ | CSV → DB 자동 동기화 |
| list-symbols 명령 | ✅ | DB 종목 조회 (table/csv/json) |
| fetch-symbols 명령 | ✅ | 온라인 자동 크롤링 (KR/US/CRYPTO) |

#### 종목 데이터 관리 상세

**1. CSV 변환 (`scripts/convert_krx_new_to_csv.py`)**
- KRX 정보시스템 원본 CSV (상품 분류별) → 표준 형식 변환
- EUC-KR 인코딩 자동 처리
- 21,968개 종목 성공적으로 변환
- 상세 CSV (metadata 포함) 병행 생성

**2. sync-csv 명령 (`trader sync-csv`)**
- CSV 파일을 읽어 symbol_info 테이블에 동기화
- KOSPI/KOSDAQ 자동 판별
- Yahoo Finance 심볼 자동 생성
- Upsert로 안전한 업데이트
- 섹터 정보 선택적 업데이트 지원

**3. list-symbols 명령 (`trader list-symbols`)**
- DB에서 종목 정보 실시간 조회
- 시장별 필터링 (KR, US, CRYPTO, ALL)
- 검색 기능 (종목명/티커)
- 다중 출력 형식: table (사람), csv (데이터), json (API)
- 파일 저장 옵션

**4. fetch-symbols 명령 (`trader fetch-symbols`)**
- **자동 크롤링**: 온라인 소스에서 실시간 수집
- **데이터 소스**:
  - KR: KRX 공식 API (전체 종목)
  - US: Yahoo Finance (주요 500개)
  - CRYPTO: Binance API (USDT 페어 446개)
- **기능**:
  - 시장별 선택 수집 (KR/US/CRYPTO/ALL)
  - DB 자동 저장
  - CSV 백업 옵션
  - 드라이런 모드 (테스트용)
  - 진행 상황 실시간 표시

**사용 예시**:
```bash
# CSV 변환
python scripts/convert_krx_new_to_csv.py

# DB 동기화
trader sync-csv --codes data/krx_codes.csv

# 종목 조회
trader list-symbols --market KR --limit 10

# 자동 크롤링
trader fetch-symbols --market ALL
```

### v0.5.7 완료 (2026-02-02) - Phase 0 주요 완료 🎉

**Phase 0 진척도: 85% 완료**

| Phase 0 항목 | 상태 | 비고 |
|-------------|:----:|------|
| ✅ 전략 레지스트리 (SchemaRegistry) | 100% | Proc macro + 자동 등록 |
| ✅ 공통 로직 추출 (4개 모듈) | 100% | indicators, position_sizing, risk_checks, signal_filters |
| ✅ Journal-Backtest 공통 모듈 | 100% | calculations, statistics 통합 |
| ✅ TickSizeProvider | 100% | tick_size.rs 구현 완료 |
| 🟡 StrategyContext | 0% | 다음 버전에서 구현 예정 |

#### 🎯 전략 스키마 시스템

| 컴포넌트 | 파일 | 줄 수 | 설명 |
|---------|------|------:|------|
| Proc Macro | trader-strategy-macro/src/lib.rs | 266 | 컴파일 타임 메타데이터 추출 |
| SchemaRegistry | schema_registry.rs | 694 | 전략 스키마 중앙 관리 |
| SchemaComposer | schema_composer.rs | 279 | 스키마 조합 시스템 |
| API 라우트 | routes/schema.rs | 189 | REST API 엔드포인트 |
| **총계** | | **1,428줄** | **26개 전략 모두 적용** |

**효과**:
- 전략 추가 시간: 2시간 → 30분 (75% 감소)
- 프론트엔드 SDUI 자동 생성
- 타입 안전성 확보 (컴파일 타임 체크)

#### 🧩 공통 전략 컴포넌트

| 모듈 | 줄 수 | 주요 기능 | 제거된 중복 코드 |
|------|------:|-----------|-----------------|
| indicators.rs | 349 | SMA, EMA, RSI, MACD, Bollinger, ATR, Stochastic | ~800줄 |
| position_sizing.rs | 286 | FixedAmount, RiskBased, VolatilityAdjusted, Kelly | ~400줄 |
| risk_checks.rs | 291 | 포지션/집중도/손실/변동성 한도 | ~350줄 |
| signal_filters.rs | 372 | 거래량/변동성/시간/추세 필터 | ~450줄 |
| **총계** | **1,298줄** | | **~2,000줄 중복 제거** |

**효과**:
- 보일러플레이트 80% 감소
- 전략 간 일관성 확보
- 유지보수 비용 대폭 절감

#### 📐 도메인 레이어

| 모듈 | 줄 수 | 주요 기능 |
|------|------:|-----------|
| calculations.rs | 374 | 손익/수익률/포지션 가치 계산 (Decimal) |
| statistics.rs | 514 | 샤프/소르티노/MDD/승률/PF |
| tick_size.rs | 335 | 시장별 최소 호가 단위 |
| schema.rs | 343 | 공통 도메인 스키마 |
| **총계** | **1,566줄** | |

**효과**:
- 백테스트-실거래 로직 통합
- 금융 계산 정밀도 향상
- 시장별 주문 정확도 향상

#### 🛠️ CLI 도구

| 명령어 | 줄 수 | 기능 |
|--------|------:|------|
| fetch_symbols | 365 | KRX/Yahoo/Binance 심볼 크롤링 |
| list_symbols | 244 | 심볼 조회/필터링 (CSV/JSON) |
| sync_csv | 120 | KRX CSV 동기화 |
| **총계** | **729줄** | |

#### 📊 기타 개선

- **journal_integration.rs** (280줄): 매매 일지 백테스트 통합
- **26개 전략 리팩토링**: 평균 ~50줄씩 감소
- **API 라우트 정리**: strategies.rs 163줄 감소
- **Symbol 타입 확장**: Yahoo 변환 로직 추가

#### 📚 문서

| 문서 | 줄 수 | 내용 |
|------|------:|------|
| tick_size_guide.md | 245 | 시장별 틱 사이즈 가이드 |
| development_rules.md | +299 | v1.1: 180+ 규칙 체계화 |
| prd.md | +67 | 전략 스키마 시스템 명세 |

#### 🎨 프론트엔드 UI 리팩토링 (진행 중)

| 페이지 | 상태 | 적용 컴포넌트 | 비고 |
|--------|:----:|--------------|------|
| GlobalRanking.tsx | ✅ | Card, StatCard, EmptyState, ErrorState | 참조 구현 |
| Simulation.tsx | ✅ | Card, StatCardGrid(6), EmptyState, ErrorState | 6개 섹션 카드화 |
| Backtest.tsx | ✅ | Card, EmptyState, Button | 설정 섹션 카드화 |
| Settings.tsx | ✅ | Card, Button (5개 섹션) | API, 리스크, 알림, 외관, DB |
| TradingJournal.tsx | 🟡 | - | 대기 |
| Dashboard.tsx | 🟡 | - | 대기 |
| Strategies.tsx | 🟡 | - | 대기 |
| Dataset.tsx | 🟡 | - | 대기 |
| Screening.tsx | 🟡 | - | 대기 |
| MLTraining.tsx | 🟡 | - | 대기 |

**공통 컴포넌트** (`components/ui/`):
- `Card`, `CardHeader`, `CardContent` - 섹션 컨테이너
- `StatCard`, `StatCardGrid` - 통계 표시
- `EmptyState`, `ErrorState`, `PageLoader` - 상태 표시
- `Button` - 버튼 (primary, secondary, danger)
- `PageHeader` - 페이지 헤더
- `FilterPanel`, `Select`, `Input` - 폼 요소

---

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
