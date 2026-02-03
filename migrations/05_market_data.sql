-- =====================================================
-- 05_market_data.sql
-- 시장 데이터 (OHLCV) 캐시 및 최신 가격 뷰
-- =====================================================
--
-- 원본 마이그레이션: 005_yahoo_candle_cache.sql, 009_rename_candle_cache.sql,
--                  022_latest_prices_materialized_view.sql
--
-- 포함 내용:
-- - ohlcv: OHLCV 캔들 데이터 (TimescaleDB Hypertable, 압축 정책)
-- - ohlcv_metadata: 심볼별 캐시 상태 메타데이터
-- - mv_latest_prices: 심볼별 최신 일봉 가격 (Materialized View)
-- - 캐시 갭 조회 함수, 통계 조회 함수
-- - 자동 메타데이터 업데이트 트리거
--
-- 데이터 소스: Yahoo Finance, KRX 등 통합
--
-- =====================================================

-- =====================================================
-- OHLCV TABLE
-- OHLCV 캔들 데이터 - TimescaleDB Hypertable
-- =====================================================

CREATE TABLE ohlcv (
    -- 복합 기본키: 심볼 + 타임프레임 + 시간
    symbol VARCHAR(50) NOT NULL,                    -- "AAPL", "005930.KS", "SPY", "BTC-USD" 등
    timeframe VARCHAR(10) NOT NULL,                 -- "1m", "5m", "15m", "30m", "1h", "1d", "1wk", "1mo"
    open_time TIMESTAMPTZ NOT NULL,                 -- 캔들 시작 시간 (UTC)

    -- OHLCV 데이터
    open DECIMAL(30, 15) NOT NULL,                  -- 시가 (Opening price)
    high DECIMAL(30, 15) NOT NULL,                  -- 고가 (Highest price)
    low DECIMAL(30, 15) NOT NULL,                   -- 저가 (Lowest price)
    close DECIMAL(30, 15) NOT NULL,                 -- 종가 (Closing price)
    volume DECIMAL(30, 15) NOT NULL,                -- 거래량 (Trading volume)

    -- 메타데이터
    close_time TIMESTAMPTZ,                         -- 캔들 종료 시간
    fetched_at TIMESTAMPTZ DEFAULT NOW(),           -- 데이터 가져온 시간

    PRIMARY KEY (symbol, timeframe, open_time)
);

-- TimescaleDB Hypertable 변환 (1개월 단위 청크)
SELECT create_hypertable('ohlcv', 'open_time',
    chunk_time_interval => INTERVAL '1 month',
    if_not_exists => TRUE
);

-- 심볼 + 타임프레임으로 최신 데이터 조회용 인덱스
CREATE INDEX idx_ohlcv_symbol_tf_time
    ON ohlcv(symbol, timeframe, open_time DESC);

-- 특정 심볼의 마지막 캐시 시간 조회용 인덱스
CREATE INDEX idx_ohlcv_last_fetch
    ON ohlcv(symbol, timeframe, fetched_at DESC);

-- 압축 정책 (30일 이상 데이터 압축)
ALTER TABLE ohlcv SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol, timeframe'
);
SELECT add_compression_policy('ohlcv', INTERVAL '30 days', if_not_exists => TRUE);

COMMENT ON TABLE ohlcv IS 'OHLCV(Open-High-Low-Close-Volume) 캔들 데이터. 다양한 데이터 소스(Yahoo Finance, KRX 등)에서 통합 관리. (TimescaleDB Hypertable)';
COMMENT ON COLUMN ohlcv.symbol IS '종목 심볼 (예: AAPL, 005930.KS, SPY, BTC-USD)';
COMMENT ON COLUMN ohlcv.timeframe IS '시간 간격: 1m, 5m, 15m, 30m, 1h, 1d, 1wk, 1mo';
COMMENT ON COLUMN ohlcv.open_time IS '캔들 시작 시간 (UTC)';
COMMENT ON COLUMN ohlcv.open IS '시가 (Opening price)';
COMMENT ON COLUMN ohlcv.high IS '고가 (Highest price)';
COMMENT ON COLUMN ohlcv.low IS '저가 (Lowest price)';
COMMENT ON COLUMN ohlcv.close IS '종가 (Closing price)';
COMMENT ON COLUMN ohlcv.volume IS '거래량 (Trading volume)';

-- =====================================================
-- OHLCV_METADATA TABLE
-- 심볼별 캐시 상태 메타데이터
-- =====================================================

CREATE TABLE ohlcv_metadata (
    symbol VARCHAR(50) NOT NULL,
    timeframe VARCHAR(10) NOT NULL,
    first_cached_time TIMESTAMPTZ,                  -- 가장 오래된 캐시 데이터 시간
    last_cached_time TIMESTAMPTZ,                   -- 가장 최근 캐시 데이터 시간
    last_updated_at TIMESTAMPTZ DEFAULT NOW(),      -- 마지막 업데이트 시간
    total_candles INT DEFAULT 0,                    -- 총 캔들 수

    PRIMARY KEY (symbol, timeframe)
);

COMMENT ON TABLE ohlcv_metadata IS '심볼/타임프레임별 OHLCV 데이터 메타정보. 증분 업데이트 및 캐시 상태 관리용.';

-- =====================================================
-- TRIGGERS
-- =====================================================

-- 메타데이터 자동 업데이트 트리거 함수
CREATE OR REPLACE FUNCTION update_ohlcv_metadata()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO ohlcv_metadata (symbol, timeframe, first_cached_time, last_cached_time, total_candles)
    VALUES (NEW.symbol, NEW.timeframe, NEW.open_time, NEW.open_time, 1)
    ON CONFLICT (symbol, timeframe) DO UPDATE SET
        first_cached_time = LEAST(ohlcv_metadata.first_cached_time, NEW.open_time),
        last_cached_time = GREATEST(ohlcv_metadata.last_cached_time, NEW.open_time),
        last_updated_at = NOW(),
        total_candles = (
            SELECT COUNT(*) FROM ohlcv
            WHERE symbol = NEW.symbol AND timeframe = NEW.timeframe
        );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- 캔들 삽입 시 메타데이터 자동 업데이트
CREATE TRIGGER trigger_update_ohlcv_metadata
    AFTER INSERT ON ohlcv
    FOR EACH ROW
    EXECUTE FUNCTION update_ohlcv_metadata();

-- =====================================================
-- HELPER FUNCTIONS
-- =====================================================

-- 특정 심볼/타임프레임의 캐시 갭 확인 함수
CREATE OR REPLACE FUNCTION get_ohlcv_gaps(
    p_symbol VARCHAR(50),
    p_timeframe VARCHAR(10),
    p_start_time TIMESTAMPTZ,
    p_end_time TIMESTAMPTZ
)
RETURNS TABLE (
    gap_start TIMESTAMPTZ,
    gap_end TIMESTAMPTZ
) AS $$
BEGIN
    -- 간단한 갭 감지: 예상 간격보다 큰 공백이 있는 경우
    -- (상세 구현은 애플리케이션 레벨에서 처리)
    RETURN QUERY
    SELECT
        open_time AS gap_start,
        LEAD(open_time) OVER (ORDER BY open_time) AS gap_end
    FROM ohlcv
    WHERE symbol = p_symbol
      AND timeframe = p_timeframe
      AND open_time BETWEEN p_start_time AND p_end_time
    ORDER BY open_time;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION get_ohlcv_gaps(VARCHAR, VARCHAR, TIMESTAMPTZ, TIMESTAMPTZ) IS
'특정 심볼/타임프레임의 캐시 갭(빠진 구간) 확인';

-- 캐시 통계 조회 함수
CREATE OR REPLACE FUNCTION get_ohlcv_stats()
RETURNS TABLE (
    symbol VARCHAR(50),
    timeframe VARCHAR(10),
    first_time TIMESTAMPTZ,
    last_time TIMESTAMPTZ,
    candle_count BIGINT,
    last_updated TIMESTAMPTZ
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        m.symbol,
        m.timeframe,
        m.first_cached_time,
        m.last_cached_time,
        m.total_candles::BIGINT,
        m.last_updated_at
    FROM ohlcv_metadata m
    ORDER BY m.last_updated_at DESC;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION get_ohlcv_stats() IS
'전체 OHLCV 캐시 통계 조회 (심볼별 캔들 수, 시간 범위 등)';

-- =====================================================
-- MATERIALIZED VIEW: mv_latest_prices
-- 심볼별 최신 일봉 가격 (스크리닝 쿼리 최적화용)
-- =====================================================

-- 일봉 기준 심볼별 최신 가격 정보
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_latest_prices AS
SELECT DISTINCT ON (symbol)
    symbol,
    open_time,
    open,
    high,
    low,
    close,
    volume
FROM ohlcv
WHERE timeframe = '1d'
ORDER BY symbol, open_time DESC
WITH DATA;

-- 고유 인덱스 (REFRESH CONCURRENTLY 필수)
CREATE UNIQUE INDEX IF NOT EXISTS idx_mv_latest_prices_symbol
    ON mv_latest_prices(symbol);

-- 가격 조회용 인덱스
CREATE INDEX IF NOT EXISTS idx_mv_latest_prices_close
    ON mv_latest_prices(close);

-- 시간 조회용 인덱스
CREATE INDEX IF NOT EXISTS idx_mv_latest_prices_time
    ON mv_latest_prices(open_time DESC);

COMMENT ON MATERIALIZED VIEW mv_latest_prices IS
'심볼별 최신 일봉 가격. 스크리닝 쿼리 성능 최적화용. REFRESH MATERIALIZED VIEW CONCURRENTLY로 갱신.';

-- Materialized View 갱신 함수
CREATE OR REPLACE FUNCTION refresh_latest_prices()
RETURNS void AS $$
BEGIN
    REFRESH MATERIALIZED VIEW CONCURRENTLY mv_latest_prices;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION refresh_latest_prices() IS
'mv_latest_prices 뷰 갱신. 새 데이터 입력 후 호출하거나 스케줄러로 주기적 실행.';

-- =====================================================
-- 사용 예시
-- =====================================================
--
-- 1. 최신 가격 조회 (스크리닝):
--    SELECT symbol, close as current_price FROM mv_latest_prices;
--
-- 2. 특정 심볼의 일봉 조회:
--    SELECT * FROM ohlcv WHERE symbol = 'AAPL' AND timeframe = '1d' ORDER BY open_time DESC LIMIT 100;
--
-- 3. 캐시 통계 확인:
--    SELECT * FROM get_ohlcv_stats();
--
-- 4. 최신 가격 뷰 갱신 (트레이딩 시간 종료 후):
--    SELECT refresh_latest_prices();
--
-- =====================================================
