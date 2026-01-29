# Trader Bot 배포 가이드

## 목차

1. [사전 요구사항](#사전-요구사항)
2. [환경 설정](#환경-설정)
3. [빌드 및 실행](#빌드-및-실행)
4. [프로덕션 배포](#프로덕션-배포)
5. [프로덕션 체크리스트](#프로덕션-체크리스트)

---

## 사전 요구사항

### 필수 소프트웨어

| 소프트웨어 | 최소 버전 | 확인 명령어 |
|-----------|----------|------------|
| Docker | 24.0+ | `docker --version` |
| Docker Compose | 2.20+ | `docker compose version` |
| Git | 2.30+ | `git --version` |

### 시스템 요구사항

**개발 환경:**
- CPU: 2 코어 이상
- RAM: 4GB 이상
- 디스크: 10GB 이상

**프로덕션 환경:**
- CPU: 4 코어 이상
- RAM: 8GB 이상
- 디스크: 50GB 이상 (SSD 권장)

### 네트워크 포트

| 서비스 | 포트 | 용도 |
|--------|------|------|
| Trader API | 3000 | REST API / WebSocket |
| PostgreSQL | 5432 | 데이터베이스 |
| Redis | 6379 | 캐시 |
| Prometheus | 9090 | 메트릭 수집 |
| Grafana | 3001 | 대시보드 |

---

## 환경 설정

### 1. 저장소 클론

```bash
git clone https://github.com/your-org/trader.git
cd trader
```

### 2. 환경변수 설정

```bash
# .env.example을 복사하여 .env 파일 생성
cp .env.example .env
```

### 3. 필수 환경변수 설정

`.env` 파일을 편집하여 다음 값을 설정합니다:

```bash
# 필수: JWT 시크릿 (32자 이상 랜덤 문자열)
JWT_SECRET=your-super-secret-jwt-key-change-in-production

# 필수: 암호화 키 (32바이트, Base64 인코딩)
ENCRYPTION_KEY=your-32-byte-encryption-key-here-base64

# 필수: 거래소 API 키 (Binance)
BINANCE_API_KEY=your_binance_api_key
BINANCE_API_SECRET=your_binance_api_secret

# 선택: Grafana 관리자 비밀번호
GRAFANA_PASSWORD=secure_grafana_password

# 선택: CORS 허용 도메인 (프로덕션)
CORS_ORIGINS=https://your-dashboard.com

# 선택: Rate Limit 설정 (분당 요청 수)
RATE_LIMIT_RPM=1200
```

### 4. 시크릿 생성 스크립트

```bash
# JWT 시크릿 생성
openssl rand -base64 32

# 암호화 키 생성
openssl rand -base64 32
```

---

## 빌드 및 실행

### 개발 환경

```bash
# 기본 서비스 시작 (TimescaleDB, Redis, Trader API)
docker compose up -d

# 개발 도구 포함 시작 (pgAdmin, Redis Commander)
docker compose --profile dev up -d

# 모니터링 포함 시작 (Prometheus, Grafana)
docker compose --profile monitoring up -d

# 모든 프로필 시작
docker compose --profile dev --profile monitoring up -d
```

### 서비스 상태 확인

```bash
# 모든 컨테이너 상태 확인
docker compose ps

# 헬스체크 확인
curl http://localhost:3000/health

# 상세 헬스체크
curl http://localhost:3000/health/ready
```

### 로그 확인

```bash
# 모든 서비스 로그
docker compose logs -f

# 특정 서비스 로그
docker compose logs -f trader-api

# 최근 100줄만 표시
docker compose logs --tail=100 trader-api
```

### 서비스 중지

```bash
# 서비스 중지 (데이터 유지)
docker compose down

# 서비스 중지 및 볼륨 삭제 (주의: 데이터 삭제됨)
docker compose down -v
```

---

## 프로덕션 배포

### 1. 이미지 빌드

```bash
# 프로덕션 이미지 빌드
docker build -t trader-api:latest .

# 버전 태그 추가
docker tag trader-api:latest trader-api:v1.0.0
```

### 2. 프로덕션 환경변수

프로덕션에서는 다음 환경변수를 반드시 변경하세요:

```bash
# .env.production
ENVIRONMENT=production
RUST_LOG=info,trader_api=info

# 보안
JWT_SECRET=<강력한-랜덤-문자열>
ENCRYPTION_KEY=<강력한-암호화-키>

# CORS (허용된 도메인만)
CORS_ORIGINS=https://your-dashboard.com

# 데이터베이스 (강력한 비밀번호)
DATABASE_URL=postgresql://trader:<strong-password>@timescaledb:5432/trader

# Grafana (기본 비밀번호 변경)
GRAFANA_PASSWORD=<strong-password>

# Binance (실거래 API 키)
BINANCE_TESTNET=false
BINANCE_API_KEY=<production-api-key>
BINANCE_API_SECRET=<production-api-secret>
```

### 3. 프로덕션 실행

```bash
# 환경 파일 지정하여 실행
docker compose --env-file .env.production up -d

# 모니터링 포함
docker compose --env-file .env.production --profile monitoring up -d
```

### 4. 리버스 프록시 설정 (Nginx 예시)

```nginx
upstream trader_api {
    server localhost:3000;
}

server {
    listen 443 ssl http2;
    server_name api.your-domain.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    # API 요청
    location /api {
        proxy_pass http://trader_api;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }

    # WebSocket
    location /ws {
        proxy_pass http://trader_api;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }

    # Health check (내부 접근만)
    location /health {
        allow 10.0.0.0/8;
        deny all;
        proxy_pass http://trader_api;
    }
}
```

---

## 프로덕션 체크리스트

### 보안

- [ ] JWT_SECRET 변경 (32자 이상)
- [ ] ENCRYPTION_KEY 변경
- [ ] 데이터베이스 비밀번호 변경
- [ ] Grafana 관리자 비밀번호 변경
- [ ] CORS_ORIGINS 설정 (허용된 도메인만)
- [ ] HTTPS 설정 (TLS 인증서)
- [ ] 방화벽 규칙 설정
- [ ] Rate Limiting 활성화

### 데이터베이스

- [ ] 백업 스크립트 설정
- [ ] 자동 백업 크론잡 등록
- [ ] 복구 절차 테스트
- [ ] TimescaleDB 압축 정책 설정

### 모니터링

- [ ] Prometheus 알림 규칙 설정
- [ ] Grafana 대시보드 확인
- [ ] 로그 수집 설정 (선택)
- [ ] 외부 알림 연동 (Slack, PagerDuty 등)

### 네트워크

- [ ] 내부 포트 외부 접근 차단 (5432, 6379, 9090)
- [ ] API 포트만 외부 노출 (3000)
- [ ] 로드밸런서 설정 (선택)
- [ ] CDN 설정 (선택)

### 운영

- [ ] 로그 로테이션 설정
- [ ] 디스크 모니터링
- [ ] 자동 재시작 정책 확인
- [ ] 장애 복구 절차 문서화

---

## 다음 단계

- [운영 절차](./operations.md) - 일상적인 운영 작업
- [모니터링 가이드](./monitoring.md) - 메트릭 및 대시보드
- [트러블슈팅](./troubleshooting.md) - 문제 해결 가이드
