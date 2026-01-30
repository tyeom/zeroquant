# syntax=docker/dockerfile:1.4
# =============================================================================
# Multi-stage Dockerfile for Trader API
# Optimized with cargo-chef + sccache + mold linker
# =============================================================================
#
# 빌드 최적화 전략:
# 1. cargo-chef로 외부 의존성과 소스코드 빌드 분리
# 2. sccache로 증분 빌드 캐시 (재빌드 시 50-80% 시간 단축)
# 3. mold 링커 사용 (lld보다 2-3배 빠름)
# 4. BuildKit 캐시 마운트로 영구 캐시 보존
# 5. 최소 런타임 이미지 (debian-slim)
#
# Crate 수정 빈도:
# - 안정적: trader-core, trader-risk, trader-notification, trader-cli
# - 중간:   trader-data, trader-exchange, trader-execution
# - 자주:   trader-strategy, trader-analytics, trader-api
#
# 빌드 명령어:
#   DOCKER_BUILDKIT=1 docker-compose build trader-api
#
# 캐시 확인:
#   docker exec trader-api sccache --show-stats
#
# 캐시 초기화:
#   docker builder prune --filter type=exec.cachemount
#
# =============================================================================

# -----------------------------------------------------------------------------
# Stage 1: Chef - cargo-chef + sccache + mold 설치
# -----------------------------------------------------------------------------
FROM rust:1.93-slim-bookworm AS chef

# 빌드 의존성 설치 (sccache, mold 포함)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    curl \
    mold \
    && rm -rf /var/lib/apt/lists/*

# cargo-chef 및 sccache 설치 (캐시 활용)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo install cargo-chef sccache

WORKDIR /app

# -----------------------------------------------------------------------------
# Stage 2: Planner - 의존성 레시피 생성
# -----------------------------------------------------------------------------
FROM chef AS planner

# Cargo.toml과 Cargo.lock 먼저 복사 (레시피 생성용)
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# 의존성 레시피 생성
RUN cargo chef prepare --recipe-path recipe.json

# -----------------------------------------------------------------------------
# Stage 3: Deps Builder - 외부 의존성만 빌드 (가장 안정적인 레이어)
# -----------------------------------------------------------------------------
FROM chef AS deps-builder

# 환경변수 설정 (mold 링커 + sccache + 병렬 빌드)
ENV CARGO_INCREMENTAL=0 \
    CARGO_NET_GIT_FETCH_WITH_CLI=true \
    RUSTFLAGS="-C link-arg=-fuse-ld=mold" \
    CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse \
    RUSTC_WRAPPER=/usr/local/cargo/bin/sccache \
    SCCACHE_DIR=/sccache \
    CARGO_BUILD_JOBS=8

# C++ 라이브러리 설치 (ONNX Runtime 빌드에 필요)
# mold는 이미 chef 스테이지에서 설치됨
RUN apt-get update && apt-get install -y --no-install-recommends \
    g++ \
    libstdc++-12-dev \
    && rm -rf /var/lib/apt/lists/*

# 레시피 복사 및 외부 의존성만 빌드 (이 레이어가 캐싱됨!)
COPY --from=planner /app/recipe.json recipe.json

# 의존성 빌드 - BuildKit 캐시 마운트로 캐시 영구 보존
# sccache 캐시도 별도로 마운트 (id로 명시적 식별)
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=cargo-target,target=/app/target,sharing=locked \
    --mount=type=cache,id=sccache,target=/sccache,sharing=locked \
    cargo chef cook --release --recipe-path recipe.json

# -----------------------------------------------------------------------------
# Stage 4: Source Builder - 소스 코드 빌드 (레이어 세분화)
# -----------------------------------------------------------------------------
FROM deps-builder AS builder

# === Layer 4a: 안정적인 crate들 (거의 변경 안됨) ===
# trader-core: 핵심 도메인 모델
# trader-risk: 리스크 관리 로직
# trader-notification: 알림 서비스
# trader-cli: CLI 도구
COPY crates/trader-core ./crates/trader-core
COPY crates/trader-risk ./crates/trader-risk
COPY crates/trader-notification ./crates/trader-notification
COPY crates/trader-cli ./crates/trader-cli

# === Layer 4b: 중간 빈도 crate들 (가끔 변경) ===
# trader-data: 데이터 계층
# trader-exchange: 거래소 연동
# trader-execution: 주문 실행
COPY crates/trader-data ./crates/trader-data
COPY crates/trader-exchange ./crates/trader-exchange
COPY crates/trader-execution ./crates/trader-execution

# === Layer 4c: 자주 변경되는 crate들 ===
# trader-strategy: 전략 엔진
# trader-analytics: 분석/백테스트
# trader-api: API 서버 (가장 자주 변경)
COPY crates/trader-strategy ./crates/trader-strategy
COPY crates/trader-analytics ./crates/trader-analytics
COPY crates/trader-api ./crates/trader-api

# === Workspace 설정 파일 ===
COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations

# 소스코드 빌드
# 의존성은 deps-builder에서 이미 빌드되어 캐시됨 (cargo-target 캐시)
# 소스 변경 감지를 위해 모든 trader-* 아티팩트 삭제 후 빌드
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=cargo-target,target=/app/target,sharing=locked \
    --mount=type=cache,id=sccache,target=/sccache,sharing=locked \
    find /app/target/release -name '*trader*' -type f -delete 2>/dev/null; \
    find /app/target/release/.fingerprint -name '*trader*' -type d -exec rm -rf {} + 2>/dev/null; \
    find /app/target/release/deps -name '*trader*' -type f -delete 2>/dev/null; \
    cargo build --release --bin trader-api && \
    cp /app/target/release/trader-api /app/trader-api && \
    sccache --show-stats

# -----------------------------------------------------------------------------
# Stage 5: Runtime - 최소 런타임 이미지
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# 런타임 의존성 설치 (최소한으로)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    libstdc++6 \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    # 보안을 위한 non-root 사용자 생성
    && groupadd -r trader && useradd -r -g trader trader

# 빌더에서 바이너리 복사
COPY --from=builder /app/trader-api /usr/local/bin/trader-api

# 설정 파일 복사 (별도 레이어)
COPY config ./config

# 소유권 설정 및 사용자 전환
RUN chown -R trader:trader /app
USER trader

# API 포트 노출
EXPOSE 3000

# 헬스체크 (시작 지연 포함)
HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

# 기본 환경변수
ENV RUST_LOG=info,trader_api=debug \
    API_HOST=0.0.0.0 \
    API_PORT=3000

# 애플리케이션 실행
CMD ["trader-api"]
