# Reality Check 추천 검증 시스템 사용 가이드

> **목적**: 전일 추천 종목의 익일 실제 성과를 자동으로 검증하여 전략 신뢰도 측정

---

## 📋 시스템 개요

Reality Check 시스템은 다음 두 단계로 작동합니다:

1. **장 마감 후**: 오늘의 추천 종목 가격 스냅샷 저장
2. **익일 장 마감 후**: 전일 추천 종목의 실제 성과 계산

### 데이터 흐름

```
[스크리닝 결과]
    → [price_snapshot 저장]
    → [익일 장 마감]
    → [reality_check 계산]
    → [통계 집계]
```

---

## 🗄️ 데이터베이스 마이그레이션

### 마이그레이션 실행

```bash
# Podman 컨테이너 내부에서 실행
podman exec -it trader-timescaledb bash
psql -U trader -d trader -f /path/to/migrations/026_reality_check_system.sql

# 또는 SQLx CLI 사용 (권장)
sqlx migrate run --database-url "postgres://trader:trader_secret@localhost:5432/trader"
```

### 생성되는 테이블

| 테이블 | 타입 | 용도 |
|--------|------|------|
| `price_snapshot` | Hypertable | 추천 종목 가격 스냅샷 저장 |
| `reality_check` | Hypertable | 실제 성과 검증 결과 |

### 생성되는 뷰

| 뷰 | 용도 |
|----|------|
| `v_reality_check_daily_stats` | 일별 승률, 평균 수익률 |
| `v_reality_check_source_stats` | 추천 소스별 성과 비교 |
| `v_reality_check_rank_stats` | 추천 순위별 성과 (Top 10) |
| `v_reality_check_recent_trend` | 최근 30일 추이 |

---

## 🔧 API 사용법

### 1. 스냅샷 저장 (매일 장 마감 후)

**엔드포인트**: `POST /api/v1/reality-check/snapshot`

**요청 예시**:
```json
{
  "snapshot_date": "2025-02-03",
  "snapshots": [
    {
      "symbol": "005930",
      "close_price": 70000,
      "volume": 10000000,
      "recommend_source": "screening_momentum",
      "recommend_rank": 1,
      "recommend_score": 95.5,
      "expected_return": 5.0,
      "expected_holding_days": 3,
      "market": "KR",
      "sector": "IT"
    },
    {
      "symbol": "000660",
      "close_price": 130000,
      "volume": 5000000,
      "recommend_source": "screening_momentum",
      "recommend_rank": 2,
      "recommend_score": 92.3,
      "expected_return": 4.5,
      "market": "KR",
      "sector": "반도체"
    }
  ]
}
```

**응답**:
```json
{
  "success": true,
  "snapshot_date": "2025-02-03",
  "saved_count": 2
}
```

### 2. Reality Check 계산 (익일 장 마감 후)

**엔드포인트**: `POST /api/v1/reality-check/calculate`

**요청 예시**:
```json
{
  "recommend_date": "2025-02-03",
  "check_date": "2025-02-04"
}
```

**응답**:
```json
{
  "success": true,
  "recommend_date": "2025-02-03",
  "check_date": "2025-02-04",
  "processed_count": 2,
  "results": [
    {
      "symbol": "005930",
      "actual_return": 2.5,
      "is_profitable": true,
      "processed_count": 2
    },
    {
      "symbol": "000660",
      "actual_return": -1.2,
      "is_profitable": false,
      "processed_count": 2
    }
  ]
}
```

### 3. 통계 조회

**엔드포인트**: `GET /api/v1/reality-check/stats?limit=30`

**응답**:
```json
{
  "daily": [
    {
      "check_date": "2025-02-04",
      "total_count": 20,
      "win_count": 12,
      "win_rate": 60.00,
      "avg_return": 1.25,
      "avg_win_return": 3.50,
      "avg_loss_return": -2.10,
      "max_return": 8.50,
      "min_return": -5.20,
      "return_stddev": 3.45
    }
  ],
  "source": [
    {
      "recommend_source": "screening_momentum",
      "total_count": 100,
      "win_count": 62,
      "win_rate": 62.00,
      "avg_return": 1.85,
      "avg_win_return": 4.20,
      "avg_loss_return": -2.50
    }
  ],
  "rank": [
    {
      "recommend_rank": 1,
      "total_count": 30,
      "win_rate": 70.00,
      "avg_return": 2.50
    }
  ]
}
```

### 4. 검증 결과 조회

**엔드포인트**: `GET /api/v1/reality-check/results?start_date=2025-01-01&end_date=2025-02-03&recommend_source=screening_momentum`

**응답**:
```json
{
  "total": 50,
  "results": [
    {
      "check_date": "2025-02-04",
      "recommend_date": "2025-02-03",
      "symbol": "005930",
      "recommend_source": "screening_momentum",
      "recommend_rank": 1,
      "recommend_score": 95.5,
      "entry_price": 70000,
      "exit_price": 71750,
      "actual_return": 2.50,
      "is_profitable": true,
      "entry_volume": 10000000,
      "exit_volume": 12000000,
      "volume_change": 20.00,
      "expected_return": 5.0,
      "return_error": -2.50,
      "market": "KR",
      "sector": "IT",
      "created_at": "2025-02-04T15:30:00Z"
    }
  ]
}
```

---

## 🤖 자동화 워크플로우

### 일일 배치 작업 (추천)

```bash
#!/bin/bash
# save_reality_check.sh

TODAY=$(date +%Y-%m-%d)
YESTERDAY=$(date -d "yesterday" +%Y-%m-%d)

# 1. 오늘 추천 종목 스냅샷 저장
curl -X POST http://localhost:3000/api/v1/reality-check/snapshot \
  -H "Content-Type: application/json" \
  -d @today_recommendations.json

# 2. 전일 추천 종목 성과 계산
curl -X POST http://localhost:3000/api/v1/reality-check/calculate \
  -H "Content-Type: application/json" \
  -d "{
    \"recommend_date\": \"$YESTERDAY\",
    \"check_date\": \"$TODAY\"
  }"
```

### Cron 설정

```cron
# 매일 15:35 (장 마감 후 5분)
35 15 * * 1-5 /home/trader/scripts/save_reality_check.sh
```

---

## 📊 Repository 직접 사용 (Rust)

### 스냅샷 저장

```rust
use trader_api::repository::{RealityCheckRepository, SnapshotInput};
use chrono::Utc;
use rust_decimal_macros::dec;

async fn save_today_snapshot(pool: &PgPool) -> Result<(), sqlx::Error> {
    let today = Utc::now().naive_utc().date();

    let snapshots = vec![
        SnapshotInput {
            symbol: "005930".to_string(),
            close_price: dec!(70000),
            volume: Some(10000000),
            recommend_source: "screening_momentum".to_string(),
            recommend_rank: Some(1),
            recommend_score: Some(dec!(95.5)),
            expected_return: Some(dec!(5.0)),
            expected_holding_days: Some(3),
            market: Some("KR".to_string()),
            sector: Some("IT".to_string()),
        },
    ];

    let saved_count = RealityCheckRepository::save_snapshots_batch(
        pool,
        today,
        &snapshots,
    ).await?;

    println!("Saved {} snapshots", saved_count);
    Ok(())
}
```

### Reality Check 계산

```rust
use chrono::Duration;

async fn calculate_yesterday_performance(pool: &PgPool) -> Result<(), sqlx::Error> {
    let today = Utc::now().naive_utc().date();
    let yesterday = today - Duration::days(1);

    let results = RealityCheckRepository::calculate_reality_check(
        pool,
        yesterday,
        today,
    ).await?;

    println!("Calculated {} reality checks", results.len());

    for result in results {
        println!(
            "{}: {}% ({})",
            result.symbol,
            result.actual_return,
            if result.is_profitable { "WIN" } else { "LOSS" }
        );
    }

    Ok(())
}
```

### 통계 조회

```rust
async fn print_stats(pool: &PgPool) -> Result<(), sqlx::Error> {
    // 일별 통계 (최근 7일)
    let daily_stats = RealityCheckRepository::get_daily_stats(pool, 7).await?;
    println!("=== 일별 통계 (최근 7일) ===");
    for stat in daily_stats {
        println!(
            "{}: 승률 {}%, 평균 수익률 {}%",
            stat.check_date, stat.win_rate, stat.avg_return
        );
    }

    // 소스별 통계
    let source_stats = RealityCheckRepository::get_source_stats(pool).await?;
    println!("\n=== 추천 소스별 통계 ===");
    for stat in source_stats {
        println!(
            "{}: 승률 {}%, 평균 수익률 {}%",
            stat.recommend_source, stat.win_rate, stat.avg_return
        );
    }

    // 랭크별 통계 (Top 10)
    let rank_stats = RealityCheckRepository::get_rank_stats(pool).await?;
    println!("\n=== 순위별 통계 ===");
    for stat in rank_stats {
        println!(
            "Rank {}: 승률 {}%, 평균 수익률 {}%",
            stat.recommend_rank, stat.win_rate, stat.avg_return
        );
    }

    Ok(())
}
```

---

## 🔍 직접 SQL 쿼리

### 최근 성과 조회

```sql
-- 최근 7일 일별 통계
SELECT * FROM v_reality_check_daily_stats LIMIT 7;

-- 특정 추천 소스의 성과
SELECT * FROM reality_check
WHERE recommend_source = 'screening_momentum'
ORDER BY check_date DESC
LIMIT 100;

-- Top 10 추천의 성과
SELECT
    recommend_rank,
    COUNT(*) as count,
    ROUND(AVG(actual_return), 2) as avg_return,
    ROUND(COUNT(*) FILTER (WHERE is_profitable)::NUMERIC / COUNT(*) * 100, 2) as win_rate
FROM reality_check
WHERE recommend_rank <= 10
GROUP BY recommend_rank
ORDER BY recommend_rank;
```

### 스냅샷 조회

```sql
-- 오늘의 추천 종목 스냅샷
SELECT * FROM price_snapshot
WHERE snapshot_date = CURRENT_DATE
ORDER BY recommend_rank;

-- 특정 종목의 스냅샷 히스토리
SELECT * FROM price_snapshot
WHERE symbol = '005930'
ORDER BY snapshot_date DESC
LIMIT 30;
```

### 수동 Reality Check 계산

```sql
-- 전일 추천 종목의 금일 성과 계산
SELECT * FROM calculate_reality_check(
    CURRENT_DATE - INTERVAL '1 day',
    CURRENT_DATE
);
```

---

## 📈 활용 사례

### 1. 전략 신뢰도 측정

```sql
-- 각 추천 소스의 신뢰도 비교
SELECT
    recommend_source,
    COUNT(*) as total,
    ROUND(AVG(actual_return), 2) as avg_return,
    ROUND(COUNT(*) FILTER (WHERE is_profitable)::NUMERIC / COUNT(*) * 100, 2) as win_rate
FROM reality_check
WHERE check_date >= CURRENT_DATE - INTERVAL '30 days'
GROUP BY recommend_source
ORDER BY avg_return DESC;
```

### 2. 백테스트 vs 실거래 괴리 분석

```sql
-- 예상 수익률 vs 실제 수익률 비교
SELECT
    recommend_source,
    ROUND(AVG(expected_return), 2) as avg_expected,
    ROUND(AVG(actual_return), 2) as avg_actual,
    ROUND(AVG(return_error), 2) as avg_error
FROM reality_check
WHERE expected_return IS NOT NULL
GROUP BY recommend_source;
```

### 3. 파라미터 튜닝 피드백

```sql
-- 추천 점수 구간별 성과
SELECT
    CASE
        WHEN recommend_score >= 90 THEN '90-100'
        WHEN recommend_score >= 80 THEN '80-89'
        WHEN recommend_score >= 70 THEN '70-79'
        ELSE '< 70'
    END as score_range,
    COUNT(*) as count,
    ROUND(AVG(actual_return), 2) as avg_return,
    ROUND(COUNT(*) FILTER (WHERE is_profitable)::NUMERIC / COUNT(*) * 100, 2) as win_rate
FROM reality_check
WHERE recommend_score IS NOT NULL
GROUP BY score_range
ORDER BY score_range DESC;
```

---

## ⚠️ 주의사항

1. **TimescaleDB 필수**: `price_snapshot`과 `reality_check`는 TimescaleDB hypertable입니다.
2. **mv_latest_prices 의존성**: Reality Check 계산은 `mv_latest_prices` 뷰에 의존합니다.
3. **데이터 갱신**: `mv_latest_prices`는 새 데이터 입력 후 `REFRESH MATERIALIZED VIEW CONCURRENTLY mv_latest_prices` 실행 필요.
4. **타임존**: 모든 날짜는 UTC 기준으로 저장됩니다.
5. **거래일 기준**: 주말/공휴일은 자동으로 제외되지 않으므로 배치 작업에서 처리 필요.

---

## 🔧 트러블슈팅

### 1. Reality Check 계산 결과가 없음

**원인**: `mv_latest_prices`가 갱신되지 않았거나, 스냅샷이 저장되지 않음

**해결**:
```sql
-- 1. mv_latest_prices 갱신 확인
SELECT COUNT(*) FROM mv_latest_prices
WHERE open_time::DATE = CURRENT_DATE;

-- 2. 스냅샷 존재 확인
SELECT COUNT(*) FROM price_snapshot
WHERE snapshot_date = CURRENT_DATE - INTERVAL '1 day';

-- 3. 수동 갱신
REFRESH MATERIALIZED VIEW CONCURRENTLY mv_latest_prices;
```

### 2. 타임존 문제

**증상**: 계산 시점이 맞지 않음

**해결**:
```rust
// KST → UTC 변환
use chrono::FixedOffset;

let kst_offset = FixedOffset::east_opt(9 * 3600).unwrap();
let kst_now = Utc::now().with_timezone(&kst_offset);
let utc_date = kst_now.naive_utc().date();
```

### 3. 성능 최적화

**문제**: 대량 스냅샷 저장 시 느림

**해결**:
```rust
// 배치 크기 조절 (1000개씩)
for chunk in snapshots.chunks(1000) {
    RealityCheckRepository::save_snapshots_batch(pool, today, chunk).await?;
}
```

---

## 📝 향후 개선 계획

- [ ] 여러 보유 기간 지원 (1일, 3일, 5일, 10일)
- [ ] 섹터별/시장별 성과 비교
- [ ] 예측 모델 정확도 추적 (ML 모델 평가)
- [ ] Grafana 대시보드 연동
- [ ] 자동 알림 (승률 급락 시 텔레그램 알림)

---

## 📚 참고 문서

- [TODO.md](./todo.md) - Phase 1-B.8 상세 요구사항
- [TimescaleDB 공식 문서](https://docs.timescale.com/)
- [SQLx 마이그레이션 가이드](https://github.com/launchbadge/sqlx/tree/main/sqlx-cli)
