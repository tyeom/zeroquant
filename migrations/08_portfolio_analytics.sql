-- =====================================================
-- 08_portfolio_analytics.sql
-- 포트폴리오 분석 및 백테스팅 시스템
-- =====================================================
--
-- 원본 마이그레이션: 007_portfolio_equity_history.sql, 010_backtest_results.sql,
--                  030_add_missing_views.sql (분석 뷰들)
--
-- 포함 내용:
-- - portfolio_equity_history: 자산 곡선 데이터
-- - backtest_results: 백테스트 결과 저장
-- - 분석 뷰들: 전략 성과, 종목별 손익, 현재 포지션 등 (030에서 추가된 뷰 8개)
--
-- =====================================================

-- =====================================================
-- PORTFOLIO_EQUITY_HISTORY TABLE
-- 자산 곡선(Equity Curve) 데이터
-- =====================================================

CREATE TABLE portfolio_equity_history (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- 자격증명 연결 (어떤 계좌의 데이터인지)
    credential_id UUID NOT NULL REFERENCES exchange_credentials(id) ON DELETE CASCADE,

    -- 스냅샷 시간
    snapshot_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- 자산 가치
    total_equity DECIMAL(30, 15) NOT NULL,          -- 총 자산 (현금 + 평가금액)
    cash_balance DECIMAL(30, 15) NOT NULL,          -- 현금 잔고
    securities_value DECIMAL(30, 15) NOT NULL,      -- 유가증권 평가금액

    -- 수익/손실
    total_pnl DECIMAL(30, 15) DEFAULT 0,            -- 총 손익
    daily_pnl DECIMAL(30, 15) DEFAULT 0,            -- 당일 손익

    -- 메타데이터
    currency VARCHAR(10) DEFAULT 'KRW',             -- 통화
    market VARCHAR(10) DEFAULT 'KR',                -- 시장 (KR, US)
    account_type VARCHAR(20),                       -- 계좌 유형 (real, paper)

    metadata JSONB DEFAULT '{}',

    created_at TIMESTAMPTZ DEFAULT NOW(),

    -- 동일 credential, 동일 시간대에 중복 방지
    UNIQUE(credential_id, snapshot_time)
);

-- 인덱스
CREATE INDEX idx_equity_history_credential_time
    ON portfolio_equity_history(credential_id, snapshot_time DESC);

CREATE INDEX idx_equity_history_time
    ON portfolio_equity_history(snapshot_time DESC);

CREATE INDEX idx_equity_history_credential_time_asc
    ON portfolio_equity_history(credential_id, snapshot_time ASC);

CREATE INDEX idx_equity_history_month
    ON portfolio_equity_history(credential_id, (date_trunc('month', snapshot_time)));

CREATE INDEX idx_equity_history_year
    ON portfolio_equity_history(credential_id, (date_trunc('year', snapshot_time)));

COMMENT ON TABLE portfolio_equity_history IS '포트폴리오 자산 가치 히스토리. 자산 곡선(Equity Curve) 차트와 성과 분석에 사용됨.';
COMMENT ON COLUMN portfolio_equity_history.total_equity IS '총 자산 가치 (현금 + 유가증권 평가금액)';
COMMENT ON COLUMN portfolio_equity_history.daily_pnl IS '당일 손익. KIS API의 일별 손익 데이터 또는 전일 대비 계산값.';

-- =====================================================
-- BACKTEST_RESULTS TABLE
-- 백테스트 결과 저장
-- =====================================================

CREATE TABLE IF NOT EXISTS backtest_results (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- 전략 정보
    strategy_id VARCHAR(100) NOT NULL,              -- strategies 테이블의 id 참조
    strategy_type VARCHAR(50) NOT NULL,             -- 전략 타입 (sma_crossover, bollinger 등)

    -- 백테스트 설정
    symbol VARCHAR(500) NOT NULL,                   -- 심볼 (다중 자산은 콤마 구분)
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    initial_capital DECIMAL(20, 2) NOT NULL,
    slippage_rate DECIMAL(10, 6) DEFAULT 0.0005,

    -- 결과 (JSONB로 저장하여 유연성 확보)
    metrics JSONB NOT NULL,                         -- 성과 지표
    config_summary JSONB NOT NULL,                  -- 설정 요약
    equity_curve JSONB NOT NULL DEFAULT '[]',       -- 자산 곡선
    trades JSONB NOT NULL DEFAULT '[]',             -- 거래 내역

    -- 상태
    success BOOLEAN NOT NULL DEFAULT TRUE,
    error_message TEXT,

    -- 메타데이터
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ                          -- soft delete
);

-- 인덱스
CREATE INDEX idx_backtest_results_strategy
    ON backtest_results(strategy_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX idx_backtest_results_type
    ON backtest_results(strategy_type, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX idx_backtest_results_symbol
    ON backtest_results(symbol, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX idx_backtest_results_recent
    ON backtest_results(created_at DESC)
    WHERE deleted_at IS NULL;

COMMENT ON TABLE backtest_results IS '백테스트 결과 저장 테이블. 전략별 백테스트 수행 결과를 영구 저장합니다.';
COMMENT ON COLUMN backtest_results.metrics IS '성과 지표 JSON: total_return_pct, annualized_return_pct, max_drawdown_pct, sharpe_ratio 등';
COMMENT ON COLUMN backtest_results.equity_curve IS '자산 곡선 JSON 배열: [{timestamp, equity, drawdown_pct}, ...]';
COMMENT ON COLUMN backtest_results.trades IS '거래 내역 JSON 배열: [{symbol, side, entry_price, exit_price, quantity, pnl, return_pct}, ...]';
COMMENT ON COLUMN backtest_results.deleted_at IS '소프트 삭제 시간. NULL이면 활성 상태';

-- =====================================================
-- ANALYTICAL VIEWS (030에서 추가된 뷰들)
-- =====================================================

-- 현재 포지션 뷰
CREATE VIEW public.journal_current_positions AS
 SELECT DISTINCT ON (position_snapshots.credential_id, position_snapshots.symbol) position_snapshots.id,
    position_snapshots.credential_id,
    position_snapshots.snapshot_time,
    position_snapshots.exchange,
    position_snapshots.symbol,
    position_snapshots.symbol_name,
    position_snapshots.side,
    position_snapshots.quantity,
    position_snapshots.entry_price,
    position_snapshots.current_price,
    position_snapshots.cost_basis,
    position_snapshots.market_value,
    position_snapshots.unrealized_pnl,
    position_snapshots.unrealized_pnl_pct,
    position_snapshots.realized_pnl,
    position_snapshots.weight_pct,
    position_snapshots.first_trade_at,
    position_snapshots.last_trade_at,
    position_snapshots.trade_count,
    position_snapshots.strategy_id
   FROM public.position_snapshots
  WHERE (position_snapshots.quantity > (0)::numeric)
  ORDER BY position_snapshots.credential_id, position_snapshots.symbol, position_snapshots.snapshot_time DESC;

-- 일별 자산 집계 뷰
CREATE VIEW public.portfolio_daily_equity AS
 SELECT portfolio_equity_history.credential_id,
    (date_trunc('day'::text, portfolio_equity_history.snapshot_time))::date AS date,
    (array_agg(portfolio_equity_history.total_equity ORDER BY portfolio_equity_history.snapshot_time DESC))[1] AS closing_equity,
    (array_agg(portfolio_equity_history.cash_balance ORDER BY portfolio_equity_history.snapshot_time DESC))[1] AS closing_cash,
    (array_agg(portfolio_equity_history.securities_value ORDER BY portfolio_equity_history.snapshot_time DESC))[1] AS closing_securities,
    (array_agg(portfolio_equity_history.total_pnl ORDER BY portfolio_equity_history.snapshot_time DESC))[1] AS total_pnl,
    (array_agg(portfolio_equity_history.daily_pnl ORDER BY portfolio_equity_history.snapshot_time DESC))[1] AS daily_pnl,
    max(portfolio_equity_history.total_equity) AS high_equity,
    min(portfolio_equity_history.total_equity) AS low_equity,
    count(*) AS snapshot_count
   FROM public.portfolio_equity_history
  GROUP BY portfolio_equity_history.credential_id, ((date_trunc('day'::text, portfolio_equity_history.snapshot_time))::date);

-- 월별 수익률 뷰
CREATE VIEW public.portfolio_monthly_returns AS
 WITH monthly_data AS (
         SELECT portfolio_equity_history.credential_id,
            (date_trunc('month'::text, portfolio_equity_history.snapshot_time))::date AS month,
            (array_agg(portfolio_equity_history.total_equity ORDER BY portfolio_equity_history.snapshot_time))[1] AS opening_equity,
            (array_agg(portfolio_equity_history.total_equity ORDER BY portfolio_equity_history.snapshot_time DESC))[1] AS closing_equity
           FROM public.portfolio_equity_history
          GROUP BY portfolio_equity_history.credential_id, ((date_trunc('month'::text, portfolio_equity_history.snapshot_time))::date)
        )
 SELECT monthly_data.credential_id,
    monthly_data.month,
    monthly_data.opening_equity,
    monthly_data.closing_equity,
        CASE
            WHEN (monthly_data.opening_equity > (0)::numeric) THEN (((monthly_data.closing_equity - monthly_data.opening_equity) / monthly_data.opening_equity) * (100)::numeric)
            ELSE (0)::numeric
        END AS return_pct
   FROM monthly_data;

-- 전략별 월간 성과 뷰
CREATE VIEW public.v_strategy_monthly_performance AS
 SELECT ec.credential_id,
    COALESCE(te.strategy_id, 'manual'::character varying) AS strategy_id,
    COALESCE(te.strategy_name, '수동 거래'::character varying) AS strategy_name,
    (EXTRACT(year FROM (ec.executed_at AT TIME ZONE 'Asia/Seoul'::text)))::integer AS year,
    (EXTRACT(month FROM (ec.executed_at AT TIME ZONE 'Asia/Seoul'::text)))::integer AS month,
    count(*) AS total_trades,
    COALESCE(sum(ec.amount), (0)::numeric) AS total_volume,
    COALESCE(sum(te.realized_pnl), (0)::numeric) AS realized_pnl,
    count(*) FILTER (WHERE (te.realized_pnl > (0)::numeric)) AS winning_trades,
    count(*) FILTER (WHERE (te.realized_pnl < (0)::numeric)) AS losing_trades
   FROM (public.execution_cache ec
     LEFT JOIN public.trade_executions te ON (((te.credential_id = ec.credential_id) AND ((te.exchange)::text = (ec.exchange)::text) AND ((te.exchange_trade_id)::text = (ec.trade_id)::text))))
  GROUP BY ec.credential_id, COALESCE(te.strategy_id, 'manual'::character varying), COALESCE(te.strategy_name, '수동 거래'::character varying), (EXTRACT(year FROM (ec.executed_at AT TIME ZONE 'Asia/Seoul'::text))), (EXTRACT(month FROM (ec.executed_at AT TIME ZONE 'Asia/Seoul'::text)));

COMMENT ON VIEW public.v_strategy_monthly_performance IS '전략별 월간 성과 추이 뷰';

-- 전략별 성과 분석 뷰
CREATE VIEW public.v_strategy_performance AS
 SELECT ec.credential_id,
    COALESCE(te.strategy_id, 'manual'::character varying) AS strategy_id,
    COALESCE(te.strategy_name, '수동 거래'::character varying) AS strategy_name,
    count(*) AS total_trades,
    count(*) FILTER (WHERE ((ec.side)::text = 'buy'::text)) AS buy_trades,
    count(*) FILTER (WHERE ((ec.side)::text = 'sell'::text)) AS sell_trades,
    count(DISTINCT ec.symbol) AS unique_symbols,
    COALESCE(sum(ec.amount), (0)::numeric) AS total_volume,
    COALESCE(sum(ec.fee), (0)::numeric) AS total_fees,
    COALESCE(sum(te.realized_pnl), (0)::numeric) AS realized_pnl,
    count(*) FILTER (WHERE (te.realized_pnl > (0)::numeric)) AS winning_trades,
    count(*) FILTER (WHERE (te.realized_pnl < (0)::numeric)) AS losing_trades,
        CASE
            WHEN (count(*) FILTER (WHERE (te.realized_pnl IS NOT NULL)) > 0) THEN round((((count(*) FILTER (WHERE (te.realized_pnl > (0)::numeric)))::numeric * (100)::numeric) / (NULLIF(count(*) FILTER (WHERE (te.realized_pnl IS NOT NULL)), 0))::numeric), 2)
            ELSE (0)::numeric
        END AS win_rate_pct,
    COALESCE(avg(te.realized_pnl) FILTER (WHERE (te.realized_pnl > (0)::numeric)), (0)::numeric) AS avg_win,
    COALESCE(abs(avg(te.realized_pnl) FILTER (WHERE (te.realized_pnl < (0)::numeric))), (0)::numeric) AS avg_loss,
        CASE
            WHEN (COALESCE(abs(sum(te.realized_pnl) FILTER (WHERE (te.realized_pnl < (0)::numeric))), (0)::numeric) > (0)::numeric) THEN round((COALESCE(sum(te.realized_pnl) FILTER (WHERE (te.realized_pnl > (0)::numeric)), (0)::numeric) / abs(COALESCE(sum(te.realized_pnl) FILTER (WHERE (te.realized_pnl < (0)::numeric)), (1)::numeric))), 2)
            ELSE NULL::numeric
        END AS profit_factor,
    max(te.realized_pnl) AS largest_win,
    min(te.realized_pnl) AS largest_loss,
    count(DISTINCT date((ec.executed_at AT TIME ZONE 'Asia/Seoul'::text))) AS active_trading_days,
    min(ec.executed_at) AS first_trade_at,
    max(ec.executed_at) AS last_trade_at
   FROM (public.execution_cache ec
     LEFT JOIN public.trade_executions te ON (((te.credential_id = ec.credential_id) AND ((te.exchange)::text = (ec.exchange)::text) AND ((te.exchange_trade_id)::text = (ec.trade_id)::text))))
  GROUP BY ec.credential_id, COALESCE(te.strategy_id, 'manual'::character varying), COALESCE(te.strategy_name, '수동 거래'::character varying);

COMMENT ON VIEW public.v_strategy_performance IS '전략별 성과 분석 뷰';

-- 종목별 손익 뷰
CREATE VIEW public.v_symbol_pnl AS
 SELECT ec.credential_id,
    ec.symbol,
    max((ec.normalized_symbol)::text) AS symbol_name,
    count(*) AS total_trades,
    COALESCE(sum(ec.quantity) FILTER (WHERE ((ec.side)::text = 'buy'::text)), (0)::numeric) AS total_buy_qty,
    COALESCE(sum(ec.quantity) FILTER (WHERE ((ec.side)::text = 'sell'::text)), (0)::numeric) AS total_sell_qty,
    COALESCE(sum(ec.amount) FILTER (WHERE ((ec.side)::text = 'buy'::text)), (0)::numeric) AS total_buy_value,
    COALESCE(sum(ec.amount) FILTER (WHERE ((ec.side)::text = 'sell'::text)), (0)::numeric) AS total_sell_value,
    COALESCE(sum(ec.fee), (0)::numeric) AS total_fees,
    COALESCE(sum(te.realized_pnl), (0)::numeric) AS realized_pnl,
    min(ec.executed_at) AS first_trade_at,
    max(ec.executed_at) AS last_trade_at
   FROM (public.execution_cache ec
     LEFT JOIN public.trade_executions te ON (((te.credential_id = ec.credential_id) AND ((te.exchange)::text = (ec.exchange)::text) AND ((te.exchange_trade_id)::text = (ec.trade_id)::text))))
  GROUP BY ec.credential_id, ec.symbol;

COMMENT ON VIEW public.v_symbol_pnl IS '종목별 손익 집계 뷰';

-- 전체 PnL 요약 뷰
CREATE VIEW public.v_total_pnl AS
 SELECT ec.credential_id,
    COALESCE(sum(te.realized_pnl), (0)::numeric) AS total_realized_pnl,
    COALESCE(sum(ec.fee), (0)::numeric) AS total_fees,
    count(*) AS total_trades,
    count(*) FILTER (WHERE ((ec.side)::text = 'buy'::text)) AS buy_trades,
    count(*) FILTER (WHERE ((ec.side)::text = 'sell'::text)) AS sell_trades,
    count(*) FILTER (WHERE (te.realized_pnl > (0)::numeric)) AS winning_trades,
    count(*) FILTER (WHERE (te.realized_pnl < (0)::numeric)) AS losing_trades,
    COALESCE(sum(ec.amount), (0)::numeric) AS total_volume,
    min(ec.executed_at) AS first_trade_at,
    max(ec.executed_at) AS last_trade_at
   FROM (public.execution_cache ec
     LEFT JOIN public.trade_executions te ON (((te.credential_id = ec.credential_id) AND ((te.exchange)::text = (ec.exchange)::text) AND ((te.exchange_trade_id)::text = (ec.trade_id)::text))))
  GROUP BY ec.credential_id;

COMMENT ON VIEW public.v_total_pnl IS '전체 PnL 요약 뷰';

-- 트레이딩 인사이트 뷰
CREATE VIEW public.v_trading_insights AS
 SELECT ec.credential_id,
    count(*) AS total_trades,
    count(*) FILTER (WHERE ((ec.side)::text = 'buy'::text)) AS buy_trades,
    count(*) FILTER (WHERE ((ec.side)::text = 'sell'::text)) AS sell_trades,
    count(DISTINCT ec.symbol) AS unique_symbols,
    COALESCE(sum(te.realized_pnl), (0)::numeric) AS total_realized_pnl,
    COALESCE(sum(ec.fee), (0)::numeric) AS total_fees,
    count(*) FILTER (WHERE (te.realized_pnl > (0)::numeric)) AS winning_trades,
    count(*) FILTER (WHERE (te.realized_pnl < (0)::numeric)) AS losing_trades,
        CASE
            WHEN (count(*) FILTER (WHERE (te.realized_pnl IS NOT NULL)) > 0) THEN round((((count(*) FILTER (WHERE (te.realized_pnl > (0)::numeric)))::numeric * (100)::numeric) / (NULLIF(count(*) FILTER (WHERE (te.realized_pnl IS NOT NULL)), 0))::numeric), 2)
            ELSE (0)::numeric
        END AS win_rate_pct,
    COALESCE(avg(te.realized_pnl) FILTER (WHERE (te.realized_pnl > (0)::numeric)), (0)::numeric) AS avg_win,
    COALESCE(abs(avg(te.realized_pnl) FILTER (WHERE (te.realized_pnl < (0)::numeric))), (0)::numeric) AS avg_loss,
        CASE
            WHEN (COALESCE(abs(sum(te.realized_pnl) FILTER (WHERE (te.realized_pnl < (0)::numeric))), (0)::numeric) > (0)::numeric) THEN round((COALESCE(sum(te.realized_pnl) FILTER (WHERE (te.realized_pnl > (0)::numeric)), (0)::numeric) / abs(COALESCE(sum(te.realized_pnl) FILTER (WHERE (te.realized_pnl < (0)::numeric)), (1)::numeric))), 2)
            ELSE NULL::numeric
        END AS profit_factor,
    (EXTRACT(day FROM (max(ec.executed_at) - min(ec.executed_at))))::integer AS trading_period_days,
    count(DISTINCT date((ec.executed_at AT TIME ZONE 'Asia/Seoul'::text))) AS active_trading_days,
    max(te.realized_pnl) AS largest_win,
    min(te.realized_pnl) AS largest_loss,
    min(ec.executed_at) AS first_trade_at,
    max(ec.executed_at) AS last_trade_at
   FROM (public.execution_cache ec
     LEFT JOIN public.trade_executions te ON (((te.credential_id = ec.credential_id) AND ((te.exchange)::text = (ec.exchange)::text) AND ((te.exchange_trade_id)::text = (ec.trade_id)::text))))
  GROUP BY ec.credential_id;

COMMENT ON VIEW public.v_trading_insights IS '투자 인사이트 통계 뷰';
