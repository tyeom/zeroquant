-- =====================================================
-- 01_foundation.sql
-- ZeroQuant 트레이딩 시스템 기본 스키마
-- =====================================================
--
-- 원본 마이그레이션: 001_initial_schema.sql
--
-- 포함 내용:
-- - PostgreSQL Extensions (TimescaleDB, UUID)
-- - 기본 ENUM 타입 정의
-- - 핵심 테이블: symbols, users, api_keys, strategies
-- - 시계열 데이터: klines, trade_ticks (Hypertables)
-- - 거래 추적: orders, trades, positions, signals
-- - 성과 분석: performance_snapshots
-- - 감사 로그: audit_logs
-- - 자동 타임스탬프 업데이트 함수 및 트리거
-- - 데이터 보존 정책
--
-- =====================================================

-- =====================================================
-- EXTENSIONS
-- =====================================================

-- TimescaleDB: 시계열 데이터 최적화
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- UUID 생성 함수
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- =====================================================
-- ENUM TYPES
-- =====================================================

-- 시장 유형
CREATE TYPE market_type AS ENUM ('crypto', 'stock', 'forex', 'futures');

-- 주문 방향
CREATE TYPE order_side AS ENUM ('buy', 'sell');

-- 주문 타입
CREATE TYPE order_type AS ENUM ('market', 'limit', 'stop', 'stop_limit', 'trailing_stop');

-- 주문 상태
CREATE TYPE order_status AS ENUM ('pending', 'open', 'partially_filled', 'filled', 'cancelled', 'rejected', 'expired');

-- 주문 유효 기간
CREATE TYPE order_time_in_force AS ENUM ('gtc', 'ioc', 'fok', 'day');

-- 시그널 타입
CREATE TYPE signal_type AS ENUM ('entry', 'exit', 'add_to_position', 'reduce_position', 'scale');

-- =====================================================
-- SYMBOLS TABLE
-- 거래 가능한 심볼 정보 (종목, 통화쌍 등)
-- =====================================================

CREATE TABLE symbols (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    base VARCHAR(20) NOT NULL,                      -- 기준 자산 (예: BTC, AAPL)
    quote VARCHAR(20) NOT NULL,                     -- 결제 자산 (예: USD, KRW)
    market_type market_type NOT NULL,               -- 시장 유형
    exchange VARCHAR(50) NOT NULL,                  -- 거래소 (binance, kis 등)
    exchange_symbol VARCHAR(50),                    -- 거래소별 심볼 코드
    is_active BOOLEAN DEFAULT true,                 -- 활성화 여부
    min_quantity DECIMAL(30, 15),                   -- 최소 주문 수량
    max_quantity DECIMAL(30, 15),                   -- 최대 주문 수량
    quantity_step DECIMAL(30, 15),                  -- 수량 단위
    min_notional DECIMAL(30, 15),                   -- 최소 주문 금액
    price_precision INT,                            -- 가격 소수점 자릿수
    quantity_precision INT,                         -- 수량 소수점 자릿수
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(base, quote, market_type, exchange)
);

CREATE INDEX idx_symbols_active ON symbols(is_active) WHERE is_active = true;
CREATE INDEX idx_symbols_exchange ON symbols(exchange);

COMMENT ON TABLE symbols IS '거래 가능한 심볼(종목) 메타데이터';
COMMENT ON COLUMN symbols.min_notional IS '최소 주문 금액 (quantity * price)';

-- =====================================================
-- KLINES (OHLCV) TABLE
-- 시계열 캔들 데이터 - TimescaleDB Hypertable
-- =====================================================

CREATE TABLE klines (
    time TIMESTAMPTZ NOT NULL,                      -- 캔들 시작 시간
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    timeframe VARCHAR(10) NOT NULL,                 -- 타임프레임 (1m, 5m, 1h, 1d 등)
    open DECIMAL(30, 15) NOT NULL,
    high DECIMAL(30, 15) NOT NULL,
    low DECIMAL(30, 15) NOT NULL,
    close DECIMAL(30, 15) NOT NULL,
    volume DECIMAL(30, 15) NOT NULL,
    quote_volume DECIMAL(30, 15),                   -- 거래대금 (volume * price)
    num_trades INT,                                 -- 거래 건수
    PRIMARY KEY (symbol_id, timeframe, time)
);

-- TimescaleDB Hypertable 변환 (1주 단위 청크)
SELECT create_hypertable('klines', 'time', chunk_time_interval => INTERVAL '1 week');

CREATE INDEX idx_klines_symbol_time ON klines(symbol_id, time DESC);

-- 압축 정책: 30일 이상 데이터 압축
ALTER TABLE klines SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol_id, timeframe'
);
SELECT add_compression_policy('klines', INTERVAL '30 days');

COMMENT ON TABLE klines IS 'OHLCV 캔들 데이터 (TimescaleDB Hypertable)';

-- =====================================================
-- TRADE_TICKS TABLE
-- 거래소 실시간 체결 데이터 - TimescaleDB Hypertable
-- =====================================================

CREATE TABLE trade_ticks (
    time TIMESTAMPTZ NOT NULL,                      -- 체결 시간
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    exchange_trade_id VARCHAR(100) NOT NULL,        -- 거래소 거래 ID
    price DECIMAL(30, 15) NOT NULL,
    quantity DECIMAL(30, 15) NOT NULL,
    is_buyer_maker BOOLEAN,                         -- 매수자가 메이커인지 여부
    PRIMARY KEY (symbol_id, time, exchange_trade_id)
);

-- TimescaleDB Hypertable 변환 (1일 단위 청크)
SELECT create_hypertable('trade_ticks', 'time', chunk_time_interval => INTERVAL '1 day');

CREATE INDEX idx_trade_ticks_symbol ON trade_ticks(symbol_id, time DESC);

-- 압축 정책: 7일 이상 데이터 압축
ALTER TABLE trade_ticks SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol_id'
);
SELECT add_compression_policy('trade_ticks', INTERVAL '7 days');

COMMENT ON TABLE trade_ticks IS '거래소 실시간 체결 데이터 (TimescaleDB Hypertable)';

-- =====================================================
-- ORDERS TABLE
-- 주문 정보
-- =====================================================

CREATE TABLE orders (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    exchange VARCHAR(50) NOT NULL,                  -- 주문 거래소
    exchange_order_id VARCHAR(100),                 -- 거래소 주문 ID
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    side order_side NOT NULL,                       -- 매수/매도
    order_type order_type NOT NULL,                 -- 주문 타입
    status order_status NOT NULL DEFAULT 'pending', -- 주문 상태
    time_in_force order_time_in_force DEFAULT 'gtc',
    quantity DECIMAL(30, 15) NOT NULL,              -- 주문 수량
    filled_quantity DECIMAL(30, 15) DEFAULT 0,      -- 체결 수량
    price DECIMAL(30, 15),                          -- 지정가 (limit 주문)
    stop_price DECIMAL(30, 15),                     -- 스톱 가격 (stop 주문)
    average_fill_price DECIMAL(30, 15),             -- 평균 체결가
    strategy_id VARCHAR(100),                       -- 전략 ID (자동 주문)
    client_order_id VARCHAR(100),                   -- 클라이언트 주문 ID
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    filled_at TIMESTAMPTZ,                          -- 완전 체결 시간
    cancelled_at TIMESTAMPTZ,                       -- 취소 시간
    metadata JSONB DEFAULT '{}'                     -- 추가 메타데이터
);

CREATE INDEX idx_orders_status ON orders(status) WHERE status IN ('pending', 'open', 'partially_filled');
CREATE INDEX idx_orders_strategy ON orders(strategy_id) WHERE strategy_id IS NOT NULL;
CREATE INDEX idx_orders_symbol ON orders(symbol_id, created_at DESC);
CREATE INDEX idx_orders_exchange ON orders(exchange, exchange_order_id);

COMMENT ON TABLE orders IS '주문 정보 (실시간 주문 추적)';

-- =====================================================
-- TRADES TABLE
-- 실제 체결된 거래 내역
-- =====================================================

CREATE TABLE trades (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    order_id UUID NOT NULL REFERENCES orders(id),
    exchange VARCHAR(50) NOT NULL,
    exchange_trade_id VARCHAR(100) NOT NULL,        -- 거래소 거래 ID
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    side order_side NOT NULL,
    quantity DECIMAL(30, 15) NOT NULL,              -- 체결 수량
    price DECIMAL(30, 15) NOT NULL,                 -- 체결 가격
    fee DECIMAL(30, 15) DEFAULT 0,                  -- 수수료
    fee_currency VARCHAR(20),                       -- 수수료 통화
    is_maker BOOLEAN DEFAULT false,                 -- 메이커 거래 여부
    executed_at TIMESTAMPTZ NOT NULL,               -- 체결 시간
    created_at TIMESTAMPTZ DEFAULT NOW(),
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX idx_trades_order ON trades(order_id);
CREATE INDEX idx_trades_symbol ON trades(symbol_id, executed_at DESC);
CREATE INDEX idx_trades_executed ON trades(executed_at DESC);

COMMENT ON TABLE trades IS '실제 체결된 거래 내역';

-- =====================================================
-- POSITIONS TABLE
-- 보유 포지션 정보
-- =====================================================

CREATE TABLE positions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    exchange VARCHAR(50) NOT NULL,
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    side order_side NOT NULL,                       -- 롱/숏 (crypto는 buy만)
    quantity DECIMAL(30, 15) NOT NULL,              -- 보유 수량
    entry_price DECIMAL(30, 15) NOT NULL,           -- 평균 진입가
    current_price DECIMAL(30, 15),                  -- 현재가
    unrealized_pnl DECIMAL(30, 15) DEFAULT 0,       -- 미실현 손익
    realized_pnl DECIMAL(30, 15) DEFAULT 0,         -- 실현 손익
    strategy_id VARCHAR(100),                       -- 전략 ID
    opened_at TIMESTAMPTZ DEFAULT NOW(),            -- 포지션 오픈 시간
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    closed_at TIMESTAMPTZ,                          -- 포지션 종료 시간
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX idx_positions_open ON positions(exchange, symbol_id) WHERE closed_at IS NULL;
CREATE INDEX idx_positions_strategy ON positions(strategy_id) WHERE strategy_id IS NOT NULL;

COMMENT ON TABLE positions IS '보유 포지션 정보 (미청산 포지션 추적)';

-- =====================================================
-- SIGNALS TABLE
-- 전략에서 생성된 매매 시그널
-- =====================================================

CREATE TABLE signals (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    strategy_id VARCHAR(100) NOT NULL,
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    side order_side NOT NULL,                       -- 매수/매도
    signal_type signal_type NOT NULL,               -- 시그널 타입
    strength DECIMAL(5, 4) NOT NULL CHECK (strength >= 0 AND strength <= 1), -- 시그널 강도 (0~1)
    suggested_price DECIMAL(30, 15),                -- 제안 가격
    stop_loss DECIMAL(30, 15),                      -- 손절가
    take_profit DECIMAL(30, 15),                    -- 익절가
    created_at TIMESTAMPTZ DEFAULT NOW(),
    processed_at TIMESTAMPTZ,                       -- 처리 완료 시간
    order_id UUID REFERENCES orders(id),            -- 생성된 주문 ID
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX idx_signals_unprocessed ON signals(created_at) WHERE processed_at IS NULL;
CREATE INDEX idx_signals_strategy ON signals(strategy_id, created_at DESC);

COMMENT ON TABLE signals IS '전략에서 생성된 매매 시그널';
COMMENT ON COLUMN signals.strength IS '시그널 강도 (0.0 ~ 1.0)';

-- =====================================================
-- STRATEGIES TABLE
-- 전략 메타데이터 및 상태
-- =====================================================

CREATE TABLE strategies (
    id VARCHAR(100) PRIMARY KEY,                    -- 전략 고유 ID
    name VARCHAR(200) NOT NULL,                     -- 전략 이름
    description TEXT,                               -- 전략 설명
    version VARCHAR(20),                            -- 전략 버전
    is_active BOOLEAN DEFAULT false,                -- 활성화 여부
    config JSONB DEFAULT '{}',                      -- 전략 설정
    risk_limits JSONB DEFAULT '{}',                 -- 리스크 제한 설정
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    last_started_at TIMESTAMPTZ,                    -- 마지막 시작 시간
    last_stopped_at TIMESTAMPTZ                     -- 마지막 중지 시간
);

COMMENT ON TABLE strategies IS '전략 메타데이터 및 실행 상태';

-- =====================================================
-- PERFORMANCE_SNAPSHOTS TABLE
-- 전략별 성과 스냅샷
-- =====================================================

CREATE TABLE performance_snapshots (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    strategy_id VARCHAR(100) REFERENCES strategies(id),
    snapshot_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    total_trades INT DEFAULT 0,                     -- 총 거래 수
    winning_trades INT DEFAULT 0,                   -- 승리 거래 수
    losing_trades INT DEFAULT 0,                    -- 패배 거래 수
    total_pnl DECIMAL(30, 15) DEFAULT 0,            -- 총 손익
    total_fees DECIMAL(30, 15) DEFAULT 0,           -- 총 수수료
    max_drawdown DECIMAL(10, 4),                    -- 최대 낙폭 (%)
    sharpe_ratio DECIMAL(10, 4),                    -- 샤프 비율
    win_rate DECIMAL(5, 4),                         -- 승률 (0~1)
    profit_factor DECIMAL(10, 4),                   -- 수익 팩터
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX idx_performance_strategy ON performance_snapshots(strategy_id, snapshot_time DESC);

COMMENT ON TABLE performance_snapshots IS '전략별 성과 스냅샷 (시계열 추적)';

-- =====================================================
-- AUDIT_LOGS TABLE
-- 시스템 감사 로그
-- =====================================================

CREATE TABLE audit_logs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_type VARCHAR(50) NOT NULL,                -- 이벤트 타입 (order_created 등)
    entity_type VARCHAR(50),                        -- 엔티티 타입 (order, strategy 등)
    entity_id UUID,                                 -- 엔티티 ID
    user_id VARCHAR(100),                           -- 사용자 ID
    details JSONB DEFAULT '{}',                     -- 상세 정보
    ip_address INET,                                -- IP 주소
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_audit_logs_time ON audit_logs(created_at DESC);
CREATE INDEX idx_audit_logs_entity ON audit_logs(entity_type, entity_id);

COMMENT ON TABLE audit_logs IS '시스템 감사 로그 (모든 주요 이벤트 추적)';

-- =====================================================
-- USERS TABLE
-- API 인증 사용자
-- =====================================================

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    username VARCHAR(50) UNIQUE NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,            -- bcrypt 해시
    role VARCHAR(20) DEFAULT 'trader',              -- 역할 (trader, admin 등)
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    last_login_at TIMESTAMPTZ
);

CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_email ON users(email);

COMMENT ON TABLE users IS 'API 인증 사용자 정보';

-- =====================================================
-- API_KEYS TABLE
-- 거래소 API 키 (암호화 저장)
-- =====================================================

CREATE TABLE api_keys (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID REFERENCES users(id),
    exchange VARCHAR(50) NOT NULL,                  -- 거래소 (binance, kis 등)
    name VARCHAR(100) NOT NULL,                     -- API 키 별칭
    api_key_encrypted BYTEA NOT NULL,               -- 암호화된 API Key
    api_secret_encrypted BYTEA NOT NULL,            -- 암호화된 API Secret
    passphrase_encrypted BYTEA,                     -- 암호화된 Passphrase (일부 거래소)
    permissions JSONB DEFAULT '["read"]',           -- 권한 (read, trade, withdraw)
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    last_used_at TIMESTAMPTZ                        -- 마지막 사용 시간
);

CREATE INDEX idx_api_keys_user ON api_keys(user_id, is_active);

COMMENT ON TABLE api_keys IS '거래소 API 키 (AES-256-GCM 암호화 저장)';

-- =====================================================
-- FUNCTIONS AND TRIGGERS
-- =====================================================

-- 자동 updated_at 업데이트 함수
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Triggers: 자동 타임스탬프 업데이트
CREATE TRIGGER update_symbols_updated_at BEFORE UPDATE ON symbols
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_orders_updated_at BEFORE UPDATE ON orders
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_positions_updated_at BEFORE UPDATE ON positions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_strategies_updated_at BEFORE UPDATE ON strategies
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_api_keys_updated_at BEFORE UPDATE ON api_keys
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- =====================================================
-- DATA RETENTION POLICIES
-- =====================================================

-- klines: 2년 보존
SELECT add_retention_policy('klines', INTERVAL '2 years');

-- trade_ticks: 6개월 보존
SELECT add_retention_policy('trade_ticks', INTERVAL '6 months');
