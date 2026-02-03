-- =====================================================
-- 07_trading_journal.sql
-- 트레이딩 저널 시스템 (체결 내역 및 포지션 스냅샷)
-- =====================================================
--
-- 원본 마이그레이션: 015_trading_journal.sql, 016_positions_credential_id.sql
--
-- 포함 내용:
-- - trade_executions: 매매일지용 체결 내역 (메모, 태그 지원)
-- - position_snapshots: 시간별 포지션 스냅샷
-- - positions 테이블 확장 (credential_id, symbol_name, symbol 컬럼 추가)
--
-- 주의: 분석용 뷰들은 08_portfolio_analytics.sql에서 생성됩니다.
--
-- =====================================================

-- =====================================================
-- TRADE_EXECUTIONS TABLE
-- 매매일지용 체결 내역
-- =====================================================

CREATE TABLE trade_executions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- 자격증명 연결 (어떤 계좌의 거래인지)
    credential_id UUID NOT NULL REFERENCES exchange_credentials(id) ON DELETE CASCADE,

    -- 거래 기본 정보
    exchange VARCHAR(50) NOT NULL,
    symbol VARCHAR(50) NOT NULL,                    -- "BTC/USDT", "005930" 등
    symbol_name VARCHAR(100),                       -- "삼성전자", "Bitcoin" 등 (표시용)

    -- 거래 유형
    side order_side NOT NULL,                       -- buy, sell
    order_type order_type NOT NULL,                 -- market, limit 등

    -- 수량 및 가격
    quantity DECIMAL(30, 15) NOT NULL,
    price DECIMAL(30, 15) NOT NULL,                 -- 체결가
    notional_value DECIMAL(30, 15) NOT NULL,        -- 거래대금 (quantity * price)

    -- 수수료
    fee DECIMAL(30, 15) DEFAULT 0,
    fee_currency VARCHAR(20),

    -- 포지션 영향
    position_effect VARCHAR(20),                    -- open, close, add, reduce
    realized_pnl DECIMAL(30, 15),                   -- 실현 손익 (청산 시)

    -- 주문 연결 (선택적)
    order_id UUID REFERENCES orders(id) ON DELETE SET NULL,
    exchange_order_id VARCHAR(100),
    exchange_trade_id VARCHAR(100),

    -- 전략 연결 (선택적)
    strategy_id VARCHAR(100),
    strategy_name VARCHAR(200),

    -- 체결 시간
    executed_at TIMESTAMPTZ NOT NULL,

    -- 메모 및 태그 (매매일지용)
    memo TEXT,                                      -- 사용자 메모
    tags JSONB DEFAULT '[]',                        -- 태그 배열 ["손절", "스윙"] 등

    -- 메타데이터
    metadata JSONB DEFAULT '{}',

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- 인덱스
CREATE INDEX idx_trade_executions_credential_time
    ON trade_executions(credential_id, executed_at DESC);

CREATE INDEX idx_trade_executions_symbol
    ON trade_executions(credential_id, symbol, executed_at DESC);

CREATE INDEX idx_trade_executions_strategy
    ON trade_executions(strategy_id, executed_at DESC)
    WHERE strategy_id IS NOT NULL;

CREATE INDEX idx_trade_executions_date
    ON trade_executions(credential_id, (executed_at::date));

COMMENT ON TABLE trade_executions IS '매매일지용 체결 내역. 거래 기록과 메모, 태그를 저장하여 트레이딩 분석 지원.';
COMMENT ON COLUMN trade_executions.position_effect IS '포지션 영향: open(신규진입), close(청산), add(추가매수), reduce(부분청산)';
COMMENT ON COLUMN trade_executions.tags IS '사용자 정의 태그. 예: ["손절", "스윙", "단타"]';

-- updated_at 트리거
CREATE TRIGGER update_trade_executions_updated_at
    BEFORE UPDATE ON trade_executions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- =====================================================
-- POSITION_SNAPSHOTS TABLE
-- 포지션 스냅샷 (시간별 추적)
-- =====================================================

CREATE TABLE position_snapshots (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- 자격증명 연결
    credential_id UUID NOT NULL REFERENCES exchange_credentials(id) ON DELETE CASCADE,

    -- 스냅샷 시간
    snapshot_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- 종목 정보
    exchange VARCHAR(50) NOT NULL,
    symbol VARCHAR(50) NOT NULL,
    symbol_name VARCHAR(100),

    -- 포지션 유형
    side order_side NOT NULL,                       -- buy(롱), sell(숏)

    -- 수량 및 가격
    quantity DECIMAL(30, 15) NOT NULL,
    entry_price DECIMAL(30, 15) NOT NULL,           -- 가중평균 매입가
    current_price DECIMAL(30, 15),                  -- 현재가

    -- 평가 금액
    cost_basis DECIMAL(30, 15) NOT NULL,            -- 매입 원가 (entry_price * quantity)
    market_value DECIMAL(30, 15),                   -- 평가 금액 (current_price * quantity)

    -- 손익
    unrealized_pnl DECIMAL(30, 15) DEFAULT 0,       -- 미실현 손익
    unrealized_pnl_pct DECIMAL(10, 4) DEFAULT 0,    -- 수익률 (%)
    realized_pnl DECIMAL(30, 15) DEFAULT 0,         -- 누적 실현 손익

    -- 포트폴리오 비중
    weight_pct DECIMAL(10, 4),                      -- 포트폴리오 내 비중 (%)

    -- 첫 매수 및 최근 거래
    first_trade_at TIMESTAMPTZ,
    last_trade_at TIMESTAMPTZ,
    trade_count INT DEFAULT 0,                      -- 해당 종목 거래 횟수

    -- 전략 연결 (선택적)
    strategy_id VARCHAR(100),

    -- 메타데이터
    metadata JSONB DEFAULT '{}',

    created_at TIMESTAMPTZ DEFAULT NOW(),

    -- 동일 credential, symbol, 시간대에 중복 방지
    UNIQUE(credential_id, symbol, snapshot_time)
);

-- 인덱스
CREATE INDEX idx_position_snapshots_credential_time
    ON position_snapshots(credential_id, snapshot_time DESC);

CREATE INDEX idx_position_snapshots_symbol
    ON position_snapshots(credential_id, symbol, snapshot_time DESC);

CREATE INDEX idx_position_snapshots_latest
    ON position_snapshots(credential_id, snapshot_time DESC)
    WHERE quantity > 0;

COMMENT ON TABLE position_snapshots IS '포지션 스냅샷. 시간별 포지션 상태를 기록하여 포지션 변화 추적.';
COMMENT ON COLUMN position_snapshots.entry_price IS '가중평균 매입가. (sum(price * quantity) / sum(quantity))';
COMMENT ON COLUMN position_snapshots.weight_pct IS '포트폴리오 내 비중. 총 자산 대비 해당 종목 비율.';

-- =====================================================
-- POSITIONS TABLE 확장
-- credential_id, symbol_name, symbol 컬럼 추가 (016)
-- =====================================================

-- 1. credential_id 컬럼 추가
ALTER TABLE positions
ADD COLUMN IF NOT EXISTS credential_id UUID REFERENCES exchange_credentials(id);

-- 2. symbol_name 컬럼 추가 (종목명 표시용)
ALTER TABLE positions
ADD COLUMN IF NOT EXISTS symbol_name VARCHAR(200);

-- 3. symbol 컬럼 추가 (심볼 문자열 직접 저장 - 거래소 중립)
ALTER TABLE positions
ADD COLUMN IF NOT EXISTS symbol VARCHAR(50);

-- 4. 기존 인덱스 삭제 후 재생성 (credential_id 포함)
DROP INDEX IF EXISTS idx_positions_open;
CREATE INDEX idx_positions_open_credential
ON positions (credential_id, exchange, symbol_id)
WHERE closed_at IS NULL;

-- 5. credential_id로 빠른 조회를 위한 인덱스
CREATE INDEX IF NOT EXISTS idx_positions_credential
ON positions (credential_id)
WHERE closed_at IS NULL;

-- 6. symbol로 조회를 위한 인덱스 (symbol_id 대신 사용 가능)
CREATE INDEX IF NOT EXISTS idx_positions_symbol
ON positions (credential_id, symbol)
WHERE closed_at IS NULL;

COMMENT ON COLUMN positions.credential_id IS '거래소 자격증명 ID (exchange_credentials.id)';
COMMENT ON COLUMN positions.symbol_name IS '종목명 (표시용)';
COMMENT ON COLUMN positions.symbol IS '심볼 코드 (예: 005930, AAPL)';

-- =====================================================
-- 사용 예시
-- =====================================================
--
-- 1. 특정 계좌의 최근 체결 내역 조회:
--    SELECT * FROM trade_executions
--    WHERE credential_id = ?
--    ORDER BY executed_at DESC LIMIT 100;
--
-- 2. 현재 보유 포지션 조회:
--    SELECT DISTINCT ON (credential_id, symbol) *
--    FROM position_snapshots
--    WHERE quantity > 0
--    ORDER BY credential_id, symbol, snapshot_time DESC;
--
-- 3. 특정 종목의 거래 이력 조회:
--    SELECT * FROM trade_executions
--    WHERE credential_id = ? AND symbol = '005930'
--    ORDER BY executed_at;
--
-- =====================================================
