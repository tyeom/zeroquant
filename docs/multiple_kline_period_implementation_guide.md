# Multiple KLine Period 구현 가이드

> **버전**: 1.0  
> **작성일**: 2026-02-02  
> **대상**: 개발자  
> **참조**: `multiple_kline_period_requirements.md`, `STRATEGY_DEVELOPMENT.md`

---

## 📋 목차

1. [빠른 시작](#-빠른-시작)
2. [Phase별 체크리스트](#-phase별-체크리스트)
3. [코드 예제](#-코드-예제)
4. [테스트 가이드](#-테스트-가이드)
5. [트러블슈팅](#-트러블슈팅)
6. [FAQ](#-faq)

---

## 🚀 빠른 시작

### 개발 순서

```
Phase 1: 데이터 모델 (1주)
    ↓
Phase 2: 데이터 조회 (1주)  ← 성능 테스트 필수
    ↓
Phase 3: Context 통합 (1주)
    ↓
Phase 4: 전략 예제 (1주)   ← MVP 완성
    ↓
Phase 5: UI/API (1.5주)    ← 사용자 경험
    ↓
Phase 6: 통합 (1.5주)      ← 프로덕션 준비
```

### MVP 범위 (Phase 1-4)

Phase 1-4 완료 시 다음 기능이 동작합니다:
- ✅ 전략에서 멀티 타임프레임 데이터 접근
- ✅ 데이터 조회 API 최적화 (< 50ms)
- ✅ 2개 이상의 예제 전략 동작
- ⏳ 프론트엔드 UI (Phase 5)
- ⏳ 백테스트 완전 통합 (Phase 6)

---

## ✅ Phase별 체크리스트

### Phase 1: 데이터 모델 확장

#### 1.1 Config 구조체 작성

**파일**: `crates/trader-strategy/src/config.rs`

```rust
// 1. MultiTimeframeConfig 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiTimeframeConfig {
    pub primary: Timeframe,
    #[serde(default)]
    pub secondary: Vec<Timeframe>,
    #[serde(default = "default_lookback")]
    pub lookback_periods: HashMap<Timeframe, usize>,
}

// 2. 유효성 검증 메서드
impl MultiTimeframeConfig {
    pub fn validate(&self) -> Result<()> {
        // TODO: Secondary가 Primary보다 큰지 확인
        // TODO: Secondary가 2개 이하인지 확인
    }
    
    pub fn all_timeframes(&self) -> Vec<Timeframe> {
        // TODO: Primary + Secondary 반환
    }
}
```

**테스트**:
- [ ] `validate()` 메서드가 잘못된 설정 감지
- [ ] `all_timeframes()` 메서드가 올바른 순서 반환
- [ ] Serde 직렬화/역직렬화 동작

#### 1.2 DB 마이그레이션

**파일**: `migrations/XXXX_add_multi_timeframe.sql`

```sql
-- 1. strategies 테이블에 컬럼 추가
ALTER TABLE strategies 
ADD COLUMN secondary_timeframes TEXT[];

-- 2. 기존 데이터 마이그레이션 (빈 배열로 초기화)
UPDATE strategies 
SET secondary_timeframes = '{}' 
WHERE secondary_timeframes IS NULL;
```

**검증**:
- [ ] 마이그레이션 실행 (`sqlx migrate run`)
- [ ] 기존 전략 데이터 정상 조회
- [ ] Rollback 테스트

#### 1.3 StrategyContext 확장

**파일**: `crates/trader-strategy/src/context.rs`

```rust
pub struct StrategyContext {
    // 기존 필드...
    
    /// 타임프레임별 캔들 데이터
    pub klines_by_timeframe: HashMap<Timeframe, Vec<Kline>>,
    
    /// 멀티 타임프레임 설정
    pub multi_tf_config: MultiTimeframeConfig,
    
    /// 현재 평가 중인 타임스탬프
    pub current_timestamp: DateTime<Utc>,
}

impl StrategyContext {
    pub fn get_klines(&self, tf: Timeframe) -> Result<&[Kline]> {
        // TODO: 구현
    }
    
    pub fn primary_klines(&self) -> Result<&[Kline]> {
        self.get_klines(self.multi_tf_config.primary)
    }
    
    pub fn latest_kline(&self, tf: Timeframe) -> Result<&Kline> {
        // TODO: 구현
    }
}
```

**테스트**:
- [ ] `get_klines()` 메서드가 올바른 데이터 반환
- [ ] 없는 타임프레임 조회 시 에러 반환
- [ ] `primary_klines()` 편의 메서드 동작

---

### Phase 2: 데이터 조회 API

#### 2.1 OhlcvCache 확장

**파일**: `crates/trader-data/src/storage/ohlcv.rs`

```rust
impl OhlcvCache {
    pub async fn get_multi_timeframe_klines(
        &self,
        symbol: &Symbol,
        timeframes: &[Timeframe],
        limit: usize,
    ) -> Result<HashMap<Timeframe, Vec<Kline>>> {
        // Step 1: Redis 멀티 GET
        let cache_keys: Vec<String> = timeframes
            .iter()
            .map(|tf| format!("ohlcv:{}:{}:latest_{}", symbol, tf, limit))
            .collect();
        
        let cached = self.redis.mget(&cache_keys).await?;
        
        // Step 2: 캐시 미스 처리
        let missing_tfs = /* 캐시 미스된 타임프레임 */;
        
        if !missing_tfs.is_empty() {
            let db_results = self.fetch_from_db(symbol, &missing_tfs, limit).await?;
            // Step 3: Redis에 캐싱
            self.cache_to_redis(db_results).await?;
        }
        
        Ok(/* 결과 반환 */)
    }
    
    async fn fetch_from_db(
        &self,
        symbol: &Symbol,
        timeframes: &[Timeframe],
        limit: usize,
    ) -> Result<HashMap<Timeframe, Vec<Kline>>> {
        // TODO: UNION ALL 쿼리 구현
    }
}
```

**SQL 쿼리 최적화**:

```sql
-- 비효율적 (3번 쿼리)
SELECT * FROM ohlcv WHERE symbol = $1 AND timeframe = '5m' LIMIT 100;
SELECT * FROM ohlcv WHERE symbol = $1 AND timeframe = '1h' LIMIT 100;
SELECT * FROM ohlcv WHERE symbol = $1 AND timeframe = '1d' LIMIT 100;

-- 최적화 (1번 쿼리)
SELECT * FROM (
    SELECT * FROM ohlcv WHERE symbol = $1 AND timeframe = '5m' 
    ORDER BY open_time DESC LIMIT 100
) UNION ALL
SELECT * FROM (
    SELECT * FROM ohlcv WHERE symbol = $1 AND timeframe = '1h' 
    ORDER BY open_time DESC LIMIT 100
) UNION ALL
SELECT * FROM (
    SELECT * FROM ohlcv WHERE symbol = $1 AND timeframe = '1d' 
    ORDER BY open_time DESC LIMIT 100
)
ORDER BY timeframe, open_time DESC;
```

**성능 테스트**:
- [ ] 단일 타임프레임 조회 벤치마크 (기준선)
- [ ] 3개 타임프레임 조회 벤치마크 (목표: < 50ms)
- [ ] 캐시 히트율 측정 (목표: > 80%)
- [ ] DB 쿼리 실행 계획 분석 (`EXPLAIN ANALYZE`)

**테스트 코드**:

```rust
#[tokio::test]
async fn test_multi_timeframe_query_performance() {
    let cache = setup_test_cache().await;
    let symbol = Symbol::from_str("BTCUSDT").unwrap();
    let timeframes = vec![Timeframe::M5, Timeframe::H1, Timeframe::D1];
    
    // Warm-up
    let _ = cache.get_multi_timeframe_klines(&symbol, &timeframes, 100).await;
    
    // Benchmark
    let start = Instant::now();
    let result = cache.get_multi_timeframe_klines(&symbol, &timeframes, 100).await;
    let elapsed = start.elapsed();
    
    assert!(result.is_ok());
    assert!(elapsed.as_millis() < 50, "Query took {}ms, expected < 50ms", elapsed.as_millis());
}
```

---

### Phase 3: Context Layer 통합

#### 3.1 StrategyExecutor 수정

**파일**: `crates/trader-strategy/src/executor.rs`

```rust
impl StrategyExecutor {
    async fn create_context(
        &self,
        strategy: &dyn Strategy,
        symbol: &Symbol,
    ) -> Result<StrategyContext> {
        // 1. 멀티 타임프레임 설정 가져오기
        let config = strategy.multi_timeframe_config();
        let timeframes = config.all_timeframes();
        
        // 2. 데이터 로드
        let klines_by_tf = self.ohlcv_cache
            .get_multi_timeframe_klines(symbol, &timeframes, 100)
            .await?;
        
        // 3. 시간 정렬
        let aligned_klines = self.align_timeframes(&klines_by_tf, &config)?;
        
        // 4. Context 생성
        Ok(StrategyContext {
            klines_by_timeframe: aligned_klines,
            multi_tf_config: config,
            current_timestamp: Utc::now(),
            // ... 기타 필드
        })
    }
    
    fn align_timeframes(
        &self,
        klines: &HashMap<Timeframe, Vec<Kline>>,
        config: &MultiTimeframeConfig,
    ) -> Result<HashMap<Timeframe, Vec<Kline>>> {
        // TODO: 미래 데이터 누출 방지 로직
    }
}
```

**Alignment 로직**:

```rust
fn align_timeframes(
    primary_kline: &Kline,
    secondary_klines: Vec<Kline>,
) -> Vec<Kline> {
    secondary_klines
        .into_iter()
        .filter(|k| k.open_time < primary_kline.open_time)
        .collect()
}
```

**테스트**:
- [ ] Context 생성 시 모든 타임프레임 데이터 로드
- [ ] Alignment가 미래 데이터 제외
- [ ] 데이터 누락 시 에러 처리

---

### Phase 4: 전략 예제 작성

#### 4.1 RSI 멀티 타임프레임 전략

**파일**: `crates/trader-strategy/src/strategies/rsi_multi_timeframe.rs`

```rust
pub struct RsiMultiTimeframeStrategy {
    config: RsiMtfConfig,
}

#[derive(StrategyConfig)]
pub struct RsiMtfConfig {
    pub symbol: Symbol,
    pub multi_timeframe: MultiTimeframeConfig,
    pub rsi_period: usize,
    pub oversold_threshold: f64,
}

#[async_trait]
impl Strategy for RsiMultiTimeframeStrategy {
    async fn analyze(&self, ctx: &StrategyContext) -> Result<Signal> {
        // Step 1: 일봉 추세 확인
        let klines_daily = ctx.get_klines(Timeframe::D1)?;
        let rsi_daily = calculate_rsi(klines_daily, self.config.rsi_period);
        
        if rsi_daily < 50.0 {
            // 일봉 약세 → 매수 금지
            return Ok(Signal::Hold);
        }
        
        // Step 2: 1시간 진입 신호
        let klines_hourly = ctx.get_klines(Timeframe::H1)?;
        let rsi_hourly = calculate_rsi(klines_hourly, self.config.rsi_period);
        
        if rsi_hourly > self.config.oversold_threshold {
            // 아직 과매도 아님
            return Ok(Signal::Hold);
        }
        
        // Step 3: 5분 확인 신호
        let klines_5m = ctx.primary_klines()?;
        let rsi_5m = calculate_rsi(klines_5m, self.config.rsi_period);
        
        if rsi_5m < self.config.oversold_threshold && is_bouncing(klines_5m) {
            return Ok(Signal::Buy);
        }
        
        Ok(Signal::Hold)
    }
}

fn is_bouncing(klines: &[Kline]) -> bool {
    // 최근 2개 캔들이 상승하는지 확인
    if klines.len() < 2 {
        return false;
    }
    klines[0].close > klines[1].close && klines[1].close > klines[2].close
}
```

**테스트 시나리오**:

```rust
#[tokio::test]
async fn test_rsi_multi_timeframe_buy_signal() {
    // Given: 일봉 RSI > 50, 1시간 RSI < 30, 5분 RSI 반등
    let ctx = create_test_context(
        daily_rsi: 55.0,
        hourly_rsi: 28.0,
        minute_rsi: 29.0,
        is_bouncing: true,
    );
    
    let strategy = RsiMultiTimeframeStrategy::new(/* config */);
    
    // When
    let signal = strategy.analyze(&ctx).await.unwrap();
    
    // Then
    assert_eq!(signal, Signal::Buy);
}

#[tokio::test]
async fn test_rsi_multi_timeframe_filter_by_daily() {
    // Given: 일봉 RSI < 50 (약세)
    let ctx = create_test_context(
        daily_rsi: 45.0,
        hourly_rsi: 28.0,
        minute_rsi: 29.0,
        is_bouncing: true,
    );
    
    let strategy = RsiMultiTimeframeStrategy::new(/* config */);
    
    // When
    let signal = strategy.analyze(&ctx).await.unwrap();
    
    // Then
    assert_eq!(signal, Signal::Hold, "일봉 약세 시 매수 금지");
}
```

#### 4.2 헬퍼 함수 작성

**파일**: `crates/trader-strategy/src/utils/multi_timeframe.rs`

```rust
/// 타임프레임별 추세 분석
pub fn analyze_trend(klines: &[Kline]) -> Trend {
    if klines.len() < 20 {
        return Trend::Neutral;
    }
    
    let ma_short = calculate_sma(klines, 10);
    let ma_long = calculate_sma(klines, 20);
    
    match ma_short.partial_cmp(&ma_long) {
        Some(Ordering::Greater) => Trend::Bullish,
        Some(Ordering::Less) => Trend::Bearish,
        _ => Trend::Neutral,
    }
}

/// 여러 타임프레임의 RSI 계산
pub fn calculate_multi_rsi(
    ctx: &StrategyContext,
    timeframes: &[Timeframe],
    period: usize,
) -> Result<HashMap<Timeframe, f64>> {
    let mut result = HashMap::new();
    
    for tf in timeframes {
        let klines = ctx.get_klines(*tf)?;
        let rsi = calculate_rsi(klines, period);
        result.insert(*tf, rsi);
    }
    
    Ok(result)
}

/// 신호 강도 평가
pub enum SignalStrength {
    Strong,   // 모든 TF 동의
    Medium,   // 일부 TF 동의
    Weak,     // 단일 TF만
}

pub fn combine_signals(
    signals: HashMap<Timeframe, Signal>,
) -> (Signal, SignalStrength) {
    let buy_count = signals.values().filter(|s| **s == Signal::Buy).count();
    let total = signals.len();
    
    match buy_count {
        n if n == total => (Signal::Buy, SignalStrength::Strong),
        n if n > 0 => (Signal::Buy, SignalStrength::Medium),
        _ => (Signal::Hold, SignalStrength::Weak),
    }
}
```

---

### Phase 5: SDUI 및 API

#### 5.1 SDUI 스키마

**파일**: `crates/trader-api/src/routes/strategies/schema.rs`

```rust
pub fn get_multi_timeframe_schema(primary: Timeframe) -> serde_json::Value {
    json!({
        "type": "multi-select",
        "id": "secondary_timeframes",
        "label": "보조 타임프레임 (최대 2개)",
        "description": "Primary보다 큰 타임프레임만 선택 가능",
        "options": get_valid_secondaries(primary),
        "max_selections": 2,
        "validation": {
            "rule": "larger_than_primary",
            "error_message": "보조 타임프레임은 Primary보다 커야 합니다"
        }
    })
}

fn get_valid_secondaries(primary: Timeframe) -> Vec<serde_json::Value> {
    let all_tfs = vec![
        Timeframe::M1, Timeframe::M3, Timeframe::M5, Timeframe::M15, Timeframe::M30,
        Timeframe::H1, Timeframe::H2, Timeframe::H4, Timeframe::H6, Timeframe::H8, Timeframe::H12,
        Timeframe::D1, Timeframe::D3, Timeframe::W1, Timeframe::MN1,
    ];
    
    all_tfs.into_iter()
        .filter(|tf| tf.as_secs() > primary.as_secs())
        .map(|tf| json!({
            "value": tf.to_string(),
            "label": tf.display_name()
        }))
        .collect()
}
```

#### 5.2 API 엔드포인트

**파일**: `crates/trader-api/src/routes/strategies/mod.rs`

```rust
// GET /api/v1/strategies/{id}/timeframes
pub async fn get_strategy_timeframes(
    Path(id): Path<i32>,
    State(state): State<AppState>,
) -> Result<Json<TimeframeResponse>> {
    let strategy = state.strategy_repo.find_by_id(id).await?;
    let config = strategy.multi_timeframe_config;
    
    Ok(Json(TimeframeResponse {
        strategy_id: id,
        primary: TimeframeInfo {
            timeframe: config.primary.to_string(),
            description: config.primary.display_name(),
            last_update: get_last_update(&state, &strategy.symbol, config.primary).await?,
        },
        secondary: config.secondary.iter().map(|tf| {
            TimeframeInfo {
                timeframe: tf.to_string(),
                description: tf.display_name(),
                last_update: /* ... */,
            }
        }).collect(),
    }))
}

#[derive(Serialize)]
pub struct TimeframeResponse {
    pub strategy_id: i32,
    pub primary: TimeframeInfo,
    pub secondary: Vec<TimeframeInfo>,
}

#[derive(Serialize)]
pub struct TimeframeInfo {
    pub timeframe: String,
    pub description: String,
    pub last_update: DateTime<Utc>,
}
```

#### 5.3 프론트엔드 컴포넌트

**파일**: `frontend/src/components/MultiTimeframeSelector.tsx`

```tsx
import { Component, For, createSignal } from "solid-js";

interface Props {
  primaryTimeframe: string;
  selectedSecondaries: string[];
  onChange: (secondaries: string[]) => void;
}

export const MultiTimeframeSelector: Component<Props> = (props) => {
  const [selected, setSelected] = createSignal<string[]>(props.selectedSecondaries);
  
  const validOptions = () => {
    const primary = parseTimeframe(props.primaryTimeframe);
    return ALL_TIMEFRAMES.filter(tf => tf.seconds > primary.seconds);
  };
  
  const handleToggle = (tf: string) => {
    const current = selected();
    
    if (current.includes(tf)) {
      // 제거
      const updated = current.filter(x => x !== tf);
      setSelected(updated);
      props.onChange(updated);
    } else if (current.length < 2) {
      // 추가 (최대 2개)
      const updated = [...current, tf];
      setSelected(updated);
      props.onChange(updated);
    } else {
      alert("최대 2개까지 선택 가능합니다");
    }
  };
  
  return (
    <div class="multi-timeframe-selector">
      <label>보조 타임프레임 (최대 2개)</label>
      <div class="options">
        <For each={validOptions()}>
          {(tf) => (
            <button
              class={selected().includes(tf.value) ? "selected" : ""}
              onClick={() => handleToggle(tf.value)}
            >
              {tf.label}
            </button>
          )}
        </For>
      </div>
      <p class="hint">
        Primary({props.primaryTimeframe})보다 큰 타임프레임만 선택 가능합니다
      </p>
    </div>
  );
};
```

---

### Phase 6: 백테스트 및 실시간 통합

#### 6.1 백테스트 엔진 수정

**파일**: `crates/trader-strategy/src/backtest/engine.rs`

```rust
impl BacktestEngine {
    async fn run_with_multi_timeframe(&mut self) -> Result<BacktestReport> {
        // 1. 히스토리 데이터 로드 (모든 타임프레임)
        let history = self.load_multi_timeframe_history().await?;
        
        // 2. Primary 타임프레임 기준으로 반복
        for kline in &history.primary_klines {
            // 3. Secondary 데이터 정렬
            let aligned_secondaries = self.align_at_timestamp(
                kline.open_time,
                &history.secondary_klines,
            );
            
            // 4. Context 생성
            let ctx = StrategyContext {
                klines_by_timeframe: aligned_secondaries,
                current_timestamp: kline.open_time,
                // ...
            };
            
            // 5. 전략 실행
            let signal = self.strategy.analyze(&ctx).await?;
            self.process_signal(signal, kline)?;
        }
        
        Ok(self.generate_report())
    }
    
    async fn load_multi_timeframe_history(&self) -> Result<MultiTimeframeHistory> {
        let config = self.strategy.multi_timeframe_config();
        let timeframes = config.all_timeframes();
        
        // 모든 타임프레임 데이터 한 번에 로드
        let klines_by_tf = self.ohlcv_cache
            .get_multi_timeframe_klines(&self.symbol, &timeframes, 10000)
            .await?;
        
        Ok(MultiTimeframeHistory { klines_by_tf })
    }
}
```

#### 6.2 WebSocket 멀티 스트림

**파일**: `crates/trader-exchange/src/websocket/binance.rs`

```rust
impl BinanceWebSocket {
    pub async fn subscribe_multi_timeframe(
        &mut self,
        symbol: &str,
        timeframes: &[Timeframe],
    ) -> Result<()> {
        let streams: Vec<String> = timeframes
            .iter()
            .map(|tf| format!("{}@kline_{}", symbol.to_lowercase(), tf.to_binance_interval()))
            .collect();
        
        // Combined stream 구독
        self.subscribe_combined(&streams).await?;
        
        Ok(())
    }
    
    pub async fn handle_kline_update(&mut self, update: KlineUpdate) -> Result<()> {
        let timeframe = Timeframe::from_binance_interval(&update.interval)?;
        
        // Context 업데이트
        self.context.update_kline(timeframe, update.kline);
        
        // Primary 타임프레임 완료 시에만 전략 재평가
        if timeframe == self.context.multi_tf_config.primary && update.is_final {
            self.evaluate_strategy().await?;
        }
        
        Ok(())
    }
}
```

---

## 🧪 테스트 가이드

### 단위 테스트

```bash
# 특정 모듈 테스트
cargo test -p trader-strategy multi_timeframe

# 성능 테스트
cargo test -p trader-data --release -- --nocapture test_multi_timeframe_query_performance
```

### 통합 테스트

```rust
#[tokio::test]
async fn test_end_to_end_multi_timeframe() {
    // 1. 전략 생성
    let strategy = RsiMultiTimeframeStrategy::new(/* ... */);
    
    // 2. 백테스트 실행
    let report = backtest(&strategy, start_date, end_date).await?;
    
    // 3. 검증
    assert!(report.total_trades > 0);
    assert!(report.win_rate > 0.5);
}
```

### 수동 테스트

```bash
# 1. 전략 생성 API 호출
curl -X POST http://localhost:8080/api/v1/strategies \
  -H "Content-Type: application/json" \
  -d '{
    "name": "RSI MTF Test",
    "strategy_type": "RsiMultiTimeframe",
    "multi_timeframe_config": {
      "primary": "5m",
      "secondary": ["1h", "1d"]
    },
    "parameters": { ... }
  }'

# 2. 타임프레임 설정 확인
curl http://localhost:8080/api/v1/strategies/1/timeframes

# 3. 백테스트 실행
curl -X POST http://localhost:8080/api/v1/backtest \
  -d '{"strategy_id": 1, "start_date": "2024-01-01", "end_date": "2024-12-31"}'
```

---

## 🔧 트러블슈팅

### 문제 1: "TimeframeNotLoaded" 에러

**증상**:
```
Error: TimeframeNotLoaded(H1)
```

**원인**: Context에 요청한 타임프레임 데이터가 없음

**해결**:
1. `multi_timeframe_config`에 해당 타임프레임 추가했는지 확인
2. `StrategyExecutor`에서 데이터 로드 로직 확인
3. 로그 확인: `RUST_LOG=trader_strategy=debug cargo run`

### 문제 2: 멀티 조회가 느림 (> 200ms)

**증상**: 성능 목표 미달성

**진단**:
```rust
// 성능 프로파일링
let start = Instant::now();
let result = cache.get_multi_timeframe_klines(...).await;
println!("Elapsed: {:?}", start.elapsed());
```

**해결**:
1. Redis 캐시 히트율 확인
2. PostgreSQL 쿼리 실행 계획 확인 (`EXPLAIN ANALYZE`)
3. 인덱스 추가:
   ```sql
   CREATE INDEX idx_ohlcv_symbol_tf_time 
   ON ohlcv(symbol, timeframe, open_time DESC);
   ```

### 문제 3: 백테스트 결과가 실시간과 다름

**증상**: 같은 전략이 백테스트와 실시간에서 다른 신호 생성

**원인**: Timeframe Alignment 버그 (미래 데이터 누출)

**디버깅**:
```rust
// Context 생성 시 로그 추가
println!("Primary timestamp: {}", ctx.current_timestamp);
for (tf, klines) in &ctx.klines_by_timeframe {
    println!("  {}: latest = {}", tf, klines[0].open_time);
    assert!(klines[0].open_time < ctx.current_timestamp, "미래 데이터 감지!");
}
```

---

## ❓ FAQ

### Q1: Secondary는 왜 최대 2개인가요?

**A**: 성능과 복잡도의 균형입니다.
- 3개 타임프레임 조회: ~50ms
- 5개 타임프레임 조회: ~120ms (목표 초과)
- 대부분의 전문 트레이더도 3개 이하 사용

필요 시 설정으로 확장 가능:
```rust
const MAX_SECONDARY_TIMEFRAMES: usize = 3; // 기본 2 → 3으로 변경
```

### Q2: 1분봉 Primary에 5분봉 Secondary는 불가능한가요?

**A**: 네, 불가능합니다.
- Secondary는 Primary보다 **큰** 타임프레임만 허용
- 이유: 작은 TF는 정보가 중복되어 의미 없음
- 예: 1분봉으로 5분봉을 만들 수 있지만, 5분봉으로 1분봉은 만들 수 없음

### Q3: 백테스트에서 Secondary 데이터가 부족하면?

**A**: 에러 처리 옵션:
1. **Strict Mode** (기본): 에러 발생, 백테스트 중단
2. **Skip Mode**: 해당 타임스탬프 건너뜀
3. **Fill Mode**: 가장 가까운 데이터로 채움 (위험)

```rust
pub enum MissingDataPolicy {
    Error,   // 기본
    Skip,
    Fill,
}
```

### Q4: 실시간에서 Secondary가 먼저 업데이트되면?

**A**: Primary 완료까지 대기합니다.
- Secondary 업데이트 → Context에 반영만
- Primary 완료 → 전략 재평가
- 이유: Primary 주기가 실제 거래 주기이므로

```
10:25:00 - 5분봉 업데이트 → 전략 실행 ✅
10:26:30 - 1시간봉 업데이트 → Context만 갱신, 실행 안함 ⏸️
10:30:00 - 5분봉 업데이트 → 전략 실행 ✅ (최신 1시간 데이터 사용)
```

### Q5: 프론트엔드 없이 CLI로만 테스트 가능한가요?

**A**: 가능합니다.

```bash
# 1. 전략 Config JSON 작성
cat > rsi_mtf.json <<EOF
{
  "name": "RSI MTF",
  "strategy_type": "RsiMultiTimeframe",
  "multi_timeframe_config": {
    "primary": "5m",
    "secondary": ["1h", "1d"]
  },
  "parameters": { ... }
}
EOF

# 2. CLI로 백테스트 실행
cargo run -p trader-cli -- backtest \
  --config rsi_mtf.json \
  --start 2024-01-01 \
  --end 2024-12-31
```

---

## 📚 추가 참조

- **상세 요구사항**: `docs/multiple_kline_period_requirements.md`
- **전략 개발 가이드**: `docs/STRATEGY_DEVELOPMENT.md`
- **API 문서**: `docs/api.md`
- **아키텍처**: `docs/architecture.md`

---

**마지막 업데이트**: 2026-02-02  
**작성자**: ZeroQuant Development Team
