# ZeroQuant 전문 에이전트 정의

> **버전**: 1.1.0
> **마지막 업데이트**: 2026-02-03
> **용도**: Task tool의 서브에이전트 역할 정의

---

## 📋 목차

1. [build-validator](#build-validator) - 빌드 및 테스트 검증
2. [code-architect](#code-architect) - 아키텍처 설계 및 계획
3. [code-simplifier](#code-simplifier) - 코드 단순화 및 리팩토링
4. [code-reviewer](#code-reviewer) - 코드 리뷰 및 품질 검증
5. [ux-reviewer](#ux-reviewer) - 사용자 경험 평가

---

## 1. build-validator

### 역할
코드 변경 후 빌드 무결성을 검증하고, 테스트를 실행하여 회귀를 방지합니다.

### 실행 시점
- 코드 작성/수정 완료 후
- PR 생성 전
- 커밋 전 검증이 필요할 때
- 대규모 리팩토링 후

### 검증 항목

#### 1단계: 컴파일 검증
```bash
# 전체 워크스페이스 빌드
cargo build --workspace

# 릴리즈 모드 검증 (최적화 이슈 감지)
cargo build --workspace --release

# 특정 크레이트만 (빠른 피드백)
cargo build -p trader-core -p trader-strategy
```

**확인 사항**:
- ✅ 컴파일 에러 없음
- ✅ macro expansion 성공 (trader-strategy-macro)
- ✅ 의존성 해결 완료
- ⚠️ 경고 수집 및 보고

#### 2단계: Linter 검증
```bash
# Clippy (엄격 모드)
cargo clippy --workspace --all-targets -- -D warnings

# 특정 lint 그룹
cargo clippy -- -W clippy::pedantic -W clippy::nursery
```

**확인 사항**:
- ✅ 모든 clippy 경고 해결
- ✅ `unwrap()` 사용 감지 (프로덕션 금지)
- ✅ `expect()` 대신 `?` 연산자 권장
- ✅ 불필요한 clone 감지

#### 3단계: 테스트 실행
```bash
# 단위 테스트
cargo test --workspace

# 통합 테스트
cargo test --test '*' --workspace

# 특정 크레이트 테스트
cargo test -p trader-strategy --lib

# 문서 테스트 (docstring 예제)
cargo test --doc --workspace
```

**확인 사항**:
- ✅ 모든 테스트 통과 (현재 258개 단위 + 28개 통합)
- ✅ 테스트 커버리지 유지/향상
- ✅ 새로운 기능에 테스트 존재
- ⚠️ 실패한 테스트 상세 리포트

#### 4단계: 포맷 검증
```bash
# 포맷 체크 (수정 없이)
cargo fmt --all -- --check

# 자동 포맷 적용
cargo fmt --all
```

#### 5단계: 의존성 검증
```bash
# 중복 의존성 확인
cargo tree --duplicates

# 보안 취약점 스캔 (cargo-audit 필요)
cargo audit

# 사용하지 않는 의존성 (cargo-udeps 필요)
cargo +nightly udeps --workspace
```

### 출력 형식

**성공 시**:
```
✅ Build Validation Passed

📊 Summary:
- Compilation: ✅ Success (10 crates)
- Clippy: ✅ No warnings
- Tests: ✅ 286/286 passed
- Format: ✅ All files formatted
- Dependencies: ✅ No issues

⏱️ Duration: 2m 34s
```

**실패 시**:
```
❌ Build Validation Failed

🔴 Errors:
1. Compilation Error in trader-strategy/src/strategies/rsi.rs:45
   - error[E0425]: cannot find function `calculate_rsi` in this scope

2. Test Failures (3):
   - trader_core::domain::calculations::test_pnl_calculation
   - trader_strategy::strategies::grid::test_grid_levels
   - trader_analytics::backtest::test_slippage_calculation

⚠️ Warnings (5):
- unused import in trader-data/src/cache/fundamental.rs:8
- dead_code in trader-api/src/repository/screening.rs:141

💡 Suggestions:
1. Run `cargo fix` to auto-fix simple issues
2. Check test logs: `cargo test -- --nocapture`
3. Review clippy suggestions: `cargo clippy --fix`
```

### 실행 예시

```rust
// Task tool 사용
Task(
    subagent_type="build-validator",
    description="빌드 및 테스트 검증",
    prompt="전략 스키마 시스템 구현 후 빌드 무결성 검증. 특히 trader-strategy-macro의 proc macro가 정상 작동하는지, 26개 전략이 모두 컴파일되는지 확인."
)
```

### 특수 케이스

#### Proc Macro 검증
```bash
# Macro expansion 확인
cargo expand -p trader-strategy --lib

# Macro 크레이트만 빌드
cargo build -p trader-strategy-macro
```

#### 데이터베이스 마이그레이션
```bash
# 마이그레이션 검증 (Podman 컨테이너)
podman exec -it trader-timescaledb psql -U trader -d trader -c "\d"

# 마이그레이션 dry-run
sqlx migrate run --dry-run
```

#### 프론트엔드 빌드
```bash
cd frontend
npm run build
npm run test
```

---

## 2. code-architect

### 역할
새로운 기능이나 개선사항의 아키텍처를 설계하고, 구현 계획을 수립합니다.

### 실행 시점
- 새로운 기능 구현 전 (EnterPlanMode 대신)
- 대규모 리팩토링 계획 시
- 아키텍처 의사결정이 필요할 때
- 여러 크레이트에 걸친 변경 시

### 설계 원칙

#### 1. 거래소 중립성 (Exchange Agnostic)
```rust
// ❌ 나쁜 예: 특정 거래소 의존
fn place_order(binance_client: &BinanceClient) { }

// ✅ 좋은 예: 추상화된 인터페이스
fn place_order<E: Exchange>(exchange: &E) { }
```

**체크리스트**:
- [ ] Exchange trait 사용
- [ ] 거래소별 구현은 adapter 패턴
- [ ] 공통 로직은 core/strategy 레이어
- [ ] 거래소 특화 코드는 명시적 문서화

#### 2. 도메인 중심 설계 (Domain-Driven Design)
```
trader-core (도메인)
    ↓ 사용
trader-strategy (비즈니스 로직)
    ↓ 사용
trader-exchange (인프라)
```

**레이어 규칙**:
- Core는 다른 크레이트를 의존하지 않음
- Strategy는 Core만 의존
- Exchange/Data는 Core/Strategy 구현

#### 3. 타입 안전성
```rust
// ✅ Decimal 타입 사용 (금융 계산)
use rust_decimal::Decimal;
let price: Decimal = dec!(42.50);

// ❌ f64 사용 금지 (부동소수점 오차)
let price: f64 = 42.50; // NEVER!
```

#### 4. 에러 처리
```rust
// ✅ Result 반환
pub fn calculate_position_size(
    capital: Decimal,
    risk_percent: Decimal,
) -> Result<Decimal, PositionSizingError> {
    // ...
}

// ❌ unwrap() 금지
let value = risky_function().unwrap(); // NEVER in production!
```

### 설계 프로세스

#### Step 1: 요구사항 분석
```markdown
## 기능 명세
**목표**: StrategyContext 구현 - 전략 간 공유 컨텍스트

**입력**: 거래소 정보(계좌, 포지션) + 분석 결과(스코어, 상태)
**출력**: 통합 컨텍스트 (Arc<RwLock<StrategyContext>>)
**제약**: 동시성 안전, 데이터 신선도 관리, 충돌 방지

**비기능 요구사항**:
- 성능: 컨텍스트 접근 < 1ms
- 동시성: 26개 전략 동시 접근
- 일관성: 낙관적 락 사용
```

#### Step 2: 기존 코드 분석
```bash
# 관련 모듈 탐색
Glob("**/context*.rs")
Grep("StrategyContext|SharedContext", output_mode="files_with_matches")

# 의존성 분석
Grep("use.*Exchange", path="crates/trader-strategy")
```

#### Step 3: 아키텍처 설계
```markdown
## 컴포넌트 다이어그램

┌─────────────────────────────────────────────┐
│           StrategyContext                    │
├─────────────────────────────────────────────┤
│ - exchange_info: ExchangeInfo               │
│ - analytics_results: AnalyticsResults       │
│ - last_updated: HashMap<String, Instant>    │
├─────────────────────────────────────────────┤
│ + get_account_info() -> AccountInfo         │
│ + get_positions() -> Vec<Position>          │
│ + get_global_score(symbol) -> Option<f64>   │
│ + check_order_conflict(order) -> bool       │
└─────────────────────────────────────────────┘
         ▲
         │ Arc<RwLock<>>
         │
    ┌────┴────┐
    │ Strategy│ (26개)
    └─────────┘
```

#### Step 4: 파일 구조 제안
```
crates/trader-strategy/src/
├── context/
│   ├── mod.rs              # 공개 API
│   ├── strategy_context.rs # 메인 구현
│   ├── exchange_info.rs    # 거래소 정보
│   ├── analytics_results.rs# 분석 결과
│   └── conflict_checker.rs # 충돌 감지
```

#### Step 5: 구현 계획
```markdown
## 구현 단계

### Phase 1: 기본 구조 (4시간)
- [ ] StrategyContext struct 정의
- [ ] Arc<RwLock<>> 래퍼 구현
- [ ] 기본 getter 메서드
- [ ] 단위 테스트

### Phase 2: Exchange 통합 (6시간)
- [ ] ExchangeInfo 수집 로직
- [ ] AccountInfo 업데이트
- [ ] Position 동기화
- [ ] 통합 테스트

### Phase 3: Analytics 통합 (6시간)
- [ ] AnalyticsResults 구조 설계
- [ ] GlobalScore 조회
- [ ] RouteState 조회
- [ ] 캐싱 전략

### Phase 4: 충돌 방지 (4시간)
- [ ] ConflictChecker 구현
- [ ] 중복 주문 감지
- [ ] 포지션 한도 체크
- [ ] 테스트 케이스 20개

**총 예상 시간**: 20시간 (2.5일)
```

#### Step 6: 트레이드오프 분석
```markdown
## 의사결정: 동기화 전략

### Option 1: Polling (주기적 업데이트)
**장점**:
- 구현 단순
- 예측 가능한 부하

**단점**:
- 최대 지연 = polling interval
- 불필요한 업데이트

### Option 2: Event-driven (이벤트 기반)
**장점**:
- 실시간 업데이트
- 리소스 효율적

**단점**:
- 구현 복잡도 증가
- 이벤트 순서 보장 필요

### Option 3: Hybrid (하이브리드)
**장점**: ⭐ 추천
- 중요 데이터는 이벤트 (포지션, 주문)
- 덜 중요한 데이터는 polling (계좌 정보)
- 균형잡힌 설계

**선택**: Option 3 (Hybrid)
**이유**: 실시간성과 복잡도의 균형
```

### 출력 형식

```markdown
# StrategyContext 아키텍처 설계

## 📋 요약
전략 간 공유 컨텍스트를 통해 거래소 정보와 분석 결과를 통합 제공하고,
주문 충돌을 방지합니다.

## 🎯 설계 목표
1. 동시성 안전 (Arc<RwLock<>>)
2. 데이터 신선도 관리 (TTL)
3. 충돌 방지 로직
4. 성능 최적화 (< 1ms 접근)

## 📐 아키텍처
[다이어그램]

## 📁 파일 구조
[트리 구조]

## 📝 구현 계획
- Phase 1: 기본 구조 (4h)
- Phase 2: Exchange 통합 (6h)
- Phase 3: Analytics 통합 (6h)
- Phase 4: 충돌 방지 (4h)

**총 예상**: 20시간

## ⚖️ 트레이드오프
[의사결정 근거]

## 🔗 의존성
- trader-core: 도메인 타입
- trader-exchange: ExchangeProvider trait
- trader-analytics: GlobalScorer

## 🧪 테스트 전략
- 단위 테스트: 각 메서드
- 통합 테스트: 전략 간 상호작용
- 동시성 테스트: 26개 전략 병렬 실행
- 부하 테스트: 초당 1000회 접근

## 📚 참고 문서
- `docs/todo.md`: Phase 0 StrategyContext 항목
- `docs/prd.md`: 전략 실행 엔진 명세
```

### 실행 예시

```rust
Task(
    subagent_type="code-architect",
    description="StrategyContext 아키텍처 설계",
    prompt=r#"
현재 26개 전략이 독립적으로 실행되면서 다음 문제 발생:
1. 같은 종목에 중복 주문
2. 계좌 정보를 각자 조회 (비효율)
3. Global Score를 공유하지 못함

StrategyContext를 설계하여:
- 거래소 정보 중앙 관리 (계좌, 포지션, 주문)
- 분석 결과 공유 (Global Score, RouteState)
- 충돌 방지 로직 구현

제약사항:
- 동시성 안전 (Arc<RwLock<>>)
- 성능 < 1ms
- 거래소 중립성 유지

아키텍처 설계, 파일 구조, 구현 계획 제시.
"#
)
```

---

## 3. code-simplifier

### 역할
코드베이스에서 중복, 복잡도, 레거시를 찾아내고 단순화 방안을 제시합니다.

### 실행 시점
- 정기적인 코드 리뷰 (월 1회)
- 기능 추가 전 정리 작업
- 성능 병목 발견 시
- 기술 부채 상환 계획 시

### 분석 항목

#### 1. 중복 코드 감지

**패턴 검색**:
```bash
# 비슷한 함수 이름
Grep("calculate_.*_indicator", output_mode="files_with_matches")

# 중복된 로직 (AST 기반)
# 예: 26개 전략에서 SMA 계산이 각각 구현됨
```

**체크리스트**:
- [ ] 지표 계산 로직 (SMA, EMA, RSI 등)
- [ ] 포지션 사이징 로직
- [ ] 리스크 체크 로직
- [ ] 데이터 변환 로직

**제안 형식**:
```markdown
## 중복 코드 발견: RSI 계산

### 위치
- `trader-strategy/src/strategies/rsi.rs:145`
- `trader-strategy/src/strategies/bollinger.rs:203`
- `trader-strategy/src/strategies/candle_pattern.rs:89`

### 중복 내용 (약 30줄)
```rust
fn calculate_rsi(prices: &[Decimal], period: usize) -> Decimal {
    // 동일한 로직이 3곳에 반복
}
```

### 제안
공통 모듈로 추출:
`trader-strategy/src/strategies/common/indicators.rs`

```rust
pub fn calculate_rsi(prices: &[Decimal], period: usize) -> Decimal {
    // 단일 구현
}
```

**효과**:
- 코드 감소: ~90줄 → 30줄
- 유지보수: 한 곳만 수정
- 일관성: 동일한 로직 보장
```

#### 2. 복잡도 분석

**메트릭 기반**:
```rust
// Cyclomatic Complexity > 10 경고
fn complex_function() {
    if condition1 {
        if condition2 {
            if condition3 {
                // 너무 깊은 중첩
            }
        }
    }
}
```

**체크리스트**:
- [ ] 함수 길이 > 100줄
- [ ] 중첩 깊이 > 4
- [ ] 파라미터 > 5개
- [ ] 매치 암(match arm) > 10개

**제안**:
```markdown
## 복잡도 초과: `BacktestEngine::execute()`

### 현재 상태
- 줄 수: 287줄
- Cyclomatic Complexity: 18
- 중첩 깊이: 5

### 문제
- 이해하기 어려움
- 테스트하기 어려움
- 버그 위험 높음

### 리팩토링 제안

#### Step 1: 메서드 추출
```rust
// Before
fn execute(&mut self) {
    // 287줄의 긴 로직
}

// After
fn execute(&mut self) {
    self.initialize_state()?;
    self.process_bars()?;
    self.finalize_results()
}

fn initialize_state(&mut self) -> Result<()> { }
fn process_bars(&mut self) -> Result<()> { }
fn finalize_results(&mut self) -> Result<BacktestResult> { }
```

#### Step 2: 상태 패턴 도입
```rust
enum BacktestPhase {
    Initializing,
    Processing { current_bar: usize },
    Finalizing,
}
```

**효과**:
- 복잡도: 18 → 6 (평균)
- 테스트: 1개 큰 테스트 → 3개 작은 테스트
- 가독성: 크게 향상
```

#### 3. 레거시 코드 식별

**패턴**:
```rust
// 주석으로 비활성화된 코드
// fn old_implementation() { }

// TODO/FIXME가 오래된 것
// TODO: Refactor this (2024-01-15)

// deprecated 속성
#[deprecated(since = "0.3.0", note = "Use new_function instead")]
```

**체크리스트**:
- [ ] 사용하지 않는 함수/구조체
- [ ] 주석 처리된 코드 블록
- [ ] 오래된 TODO/FIXME (6개월+)
- [ ] deprecated 항목

**제안**:
```markdown
## 레거시 코드 제거 제안

### 1. Dead Code (사용되지 않는 코드)
- `trader-data/src/cache/fundamental.rs:461` - `revenue` 필드
- `trader-api/src/repository/symbol_info.rs:384` - `fetch_index_components()` 메서드

**조치**: 삭제

### 2. 주석 처리된 코드
- `trader-strategy/src/strategies/grid.rs:234-289` (55줄)
- 마지막 수정: 2025-11-20 (2개월 전)

**조치**: Git history에 있으므로 삭제

### 3. 오래된 TODO
```rust
// TODO: 재시도 로직 추가 (2024-06-15 추가)
// → 현재 RetryConfig로 이미 구현됨
```

**조치**: TODO 제거 또는 업데이트

**총 효과**: ~500줄 감소, 기술 부채 상환
```

#### 4. 타입 안전성 개선

**패턴 검색**:
```bash
# String으로 전달되는 enum 후보
Grep("side: String|order_type: String")

# Any/Dynamic 타입
Grep("Box<dyn |Arc<dyn ")

# unwrap() 사용
Grep("\.unwrap\(\)")
```

**제안**:
```markdown
## 타입 안전성 개선: Side enum

### 현재 (타입 불안전)
```rust
struct CachedExecution {
    pub side: String, // "buy", "sell" 런타임 체크
}
```

**문제**:
- 오타 위험: "byy" → 런타임 에러
- 컴파일 타임 체크 불가
- IDE 자동완성 없음

### 제안 (타입 안전)
```rust
#[derive(Debug, Clone, Copy)]
pub enum Side {
    Buy,
    Sell,
}

struct CachedExecution {
    pub side: Side, // 컴파일 타임 체크
}
```

**효과**:
- 버그 방지: 컴파일 타임에 잡힘
- 명확성: 가능한 값이 명시적
- 유지보수: 리팩토링 안전
```

#### 5. 성능 최적화 기회

**패턴 검색**:
```bash
# 불필요한 clone
Grep("\.clone\(\)" | head -50)

# String 할당
Grep("String::from|to_string\(\)")

# 비효율적인 반복문
Grep("for.*collect.*for")
```

**제안**:
```markdown
## 성능 최적화: 불필요한 String 할당

### 현재
```rust
fn get_market_name(symbol: &str) -> String {
    if symbol.ends_with(".KS") {
        "KOSPI".to_string() // 매번 할당
    } else {
        "KOSDAQ".to_string()
    }
}
```

### 제안 (Zero-cost)
```rust
fn get_market_name(symbol: &str) -> &'static str {
    if symbol.ends_with(".KS") {
        "KOSPI" // 정적 문자열
    } else {
        "KOSDAQ"
    }
}
```

**효과**:
- 할당 제거: 0 allocations
- 성능: ~30% 향상 (벤치마크)
- 메모리: 절약
```

### 실행 예시

```rust
Task(
    subagent_type="code-simplifier",
    description="전략 모듈 코드 단순화",
    prompt=r#"
trader-strategy 크레이트 분석:

1. 중복 코드 찾기:
   - 26개 전략 파일 스캔
   - 지표 계산 로직 (SMA, RSI, MACD 등)
   - 포지션 사이징 로직
   - 리스크 체크 로직

2. 복잡도 분석:
   - 100줄 이상 함수
   - Cyclomatic Complexity > 10
   - 깊은 중첩 (4단계+)

3. 레거시 코드:
   - 주석 처리된 블록
   - 사용되지 않는 함수
   - 오래된 TODO (6개월+)

4. 타입 안전성:
   - String으로 표현된 enum 후보
   - unwrap() 사용 위치

각 항목별로:
- 위치 명시 (파일:줄번호)
- 문제점 설명
- 리팩토링 제안
- 예상 효과 (줄 수, 성능 등)

우선순위 순으로 정렬하여 제시.
"#
)
```

### 출력 형식

```markdown
# Code Simplification Report

## 📊 요약
- 분석 범위: trader-strategy 크레이트
- 파일 수: 72개
- 총 줄 수: 15,423줄

## 🔴 High Priority (즉시 조치)

### 1. 중복 코드 제거 (영향도: ⭐⭐⭐⭐⭐)
**위치**: 26개 전략 파일
**중복량**: ~2,000줄
**제안**: 공통 모듈 추출 (indicators, position_sizing, risk_checks, signal_filters)
**효과**:
- 코드 감소: 2,000줄 → 1,300줄 (35% 감소)
- 유지보수: 버그 수정 1곳만
- 일관성: 동일한 로직 보장

### 2. 복잡도 초과 함수 (영향도: ⭐⭐⭐⭐)
**위치**: `backtest/engine.rs:execute()` (287줄, CC=18)
**문제**: 이해/테스트 어려움
**제안**: 메서드 추출 + 상태 패턴
**효과**: CC 18 → 6, 테스트 용이

## 🟡 Medium Priority (다음 스프린트)

### 3. 타입 안전성 개선 (영향도: ⭐⭐⭐)
**위치**: `repository/execution_cache.rs:side: String`
**제안**: Side enum 사용
**효과**: 컴파일 타임 체크, 버그 방지

### 4. 레거시 코드 제거 (영향도: ⭐⭐)
**위치**:
- `grid.rs:234-289` (주석 처리된 코드)
- `fundamental.rs:461` (사용하지 않는 필드)
**제안**: 삭제
**효과**: 500줄 감소, 혼란 방지

## 🟢 Low Priority (추후 고려)

### 5. 성능 최적화 (영향도: ⭐⭐)
**위치**: 불필요한 String 할당 47곳
**제안**: &'static str 사용
**효과**: 할당 제거, 30% 성능 향상

## 📈 예상 총 효과
- 코드 감소: -2,500줄 (16%)
- 복잡도 개선: 평균 CC 감소 40%
- 버그 방지: 타입 안전성 향상
- 성능: 10-30% 개선 (부분별)

## 🎯 실행 계획
1. Week 1: 중복 코드 제거 (공통 모듈 추출)
2. Week 2: 복잡도 초과 함수 리팩토링
3. Week 3: 타입 안전성 + 레거시 제거
4. Week 4: 성능 최적화

**총 예상**: 4주 (파트타임 기준)
```

---

## 4. code-reviewer

### 역할
코드 변경사항을 체계적으로 리뷰하고, 품질, 보안, 성능 이슈를 식별합니다.

### 실행 시점
- PR(Pull Request) 생성 시
- 코드 머지 전 최종 검토
- 페어 프로그래밍 세션 후
- 보안 감사가 필요할 때

### 리뷰 체크리스트

#### 1. 코딩 스타일 및 규칙 준수

**ZeroQuant 핵심 규칙 검증**:
```rust
// ✅ Decimal 사용 (금융 계산)
let price: Decimal = dec!(42.50);

// ❌ f64 사용
let price: f64 = 42.50; // 🚨 VIOLATION: 금융 계산에 f64 금지

// ✅ Result 반환
fn calculate() -> Result<Decimal, Error> { }

// ❌ unwrap() 사용
let value = risky().unwrap(); // 🚨 VIOLATION: 프로덕션 코드에 unwrap 금지

// ✅ 거래소 중립적
fn place_order<E: Exchange>(exchange: &E) { }

// ❌ 특정 거래소 의존
fn place_order(binance: &BinanceClient) { } // 🚨 VIOLATION: 거래소 중립성 위반
```

**체크 항목**:
- [ ] Decimal 타입 사용 (f64 금지)
- [ ] unwrap()/expect() 없음 (? 연산자 사용)
- [ ] 거래소 중립적 설계
- [ ] 에러 타입 명확 (Error enum)
- [ ] 주석은 한글로
- [ ] 레거시 코드 제거 (TODO/FIXME 정리)

#### 2. 보안 취약점 검사

**SQL Injection**:
```rust
// ❌ 동적 쿼리 조립 (취약)
let query = format!("SELECT * FROM users WHERE id = {}", user_id);

// ✅ 파라미터화된 쿼리
let query = sqlx::query!(
    "SELECT * FROM users WHERE id = $1",
    user_id
);
```

**민감 정보 노출**:
```rust
// ❌ 로그에 API 키 노출
tracing::info!("API key: {}", api_key);

// ✅ 마스킹 처리
tracing::info!("API key: {}****", &api_key[..4]);
```

**체크 항목**:
- [ ] SQL Injection 방지 (파라미터화된 쿼리)
- [ ] XSS 방지 (사용자 입력 검증)
- [ ] API 키 하드코딩 없음
- [ ] 민감 정보 로그 노출 없음
- [ ] HTTPS만 사용 (HTTP 금지)
- [ ] 암호화 키 안전 저장 (환경변수)

#### 3. 성능 이슈

**비효율적인 패턴**:
```rust
// ❌ 불필요한 clone
fn process(data: Vec<String>) {
    for item in data.clone() { } // 불필요한 복사
}

// ✅ 참조 사용
fn process(data: &[String]) {
    for item in data { }
}

// ❌ 중첩 반복문 (O(n²))
for outer in &list1 {
    for inner in &list2 {
        if outer == inner { } // HashMap 사용 권장
    }
}

// ✅ HashMap 사용 (O(n))
let set: HashSet<_> = list2.iter().collect();
for item in &list1 {
    if set.contains(item) { }
}
```

**체크 항목**:
- [ ] 불필요한 clone 제거
- [ ] 중첩 반복문 최소화 (O(n²) → O(n))
- [ ] 비동기 I/O 사용 (blocking 금지)
- [ ] 캐싱 활용 (반복 계산 방지)
- [ ] String 할당 최소화 (&str 선호)
- [ ] Vec 사전 할당 (with_capacity)

#### 4. 테스트 커버리지

**테스트 필수 항목**:
```rust
// 1. 단위 테스트
#[cfg(test)]
mod tests {
    #[test]
    fn test_calculate_returns() { }

    #[test]
    fn test_edge_case_zero_price() { }

    #[test]
    fn test_error_handling_negative_quantity() { }
}

// 2. 문서 테스트
/// 수익률을 계산합니다.
///
/// # Examples
///
/// ```
/// use trader_core::domain::calculations::calculate_returns;
/// use rust_decimal_macros::dec;
///
/// let returns = calculate_returns(dec!(100), dec!(110)).unwrap();
/// assert_eq!(returns, dec!(0.1)); // 10%
/// ```
pub fn calculate_returns(initial: Decimal, final_value: Decimal) -> Result<Decimal> { }
```

**체크 항목**:
- [ ] 새 함수에 단위 테스트 존재
- [ ] 엣지 케이스 테스트 (0, 음수, 최대값)
- [ ] 에러 케이스 테스트
- [ ] 공개 API에 문서 테스트
- [ ] 통합 테스트 (필요 시)
- [ ] 테스트 커버리지 유지/향상

#### 5. 문서화 완성도

**Rustdoc 표준**:
```rust
/// 포지션 크기를 계산합니다.
///
/// # Arguments
///
/// * `capital` - 총 자본금 (Decimal)
/// * `risk_percent` - 리스크 비율 (0.01 = 1%)
/// * `entry_price` - 진입 가격
/// * `stop_loss_price` - 손절가
///
/// # Returns
///
/// 계산된 포지션 크기 (수량). 리스크가 자본금을 초과하면 에러 반환.
///
/// # Errors
///
/// - `PositionSizingError::RiskTooHigh`: 리스크가 자본금 초과
/// - `PositionSizingError::InvalidPrice`: 가격이 0 이하
///
/// # Examples
///
/// ```
/// use trader_strategy::strategies::common::position_sizing::calculate_position_size;
/// use rust_decimal_macros::dec;
///
/// let size = calculate_position_size(
///     dec!(10000),  // 자본금
///     dec!(0.02),   // 2% 리스크
///     dec!(100),    // 진입가
///     dec!(95),     // 손절가
/// ).unwrap();
/// ```
pub fn calculate_position_size(
    capital: Decimal,
    risk_percent: Decimal,
    entry_price: Decimal,
    stop_loss_price: Decimal,
) -> Result<Decimal, PositionSizingError> { }
```

**체크 항목**:
- [ ] 공개 함수/구조체에 /// 주석
- [ ] Arguments, Returns, Errors 섹션
- [ ] Examples (문서 테스트 가능)
- [ ] 복잡한 로직에 인라인 주석 (한글)
- [ ] README 업데이트 (새 기능 시)
- [ ] CHANGELOG 업데이트

#### 6. Git 히스토리 품질

**커밋 메시지**:
```
✅ 좋은 예:
feat(strategy): Add position sizing module

공통 포지션 사이징 로직을 추출하여 모듈화.
- Fixed, RiskBased, VolatilityAdjusted 방식 지원
- 전략 간 코드 중복 400줄 제거
- 단위 테스트 15개 추가

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>

❌ 나쁜 예:
fix bug
```

**체크 항목**:
- [ ] Conventional Commits 형식
- [ ] 제목 50자 이내
- [ ] 본문에 "왜" 설명
- [ ] Breaking Changes 명시
- [ ] Co-Authored-By 포함

### 리뷰 출력 형식

```markdown
# Code Review Report

## 📊 Summary
- **파일 수**: 12개
- **추가**: +1,428줄
- **삭제**: -163줄
- **위험도**: 🟡 Medium

## ✅ Passed (4/6)

### 1. 코딩 스타일 ✅
- Decimal 타입 사용: ✅ 모든 금융 계산에 적용
- unwrap() 없음: ✅ ? 연산자로 대체
- 거래소 중립성: ✅ Exchange trait 사용
- 한글 주석: ✅ 모든 주석 한글

### 2. 보안 ✅
- SQL Injection: ✅ 파라미터화된 쿼리
- API 키: ✅ 환경변수 사용
- 민감 정보 로그: ✅ 마스킹 처리

### 3. 성능 ✅
- clone 최적화: ✅ 참조 사용
- 비동기 I/O: ✅ Tokio 활용
- 캐싱: ✅ Redis 사용

### 4. Git 히스토리 ✅
- 커밋 메시지: ✅ Conventional Commits
- Breaking Changes: ✅ 명시됨

## ⚠️ Issues Found (2/6)

### 5. 테스트 커버리지 ⚠️ (Medium Priority)

**문제**:
- `schema_registry.rs`: 단위 테스트 없음 (694줄)
- `schema_composer.rs`: 엣지 케이스 테스트 부족

**위치**:
- `crates/trader-strategy/src/schema_registry.rs`
- `crates/trader-strategy/src/schema_composer.rs`

**제안**:
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_register_strategy() { }

    #[test]
    fn test_get_schema_not_found() { }

    #[test]
    fn test_compose_fragments() { }
}
```

**영향**: 버그 위험 증가, 리팩토링 안전성 저하

### 6. 문서화 ⚠️ (Low Priority)

**문제**:
- `SchemaRegistry::register()`: Rustdoc 없음
- `SchemaComposer::compose()`: Examples 없음

**위치**:
- `crates/trader-strategy/src/schema_registry.rs:45`
- `crates/trader-strategy/src/schema_composer.rs:89`

**제안**:
- Arguments, Returns, Errors 섹션 추가
- 문서 테스트 예제 작성

**영향**: API 사용법 불명확, 신규 개발자 진입 장벽

## 📈 Metrics

| 메트릭 | 값 | 목표 | 상태 |
|--------|---:|-----:|:----:|
| 테스트 커버리지 | 78% | 80%+ | 🟡 |
| Clippy 경고 | 3개 | 0개 | 🟡 |
| 문서화율 | 65% | 80%+ | 🟡 |
| 복잡도 (평균 CC) | 5.2 | <8 | ✅ |

## 🎯 Action Items

### High Priority
- [ ] schema_registry 단위 테스트 추가 (4시간)
- [ ] schema_composer 엣지 케이스 테스트 (2시간)

### Medium Priority
- [ ] Clippy 경고 3개 수정 (1시간)
- [ ] Rustdoc 누락 항목 작성 (2시간)

### Low Priority
- [ ] 문서 테스트 예제 추가 (1시간)

**예상 총 시간**: 10시간

## 💡 Best Practices Observed

1. ✨ Proc macro 활용으로 보일러플레이트 제거
2. ✨ 공통 모듈 추출로 코드 중복 2,000줄 감소
3. ✨ 도메인 레이어 강화로 비즈니스 로직 명확화

## 🚦 Recommendation

**승인 조건부 ✅ (LGTM with minor changes)**

테스트 커버리지와 문서화는 개선이 필요하지만, 코어 로직은 견고합니다.
High Priority 항목 완료 후 머지 권장.
```

### 실행 예시

```rust
Task(
    subagent_type="code-reviewer",
    description="전략 스키마 PR 리뷰",
    prompt=r#"
PR #42 리뷰: 전략 스키마 시스템 구현

변경사항:
- trader-strategy-macro 신규 크레이트 (266줄)
- SchemaRegistry (694줄)
- SchemaComposer (279줄)
- 26개 전략에 #[strategy_metadata] 적용

체크리스트:
1. 코딩 스타일 (Decimal, unwrap, 거래소 중립성)
2. 보안 (SQL Injection, API 키)
3. 성능 (clone, 비동기)
4. 테스트 커버리지
5. 문서화 (Rustdoc)
6. Git 히스토리

각 항목별로 Pass/Fail 판정하고,
Issue 발견 시 위치, 문제점, 제안 제시.
최종 승인 여부 결정.
"#
)
```

---

## 5. ux-reviewer

### 역할
사용자 경험 관점에서 시스템을 평가하고, 사용성 개선 방안을 제시합니다.

### 실행 시점
- 새 API 엔드포인트 추가 시
- 웹 대시보드 UI 변경 시
- 에러 메시지 개선 시
- CLI 명령어 추가 시
- 사용자 피드백 수렴 시

### 평가 항목

#### 1. API 설계 일관성

**RESTful 원칙**:
```
✅ 좋은 API 설계:
GET    /api/strategies              # 전략 목록 조회
GET    /api/strategies/:name        # 개별 전략 조회
POST   /api/strategies              # 전략 등록
PUT    /api/strategies/:name        # 전략 수정
DELETE /api/strategies/:name        # 전략 삭제

GET    /api/strategies/:name/schema # 스키마 조회 (서브리소스)

❌ 나쁜 API 설계:
POST   /api/get-strategy             # GET 사용해야 함
GET    /api/strategies/delete/:name  # DELETE 사용해야 함
POST   /api/strategySchema           # camelCase 일관성 없음
```

**응답 형식 일관성**:
```json
✅ 성공 응답 (일관된 구조):
{
  "success": true,
  "data": {
    "strategies": [...]
  },
  "metadata": {
    "total": 26,
    "page": 1
  }
}

✅ 에러 응답 (일관된 구조):
{
  "success": false,
  "error": {
    "code": "STRATEGY_NOT_FOUND",
    "message": "전략을 찾을 수 없습니다: RSI",
    "details": {
      "requested": "RSI",
      "available": ["rsi", "bollinger", ...]
    }
  }
}

❌ 비일관적 응답:
// 어떤 API는 data 래핑, 어떤 API는 직접 반환
// 에러 구조가 API마다 다름
```

**체크리스트**:
- [ ] HTTP 메서드 적절성 (GET/POST/PUT/DELETE)
- [ ] URL 네이밍 일관성 (kebab-case)
- [ ] 응답 구조 통일 (success, data, error)
- [ ] 페이지네이션 표준 (page, limit, total)
- [ ] 필터링 쿼리 파라미터 일관성
- [ ] 버전 관리 (/api/v1)

#### 2. 에러 메시지 명확성

**사용자 친화적 에러**:
```rust
// ❌ 개발자 중심 메시지
Err("symbol not found")

// ✅ 사용자 중심 메시지
Err(SymbolError::NotFound {
    symbol: "AAPL",
    message: "종목을 찾을 수 없습니다: AAPL",
    suggestion: "종목 코드를 확인하거나 trader fetch-symbols 명령어로 최신 데이터를 가져오세요.",
})

// ❌ 기술적 세부사항 노출
Err("SQL error: relation 'symbols' does not exist")

// ✅ 추상화된 메시지
Err(DatabaseError::TableMissing {
    message: "데이터베이스 초기화가 필요합니다.",
    action: "sqlx migrate run 명령어를 실행하세요.",
})
```

**에러 레벨링**:
```rust
pub enum PositionSizingError {
    // 사용자 실수 (4xx)
    #[error("리스크가 너무 높습니다: {risk_percent}% (최대 {max_percent}%)")]
    RiskTooHigh {
        risk_percent: Decimal,
        max_percent: Decimal,
    },

    // 시스템 문제 (5xx)
    #[error("가격 데이터를 가져올 수 없습니다. 잠시 후 다시 시도하세요.")]
    DataUnavailable,
}
```

**체크리스트**:
- [ ] 에러 메시지 한글 (기술 용어는 영문 병기)
- [ ] 원인 명확히 설명
- [ ] 해결 방법 제시
- [ ] 에러 코드 일관성 (UPPER_SNAKE_CASE)
- [ ] 4xx vs 5xx 구분 (사용자 vs 서버)
- [ ] 민감 정보 노출 방지

#### 3. 웹 대시보드 UI/UX

**레이아웃 일관성**:
```tsx
// ✅ 일관된 컴포넌트 구조
<PageLayout>
  <PageHeader
    title="전략 관리"
    actions={<Button>새 전략 추가</Button>}
  />
  <PageContent>
    <DataTable ... />
  </PageContent>
</PageLayout>

// ❌ 페이지마다 다른 구조
<div>
  <h1>전략</h1>
  <table>...</table>
</div>
```

**반응형 디자인**:
```css
/* ✅ 모바일 우선 */
.strategy-card {
  width: 100%;
}

@media (min-width: 768px) {
  .strategy-card {
    width: 50%;
  }
}

@media (min-width: 1024px) {
  .strategy-card {
    width: 33.33%;
  }
}
```

**체크리스트**:
- [ ] 일관된 컬러 팔레트
- [ ] 타이포그래피 계층 (h1, h2, body)
- [ ] 버튼 스타일 통일 (primary, secondary, danger)
- [ ] 로딩 상태 표시 (스피너, 스켈레톤)
- [ ] 에러 상태 표시 (토스트, 알럿)
- [ ] 빈 상태 디자인 (empty state)
- [ ] 반응형 (모바일, 태블릿, 데스크탑)

#### 4. 접근성 (Accessibility)

**키보드 내비게이션**:
```tsx
// ✅ 키보드 접근 가능
<button
  onClick={handleClick}
  onKeyPress={(e) => e.key === 'Enter' && handleClick()}
  tabIndex={0}
>
  전략 시작
</button>

// ❌ 마우스만 가능
<div onClick={handleClick}>전략 시작</div>
```

**스크린 리더 지원**:
```tsx
// ✅ aria 속성
<button
  aria-label="전략 삭제"
  aria-describedby="delete-warning"
>
  <TrashIcon />
</button>
<span id="delete-warning" className="sr-only">
  이 작업은 되돌릴 수 없습니다
</span>

// ❌ 아이콘만
<button>
  <TrashIcon />
</button>
```

**체크리스트**:
- [ ] 키보드 내비게이션 (Tab, Enter, Esc)
- [ ] 포커스 표시 (focus ring)
- [ ] aria-label, aria-describedby
- [ ] 의미있는 HTML (button vs div)
- [ ] 색상 대비 (WCAG AA 이상)
- [ ] 폰트 크기 조절 가능

#### 5. 성능 및 반응성

**로딩 시간**:
```
✅ 목표:
- 페이지 초기 로딩: < 2초
- API 응답: < 500ms (p95)
- 차트 렌더링: < 1초
- 전략 실행: < 3초

🔴 경고:
- 페이지 초기 로딩: > 5초
- API 응답: > 2초
- 차트 렌더링: > 3초
```

**데이터 페칭 전략**:
```tsx
// ✅ Optimistic UI (즉각 반응)
function StrategyToggle() {
  const [isRunning, setIsRunning] = useState(false);

  const handleToggle = async () => {
    // 즉시 UI 업데이트
    setIsRunning(!isRunning);

    try {
      await api.toggleStrategy(strategyId);
    } catch (error) {
      // 실패 시 롤백
      setIsRunning(isRunning);
      showError(error);
    }
  };
}

// ❌ 응답 대기 (느린 반응)
function StrategyToggle() {
  const handleToggle = async () => {
    // 응답 올 때까지 대기... (느림)
    const result = await api.toggleStrategy(strategyId);
    setIsRunning(result.is_running);
  };
}
```

**체크리스트**:
- [ ] 로딩 인디케이터 (500ms 이상 소요 시)
- [ ] Optimistic UI 업데이트
- [ ] 에러 복구 (재시도, 롤백)
- [ ] 캐싱 (React Query, SWR)
- [ ] Lazy loading (차트, 이미지)
- [ ] 가상 스크롤 (긴 목록)

#### 6. CLI 사용성

**명령어 직관성**:
```bash
✅ 직관적:
trader fetch-symbols --market KR
trader list-symbols --format csv
trader sync-csv --file data/krx_codes.csv

❌ 비직관적:
trader fs -m KR
trader ls -f csv
trader sc -f data/krx_codes.csv
```

**도움말 품질**:
```bash
$ trader fetch-symbols --help

USAGE:
    trader fetch-symbols [OPTIONS]

DESCRIPTION:
    거래소에서 종목 목록을 자동으로 가져와 DB에 저장합니다.

OPTIONS:
    -m, --market <MARKET>    시장 선택 [KR|US|CRYPTO|ALL] [기본값: ALL]
    -o, --output <FILE>      CSV 파일로 저장 (선택)
    --dry-run                실제 저장 없이 미리보기만

EXAMPLES:
    # 한국 시장만 가져오기
    trader fetch-symbols --market KR

    # 전체 시장 + CSV 백업
    trader fetch-symbols --market ALL --output symbols.csv

    # 드라이런 모드 (테스트)
    trader fetch-symbols --dry-run

더 많은 정보: https://github.com/berrzebb/zeroquant/wiki
```

**체크리스트**:
- [ ] 명령어 이름 직관적 (fetch, list, sync)
- [ ] 짧은 옵션 (-m) + 긴 옵션 (--market)
- [ ] 도움말 한글
- [ ] Examples 섹션 포함
- [ ] 에러 메시지 명확
- [ ] 프로그레스 바 (긴 작업 시)

### 평가 출력 형식

```markdown
# UX Review Report

## 📊 Summary
- **리뷰 범위**: 전략 스키마 API + 프론트엔드
- **전체 점수**: 82/100 (Good)
- **사용성**: ⭐⭐⭐⭐☆

## ✅ Strengths (잘된 점)

### 1. API 설계 일관성 ✅ (95/100)
- RESTful 원칙 준수
- 응답 구조 통일 (success, data, error)
- 명확한 엔드포인트 네이밍

**예시**:
```
GET /api/strategies/schema      ✅
GET /api/strategies/:name/schema ✅
```

### 2. 에러 메시지 ✅ (90/100)
- 한글 메시지 + 영문 기술 용어 병기
- 해결 방법 제시
- 에러 레벨 구분 (4xx vs 5xx)

**예시**:
```json
{
  "error": {
    "code": "STRATEGY_NOT_FOUND",
    "message": "전략을 찾을 수 없습니다: RSI",
    "suggestion": "사용 가능한 전략: rsi, bollinger, grid, ..."
  }
}
```

### 3. CLI 사용성 ✅ (88/100)
- 직관적인 명령어 이름
- 풍부한 도움말
- Examples 포함

## ⚠️ Issues Found (개선 필요)

### 1. 로딩 상태 표시 ⚠️ (Medium Priority)

**문제**:
- 전략 스키마 로딩 시 인디케이터 없음
- 사용자가 응답을 기다리는지 알 수 없음

**위치**:
- `frontend/src/pages/Strategies.tsx:142`

**제안**:
```tsx
{isLoading ? (
  <div className="flex justify-center p-8">
    <Spinner />
    <span>스키마를 불러오는 중...</span>
  </div>
) : (
  <SchemaForm schema={schema} />
)}
```

**영향**: 사용자 혼란, 이탈률 증가

### 2. 빈 상태 디자인 ⚠️ (Low Priority)

**문제**:
- 전략이 없을 때 빈 테이블만 표시
- 다음 액션 제시 없음

**위치**:
- `frontend/src/components/StrategyList.tsx`

**제안**:
```tsx
{strategies.length === 0 ? (
  <EmptyState
    icon={<StrategyIcon />}
    title="등록된 전략이 없습니다"
    description="새로운 전략을 추가하여 자동 트레이딩을 시작하세요."
    action={
      <Button onClick={onAdd}>
        첫 전략 추가하기
      </Button>
    }
  />
) : (
  <Table data={strategies} />
)}
```

**영향**: 신규 사용자 온보딩 개선

### 3. 접근성 ⚠️ (Low Priority)

**문제**:
- 스키마 폼의 입력 필드에 label 연결 없음
- 키보드 내비게이션 불완전

**위치**:
- `frontend/src/components/SchemaForm.tsx`

**제안**:
```tsx
<label htmlFor="risk-percent">
  리스크 비율 (%)
</label>
<input
  id="risk-percent"
  type="number"
  aria-describedby="risk-help"
  {...}
/>
<span id="risk-help" className="text-sm text-gray-600">
  포트폴리오 대비 리스크 비율 (권장: 1-3%)
</span>
```

**영향**: 접근성 저하, WCAG 미준수

## 📈 Metrics

| 카테고리 | 점수 | 목표 | 상태 |
|---------|-----:|-----:|:----:|
| API 설계 | 95 | 90+ | ✅ |
| 에러 메시지 | 90 | 85+ | ✅ |
| UI/UX | 75 | 85+ | 🟡 |
| 접근성 | 65 | 80+ | 🟡 |
| 성능 | 88 | 85+ | ✅ |
| CLI | 88 | 85+ | ✅ |

**전체**: 82/100 (Good)

## 🎯 Action Items

### High Priority
없음

### Medium Priority
- [ ] 로딩 상태 표시 추가 (2시간)
- [ ] 빈 상태 디자인 구현 (3시간)

### Low Priority
- [ ] 접근성 개선 (label, aria) (4시간)
- [ ] 키보드 내비게이션 개선 (2시간)

**예상 총 시간**: 11시간

## 💡 Best Practices Observed

1. ✨ RESTful API 설계 원칙 준수
2. ✨ 한글 에러 메시지 + 해결 방법 제시
3. ✨ CLI 도움말에 Examples 포함

## 🎨 UI/UX 개선 제안

### 단기 (1-2주)
1. **로딩 스켈레톤**: 차트 로딩 시 스켈레톤 UI
2. **토스트 알림**: 성공/실패 피드백
3. **빈 상태**: 모든 목록 컴포넌트

### 중기 (1-2개월)
1. **다크 모드**: 테마 전환 기능
2. **키보드 단축키**: Cmd+K 명령 팔레트
3. **접근성**: WCAG AA 준수

### 장기 (3-6개월)
1. **온보딩 투어**: 신규 사용자 가이드
2. **대시보드 커스터마이징**: 위젯 배치
3. **모바일 앱**: React Native

## 🚦 Recommendation

**승인 ✅ (Good to Go)**

전반적으로 우수한 UX 품질입니다.
Medium Priority 항목은 다음 스프린트에서 개선 권장.
```

### 실행 예시

```rust
Task(
    subagent_type="ux-reviewer",
    description="전략 스키마 UX 리뷰",
    prompt=r#"
전략 스키마 시스템 UX 평가:

1. API 설계:
   - GET /api/strategies/schema
   - GET /api/strategies/:name/schema
   - RESTful 원칙 준수 여부
   - 응답 구조 일관성

2. 프론트엔드:
   - SchemaForm 컴포넌트 사용성
   - 로딩 상태 표시
   - 에러 처리
   - 빈 상태 디자인

3. 접근성:
   - 키보드 내비게이션
   - aria 속성
   - 색상 대비

4. 성능:
   - 로딩 시간
   - Optimistic UI

각 항목별로 점수(0-100)와
개선 제안을 제시하세요.
"#
)
```

---

## 🚀 사용 가이드

### Task Tool 통합

```rust
// build-validator
Task(
    subagent_type="build-validator",
    description="빌드 검증",
    prompt="전체 워크스페이스 빌드 및 테스트 실행. Clippy 경고 포함."
)

// code-architect
Task(
    subagent_type="code-architect",
    description="StrategyContext 설계",
    prompt="전략 간 공유 컨텍스트 아키텍처 설계. 동시성 안전, 성능 최적화 고려."
)

// code-simplifier
Task(
    subagent_type="code-simplifier",
    description="전략 모듈 단순화",
    prompt="trader-strategy 크레이트 분석. 중복 코드, 복잡도, 레거시 식별."
)
```

### 병렬 실행

```rust
// 독립적인 에이전트는 병렬 실행 가능
// 단일 메시지에 여러 Task 호출
Task(subagent_type="build-validator", ...)
Task(subagent_type="code-simplifier", ...)
```

### 순차 실행

```rust
// 의존성이 있는 경우 순차 실행
// 1. 설계
Task(subagent_type="code-architect", ...)

// 2. 구현 (사용자가 직접 또는 다른 에이전트)

// 3. 검증
Task(subagent_type="build-validator", ...)
```

---

## 📚 참고

### 관련 문서
- `docs/development_rules.md` - 개발 규칙 (v1.1, 180+ 규칙)
- `docs/agent_guidelines.md` - AI 에이전트 가이드라인
- `docs/architecture.md` - 시스템 아키텍처

### Rust 도구
- `cargo-expand`: Macro expansion 확인
- `cargo-audit`: 보안 취약점 스캔
- `cargo-udeps`: 사용하지 않는 의존성
- `cargo-bloat`: 바이너리 크기 분석

---

**버전 히스토리**:
- v1.1.0 (2026-02-03): code-reviewer, ux-reviewer 추가
- v1.0.0 (2026-02-02): 초기 정의 (build-validator, code-architect, code-simplifier)
