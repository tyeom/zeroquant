# Trader Bot 운영 절차 가이드

## 목차

1. [일상 운영](#일상-운영)
2. [서비스 관리](#서비스-관리)
3. [로그 관리](#로그-관리)
4. [백업 및 복구](#백업-및-복구)
5. [스케일링](#스케일링)
6. [유지보수](#유지보수)

---

## 일상 운영

### 시스템 상태 확인 (매일)

```bash
# 1. 모든 컨테이너 상태 확인
docker compose ps

# 2. API 헬스체크
curl -s http://localhost:3000/health | jq

# 3. 상세 헬스체크 (컴포넌트별)
curl -s http://localhost:3000/health/ready | jq

# 4. 디스크 사용량 확인
df -h

# 5. Docker 볼륨 사용량
docker system df -v
```

### 예상 출력

```json
// /health 응답
{
  "status": "healthy",
  "version": "0.1.0"
}

// /health/ready 응답
{
  "status": "ready",
  "timestamp": "2025-01-28T10:30:00Z",
  "components": {
    "strategy_engine": "healthy",
    "risk_manager": "healthy",
    "executor": "healthy"
  }
}
```

### 성능 지표 확인

```bash
# Prometheus 메트릭 확인
curl -s http://localhost:3000/metrics | grep http_requests

# 주요 지표 확인
curl -s http://localhost:3000/metrics | grep -E "^(http_requests|websocket_connections|orders_total)"
```

---

## 서비스 관리

### 서비스 시작

```bash
# 기본 서비스 시작
docker compose up -d

# 특정 서비스만 시작
docker compose up -d trader-api

# 모니터링 포함 시작
docker compose --profile monitoring up -d

# 로그와 함께 시작 (포그라운드)
docker compose up
```

### 서비스 중지

```bash
# 모든 서비스 중지 (데이터 유지)
docker compose down

# 특정 서비스만 중지
docker compose stop trader-api

# 강제 중지 (응답 없을 때)
docker compose kill trader-api
```

### 서비스 재시작

```bash
# 단일 서비스 재시작
docker compose restart trader-api

# 모든 서비스 재시작
docker compose restart

# 무중단 재시작 (이미지 재빌드 포함)
docker compose up -d --build --force-recreate trader-api
```

### Graceful Shutdown

Trader API는 SIGTERM 신호를 받으면 graceful shutdown을 수행합니다:

1. 새로운 요청 거부
2. 진행 중인 요청 완료 대기
3. WebSocket 연결 종료
4. 리소스 정리

```bash
# Graceful shutdown (30초 대기)
docker compose stop -t 30 trader-api

# 상태 확인
docker compose logs --tail=20 trader-api
```

### 설정 변경 후 적용

```bash
# 환경변수 변경 시
docker compose up -d --force-recreate trader-api

# docker-compose.yml 변경 시
docker compose up -d

# 이미지 재빌드가 필요한 경우
docker compose build trader-api
docker compose up -d trader-api
```

---

## 로그 관리

### 로그 확인

```bash
# 실시간 로그 스트리밍
docker compose logs -f trader-api

# 최근 로그 (줄 수 지정)
docker compose logs --tail=100 trader-api

# 특정 시간 이후 로그
docker compose logs --since="2025-01-28T10:00:00" trader-api

# 모든 서비스 로그
docker compose logs -f

# 로그 파일로 저장
docker compose logs trader-api > logs/trader-api-$(date +%Y%m%d).log
```

### 로그 레벨 변경

환경변수 `RUST_LOG`를 수정하여 로그 레벨을 조정합니다:

```bash
# .env 파일 수정
RUST_LOG=debug,trader_api=trace

# 변경 적용
docker compose up -d --force-recreate trader-api
```

**로그 레벨:**
- `error`: 오류만
- `warn`: 경고 이상
- `info`: 일반 정보 (기본값)
- `debug`: 디버그 정보
- `trace`: 상세 추적

### 로그 로테이션 설정

`docker-compose.yml`에 로깅 옵션 추가:

```yaml
trader-api:
  logging:
    driver: "json-file"
    options:
      max-size: "100m"
      max-file: "5"
```

### 로그 분석 명령어

```bash
# 에러 로그만 필터링
docker compose logs trader-api | grep -i error

# 특정 IP의 요청 추적
docker compose logs trader-api | grep "192.168.1.100"

# Rate limit 초과 확인
docker compose logs trader-api | grep "Rate limit exceeded"

# Circuit breaker 상태 변경 확인
docker compose logs trader-api | grep -i "circuit"
```

---

## 백업 및 복구

### 데이터베이스 백업

```bash
# 수동 백업 생성
docker compose exec timescaledb pg_dump -U trader -d trader > backups/trader_$(date +%Y%m%d_%H%M%S).sql

# 압축 백업
docker compose exec timescaledb pg_dump -U trader -d trader | gzip > backups/trader_$(date +%Y%m%d_%H%M%S).sql.gz

# 특정 테이블만 백업
docker compose exec timescaledb pg_dump -U trader -d trader -t orders -t positions > backups/orders_positions_$(date +%Y%m%d).sql
```

### 데이터베이스 복구

```bash
# 서비스 중지 (데이터 무결성을 위해)
docker compose stop trader-api

# 백업 복원
cat backups/trader_20250128_100000.sql | docker compose exec -T timescaledb psql -U trader -d trader

# 압축 백업 복원
gunzip -c backups/trader_20250128_100000.sql.gz | docker compose exec -T timescaledb psql -U trader -d trader

# 서비스 재시작
docker compose start trader-api
```

### Redis 백업

```bash
# RDB 스냅샷 생성
docker compose exec redis redis-cli BGSAVE

# 스냅샷 파일 복사
docker compose cp redis:/data/dump.rdb backups/redis_$(date +%Y%m%d).rdb
```

### Redis 복구

```bash
# 서비스 중지
docker compose stop redis

# 스냅샷 복원
docker compose cp backups/redis_20250128.rdb redis:/data/dump.rdb

# 서비스 시작
docker compose start redis
```

### 자동 백업 스크립트

`scripts/backup.sh`:

```bash
#!/bin/bash
set -e

BACKUP_DIR="/var/backups/trader"
DATE=$(date +%Y%m%d_%H%M%S)

# 백업 디렉토리 생성
mkdir -p $BACKUP_DIR

# PostgreSQL 백업
docker compose exec -T timescaledb pg_dump -U trader -d trader | gzip > $BACKUP_DIR/db_$DATE.sql.gz

# Redis 백업
docker compose exec redis redis-cli BGSAVE
sleep 5
docker compose cp redis:/data/dump.rdb $BACKUP_DIR/redis_$DATE.rdb

# 30일 이상된 백업 삭제
find $BACKUP_DIR -name "*.sql.gz" -mtime +30 -delete
find $BACKUP_DIR -name "*.rdb" -mtime +30 -delete

echo "Backup completed: $DATE"
```

### 크론잡 등록

```bash
# 매일 새벽 3시 백업
0 3 * * * /opt/trader/scripts/backup.sh >> /var/log/trader-backup.log 2>&1
```

---

## 스케일링

### 수직 스케일링 (리소스 증가)

`docker-compose.yml`에서 리소스 제한 설정:

```yaml
trader-api:
  deploy:
    resources:
      limits:
        cpus: '2'
        memory: 2G
      reservations:
        cpus: '1'
        memory: 1G
```

### 수평 스케일링 (복제)

> 참고: 현재 구조에서 trader-api는 상태를 가지고 있어 복제 시 주의가 필요합니다.

```bash
# 복제본 수 조정 (로드밸런서 필요)
docker compose up -d --scale trader-api=3
```

### Redis 클러스터

고가용성이 필요한 경우 Redis Sentinel 또는 Redis Cluster를 고려하세요.

### 데이터베이스 읽기 복제본

대규모 읽기 부하가 있는 경우:

```yaml
# docker-compose.prod.yml
timescaledb-replica:
  image: timescale/timescaledb:latest-pg15
  environment:
    POSTGRES_USER: trader
    POSTGRES_PASSWORD: trader_secret
  command: >
    postgres
    -c hot_standby=on
```

---

## 유지보수

### 정기 점검 (주간)

```bash
# 1. Docker 이미지 업데이트 확인
docker compose pull

# 2. 사용하지 않는 이미지 정리
docker image prune -f

# 3. 볼륨 정리 (주의: 사용 중인 볼륨 확인)
docker volume prune -f

# 4. 네트워크 정리
docker network prune -f

# 5. 시스템 전체 정리
docker system prune -f
```

### 버전 업그레이드

```bash
# 1. 현재 상태 백업
./scripts/backup.sh

# 2. 새 버전 빌드
git pull origin main
docker compose build trader-api

# 3. 롤링 업데이트
docker compose up -d trader-api

# 4. 헬스체크 확인
curl http://localhost:3000/health

# 5. 문제 발생 시 롤백
docker compose down
docker tag trader-api:previous trader-api:latest
docker compose up -d
```

### TimescaleDB 유지보수

```bash
# 압축 정책 확인
docker compose exec timescaledb psql -U trader -d trader -c "SELECT * FROM timescaledb_information.jobs;"

# 수동 압축 실행
docker compose exec timescaledb psql -U trader -d trader -c "CALL run_job(1000);"

# 오래된 데이터 삭제 (90일 이상)
docker compose exec timescaledb psql -U trader -d trader -c "SELECT drop_chunks('klines', interval '90 days');"
```

### 보안 업데이트

```bash
# 1. 베이스 이미지 업데이트
docker compose pull

# 2. 이미지 재빌드
docker compose build --no-cache

# 3. 서비스 재시작
docker compose up -d
```

---

## 참고 문서

- [배포 가이드](./deployment.md)
- [모니터링 가이드](./monitoring.md)
- [트러블슈팅](./troubleshooting.md)
