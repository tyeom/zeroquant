-- Portfolio Equity History Table
-- 자산 곡선(Equity Curve) 데이터를 저장하기 위한 테이블

-- =====================================================
-- PORTFOLIO EQUITY HISTORY TABLE
-- =====================================================

CREATE TABLE portfolio_equity_history (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- 자격증명 연결 (어떤 계좌의 데이터인지)
    credential_id UUID NOT NULL REFERENCES exchange_credentials(id) ON DELETE CASCADE,

    -- 스냅샷 시간
    snapshot_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- 자산 가치
    total_equity DECIMAL(30, 15) NOT NULL,      -- 총 자산 (현금 + 평가금액)
    cash_balance DECIMAL(30, 15) NOT NULL,       -- 현금 잔고
    securities_value DECIMAL(30, 15) NOT NULL,   -- 유가증권 평가금액

    -- 수익/손실
    total_pnl DECIMAL(30, 15) DEFAULT 0,         -- 총 손익
    daily_pnl DECIMAL(30, 15) DEFAULT 0,         -- 당일 손익

    -- 메타데이터
    currency VARCHAR(10) DEFAULT 'KRW',          -- 통화
    market VARCHAR(10) DEFAULT 'KR',             -- 시장 (KR, US)
    account_type VARCHAR(20),                    -- 계좌 유형 (real, paper)

    metadata JSONB DEFAULT '{}',

    created_at TIMESTAMPTZ DEFAULT NOW(),

    -- 동일 credential, 동일 시간대에 중복 방지 (1시간 단위)
    UNIQUE(credential_id, snapshot_time)
);

-- 인덱스
CREATE INDEX idx_equity_history_credential_time
    ON portfolio_equity_history(credential_id, snapshot_time DESC);

CREATE INDEX idx_equity_history_time
    ON portfolio_equity_history(snapshot_time DESC);

-- 시간순 조회 최적화
CREATE INDEX idx_equity_history_credential_time_asc
    ON portfolio_equity_history(credential_id, snapshot_time ASC);

-- =====================================================
-- 집계 함수용 인덱스 (월별, 연별 수익률 계산)
-- =====================================================

CREATE INDEX idx_equity_history_month
    ON portfolio_equity_history(credential_id, date_trunc('month', snapshot_time));

CREATE INDEX idx_equity_history_year
    ON portfolio_equity_history(credential_id, date_trunc('year', snapshot_time));

-- =====================================================
-- 데이터 보존 정책 (2년 보관)
-- =====================================================

-- TimescaleDB가 활성화된 경우 hypertable로 변환 가능
-- SELECT create_hypertable('portfolio_equity_history', 'snapshot_time',
--     chunk_time_interval => INTERVAL '1 month',
--     if_not_exists => TRUE);

-- =====================================================
-- 일별 집계 뷰 (대시보드 성능 최적화)
-- =====================================================

CREATE OR REPLACE VIEW portfolio_daily_equity AS
SELECT
    credential_id,
    date_trunc('day', snapshot_time)::date as date,
    -- 당일 마지막 스냅샷 기준
    (array_agg(total_equity ORDER BY snapshot_time DESC))[1] as closing_equity,
    (array_agg(cash_balance ORDER BY snapshot_time DESC))[1] as closing_cash,
    (array_agg(securities_value ORDER BY snapshot_time DESC))[1] as closing_securities,
    (array_agg(total_pnl ORDER BY snapshot_time DESC))[1] as total_pnl,
    (array_agg(daily_pnl ORDER BY snapshot_time DESC))[1] as daily_pnl,
    -- 당일 최고/최저
    MAX(total_equity) as high_equity,
    MIN(total_equity) as low_equity,
    -- 스냅샷 수
    COUNT(*) as snapshot_count
FROM portfolio_equity_history
GROUP BY credential_id, date_trunc('day', snapshot_time)::date;

-- =====================================================
-- 월별 수익률 뷰
-- =====================================================

CREATE OR REPLACE VIEW portfolio_monthly_returns AS
WITH monthly_data AS (
    SELECT
        credential_id,
        date_trunc('month', snapshot_time)::date as month,
        (array_agg(total_equity ORDER BY snapshot_time ASC))[1] as opening_equity,
        (array_agg(total_equity ORDER BY snapshot_time DESC))[1] as closing_equity
    FROM portfolio_equity_history
    GROUP BY credential_id, date_trunc('month', snapshot_time)::date
)
SELECT
    credential_id,
    month,
    opening_equity,
    closing_equity,
    CASE
        WHEN opening_equity > 0 THEN
            ((closing_equity - opening_equity) / opening_equity * 100)
        ELSE 0
    END as return_pct
FROM monthly_data;

-- =====================================================
-- COMMENT
-- =====================================================

COMMENT ON TABLE portfolio_equity_history IS
    '포트폴리오 자산 가치 히스토리. 자산 곡선(Equity Curve) 차트와 성과 분석에 사용됨.';

COMMENT ON COLUMN portfolio_equity_history.total_equity IS
    '총 자산 가치 (현금 + 유가증권 평가금액)';

COMMENT ON COLUMN portfolio_equity_history.daily_pnl IS
    '당일 손익. KIS API의 일별 손익 데이터 또는 전일 대비 계산값.';
