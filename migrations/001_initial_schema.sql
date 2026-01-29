-- Trading Bot Initial Schema
-- TimescaleDB hypertables for time-series data

-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- =====================================================
-- ENUM TYPES
-- =====================================================

CREATE TYPE market_type AS ENUM ('crypto', 'stock', 'forex', 'futures');
CREATE TYPE order_side AS ENUM ('buy', 'sell');
CREATE TYPE order_type AS ENUM ('market', 'limit', 'stop', 'stop_limit', 'trailing_stop');
CREATE TYPE order_status AS ENUM ('pending', 'open', 'partially_filled', 'filled', 'cancelled', 'rejected', 'expired');
CREATE TYPE order_time_in_force AS ENUM ('gtc', 'ioc', 'fok', 'day');
CREATE TYPE signal_type AS ENUM ('entry', 'exit', 'add_to_position', 'reduce_position', 'scale');

-- =====================================================
-- SYMBOLS TABLE
-- =====================================================

CREATE TABLE symbols (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    base VARCHAR(20) NOT NULL,
    quote VARCHAR(20) NOT NULL,
    market_type market_type NOT NULL,
    exchange VARCHAR(50) NOT NULL,
    exchange_symbol VARCHAR(50),
    is_active BOOLEAN DEFAULT true,
    min_quantity DECIMAL(30, 15),
    max_quantity DECIMAL(30, 15),
    quantity_step DECIMAL(30, 15),
    min_notional DECIMAL(30, 15),
    price_precision INT,
    quantity_precision INT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(base, quote, market_type, exchange)
);

CREATE INDEX idx_symbols_active ON symbols(is_active) WHERE is_active = true;
CREATE INDEX idx_symbols_exchange ON symbols(exchange);

-- =====================================================
-- KLINES (OHLCV) TABLE - TimescaleDB Hypertable
-- =====================================================

CREATE TABLE klines (
    time TIMESTAMPTZ NOT NULL,
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    timeframe VARCHAR(10) NOT NULL,
    open DECIMAL(30, 15) NOT NULL,
    high DECIMAL(30, 15) NOT NULL,
    low DECIMAL(30, 15) NOT NULL,
    close DECIMAL(30, 15) NOT NULL,
    volume DECIMAL(30, 15) NOT NULL,
    quote_volume DECIMAL(30, 15),
    num_trades INT,
    PRIMARY KEY (symbol_id, timeframe, time)
);

-- Convert to hypertable
SELECT create_hypertable('klines', 'time', chunk_time_interval => INTERVAL '1 week');

-- Create indexes
CREATE INDEX idx_klines_symbol_time ON klines(symbol_id, time DESC);

-- Add compression policy (compress data older than 30 days)
ALTER TABLE klines SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol_id, timeframe'
);
SELECT add_compression_policy('klines', INTERVAL '30 days');

-- =====================================================
-- TRADES TABLE (Exchange Trades) - TimescaleDB Hypertable
-- =====================================================

CREATE TABLE trade_ticks (
    time TIMESTAMPTZ NOT NULL,
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    exchange_trade_id VARCHAR(100) NOT NULL,
    price DECIMAL(30, 15) NOT NULL,
    quantity DECIMAL(30, 15) NOT NULL,
    is_buyer_maker BOOLEAN,
    PRIMARY KEY (symbol_id, time, exchange_trade_id)
);

-- Convert to hypertable
SELECT create_hypertable('trade_ticks', 'time', chunk_time_interval => INTERVAL '1 day');

CREATE INDEX idx_trade_ticks_symbol ON trade_ticks(symbol_id, time DESC);

-- Compression policy
ALTER TABLE trade_ticks SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol_id'
);
SELECT add_compression_policy('trade_ticks', INTERVAL '7 days');

-- =====================================================
-- ORDERS TABLE
-- =====================================================

CREATE TABLE orders (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    exchange VARCHAR(50) NOT NULL,
    exchange_order_id VARCHAR(100),
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    side order_side NOT NULL,
    order_type order_type NOT NULL,
    status order_status NOT NULL DEFAULT 'pending',
    time_in_force order_time_in_force DEFAULT 'gtc',
    quantity DECIMAL(30, 15) NOT NULL,
    filled_quantity DECIMAL(30, 15) DEFAULT 0,
    price DECIMAL(30, 15),
    stop_price DECIMAL(30, 15),
    average_fill_price DECIMAL(30, 15),
    strategy_id VARCHAR(100),
    client_order_id VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    filled_at TIMESTAMPTZ,
    cancelled_at TIMESTAMPTZ,
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX idx_orders_status ON orders(status) WHERE status IN ('pending', 'open', 'partially_filled');
CREATE INDEX idx_orders_strategy ON orders(strategy_id) WHERE strategy_id IS NOT NULL;
CREATE INDEX idx_orders_symbol ON orders(symbol_id, created_at DESC);
CREATE INDEX idx_orders_exchange ON orders(exchange, exchange_order_id);

-- =====================================================
-- TRADES TABLE (Our Executed Trades)
-- =====================================================

CREATE TABLE trades (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    order_id UUID NOT NULL REFERENCES orders(id),
    exchange VARCHAR(50) NOT NULL,
    exchange_trade_id VARCHAR(100) NOT NULL,
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    side order_side NOT NULL,
    quantity DECIMAL(30, 15) NOT NULL,
    price DECIMAL(30, 15) NOT NULL,
    fee DECIMAL(30, 15) DEFAULT 0,
    fee_currency VARCHAR(20),
    is_maker BOOLEAN DEFAULT false,
    executed_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX idx_trades_order ON trades(order_id);
CREATE INDEX idx_trades_symbol ON trades(symbol_id, executed_at DESC);
CREATE INDEX idx_trades_executed ON trades(executed_at DESC);

-- =====================================================
-- POSITIONS TABLE
-- =====================================================

CREATE TABLE positions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    exchange VARCHAR(50) NOT NULL,
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    side order_side NOT NULL,
    quantity DECIMAL(30, 15) NOT NULL,
    entry_price DECIMAL(30, 15) NOT NULL,
    current_price DECIMAL(30, 15),
    unrealized_pnl DECIMAL(30, 15) DEFAULT 0,
    realized_pnl DECIMAL(30, 15) DEFAULT 0,
    strategy_id VARCHAR(100),
    opened_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    closed_at TIMESTAMPTZ,
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX idx_positions_open ON positions(exchange, symbol_id) WHERE closed_at IS NULL;
CREATE INDEX idx_positions_strategy ON positions(strategy_id) WHERE strategy_id IS NOT NULL;

-- =====================================================
-- SIGNALS TABLE
-- =====================================================

CREATE TABLE signals (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    strategy_id VARCHAR(100) NOT NULL,
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    side order_side NOT NULL,
    signal_type signal_type NOT NULL,
    strength DECIMAL(5, 4) NOT NULL CHECK (strength >= 0 AND strength <= 1),
    suggested_price DECIMAL(30, 15),
    stop_loss DECIMAL(30, 15),
    take_profit DECIMAL(30, 15),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    processed_at TIMESTAMPTZ,
    order_id UUID REFERENCES orders(id),
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX idx_signals_unprocessed ON signals(created_at) WHERE processed_at IS NULL;
CREATE INDEX idx_signals_strategy ON signals(strategy_id, created_at DESC);

-- =====================================================
-- STRATEGIES TABLE
-- =====================================================

CREATE TABLE strategies (
    id VARCHAR(100) PRIMARY KEY,
    name VARCHAR(200) NOT NULL,
    description TEXT,
    version VARCHAR(20),
    is_active BOOLEAN DEFAULT false,
    config JSONB DEFAULT '{}',
    risk_limits JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    last_started_at TIMESTAMPTZ,
    last_stopped_at TIMESTAMPTZ
);

-- =====================================================
-- PERFORMANCE SNAPSHOTS TABLE
-- =====================================================

CREATE TABLE performance_snapshots (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    strategy_id VARCHAR(100) REFERENCES strategies(id),
    snapshot_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    total_trades INT DEFAULT 0,
    winning_trades INT DEFAULT 0,
    losing_trades INT DEFAULT 0,
    total_pnl DECIMAL(30, 15) DEFAULT 0,
    total_fees DECIMAL(30, 15) DEFAULT 0,
    max_drawdown DECIMAL(10, 4),
    sharpe_ratio DECIMAL(10, 4),
    win_rate DECIMAL(5, 4),
    profit_factor DECIMAL(10, 4),
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX idx_performance_strategy ON performance_snapshots(strategy_id, snapshot_time DESC);

-- =====================================================
-- AUDIT LOG TABLE
-- =====================================================

CREATE TABLE audit_logs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_type VARCHAR(50) NOT NULL,
    entity_type VARCHAR(50),
    entity_id UUID,
    user_id VARCHAR(100),
    details JSONB DEFAULT '{}',
    ip_address INET,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_audit_logs_time ON audit_logs(created_at DESC);
CREATE INDEX idx_audit_logs_entity ON audit_logs(entity_type, entity_id);

-- =====================================================
-- USERS TABLE (for API authentication)
-- =====================================================

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    username VARCHAR(50) UNIQUE NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    role VARCHAR(20) DEFAULT 'trader',
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    last_login_at TIMESTAMPTZ
);

CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_email ON users(email);

-- =====================================================
-- API KEYS TABLE (for exchange connections)
-- =====================================================

CREATE TABLE api_keys (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID REFERENCES users(id),
    exchange VARCHAR(50) NOT NULL,
    name VARCHAR(100) NOT NULL,
    api_key_encrypted BYTEA NOT NULL,
    api_secret_encrypted BYTEA NOT NULL,
    passphrase_encrypted BYTEA,
    permissions JSONB DEFAULT '["read"]',
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    last_used_at TIMESTAMPTZ
);

CREATE INDEX idx_api_keys_user ON api_keys(user_id, is_active);

-- =====================================================
-- FUNCTIONS
-- =====================================================

-- Update timestamp function
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Apply update trigger to relevant tables
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

-- Drop old klines data (keep 2 years)
SELECT add_retention_policy('klines', INTERVAL '2 years');

-- Drop old trade ticks (keep 6 months)
SELECT add_retention_policy('trade_ticks', INTERVAL '6 months');
