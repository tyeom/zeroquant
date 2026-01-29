-- Yahoo Finance Candle Cache Table
-- 전략, 백테스팅, 시뮬레이션, 트레이딩에서 공통으로 사용하는 캔들 데이터 캐시

-- =====================================================
-- YAHOO CANDLE CACHE TABLE - TimescaleDB Hypertable
-- =====================================================

-- 심볼 문자열과 타임프레임으로 직접 조회 가능한 구조
-- 기존 klines 테이블과 별도로 운영 (Yahoo Finance 전용)

CREATE TABLE yahoo_candle_cache (
    -- 복합 기본키: 심볼 + 타임프레임 + 시간
    symbol VARCHAR(50) NOT NULL,          -- "AAPL", "005930.KS", "SPY" 등
    timeframe VARCHAR(10) NOT NULL,       -- "1m", "5m", "15m", "30m", "1h", "1d", "1wk", "1mo"
    open_time TIMESTAMPTZ NOT NULL,       -- 캔들 시작 시간 (UTC)

    -- OHLCV 데이터
    open DECIMAL(30, 15) NOT NULL,
    high DECIMAL(30, 15) NOT NULL,
    low DECIMAL(30, 15) NOT NULL,
    close DECIMAL(30, 15) NOT NULL,
    volume DECIMAL(30, 15) NOT NULL,

    -- 메타데이터
    close_time TIMESTAMPTZ,               -- 캔들 종료 시간
    fetched_at TIMESTAMPTZ DEFAULT NOW(), -- 데이터 가져온 시간

    PRIMARY KEY (symbol, timeframe, open_time)
);

-- Convert to hypertable (시간 기반 파티셔닝)
SELECT create_hypertable('yahoo_candle_cache', 'open_time',
    chunk_time_interval => INTERVAL '1 month',
    if_not_exists => TRUE
);

-- 심볼 + 타임프레임으로 최신 데이터 조회용 인덱스
CREATE INDEX idx_yahoo_cache_symbol_tf_time
    ON yahoo_candle_cache(symbol, timeframe, open_time DESC);

-- 특정 심볼의 마지막 캐시 시간 조회용 인덱스
CREATE INDEX idx_yahoo_cache_last_fetch
    ON yahoo_candle_cache(symbol, timeframe, fetched_at DESC);

-- 압축 정책 (30일 이상 데이터 압축)
ALTER TABLE yahoo_candle_cache SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol, timeframe'
);
SELECT add_compression_policy('yahoo_candle_cache', INTERVAL '30 days', if_not_exists => TRUE);

-- 보존 정책: 분봉 데이터는 90일, 일봉 이상은 5년 보존
-- (TimescaleDB continuous aggregate 또는 별도 정책으로 관리)

-- =====================================================
-- CACHE METADATA TABLE
-- =====================================================

-- 심볼별/타임프레임별 캐시 상태 추적
CREATE TABLE yahoo_cache_metadata (
    symbol VARCHAR(50) NOT NULL,
    timeframe VARCHAR(10) NOT NULL,
    first_cached_time TIMESTAMPTZ,        -- 가장 오래된 캐시 데이터 시간
    last_cached_time TIMESTAMPTZ,         -- 가장 최근 캐시 데이터 시간
    last_updated_at TIMESTAMPTZ DEFAULT NOW(),  -- 마지막 업데이트 시간
    total_candles INT DEFAULT 0,          -- 총 캔들 수

    PRIMARY KEY (symbol, timeframe)
);

-- 메타데이터 업데이트 트리거 함수
CREATE OR REPLACE FUNCTION update_yahoo_cache_metadata()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO yahoo_cache_metadata (symbol, timeframe, first_cached_time, last_cached_time, total_candles)
    VALUES (NEW.symbol, NEW.timeframe, NEW.open_time, NEW.open_time, 1)
    ON CONFLICT (symbol, timeframe) DO UPDATE SET
        first_cached_time = LEAST(yahoo_cache_metadata.first_cached_time, NEW.open_time),
        last_cached_time = GREATEST(yahoo_cache_metadata.last_cached_time, NEW.open_time),
        last_updated_at = NOW(),
        total_candles = (
            SELECT COUNT(*) FROM yahoo_candle_cache
            WHERE symbol = NEW.symbol AND timeframe = NEW.timeframe
        );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- 캔들 삽입 시 메타데이터 자동 업데이트
CREATE TRIGGER trigger_update_yahoo_cache_metadata
    AFTER INSERT ON yahoo_candle_cache
    FOR EACH ROW
    EXECUTE FUNCTION update_yahoo_cache_metadata();

-- =====================================================
-- HELPER FUNCTIONS
-- =====================================================

-- 특정 심볼/타임프레임의 캐시 갭 확인 함수
CREATE OR REPLACE FUNCTION get_yahoo_cache_gaps(
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
    FROM yahoo_candle_cache
    WHERE symbol = p_symbol
      AND timeframe = p_timeframe
      AND open_time BETWEEN p_start_time AND p_end_time
    ORDER BY open_time;
END;
$$ LANGUAGE plpgsql;

-- 캐시 통계 조회 함수
CREATE OR REPLACE FUNCTION get_yahoo_cache_stats()
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
    FROM yahoo_cache_metadata m
    ORDER BY m.last_updated_at DESC;
END;
$$ LANGUAGE plpgsql;

-- =====================================================
-- COMMENTS
-- =====================================================

COMMENT ON TABLE yahoo_candle_cache IS
'Yahoo Finance에서 가져온 캔들 데이터 캐시. 전략/백테스팅/시뮬레이션/트레이딩에서 공통 사용.';

COMMENT ON TABLE yahoo_cache_metadata IS
'심볼/타임프레임별 캐시 상태 메타데이터. 증분 업데이트 시 마지막 시간 확인용.';

COMMENT ON COLUMN yahoo_candle_cache.symbol IS
'Yahoo Finance 심볼 (예: AAPL, 005930.KS, SPY)';

COMMENT ON COLUMN yahoo_candle_cache.timeframe IS
'캔들 간격: 1m, 5m, 15m, 30m, 1h, 1d, 1wk, 1mo';
