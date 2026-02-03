# Python 전략 모듈 가이드

> `python-strategy/추가파일/` 폴더의 Python 코드를 분석하여
> ZeroQuant에 통합할 때 참조할 수 있도록 정리한 문서입니다.
> **해외 거래소(US, EU, Asia) 적용을 위한 일반화 방안**을 포함합니다.

---

## 📁 파일 개요

| 파일명 | 주요 기능 | 적용 시장 | 일반화 난이도 |
|--------|-----------|-----------|---------------|
| `schema.py` | 종목 상태 enum | 전체 | ⭐ 쉬움 |
| `price_utils.py` | 호가단위/가격 포맷 | KR → 전체 | ⭐⭐ 보통 |
| `naver_crawler.py` | 뉴스 크롤러 | KR → 전체 | ⭐⭐⭐ 어려움 |
| `dart_collector.py` | 공시 분석 | KR → 전체 | ⭐⭐⭐ 어려움 |
| `ml.py` | ML 예측 엔진 | 전체 | ⭐ 쉬움 |
| `strategy_lab.py` | 백테스트 UI | 전체 | ⭐ 쉬움 |
| `collector2.py` | 팩터 분석 | KR → 전체 | ⭐⭐ 보통 |
| `all.py` | 대시보드 | 전체 | ⭐ 쉬움 |
| `additional.py` | **Global Score 스코어링** | KR → 전체 | ⭐⭐ 보통 |

---

## 1. schema.py - 종목 상태 정의

### 원본 코드
```python
class RouteState:
    OVERHEAT = "OVERHEAT"   # 과열 - 익절/주의
    WAIT = "WAIT"           # 대기 - 타점 대기
    ARMED = "ARMED"         # 임박 - 진입 준비 (스퀴즈)
    ATTACK = "ATTACK"       # 공략 - 진입 시그널
    NEUTRAL = "NEUTRAL"     # 중립
```

### 일반화
시장에 관계없이 동일하게 적용 가능합니다.

### Rust 구현
```rust
// trader-core/src/types/route_state.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum RouteState {
    Overheat,   // 과열 - 익절 고려
    Wait,       // 대기 - 타점 대기
    Armed,      // 임박 - 진입 준비
    Attack,     // 공략 - 진입 시그널
    Neutral,    // 중립
}
```

---

## 2. price_utils.py - 호가단위 유틸리티

### 원본 (KRX 전용)
```python
def krx_tick_size(price: float) -> int:
    if price < 2000: return 1
    if price < 5000: return 5
    # ... KRX 7단계 호가단위
```

### 일반화 설계

**거래소별 틱 사이즈 규칙:**

| 거래소 | 규칙 | 예시 |
|--------|------|------|
| **KRX** | 가격대별 7단계 | 50,000원 → 100원 |
| **NYSE/NASDAQ** | 고정 $0.01 (페니 틱) | $150.00 → $0.01 |
| **LSE** | 가격대별 변동 | £10 이하 0.25p |
| **TSE (일본)** | 가격대별 변동 | ¥3,000 이하 1円 |
| **HKEX** | 가격대별 변동 | HK$0.25~5,000 |
| **Binance** | 심볼별 상이 | BTC: 0.01 USDT |

### Rust 구현 (일반화)
```rust
// trader-core/src/utils/tick_size.rs

use rust_decimal::Decimal;
use rust_decimal_macros::dec;

/// 거래소 유형
#[derive(Debug, Clone, Copy)]
pub enum Exchange {
    Krx,       // 한국 (KOSPI, KOSDAQ)
    UsEquity,  // 미국 주식 (NYSE, NASDAQ, AMEX)
    Lse,       // 런던
    Tse,       // 일본
    Hkex,      // 홍콩
    Binance,   // 바이낸스 (암호화폐)
}

/// 틱 사이즈 제공자 trait
pub trait TickSizeProvider {
    fn tick_size(&self, price: Decimal) -> Decimal;
    fn round_to_tick(&self, price: Decimal, method: RoundMethod) -> Decimal;
}

/// KRX 틱 사이즈 (7단계)
pub struct KrxTickSize;

impl TickSizeProvider for KrxTickSize {
    fn tick_size(&self, price: Decimal) -> Decimal {
        match price {
            p if p < dec!(2000) => dec!(1),
            p if p < dec!(5000) => dec!(5),
            p if p < dec!(20000) => dec!(10),
            p if p < dec!(50000) => dec!(50),
            p if p < dec!(200000) => dec!(100),
            p if p < dec!(500000) => dec!(500),
            _ => dec!(1000),
        }
    }
    // ...
}

/// 미국 주식 틱 사이즈 (고정 $0.01)
pub struct UsEquityTickSize;

impl TickSizeProvider for UsEquityTickSize {
    fn tick_size(&self, _price: Decimal) -> Decimal {
        dec!(0.01)  // 페니 틱
    }
    // ...
}

/// 거래소별 틱 사이즈 팩토리
pub fn get_tick_provider(exchange: Exchange) -> Box<dyn TickSizeProvider> {
    match exchange {
        Exchange::Krx => Box::new(KrxTickSize),
        Exchange::UsEquity => Box::new(UsEquityTickSize),
        Exchange::Binance => Box::new(BinanceTickSize::default()),
        // ...
    }
}
```

### 가격 포맷팅 (통화별)
```rust
// trader-core/src/utils/format.rs

pub fn format_price(value: Decimal, currency: Currency) -> String {
    match currency {
        Currency::Krw => format!("{}원", value.round().to_string().replace(...)),
        Currency::Usd => format!("${:.2}", value),
        Currency::Eur => format!("€{:.2}", value),
        Currency::Gbp => format!("£{:.2}", value),
        Currency::Jpy => format!("¥{}", value.round()),
        Currency::Hkd => format!("HK${:.2}", value),
    }
}
```

---

## 3. naver_crawler.py → 뉴스 수집 일반화

### 원본 (네이버 금융 전용)
```python
url = f"https://finance.naver.com/item/news_news.naver?code={code}"
```

### 일반화 설계

**시장별 뉴스 소스:**

| 시장 | 소스 | API/방식 |
|------|------|----------|
| KR | 네이버 금융 | 크롤링 (EUC-KR) |
| US | Yahoo Finance | 크롤링/RSS |
| US | Finnhub | REST API (무료) |
| US | Alpha Vantage | REST API |
| Global | NewsAPI | REST API |
| Global | Google News | RSS |

### Rust 구현 (일반화)
```rust
// trader-data/src/news/mod.rs

pub trait NewsProvider: Send + Sync {
    async fn fetch_news(&self, symbol: &str, days: u32) -> Result<Vec<NewsItem>>;
}

pub struct NewsItem {
    pub headline: String,
    pub source: String,
    pub published_at: DateTime<Utc>,
    pub url: Option<String>,
    pub sentiment: Option<f32>,  // -1.0 ~ 1.0
}

// 네이버 금융 (KR)
pub struct NaverNewsProvider { /* ... */ }

// Finnhub (US, 무료 API)
pub struct FinnhubNewsProvider {
    api_key: String,
}

impl NewsProvider for FinnhubNewsProvider {
    async fn fetch_news(&self, symbol: &str, days: u32) -> Result<Vec<NewsItem>> {
        // GET https://finnhub.io/api/v1/company-news
        // ?symbol=AAPL&from=2024-01-01&to=2024-01-10&token=xxx
    }
}

// 팩토리
pub fn get_news_provider(market: Market) -> Box<dyn NewsProvider> {
    match market {
        Market::Kr => Box::new(NaverNewsProvider::new()),
        Market::Us => Box::new(FinnhubNewsProvider::new()),
        _ => Box::new(YahooNewsProvider::new()),
    }
}
```

---

## 4. dart_collector.py → 공시 시스템 일반화

### 원본 (DART 전용)
```python
self.dart = OpenDartReader(dart_api_key)  # 한국 DART
```

### 일반화 설계

**시장별 공시 시스템:**

| 시장 | 시스템 | 데이터 형식 |
|------|--------|-------------|
| KR | DART (금융감독원) | XML/JSON |
| US | SEC EDGAR | XML (XBRL) |
| UK | Companies House | JSON |
| JP | EDINET | XML |
| HK | HKEX News | HTML |

### Rust 구현 (일반화)
```rust
// trader-data/src/disclosure/mod.rs

pub trait DisclosureProvider: Send + Sync {
    async fn get_filings(&self, symbol: &str, days: u32) -> Result<Vec<Filing>>;
    async fn get_filing_content(&self, filing_id: &str) -> Result<String>;
}

pub struct Filing {
    pub id: String,
    pub title: String,
    pub filing_type: FilingType,  // 10-K, 8-K, 공급계약 등
    pub filed_at: DateTime<Utc>,
    pub url: String,
}

pub enum FilingType {
    // US (SEC)
    Form10K,      // 연간 보고서
    Form10Q,      // 분기 보고서
    Form8K,       // 수시 공시
    // KR (DART)
    AnnualReport,
    QuarterlyReport,
    MaterialContract,  // 공급계약
    CapitalIncrease,   // 유상증자
    // Common
    Other(String),
}

// DART 구현 (KR)
pub struct DartProvider { api_key: String }

// SEC EDGAR 구현 (US)
pub struct EdgarProvider;

impl DisclosureProvider for EdgarProvider {
    async fn get_filings(&self, symbol: &str, days: u32) -> Result<Vec<Filing>> {
        // SEC EDGAR는 CIK(Central Index Key)로 조회
        // https://www.sec.gov/cgi-bin/browse-edgar?action=getcompany&CIK=...
    }
}
```

### LLM 분석 일반화
```rust
// trader-analytics/src/disclosure_analyzer.rs

pub struct DisclosureAnalyzer {
    llm_client: Box<dyn LlmClient>,
}

impl DisclosureAnalyzer {
    /// 공시 분석 (시장 불문)
    pub async fn analyze(&self, filing: &Filing, content: &str) -> AnalysisResult {
        let prompt = self.build_prompt(filing, content);
        let response = self.llm_client.generate(&prompt).await?;
        self.parse_response(&response)
    }

    fn build_prompt(&self, filing: &Filing, content: &str) -> String {
        format!(r#"
        Analyze this corporate filing and rate its impact on stock price.

        [Filing Type] {:?}
        [Title] {}
        [Content] {}

        Respond in JSON: {{"score": -5 to +5, "reason": "brief explanation"}}
        "#, filing.filing_type, filing.title, &content[..10000.min(content.len())])
    }
}
```

---

## 5. ml.py - ML 예측 엔진

### 일반화 포인트
이미 시장에 관계없이 적용 가능한 구조입니다.

**피처 일반화:**

| 피처 | 설명 | 시장 의존성 |
|------|------|-------------|
| OHLCV | 기본 가격/거래량 | ❌ 없음 |
| Low_Trend | 저점 상승 강도 | ❌ 없음 |
| Vol_Quality | 양봉/음봉 거래량 비율 | ❌ 없음 |
| Range_Pos | 박스권 내 위치 | ❌ 없음 |
| Dist_MA20 | MA20 이격도 | ❌ 없음 |
| BB_Width | 볼린저 밴드 폭 | ❌ 없음 |
| RSI | 과매수/과매도 | ❌ 없음 |

### Rust 구현
```rust
// trader-analytics/src/ml/features.rs

pub struct StructuralFeatures {
    pub low_trend: f64,      // Higher Low 강도
    pub vol_quality: f64,    // 매수세/매도세 비율
    pub range_pos: f64,      // 박스권 위치 (0~1)
    pub dist_ma20: f64,      // MA20 이격도
    pub bb_width: f64,       // BB 폭 (에너지 응축)
    pub rsi: f64,            // RSI
}

impl StructuralFeatures {
    /// OHLCV 데이터로부터 피처 계산 (시장 불문)
    pub fn from_candles(candles: &[Candle]) -> Option<Self> {
        if candles.len() < 30 { return None; }

        // 1. Low_Trend (저점 상승 강도)
        let min_prev = candles[..10].iter().map(|c| c.low).min()?;
        let min_curr = candles[10..20].iter().map(|c| c.low).min()?;
        let low_trend = (min_curr - min_prev) / min_prev;

        // 2. Vol_Quality (양봉 vs 음봉 거래량)
        let (vol_up, vol_down) = candles.iter().fold((0.0, 0.0), |(up, down), c| {
            if c.close > c.open {
                (up + c.volume, down)
            } else {
                (up, down + c.volume)
            }
        });
        let vol_quality = if vol_down > 0.0 { vol_up / vol_down } else { 1.0 };

        // ... 나머지 피처 계산

        Some(Self { low_trend, vol_quality, /* ... */ })
    }
}
```

---

## 6. collector2.py - 팩터 분석 일반화

### 팩터 가중치 (시장 공통)
```python
W_RR = 0.25     # Risk/Reward 비율
W_T1 = 0.18     # 목표가 근접도
W_SL = 0.12     # 손절폭
W_NEAR = 0.12   # 진입가 근접도
W_MOM = 0.10    # 모멘텀
W_LIQ = 0.13    # 유동성
W_TEC = 0.10    # 기술적 지표
```

### 시장별 조정 필요 항목

| 항목 | KR 기준 | US 조정 | 이유 |
|------|---------|---------|------|
| MIN_TURNOVER | 50억원 | $50M | 시장 규모 차이 |
| MIN_MCAP | 1,000억원 | $500M | 소형주 기준 차이 |
| RSI_RANGE | 45-65 | 30-70 | 시장 변동성 차이 |

### Rust 구현
```rust
// trader-analytics/src/screening/factor_config.rs

pub struct FactorConfig {
    pub min_turnover: Decimal,  // 최소 거래대금
    pub min_market_cap: Decimal,
    pub rsi_low: f64,
    pub rsi_high: f64,
    // 팩터 가중치 (시장 공통)
    pub weight_rr: f64,
    pub weight_momentum: f64,
    pub weight_liquidity: f64,
    // ...
}

impl FactorConfig {
    pub fn for_market(market: Market) -> Self {
        match market {
            Market::Kr => Self {
                min_turnover: dec!(5_000_000_000),  // 50억원
                min_market_cap: dec!(100_000_000_000),  // 1,000억원
                rsi_low: 45.0,
                rsi_high: 65.0,
                ..Default::default()
            },
            Market::Us => Self {
                min_turnover: dec!(50_000_000),  // $50M
                min_market_cap: dec!(500_000_000),  // $500M
                rsi_low: 30.0,
                rsi_high: 70.0,
                ..Default::default()
            },
            _ => Self::default(),
        }
    }
}
```

---

## 7. 데이터 소스 일반화

### 원본 (KR 전용)
```python
import FinanceDataReader as fdr  # 한국 주식
from pykrx import stock           # KRX 데이터
```

### 일반화된 데이터 소스

| 기능 | KR | US | Global |
|------|-----|-----|--------|
| 주가 | FinanceDataReader | Yahoo Finance | Yahoo Finance |
| 종목 목록 | pykrx, KRX | SEC, Finnhub | 거래소별 |
| 시가총액 | pykrx | Yahoo Finance | Yahoo Finance |
| 재무제표 | DART | SEC EDGAR | Yahoo Finance |

### 기존 ZeroQuant 연동
```rust
// 이미 구현된 데이터 소스 활용
use trader_data::yahoo::YahooProvider;     // 글로벌 주가
use trader_exchange::binance::BinanceApi;  // 암호화폐
use trader_exchange::kis::KisApi;          // 한국 주식
```

---

## 8. additional.py - Global Score 스코어링 시스템

### 개요
종합 스코어링 엔진으로, 모든 기술적 지표를 단일 점수(GLOBAL_SCORE 0~100)로 종합합니다.

### 핵심 아키텍처

#### 가중치 시스템 (고정, 합계=1.0)
```python
W_RR   = 0.25  # 보상대비위험 (Risk/Reward Ratio)
W_T1   = 0.18  # 목표가1 여유율
W_SL   = 0.12  # 손절가 여유율
W_NEAR = 0.12  # 현재가-추천가 근접도
W_MOM  = 0.10  # 모멘텀 (ERS + MACD slope + RSI 중심)
W_LIQ  = 0.13  # 유동성 (거래대금 퍼센타일)
W_TEC  = 0.10  # 기술균형 (VolZ 스윗스팟 + 乖離 안정성)
```

#### 페널티 시스템 (점수 차감)
```python
P_OVERHEAT_5D  = 6.0  # 5일 수익률 +10% 초과 시
P_OVERHEAT_10D = 6.0  # 10일 수익률 +20% 초과 시
P_RSI_OUT      = 4.0  # RSI 45~65 밴드 이탈
P_MACD_NEG     = 4.0  # MACD 기울기 음수
P_NEAR_FAR     = 4.0  # 진입가 괴리 과다
P_LIQ_LOW      = 4.0  # 유동성 하위 20%
P_VOL_SPIKE    = 2.0  # 변동성 스파이크 (VolZ > 3)
```

#### 유동성 하드컷 (시장별)
```python
MIN_TURN_KOSPI  = 200.0  # KOSPI: 200억원 이상
MIN_TURN_KOSDAQ = 100.0  # KOSDAQ: 100억원 이상
# 후보 부족 시 자동 완화: KOSPI 150억, KOSDAQ 80억
```

#### 품질 게이트
```python
PASS_EBS = 4  # EBS(Entry Balance Score) ≥ 4
# 후보 부족 시 자동 완화: EBS ≥ 3
```

### 일반화 설계

#### 시장별 유동성 기준

| 시장 | 최소 거래대금 | 완화 기준 | 비고 |
|------|--------------|----------|------|
| **KR-KOSPI** | 200억원 | 150억원 | 대형주 |
| **KR-KOSDAQ** | 100억원 | 80억원 | 중소형주 |
| **US-NYSE/NASDAQ** | $100M | $50M | 일평균 거래대금 |
| **US-SmallCap** | $10M | $5M | 소형주 |
| **JP-TSE Prime** | ¥10B | ¥5B | 프라임 마켓 |
| **HK-Main Board** | HK$50M | HK$20M | 메인보드 |

#### ERS(Entry Ready Score) 계산
```python
# ERS = 3개 조건의 합 (0~3)
ers_bits = (
    (ebs >= PASS_EBS).astype(int) +      # 품질게이트 통과
    (macd_slope > 0).astype(int) +       # 모멘텀 상승
    ((rsi >= 45) & (rsi <= 65)).astype(int)  # RSI 중립대
)
ers_norm = ers_bits / 3.0  # 정규화 (0~1)
```

### Rust 구현

```rust
// trader-analytics/src/scoring/global_rank.rs

use rust_decimal::Decimal;
use rust_decimal_macros::dec;

/// 고정 가중치 (시장 불문)
pub struct ScoringWeights {
    pub risk_reward: Decimal,    // W_RR = 0.25
    pub target_room: Decimal,    // W_T1 = 0.18
    pub stop_room: Decimal,      // W_SL = 0.12
    pub entry_proximity: Decimal,// W_NEAR = 0.12
    pub momentum: Decimal,       // W_MOM = 0.10
    pub liquidity: Decimal,      // W_LIQ = 0.13
    pub technical: Decimal,      // W_TEC = 0.10
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            risk_reward: dec!(0.25),
            target_room: dec!(0.18),
            stop_room: dec!(0.12),
            entry_proximity: dec!(0.12),
            momentum: dec!(0.10),
            liquidity: dec!(0.13),
            technical: dec!(0.10),
        }
    }
}

/// 페널티 설정 (시장별 조정 가능)
pub struct PenaltyConfig {
    pub overheat_5d: f64,       // 5일 과열
    pub overheat_10d: f64,      // 10일 과열
    pub rsi_out_of_band: f64,   // RSI 밴드 이탈
    pub macd_negative: f64,     // MACD 음수
    pub entry_far: f64,         // 진입가 괴리
    pub low_liquidity: f64,     // 저유동성
    pub volatility_spike: f64,  // 변동성 스파이크
}

/// 유동성 게이트 (시장별 설정)
pub struct LiquidityGate {
    pub min_turnover: Decimal,
    pub relaxed_turnover: Decimal,
}

impl LiquidityGate {
    pub fn for_market(market: Market) -> Self {
        match market {
            Market::KrKospi => Self {
                min_turnover: dec!(20_000_000_000),     // 200억원
                relaxed_turnover: dec!(15_000_000_000), // 150억원
            },
            Market::KrKosdaq => Self {
                min_turnover: dec!(10_000_000_000),     // 100억원
                relaxed_turnover: dec!(8_000_000_000),  // 80억원
            },
            Market::UsNyse | Market::UsNasdaq => Self {
                min_turnover: dec!(100_000_000),        // $100M
                relaxed_turnover: dec!(50_000_000),     // $50M
            },
            Market::UsSmallCap => Self {
                min_turnover: dec!(10_000_000),         // $10M
                relaxed_turnover: dec!(5_000_000),      // $5M
            },
            _ => Self::default(),
        }
    }
}

/// Global Score 계산기
pub struct GlobalScorer {
    weights: ScoringWeights,
    penalties: PenaltyConfig,
    liquidity_gate: LiquidityGate,
}

impl GlobalScorer {
    /// 종목 점수 계산 (0~100)
    pub fn calculate(&self, data: &SymbolData) -> ScoreResult {
        // 1. 개별 팩터 정규화 (0~1)
        let rr_norm = self.normalize_risk_reward(data);
        let t1_norm = self.normalize_target_room(data);
        let sl_norm = self.normalize_stop_room(data);
        let near_norm = self.normalize_entry_proximity(data);
        let mom_norm = self.calculate_momentum(data);
        let liq_norm = self.normalize_liquidity(data);
        let tec_norm = self.calculate_technical_balance(data);

        // 2. 가중 합계 (0~100)
        let base_score = 100.0 * (
            self.weights.risk_reward * rr_norm +
            self.weights.target_room * t1_norm +
            self.weights.stop_room * sl_norm +
            self.weights.entry_proximity * near_norm +
            self.weights.momentum * mom_norm +
            self.weights.liquidity * liq_norm +
            self.weights.technical * tec_norm
        );

        // 3. 페널티 적용
        let penalty = self.calculate_penalties(data);
        let final_score = (base_score - penalty).clamp(0.0, 100.0);

        ScoreResult {
            global_score: final_score,
            components: ScoreComponents { rr_norm, t1_norm, /* ... */ },
            passed_gate: self.check_liquidity_gate(data),
        }
    }

    /// 모멘텀 점수 (ERS 기반)
    fn calculate_momentum(&self, data: &SymbolData) -> f64 {
        let ebs_ok = if data.ebs >= 4 { 1.0 } else { 0.0 };
        let slope_ok = if data.macd_slope > 0.0 { 1.0 } else { 0.0 };
        let rsi_ok = if (45.0..=65.0).contains(&data.rsi) { 1.0 } else { 0.0 };

        let ers = (ebs_ok + slope_ok + rsi_ok) / 3.0;
        let slope_norm = (data.macd_slope.max(0.0) / self.slope_cap).min(1.0);
        let rsi_center = 1.0 - ((data.rsi - 55.0).abs() / 10.0).min(1.0);

        (0.5 * ers + 0.3 * slope_norm + 0.2 * rsi_center).clamp(0.0, 1.0)
    }

    /// 기술적 균형 점수
    fn calculate_technical_balance(&self, data: &SymbolData) -> f64 {
        // VolZ 스윗스팟: 1에 가까울수록 좋음
        let vol_sweet = 1.0 - ((data.vol_z - 1.0).abs() / 3.0).min(1.0);
        // 乖離 안정성: 0에 가까울수록 좋음
        let kairi_norm = 1.0 - (data.kairi.abs() / self.kairi_cap).min(1.0);

        0.6 * vol_sweet + 0.4 * kairi_norm
    }
}
```

### API 통합 예시

```rust
// POST /api/v1/ranking/global
#[derive(Deserialize)]
pub struct GlobalRankRequest {
    pub market: Market,
    pub top_n: Option<usize>,  // 기본값: 10
    pub include_relaxed: Option<bool>,  // 완화 조건 포함 여부
}

#[derive(Serialize)]
pub struct GlobalRankResponse {
    pub rankings: Vec<RankedSymbol>,
    pub generated_at: DateTime<Utc>,
    pub gate_mode: GateMode,  // "strict" | "relaxed"
}

#[derive(Serialize)]
pub struct RankedSymbol {
    pub rank: u32,
    pub ticker: String,
    pub name: String,
    pub global_score: f64,
    pub components: ScoreComponents,
    pub passed_quality_gate: bool,
}
```

---

## 9. REGIME - 시장 레짐 분류 ⭐ NEW

### 개요
종목의 현재 추세 단계를 5단계로 분류하여 매매 타이밍 판단에 활용합니다.

### 원본 코드 (collector2.py)
```python
def detect_regime_row(row: pd.Series) -> str:
    """추세 단계(REGIME)를 텍스트로 분류"""
    rel60 = row.get("rel_60d_%", 0.0)  # 60일 초과수익(α)
    slope = row.get("MACD_Slope_PCT", 0.0)
    rsi = row.get("RSI14", 50.0)

    # ① 강한 상승 추세
    if rel60 > 10 and slope > 0 and 50 <= rsi <= 70:
        return "① 강한 상승 추세"
    # ② 상승 후 조정 구간
    if rel60 > 5 and slope <= 0:
        return "② 상승 후 조정"
    # ③ 박스 / 중립
    if -5 <= rel60 <= 5:
        return "③ 박스 / 중립"
    # ④ 바닥 반등 시도
    if rel60 <= -5 and slope > 0:
        return "④ 바닥 반등 시도"
    # ⑤ 하락 / 약세
    return "⑤ 하락 / 약세"
```

### Rust 구현
```rust
// trader-core/src/types/market_regime.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketRegime {
    StrongUptrend,    // ① 강한 상승 추세
    Correction,       // ② 상승 후 조정
    Sideways,         // ③ 박스 / 중립
    BottomBounce,     // ④ 바닥 반등 시도
    Downtrend,        // ⑤ 하락 / 약세
}

impl MarketRegime {
    pub fn detect(rel_60d_pct: f64, macd_slope: f64, rsi: f64) -> Self {
        if rel_60d_pct > 10.0 && macd_slope > 0.0 && (50.0..=70.0).contains(&rsi) {
            Self::StrongUptrend
        } else if rel_60d_pct > 5.0 && macd_slope <= 0.0 {
            Self::Correction
        } else if (-5.0..=5.0).contains(&rel_60d_pct) {
            Self::Sideways
        } else if rel_60d_pct <= -5.0 && macd_slope > 0.0 {
            Self::BottomBounce
        } else {
            Self::Downtrend
        }
    }
}
```

---

## 10. TRIGGER - 진입 트리거 시스템 ⭐ NEW

### 개요
여러 기술적 조건을 종합하여 진입 신호 강도(TRIGGER_SCORE)와 트리거 문자열을 생성합니다.

### 트리거 유형
| 트리거 | 조건 | 점수 |
|--------|------|------|
| 🚀급등시동 | TTM Squeeze 해제 + MACD 골든 | +30 |
| 📦박스돌파 | Range_Pos > 0.95 + 거래량 증가 | +25 |
| 🔥거래폭증 | Vol_Z > 2.5 + 양봉 | +20 |
| ⚡모멘텀 | RSI 상승 + MACD 기울기 양수 | +15 |
| 🔨망치형 | 캔들패턴 망치형 감지 | +10 |
| 💪장악형 | 캔들패턴 상승장악형 감지 | +10 |

### Rust 구현
```rust
// trader-analytics/src/trigger/mod.rs
pub struct TriggerResult {
    pub score: f64,           // 0~100
    pub triggers: Vec<TriggerType>,
    pub label: String,        // "🚀급등시동, 📦박스돌파"
}

pub enum TriggerType {
    SqueezeBreak,    // TTM Squeeze 해제
    BoxBreakout,     // 박스권 돌파
    VolumeSpike,     // 거래량 폭증
    MomentumUp,      // 모멘텀 상승
    HammerCandle,    // 망치형 캔들
    Engulfing,       // 장악형 캔들
}

impl TriggerResult {
    pub fn calculate(data: &SymbolAnalysis) -> Self {
        let mut score = 0.0;
        let mut triggers = Vec::new();

        // TTM Squeeze 해제 체크
        if data.ttm_squeeze_released && data.macd_golden_cross {
            score += 30.0;
            triggers.push(TriggerType::SqueezeBreak);
        }
        // ... 나머지 트리거

        Self { score, triggers, label: Self::build_label(&triggers) }
    }
}
```

---

## 11. Macro Filter - 매크로 환경 필터 ⭐ NEW

### 개요
USD/KRW 환율과 나스닥 지수를 모니터링하여 시장 위험도를 평가하고 진입 기준을 동적으로 조정합니다.

### 원본 코드 (collector2.py)
```python
def check_macro_env(trade_ymd: str) -> Tuple[str, str, int, int]:
    """
    Returns:
        risk_level: 'CRITICAL', 'HIGH', 'NORMAL'
        summary_msg: 텔레그램 출력용 메시지
        adj_ebs: 조정된 EBS 기준 (기본 4)
        rec_limit: 추천 종목 수 제한
    """
    # 1. USD/KRW 환율 조회
    curr_usd = fdr.DataReader('USD/KRW')['Close'].iloc[-1]
    usd_chg = (curr_usd - prev_usd) / prev_usd * 100

    # 2. 나스닥 조회
    nas_chg = (curr_nas - prev_nas) / prev_nas * 100

    # 위험도 판정
    risk_score = 0
    if curr_usd >= 1400 or usd_chg >= 0.5:
        risk_score += 1
    if nas_chg <= -2.0:
        risk_score += 2

    if risk_score >= 2:
        return "CRITICAL", msg, PASS_EBS + 1, 3
    elif risk_score == 1:
        return "HIGH", msg, PASS_EBS + 1, 5
    return "NORMAL", msg, PASS_EBS, 5
```

### Rust 구현
```rust
// trader-analytics/src/macro_filter.rs
pub struct MacroEnvironment {
    pub risk_level: MacroRisk,
    pub usd_krw: Decimal,
    pub usd_change_pct: f64,
    pub nasdaq_change_pct: f64,
    pub adjusted_ebs: u8,
    pub recommendation_limit: usize,
}

pub enum MacroRisk {
    Critical,  // EBS +1, 추천 3개
    High,      // EBS +1, 추천 5개
    Normal,    // 기본값
}
```

---

## 12. TTM Squeeze 상세 구현 ⭐ NEW

### 개요
John Carter의 TTM Squeeze: Bollinger Band가 Keltner Channel 내부로 들어가면 에너지 응축 상태(Squeeze)로 판단.

### 원본 로직
```python
# Bollinger Band
bb_upper = ma20 + 2 * std20
bb_lower = ma20 - 2 * std20

# Keltner Channel (ATR 기반)
kc_upper = ma20 + 1.5 * atr20
kc_lower = ma20 - 1.5 * atr20

# TTM Squeeze 조건: BB가 KC 안에 있으면 Squeeze
ttm_squeeze = (bb_lower > kc_lower) and (bb_upper < kc_upper)

# Squeeze 연속 일수 카운트
ttm_squeeze_cnt = consecutive_count(ttm_squeeze_series)
```

### Rust 구현
```rust
// trader-analytics/src/indicators/ttm_squeeze.rs
pub struct TtmSqueeze {
    pub is_squeeze: bool,
    pub squeeze_count: u32,       // 연속 스퀴즈 일수
    pub momentum: Decimal,        // 스퀴즈 모멘텀 (방향)
    pub released: bool,           // 이번 봉에서 해제되었는가?
}

impl TtmSqueeze {
    pub fn calculate(candles: &[Candle], bb_period: usize, kc_mult: f64) -> Self {
        let bb = BollingerBands::new(bb_period, 2.0).calculate(candles);
        let kc = KeltnerChannel::new(bb_period, kc_mult).calculate(candles);

        let is_squeeze = bb.lower > kc.lower && bb.upper < kc.upper;
        // ...
    }
}
```

---

## 13. 추가 기술적 지표 ⭐ NEW

### HMA (Hull Moving Average)
```python
def calc_hma(s: pd.Series, period: int) -> pd.Series:
    """반응 속도가 빠르고 휩소가 적은 이평선"""
    half_length = int(period / 2)
    sqrt_length = int(math.sqrt(period))
    wma_half = wma(s, half_length)
    wma_full = wma(s, period)
    raw_hma = 2 * wma_half - wma_full
    return wma(raw_hma, sqrt_length)
```

### OBV (On-Balance Volume)
```python
def calc_obv(close: pd.Series, volume: pd.Series) -> pd.Series:
    """스마트 머니 추적 지표"""
    change = np.sign(close.diff()).fillna(0)
    obv = (change * volume).cumsum()
    return obv
```

### SuperTrend
```python
def calc_supertrend(high, low, close, period=10, multiplier=3.0):
    """추세 추종 지표 (매수/매도 신호)"""
    atr = calc_atr(high, low, close, period)
    hl2 = (high + low) / 2
    basic_upper = hl2 + (multiplier * atr)
    basic_lower = hl2 - (multiplier * atr)
    # ... 추세 결정 로직
    return supertrend_line, trend_direction
```

### 캔들 패턴 감지
```python
def check_candle_pattern(o, h, l, c) -> List[str]:
    """망치형, 장악형 패턴 감지"""
    patterns = []
    body = abs(c[-1] - o[-1])
    lower_shadow = min(c[-1], o[-1]) - l[-1]

    # 망치형: 아랫꼬리 >= 몸통*2
    if lower_shadow >= body * 2:
        patterns.append("망치형")

    # 상승 장악형: 전일 음봉 -> 금일 양봉이 감쌈
    if prev_red and curr_green and curr_engulfs_prev:
        patterns.append("장악형")

    return patterns
```

---

## 14. Market Breadth - 시장 온도 ⭐ NEW

### 개요
20일 이동평균선을 상회하는 종목 비율로 시장 전체의 건강 상태를 측정합니다.

### 원본 코드
```python
def compute_market_breadth(df: pd.DataFrame) -> Dict[str, float]:
    """20일선 상회 비율(%) = 시장 온도"""
    return {
        "ALL": df["Above_MA20"].mean() * 100,
        "KOSPI": df[df["시장"]=="KOSPI"]["Above_MA20"].mean() * 100,
        "KOSDAQ": df[df["시장"]=="KOSDAQ"]["Above_MA20"].mean() * 100,
    }

def label_market_temp(breadth_all: float) -> str:
    if breadth_all >= 65: return "🔥 과열"
    if breadth_all <= 35: return "🧊 침체"
    return "🌤 중립"
```

### Rust 구현
```rust
pub struct MarketBreadth {
    pub all: f64,
    pub kospi: f64,
    pub kosdaq: f64,
    pub temperature: MarketTemperature,
}

pub enum MarketTemperature {
    Overheat,   // >= 65%
    Neutral,    // 35~65%
    Cold,       // <= 35%
}
```

---

## 15. Reality Check - 추천 검증 시스템 ⭐ NEW

### 개요
전일 추천 종목의 익일 실제 성과를 자동으로 검증하여 전략 신뢰도를 측정합니다.

### 원본 코드
```python
def run_reality_check(out_dir: str, trade_ymd: str) -> None:
    """전일 추천 종목 vs 오늘 종가 비교"""
    # 1. 오늘 종가 스냅샷 로드
    snap = pd.read_csv(f"price_snapshot_{trade_ymd}.csv")

    # 2. 전일 추천 파일 로드 (상위 30개)
    prev = pd.read_csv(f"recommend_{prev_ymd}.csv").head(30)

    # 3. 수익률 계산
    prev["전일→오늘_수익률%"] = (오늘종가 / 추천매수가 - 1.0) * 100

    # 4. 검증 결과 저장
    prev.to_csv(f"reality_check_{trade_ymd}.csv")
```

### 활용
- 전략 신뢰도 측정 (승률, 평균 수익률)
- 하이퍼파라미터 튜닝 피드백
- 백테스트와 실제 성과 괴리 분석

---

## 16. Strategy Lab - 백테스트 시뮬레이터 ⭐ NEW

### 개요
Streamlit 기반의 대화형 백테스트 시뮬레이터. 파라미터를 조정하며 과거 추천 종목의 가상 매매 성과를 검증합니다.

### 주요 기능
- **필터링 조건**: 최소 점수, RSI 범위, MFI 최소값
- **매매 규칙**: 보유 기간, 목표 수익률, 손절 비율
- **시뮬레이션 결과**: 승률, 평균 수익률, 누적 수익 곡선

### 원본 (strategy_lab.py)
```python
def run_simulation(df, price_map, hold_days, target_pct, stop_pct):
    for row in df.iterrows():
        entry_price = row['추천매수가']
        future_data = ohlcv[entry_date:].head(hold_days)

        # 익절/손절 체크
        if min_low <= stop_price:
            status = "STOP"
        elif max_high >= target_price:
            status = "WIN"
        else:
            status = "HOLD"

        ret = (exit_price - entry_price) / entry_price * 100 - 0.25  # 수수료
```

---

## 17. Sector RS - 섹터 상대강도 ⭐ NEW

### 개요
단순 수익률이 아닌 시장 대비 초과수익(Relative Strength)으로 진짜 주도 섹터를 발굴합니다.

### 원본 코드
```python
def add_sector_momentum(df: pd.DataFrame, group_col: str = "업종_대분류"):
    # 1. 단순 모멘텀 (5일 평균 수익률)
    g_ret = df.groupby(group_col)["ret_5d_%"].mean()

    # 2. 시장 대비 초과 수익 (20일 평균 RS)
    g_rs = df.groupby(group_col)["rel_20d_%"].mean()

    # 3. 종합 섹터 점수 (RS 60% + 수익 40%)
    sector_score = (g_ret * 0.4) + (g_rs * 0.6)

    df["SECTOR_RS"] = df[group_col].map(g_rs)
    df["SECTOR_RANK"] = df[group_col].map(sector_score.rank(ascending=False))
```

---

## 🔧 통합 로드맵 (일반화 우선순위)

### Phase 1: 핵심 유틸리티 일반화 (1주)
- [ ] `TickSizeProvider` trait + 거래소별 구현
- [ ] `RouteState` enum 추가
- [ ] 통화별 가격 포맷팅

### Phase 2: ML 피처 통합 (1주)
- [ ] `StructuralFeatures` 계산 로직
- [ ] 기존 ML 파이프라인에 피처 추가
- [ ] ONNX 모델 업데이트

### Phase 3: Global Rank 스코어링 (1주) ⭐ NEW
- [ ] `GlobalScorer` 구현 (가중치 + 페널티 시스템)
- [ ] `LiquidityGate` 시장별 설정
- [ ] `ERS(Entry Ready Score)` 계산 로직
- [ ] `/api/v1/ranking/global` API 엔드포인트

### Phase 4: 팩터 분석 확장 (2주)
- [ ] `FactorConfig` 시장별 설정
- [ ] 스크리닝 API에 팩터 점수 추가
- [ ] TTM Squeeze 감지 로직

### Phase 5: 외부 데이터 연동 (선택)
- [ ] Finnhub 뉴스 API 연동
- [ ] SEC EDGAR 공시 수집 (US)
- [ ] LLM 분석 서비스 (별도 마이크로서비스)

---

## 📊 API 확장 제안

### 스크리닝 API 확장
```
POST /api/v1/screening
{
    "market": "US",
    "filters": {
        "min_market_cap": 500000000,
        "min_turnover": 50000000,
        "rsi_range": [30, 70]
    },
    "factors": ["momentum", "squeeze", "volume_quality"],
    "sort_by": "factor_score",
    "limit": 50
}
```

### 응답 확장
```json
{
    "symbols": [
        {
            "ticker": "AAPL",
            "name": "Apple Inc.",
            "market": "US",
            "exchange": "NASDAQ",
            "route_state": "ARMED",
            "factor_score": 85.5,
            "factors": {
                "momentum": 90,
                "squeeze_days": 5,
                "vol_quality": 1.35,
                "range_pos": 0.85
            }
        }
    ]
}
```

---

## 📂 샘플 데이터 (data/samples/)

Python 전략 모듈의 입출력 데이터 형식을 이해하기 위한 샘플 파일입니다.

### 파일 목록

| 파일명 | 크기 | 용도 |
|--------|------|------|
| `recommend_sample.csv` | 550KB | 종목 추천 전체 데이터 |
| `reality_check_sample.csv` | 32KB | 추천 결과 검증 데이터 |
| `rank_validation_sample.csv` | 28KB | 일별 백테스트 결과 |
| `rank_validation_summary_sample.csv` | 2.7KB | 백테스트 요약 통계 |

---

### 1. recommend_sample.csv - 추천 종목 데이터

**핵심 컬럼 (~90개 중 주요 항목)**

#### 기본 정보
| 컬럼 | 설명 | 예시 |
|------|------|------|
| `종목코드` | 6자리 종목 코드 | `012450` |
| `종목명` | 종목 이름 | `한화에어로스페이스` |
| `시장` | 거래소 | `KOSPI`, `KOSDAQ` |
| `업종_대분류` | 섹터 분류 | `조선·기계·설비` |

#### 스코어링 (Global Score 시스템)
| 컬럼 | 설명 | 범위 |
|------|------|------|
| `GLOBAL_SCORE` | Global Rank 점수 | 0~100 |
| `GLOBAL_RANK` | 글로벌 순위 | 1~ |
| `ENTRY_SCORE` | 진입 점수 | 0~100 |
| `RANK_SCORE` | 기존 랭크 점수 | 0~100 |
| `ML_SCORE` | ML 예측 점수 | 0~100 |
| `NEWS_SCORE` | 뉴스 점수 | 0~100 |
| `EBS` | Entry Balance Score | 0~15 |

#### 정규화 팩터 (0~1)
| 컬럼 | 설명 |
|------|------|
| `NORM_RR` | Risk/Reward 정규화 |
| `NORM_T1` | 목표가1 여유 정규화 |
| `NORM_SL` | 손절가 여유 정규화 |
| `NORM_NEAR` | 진입가 근접 정규화 |
| `NORM_MOM` | 모멘텀 정규화 |
| `NORM_LIQ` | 유동성 정규화 |
| `NORM_TEC` | 기술균형 정규화 |

#### 매매 가격
| 컬럼 | 설명 |
|------|------|
| `추천매수가` | 진입 가격 |
| `손절가` | 손절 가격 |
| `추천매도가1` | 목표가 1 |
| `추천매도가2` | 목표가 2 |
| `RR1` | Risk/Reward Ratio |

#### 기술적 지표
| 컬럼 | 설명 |
|------|------|
| `RSI14` | RSI 14일 |
| `MFI14` | MFI 14일 |
| `MACD_Slope_PCT` | MACD 기울기 (%) |
| `BB_BW` | 볼린저 밴드 폭 |
| `TTM_SQUEEZE` | TTM Squeeze 상태 |
| `TTM_SQUEEZE_CNT` | Squeeze 지속 일수 |
| `VWAP` | 거래량 가중 평균가 |
| `HMA20` | Hull MA 20일 |

#### 상태 플래그
| 컬럼 | 설명 | 값 |
|------|------|-----|
| `ROUTE` | 매매 경로 | `WAIT`, `ATTACK`, `ARMED` 등 |
| `REGIME` | 시장 레짐 | `① 강한 상승 추세` ~ `⑤ 하락 / 약세` |
| `TRIGGER` | 트리거 신호 | `🚀급등시동`, `📦박스돌파` 등 |

#### 켈리/포지션 사이징
| 컬럼 | 설명 |
|------|------|
| `켈리_수량` | 켈리 기준 추천 수량 |
| `켈리_금액(원)` | 켈리 기준 투자 금액 |
| `추천수량` | 최종 추천 수량 |
| `추천금액(만원)` | 최종 추천 금액 |

---

### 2. reality_check_sample.csv - 추천 검증 데이터

추천 종목의 실제 성과를 검증하기 위한 데이터입니다.

**추가 검증 컬럼**
| 컬럼 | 설명 |
|------|------|
| `오늘종가` | 검증일 종가 |
| `전일추천매수가` | 추천 당시 매수가 |
| `전일→오늘_수익률%` | 실제 수익률 |
| `검증기준일` | 검증 날짜 |
| `비교대상추천일` | 원본 추천 날짜 |

---

### 3. rank_validation_sample.csv - 일별 백테스트

**컬럼 구조**
```
추천일, 비교종가일, H(영업일), METHOD, TOPK, N,
WIN_RATE_%, AVG_RET_%, MED_RET_%, HIT_2%_%, HIT_5%_%,
AVG_MDD_%, WORST_MDD_%
```

| 컬럼 | 설명 |
|------|------|
| `H(영업일)` | 보유 기간 (영업일) |
| `METHOD` | 스코어링 방법 (`ENTRY_SCORE`, `GLOBAL_SCORE` 등) |
| `TOPK` | 상위 K개 종목 |
| `WIN_RATE_%` | 승률 |
| `AVG_RET_%` | 평균 수익률 |
| `MED_RET_%` | 중앙값 수익률 |
| `HIT_2%_%` | 2% 이상 달성 비율 |
| `HIT_5%_%` | 5% 이상 달성 비율 |
| `AVG_MDD_%` | 평균 MDD |
| `WORST_MDD_%` | 최악 MDD |

---

### 4. rank_validation_summary_sample.csv - 백테스트 요약

일별 백테스트 결과를 METHOD/TOPK/보유기간별로 집계한 요약 통계입니다.

```csv
METHOD,TOPK,H(영업일),TOTAL_N,WIN_RATE_%,AVG_RET_%,...
ENTRY_SCORE,1,1,13.0,23.1,-8.4,...
ENTRY_SCORE,1,3,11.0,18.2,-11.49,...
```

---

### Rust 데이터 모델

```rust
// trader-analytics/src/models/recommendation.rs

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// 추천 종목 데이터 (recommend_sample.csv 매핑)
#[derive(Debug, Serialize, Deserialize)]
pub struct Recommendation {
    // 기본 정보
    pub ticker: String,
    pub name: String,
    pub market: Market,
    pub sector: String,

    // 가격
    pub close: Decimal,
    pub entry_price: Decimal,
    pub stop_price: Decimal,
    pub target1: Decimal,
    pub target2: Option<Decimal>,

    // 스코어링
    pub global_score: f64,
    pub global_rank: u32,
    pub entry_score: f64,
    pub ml_score: f64,
    pub ebs: u8,

    // 정규화 팩터
    pub norm_factors: NormalizedFactors,

    // 상태
    pub route: RouteState,
    pub regime: MarketRegime,
    pub trigger: Option<String>,

    // 기술적 지표
    pub technicals: TechnicalIndicators,

    // 포지션 사이징
    pub kelly_qty: u32,
    pub kelly_amount: Decimal,
    pub recommended_qty: u32,

    // 메타
    pub base_date: NaiveDate,
    pub ai_comment: Option<String>,
}

/// 정규화된 팩터 (0~1)
#[derive(Debug, Serialize, Deserialize)]
pub struct NormalizedFactors {
    pub risk_reward: f64,   // NORM_RR
    pub target_room: f64,   // NORM_T1
    pub stop_room: f64,     // NORM_SL
    pub entry_near: f64,    // NORM_NEAR
    pub momentum: f64,      // NORM_MOM
    pub liquidity: f64,     // NORM_LIQ
    pub technical: f64,     // NORM_TEC
}

/// 백테스트 검증 결과
#[derive(Debug, Serialize, Deserialize)]
pub struct ValidationResult {
    pub rec_date: NaiveDate,
    pub compare_date: NaiveDate,
    pub holding_days: u8,
    pub method: ScoringMethod,
    pub top_k: u8,
    pub sample_size: u32,
    pub win_rate: f64,
    pub avg_return: f64,
    pub median_return: f64,
    pub hit_2pct: f64,
    pub hit_5pct: f64,
    pub avg_mdd: f64,
    pub worst_mdd: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ScoringMethod {
    EntryScore,
    GlobalScore,
    RankScore,
    MlScore,
}
```

---

## 📚 참고 자료

### 거래소별 틱 사이즈
- [KRX 호가가격단위](https://www.krx.co.kr)
- [NYSE Tick Size Pilot](https://www.nyse.com/markets/nyse/trading-info)
- [LSE Tick Size](https://www.londonstockexchange.com)

### 공시 시스템
- [DART Open API](https://opendart.fss.or.kr)
- [SEC EDGAR](https://www.sec.gov/edgar)
- [Finnhub API](https://finnhub.io/docs/api)

### 기술적 분석
- [TTM Squeeze (John Carter)](https://school.stockcharts.com/doku.php?id=technical_indicators:ttm_squeeze)
- [Hull Moving Average](https://school.stockcharts.com/doku.php?id=technical_indicators:hull_moving_average)

---

## 18. Fear & Greed Index - 공포/탐욕 지수 ⭐ NEW

### 개요
RSI와 이격도(Disparity)를 결합하여 시장 심리를 0~100 점수로 평가합니다.

### 원본 코드 (all.py)
```python
def get_fear_greed_index(scored_df: pd.DataFrame):
    """KOSPI 지수 기반 공포/탐욕 계산"""
    # 1. RSI(14) 계산
    df = fdr.DataReader("KS11")
    rsi = 100 - (100 / (1 + rs))
    current_rsi = float(rsi.iloc[-1])

    # 2. 이격도 계산 (현재가 / MA20 * 100)
    ma20 = df["Close"].rolling(20).mean()
    disparity = float(df["Close"].iloc[-1] / ma20.iloc[-1] * 100)

    # 3. 종합 점수
    score = current_rsi
    if disparity > 105: score += 10   # 과열
    elif disparity < 95: score -= 10  # 침체

    score = max(0.0, min(100.0, score))

    # 4. 상태 판정
    if score >= 75: status = "매도 권장 (탐욕)"
    elif score >= 60: status = "과열 구간"
    elif score <= 25: status = "적극 매수 (공포)"
    elif score <= 40: status = "침체 구간"
    else: status = "중립 (관망)"
```

### Rust 구현
```rust
// trader-analytics/src/market/fear_greed.rs

pub struct FearGreedIndex {
    pub score: f64,           // 0~100
    pub status: MarketSentiment,
    pub rsi_component: f64,
    pub disparity_adjustment: f64,
}

pub enum MarketSentiment {
    ExtremeGreed,   // >= 75 - 매도 권장
    Greed,          // 60~75 - 과열
    Neutral,        // 40~60 - 중립
    Fear,           // 25~40 - 침체
    ExtremeFear,    // <= 25 - 적극 매수
}

impl FearGreedIndex {
    pub fn calculate(index_data: &[Candle]) -> Self {
        let rsi = calculate_rsi(&index_data, 14);
        let ma20 = calculate_sma(&index_data, 20);
        let current_price = index_data.last().unwrap().close;
        let disparity = (current_price / ma20 * 100.0) as f64;

        let mut score = rsi;
        let adjustment = if disparity > 105.0 { 10.0 }
                        else if disparity < 95.0 { -10.0 }
                        else { 0.0 };
        score = (score + adjustment).clamp(0.0, 100.0);

        Self {
            score,
            status: MarketSentiment::from_score(score),
            rsi_component: rsi,
            disparity_adjustment: adjustment,
        }
    }
}
```

---

## 19. Kelly Position Sizing - 켈리 자금 관리 ⭐ NEW

### 개요
켈리 공식을 사용하여 최적 포지션 비중을 계산합니다.

### 원본 코드 (all.py)
```python
def plot_kelly_visual(win_rate_est, reward_risk, kelly_pct):
    """켈리 베팅 비중 시각화"""
    # 켈리 공식: f* = (bp - q) / b
    # b = reward/risk, p = 승률, q = 1-p

    metrics = ['승률(Win Rate)', '손익비(Reward/Risk)', '켈리 권장 비중']
    values = [win_rate_est * 100, reward_risk * 10, kelly_pct * 100]
```

### Rust 구현
```rust
// trader-core/src/utils/kelly.rs

pub struct KellyResult {
    pub full_kelly: f64,       // 원본 켈리 비중
    pub half_kelly: f64,       // 절반 켈리 (보수적)
    pub recommended: f64,      // 권장 비중 (캡 적용)
    pub win_rate: f64,
    pub reward_risk: f64,
}

impl KellyResult {
    /// 켈리 공식: f* = (bp - q) / b
    /// b = reward/risk, p = 승률, q = 1-p
    pub fn calculate(win_rate: f64, reward_risk: f64) -> Self {
        let p = win_rate;
        let q = 1.0 - p;
        let b = reward_risk;

        let full = (b * p - q) / b;
        let half = full / 2.0;
        let recommended = half.clamp(0.0, 0.25); // 최대 25% 캡

        Self {
            full_kelly: full.max(0.0),
            half_kelly: half.max(0.0),
            recommended,
            win_rate,
            reward_risk,
        }
    }
}
```

---

## 20. 7-Factor Radar Chart - 팩터 레이더 차트 ⭐ NEW

### 개요
종목의 7개 정규화 팩터를 레이더 차트로 시각화합니다.

### 7개 팩터
| 팩터 | 컬럼명 | 의미 |
|------|--------|------|
| 모멘텀 | `NORM_MOM` | 가격 상승 추세 강도 |
| 가성비 | `NORM_RR` | Risk/Reward 비율 |
| 수익여력 | `NORM_T1` | 목표가까지 여유 |
| 안전성 | `NORM_SL` | 손절가까지 여유 |
| 타점 | `NORM_NEAR` | 진입가 근접도 |
| 유동성 | `NORM_LIQ` | 거래대금 수준 |
| 기술/세력 | `NORM_TEC` | 기술적 균형 상태 |

### 원본 코드 (all.py)
```python
def plot_radar_chart(row):
    """7-Factor 레이더 차트"""
    stats = {
        "모멘텀(MOM)": row.get("NORM_MOM") * 100,
        "가성비(RR)": row.get("NORM_RR") * 100,
        "수익여력(T1)": row.get("NORM_T1") * 100,
        "안전성(SL)": row.get("NORM_SL") * 100,
        "타점(NEAR)": row.get("NORM_NEAR") * 100,
        "유동성(LIQ)": row.get("NORM_LIQ") * 100,
        "기술/세력(TEC)": row.get("NORM_TEC") * 100,
    }
    # Scatterpolar로 렌더링
```

---

## 21. Score Waterfall - 점수 기여도 분석 ⭐ NEW

### 개요
최종 점수가 어떤 팩터에서 기여받았는지 워터폴 차트로 시각화합니다.

### 원본 코드 (all.py)
```python
def plot_score_waterfall(row):
    """종목 점수 구성 요소를 워터폴 차트로 시각화"""
    w_map = {'RR': 0.25, 'T1': 0.18, 'LIQ': 0.13, 'SL': 0.12,
             'NEAR': 0.12, 'MOM': 0.10, 'TEC': 0.10}

    contributions = {}
    contributions["가성비(RR)"] = row.get("NORM_RR") * 100 * w_map['RR']
    contributions["수익여력"] = row.get("NORM_T1") * 100 * w_map['T1']
    # ... 나머지 팩터

    # 보정치 계산 (FINAL_SCORE - 계산된 합계)
    adjustment = final_score - sum(contributions.values())
    if abs(adjustment) > 0.5:
        contributions["보정/감점"] = adjustment
```

---

## 22. Correlation Heatmap - 상관관계 분석 ⭐ NEW

### 개요
상위 종목들의 주가 움직임 상관계수를 히트맵으로 시각화하여 분산 투자를 지원합니다.

### 원본 코드 (all.py)
```python
def plot_correlation_heatmap(df_target):
    """Top 종목들의 주가 상관관계 히트맵 (최근 60일)"""
    targets = df_target.head(10)
    price_data = {}

    for code, name in zip(codes, names):
        d = get_stock_chart_data(code)
        price_data[name] = d['Close'].tail(60)

    df_prices = pd.DataFrame(price_data).dropna()
    df_corr = df_prices.corr()

    # imshow로 히트맵 렌더링 (빨강=양의상관, 파랑=음의상관)
```

### 활용
- **분산 투자**: 상관계수 낮은(파란색) 종목 조합
- **포트폴리오 리스크**: 높은 상관관계 → 집중 위험

---

## 23. Volume Profile - 매물대 분석 ⭐ NEW

### 개요
가격대별 거래량 분포를 수평 막대로 표시하여 지지/저항 구간을 식별합니다.

### 원본 코드 (all.py)
```python
def add_volume_profile(fig, df):
    """차트 우측에 매물대(Volume Profile) 추가"""
    price_min, price_max = df['Low'].min(), df['High'].max()
    bins = np.linspace(price_min, price_max, 50)

    # 가격 구간별 거래량 합산
    hist, bin_edges = np.histogram(df['Close'], bins=bins, weights=df['Volume'])

    # 가로 막대 그래프 추가
    bar_trace = go.Bar(
        y=bin_edges[:-1], x=hist, orientation='h',
        marker=dict(color='rgba(128, 128, 128, 0.15)')
    )
```

### 활용
- **지지 구간**: 매물대 두꺼운 가격대 = 강한 지지
- **저항 구간**: 이전 대량 거래 구간 돌파 시 저항

---

## 24. Opportunity Map - 기회 포착 지도 ⭐ NEW

### 개요
TOTAL_SCORE(구조 점수)와 TRIGGER_SCORE(타이밍 점수)를 X-Y 축으로 한 산점도입니다.

### 원본 코드 (all.py)
```python
def plot_opportunity_map(df):
    """기회 포착 산점도 - 우상단 = 1군 주도주"""
    fig = px.scatter(
        df,
        x="TOTAL_SCORE",      # 구조 점수 (체력)
        y="TRIGGER_SCORE",    # 타이밍 점수 (맥점)
        size="거래대금(억원)",
        color="ROUTE",
        hover_name="종목명"
    )

    # 기준선 및 강조 박스
    fig.add_hline(y=60, annotation_text="급등 임박선")
    fig.add_vline(x=70, annotation_text="구조 우량선")
    fig.add_shape(type="rect", x0=70, y0=60, x1=100, y1=100)  # Hot Zone
```

### 해석
- **우상단 (x>70, y>60)**: 대장주 후보 (구조+타이밍 모두 양호)
- **점 크기**: 거래대금 (유동성)
- **점 색상**: RouteState (ATTACK=빨강, ARMED=주황)

---

## 25. Kanban Board - 상태별 칸반 보드 ⭐ NEW

### 개요
종목을 ATTACK/ARMED/WATCH 상태별로 카드 형태로 시각화합니다.

### 원본 코드 (all.py)
```python
def render_kanban_board(df):
    """Active 종목을 상태별 카드 형태로 시각화"""
    col_attack, col_armed, col_watch = st.columns(3)

    df_attack = df[df['ROUTE'].str.contains("ATTACK|공략")]
    df_armed = df[df['ROUTE'].str.contains("ARMED|임박")]
    df_watch = df[~df.index.isin(df_attack.index.union(df_armed.index))]

    # 각 레인 렌더링
    _render_card(col_attack, "진입 (ATTACK)", df_attack, "#FF4B4B", "🚀")
    _render_card(col_armed, "준비 (ARMED)", df_armed, "#FFA726", "🔫")
    _render_card(col_watch, "관찰 (WATCH)", df_watch, "#29B6F6", "👀")
```

### 카드 내용
- 종목명, 코드
- 종합점수, 트리거점수
- 매수가, 손절가
- 손익비(RR) 프로그레스바

---

## 26. Survival Days - 생존일 추적 ⭐ NEW

### 개요
종목이 상위권에 연속으로 유지된 일수를 추적하여 "오래 살아남은 종목"을 식별합니다.

### 원본 코드 (all.py)
```python
def get_survival_days(current_codes: list, lookback: int = 15) -> dict:
    """최근 N일간 '상위권 생존 일수' 계산"""
    days_map = {code: 1 for code in current_codes}

    # 과거 파일 역추적
    files = sorted(glob.glob("recommend_*.csv"), reverse=True)[1:lookback+1]

    survivors = set(current_codes)
    for f_path in files:
        df_past = pd.read_csv(f_path)
        past_set = set(df_past["종목코드"])

        # 연속성 체크 - 한 번이라도 탈락하면 카운트 중단
        next_survivors = set()
        for code in survivors:
            if code in past_set:
                days_map[code] += 1
                next_survivors.add(code)
        survivors = next_survivors
```

### 활용
- **생존일 높음**: 꾸준한 강세 종목
- **신규 진입(1일)**: 새로운 테마주

---

## 27. AI Consensus Chart - AI vs 퀀트 합의 ⭐ NEW

### 개요
ML_SCORE(AI 예측)와 RANK_SCORE(퀀트 룰) 산점도로 "AI와 퀀트 모두가 추천하는 종목"을 식별합니다.

### 원본 코드 (all.py)
```python
def plot_ai_consensus(df):
    """AI Score vs Rule Score 산점도"""
    fig = px.scatter(
        df,
        x="RANK_SCORE",      # 퀀트(룰 기반)
        y="ML_SCORE",        # AI(ML 예측)
        color="TOTAL_SCORE",
        size="거래대금(억원)",
        hover_name="종목명"
    )

    # 기준선 (80점)
    fig.add_hline(y=80, annotation_text="AI 강력매수")
    fig.add_vline(x=80, annotation_text="퀀트 강력매수")

    # Hot Zone (우상단)
    fig.add_shape(type="rect", x0=80, y0=80, x1=100, y1=100)
```

---

## 28. Sector Visualization - 섹터 시각화 ⭐ NEW

### 28.1 Sector Treemap (섹터 트리맵)
```python
def plot_sector_treemap(df_map):
    """섹터별 거래대금 기반 트리맵"""
    fig = px.treemap(
        df_map,
        path=["업종_대분류", "종목명"],
        values="거래대금(억원)",
        color="LDY_SCORE",
        color_continuous_scale="RdYlGn"
    )
```

### 28.2 Sector Momentum Bar (섹터 모멘텀)
```python
def plot_sector_momentum_bar(scored_df):
    """섹터별 5일 평균 수익률 Top 10"""
    grp = scored_df.groupby("업종_대분류")["ret_5d_%"].mean()
    grp = grp.sort_values(ascending=False).head(10)
```

### 28.3 Regime Summary (레짐 요약)
```python
def plot_regime_summary(scored_df):
    """REGIME별 평균 성과 테이블"""
    grp = scored_df.groupby("REGIME")[["LDY_SCORE", "ret_5d_%"]].mean()
```

---

## 29. Weekly MA20 - 주봉 20선 ⭐ NEW

### 개요
일봉 차트에 주봉 20일선을 오버레이하여 대추세 방향을 표시합니다.

### 원본 코드 (all.py)
```python
# 일봉 → 주봉 리샘플링
logic_w = {'Open': 'first', 'High': 'max', 'Low': 'min', 'Close': 'last'}
df_w = df.resample('W').apply(logic_w)
df_w['WMA20'] = df_w['Close'].rolling(20).mean()

# 일봉에 주봉 20선 매핑
df['WEEKLY_MA20'] = df.index.map(
    lambda x: df_w.loc[df_w.index <= x, 'WMA20'].iloc[-1]
)
```

### 활용
- **주봉 20선 위**: 중장기 상승 추세
- **주봉 20선 아래**: 중장기 하락 추세
- **점선 스타일**: 회색 점선으로 표시

---

## 30. Dynamic Route Tagging - 분포 기반 라우트 ⭐ NEW

### 개요
데이터 분포(퍼센타일) 기반으로 동적 임계값을 계산하여 라우트를 결정합니다.

### 원본 코드 (all.py)
```python
def compute_dynamic_thresholds(df):
    """분포 기반 임계값 계산"""
    thr = {}
    thr['r5_q75'] = np.nanpercentile(df['ret_5d_%'], 75)      # 5일수익 상위25%
    thr['slope_q60'] = np.nanpercentile(df['MACD_Slope'], 60) # MACD slope 상위40%
    thr['ebs_q60'] = np.nanpercentile(df['EBS'], 60)          # EBS 상위40%
    thr['now_gap_q25'] = np.nanpercentile(df['Now%'], 25)     # 진입괴리 하위25%
    return thr

def route_tag_dynamic(row, th):
    """동적 임계값 기반 라우트 판정"""
    # TTM Squeeze
    if row.get("TTM_SQUEEZE") == 1:
        return "🔥 SQZ (폭발대기)"

    # 강한 돌파
    if (r5 >= th['r5_q75'] and slope >= th['slope_q60']
        and ebs >= th['ebs_q60'] and now_pct <= th['now_gap_q25']
        and rr1 >= 0.5):
        return "🔼 BRK (강력 돌파)"

    # Watch 영역
    if (slope > 0 and r5 > 0) or (...):
        return "🔺 Watch (상승 준비)"

    return "🔺 Watch (상승 준비)"
```

### 장점
- **시장 상황 적응**: 상승장/하락장에서 자동 기준 조정
- **상대 평가**: 절대값이 아닌 상대 순위 기반

---

## 31. DART Filter Integration - 공시 필터 ⭐ NEW

### 개요
DartAnalyzer를 사용하여 악재 공시가 있는 종목의 점수를 자동 감점합니다.

### 원본 코드 (all.py)
```python
# DART 필터 적용
dart_key = get_conf("DART_API_KEY", "")
gemini_key = get_conf("GEMINI_API_KEY", "")
analyzer = DartAnalyzer(dart_api_key=dart_key, gemini_api_key=gemini_key)
scored = analyzer.apply_dart_filter(scored)

# DART 악재 반영
if "DART_SCORE" in scored.columns:
    bad_mask = scored["DART_SCORE"] <= -4
    if bad_mask.any():
        scored.loc[bad_mask, ["FINAL_SCORE", "LDY_SCORE", "TOTAL_SCORE"]] = 0
```

### DART_SCORE 의미
- `DART_SCORE >= 0`: 중립/호재
- `DART_SCORE < 0`: 악재 (절대값이 클수록 심각)
- `DART_SCORE <= -4`: 치명적 악재 → 점수 0점 처리

---

## 32. Interactive Chart - 통합 차트 옵션 ⭐ NEW

### 개요
캔들스틱 차트에 다양한 오버레이를 선택적으로 표시합니다.

### 지원 오버레이
| 옵션 | 설명 | 기본값 |
|------|------|:------:|
| `show_bb` | 볼린저 밴드 | ✅ |
| `show_kc` | 켈트너 채널 | ❌ |
| `show_rsi` | RSI 서브차트 | ❌ |
| `show_vwap` | VWAP 라인 | ❌ |
| `show_hma` | Hull MA | ❌ |
| `show_obv` | OBV 서브차트 | ❌ |
| `show_vp` | Volume Profile | ✅ |

### 가격 라인
- **진입가**: 🚀 오렌지 점선
- **손절가**: 🛡️ 하늘색 점선
- **목표가**: 💰 초록색 점선
- **VWAP**: 🟣 마젠타 실선

### SuperTrend 표시
- **상승 추세**: 초록색 실선 (지지선)
- **하락 추세**: 빨간색 점선 (저항선)

---

## 33. Score History - 점수 히스토리 (DuckDB) ⭐ NEW

### 개요
DuckDB를 사용하여 종목의 과거 추천 내역과 점수 변화를 저장/조회합니다.

### 원본 코드 (all.py)
```python
def get_stock_history_from_db(code: str):
    """DuckDB에서 과거 추천 내역 조회"""
    db_path = "ldy_trader.db"
    conn = duckdb.connect(db_path, read_only=True)

    query = f"""
        SELECT trade_date, close_price, ldy_score, rank_score, ai_comment
        FROM daily_recommend
        WHERE code = '{code}'
        ORDER BY trade_date ASC
    """
    df = conn.execute(query).fetchdf()
    return df
```

### 시각화
```python
def plot_score_history_chart(history_df, stock_name):
    """점수(LDY, RANK)와 주가(Close) 이중축 차트"""
    fig = make_subplots(specs=[[{"secondary_y": True}]])

    # 좌측 축: 점수
    fig.add_trace(go.Scatter(x=df['trade_date'], y=df['ldy_score'], name="기초 점수"))
    fig.add_trace(go.Scatter(x=df['trade_date'], y=df['rank_score'], name="랭킹 점수"))

    # 우측 축: 주가
    fig.add_trace(go.Scatter(x=df['trade_date'], y=df['close_price'], name="주가"), secondary_y=True)
```

---

## 🔧 통합 로드맵 (일반화 우선순위) - 업데이트

### Phase 3: 대시보드 시각화 (1.5주) ⭐ NEW

| 항목 | 예상 시간 | 의존성 |
|------|----------:|--------|
| Fear & Greed Index | 4시간 | 시장 지수 데이터 |
| Kelly Position Sizing | 2시간 | 백테스트 통계 |
| 7-Factor Radar | 4시간 | NORM_* 팩터 |
| Score Waterfall | 4시간 | 가중치 시스템 |
| Correlation Heatmap | 6시간 | 주가 데이터 |
| Volume Profile | 6시간 | OHLCV 데이터 |
| Opportunity Map | 4시간 | TOTAL/TRIGGER 점수 |
| Kanban Board | 4시간 | RouteState |
| Survival Days | 4시간 | 히스토리 데이터 |
| Sector Treemap/Bar | 4시간 | 섹터 분류 |
| **총계** | **~46시간** | |
