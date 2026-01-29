# Trader Bot 모니터링 가이드

## 목차

1. [개요](#개요)
2. [Prometheus 메트릭](#prometheus-메트릭)
3. [Grafana 대시보드](#grafana-대시보드)
4. [알림 설정](#알림-설정)
5. [커스텀 모니터링](#커스텀-모니터링)

---

## 개요

### 모니터링 아키텍처

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│ Trader API  │────▶│  Prometheus │────▶│   Grafana   │
│  /metrics   │     │    :9090    │     │    :3001    │
└─────────────┘     └─────────────┘     └─────────────┘
                           │
                           ▼
                    ┌─────────────┐
                    │ Alertmanager│
                    │   (선택)    │
                    └─────────────┘
```

### 모니터링 서비스 시작

```bash
# 모니터링 프로필로 시작
docker compose --profile monitoring up -d

# 서비스 접속
# - Prometheus: http://localhost:9090
# - Grafana: http://localhost:3001 (admin/admin)
```

---

## Prometheus 메트릭

### 메트릭 엔드포인트

```bash
# 메트릭 확인
curl http://localhost:3000/metrics

# 특정 메트릭 필터링
curl -s http://localhost:3000/metrics | grep http_requests
```

### HTTP 요청 메트릭

| 메트릭 | 타입 | 레이블 | 설명 |
|--------|------|--------|------|
| `http_requests_total` | Counter | method, path, status | 총 HTTP 요청 수 |
| `http_request_duration_seconds` | Histogram | method, path | 요청 처리 시간 |

**PromQL 예시:**

```promql
# 분당 요청 수
rate(http_requests_total[1m])

# 엔드포인트별 요청 수
sum by (path) (rate(http_requests_total[5m]))

# 에러율 (4xx + 5xx)
sum(rate(http_requests_total{status=~"4.*|5.*"}[5m])) / sum(rate(http_requests_total[5m])) * 100

# P95 레이턴시
histogram_quantile(0.95, sum(rate(http_request_duration_seconds_bucket[5m])) by (le, path))

# P99 레이턴시
histogram_quantile(0.99, sum(rate(http_request_duration_seconds_bucket[5m])) by (le))
```

### WebSocket 메트릭

| 메트릭 | 타입 | 설명 |
|--------|------|------|
| `websocket_connections_total` | Gauge | 현재 활성 WebSocket 연결 수 |

**PromQL 예시:**

```promql
# 현재 연결 수
websocket_connections_total

# 연결 수 변화 추이
changes(websocket_connections_total[1h])
```

### 거래 메트릭

| 메트릭 | 타입 | 레이블 | 설명 |
|--------|------|--------|------|
| `orders_total` | Counter | side, status, exchange | 총 주문 수 |
| `positions_open` | Gauge | symbol | 오픈 포지션 수 |
| `realized_pnl_total` | Counter | symbol | 실현 손익 |

**PromQL 예시:**

```promql
# 24시간 주문 수
increase(orders_total[24h])

# 매수/매도 비율
sum(rate(orders_total{side="buy"}[1h])) / sum(rate(orders_total[1h])) * 100

# 주문 성공률
sum(rate(orders_total{status="filled"}[1h])) / sum(rate(orders_total[1h])) * 100

# 심볼별 오픈 포지션
positions_open

# 누적 실현 손익
realized_pnl_total
```

### Rate Limiting 메트릭

| 메트릭 | 타입 | 레이블 | 설명 |
|--------|------|--------|------|
| `rate_limit_requests_total` | Counter | status | Rate limit 상태별 요청 수 |

**PromQL 예시:**

```promql
# Rate limit 초과 비율
sum(rate(rate_limit_requests_total{status="limited"}[5m])) / sum(rate(rate_limit_requests_total[5m])) * 100

# 허용된 요청 수
rate(rate_limit_requests_total{status="allowed"}[1m])
```

---

## Grafana 대시보드

### 기본 접속 정보

- **URL**: http://localhost:3001
- **사용자명**: admin
- **비밀번호**: admin (또는 GRAFANA_PASSWORD 환경변수)

### 제공되는 대시보드

#### 1. Trading Overview (거래 개요)

**위치**: Dashboards → Trading Overview

**패널 목록:**
- Orders (24h): 24시간 총 주문 수
- Open Positions: 현재 오픈 포지션 수
- Realized Profit: 누적 실현 손익
- WebSocket Connections: 활성 WebSocket 연결 수
- Orders by Side: 매수/매도 비율 파이 차트
- Orders by Status: 주문 상태별 분포
- PnL Over Time: 시간별 손익 추이
- Open Positions by Symbol: 심볼별 포지션 분포
- Orders by Exchange: 거래소별 주문 분포
- WebSocket Connections Over Time: 연결 수 추이

#### 2. API Performance (API 성능)

**위치**: Dashboards → API Performance

**패널 목록:**
- API Status: API 서버 상태 (UP/DOWN)
- Request Rate: 분당 요청 수
- P50 Latency: 중간값 응답 시간
- P95 Latency: 95 백분위 응답 시간
- P99 Latency: 99 백분위 응답 시간
- Error Rate: 에러 비율 (%)
- Latency by Endpoint: 엔드포인트별 레이턴시
- Request Rate by Endpoint: 엔드포인트별 요청률
- Request Rate by Method: HTTP 메서드별 요청률
- Response Status Codes: 응답 코드 분포
- P95 Latency Bar Gauge: 레이턴시 게이지

### 대시보드 커스터마이징

#### 새 패널 추가

1. 대시보드 편집 모드 진입 (연필 아이콘)
2. "Add" → "Visualization" 클릭
3. 데이터 소스 선택: Prometheus
4. PromQL 쿼리 입력
5. 시각화 유형 선택 (Graph, Stat, Gauge 등)
6. 저장

#### 패널 예시: 분당 요청 수

```yaml
Title: Requests per Minute
Query: rate(http_requests_total[1m]) * 60
Visualization: Stat
Unit: requests/min
```

#### 패널 예시: 에러율 게이지

```yaml
Title: Error Rate
Query: sum(rate(http_requests_total{status=~"5.*"}[5m])) / sum(rate(http_requests_total[5m])) * 100
Visualization: Gauge
Unit: percent (0-100)
Thresholds:
  - 0-1: green
  - 1-5: yellow
  - 5+: red
```

### 대시보드 내보내기/가져오기

```bash
# 대시보드 JSON 내보내기
# Grafana UI: Dashboard → Share → Export → Save to file

# 대시보드 파일 위치
ls monitoring/grafana/dashboards/

# 프로비저닝으로 자동 로드됨
```

---

## 알림 설정

### Alertmanager 설정

`monitoring/prometheus/alertmanager.yml`:

```yaml
global:
  smtp_smarthost: 'smtp.gmail.com:587'
  smtp_from: 'alerts@your-domain.com'
  smtp_auth_username: 'your-email@gmail.com'
  smtp_auth_password: 'app-password'

route:
  group_by: ['alertname']
  group_wait: 30s
  group_interval: 5m
  repeat_interval: 4h
  receiver: 'email-notifications'

receivers:
  - name: 'email-notifications'
    email_configs:
      - to: 'admin@your-domain.com'
        send_resolved: true
```

### Prometheus 알림 규칙

`monitoring/prometheus/alerts/trader.yml`:

```yaml
groups:
  - name: trader-alerts
    rules:
      # API 다운 알림
      - alert: TraderAPIDown
        expr: up{job="trader-api"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Trader API is down"
          description: "Trader API has been down for more than 1 minute."

      # 높은 에러율
      - alert: HighErrorRate
        expr: sum(rate(http_requests_total{status=~"5.*"}[5m])) / sum(rate(http_requests_total[5m])) > 0.05
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High error rate detected"
          description: "Error rate is above 5% for the last 5 minutes."

      # 높은 레이턴시
      - alert: HighLatency
        expr: histogram_quantile(0.95, sum(rate(http_request_duration_seconds_bucket[5m])) by (le)) > 1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High latency detected"
          description: "P95 latency is above 1 second."

      # Rate limit 초과 빈번
      - alert: RateLimitExceeded
        expr: rate(rate_limit_requests_total{status="limited"}[5m]) > 10
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Rate limiting is frequently triggered"
          description: "More than 10 requests per second are being rate limited."

      # WebSocket 연결 급감
      - alert: WebSocketConnectionsDrop
        expr: decrease(websocket_connections_total[5m]) > 10
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "WebSocket connections dropping"
          description: "WebSocket connections dropped by more than 10 in 5 minutes."

      # 일일 손실 한도 경고
      - alert: DailyLossLimit
        expr: realized_pnl_total < -1000
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Daily loss limit approaching"
          description: "Realized PnL is below -$1000."
```

### Slack 알림 설정

```yaml
receivers:
  - name: 'slack-notifications'
    slack_configs:
      - api_url: 'https://hooks.slack.com/services/YOUR/SLACK/WEBHOOK'
        channel: '#trading-alerts'
        send_resolved: true
        title: '{{ .Status | toUpper }}: {{ .CommonAnnotations.summary }}'
        text: '{{ .CommonAnnotations.description }}'
```

### Grafana 알림 설정

1. Grafana UI → Alerting → Alert rules
2. "New alert rule" 클릭
3. 조건 설정:
   - Query: PromQL 쿼리 입력
   - Condition: 임계값 설정
4. 알림 채널 선택
5. 저장

---

## 커스텀 모니터링

### 새 메트릭 추가

`crates/trader-api/src/metrics.rs`에 메트릭 추가:

```rust
use metrics::{counter, gauge, histogram};

// 카운터 (증가만 가능)
counter!("custom_events_total", "type" => event_type).increment(1);

// 게이지 (증감 가능)
gauge!("custom_queue_size").set(queue_size as f64);

// 히스토그램 (분포 측정)
histogram!("custom_processing_time_seconds").record(duration.as_secs_f64());
```

### Prometheus 스크랩 설정 추가

`monitoring/prometheus/prometheus.yml`:

```yaml
scrape_configs:
  # 새 서비스 추가
  - job_name: 'my-new-service'
    static_configs:
      - targets: ['my-service:8080']
    metrics_path: /metrics
    scrape_interval: 15s
```

### 외부 모니터링 서비스 연동

#### Datadog 연동

```yaml
# docker-compose.yml에 추가
datadog-agent:
  image: datadog/agent:latest
  environment:
    - DD_API_KEY=${DD_API_KEY}
    - DD_SITE=datadoghq.com
    - DD_DOGSTATSD_NON_LOCAL_TRAFFIC=true
  volumes:
    - /var/run/docker.sock:/var/run/docker.sock:ro
```

#### CloudWatch 연동

AWS CloudWatch에 메트릭을 푸시하려면 Prometheus Remote Write를 설정합니다.

---

## 참고 문서

- [배포 가이드](./deployment.md)
- [운영 절차](./operations.md)
- [트러블슈팅](./troubleshooting.md)
- [Prometheus 문서](https://prometheus.io/docs/)
- [Grafana 문서](https://grafana.com/docs/)
