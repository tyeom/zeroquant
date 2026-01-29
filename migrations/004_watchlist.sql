-- Watchlist Table for managing user's favorite symbols
-- Migration: 004_watchlist.sql

-- =====================================================
-- WATCHLIST TABLE
-- =====================================================

CREATE TABLE watchlist (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    symbol VARCHAR(50) NOT NULL,
    market VARCHAR(10) NOT NULL,  -- 'KR', 'US', 'crypto'
    display_name VARCHAR(100),
    sort_order INT DEFAULT 0,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(symbol, market)
);

CREATE INDEX idx_watchlist_active ON watchlist(is_active) WHERE is_active = true;
CREATE INDEX idx_watchlist_sort ON watchlist(sort_order);

-- Insert default watchlist items
-- Note: display_name must match WebSocket simulator symbols (use hyphens, not spaces)
INSERT INTO watchlist (symbol, market, display_name, sort_order) VALUES
    ('069500', 'KR', 'KODEX-200', 1),
    ('122630', 'KR', 'KODEX-레버리지', 2),
    ('SPY', 'US', 'SPY', 3),
    ('QQQ', 'US', 'QQQ', 4),
    ('TQQQ', 'US', 'TQQQ', 5);
