# =============================================================================
# Multi-stage Dockerfile for Trader API (Optimized with cargo-chef + BuildKit)
# =============================================================================
#
# 빌드 최적화 전략:
# 1. cargo-chef로 의존성과 소스코드 빌드 분리
# 2. BuildKit 캐시 마운트로 Cargo 캐시 영구 보존
# 3. 병렬 빌드 최적화
# 4. 최소 런타임 이미지
#
# 빌드 명령어:
#   DOCKER_BUILDKIT=1 docker-compose build trader-api
#
# 캐시 초기화:
#   docker builder prune --filter type=exec.cachemount
#
# =============================================================================

# syntax=docker/dockerfile:1.4

# -----------------------------------------------------------------------------
# Stage 1: Chef - cargo-chef 설치
# -----------------------------------------------------------------------------
FROM rust:1.93-slim-bookworm AS chef

# 빌드 의존성 설치 (한 번에 설치하여 레이어 최소화)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# cargo-chef 설치
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo install cargo-chef

WORKDIR /app

# -----------------------------------------------------------------------------
# Stage 2: Planner - 의존성 레시피 생성
# -----------------------------------------------------------------------------
FROM chef AS planner

# Cargo.toml과 Cargo.lock 먼저 복사 (레시피 생성용)
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# 의존성 레시피 생성 (소스코드 해시 기반)
RUN cargo chef prepare --recipe-path recipe.json

# -----------------------------------------------------------------------------
# Stage 3: Builder - 의존성 빌드 (캐싱됨) + 소스 빌드
# -----------------------------------------------------------------------------
FROM chef AS builder

# 환경변수 설정
ENV CARGO_INCREMENTAL=0 \
    CARGO_NET_GIT_FETCH_WITH_CLI=true \
    RUSTFLAGS="-C link-arg=-fuse-ld=lld" \
    CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse

# lld 링커 + C++ 라이브러리 설치 (ONNX Runtime 빌드에 필요)
RUN apt-get update && apt-get install -y --no-install-recommends \
    lld \
    g++ \
    libstdc++-12-dev \
    && rm -rf /var/lib/apt/lists/*

# 레시피 복사 및 의존성만 빌드 (이 레이어가 캐싱됨!)
COPY --from=planner /app/recipe.json recipe.json

# 의존성 빌드 - BuildKit 캐시 마운트로 Cargo 캐시 보존
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/app/target,sharing=locked \
    cargo chef cook --release --recipe-path recipe.json

# 이제 실제 소스코드 복사
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY migrations ./migrations

# 소스코드 빌드 (의존성은 이미 빌드됨, 빠름!)
# target 캐시를 공유하되, 최종 바이너리는 별도 위치에 복사
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/app/target,sharing=locked \
    cargo build --release --bin trader-api && \
    cp /app/target/release/trader-api /app/trader-api

# -----------------------------------------------------------------------------
# Stage 4: Runtime - 최소 런타임 이미지
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# 런타임 의존성 설치 (ONNX Runtime 실행에 libstdc++ 필요)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    libstdc++6 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# 보안을 위한 non-root 사용자 생성
RUN groupadd -r trader && useradd -r -g trader trader

# 빌더에서 바이너리 복사
COPY --from=builder /app/trader-api /usr/local/bin/trader-api

# 설정 파일 복사
COPY config ./config

# 소유권 설정
RUN chown -R trader:trader /app

# non-root 사용자로 전환
USER trader

# API 포트 노출
EXPOSE 3000

# 헬스체크
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

# 기본 환경변수
ENV RUST_LOG=info,trader_api=debug \
    API_HOST=0.0.0.0 \
    API_PORT=3000

# 애플리케이션 실행
CMD ["trader-api"]
