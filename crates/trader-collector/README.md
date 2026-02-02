# Trader Collector

Standalone data collector for ZeroQuant trading system.

## 📋 기능

- **심볼 동기화**: KRX, Binance, Yahoo Finance에서 종목 정보 동기화
- **OHLCV 수집**: 일봉 데이터 수집 (KRX)
- **Fundamental 수집**: 재무 지표 수집 (Yahoo Finance, 향후 구현)

## 🚀 빠른 시작

### 1. 환경변수 설정

```bash
cp .env.example .env
# .env 파일 수정 (DATABASE_URL 등)
```

### 2. 빌드

```bash
cargo build --bin trader-collector --release
```

### 3. 실행

```bash
# 심볼 동기화
./target/release/trader-collector sync-symbols

# OHLCV 수집 (모든 활성 심볼)
./target/release/trader-collector collect-ohlcv

# 특정 심볼만 수집
./target/release/trader-collector collect-ohlcv --symbols "005930,000660,035420"

# 전체 워크플로우 (심볼 동기화 → OHLCV 수집)
./target/release/trader-collector run-all

# 데몬 모드 (주기적으로 전체 워크플로우 자동 실행)
./target/release/trader-collector daemon
```

## 📊 사용 예시

### 데몬 모드 (권장)

**실시간 자동 수집**을 위한 가장 간단한 방법:

```bash
# 기본 설정 (60분 주기)
./trader-collector daemon

# 주기 변경 (환경변수)
DAEMON_INTERVAL_MINUTES=30 ./trader-collector daemon

# 백그라운드 실행
nohup ./trader-collector daemon > collector.log 2>&1 &

# systemd 서비스
sudo systemctl start trader-collector-daemon
sudo systemctl enable trader-collector-daemon
```

### Cron으로 주기적 실행

```cron
# 매일 오전 9시: 심볼 동기화
0 9 * * * cd /app && ./trader-collector sync-symbols >> /var/log/trader/sync.log 2>&1

# 매일 오후 6시: OHLCV 수집
0 18 * * * cd /app && ./trader-collector collect-ohlcv >> /var/log/trader/ohlcv.log 2>&1
```

### systemd Timer

```bash
# systemctl 파일 예시는 docs/standalone_collector_design.md 참조
sudo systemctl enable trader-collector-ohlcv.timer
sudo systemctl start trader-collector-ohlcv.timer
```

## 📖 문서

- **상세 설계**: `docs/standalone_collector_design.md`
- **빠른 시작**: `docs/collector_quick_start.md`
- **환경변수**: `docs/collector_env_example.env`

## 🔧 개발

```bash
# 테스트 실행
cargo test --bin trader-collector

# 로그 레벨 조정
./trader-collector --log-level debug collect-ohlcv
```

## ⚙️ 환경변수

| 변수 | 기본값 | 설명 |
|------|--------|------|
| `DATABASE_URL` | (필수) | PostgreSQL 연결 문자열 |
| `SYMBOL_SYNC_MIN_COUNT` | 100 | 최소 심볼 수 |
| `SYMBOL_SYNC_KRX` | true | KRX 동기화 활성화 |
| `OHLCV_BATCH_SIZE` | 50 | 배치당 심볼 수 |
| `OHLCV_REQUEST_DELAY_MS` | 500 | API 요청 간 딜레이 (밀리초) |
| `DAEMON_INTERVAL_MINUTES` | 60 | 데몬 모드 실행 주기 (분) |

전체 환경변수 목록: `.env.example` 참조

## 📝 라이선스

MIT
