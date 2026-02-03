pg_dump: warning: there are circular foreign-key constraints on this table:
pg_dump: detail: hypertable
pg_dump: hint: You might not be able to restore the dump without using --disable-triggers or temporarily dropping the constraints.
pg_dump: hint: Consider using a full dump instead of a --data-only dump to avoid this problem.
pg_dump: warning: there are circular foreign-key constraints on this table:
pg_dump: detail: chunk
pg_dump: hint: You might not be able to restore the dump without using --disable-triggers or temporarily dropping the constraints.
pg_dump: hint: Consider using a full dump instead of a --data-only dump to avoid this problem.
pg_dump: warning: there are circular foreign-key constraints on this table:
pg_dump: detail: continuous_agg
pg_dump: hint: You might not be able to restore the dump without using --disable-triggers or temporarily dropping the constraints.
pg_dump: hint: Consider using a full dump instead of a --data-only dump to avoid this problem.
--
-- PostgreSQL database dump
--

-- Dumped from database version 15.13
-- Dumped by pg_dump version 15.13

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: timescaledb; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS timescaledb WITH SCHEMA public;


--
-- Name: EXTENSION timescaledb; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON EXTENSION timescaledb IS 'Enables scalable inserts and complex queries for time-series data (Community Edition)';


--
-- Name: uuid-ossp; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS "uuid-ossp" WITH SCHEMA public;


--
-- Name: EXTENSION "uuid-ossp"; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON EXTENSION "uuid-ossp" IS 'generate universally unique identifiers (UUIDs)';


--
-- Name: market_type; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.market_type AS ENUM (
    'crypto',
    'stock',
    'forex',
    'futures'
);


--
-- Name: order_side; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.order_side AS ENUM (
    'buy',
    'sell'
);


--
-- Name: order_status; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.order_status AS ENUM (
    'pending',
    'open',
    'partially_filled',
    'filled',
    'cancelled',
    'rejected',
    'expired'
);


--
-- Name: order_time_in_force; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.order_time_in_force AS ENUM (
    'gtc',
    'ioc',
    'fok',
    'day'
);


--
-- Name: order_type; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.order_type AS ENUM (
    'market',
    'limit',
    'stop',
    'stop_limit',
    'trailing_stop'
);


--
-- Name: signal_type; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.signal_type AS ENUM (
    'entry',
    'exit',
    'add_to_position',
    'reduce_position',
    'scale'
);


--
-- Name: calculate_reality_check(date, date); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.calculate_reality_check(p_recommend_date date, p_check_date date) RETURNS TABLE(symbol character varying, actual_return numeric, is_profitable boolean, processed_count integer)
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_processed INT := 0;
BEGIN
    -- 전일 스냅샷과 금일 가격을 조인하여 성과 계산
    INSERT INTO reality_check (
        check_date,
        recommend_date,
        symbol,
        recommend_source,
        recommend_rank,
        recommend_score,
        entry_price,
        exit_price,
        actual_return,
        is_profitable,
        entry_volume,
        exit_volume,
        volume_change,
        expected_return,
        return_error,
        market,
        sector
    )
    SELECT
        p_check_date,
        ps.snapshot_date,
        ps.symbol,
        ps.recommend_source,
        ps.recommend_rank,
        ps.recommend_score,
        ps.close_price AS entry_price,
        today.close AS exit_price,
        ROUND(((today.close - ps.close_price) / ps.close_price * 100)::NUMERIC, 4) AS actual_return,
        today.close >= ps.close_price AS is_profitable,
        ps.volume AS entry_volume,
        today.volume AS exit_volume,
        CASE
            WHEN ps.volume > 0 THEN ROUND(((today.volume::NUMERIC - ps.volume::NUMERIC) / ps.volume::NUMERIC * 100), 4)
            ELSE NULL
        END AS volume_change,
        ps.expected_return,
        CASE
            WHEN ps.expected_return IS NOT NULL
            THEN ROUND((((today.close - ps.close_price) / ps.close_price * 100) - ps.expected_return)::NUMERIC, 4)
            ELSE NULL
        END AS return_error,
        ps.market,
        ps.sector
    FROM price_snapshot ps
    INNER JOIN mv_latest_prices today ON ps.symbol = today.symbol
    WHERE ps.snapshot_date = p_recommend_date
        AND today.open_time::DATE = p_check_date
    ON CONFLICT (check_date, symbol) DO UPDATE SET
        exit_price = EXCLUDED.exit_price,
        actual_return = EXCLUDED.actual_return,
        is_profitable = EXCLUDED.is_profitable,
        exit_volume = EXCLUDED.exit_volume,
        volume_change = EXCLUDED.volume_change,
        return_error = EXCLUDED.return_error;

    GET DIAGNOSTICS v_processed = ROW_COUNT;

    RETURN QUERY
    SELECT
        rc.symbol,
        rc.actual_return,
        rc.is_profitable,
        v_processed
    FROM reality_check rc
    WHERE rc.check_date = p_check_date
    ORDER BY rc.actual_return DESC;
END;
$$;


--
-- Name: FUNCTION calculate_reality_check(p_recommend_date date, p_check_date date); Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON FUNCTION public.calculate_reality_check(p_recommend_date date, p_check_date date) IS '전일 추천 종목의 금일 성과를 자동 계산하여 reality_check 테이블에 저장';


--
-- Name: get_yahoo_cache_gaps(character varying, character varying, timestamp with time zone, timestamp with time zone); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.get_yahoo_cache_gaps(p_symbol character varying, p_timeframe character varying, p_start_time timestamp with time zone, p_end_time timestamp with time zone) RETURNS TABLE(gap_start timestamp with time zone, gap_end timestamp with time zone)
    LANGUAGE plpgsql
    AS $$
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
$$;


--
-- Name: get_yahoo_cache_stats(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.get_yahoo_cache_stats() RETURNS TABLE(symbol character varying, timeframe character varying, first_time timestamp with time zone, last_time timestamp with time zone, candle_count bigint, last_updated timestamp with time zone)
    LANGUAGE plpgsql
    AS $$
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
$$;


--
-- Name: record_symbol_fetch_failure(uuid, text, integer); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.record_symbol_fetch_failure(p_symbol_info_id uuid, p_error_message text, p_max_failures integer DEFAULT 3) RETURNS boolean
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_new_count INTEGER;
    v_deactivated BOOLEAN := FALSE;
BEGIN
    UPDATE symbol_info
    SET fetch_fail_count = fetch_fail_count + 1,
        last_fetch_error = p_error_message,
        last_fetch_attempt = NOW(),
        updated_at = NOW()
    WHERE id = p_symbol_info_id
    RETURNING fetch_fail_count INTO v_new_count;

    IF v_new_count >= p_max_failures THEN
        UPDATE symbol_info
        SET is_active = FALSE,
            updated_at = NOW()
        WHERE id = p_symbol_info_id;
        v_deactivated := TRUE;
    END IF;

    RETURN v_deactivated;
END;
$$;


--
-- Name: refresh_latest_prices(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.refresh_latest_prices() RETURNS void
    LANGUAGE plpgsql
    AS $$
BEGIN
    REFRESH MATERIALIZED VIEW CONCURRENTLY mv_latest_prices;
END;
$$;


--
-- Name: FUNCTION refresh_latest_prices(); Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON FUNCTION public.refresh_latest_prices() IS 'mv_latest_prices 뷰 갱신. 새 데이터 입력 후 호출.';


--
-- Name: reset_symbol_fetch_failure(uuid); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.reset_symbol_fetch_failure(p_symbol_info_id uuid) RETURNS void
    LANGUAGE plpgsql
    AS $$
BEGIN
    UPDATE symbol_info
    SET fetch_fail_count = 0,
        last_fetch_error = NULL,
        last_fetch_attempt = NOW(),
        updated_at = NOW()
    WHERE id = p_symbol_info_id;
END;
$$;


--
-- Name: update_exchange_credentials_updated_at(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.update_exchange_credentials_updated_at() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;


--
-- Name: update_ohlcv_metadata(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.update_ohlcv_metadata() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
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
$$;


--
-- Name: update_updated_at_column(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.update_updated_at_column() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;


--
-- Name: update_yahoo_cache_metadata(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.update_yahoo_cache_metadata() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
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
$$;


SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: _compressed_hypertable_2; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._compressed_hypertable_2 (
);


--
-- Name: _compressed_hypertable_4; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._compressed_hypertable_4 (
);


--
-- Name: _compressed_hypertable_7; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._compressed_hypertable_7 (
);


--
-- Name: klines; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.klines (
    "time" timestamp with time zone NOT NULL,
    symbol_id uuid NOT NULL,
    timeframe character varying(10) NOT NULL,
    open numeric(30,15) NOT NULL,
    high numeric(30,15) NOT NULL,
    low numeric(30,15) NOT NULL,
    close numeric(30,15) NOT NULL,
    volume numeric(30,15) NOT NULL,
    quote_volume numeric(30,15),
    num_trades integer
);


--
-- Name: _hyper_1_100_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_100_chunk (
    CONSTRAINT constraint_100 CHECK ((("time" >= '2024-10-17 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-10-24 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_101_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_101_chunk (
    CONSTRAINT constraint_101 CHECK ((("time" >= '2024-10-24 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-10-31 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_102_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_102_chunk (
    CONSTRAINT constraint_102 CHECK ((("time" >= '2024-10-31 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-11-07 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_103_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_103_chunk (
    CONSTRAINT constraint_103 CHECK ((("time" >= '2024-11-07 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-11-14 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_104_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_104_chunk (
    CONSTRAINT constraint_104 CHECK ((("time" >= '2024-11-14 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-11-21 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_105_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_105_chunk (
    CONSTRAINT constraint_105 CHECK ((("time" >= '2024-11-21 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-11-28 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_106_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_106_chunk (
    CONSTRAINT constraint_106 CHECK ((("time" >= '2024-11-28 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-12-05 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_107_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_107_chunk (
    CONSTRAINT constraint_107 CHECK ((("time" >= '2024-12-05 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-12-12 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_108_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_108_chunk (
    CONSTRAINT constraint_108 CHECK ((("time" >= '2024-12-12 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-12-19 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_109_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_109_chunk (
    CONSTRAINT constraint_109 CHECK ((("time" >= '2024-12-19 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-12-26 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_10_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_10_chunk (
    CONSTRAINT constraint_10 CHECK ((("time" >= '2025-03-06 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-03-13 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_110_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_110_chunk (
    CONSTRAINT constraint_110 CHECK ((("time" >= '2024-12-26 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-01-02 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_11_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_11_chunk (
    CONSTRAINT constraint_11 CHECK ((("time" >= '2025-03-13 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-03-20 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_12_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_12_chunk (
    CONSTRAINT constraint_12 CHECK ((("time" >= '2025-03-20 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-03-27 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_13_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_13_chunk (
    CONSTRAINT constraint_13 CHECK ((("time" >= '2025-03-27 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-04-03 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_14_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_14_chunk (
    CONSTRAINT constraint_14 CHECK ((("time" >= '2025-04-03 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-04-10 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_15_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_15_chunk (
    CONSTRAINT constraint_15 CHECK ((("time" >= '2025-04-10 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-04-17 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_16_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_16_chunk (
    CONSTRAINT constraint_16 CHECK ((("time" >= '2025-04-17 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-04-24 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_17_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_17_chunk (
    CONSTRAINT constraint_17 CHECK ((("time" >= '2025-04-24 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-05-01 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_18_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_18_chunk (
    CONSTRAINT constraint_18 CHECK ((("time" >= '2025-05-01 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-05-08 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_19_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_19_chunk (
    CONSTRAINT constraint_19 CHECK ((("time" >= '2025-05-08 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-05-15 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_1_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_1_chunk (
    CONSTRAINT constraint_1 CHECK ((("time" >= '2025-01-02 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-01-09 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_20_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_20_chunk (
    CONSTRAINT constraint_20 CHECK ((("time" >= '2025-05-15 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-05-22 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_21_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_21_chunk (
    CONSTRAINT constraint_21 CHECK ((("time" >= '2025-05-22 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-05-29 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_22_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_22_chunk (
    CONSTRAINT constraint_22 CHECK ((("time" >= '2025-05-29 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-06-05 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_23_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_23_chunk (
    CONSTRAINT constraint_23 CHECK ((("time" >= '2025-06-05 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-06-12 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_24_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_24_chunk (
    CONSTRAINT constraint_24 CHECK ((("time" >= '2025-06-12 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-06-19 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_25_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_25_chunk (
    CONSTRAINT constraint_25 CHECK ((("time" >= '2025-06-19 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-06-26 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_26_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_26_chunk (
    CONSTRAINT constraint_26 CHECK ((("time" >= '2025-06-26 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-07-03 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_27_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_27_chunk (
    CONSTRAINT constraint_27 CHECK ((("time" >= '2025-07-03 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-07-10 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_28_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_28_chunk (
    CONSTRAINT constraint_28 CHECK ((("time" >= '2025-07-10 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-07-17 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_29_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_29_chunk (
    CONSTRAINT constraint_29 CHECK ((("time" >= '2025-07-17 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-07-24 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_2_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_2_chunk (
    CONSTRAINT constraint_2 CHECK ((("time" >= '2025-01-09 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-01-16 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_30_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_30_chunk (
    CONSTRAINT constraint_30 CHECK ((("time" >= '2025-07-24 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-07-31 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_31_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_31_chunk (
    CONSTRAINT constraint_31 CHECK ((("time" >= '2025-07-31 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-08-07 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_32_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_32_chunk (
    CONSTRAINT constraint_32 CHECK ((("time" >= '2025-08-07 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-08-14 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_33_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_33_chunk (
    CONSTRAINT constraint_33 CHECK ((("time" >= '2025-08-14 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-08-21 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_34_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_34_chunk (
    CONSTRAINT constraint_34 CHECK ((("time" >= '2025-08-21 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-08-28 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_35_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_35_chunk (
    CONSTRAINT constraint_35 CHECK ((("time" >= '2025-08-28 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-09-04 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_36_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_36_chunk (
    CONSTRAINT constraint_36 CHECK ((("time" >= '2025-09-04 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-09-11 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_37_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_37_chunk (
    CONSTRAINT constraint_37 CHECK ((("time" >= '2025-09-11 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-09-18 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_38_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_38_chunk (
    CONSTRAINT constraint_38 CHECK ((("time" >= '2025-09-18 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-09-25 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_39_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_39_chunk (
    CONSTRAINT constraint_39 CHECK ((("time" >= '2025-09-25 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-10-02 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_3_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_3_chunk (
    CONSTRAINT constraint_3 CHECK ((("time" >= '2025-01-16 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-01-23 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_40_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_40_chunk (
    CONSTRAINT constraint_40 CHECK ((("time" >= '2025-10-02 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-10-09 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_41_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_41_chunk (
    CONSTRAINT constraint_41 CHECK ((("time" >= '2025-10-09 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-10-16 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_42_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_42_chunk (
    CONSTRAINT constraint_42 CHECK ((("time" >= '2025-10-16 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-10-23 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_43_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_43_chunk (
    CONSTRAINT constraint_43 CHECK ((("time" >= '2025-10-23 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-10-30 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_44_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_44_chunk (
    CONSTRAINT constraint_44 CHECK ((("time" >= '2025-10-30 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-11-06 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_45_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_45_chunk (
    CONSTRAINT constraint_45 CHECK ((("time" >= '2025-11-06 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-11-13 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_46_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_46_chunk (
    CONSTRAINT constraint_46 CHECK ((("time" >= '2025-11-13 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-11-20 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_47_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_47_chunk (
    CONSTRAINT constraint_47 CHECK ((("time" >= '2025-11-20 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-11-27 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_48_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_48_chunk (
    CONSTRAINT constraint_48 CHECK ((("time" >= '2025-11-27 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-12-04 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_49_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_49_chunk (
    CONSTRAINT constraint_49 CHECK ((("time" >= '2025-12-04 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-12-11 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_4_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_4_chunk (
    CONSTRAINT constraint_4 CHECK ((("time" >= '2025-01-23 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-01-30 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_50_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_50_chunk (
    CONSTRAINT constraint_50 CHECK ((("time" >= '2025-12-11 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-12-18 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_51_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_51_chunk (
    CONSTRAINT constraint_51 CHECK ((("time" >= '2025-12-18 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-12-25 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_52_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_52_chunk (
    CONSTRAINT constraint_52 CHECK ((("time" >= '2025-12-25 00:00:00+00'::timestamp with time zone) AND ("time" < '2026-01-01 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_53_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_53_chunk (
    CONSTRAINT constraint_53 CHECK ((("time" >= '2026-01-01 00:00:00+00'::timestamp with time zone) AND ("time" < '2026-01-08 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_54_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_54_chunk (
    CONSTRAINT constraint_54 CHECK ((("time" >= '2026-01-08 00:00:00+00'::timestamp with time zone) AND ("time" < '2026-01-15 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_55_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_55_chunk (
    CONSTRAINT constraint_55 CHECK ((("time" >= '2026-01-15 00:00:00+00'::timestamp with time zone) AND ("time" < '2026-01-22 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_56_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_56_chunk (
    CONSTRAINT constraint_56 CHECK ((("time" >= '2026-01-22 00:00:00+00'::timestamp with time zone) AND ("time" < '2026-01-29 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_57_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_57_chunk (
    CONSTRAINT constraint_57 CHECK ((("time" >= '2026-01-29 00:00:00+00'::timestamp with time zone) AND ("time" < '2026-02-05 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_5_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_5_chunk (
    CONSTRAINT constraint_5 CHECK ((("time" >= '2025-01-30 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-02-06 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_62_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_62_chunk (
    CONSTRAINT constraint_62 CHECK ((("time" >= '2024-01-25 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-02-01 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_63_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_63_chunk (
    CONSTRAINT constraint_63 CHECK ((("time" >= '2024-02-01 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-02-08 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_64_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_64_chunk (
    CONSTRAINT constraint_64 CHECK ((("time" >= '2024-02-08 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-02-15 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_65_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_65_chunk (
    CONSTRAINT constraint_65 CHECK ((("time" >= '2024-02-15 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-02-22 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_66_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_66_chunk (
    CONSTRAINT constraint_66 CHECK ((("time" >= '2024-02-22 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-02-29 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_67_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_67_chunk (
    CONSTRAINT constraint_67 CHECK ((("time" >= '2024-02-29 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-03-07 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_68_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_68_chunk (
    CONSTRAINT constraint_68 CHECK ((("time" >= '2024-03-07 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-03-14 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_69_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_69_chunk (
    CONSTRAINT constraint_69 CHECK ((("time" >= '2024-03-14 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-03-21 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_6_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_6_chunk (
    CONSTRAINT constraint_6 CHECK ((("time" >= '2025-02-06 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-02-13 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_70_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_70_chunk (
    CONSTRAINT constraint_70 CHECK ((("time" >= '2024-03-21 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-03-28 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_71_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_71_chunk (
    CONSTRAINT constraint_71 CHECK ((("time" >= '2024-03-28 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-04-04 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_72_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_72_chunk (
    CONSTRAINT constraint_72 CHECK ((("time" >= '2024-04-04 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-04-11 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_73_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_73_chunk (
    CONSTRAINT constraint_73 CHECK ((("time" >= '2024-04-11 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-04-18 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_74_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_74_chunk (
    CONSTRAINT constraint_74 CHECK ((("time" >= '2024-04-18 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-04-25 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_75_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_75_chunk (
    CONSTRAINT constraint_75 CHECK ((("time" >= '2024-04-25 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-05-02 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_76_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_76_chunk (
    CONSTRAINT constraint_76 CHECK ((("time" >= '2024-05-02 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-05-09 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_77_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_77_chunk (
    CONSTRAINT constraint_77 CHECK ((("time" >= '2024-05-09 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-05-16 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_78_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_78_chunk (
    CONSTRAINT constraint_78 CHECK ((("time" >= '2024-05-16 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-05-23 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_79_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_79_chunk (
    CONSTRAINT constraint_79 CHECK ((("time" >= '2024-05-23 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-05-30 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_7_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_7_chunk (
    CONSTRAINT constraint_7 CHECK ((("time" >= '2025-02-13 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-02-20 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_80_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_80_chunk (
    CONSTRAINT constraint_80 CHECK ((("time" >= '2024-05-30 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-06-06 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_81_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_81_chunk (
    CONSTRAINT constraint_81 CHECK ((("time" >= '2024-06-06 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-06-13 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_82_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_82_chunk (
    CONSTRAINT constraint_82 CHECK ((("time" >= '2024-06-13 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-06-20 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_83_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_83_chunk (
    CONSTRAINT constraint_83 CHECK ((("time" >= '2024-06-20 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-06-27 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_84_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_84_chunk (
    CONSTRAINT constraint_84 CHECK ((("time" >= '2024-06-27 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-07-04 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_85_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_85_chunk (
    CONSTRAINT constraint_85 CHECK ((("time" >= '2024-07-04 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-07-11 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_86_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_86_chunk (
    CONSTRAINT constraint_86 CHECK ((("time" >= '2024-07-11 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-07-18 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_87_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_87_chunk (
    CONSTRAINT constraint_87 CHECK ((("time" >= '2024-07-18 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-07-25 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_88_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_88_chunk (
    CONSTRAINT constraint_88 CHECK ((("time" >= '2024-07-25 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-08-01 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_89_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_89_chunk (
    CONSTRAINT constraint_89 CHECK ((("time" >= '2024-08-01 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-08-08 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_8_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_8_chunk (
    CONSTRAINT constraint_8 CHECK ((("time" >= '2025-02-20 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-02-27 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_90_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_90_chunk (
    CONSTRAINT constraint_90 CHECK ((("time" >= '2024-08-08 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-08-15 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_91_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_91_chunk (
    CONSTRAINT constraint_91 CHECK ((("time" >= '2024-08-15 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-08-22 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_92_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_92_chunk (
    CONSTRAINT constraint_92 CHECK ((("time" >= '2024-08-22 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-08-29 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_93_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_93_chunk (
    CONSTRAINT constraint_93 CHECK ((("time" >= '2024-08-29 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-09-05 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_94_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_94_chunk (
    CONSTRAINT constraint_94 CHECK ((("time" >= '2024-09-05 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-09-12 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_95_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_95_chunk (
    CONSTRAINT constraint_95 CHECK ((("time" >= '2024-09-12 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-09-19 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_96_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_96_chunk (
    CONSTRAINT constraint_96 CHECK ((("time" >= '2024-09-19 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-09-26 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_97_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_97_chunk (
    CONSTRAINT constraint_97 CHECK ((("time" >= '2024-09-26 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-10-03 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_98_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_98_chunk (
    CONSTRAINT constraint_98 CHECK ((("time" >= '2024-10-03 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-10-10 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_99_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_99_chunk (
    CONSTRAINT constraint_99 CHECK ((("time" >= '2024-10-10 00:00:00+00'::timestamp with time zone) AND ("time" < '2024-10-17 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: _hyper_1_9_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_1_9_chunk (
    CONSTRAINT constraint_9 CHECK ((("time" >= '2025-02-27 00:00:00+00'::timestamp with time zone) AND ("time" < '2025-03-06 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.klines);


--
-- Name: ohlcv; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.ohlcv (
    symbol character varying(50) NOT NULL,
    timeframe character varying(10) NOT NULL,
    open_time timestamp with time zone NOT NULL,
    open numeric(30,15) NOT NULL,
    high numeric(30,15) NOT NULL,
    low numeric(30,15) NOT NULL,
    close numeric(30,15) NOT NULL,
    volume numeric(30,15) NOT NULL,
    close_time timestamp with time zone,
    fetched_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE ohlcv; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.ohlcv IS 'Yahoo Finance에서 가져온 캔들 데이터 캐시. 전략/백테스팅/시뮬레이션/트레이딩에서 공통 사용.';


--
-- Name: COLUMN ohlcv.symbol; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.ohlcv.symbol IS 'Yahoo Finance 심볼 (예: AAPL, 005930.KS, SPY)';


--
-- Name: COLUMN ohlcv.timeframe; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.ohlcv.timeframe IS '캔들 간격: 1m, 5m, 15m, 30m, 1h, 1d, 1wk, 1mo';


--
-- Name: _hyper_6_111_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_111_chunk (
    CONSTRAINT constraint_111 CHECK (((open_time >= '2026-01-07 00:00:00+00'::timestamp with time zone) AND (open_time < '2026-02-06 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_392_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_392_chunk (
    CONSTRAINT constraint_292 CHECK (((open_time >= '2024-01-18 00:00:00+00'::timestamp with time zone) AND (open_time < '2024-02-17 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_393_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_393_chunk (
    CONSTRAINT constraint_293 CHECK (((open_time >= '2024-02-17 00:00:00+00'::timestamp with time zone) AND (open_time < '2024-03-18 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_394_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_394_chunk (
    CONSTRAINT constraint_294 CHECK (((open_time >= '2024-03-18 00:00:00+00'::timestamp with time zone) AND (open_time < '2024-04-17 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_395_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_395_chunk (
    CONSTRAINT constraint_295 CHECK (((open_time >= '2024-04-17 00:00:00+00'::timestamp with time zone) AND (open_time < '2024-05-17 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_396_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_396_chunk (
    CONSTRAINT constraint_296 CHECK (((open_time >= '2024-05-17 00:00:00+00'::timestamp with time zone) AND (open_time < '2024-06-16 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_397_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_397_chunk (
    CONSTRAINT constraint_297 CHECK (((open_time >= '2024-06-16 00:00:00+00'::timestamp with time zone) AND (open_time < '2024-07-16 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_398_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_398_chunk (
    CONSTRAINT constraint_298 CHECK (((open_time >= '2024-07-16 00:00:00+00'::timestamp with time zone) AND (open_time < '2024-08-15 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_399_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_399_chunk (
    CONSTRAINT constraint_299 CHECK (((open_time >= '2024-08-15 00:00:00+00'::timestamp with time zone) AND (open_time < '2024-09-14 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_400_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_400_chunk (
    CONSTRAINT constraint_300 CHECK (((open_time >= '2024-09-14 00:00:00+00'::timestamp with time zone) AND (open_time < '2024-10-14 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_401_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_401_chunk (
    CONSTRAINT constraint_301 CHECK (((open_time >= '2024-10-14 00:00:00+00'::timestamp with time zone) AND (open_time < '2024-11-13 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_402_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_402_chunk (
    CONSTRAINT constraint_302 CHECK (((open_time >= '2024-11-13 00:00:00+00'::timestamp with time zone) AND (open_time < '2024-12-13 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_403_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_403_chunk (
    CONSTRAINT constraint_303 CHECK (((open_time >= '2024-12-13 00:00:00+00'::timestamp with time zone) AND (open_time < '2025-01-12 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_404_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_404_chunk (
    CONSTRAINT constraint_304 CHECK (((open_time >= '2025-01-12 00:00:00+00'::timestamp with time zone) AND (open_time < '2025-02-11 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_405_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_405_chunk (
    CONSTRAINT constraint_305 CHECK (((open_time >= '2025-02-11 00:00:00+00'::timestamp with time zone) AND (open_time < '2025-03-13 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_406_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_406_chunk (
    CONSTRAINT constraint_306 CHECK (((open_time >= '2025-03-13 00:00:00+00'::timestamp with time zone) AND (open_time < '2025-04-12 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_407_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_407_chunk (
    CONSTRAINT constraint_307 CHECK (((open_time >= '2025-04-12 00:00:00+00'::timestamp with time zone) AND (open_time < '2025-05-12 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_408_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_408_chunk (
    CONSTRAINT constraint_308 CHECK (((open_time >= '2025-05-12 00:00:00+00'::timestamp with time zone) AND (open_time < '2025-06-11 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_409_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_409_chunk (
    CONSTRAINT constraint_309 CHECK (((open_time >= '2025-06-11 00:00:00+00'::timestamp with time zone) AND (open_time < '2025-07-11 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_410_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_410_chunk (
    CONSTRAINT constraint_310 CHECK (((open_time >= '2025-07-11 00:00:00+00'::timestamp with time zone) AND (open_time < '2025-08-10 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_411_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_411_chunk (
    CONSTRAINT constraint_311 CHECK (((open_time >= '2025-08-10 00:00:00+00'::timestamp with time zone) AND (open_time < '2025-09-09 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_412_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_412_chunk (
    CONSTRAINT constraint_312 CHECK (((open_time >= '2025-09-09 00:00:00+00'::timestamp with time zone) AND (open_time < '2025-10-09 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_413_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_413_chunk (
    CONSTRAINT constraint_313 CHECK (((open_time >= '2025-10-09 00:00:00+00'::timestamp with time zone) AND (open_time < '2025-11-08 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_414_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_414_chunk (
    CONSTRAINT constraint_314 CHECK (((open_time >= '2025-11-08 00:00:00+00'::timestamp with time zone) AND (open_time < '2025-12-08 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_415_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_415_chunk (
    CONSTRAINT constraint_315 CHECK (((open_time >= '2025-12-08 00:00:00+00'::timestamp with time zone) AND (open_time < '2026-01-07 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_547_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_547_chunk (
    CONSTRAINT constraint_316 CHECK (((open_time >= '2023-12-19 00:00:00+00'::timestamp with time zone) AND (open_time < '2024-01-18 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: _hyper_6_549_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal._hyper_6_549_chunk (
    CONSTRAINT constraint_317 CHECK (((open_time >= '2023-11-19 00:00:00+00'::timestamp with time zone) AND (open_time < '2023-12-19 00:00:00+00'::timestamp with time zone)))
)
INHERITS (public.ohlcv);


--
-- Name: compress_hyper_2_112_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_112_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_112_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_112_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_112_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_112_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_112_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_112_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_112_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_112_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_112_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_112_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_112_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_112_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_112_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_112_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_112_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_112_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_112_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_112_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_112_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_113_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_113_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_113_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_113_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_113_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_113_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_113_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_113_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_113_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_113_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_113_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_113_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_113_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_113_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_113_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_113_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_113_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_113_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_113_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_113_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_113_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_114_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_114_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_114_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_114_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_114_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_114_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_114_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_114_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_114_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_114_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_114_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_114_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_114_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_114_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_114_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_114_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_114_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_114_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_114_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_114_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_114_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_115_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_115_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_115_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_115_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_115_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_115_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_115_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_115_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_115_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_115_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_115_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_115_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_115_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_115_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_115_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_115_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_115_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_115_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_115_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_115_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_115_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_116_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_116_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_116_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_116_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_116_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_116_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_116_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_116_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_116_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_116_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_116_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_116_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_116_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_116_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_116_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_116_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_116_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_116_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_116_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_116_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_116_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_117_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_117_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_117_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_117_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_117_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_117_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_117_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_117_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_117_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_117_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_117_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_117_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_117_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_117_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_117_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_117_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_117_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_117_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_117_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_117_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_117_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_118_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_118_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_118_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_118_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_118_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_118_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_118_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_118_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_118_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_118_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_118_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_118_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_118_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_118_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_118_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_118_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_118_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_118_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_118_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_118_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_118_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_119_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_119_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_119_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_119_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_119_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_119_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_119_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_119_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_119_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_119_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_119_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_119_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_119_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_119_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_119_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_119_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_119_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_119_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_119_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_119_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_119_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_120_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_120_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_120_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_120_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_120_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_120_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_120_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_120_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_120_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_120_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_120_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_120_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_120_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_120_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_120_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_120_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_120_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_120_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_120_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_120_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_120_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_121_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_121_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_121_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_121_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_121_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_121_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_121_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_121_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_121_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_121_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_121_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_121_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_121_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_121_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_121_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_121_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_121_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_121_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_121_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_121_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_121_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_122_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_122_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_122_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_122_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_122_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_122_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_122_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_122_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_122_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_122_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_122_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_122_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_122_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_122_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_122_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_122_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_122_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_122_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_122_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_122_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_122_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_123_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_123_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_123_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_123_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_123_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_123_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_123_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_123_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_123_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_123_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_123_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_123_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_123_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_123_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_123_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_123_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_123_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_123_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_123_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_123_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_123_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_124_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_124_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_124_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_124_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_124_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_124_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_124_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_124_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_124_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_124_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_124_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_124_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_124_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_124_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_124_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_124_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_124_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_124_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_124_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_124_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_124_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_125_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_125_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_125_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_125_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_125_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_125_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_125_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_125_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_125_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_125_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_125_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_125_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_125_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_125_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_125_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_125_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_125_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_125_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_125_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_125_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_125_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_126_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_126_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_126_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_126_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_126_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_126_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_126_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_126_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_126_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_126_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_126_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_126_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_126_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_126_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_126_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_126_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_126_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_126_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_126_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_126_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_126_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_127_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_127_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_127_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_127_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_127_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_127_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_127_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_127_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_127_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_127_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_127_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_127_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_127_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_127_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_127_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_127_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_127_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_127_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_127_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_127_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_127_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_128_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_128_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_128_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_128_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_128_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_128_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_128_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_128_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_128_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_128_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_128_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_128_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_128_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_128_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_128_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_128_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_128_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_128_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_128_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_128_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_128_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_129_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_129_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_129_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_129_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_129_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_129_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_129_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_129_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_129_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_129_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_129_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_129_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_129_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_129_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_129_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_129_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_129_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_129_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_129_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_129_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_129_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_130_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_130_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_130_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_130_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_130_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_130_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_130_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_130_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_130_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_130_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_130_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_130_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_130_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_130_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_130_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_130_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_130_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_130_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_130_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_130_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_130_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_131_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_131_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_131_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_131_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_131_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_131_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_131_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_131_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_131_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_131_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_131_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_131_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_131_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_131_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_131_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_131_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_131_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_131_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_131_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_131_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_131_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_132_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_132_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_132_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_132_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_132_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_132_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_132_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_132_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_132_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_132_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_132_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_132_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_132_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_132_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_132_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_132_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_132_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_132_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_132_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_132_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_132_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_133_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_133_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_133_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_133_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_133_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_133_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_133_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_133_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_133_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_133_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_133_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_133_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_133_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_133_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_133_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_133_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_133_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_133_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_133_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_133_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_133_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_134_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_134_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_134_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_134_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_134_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_134_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_134_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_134_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_134_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_134_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_134_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_134_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_134_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_134_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_134_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_134_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_134_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_134_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_134_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_134_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_134_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_135_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_135_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_135_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_135_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_135_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_135_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_135_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_135_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_135_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_135_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_135_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_135_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_135_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_135_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_135_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_135_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_135_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_135_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_135_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_135_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_135_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_136_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_136_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_136_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_136_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_136_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_136_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_136_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_136_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_136_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_136_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_136_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_136_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_136_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_136_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_136_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_136_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_136_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_136_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_136_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_136_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_136_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_137_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_137_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_137_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_137_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_137_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_137_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_137_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_137_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_137_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_137_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_137_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_137_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_137_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_137_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_137_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_137_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_137_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_137_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_137_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_137_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_137_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_138_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_138_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_138_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_138_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_138_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_138_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_138_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_138_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_138_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_138_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_138_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_138_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_138_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_138_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_138_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_138_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_138_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_138_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_138_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_138_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_138_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_139_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_139_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_139_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_139_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_139_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_139_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_139_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_139_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_139_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_139_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_139_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_139_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_139_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_139_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_139_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_139_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_139_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_139_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_139_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_139_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_139_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_140_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_140_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_140_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_140_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_140_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_140_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_140_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_140_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_140_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_140_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_140_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_140_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_140_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_140_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_140_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_140_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_140_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_140_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_140_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_140_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_140_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_141_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_141_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_141_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_141_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_141_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_141_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_141_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_141_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_141_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_141_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_141_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_141_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_141_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_141_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_141_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_141_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_141_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_141_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_141_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_141_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_141_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_142_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_142_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_142_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_142_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_142_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_142_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_142_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_142_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_142_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_142_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_142_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_142_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_142_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_142_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_142_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_142_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_142_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_142_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_142_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_142_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_142_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_143_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_143_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_143_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_143_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_143_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_143_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_143_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_143_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_143_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_143_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_143_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_143_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_143_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_143_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_143_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_143_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_143_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_143_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_143_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_143_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_143_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_144_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_144_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_144_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_144_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_144_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_144_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_144_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_144_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_144_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_144_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_144_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_144_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_144_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_144_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_144_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_144_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_144_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_144_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_144_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_144_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_144_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_145_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_145_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_145_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_145_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_145_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_145_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_145_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_145_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_145_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_145_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_145_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_145_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_145_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_145_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_145_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_145_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_145_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_145_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_145_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_145_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_145_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_146_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_146_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_146_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_146_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_146_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_146_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_146_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_146_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_146_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_146_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_146_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_146_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_146_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_146_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_146_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_146_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_146_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_146_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_146_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_146_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_146_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_147_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_147_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_147_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_147_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_147_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_147_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_147_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_147_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_147_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_147_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_147_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_147_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_147_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_147_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_147_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_147_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_147_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_147_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_147_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_147_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_147_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_148_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_148_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_148_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_148_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_148_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_148_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_148_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_148_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_148_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_148_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_148_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_148_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_148_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_148_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_148_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_148_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_148_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_148_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_148_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_148_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_148_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_149_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_149_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_149_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_149_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_149_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_149_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_149_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_149_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_149_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_149_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_149_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_149_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_149_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_149_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_149_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_149_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_149_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_149_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_149_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_149_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_149_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_150_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_150_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_150_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_150_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_150_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_150_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_150_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_150_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_150_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_150_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_150_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_150_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_150_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_150_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_150_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_150_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_150_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_150_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_150_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_150_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_150_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_151_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_151_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_151_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_151_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_151_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_151_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_151_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_151_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_151_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_151_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_151_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_151_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_151_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_151_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_151_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_151_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_151_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_151_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_151_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_151_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_151_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_152_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_152_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_152_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_152_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_152_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_152_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_152_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_152_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_152_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_152_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_152_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_152_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_152_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_152_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_152_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_152_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_152_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_152_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_152_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_152_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_152_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_153_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_153_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_153_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_153_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_153_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_153_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_153_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_153_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_153_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_153_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_153_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_153_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_153_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_153_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_153_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_153_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_153_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_153_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_153_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_153_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_153_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_154_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_154_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_154_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_154_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_154_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_154_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_154_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_154_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_154_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_154_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_154_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_154_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_154_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_154_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_154_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_154_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_154_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_154_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_154_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_154_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_154_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_155_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_155_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_155_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_155_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_155_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_155_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_155_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_155_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_155_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_155_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_155_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_155_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_155_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_155_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_155_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_155_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_155_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_155_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_155_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_155_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_155_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_156_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_156_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_156_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_156_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_156_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_156_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_156_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_156_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_156_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_156_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_156_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_156_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_156_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_156_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_156_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_156_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_156_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_156_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_156_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_156_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_156_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_157_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_157_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_157_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_157_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_157_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_157_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_157_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_157_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_157_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_157_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_157_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_157_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_157_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_157_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_157_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_157_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_157_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_157_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_157_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_157_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_157_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_158_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_158_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_158_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_158_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_158_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_158_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_158_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_158_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_158_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_158_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_158_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_158_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_158_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_158_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_158_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_158_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_158_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_158_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_158_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_158_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_158_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_159_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_159_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_159_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_159_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_159_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_159_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_159_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_159_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_159_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_159_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_159_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_159_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_159_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_159_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_159_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_159_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_159_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_159_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_159_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_159_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_159_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_160_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_160_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_160_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_160_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_160_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_160_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_160_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_160_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_160_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_160_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_160_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_160_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_160_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_160_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_160_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_160_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_160_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_160_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_160_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_160_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_160_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_161_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_161_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_161_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_161_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_161_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_161_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_161_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_161_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_161_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_161_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_161_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_161_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_161_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_161_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_161_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_161_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_161_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_161_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_161_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_161_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_161_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_162_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_162_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_162_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_162_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_162_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_162_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_162_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_162_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_162_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_162_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_162_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_162_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_162_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_162_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_162_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_162_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_162_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_162_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_162_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_162_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_162_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_163_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_163_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_163_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_163_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_163_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_163_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_163_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_163_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_163_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_163_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_163_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_163_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_163_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_163_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_163_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_163_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_163_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_163_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_163_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_163_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_163_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_164_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_164_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_164_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_164_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_164_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_164_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_164_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_164_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_164_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_164_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_164_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_164_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_164_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_164_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_164_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_164_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_164_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_164_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_164_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_164_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_164_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_165_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_165_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_165_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_165_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_165_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_165_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_165_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_165_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_165_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_165_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_165_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_165_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_165_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_165_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_165_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_165_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_165_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_165_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_165_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_165_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_165_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_166_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_166_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_166_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_166_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_166_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_166_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_166_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_166_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_166_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_166_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_166_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_166_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_166_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_166_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_166_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_166_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_166_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_166_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_166_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_166_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_166_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_167_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_167_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_167_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_167_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_167_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_167_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_167_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_167_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_167_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_167_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_167_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_167_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_167_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_167_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_167_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_167_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_167_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_167_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_167_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_167_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_167_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_168_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_168_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_168_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_168_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_168_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_168_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_168_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_168_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_168_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_168_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_168_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_168_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_168_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_168_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_168_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_168_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_168_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_168_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_168_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_168_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_168_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_169_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_169_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_169_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_169_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_169_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_169_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_169_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_169_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_169_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_169_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_169_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_169_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_169_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_169_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_169_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_169_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_169_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_169_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_169_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_169_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_169_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_170_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_170_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_170_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_170_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_170_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_170_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_170_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_170_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_170_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_170_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_170_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_170_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_170_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_170_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_170_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_170_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_170_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_170_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_170_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_170_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_170_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_171_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_171_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_171_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_171_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_171_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_171_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_171_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_171_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_171_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_171_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_171_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_171_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_171_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_171_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_171_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_171_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_171_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_171_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_171_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_171_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_171_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_172_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_172_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_172_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_172_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_172_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_172_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_172_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_172_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_172_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_172_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_172_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_172_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_172_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_172_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_172_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_172_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_172_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_172_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_172_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_172_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_172_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_173_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_173_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_173_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_173_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_173_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_173_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_173_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_173_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_173_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_173_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_173_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_173_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_173_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_173_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_173_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_173_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_173_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_173_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_173_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_173_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_173_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_174_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_174_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_174_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_174_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_174_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_174_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_174_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_174_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_174_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_174_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_174_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_174_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_174_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_174_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_174_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_174_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_174_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_174_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_174_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_174_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_174_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_175_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_175_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_175_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_175_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_175_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_175_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_175_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_175_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_175_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_175_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_175_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_175_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_175_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_175_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_175_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_175_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_175_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_175_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_175_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_175_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_175_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_176_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_176_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_176_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_176_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_176_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_176_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_176_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_176_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_176_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_176_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_176_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_176_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_176_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_176_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_176_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_176_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_176_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_176_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_176_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_176_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_176_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_177_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_177_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_177_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_177_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_177_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_177_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_177_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_177_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_177_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_177_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_177_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_177_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_177_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_177_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_177_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_177_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_177_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_177_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_177_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_177_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_177_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_178_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_178_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_178_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_178_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_178_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_178_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_178_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_178_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_178_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_178_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_178_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_178_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_178_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_178_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_178_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_178_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_178_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_178_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_178_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_178_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_178_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_179_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_179_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_179_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_179_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_179_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_179_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_179_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_179_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_179_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_179_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_179_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_179_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_179_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_179_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_179_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_179_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_179_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_179_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_179_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_179_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_179_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_180_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_180_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_180_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_180_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_180_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_180_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_180_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_180_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_180_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_180_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_180_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_180_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_180_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_180_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_180_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_180_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_180_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_180_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_180_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_180_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_180_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_181_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_181_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_181_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_181_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_181_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_181_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_181_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_181_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_181_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_181_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_181_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_181_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_181_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_181_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_181_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_181_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_181_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_181_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_181_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_181_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_181_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_182_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_182_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_182_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_182_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_182_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_182_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_182_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_182_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_182_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_182_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_182_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_182_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_182_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_182_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_182_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_182_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_182_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_182_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_182_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_182_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_182_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_183_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_183_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_183_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_183_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_183_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_183_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_183_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_183_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_183_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_183_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_183_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_183_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_183_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_183_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_183_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_183_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_183_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_183_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_183_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_183_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_183_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_184_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_184_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_184_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_184_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_184_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_184_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_184_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_184_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_184_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_184_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_184_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_184_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_184_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_184_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_184_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_184_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_184_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_184_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_184_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_184_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_184_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_185_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_185_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_185_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_185_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_185_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_185_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_185_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_185_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_185_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_185_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_185_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_185_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_185_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_185_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_185_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_185_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_185_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_185_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_185_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_185_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_185_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_186_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_186_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_186_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_186_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_186_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_186_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_186_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_186_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_186_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_186_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_186_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_186_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_186_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_186_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_186_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_186_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_186_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_186_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_186_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_186_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_186_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_187_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_187_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_187_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_187_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_187_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_187_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_187_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_187_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_187_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_187_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_187_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_187_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_187_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_187_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_187_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_187_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_187_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_187_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_187_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_187_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_187_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_188_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_188_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_188_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_188_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_188_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_188_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_188_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_188_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_188_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_188_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_188_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_188_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_188_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_188_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_188_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_188_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_188_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_188_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_188_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_188_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_188_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_189_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_189_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_189_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_189_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_189_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_189_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_189_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_189_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_189_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_189_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_189_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_189_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_189_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_189_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_189_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_189_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_189_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_189_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_189_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_189_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_189_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_190_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_190_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_190_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_190_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_190_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_190_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_190_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_190_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_190_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_190_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_190_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_190_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_190_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_190_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_190_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_190_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_190_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_190_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_190_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_190_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_190_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_191_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_191_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_191_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_191_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_191_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_191_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_191_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_191_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_191_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_191_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_191_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_191_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_191_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_191_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_191_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_191_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_191_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_191_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_191_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_191_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_191_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_192_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_192_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_192_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_192_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_192_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_192_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_192_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_192_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_192_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_192_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_192_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_192_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_192_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_192_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_192_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_192_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_192_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_192_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_192_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_192_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_192_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_193_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_193_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_193_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_193_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_193_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_193_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_193_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_193_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_193_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_193_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_193_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_193_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_193_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_193_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_193_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_193_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_193_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_193_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_193_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_193_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_193_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_194_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_194_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_194_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_194_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_194_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_194_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_194_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_194_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_194_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_194_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_194_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_194_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_194_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_194_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_194_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_194_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_194_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_194_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_194_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_194_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_194_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_195_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_195_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_195_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_195_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_195_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_195_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_195_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_195_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_195_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_195_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_195_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_195_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_195_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_195_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_195_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_195_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_195_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_195_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_195_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_195_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_195_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_196_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_196_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_196_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_196_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_196_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_196_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_196_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_196_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_196_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_196_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_196_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_196_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_196_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_196_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_196_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_196_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_196_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_196_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_196_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_196_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_196_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_197_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_197_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_197_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_197_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_197_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_197_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_197_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_197_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_197_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_197_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_197_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_197_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_197_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_197_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_197_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_197_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_197_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_197_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_197_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_197_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_197_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_198_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_198_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_198_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_198_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_198_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_198_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_198_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_198_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_198_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_198_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_198_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_198_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_198_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_198_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_198_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_198_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_198_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_198_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_198_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_198_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_198_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_199_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_199_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_199_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_199_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_199_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_199_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_199_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_199_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_199_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_199_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_199_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_199_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_199_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_199_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_199_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_199_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_199_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_199_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_199_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_199_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_199_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_200_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_200_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_200_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_200_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_200_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_200_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_200_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_200_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_200_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_200_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_200_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_200_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_200_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_200_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_200_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_200_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_200_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_200_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_200_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_200_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_200_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_201_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_201_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_201_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_201_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_201_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_201_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_201_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_201_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_201_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_201_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_201_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_201_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_201_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_201_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_201_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_201_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_201_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_201_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_201_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_201_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_201_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_202_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_202_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_202_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_202_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_202_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_202_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_202_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_202_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_202_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_202_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_202_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_202_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_202_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_202_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_202_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_202_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_202_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_202_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_202_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_202_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_202_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_203_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_203_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_203_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_203_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_203_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_203_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_203_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_203_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_203_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_203_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_203_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_203_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_203_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_203_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_203_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_203_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_203_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_203_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_203_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_203_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_203_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_204_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_204_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_204_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_204_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_204_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_204_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_204_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_204_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_204_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_204_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_204_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_204_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_204_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_204_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_204_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_204_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_204_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_204_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_204_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_204_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_204_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_205_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_205_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_205_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_205_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_205_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_205_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_205_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_205_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_205_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_205_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_205_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_205_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_205_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_205_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_205_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_205_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_205_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_205_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_205_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_205_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_205_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_206_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_206_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_206_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_206_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_206_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_206_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_206_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_206_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_206_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_206_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_206_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_206_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_206_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_206_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_206_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_206_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_206_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_206_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_206_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_206_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_206_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_207_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_207_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_207_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_207_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_207_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_207_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_207_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_207_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_207_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_207_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_207_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_207_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_207_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_207_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_207_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_207_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_207_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_207_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_207_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_207_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_207_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_208_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_208_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_208_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_208_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_208_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_208_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_208_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_208_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_208_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_208_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_208_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_208_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_208_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_208_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_208_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_208_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_208_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_208_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_208_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_208_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_208_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_209_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_209_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_209_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_209_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_209_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_209_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_209_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_209_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_209_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_209_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_209_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_209_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_209_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_209_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_209_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_209_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_209_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_209_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_209_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_209_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_209_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_210_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_210_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_210_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_210_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_210_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_210_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_210_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_210_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_210_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_210_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_210_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_210_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_210_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_210_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_210_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_210_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_210_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_210_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_210_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_210_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_210_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_211_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_211_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_211_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_211_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_211_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_211_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_211_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_211_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_211_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_211_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_211_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_211_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_211_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_211_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_211_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_211_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_211_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_211_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_211_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_211_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_211_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_2_551_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_2_551_chunk (
    _ts_meta_count integer,
    symbol_id uuid,
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    "time" _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    quote_volume _timescaledb_internal.compressed_data,
    num_trades _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_551_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_551_chunk ALTER COLUMN symbol_id SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_551_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_551_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_551_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_551_chunk ALTER COLUMN "time" SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_551_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_551_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_551_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_551_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_551_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_551_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_551_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_551_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_551_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_551_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_551_chunk ALTER COLUMN quote_volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_551_chunk ALTER COLUMN quote_volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_2_551_chunk ALTER COLUMN num_trades SET STATISTICS 0;


--
-- Name: compress_hyper_7_416_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_416_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_416_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_416_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_416_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_416_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_416_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_416_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_416_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_416_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_416_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_416_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_416_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_416_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_416_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_416_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_416_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_416_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_416_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_416_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_416_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_416_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_417_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_417_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_417_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_417_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_417_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_417_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_417_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_417_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_417_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_417_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_417_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_417_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_417_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_417_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_417_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_417_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_417_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_417_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_417_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_417_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_417_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_417_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_418_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_418_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_418_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_418_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_418_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_418_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_418_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_418_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_418_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_418_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_418_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_418_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_418_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_418_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_418_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_418_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_418_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_418_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_418_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_418_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_418_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_418_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_419_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_419_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_419_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_419_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_419_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_419_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_419_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_419_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_419_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_419_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_419_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_419_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_419_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_419_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_419_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_419_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_419_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_419_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_419_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_419_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_419_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_419_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_420_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_420_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_420_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_420_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_420_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_420_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_420_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_420_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_420_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_420_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_420_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_420_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_420_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_420_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_420_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_420_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_420_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_420_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_420_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_420_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_420_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_420_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_421_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_421_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_421_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_421_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_421_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_421_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_421_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_421_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_421_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_421_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_421_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_421_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_421_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_421_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_421_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_421_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_421_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_421_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_421_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_421_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_421_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_421_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_422_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_422_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_422_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_422_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_422_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_422_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_422_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_422_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_422_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_422_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_422_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_422_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_422_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_422_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_422_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_422_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_422_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_422_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_422_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_422_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_422_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_422_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_423_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_423_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_423_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_423_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_423_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_423_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_423_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_423_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_423_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_423_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_423_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_423_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_423_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_423_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_423_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_423_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_423_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_423_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_423_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_423_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_423_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_423_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_424_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_424_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_424_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_424_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_424_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_424_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_424_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_424_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_424_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_424_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_424_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_424_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_424_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_424_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_424_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_424_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_424_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_424_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_424_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_424_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_424_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_424_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_425_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_425_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_425_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_425_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_425_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_425_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_425_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_425_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_425_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_425_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_425_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_425_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_425_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_425_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_425_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_425_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_425_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_425_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_425_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_425_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_425_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_425_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_426_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_426_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_426_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_426_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_426_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_426_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_426_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_426_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_426_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_426_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_426_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_426_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_426_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_426_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_426_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_426_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_426_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_426_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_426_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_426_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_426_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_426_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_427_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_427_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_427_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_427_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_427_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_427_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_427_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_427_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_427_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_427_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_427_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_427_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_427_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_427_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_427_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_427_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_427_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_427_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_427_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_427_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_427_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_427_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_428_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_428_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_428_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_428_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_428_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_428_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_428_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_428_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_428_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_428_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_428_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_428_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_428_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_428_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_428_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_428_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_428_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_428_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_428_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_428_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_428_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_428_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_429_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_429_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_429_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_429_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_429_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_429_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_429_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_429_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_429_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_429_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_429_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_429_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_429_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_429_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_429_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_429_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_429_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_429_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_429_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_429_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_429_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_429_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_430_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_430_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_430_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_430_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_430_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_430_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_430_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_430_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_430_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_430_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_430_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_430_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_430_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_430_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_430_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_430_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_430_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_430_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_430_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_430_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_430_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_430_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_431_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_431_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_431_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_431_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_431_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_431_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_431_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_431_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_431_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_431_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_431_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_431_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_431_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_431_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_431_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_431_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_431_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_431_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_431_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_431_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_431_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_431_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_432_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_432_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_432_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_432_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_432_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_432_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_432_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_432_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_432_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_432_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_432_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_432_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_432_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_432_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_432_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_432_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_432_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_432_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_432_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_432_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_432_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_432_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_433_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_433_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_433_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_433_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_433_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_433_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_433_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_433_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_433_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_433_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_433_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_433_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_433_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_433_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_433_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_433_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_433_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_433_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_433_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_433_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_433_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_433_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_434_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_434_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_434_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_434_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_434_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_434_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_434_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_434_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_434_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_434_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_434_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_434_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_434_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_434_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_434_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_434_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_434_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_434_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_434_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_434_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_434_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_434_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_435_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_435_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_435_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_435_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_435_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_435_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_435_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_435_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_435_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_435_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_435_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_435_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_435_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_435_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_435_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_435_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_435_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_435_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_435_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_435_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_435_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_435_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_436_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_436_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_436_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_436_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_436_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_436_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_436_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_436_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_436_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_436_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_436_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_436_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_436_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_436_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_436_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_436_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_436_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_436_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_436_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_436_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_436_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_436_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_437_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_437_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_437_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_437_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_437_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_437_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_437_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_437_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_437_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_437_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_437_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_437_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_437_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_437_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_437_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_437_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_437_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_437_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_437_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_437_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_437_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_437_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_438_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_438_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_438_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_438_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_438_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_438_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_438_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_438_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_438_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_438_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_438_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_438_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_438_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_438_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_438_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_438_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_438_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_438_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_438_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_438_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_438_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_438_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_548_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_548_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_548_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_548_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_548_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_548_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_548_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_548_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_548_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_548_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_548_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_548_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_548_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_548_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_548_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_548_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_548_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_548_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_548_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_548_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_548_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_548_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: compress_hyper_7_550_chunk; Type: TABLE; Schema: _timescaledb_internal; Owner: -
--

CREATE TABLE _timescaledb_internal.compress_hyper_7_550_chunk (
    _ts_meta_count integer,
    symbol character varying(50),
    timeframe character varying(10),
    _ts_meta_min_1 timestamp with time zone,
    _ts_meta_max_1 timestamp with time zone,
    open_time _timescaledb_internal.compressed_data,
    open _timescaledb_internal.compressed_data,
    high _timescaledb_internal.compressed_data,
    low _timescaledb_internal.compressed_data,
    close _timescaledb_internal.compressed_data,
    volume _timescaledb_internal.compressed_data,
    close_time _timescaledb_internal.compressed_data,
    _ts_meta_v2_min_fetched_at timestamp with time zone,
    _ts_meta_v2_max_fetched_at timestamp with time zone,
    fetched_at _timescaledb_internal.compressed_data
)
WITH (toast_tuple_target='128');
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_550_chunk ALTER COLUMN _ts_meta_count SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_550_chunk ALTER COLUMN symbol SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_550_chunk ALTER COLUMN timeframe SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_550_chunk ALTER COLUMN _ts_meta_min_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_550_chunk ALTER COLUMN _ts_meta_max_1 SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_550_chunk ALTER COLUMN open_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_550_chunk ALTER COLUMN open SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_550_chunk ALTER COLUMN open SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_550_chunk ALTER COLUMN high SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_550_chunk ALTER COLUMN high SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_550_chunk ALTER COLUMN low SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_550_chunk ALTER COLUMN low SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_550_chunk ALTER COLUMN close SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_550_chunk ALTER COLUMN close SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_550_chunk ALTER COLUMN volume SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_550_chunk ALTER COLUMN volume SET STORAGE EXTENDED;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_550_chunk ALTER COLUMN close_time SET STATISTICS 0;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_550_chunk ALTER COLUMN _ts_meta_v2_min_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_550_chunk ALTER COLUMN _ts_meta_v2_max_fetched_at SET STATISTICS 1000;
ALTER TABLE ONLY _timescaledb_internal.compress_hyper_7_550_chunk ALTER COLUMN fetched_at SET STATISTICS 0;


--
-- Name: api_keys; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.api_keys (
    id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    user_id uuid,
    exchange character varying(50) NOT NULL,
    name character varying(100) NOT NULL,
    api_key_encrypted bytea NOT NULL,
    api_secret_encrypted bytea NOT NULL,
    passphrase_encrypted bytea,
    permissions jsonb DEFAULT '["read"]'::jsonb,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    last_used_at timestamp with time zone
);


--
-- Name: app_settings; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.app_settings (
    setting_key character varying(100) NOT NULL,
    setting_value text DEFAULT ''::text NOT NULL,
    description text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: audit_logs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.audit_logs (
    id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    event_type character varying(50) NOT NULL,
    entity_type character varying(50),
    entity_id uuid,
    user_id character varying(100),
    details jsonb DEFAULT '{}'::jsonb,
    ip_address inet,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: backtest_results; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.backtest_results (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    strategy_id character varying(100) NOT NULL,
    strategy_type character varying(50) NOT NULL,
    symbol character varying(500) NOT NULL,
    start_date date NOT NULL,
    end_date date NOT NULL,
    initial_capital numeric(20,2) NOT NULL,
    slippage_rate numeric(10,6) DEFAULT 0.0005,
    metrics jsonb NOT NULL,
    config_summary jsonb NOT NULL,
    equity_curve jsonb DEFAULT '[]'::jsonb NOT NULL,
    trades jsonb DEFAULT '[]'::jsonb NOT NULL,
    success boolean DEFAULT true NOT NULL,
    error_message text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone
);


--
-- Name: TABLE backtest_results; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.backtest_results IS '백테스트 결과 저장 테이블. 전략별 백테스트 수행 결과를 영구 저장합니다.';


--
-- Name: COLUMN backtest_results.id; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.backtest_results.id IS '결과 고유 ID (UUID)';


--
-- Name: COLUMN backtest_results.strategy_id; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.backtest_results.strategy_id IS '전략 ID (strategies 테이블 참조)';


--
-- Name: COLUMN backtest_results.strategy_type; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.backtest_results.strategy_type IS '전략 타입 (sma_crossover, bollinger, haa 등)';


--
-- Name: COLUMN backtest_results.symbol; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.backtest_results.symbol IS '백테스트 대상 심볼. 다중 자산 전략은 콤마로 구분';


--
-- Name: COLUMN backtest_results.metrics; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.backtest_results.metrics IS '성과 지표 JSON: total_return_pct, annualized_return_pct, max_drawdown_pct, sharpe_ratio 등';


--
-- Name: COLUMN backtest_results.equity_curve; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.backtest_results.equity_curve IS '자산 곡선 JSON 배열: [{timestamp, equity, drawdown_pct}, ...]';


--
-- Name: COLUMN backtest_results.trades; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.backtest_results.trades IS '거래 내역 JSON 배열: [{symbol, side, entry_price, exit_price, quantity, pnl, return_pct}, ...]';


--
-- Name: COLUMN backtest_results.deleted_at; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.backtest_results.deleted_at IS '소프트 삭제 시간. NULL이면 활성 상태';


--
-- Name: credential_access_logs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.credential_access_logs (
    id bigint NOT NULL,
    credential_type character varying(50) NOT NULL,
    credential_id uuid NOT NULL,
    action character varying(50) NOT NULL,
    accessor_ip character varying(45),
    user_agent text,
    success boolean DEFAULT true NOT NULL,
    error_message text,
    accessed_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: TABLE credential_access_logs; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.credential_access_logs IS '자격증명 접근 감사 로그';


--
-- Name: credential_access_logs_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.credential_access_logs_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: credential_access_logs_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.credential_access_logs_id_seq OWNED BY public.credential_access_logs.id;


--
-- Name: exchange_credentials; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.exchange_credentials (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    exchange_id character varying(50) NOT NULL,
    exchange_name character varying(100) NOT NULL,
    market_type character varying(20) NOT NULL,
    encrypted_credentials bytea NOT NULL,
    encryption_version integer DEFAULT 1 NOT NULL,
    encryption_nonce bytea NOT NULL,
    is_active boolean DEFAULT true NOT NULL,
    is_testnet boolean DEFAULT false NOT NULL,
    permissions jsonb,
    settings jsonb DEFAULT '{}'::jsonb,
    last_used_at timestamp with time zone,
    last_verified_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: TABLE exchange_credentials; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.exchange_credentials IS '거래소 API 자격증명 (AES-256-GCM 암호화)';


--
-- Name: COLUMN exchange_credentials.encrypted_credentials; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.exchange_credentials.encrypted_credentials IS 'AES-256-GCM으로 암호화된 JSON (api_key, api_secret 등)';


--
-- Name: COLUMN exchange_credentials.encryption_nonce; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.exchange_credentials.encryption_nonce IS 'AES-GCM nonce (12바이트, 각 암호화마다 고유)';


--
-- Name: execution_cache; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.execution_cache (
    id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    credential_id uuid NOT NULL,
    exchange character varying(50) NOT NULL,
    executed_at timestamp with time zone NOT NULL,
    symbol character varying(50) NOT NULL,
    normalized_symbol character varying(50),
    side character varying(10) NOT NULL,
    quantity numeric(30,15) NOT NULL,
    price numeric(30,15) NOT NULL,
    amount numeric(30,15) NOT NULL,
    fee numeric(30,15),
    fee_currency character varying(20),
    order_id character varying(100) NOT NULL,
    trade_id character varying(100),
    order_type character varying(20),
    raw_data jsonb,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE execution_cache; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.execution_cache IS '체결 내역 캐시 - 거래소 중립적 증분 조회 지원';


--
-- Name: COLUMN execution_cache.exchange; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.execution_cache.exchange IS '거래소 식별자 (kis, binance, coinbase 등)';


--
-- Name: COLUMN execution_cache.normalized_symbol; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.execution_cache.normalized_symbol IS '정규화된 심볼 형식 (BTC/USDT, 005930.KS)';


--
-- Name: execution_cache_meta; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.execution_cache_meta (
    id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    credential_id uuid NOT NULL,
    exchange character varying(50) NOT NULL,
    earliest_date date,
    latest_date date,
    total_records integer DEFAULT 0,
    last_sync_at timestamp with time zone,
    last_sync_status character varying(20),
    last_sync_message text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE execution_cache_meta; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.execution_cache_meta IS '체결 캐시 메타데이터 - 거래소별 마지막 동기화 정보';


--
-- Name: COLUMN execution_cache_meta.latest_date; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.execution_cache_meta.latest_date IS '가장 최근 캐시된 일자 - 다음 조회 시작점';


--
-- Name: position_snapshots; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.position_snapshots (
    id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    credential_id uuid NOT NULL,
    snapshot_time timestamp with time zone DEFAULT now() NOT NULL,
    exchange character varying(50) NOT NULL,
    symbol character varying(50) NOT NULL,
    symbol_name character varying(100),
    side public.order_side NOT NULL,
    quantity numeric(30,15) NOT NULL,
    entry_price numeric(30,15) NOT NULL,
    current_price numeric(30,15),
    cost_basis numeric(30,15) NOT NULL,
    market_value numeric(30,15),
    unrealized_pnl numeric(30,15) DEFAULT 0,
    unrealized_pnl_pct numeric(10,4) DEFAULT 0,
    realized_pnl numeric(30,15) DEFAULT 0,
    weight_pct numeric(10,4),
    first_trade_at timestamp with time zone,
    last_trade_at timestamp with time zone,
    trade_count integer DEFAULT 0,
    strategy_id character varying(100),
    metadata jsonb DEFAULT '{}'::jsonb,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE position_snapshots; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.position_snapshots IS '포지션 스냅샷. 시간별 포지션 상태를 기록하여 포지션 변화 추적.';


--
-- Name: COLUMN position_snapshots.entry_price; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.position_snapshots.entry_price IS '가중평균 매입가. (sum(price * quantity) / sum(quantity))';


--
-- Name: COLUMN position_snapshots.weight_pct; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.position_snapshots.weight_pct IS '포트폴리오 내 비중. 총 자산 대비 해당 종목 비율.';


--
-- Name: journal_current_positions; Type: VIEW; Schema: public; Owner: -
--

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


--
-- Name: trade_executions; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.trade_executions (
    id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    credential_id uuid NOT NULL,
    exchange character varying(50) NOT NULL,
    symbol character varying(50) NOT NULL,
    symbol_name character varying(100),
    side public.order_side NOT NULL,
    order_type public.order_type NOT NULL,
    quantity numeric(30,15) NOT NULL,
    price numeric(30,15) NOT NULL,
    notional_value numeric(30,15) NOT NULL,
    fee numeric(30,15) DEFAULT 0,
    fee_currency character varying(20),
    position_effect character varying(20),
    realized_pnl numeric(30,15),
    order_id uuid,
    exchange_order_id character varying(100),
    exchange_trade_id character varying(100),
    strategy_id character varying(100),
    strategy_name character varying(200),
    executed_at timestamp with time zone NOT NULL,
    memo text,
    tags jsonb DEFAULT '[]'::jsonb,
    metadata jsonb DEFAULT '{}'::jsonb,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE trade_executions; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.trade_executions IS '매매일지용 체결 내역. 거래 기록과 메모, 태그를 저장하여 트레이딩 분석 지원.';


--
-- Name: COLUMN trade_executions.position_effect; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.trade_executions.position_effect IS '포지션 영향: open(신규진입), close(청산), add(추가매수), reduce(부분청산)';


--
-- Name: COLUMN trade_executions.tags; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.trade_executions.tags IS '사용자 정의 태그. 예: ["손절", "스윙", "단타"]';


--
-- Name: journal_daily_summary; Type: VIEW; Schema: public; Owner: -
--

CREATE VIEW public.journal_daily_summary AS
 SELECT trade_executions.credential_id,
    date(trade_executions.executed_at) AS trade_date,
    count(*) AS total_trades,
    count(*) FILTER (WHERE (trade_executions.side = 'buy'::public.order_side)) AS buy_count,
    count(*) FILTER (WHERE (trade_executions.side = 'sell'::public.order_side)) AS sell_count,
    sum(trade_executions.notional_value) AS total_volume,
    sum(trade_executions.fee) AS total_fees,
    sum(trade_executions.realized_pnl) FILTER (WHERE (trade_executions.realized_pnl IS NOT NULL)) AS realized_pnl,
    count(DISTINCT trade_executions.symbol) AS symbol_count
   FROM public.trade_executions
  GROUP BY trade_executions.credential_id, (date(trade_executions.executed_at));


--
-- Name: journal_symbol_pnl; Type: VIEW; Schema: public; Owner: -
--

CREATE VIEW public.journal_symbol_pnl AS
 SELECT trade_executions.credential_id,
    trade_executions.symbol,
    trade_executions.symbol_name,
    count(*) AS total_trades,
    sum(trade_executions.quantity) FILTER (WHERE (trade_executions.side = 'buy'::public.order_side)) AS total_buy_qty,
    sum(trade_executions.quantity) FILTER (WHERE (trade_executions.side = 'sell'::public.order_side)) AS total_sell_qty,
    sum(trade_executions.notional_value) FILTER (WHERE (trade_executions.side = 'buy'::public.order_side)) AS total_buy_value,
    sum(trade_executions.notional_value) FILTER (WHERE (trade_executions.side = 'sell'::public.order_side)) AS total_sell_value,
    sum(trade_executions.fee) AS total_fees,
    sum(COALESCE(trade_executions.realized_pnl, (0)::numeric)) AS realized_pnl,
    min(trade_executions.executed_at) AS first_trade_at,
    max(trade_executions.executed_at) AS last_trade_at
   FROM public.trade_executions
  GROUP BY trade_executions.credential_id, trade_executions.symbol, trade_executions.symbol_name;


--
-- Name: mv_latest_prices; Type: MATERIALIZED VIEW; Schema: public; Owner: -
--

CREATE MATERIALIZED VIEW public.mv_latest_prices AS
 SELECT DISTINCT ON (ohlcv.symbol) ohlcv.symbol,
    ohlcv.open_time,
    ohlcv.open,
    ohlcv.high,
    ohlcv.low,
    ohlcv.close,
    ohlcv.volume
   FROM public.ohlcv
  WHERE ((ohlcv.timeframe)::text = '1d'::text)
  ORDER BY ohlcv.symbol, ohlcv.open_time DESC
  WITH NO DATA;


--
-- Name: MATERIALIZED VIEW mv_latest_prices; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON MATERIALIZED VIEW public.mv_latest_prices IS '심볼별 최신 일봉 가격. 스크리닝 쿼리 성능 최적화용.';


--
-- Name: ohlcv_metadata; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.ohlcv_metadata (
    symbol character varying(50) NOT NULL,
    timeframe character varying(10) NOT NULL,
    first_cached_time timestamp with time zone,
    last_cached_time timestamp with time zone,
    last_updated_at timestamp with time zone DEFAULT now(),
    total_candles integer DEFAULT 0
);


--
-- Name: TABLE ohlcv_metadata; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.ohlcv_metadata IS '심볼/타임프레임별 캐시 상태 메타데이터. 증분 업데이트 시 마지막 시간 확인용.';


--
-- Name: orders; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.orders (
    id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    exchange character varying(50) NOT NULL,
    exchange_order_id character varying(100),
    symbol_id uuid NOT NULL,
    side public.order_side NOT NULL,
    order_type public.order_type NOT NULL,
    status public.order_status DEFAULT 'pending'::public.order_status NOT NULL,
    time_in_force public.order_time_in_force DEFAULT 'gtc'::public.order_time_in_force,
    quantity numeric(30,15) NOT NULL,
    filled_quantity numeric(30,15) DEFAULT 0,
    price numeric(30,15),
    stop_price numeric(30,15),
    average_fill_price numeric(30,15),
    strategy_id character varying(100),
    client_order_id character varying(100),
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    filled_at timestamp with time zone,
    cancelled_at timestamp with time zone,
    metadata jsonb DEFAULT '{}'::jsonb
);


--
-- Name: performance_snapshots; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.performance_snapshots (
    id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    strategy_id character varying(100),
    snapshot_time timestamp with time zone DEFAULT now() NOT NULL,
    total_trades integer DEFAULT 0,
    winning_trades integer DEFAULT 0,
    losing_trades integer DEFAULT 0,
    total_pnl numeric(30,15) DEFAULT 0,
    total_fees numeric(30,15) DEFAULT 0,
    max_drawdown numeric(10,4),
    sharpe_ratio numeric(10,4),
    win_rate numeric(5,4),
    profit_factor numeric(10,4),
    metadata jsonb DEFAULT '{}'::jsonb
);


--
-- Name: portfolio_equity_history; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.portfolio_equity_history (
    id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    credential_id uuid NOT NULL,
    snapshot_time timestamp with time zone DEFAULT now() NOT NULL,
    total_equity numeric(30,15) NOT NULL,
    cash_balance numeric(30,15) NOT NULL,
    securities_value numeric(30,15) NOT NULL,
    total_pnl numeric(30,15) DEFAULT 0,
    daily_pnl numeric(30,15) DEFAULT 0,
    currency character varying(10) DEFAULT 'KRW'::character varying,
    market character varying(10) DEFAULT 'KR'::character varying,
    account_type character varying(20),
    metadata jsonb DEFAULT '{}'::jsonb,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE portfolio_equity_history; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.portfolio_equity_history IS '포트폴리오 자산 가치 히스토리. 자산 곡선(Equity Curve) 차트와 성과 분석에 사용됨.';


--
-- Name: COLUMN portfolio_equity_history.total_equity; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.portfolio_equity_history.total_equity IS '총 자산 가치 (현금 + 유가증권 평가금액)';


--
-- Name: COLUMN portfolio_equity_history.daily_pnl; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.portfolio_equity_history.daily_pnl IS '당일 손익. KIS API의 일별 손익 데이터 또는 전일 대비 계산값.';


--
-- Name: portfolio_daily_equity; Type: VIEW; Schema: public; Owner: -
--

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


--
-- Name: portfolio_monthly_returns; Type: VIEW; Schema: public; Owner: -
--

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


--
-- Name: positions; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.positions (
    id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    exchange character varying(50) NOT NULL,
    symbol_id uuid NOT NULL,
    side public.order_side NOT NULL,
    quantity numeric(30,15) NOT NULL,
    entry_price numeric(30,15) NOT NULL,
    current_price numeric(30,15),
    unrealized_pnl numeric(30,15) DEFAULT 0,
    realized_pnl numeric(30,15) DEFAULT 0,
    strategy_id character varying(100),
    opened_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    closed_at timestamp with time zone,
    metadata jsonb DEFAULT '{}'::jsonb,
    credential_id uuid,
    symbol_name character varying(200),
    symbol character varying(50)
);


--
-- Name: COLUMN positions.credential_id; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.positions.credential_id IS '거래소 자격증명 ID';


--
-- Name: COLUMN positions.symbol_name; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.positions.symbol_name IS '종목명 (표시용)';


--
-- Name: COLUMN positions.symbol; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.positions.symbol IS '심볼 코드';


--
-- Name: price_snapshot; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.price_snapshot (
    snapshot_date date NOT NULL,
    symbol character varying(20) NOT NULL,
    close_price numeric(20,4) NOT NULL,
    volume bigint,
    recommend_source character varying(50),
    recommend_rank integer,
    recommend_score numeric(5,2),
    expected_return numeric(8,4),
    expected_holding_days integer,
    market character varying(20),
    sector character varying(50),
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE price_snapshot; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.price_snapshot IS '추천 종목 가격 스냅샷. 매일 장 마감 후 스크리닝/전략 추천 종목의 가격 저장';


--
-- Name: reality_check; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.reality_check (
    check_date date NOT NULL,
    recommend_date date NOT NULL,
    symbol character varying(20) NOT NULL,
    recommend_source character varying(50),
    recommend_rank integer,
    recommend_score numeric(5,2),
    entry_price numeric(20,4) NOT NULL,
    exit_price numeric(20,4) NOT NULL,
    actual_return numeric(8,4) NOT NULL,
    is_profitable boolean NOT NULL,
    entry_volume bigint,
    exit_volume bigint,
    volume_change numeric(8,4),
    expected_return numeric(8,4),
    return_error numeric(8,4),
    max_profit numeric(8,4),
    max_drawdown numeric(8,4),
    volatility numeric(8,4),
    market character varying(20),
    sector character varying(50),
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE reality_check; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.reality_check IS '추천 검증 결과. 전일 추천 종목의 익일 실제 성과 계산 결과';


--
-- Name: signal_marker; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.signal_marker (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    symbol_id uuid NOT NULL,
    "timestamp" timestamp with time zone NOT NULL,
    signal_type character varying(20) NOT NULL,
    side character varying(10),
    price numeric(20,8) NOT NULL,
    strength double precision DEFAULT 0.0 NOT NULL,
    indicators jsonb DEFAULT '{}'::jsonb NOT NULL,
    reason text DEFAULT ''::text NOT NULL,
    strategy_id character varying(100) NOT NULL,
    strategy_name character varying(200) NOT NULL,
    executed boolean DEFAULT false NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: TABLE signal_marker; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.signal_marker IS '백테스트 및 실거래 신호 마커 (차트 표시 및 분석용)';


--
-- Name: COLUMN signal_marker.indicators; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.signal_marker.indicators IS '기술적 지표 값 (JSONB): RSI, MACD, BB, RouteState 등';


--
-- Name: COLUMN signal_marker.reason; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.signal_marker.reason IS '신호 발생 이유 (사람이 읽을 수 있는 형태)';


--
-- Name: COLUMN signal_marker.executed; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.signal_marker.executed IS '백테스트에서 실제 체결 여부';


--
-- Name: COLUMN signal_marker.metadata; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.signal_marker.metadata IS '확장용 메타데이터';


--
-- Name: signals; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.signals (
    id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    strategy_id character varying(100) NOT NULL,
    symbol_id uuid NOT NULL,
    side public.order_side NOT NULL,
    signal_type public.signal_type NOT NULL,
    strength numeric(5,4) NOT NULL,
    suggested_price numeric(30,15),
    stop_loss numeric(30,15),
    take_profit numeric(30,15),
    created_at timestamp with time zone DEFAULT now(),
    processed_at timestamp with time zone,
    order_id uuid,
    metadata jsonb DEFAULT '{}'::jsonb,
    CONSTRAINT signals_strength_check CHECK (((strength >= (0)::numeric) AND (strength <= (1)::numeric)))
);


--
-- Name: strategies; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.strategies (
    id character varying(100) NOT NULL,
    name character varying(200) NOT NULL,
    description text,
    version character varying(20),
    is_active boolean DEFAULT false,
    config jsonb DEFAULT '{}'::jsonb,
    risk_limits jsonb DEFAULT '{}'::jsonb,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    last_started_at timestamp with time zone,
    last_stopped_at timestamp with time zone,
    strategy_type character varying(50),
    symbols jsonb DEFAULT '[]'::jsonb,
    market character varying(20) DEFAULT 'KR'::character varying,
    timeframe character varying(10) DEFAULT '1d'::character varying,
    allocated_capital numeric(30,15),
    risk_profile character varying(20) DEFAULT 'default'::character varying
);


--
-- Name: COLUMN strategies.timeframe; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.strategies.timeframe IS '전략 실행 타임프레임 (1m, 5m, 15m, 30m, 1h, 4h, 1d, 1w, 1M)';


--
-- Name: strategy_presets; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.strategy_presets (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    strategy_type character varying(100) NOT NULL,
    preset_name character varying(200) NOT NULL,
    description text,
    parameters jsonb NOT NULL,
    schema_version integer DEFAULT 1 NOT NULL,
    is_default boolean DEFAULT false NOT NULL,
    is_public boolean DEFAULT false NOT NULL,
    tags character varying(50)[] DEFAULT '{}'::character varying[],
    performance_metrics jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: TABLE strategy_presets; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.strategy_presets IS '전략 파라미터 프리셋';


--
-- Name: symbol_fundamental; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.symbol_fundamental (
    id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    symbol_info_id uuid NOT NULL,
    market_cap numeric(20,2),
    shares_outstanding bigint,
    float_shares bigint,
    week_52_high numeric(20,4),
    week_52_low numeric(20,4),
    avg_volume_10d bigint,
    avg_volume_3m bigint,
    per numeric(12,4),
    forward_per numeric(12,4),
    pbr numeric(12,4),
    psr numeric(12,4),
    pcr numeric(12,4),
    ev_ebitda numeric(12,4),
    eps numeric(20,4),
    bps numeric(20,4),
    dps numeric(20,4),
    sps numeric(20,4),
    dividend_yield numeric(12,4),
    dividend_payout_ratio numeric(12,4),
    ex_dividend_date date,
    revenue numeric(20,2),
    operating_income numeric(20,2),
    net_income numeric(20,2),
    total_assets numeric(20,2),
    total_liabilities numeric(20,2),
    total_equity numeric(20,2),
    roe numeric(12,4),
    roa numeric(12,4),
    operating_margin numeric(12,4),
    net_profit_margin numeric(12,4),
    gross_margin numeric(12,4),
    debt_ratio numeric(12,4),
    current_ratio numeric(12,4),
    quick_ratio numeric(12,4),
    interest_coverage numeric(12,4),
    revenue_growth_yoy numeric(12,4),
    earnings_growth_yoy numeric(12,4),
    revenue_growth_3y numeric(12,4),
    earnings_growth_3y numeric(12,4),
    data_source character varying(50),
    fiscal_year_end character varying(10),
    currency character varying(10) DEFAULT 'KRW'::character varying,
    fetched_at timestamp with time zone DEFAULT now(),
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    ttm_squeeze boolean DEFAULT false,
    ttm_squeeze_cnt integer DEFAULT 0,
    regime character varying(20)
);


--
-- Name: TABLE symbol_fundamental; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.symbol_fundamental IS '심볼 펀더멘털 데이터 - 시가총액, PER, PBR, 재무 지표 등';


--
-- Name: COLUMN symbol_fundamental.symbol_info_id; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.symbol_fundamental.symbol_info_id IS 'symbol_info 테이블 FK';


--
-- Name: COLUMN symbol_fundamental.market_cap; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.symbol_fundamental.market_cap IS '시가총액 (통화 단위)';


--
-- Name: COLUMN symbol_fundamental.per; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.symbol_fundamental.per IS '주가수익비율 (Price to Earnings Ratio)';


--
-- Name: COLUMN symbol_fundamental.pbr; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.symbol_fundamental.pbr IS '주가순자산비율 (Price to Book Ratio)';


--
-- Name: COLUMN symbol_fundamental.roe; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.symbol_fundamental.roe IS 'ROE (자기자본이익률) % - 극단적 값 허용';


--
-- Name: COLUMN symbol_fundamental.operating_margin; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.symbol_fundamental.operating_margin IS '영업이익률 % - 적자 기업의 큰 음수 허용';


--
-- Name: COLUMN symbol_fundamental.revenue_growth_yoy; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.symbol_fundamental.revenue_growth_yoy IS '매출 성장률 YoY % - 스타트업 고성장 허용';


--
-- Name: COLUMN symbol_fundamental.earnings_growth_yoy; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.symbol_fundamental.earnings_growth_yoy IS '이익 성장률 YoY % - 적자→흑자 전환 시 극단값 허용';


--
-- Name: COLUMN symbol_fundamental.ttm_squeeze; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.symbol_fundamental.ttm_squeeze IS 'TTM Squeeze 상태 (BB가 KC 내부에 있으면 true)';


--
-- Name: COLUMN symbol_fundamental.ttm_squeeze_cnt; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.symbol_fundamental.ttm_squeeze_cnt IS 'TTM Squeeze 연속 카운트 (에너지 응축 기간)';


--
-- Name: COLUMN symbol_fundamental.regime; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.symbol_fundamental.regime IS '시장 레짐: STRONG_UPTREND, CORRECTION, SIDEWAYS, BOTTOM_BOUNCE, DOWNTREND';


--
-- Name: symbol_info; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.symbol_info (
    id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    ticker character varying(20) NOT NULL,
    name character varying(200) NOT NULL,
    name_en character varying(200),
    market character varying(20) NOT NULL,
    exchange character varying(50),
    sector character varying(100),
    yahoo_symbol character varying(30),
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    fetch_fail_count integer DEFAULT 0,
    last_fetch_error text,
    last_fetch_attempt timestamp with time zone,
    symbol_type character varying(20) DEFAULT 'STOCK'::character varying
);


--
-- Name: TABLE symbol_info; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.symbol_info IS '심볼 메타데이터 - 티커와 회사명 매핑';


--
-- Name: COLUMN symbol_info.ticker; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.symbol_info.ticker IS '티커 코드 (예: AAPL, 005930)';


--
-- Name: COLUMN symbol_info.name; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.symbol_info.name IS '회사명 (예: Apple Inc., 삼성전자)';


--
-- Name: COLUMN symbol_info.yahoo_symbol; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.symbol_info.yahoo_symbol IS 'Yahoo Finance 호환 심볼 (예: 005930.KS, AAPL)';


--
-- Name: COLUMN symbol_info.symbol_type; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.symbol_info.symbol_type IS '종목 유형 (STOCK, ETF, ETN, WARRANT, REIT, PREFERRED)';


--
-- Name: symbols; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.symbols (
    id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    base character varying(20) NOT NULL,
    quote character varying(20) NOT NULL,
    market_type public.market_type NOT NULL,
    exchange character varying(50) NOT NULL,
    exchange_symbol character varying(50),
    is_active boolean DEFAULT true,
    min_quantity numeric(30,15),
    max_quantity numeric(30,15),
    quantity_step numeric(30,15),
    min_notional numeric(30,15),
    price_precision integer,
    quantity_precision integer,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: telegram_settings; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.telegram_settings (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    encrypted_bot_token bytea NOT NULL,
    encryption_nonce_token bytea NOT NULL,
    encrypted_chat_id bytea NOT NULL,
    encryption_nonce_chat bytea NOT NULL,
    encryption_version integer DEFAULT 1 NOT NULL,
    is_enabled boolean DEFAULT true NOT NULL,
    notification_settings jsonb DEFAULT '{"error_alerts": true, "order_filled": true, "daily_summary": true, "risk_warnings": true, "trade_executed": true, "position_closed": true, "position_opened": true, "stop_loss_triggered": true, "take_profit_triggered": true}'::jsonb,
    bot_username character varying(100),
    chat_type character varying(20),
    last_message_at timestamp with time zone,
    last_verified_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: TABLE telegram_settings; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.telegram_settings IS '텔레그램 봇 설정 (암호화)';


--
-- Name: trade_ticks; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.trade_ticks (
    "time" timestamp with time zone NOT NULL,
    symbol_id uuid NOT NULL,
    exchange_trade_id character varying(100) NOT NULL,
    price numeric(30,15) NOT NULL,
    quantity numeric(30,15) NOT NULL,
    is_buyer_maker boolean
);


--
-- Name: trades; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.trades (
    id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    order_id uuid NOT NULL,
    exchange character varying(50) NOT NULL,
    exchange_trade_id character varying(100) NOT NULL,
    symbol_id uuid NOT NULL,
    side public.order_side NOT NULL,
    quantity numeric(30,15) NOT NULL,
    price numeric(30,15) NOT NULL,
    fee numeric(30,15) DEFAULT 0,
    fee_currency character varying(20),
    is_maker boolean DEFAULT false,
    executed_at timestamp with time zone NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    metadata jsonb DEFAULT '{}'::jsonb
);


--
-- Name: users; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.users (
    id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    username character varying(50) NOT NULL,
    email character varying(255) NOT NULL,
    password_hash character varying(255) NOT NULL,
    role character varying(20) DEFAULT 'trader'::character varying,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    last_login_at timestamp with time zone
);


--
-- Name: v_daily_pnl; Type: VIEW; Schema: public; Owner: -
--

CREATE VIEW public.v_daily_pnl AS
 SELECT ec.credential_id,
    date((ec.executed_at AT TIME ZONE 'Asia/Seoul'::text)) AS trade_date,
    count(*) AS total_trades,
    count(*) FILTER (WHERE ((ec.side)::text = 'buy'::text)) AS buy_count,
    count(*) FILTER (WHERE ((ec.side)::text = 'sell'::text)) AS sell_count,
    COALESCE(sum(ec.amount), (0)::numeric) AS total_volume,
    COALESCE(sum(ec.fee), (0)::numeric) AS total_fees,
    COALESCE(sum(te.realized_pnl), (0)::numeric) AS realized_pnl,
    count(DISTINCT ec.symbol) AS symbol_count
   FROM (public.execution_cache ec
     LEFT JOIN public.trade_executions te ON (((te.credential_id = ec.credential_id) AND ((te.exchange)::text = (ec.exchange)::text) AND ((te.exchange_trade_id)::text = (ec.trade_id)::text))))
  GROUP BY ec.credential_id, (date((ec.executed_at AT TIME ZONE 'Asia/Seoul'::text)));


--
-- Name: VIEW v_daily_pnl; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON VIEW public.v_daily_pnl IS '일별 손익 집계 뷰';


--
-- Name: v_cumulative_pnl; Type: VIEW; Schema: public; Owner: -
--

CREATE VIEW public.v_cumulative_pnl AS
 SELECT v_daily_pnl.credential_id,
    v_daily_pnl.trade_date,
    v_daily_pnl.total_trades,
    v_daily_pnl.realized_pnl,
    v_daily_pnl.total_fees,
    sum(v_daily_pnl.realized_pnl) OVER (PARTITION BY v_daily_pnl.credential_id ORDER BY v_daily_pnl.trade_date) AS cumulative_pnl,
    sum(v_daily_pnl.total_fees) OVER (PARTITION BY v_daily_pnl.credential_id ORDER BY v_daily_pnl.trade_date) AS cumulative_fees,
    (sum(v_daily_pnl.total_trades) OVER (PARTITION BY v_daily_pnl.credential_id ORDER BY v_daily_pnl.trade_date))::bigint AS cumulative_trades
   FROM public.v_daily_pnl
  ORDER BY v_daily_pnl.credential_id, v_daily_pnl.trade_date;


--
-- Name: VIEW v_cumulative_pnl; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON VIEW public.v_cumulative_pnl IS '일별 누적 손익 뷰';


--
-- Name: v_journal_executions; Type: VIEW; Schema: public; Owner: -
--

CREATE VIEW public.v_journal_executions AS
 SELECT ec.id,
    ec.credential_id,
    ec.exchange,
    ec.symbol,
    ec.normalized_symbol AS symbol_name,
    ec.side,
    COALESCE(ec.order_type, 'market'::character varying) AS order_type,
    ec.quantity,
    ec.price,
    ec.amount AS notional_value,
    ec.fee,
    ec.fee_currency,
    te.position_effect,
    te.realized_pnl,
    te.order_id,
    ec.order_id AS exchange_order_id,
    ec.trade_id AS exchange_trade_id,
    te.strategy_id,
    te.strategy_name,
    ec.executed_at,
    te.memo,
    te.tags,
    te.metadata,
    ec.created_at,
    te.updated_at
   FROM (public.execution_cache ec
     LEFT JOIN public.trade_executions te ON (((te.credential_id = ec.credential_id) AND ((te.exchange)::text = (ec.exchange)::text) AND ((te.exchange_trade_id)::text = (ec.trade_id)::text))));


--
-- Name: VIEW v_journal_executions; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON VIEW public.v_journal_executions IS '매매일지 체결 내역 통합 뷰 (execution_cache + trade_executions)';


--
-- Name: v_monthly_pnl; Type: VIEW; Schema: public; Owner: -
--

CREATE VIEW public.v_monthly_pnl AS
 SELECT ec.credential_id,
    (EXTRACT(year FROM (ec.executed_at AT TIME ZONE 'Asia/Seoul'::text)))::integer AS year,
    (EXTRACT(month FROM (ec.executed_at AT TIME ZONE 'Asia/Seoul'::text)))::integer AS month,
    count(*) AS total_trades,
    count(*) FILTER (WHERE ((ec.side)::text = 'buy'::text)) AS buy_count,
    count(*) FILTER (WHERE ((ec.side)::text = 'sell'::text)) AS sell_count,
    COALESCE(sum(ec.amount), (0)::numeric) AS total_volume,
    COALESCE(sum(ec.fee), (0)::numeric) AS total_fees,
    COALESCE(sum(te.realized_pnl), (0)::numeric) AS realized_pnl,
    count(DISTINCT ec.symbol) AS symbol_count,
    count(DISTINCT date((ec.executed_at AT TIME ZONE 'Asia/Seoul'::text))) AS trading_days
   FROM (public.execution_cache ec
     LEFT JOIN public.trade_executions te ON (((te.credential_id = ec.credential_id) AND ((te.exchange)::text = (ec.exchange)::text) AND ((te.exchange_trade_id)::text = (ec.trade_id)::text))))
  GROUP BY ec.credential_id, (EXTRACT(year FROM (ec.executed_at AT TIME ZONE 'Asia/Seoul'::text))), (EXTRACT(month FROM (ec.executed_at AT TIME ZONE 'Asia/Seoul'::text)));


--
-- Name: VIEW v_monthly_pnl; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON VIEW public.v_monthly_pnl IS '월별 손익 집계 뷰';


--
-- Name: v_reality_check_daily_stats; Type: VIEW; Schema: public; Owner: -
--

CREATE VIEW public.v_reality_check_daily_stats AS
 SELECT reality_check.check_date,
    count(*) AS total_count,
    count(*) FILTER (WHERE reality_check.is_profitable) AS win_count,
    round((((count(*) FILTER (WHERE reality_check.is_profitable))::numeric / (count(*))::numeric) * (100)::numeric), 2) AS win_rate,
    round(avg(reality_check.actual_return), 4) AS avg_return,
    round(avg(reality_check.actual_return) FILTER (WHERE reality_check.is_profitable), 4) AS avg_win_return,
    round(avg(reality_check.actual_return) FILTER (WHERE (NOT reality_check.is_profitable)), 4) AS avg_loss_return,
    round(max(reality_check.actual_return), 4) AS max_return,
    round(min(reality_check.actual_return), 4) AS min_return,
    round(stddev(reality_check.actual_return), 4) AS return_stddev
   FROM public.reality_check
  GROUP BY reality_check.check_date
  ORDER BY reality_check.check_date DESC;


--
-- Name: VIEW v_reality_check_daily_stats; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON VIEW public.v_reality_check_daily_stats IS '일별 승률, 평균 수익률 등 주요 통계';


--
-- Name: v_reality_check_rank_stats; Type: VIEW; Schema: public; Owner: -
--

CREATE VIEW public.v_reality_check_rank_stats AS
 SELECT reality_check.recommend_rank,
    count(*) AS total_count,
    round((((count(*) FILTER (WHERE reality_check.is_profitable))::numeric / (count(*))::numeric) * (100)::numeric), 2) AS win_rate,
    round(avg(reality_check.actual_return), 4) AS avg_return
   FROM public.reality_check
  WHERE ((reality_check.recommend_rank IS NOT NULL) AND (reality_check.recommend_rank <= 10))
  GROUP BY reality_check.recommend_rank
  ORDER BY reality_check.recommend_rank;


--
-- Name: VIEW v_reality_check_rank_stats; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON VIEW public.v_reality_check_rank_stats IS '추천 순위별 성과 분석 (Top 10)';


--
-- Name: v_reality_check_recent_trend; Type: VIEW; Schema: public; Owner: -
--

CREATE VIEW public.v_reality_check_recent_trend AS
 SELECT reality_check.check_date,
    reality_check.recommend_source,
    count(*) AS count,
    round((((count(*) FILTER (WHERE reality_check.is_profitable))::numeric / (count(*))::numeric) * (100)::numeric), 2) AS win_rate,
    round(avg(reality_check.actual_return), 4) AS avg_return
   FROM public.reality_check
  WHERE (reality_check.check_date >= (CURRENT_DATE - '30 days'::interval))
  GROUP BY reality_check.check_date, reality_check.recommend_source
  ORDER BY reality_check.check_date DESC, reality_check.recommend_source;


--
-- Name: VIEW v_reality_check_recent_trend; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON VIEW public.v_reality_check_recent_trend IS '최근 30일 성과 추이 (일별/소스별)';


--
-- Name: v_reality_check_source_stats; Type: VIEW; Schema: public; Owner: -
--

CREATE VIEW public.v_reality_check_source_stats AS
 SELECT reality_check.recommend_source,
    count(*) AS total_count,
    count(*) FILTER (WHERE reality_check.is_profitable) AS win_count,
    round((((count(*) FILTER (WHERE reality_check.is_profitable))::numeric / (count(*))::numeric) * (100)::numeric), 2) AS win_rate,
    round(avg(reality_check.actual_return), 4) AS avg_return,
    round(avg(reality_check.actual_return) FILTER (WHERE reality_check.is_profitable), 4) AS avg_win_return,
    round(avg(reality_check.actual_return) FILTER (WHERE (NOT reality_check.is_profitable)), 4) AS avg_loss_return
   FROM public.reality_check
  GROUP BY reality_check.recommend_source
  ORDER BY (round(avg(reality_check.actual_return), 4)) DESC;


--
-- Name: VIEW v_reality_check_source_stats; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON VIEW public.v_reality_check_source_stats IS '추천 소스(screening/전략)별 성과 비교';


--
-- Name: v_strategy_monthly_performance; Type: VIEW; Schema: public; Owner: -
--

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


--
-- Name: VIEW v_strategy_monthly_performance; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON VIEW public.v_strategy_monthly_performance IS '전략별 월간 성과 추이 뷰';


--
-- Name: v_strategy_performance; Type: VIEW; Schema: public; Owner: -
--

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


--
-- Name: VIEW v_strategy_performance; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON VIEW public.v_strategy_performance IS '전략별 성과 분석 뷰';


--
-- Name: v_symbol_fetch_failures; Type: VIEW; Schema: public; Owner: -
--

CREATE VIEW public.v_symbol_fetch_failures AS
 SELECT si.id,
    si.ticker,
    si.name,
    si.market,
    si.exchange,
    si.yahoo_symbol,
    si.is_active,
    si.fetch_fail_count,
    si.last_fetch_error,
    si.last_fetch_attempt,
        CASE
            WHEN (si.fetch_fail_count >= 3) THEN 'CRITICAL'::text
            WHEN (si.fetch_fail_count >= 2) THEN 'WARNING'::text
            WHEN (si.fetch_fail_count >= 1) THEN 'MINOR'::text
            ELSE 'OK'::text
        END AS failure_level
   FROM public.symbol_info si
  WHERE (si.fetch_fail_count > 0)
  ORDER BY si.fetch_fail_count DESC, si.last_fetch_attempt DESC;


--
-- Name: v_symbol_pnl; Type: VIEW; Schema: public; Owner: -
--

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


--
-- Name: VIEW v_symbol_pnl; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON VIEW public.v_symbol_pnl IS '종목별 손익 집계 뷰';


--
-- Name: v_symbol_with_fundamental; Type: VIEW; Schema: public; Owner: -
--

CREATE VIEW public.v_symbol_with_fundamental AS
 SELECT si.id,
    si.ticker,
    si.name,
    si.name_en,
    si.market,
    si.exchange,
    si.sector,
    si.yahoo_symbol,
    si.is_active,
    sf.market_cap,
    sf.per,
    sf.pbr,
    sf.eps,
    sf.bps,
    sf.dividend_yield,
    sf.roe,
    sf.roa,
    sf.operating_margin,
    sf.debt_ratio,
    sf.week_52_high,
    sf.week_52_low,
    sf.avg_volume_10d,
    sf.revenue,
    sf.operating_income,
    sf.net_income,
    sf.revenue_growth_yoy,
    sf.earnings_growth_yoy,
    sf.regime,
    sf.data_source AS fundamental_source,
    sf.fetched_at AS fundamental_fetched_at,
    sf.updated_at AS fundamental_updated_at
   FROM (public.symbol_info si
     LEFT JOIN public.symbol_fundamental sf ON ((si.id = sf.symbol_info_id)))
  WHERE (si.is_active = true);


--
-- Name: VIEW v_symbol_with_fundamental; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON VIEW public.v_symbol_with_fundamental IS '심볼 기본정보와 펀더멘털 통합 조회용 뷰 (regime 포함)';


--
-- Name: v_total_pnl; Type: VIEW; Schema: public; Owner: -
--

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


--
-- Name: VIEW v_total_pnl; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON VIEW public.v_total_pnl IS '전체 PnL 요약 뷰';


--
-- Name: v_trading_insights; Type: VIEW; Schema: public; Owner: -
--

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


--
-- Name: VIEW v_trading_insights; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON VIEW public.v_trading_insights IS '투자 인사이트 통계 뷰';


--
-- Name: v_weekly_pnl; Type: VIEW; Schema: public; Owner: -
--

CREATE VIEW public.v_weekly_pnl AS
 SELECT ec.credential_id,
    (date_trunc('week'::text, (ec.executed_at AT TIME ZONE 'Asia/Seoul'::text)))::date AS week_start,
    count(*) AS total_trades,
    count(*) FILTER (WHERE ((ec.side)::text = 'buy'::text)) AS buy_count,
    count(*) FILTER (WHERE ((ec.side)::text = 'sell'::text)) AS sell_count,
    COALESCE(sum(ec.amount), (0)::numeric) AS total_volume,
    COALESCE(sum(ec.fee), (0)::numeric) AS total_fees,
    COALESCE(sum(te.realized_pnl), (0)::numeric) AS realized_pnl,
    count(DISTINCT ec.symbol) AS symbol_count,
    count(DISTINCT date((ec.executed_at AT TIME ZONE 'Asia/Seoul'::text))) AS trading_days
   FROM (public.execution_cache ec
     LEFT JOIN public.trade_executions te ON (((te.credential_id = ec.credential_id) AND ((te.exchange)::text = (ec.exchange)::text) AND ((te.exchange_trade_id)::text = (ec.trade_id)::text))))
  GROUP BY ec.credential_id, (date_trunc('week'::text, (ec.executed_at AT TIME ZONE 'Asia/Seoul'::text)));


--
-- Name: VIEW v_weekly_pnl; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON VIEW public.v_weekly_pnl IS '주별 손익 집계 뷰';


--
-- Name: v_yearly_pnl; Type: VIEW; Schema: public; Owner: -
--

CREATE VIEW public.v_yearly_pnl AS
 SELECT ec.credential_id,
    (EXTRACT(year FROM (ec.executed_at AT TIME ZONE 'Asia/Seoul'::text)))::integer AS year,
    count(*) AS total_trades,
    count(*) FILTER (WHERE ((ec.side)::text = 'buy'::text)) AS buy_count,
    count(*) FILTER (WHERE ((ec.side)::text = 'sell'::text)) AS sell_count,
    COALESCE(sum(ec.amount), (0)::numeric) AS total_volume,
    COALESCE(sum(ec.fee), (0)::numeric) AS total_fees,
    COALESCE(sum(te.realized_pnl), (0)::numeric) AS realized_pnl,
    count(DISTINCT ec.symbol) AS symbol_count,
    count(DISTINCT date((ec.executed_at AT TIME ZONE 'Asia/Seoul'::text))) AS trading_days,
    count(DISTINCT EXTRACT(month FROM (ec.executed_at AT TIME ZONE 'Asia/Seoul'::text))) AS trading_months
   FROM (public.execution_cache ec
     LEFT JOIN public.trade_executions te ON (((te.credential_id = ec.credential_id) AND ((te.exchange)::text = (ec.exchange)::text) AND ((te.exchange_trade_id)::text = (ec.trade_id)::text))))
  GROUP BY ec.credential_id, (EXTRACT(year FROM (ec.executed_at AT TIME ZONE 'Asia/Seoul'::text)));


--
-- Name: VIEW v_yearly_pnl; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON VIEW public.v_yearly_pnl IS '연도별 손익 집계 뷰';


--
-- Name: watchlist; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.watchlist (
    id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    symbol character varying(50) NOT NULL,
    market character varying(10) NOT NULL,
    display_name character varying(100),
    sort_order integer DEFAULT 0,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: _hyper_6_111_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_111_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_392_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_392_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_393_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_393_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_394_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_394_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_395_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_395_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_396_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_396_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_397_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_397_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_398_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_398_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_399_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_399_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_400_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_400_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_401_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_401_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_402_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_402_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_403_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_403_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_404_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_404_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_405_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_405_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_406_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_406_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_407_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_407_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_408_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_408_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_409_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_409_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_410_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_410_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_411_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_411_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_412_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_412_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_413_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_413_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_414_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_414_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_415_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_415_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_547_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_547_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: _hyper_6_549_chunk fetched_at; Type: DEFAULT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_549_chunk ALTER COLUMN fetched_at SET DEFAULT now();


--
-- Name: credential_access_logs id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.credential_access_logs ALTER COLUMN id SET DEFAULT nextval('public.credential_access_logs_id_seq'::regclass);


--
-- Name: _hyper_1_100_chunk 100_199_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_100_chunk
    ADD CONSTRAINT "100_199_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_101_chunk 101_201_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_101_chunk
    ADD CONSTRAINT "101_201_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_102_chunk 102_203_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_102_chunk
    ADD CONSTRAINT "102_203_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_103_chunk 103_205_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_103_chunk
    ADD CONSTRAINT "103_205_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_104_chunk 104_207_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_104_chunk
    ADD CONSTRAINT "104_207_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_105_chunk 105_209_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_105_chunk
    ADD CONSTRAINT "105_209_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_106_chunk 106_211_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_106_chunk
    ADD CONSTRAINT "106_211_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_107_chunk 107_213_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_107_chunk
    ADD CONSTRAINT "107_213_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_108_chunk 108_215_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_108_chunk
    ADD CONSTRAINT "108_215_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_109_chunk 109_217_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_109_chunk
    ADD CONSTRAINT "109_217_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_10_chunk 10_19_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_10_chunk
    ADD CONSTRAINT "10_19_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_110_chunk 110_219_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_110_chunk
    ADD CONSTRAINT "110_219_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_6_111_chunk 111_221_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_111_chunk
    ADD CONSTRAINT "111_221_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_1_11_chunk 11_21_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_11_chunk
    ADD CONSTRAINT "11_21_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_12_chunk 12_23_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_12_chunk
    ADD CONSTRAINT "12_23_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_13_chunk 13_25_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_13_chunk
    ADD CONSTRAINT "13_25_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_14_chunk 14_27_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_14_chunk
    ADD CONSTRAINT "14_27_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_15_chunk 15_29_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_15_chunk
    ADD CONSTRAINT "15_29_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_16_chunk 16_31_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_16_chunk
    ADD CONSTRAINT "16_31_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_17_chunk 17_33_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_17_chunk
    ADD CONSTRAINT "17_33_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_18_chunk 18_35_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_18_chunk
    ADD CONSTRAINT "18_35_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_19_chunk 19_37_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_19_chunk
    ADD CONSTRAINT "19_37_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_1_chunk 1_1_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_1_chunk
    ADD CONSTRAINT "1_1_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_20_chunk 20_39_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_20_chunk
    ADD CONSTRAINT "20_39_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_21_chunk 21_41_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_21_chunk
    ADD CONSTRAINT "21_41_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_22_chunk 22_43_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_22_chunk
    ADD CONSTRAINT "22_43_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_23_chunk 23_45_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_23_chunk
    ADD CONSTRAINT "23_45_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_24_chunk 24_47_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_24_chunk
    ADD CONSTRAINT "24_47_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_25_chunk 25_49_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_25_chunk
    ADD CONSTRAINT "25_49_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_26_chunk 26_51_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_26_chunk
    ADD CONSTRAINT "26_51_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_27_chunk 27_53_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_27_chunk
    ADD CONSTRAINT "27_53_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_28_chunk 28_55_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_28_chunk
    ADD CONSTRAINT "28_55_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_29_chunk 29_57_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_29_chunk
    ADD CONSTRAINT "29_57_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_2_chunk 2_3_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_2_chunk
    ADD CONSTRAINT "2_3_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_30_chunk 30_59_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_30_chunk
    ADD CONSTRAINT "30_59_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_31_chunk 31_61_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_31_chunk
    ADD CONSTRAINT "31_61_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_32_chunk 32_63_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_32_chunk
    ADD CONSTRAINT "32_63_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_33_chunk 33_65_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_33_chunk
    ADD CONSTRAINT "33_65_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_34_chunk 34_67_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_34_chunk
    ADD CONSTRAINT "34_67_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_35_chunk 35_69_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_35_chunk
    ADD CONSTRAINT "35_69_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_36_chunk 36_71_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_36_chunk
    ADD CONSTRAINT "36_71_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_37_chunk 37_73_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_37_chunk
    ADD CONSTRAINT "37_73_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_38_chunk 38_75_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_38_chunk
    ADD CONSTRAINT "38_75_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_6_392_chunk 392_510_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_392_chunk
    ADD CONSTRAINT "392_510_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_6_393_chunk 393_511_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_393_chunk
    ADD CONSTRAINT "393_511_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_6_394_chunk 394_512_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_394_chunk
    ADD CONSTRAINT "394_512_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_6_395_chunk 395_513_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_395_chunk
    ADD CONSTRAINT "395_513_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_6_396_chunk 396_514_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_396_chunk
    ADD CONSTRAINT "396_514_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_6_397_chunk 397_515_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_397_chunk
    ADD CONSTRAINT "397_515_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_6_398_chunk 398_516_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_398_chunk
    ADD CONSTRAINT "398_516_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_6_399_chunk 399_517_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_399_chunk
    ADD CONSTRAINT "399_517_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_1_39_chunk 39_77_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_39_chunk
    ADD CONSTRAINT "39_77_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_3_chunk 3_5_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_3_chunk
    ADD CONSTRAINT "3_5_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_6_400_chunk 400_518_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_400_chunk
    ADD CONSTRAINT "400_518_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_6_401_chunk 401_519_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_401_chunk
    ADD CONSTRAINT "401_519_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_6_402_chunk 402_520_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_402_chunk
    ADD CONSTRAINT "402_520_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_6_403_chunk 403_521_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_403_chunk
    ADD CONSTRAINT "403_521_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_6_404_chunk 404_522_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_404_chunk
    ADD CONSTRAINT "404_522_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_6_405_chunk 405_523_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_405_chunk
    ADD CONSTRAINT "405_523_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_6_406_chunk 406_524_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_406_chunk
    ADD CONSTRAINT "406_524_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_6_407_chunk 407_525_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_407_chunk
    ADD CONSTRAINT "407_525_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_6_408_chunk 408_526_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_408_chunk
    ADD CONSTRAINT "408_526_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_6_409_chunk 409_527_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_409_chunk
    ADD CONSTRAINT "409_527_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_1_40_chunk 40_79_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_40_chunk
    ADD CONSTRAINT "40_79_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_6_410_chunk 410_528_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_410_chunk
    ADD CONSTRAINT "410_528_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_6_411_chunk 411_529_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_411_chunk
    ADD CONSTRAINT "411_529_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_6_412_chunk 412_530_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_412_chunk
    ADD CONSTRAINT "412_530_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_6_413_chunk 413_531_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_413_chunk
    ADD CONSTRAINT "413_531_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_6_414_chunk 414_532_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_414_chunk
    ADD CONSTRAINT "414_532_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_6_415_chunk 415_533_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_415_chunk
    ADD CONSTRAINT "415_533_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_1_41_chunk 41_81_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_41_chunk
    ADD CONSTRAINT "41_81_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_42_chunk 42_83_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_42_chunk
    ADD CONSTRAINT "42_83_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_43_chunk 43_85_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_43_chunk
    ADD CONSTRAINT "43_85_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_44_chunk 44_87_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_44_chunk
    ADD CONSTRAINT "44_87_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_45_chunk 45_89_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_45_chunk
    ADD CONSTRAINT "45_89_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_46_chunk 46_91_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_46_chunk
    ADD CONSTRAINT "46_91_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_47_chunk 47_93_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_47_chunk
    ADD CONSTRAINT "47_93_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_48_chunk 48_95_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_48_chunk
    ADD CONSTRAINT "48_95_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_49_chunk 49_97_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_49_chunk
    ADD CONSTRAINT "49_97_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_4_chunk 4_7_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_4_chunk
    ADD CONSTRAINT "4_7_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_50_chunk 50_99_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_50_chunk
    ADD CONSTRAINT "50_99_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_51_chunk 51_101_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_51_chunk
    ADD CONSTRAINT "51_101_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_52_chunk 52_103_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_52_chunk
    ADD CONSTRAINT "52_103_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_53_chunk 53_105_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_53_chunk
    ADD CONSTRAINT "53_105_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_6_547_chunk 547_534_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_547_chunk
    ADD CONSTRAINT "547_534_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_6_549_chunk 549_535_yahoo_candle_cache_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_6_549_chunk
    ADD CONSTRAINT "549_535_yahoo_candle_cache_pkey" PRIMARY KEY (symbol, timeframe, open_time);


--
-- Name: _hyper_1_54_chunk 54_107_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_54_chunk
    ADD CONSTRAINT "54_107_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_55_chunk 55_109_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_55_chunk
    ADD CONSTRAINT "55_109_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_56_chunk 56_111_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_56_chunk
    ADD CONSTRAINT "56_111_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_57_chunk 57_113_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_57_chunk
    ADD CONSTRAINT "57_113_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_5_chunk 5_9_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_5_chunk
    ADD CONSTRAINT "5_9_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_62_chunk 62_123_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_62_chunk
    ADD CONSTRAINT "62_123_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_63_chunk 63_125_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_63_chunk
    ADD CONSTRAINT "63_125_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_64_chunk 64_127_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_64_chunk
    ADD CONSTRAINT "64_127_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_65_chunk 65_129_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_65_chunk
    ADD CONSTRAINT "65_129_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_66_chunk 66_131_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_66_chunk
    ADD CONSTRAINT "66_131_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_67_chunk 67_133_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_67_chunk
    ADD CONSTRAINT "67_133_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_68_chunk 68_135_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_68_chunk
    ADD CONSTRAINT "68_135_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_69_chunk 69_137_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_69_chunk
    ADD CONSTRAINT "69_137_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_6_chunk 6_11_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_6_chunk
    ADD CONSTRAINT "6_11_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_70_chunk 70_139_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_70_chunk
    ADD CONSTRAINT "70_139_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_71_chunk 71_141_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_71_chunk
    ADD CONSTRAINT "71_141_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_72_chunk 72_143_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_72_chunk
    ADD CONSTRAINT "72_143_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_73_chunk 73_145_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_73_chunk
    ADD CONSTRAINT "73_145_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_74_chunk 74_147_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_74_chunk
    ADD CONSTRAINT "74_147_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_75_chunk 75_149_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_75_chunk
    ADD CONSTRAINT "75_149_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_76_chunk 76_151_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_76_chunk
    ADD CONSTRAINT "76_151_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_77_chunk 77_153_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_77_chunk
    ADD CONSTRAINT "77_153_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_78_chunk 78_155_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_78_chunk
    ADD CONSTRAINT "78_155_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_79_chunk 79_157_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_79_chunk
    ADD CONSTRAINT "79_157_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_7_chunk 7_13_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_7_chunk
    ADD CONSTRAINT "7_13_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_80_chunk 80_159_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_80_chunk
    ADD CONSTRAINT "80_159_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_81_chunk 81_161_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_81_chunk
    ADD CONSTRAINT "81_161_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_82_chunk 82_163_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_82_chunk
    ADD CONSTRAINT "82_163_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_83_chunk 83_165_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_83_chunk
    ADD CONSTRAINT "83_165_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_84_chunk 84_167_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_84_chunk
    ADD CONSTRAINT "84_167_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_85_chunk 85_169_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_85_chunk
    ADD CONSTRAINT "85_169_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_86_chunk 86_171_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_86_chunk
    ADD CONSTRAINT "86_171_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_87_chunk 87_173_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_87_chunk
    ADD CONSTRAINT "87_173_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_88_chunk 88_175_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_88_chunk
    ADD CONSTRAINT "88_175_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_89_chunk 89_177_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_89_chunk
    ADD CONSTRAINT "89_177_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_8_chunk 8_15_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_8_chunk
    ADD CONSTRAINT "8_15_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_90_chunk 90_179_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_90_chunk
    ADD CONSTRAINT "90_179_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_91_chunk 91_181_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_91_chunk
    ADD CONSTRAINT "91_181_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_92_chunk 92_183_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_92_chunk
    ADD CONSTRAINT "92_183_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_93_chunk 93_185_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_93_chunk
    ADD CONSTRAINT "93_185_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_94_chunk 94_187_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_94_chunk
    ADD CONSTRAINT "94_187_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_95_chunk 95_189_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_95_chunk
    ADD CONSTRAINT "95_189_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_96_chunk 96_191_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_96_chunk
    ADD CONSTRAINT "96_191_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_97_chunk 97_193_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_97_chunk
    ADD CONSTRAINT "97_193_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_98_chunk 98_195_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_98_chunk
    ADD CONSTRAINT "98_195_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_99_chunk 99_197_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_99_chunk
    ADD CONSTRAINT "99_197_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: _hyper_1_9_chunk 9_17_klines_pkey; Type: CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_9_chunk
    ADD CONSTRAINT "9_17_klines_pkey" PRIMARY KEY (symbol_id, timeframe, "time");


--
-- Name: api_keys api_keys_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.api_keys
    ADD CONSTRAINT api_keys_pkey PRIMARY KEY (id);


--
-- Name: app_settings app_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.app_settings
    ADD CONSTRAINT app_settings_pkey PRIMARY KEY (setting_key);


--
-- Name: audit_logs audit_logs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs
    ADD CONSTRAINT audit_logs_pkey PRIMARY KEY (id);


--
-- Name: backtest_results backtest_results_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.backtest_results
    ADD CONSTRAINT backtest_results_pkey PRIMARY KEY (id);


--
-- Name: credential_access_logs credential_access_logs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.credential_access_logs
    ADD CONSTRAINT credential_access_logs_pkey PRIMARY KEY (id);


--
-- Name: exchange_credentials exchange_credentials_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.exchange_credentials
    ADD CONSTRAINT exchange_credentials_pkey PRIMARY KEY (id);


--
-- Name: execution_cache_meta execution_cache_meta_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.execution_cache_meta
    ADD CONSTRAINT execution_cache_meta_pkey PRIMARY KEY (id);


--
-- Name: execution_cache execution_cache_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.execution_cache
    ADD CONSTRAINT execution_cache_pkey PRIMARY KEY (id);


--
-- Name: orders orders_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.orders
    ADD CONSTRAINT orders_pkey PRIMARY KEY (id);


--
-- Name: performance_snapshots performance_snapshots_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.performance_snapshots
    ADD CONSTRAINT performance_snapshots_pkey PRIMARY KEY (id);


--
-- Name: portfolio_equity_history portfolio_equity_history_credential_id_snapshot_time_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.portfolio_equity_history
    ADD CONSTRAINT portfolio_equity_history_credential_id_snapshot_time_key UNIQUE (credential_id, snapshot_time);


--
-- Name: portfolio_equity_history portfolio_equity_history_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.portfolio_equity_history
    ADD CONSTRAINT portfolio_equity_history_pkey PRIMARY KEY (id);


--
-- Name: position_snapshots position_snapshots_credential_id_symbol_snapshot_time_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.position_snapshots
    ADD CONSTRAINT position_snapshots_credential_id_symbol_snapshot_time_key UNIQUE (credential_id, symbol, snapshot_time);


--
-- Name: position_snapshots position_snapshots_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.position_snapshots
    ADD CONSTRAINT position_snapshots_pkey PRIMARY KEY (id);


--
-- Name: positions positions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.positions
    ADD CONSTRAINT positions_pkey PRIMARY KEY (id);


--
-- Name: price_snapshot price_snapshot_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.price_snapshot
    ADD CONSTRAINT price_snapshot_pkey PRIMARY KEY (snapshot_date, symbol);


--
-- Name: reality_check reality_check_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.reality_check
    ADD CONSTRAINT reality_check_pkey PRIMARY KEY (check_date, symbol);


--
-- Name: signal_marker signal_marker_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.signal_marker
    ADD CONSTRAINT signal_marker_pkey PRIMARY KEY (id);


--
-- Name: signals signals_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.signals
    ADD CONSTRAINT signals_pkey PRIMARY KEY (id);


--
-- Name: strategies strategies_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.strategies
    ADD CONSTRAINT strategies_pkey PRIMARY KEY (id);


--
-- Name: strategy_presets strategy_presets_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.strategy_presets
    ADD CONSTRAINT strategy_presets_pkey PRIMARY KEY (id);


--
-- Name: symbol_fundamental symbol_fundamental_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.symbol_fundamental
    ADD CONSTRAINT symbol_fundamental_pkey PRIMARY KEY (id);


--
-- Name: symbol_info symbol_info_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.symbol_info
    ADD CONSTRAINT symbol_info_pkey PRIMARY KEY (id);


--
-- Name: symbols symbols_base_quote_market_type_exchange_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.symbols
    ADD CONSTRAINT symbols_base_quote_market_type_exchange_key UNIQUE (base, quote, market_type, exchange);


--
-- Name: symbols symbols_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.symbols
    ADD CONSTRAINT symbols_pkey PRIMARY KEY (id);


--
-- Name: telegram_settings telegram_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.telegram_settings
    ADD CONSTRAINT telegram_settings_pkey PRIMARY KEY (id);


--
-- Name: trade_executions trade_executions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.trade_executions
    ADD CONSTRAINT trade_executions_pkey PRIMARY KEY (id);


--
-- Name: trades trades_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.trades
    ADD CONSTRAINT trades_pkey PRIMARY KEY (id);


--
-- Name: execution_cache_meta unique_cache_meta; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.execution_cache_meta
    ADD CONSTRAINT unique_cache_meta UNIQUE (credential_id, exchange);


--
-- Name: exchange_credentials unique_exchange_account; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.exchange_credentials
    ADD CONSTRAINT unique_exchange_account UNIQUE (exchange_id, market_type, is_testnet, exchange_name);


--
-- Name: CONSTRAINT unique_exchange_account ON exchange_credentials; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON CONSTRAINT unique_exchange_account ON public.exchange_credentials IS '거래소별 계좌 구분 (동일 거래소에서 일반/ISA 등 여러 계좌 허용)';


--
-- Name: symbol_fundamental unique_symbol_fundamental; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.symbol_fundamental
    ADD CONSTRAINT unique_symbol_fundamental UNIQUE (symbol_info_id);


--
-- Name: symbol_info unique_symbol_market; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.symbol_info
    ADD CONSTRAINT unique_symbol_market UNIQUE (ticker, market);


--
-- Name: users users_email_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_email_key UNIQUE (email);


--
-- Name: users users_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_pkey PRIMARY KEY (id);


--
-- Name: users users_username_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_username_key UNIQUE (username);


--
-- Name: watchlist watchlist_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.watchlist
    ADD CONSTRAINT watchlist_pkey PRIMARY KEY (id);


--
-- Name: watchlist watchlist_symbol_market_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.watchlist
    ADD CONSTRAINT watchlist_symbol_market_key UNIQUE (symbol, market);


--
-- Name: ohlcv_metadata yahoo_cache_metadata_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.ohlcv_metadata
    ADD CONSTRAINT yahoo_cache_metadata_pkey PRIMARY KEY (symbol, timeframe);


--
-- Name: _hyper_1_100_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_100_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_100_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_100_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_100_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_100_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_100_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_100_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_100_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_100_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_100_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_100_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_101_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_101_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_101_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_101_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_101_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_101_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_101_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_101_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_101_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_101_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_101_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_101_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_102_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_102_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_102_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_102_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_102_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_102_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_102_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_102_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_102_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_102_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_102_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_102_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_103_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_103_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_103_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_103_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_103_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_103_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_103_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_103_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_103_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_103_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_103_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_103_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_104_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_104_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_104_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_104_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_104_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_104_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_104_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_104_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_104_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_104_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_104_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_104_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_105_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_105_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_105_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_105_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_105_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_105_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_105_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_105_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_105_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_105_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_105_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_105_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_106_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_106_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_106_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_106_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_106_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_106_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_106_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_106_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_106_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_106_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_106_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_106_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_107_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_107_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_107_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_107_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_107_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_107_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_107_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_107_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_107_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_107_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_107_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_107_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_108_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_108_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_108_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_108_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_108_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_108_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_108_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_108_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_108_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_108_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_108_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_108_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_109_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_109_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_109_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_109_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_109_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_109_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_109_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_109_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_109_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_109_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_109_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_109_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_10_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_10_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_10_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_10_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_10_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_10_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_10_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_10_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_10_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_10_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_10_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_10_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_110_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_110_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_110_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_110_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_110_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_110_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_110_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_110_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_110_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_110_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_110_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_110_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_11_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_11_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_11_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_11_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_11_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_11_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_11_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_11_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_11_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_11_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_11_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_11_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_12_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_12_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_12_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_12_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_12_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_12_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_12_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_12_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_12_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_12_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_12_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_12_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_13_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_13_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_13_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_13_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_13_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_13_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_13_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_13_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_13_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_13_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_13_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_13_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_14_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_14_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_14_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_14_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_14_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_14_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_14_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_14_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_14_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_14_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_14_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_14_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_15_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_15_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_15_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_15_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_15_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_15_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_15_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_15_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_15_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_15_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_15_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_15_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_16_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_16_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_16_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_16_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_16_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_16_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_16_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_16_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_16_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_16_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_16_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_16_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_17_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_17_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_17_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_17_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_17_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_17_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_17_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_17_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_17_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_17_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_17_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_17_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_18_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_18_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_18_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_18_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_18_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_18_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_18_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_18_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_18_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_18_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_18_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_18_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_19_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_19_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_19_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_19_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_19_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_19_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_19_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_19_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_19_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_19_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_19_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_19_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_1_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_1_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_1_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_1_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_1_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_1_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_1_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_1_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_1_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_1_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_1_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_1_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_20_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_20_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_20_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_20_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_20_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_20_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_20_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_20_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_20_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_20_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_20_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_20_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_21_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_21_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_21_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_21_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_21_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_21_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_21_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_21_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_21_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_21_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_21_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_21_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_22_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_22_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_22_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_22_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_22_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_22_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_22_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_22_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_22_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_22_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_22_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_22_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_23_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_23_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_23_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_23_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_23_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_23_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_23_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_23_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_23_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_23_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_23_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_23_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_24_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_24_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_24_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_24_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_24_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_24_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_24_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_24_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_24_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_24_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_24_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_24_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_25_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_25_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_25_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_25_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_25_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_25_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_25_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_25_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_25_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_25_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_25_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_25_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_26_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_26_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_26_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_26_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_26_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_26_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_26_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_26_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_26_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_26_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_26_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_26_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_27_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_27_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_27_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_27_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_27_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_27_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_27_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_27_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_27_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_27_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_27_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_27_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_28_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_28_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_28_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_28_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_28_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_28_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_28_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_28_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_28_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_28_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_28_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_28_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_29_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_29_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_29_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_29_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_29_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_29_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_29_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_29_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_29_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_29_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_29_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_29_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_2_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_2_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_2_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_2_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_2_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_2_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_2_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_2_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_2_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_2_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_2_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_2_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_30_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_30_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_30_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_30_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_30_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_30_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_30_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_30_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_30_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_30_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_30_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_30_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_31_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_31_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_31_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_31_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_31_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_31_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_31_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_31_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_31_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_31_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_31_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_31_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_32_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_32_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_32_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_32_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_32_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_32_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_32_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_32_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_32_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_32_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_32_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_32_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_33_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_33_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_33_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_33_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_33_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_33_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_33_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_33_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_33_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_33_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_33_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_33_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_34_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_34_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_34_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_34_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_34_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_34_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_34_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_34_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_34_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_34_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_34_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_34_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_35_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_35_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_35_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_35_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_35_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_35_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_35_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_35_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_35_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_35_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_35_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_35_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_36_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_36_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_36_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_36_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_36_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_36_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_36_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_36_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_36_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_36_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_36_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_36_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_37_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_37_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_37_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_37_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_37_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_37_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_37_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_37_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_37_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_37_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_37_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_37_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_38_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_38_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_38_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_38_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_38_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_38_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_38_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_38_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_38_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_38_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_38_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_38_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_39_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_39_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_39_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_39_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_39_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_39_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_39_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_39_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_39_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_39_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_39_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_39_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_3_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_3_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_3_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_3_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_3_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_3_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_3_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_3_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_3_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_3_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_3_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_3_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_40_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_40_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_40_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_40_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_40_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_40_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_40_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_40_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_40_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_40_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_40_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_40_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_41_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_41_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_41_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_41_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_41_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_41_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_41_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_41_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_41_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_41_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_41_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_41_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_42_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_42_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_42_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_42_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_42_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_42_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_42_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_42_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_42_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_42_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_42_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_42_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_43_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_43_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_43_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_43_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_43_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_43_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_43_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_43_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_43_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_43_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_43_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_43_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_44_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_44_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_44_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_44_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_44_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_44_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_44_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_44_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_44_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_44_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_44_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_44_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_45_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_45_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_45_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_45_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_45_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_45_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_45_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_45_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_45_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_45_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_45_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_45_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_46_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_46_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_46_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_46_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_46_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_46_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_46_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_46_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_46_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_46_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_46_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_46_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_47_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_47_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_47_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_47_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_47_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_47_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_47_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_47_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_47_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_47_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_47_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_47_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_48_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_48_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_48_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_48_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_48_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_48_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_48_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_48_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_48_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_48_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_48_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_48_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_49_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_49_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_49_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_49_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_49_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_49_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_49_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_49_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_49_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_49_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_49_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_49_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_4_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_4_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_4_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_4_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_4_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_4_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_4_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_4_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_4_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_4_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_4_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_4_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_50_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_50_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_50_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_50_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_50_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_50_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_50_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_50_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_50_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_50_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_50_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_50_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_51_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_51_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_51_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_51_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_51_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_51_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_51_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_51_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_51_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_51_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_51_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_51_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_52_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_52_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_52_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_52_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_52_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_52_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_52_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_52_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_52_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_52_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_52_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_52_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_53_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_53_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_53_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_53_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_53_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_53_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_53_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_53_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_53_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_53_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_53_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_53_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_54_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_54_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_54_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_54_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_54_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_54_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_54_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_54_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_54_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_54_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_54_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_54_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_55_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_55_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_55_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_55_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_55_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_55_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_55_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_55_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_55_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_55_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_55_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_55_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_56_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_56_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_56_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_56_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_56_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_56_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_56_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_56_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_56_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_56_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_56_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_56_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_57_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_57_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_57_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_57_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_57_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_57_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_57_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_57_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_57_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_57_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_57_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_57_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_5_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_5_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_5_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_5_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_5_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_5_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_5_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_5_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_5_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_5_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_5_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_5_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_62_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_62_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_62_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_62_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_62_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_62_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_62_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_62_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_62_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_62_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_62_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_62_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_63_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_63_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_63_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_63_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_63_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_63_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_63_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_63_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_63_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_63_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_63_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_63_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_64_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_64_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_64_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_64_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_64_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_64_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_64_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_64_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_64_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_64_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_64_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_64_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_65_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_65_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_65_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_65_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_65_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_65_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_65_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_65_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_65_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_65_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_65_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_65_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_66_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_66_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_66_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_66_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_66_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_66_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_66_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_66_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_66_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_66_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_66_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_66_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_67_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_67_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_67_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_67_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_67_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_67_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_67_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_67_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_67_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_67_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_67_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_67_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_68_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_68_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_68_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_68_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_68_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_68_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_68_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_68_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_68_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_68_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_68_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_68_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_69_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_69_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_69_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_69_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_69_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_69_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_69_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_69_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_69_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_69_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_69_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_69_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_6_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_6_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_6_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_6_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_6_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_6_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_6_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_6_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_6_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_6_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_6_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_6_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_70_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_70_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_70_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_70_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_70_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_70_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_70_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_70_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_70_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_70_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_70_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_70_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_71_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_71_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_71_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_71_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_71_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_71_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_71_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_71_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_71_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_71_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_71_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_71_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_72_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_72_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_72_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_72_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_72_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_72_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_72_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_72_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_72_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_72_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_72_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_72_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_73_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_73_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_73_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_73_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_73_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_73_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_73_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_73_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_73_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_73_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_73_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_73_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_74_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_74_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_74_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_74_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_74_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_74_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_74_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_74_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_74_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_74_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_74_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_74_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_75_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_75_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_75_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_75_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_75_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_75_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_75_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_75_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_75_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_75_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_75_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_75_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_76_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_76_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_76_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_76_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_76_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_76_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_76_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_76_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_76_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_76_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_76_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_76_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_77_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_77_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_77_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_77_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_77_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_77_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_77_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_77_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_77_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_77_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_77_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_77_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_78_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_78_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_78_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_78_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_78_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_78_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_78_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_78_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_78_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_78_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_78_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_78_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_79_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_79_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_79_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_79_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_79_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_79_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_79_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_79_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_79_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_79_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_79_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_79_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_7_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_7_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_7_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_7_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_7_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_7_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_7_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_7_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_7_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_7_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_7_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_7_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_80_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_80_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_80_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_80_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_80_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_80_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_80_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_80_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_80_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_80_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_80_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_80_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_81_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_81_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_81_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_81_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_81_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_81_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_81_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_81_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_81_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_81_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_81_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_81_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_82_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_82_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_82_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_82_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_82_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_82_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_82_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_82_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_82_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_82_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_82_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_82_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_83_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_83_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_83_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_83_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_83_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_83_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_83_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_83_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_83_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_83_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_83_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_83_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_84_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_84_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_84_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_84_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_84_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_84_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_84_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_84_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_84_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_84_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_84_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_84_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_85_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_85_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_85_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_85_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_85_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_85_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_85_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_85_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_85_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_85_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_85_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_85_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_86_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_86_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_86_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_86_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_86_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_86_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_86_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_86_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_86_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_86_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_86_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_86_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_87_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_87_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_87_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_87_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_87_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_87_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_87_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_87_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_87_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_87_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_87_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_87_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_88_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_88_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_88_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_88_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_88_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_88_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_88_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_88_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_88_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_88_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_88_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_88_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_89_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_89_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_89_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_89_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_89_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_89_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_89_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_89_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_89_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_89_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_89_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_89_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_8_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_8_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_8_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_8_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_8_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_8_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_8_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_8_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_8_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_8_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_8_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_8_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_90_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_90_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_90_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_90_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_90_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_90_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_90_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_90_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_90_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_90_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_90_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_90_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_91_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_91_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_91_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_91_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_91_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_91_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_91_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_91_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_91_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_91_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_91_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_91_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_92_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_92_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_92_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_92_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_92_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_92_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_92_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_92_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_92_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_92_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_92_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_92_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_93_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_93_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_93_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_93_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_93_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_93_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_93_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_93_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_93_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_93_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_93_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_93_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_94_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_94_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_94_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_94_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_94_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_94_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_94_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_94_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_94_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_94_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_94_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_94_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_95_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_95_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_95_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_95_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_95_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_95_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_95_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_95_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_95_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_95_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_95_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_95_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_96_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_96_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_96_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_96_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_96_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_96_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_96_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_96_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_96_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_96_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_96_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_96_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_97_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_97_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_97_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_97_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_97_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_97_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_97_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_97_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_97_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_97_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_97_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_97_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_98_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_98_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_98_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_98_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_98_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_98_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_98_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_98_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_98_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_98_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_98_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_98_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_99_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_99_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_99_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_99_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_99_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_99_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_99_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_99_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_99_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_99_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_99_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_99_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_9_chunk_idx_klines_symbol_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_9_chunk_idx_klines_symbol_time ON _timescaledb_internal._hyper_1_9_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_9_chunk_idx_klines_symbol_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_9_chunk_idx_klines_symbol_time_1 ON _timescaledb_internal._hyper_1_9_chunk USING btree (symbol_id, "time" DESC);


--
-- Name: _hyper_1_9_chunk_klines_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_9_chunk_klines_time_idx ON _timescaledb_internal._hyper_1_9_chunk USING btree ("time" DESC);


--
-- Name: _hyper_1_9_chunk_klines_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_1_9_chunk_klines_time_idx_1 ON _timescaledb_internal._hyper_1_9_chunk USING btree ("time" DESC);


--
-- Name: _hyper_6_111_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_111_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_111_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_111_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_111_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_111_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_111_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_111_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_111_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_111_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_111_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_111_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_111_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_111_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_111_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_111_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_111_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_111_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_392_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_392_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_392_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_392_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_392_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_392_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_392_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_392_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_392_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_392_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_392_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_392_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_392_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_392_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_392_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_392_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_392_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_392_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_393_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_393_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_393_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_393_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_393_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_393_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_393_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_393_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_393_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_393_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_393_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_393_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_393_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_393_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_393_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_393_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_393_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_393_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_394_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_394_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_394_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_394_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_394_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_394_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_394_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_394_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_394_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_394_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_394_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_394_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_394_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_394_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_394_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_394_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_394_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_394_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_395_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_395_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_395_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_395_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_395_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_395_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_395_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_395_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_395_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_395_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_395_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_395_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_395_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_395_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_395_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_395_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_395_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_395_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_396_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_396_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_396_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_396_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_396_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_396_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_396_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_396_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_396_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_396_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_396_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_396_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_396_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_396_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_396_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_396_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_396_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_396_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_397_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_397_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_397_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_397_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_397_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_397_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_397_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_397_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_397_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_397_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_397_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_397_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_397_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_397_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_397_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_397_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_397_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_397_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_398_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_398_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_398_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_398_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_398_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_398_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_398_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_398_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_398_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_398_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_398_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_398_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_398_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_398_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_398_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_398_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_398_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_398_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_399_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_399_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_399_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_399_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_399_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_399_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_399_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_399_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_399_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_399_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_399_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_399_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_399_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_399_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_399_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_399_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_399_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_399_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_400_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_400_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_400_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_400_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_400_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_400_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_400_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_400_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_400_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_400_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_400_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_400_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_400_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_400_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_400_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_400_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_400_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_400_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_401_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_401_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_401_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_401_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_401_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_401_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_401_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_401_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_401_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_401_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_401_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_401_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_401_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_401_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_401_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_401_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_401_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_401_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_402_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_402_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_402_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_402_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_402_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_402_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_402_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_402_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_402_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_402_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_402_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_402_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_402_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_402_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_402_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_402_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_402_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_402_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_403_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_403_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_403_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_403_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_403_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_403_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_403_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_403_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_403_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_403_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_403_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_403_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_403_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_403_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_403_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_403_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_403_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_403_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_404_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_404_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_404_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_404_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_404_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_404_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_404_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_404_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_404_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_404_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_404_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_404_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_404_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_404_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_404_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_404_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_404_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_404_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_405_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_405_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_405_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_405_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_405_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_405_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_405_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_405_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_405_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_405_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_405_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_405_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_405_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_405_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_405_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_405_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_405_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_405_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_406_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_406_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_406_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_406_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_406_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_406_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_406_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_406_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_406_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_406_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_406_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_406_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_406_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_406_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_406_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_406_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_406_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_406_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_407_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_407_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_407_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_407_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_407_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_407_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_407_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_407_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_407_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_407_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_407_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_407_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_407_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_407_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_407_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_407_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_407_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_407_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_408_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_408_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_408_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_408_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_408_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_408_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_408_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_408_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_408_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_408_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_408_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_408_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_408_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_408_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_408_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_408_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_408_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_408_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_409_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_409_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_409_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_409_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_409_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_409_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_409_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_409_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_409_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_409_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_409_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_409_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_409_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_409_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_409_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_409_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_409_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_409_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_410_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_410_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_410_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_410_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_410_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_410_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_410_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_410_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_410_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_410_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_410_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_410_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_410_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_410_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_410_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_410_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_410_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_410_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_411_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_411_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_411_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_411_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_411_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_411_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_411_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_411_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_411_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_411_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_411_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_411_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_411_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_411_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_411_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_411_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_411_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_411_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_412_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_412_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_412_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_412_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_412_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_412_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_412_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_412_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_412_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_412_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_412_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_412_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_412_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_412_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_412_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_412_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_412_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_412_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_413_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_413_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_413_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_413_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_413_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_413_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_413_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_413_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_413_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_413_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_413_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_413_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_413_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_413_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_413_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_413_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_413_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_413_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_414_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_414_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_414_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_414_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_414_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_414_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_414_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_414_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_414_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_414_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_414_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_414_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_414_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_414_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_414_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_414_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_414_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_414_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_415_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_415_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_415_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_415_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_415_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_415_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_415_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_415_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_415_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_415_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_415_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_415_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_415_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_415_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_415_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_415_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_415_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_415_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_547_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_547_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_547_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_547_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_547_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_547_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_547_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_547_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_547_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_547_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_547_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_547_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_547_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_547_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_547_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_547_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_547_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_547_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_549_chunk_idx_ohlcv_last_fetch; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_549_chunk_idx_ohlcv_last_fetch ON _timescaledb_internal._hyper_6_549_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_549_chunk_idx_ohlcv_last_fetch_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_549_chunk_idx_ohlcv_last_fetch_1 ON _timescaledb_internal._hyper_6_549_chunk USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: _hyper_6_549_chunk_idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE UNIQUE INDEX _hyper_6_549_chunk_idx_ohlcv_symbol_tf_time ON _timescaledb_internal._hyper_6_549_chunk USING btree (symbol, timeframe, open_time);


--
-- Name: _hyper_6_549_chunk_idx_ohlcv_symbol_tf_time_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_549_chunk_idx_ohlcv_symbol_tf_time_1 ON _timescaledb_internal._hyper_6_549_chunk USING btree (symbol, timeframe, open_time DESC);


--
-- Name: _hyper_6_549_chunk_yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_549_chunk_yahoo_candle_cache_open_time_idx ON _timescaledb_internal._hyper_6_549_chunk USING btree (open_time DESC);


--
-- Name: _hyper_6_549_chunk_yahoo_candle_cache_open_time_idx_1; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX _hyper_6_549_chunk_yahoo_candle_cache_open_time_idx_1 ON _timescaledb_internal._hyper_6_549_chunk USING btree (open_time DESC);


--
-- Name: compress_hyper_2_112_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_112_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_112_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_113_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_113_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_113_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_114_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_114_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_114_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_115_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_115_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_115_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_116_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_116_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_116_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_117_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_117_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_117_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_118_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_118_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_118_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_119_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_119_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_119_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_120_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_120_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_120_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_121_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_121_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_121_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_122_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_122_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_122_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_123_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_123_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_123_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_124_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_124_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_124_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_125_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_125_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_125_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_126_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_126_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_126_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_127_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_127_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_127_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_128_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_128_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_128_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_129_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_129_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_129_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_130_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_130_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_130_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_131_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_131_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_131_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_132_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_132_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_132_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_133_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_133_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_133_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_134_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_134_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_134_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_135_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_135_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_135_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_136_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_136_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_136_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_137_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_137_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_137_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_138_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_138_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_138_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_139_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_139_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_139_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_140_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_140_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_140_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_141_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_141_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_141_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_142_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_142_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_142_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_143_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_143_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_143_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_144_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_144_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_144_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_145_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_145_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_145_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_146_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_146_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_146_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_147_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_147_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_147_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_148_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_148_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_148_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_149_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_149_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_149_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_150_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_150_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_150_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_151_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_151_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_151_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_152_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_152_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_152_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_153_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_153_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_153_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_154_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_154_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_154_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_155_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_155_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_155_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_156_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_156_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_156_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_157_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_157_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_157_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_158_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_158_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_158_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_159_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_159_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_159_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_160_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_160_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_160_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_161_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_161_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_161_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_162_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_162_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_162_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_163_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_163_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_163_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_164_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_164_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_164_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_165_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_165_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_165_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_166_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_166_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_166_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_167_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_167_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_167_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_168_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_168_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_168_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_169_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_169_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_169_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_170_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_170_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_170_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_171_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_171_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_171_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_172_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_172_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_172_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_173_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_173_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_173_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_174_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_174_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_174_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_175_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_175_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_175_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_176_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_176_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_176_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_177_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_177_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_177_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_178_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_178_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_178_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_179_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_179_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_179_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_180_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_180_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_180_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_181_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_181_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_181_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_182_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_182_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_182_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_183_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_183_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_183_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_184_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_184_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_184_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_185_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_185_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_185_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_186_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_186_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_186_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_187_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_187_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_187_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_188_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_188_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_188_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_189_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_189_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_189_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_190_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_190_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_190_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_191_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_191_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_191_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_192_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_192_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_192_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_193_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_193_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_193_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_194_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_194_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_194_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_195_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_195_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_195_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_196_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_196_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_196_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_197_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_197_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_197_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_198_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_198_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_198_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_199_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_199_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_199_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_200_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_200_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_200_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_201_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_201_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_201_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_202_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_202_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_202_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_203_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_203_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_203_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_204_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_204_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_204_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_205_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_205_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_205_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_206_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_206_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_206_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_207_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_207_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_207_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_208_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_208_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_208_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_209_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_209_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_209_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_210_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_210_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_210_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_211_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_211_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_211_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_2_551_chunk_symbol_id_timeframe__ts_meta_min_idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_2_551_chunk_symbol_id_timeframe__ts_meta_min_idx ON _timescaledb_internal.compress_hyper_2_551_chunk USING btree (symbol_id, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_416_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_416_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_416_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_417_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_417_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_417_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_418_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_418_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_418_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_419_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_419_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_419_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_420_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_420_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_420_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_421_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_421_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_421_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_422_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_422_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_422_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_423_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_423_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_423_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_424_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_424_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_424_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_425_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_425_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_425_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_426_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_426_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_426_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_427_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_427_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_427_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_428_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_428_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_428_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_429_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_429_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_429_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_430_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_430_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_430_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_431_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_431_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_431_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_432_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_432_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_432_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_433_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_433_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_433_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_434_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_434_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_434_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_435_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_435_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_435_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_436_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_436_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_436_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_437_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_437_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_437_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_438_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_438_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_438_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_548_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_548_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_548_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: compress_hyper_7_550_chunk_symbol_timeframe__ts_meta_min_1__idx; Type: INDEX; Schema: _timescaledb_internal; Owner: -
--

CREATE INDEX compress_hyper_7_550_chunk_symbol_timeframe__ts_meta_min_1__idx ON _timescaledb_internal.compress_hyper_7_550_chunk USING btree (symbol, timeframe, _ts_meta_min_1 DESC, _ts_meta_max_1 DESC);


--
-- Name: idx_api_keys_user; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_api_keys_user ON public.api_keys USING btree (user_id, is_active);


--
-- Name: idx_app_settings_key; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_app_settings_key ON public.app_settings USING btree (setting_key);


--
-- Name: idx_audit_logs_entity; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_logs_entity ON public.audit_logs USING btree (entity_type, entity_id);


--
-- Name: idx_audit_logs_time; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_logs_time ON public.audit_logs USING btree (created_at DESC);


--
-- Name: idx_backtest_results_recent; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_backtest_results_recent ON public.backtest_results USING btree (created_at DESC) WHERE (deleted_at IS NULL);


--
-- Name: idx_backtest_results_strategy; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_backtest_results_strategy ON public.backtest_results USING btree (strategy_id, created_at DESC) WHERE (deleted_at IS NULL);


--
-- Name: idx_backtest_results_symbol; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_backtest_results_symbol ON public.backtest_results USING btree (symbol, created_at DESC) WHERE (deleted_at IS NULL);


--
-- Name: idx_backtest_results_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_backtest_results_type ON public.backtest_results USING btree (strategy_type, created_at DESC) WHERE (deleted_at IS NULL);


--
-- Name: idx_credential_access_logs_action; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_credential_access_logs_action ON public.credential_access_logs USING btree (action);


--
-- Name: idx_credential_access_logs_type_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_credential_access_logs_type_id ON public.credential_access_logs USING btree (credential_type, credential_id);


--
-- Name: idx_equity_history_credential_time; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_equity_history_credential_time ON public.portfolio_equity_history USING btree (credential_id, snapshot_time DESC);


--
-- Name: idx_equity_history_credential_time_asc; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_equity_history_credential_time_asc ON public.portfolio_equity_history USING btree (credential_id, snapshot_time);


--
-- Name: idx_equity_history_time; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_equity_history_time ON public.portfolio_equity_history USING btree (snapshot_time DESC);


--
-- Name: idx_exchange_credentials_active; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_exchange_credentials_active ON public.exchange_credentials USING btree (is_active) WHERE (is_active = true);


--
-- Name: idx_exchange_credentials_exchange; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_exchange_credentials_exchange ON public.exchange_credentials USING btree (exchange_id);


--
-- Name: idx_exec_cache_credential; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_exec_cache_credential ON public.execution_cache USING btree (credential_id);


--
-- Name: idx_exec_cache_exchange; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_exec_cache_exchange ON public.execution_cache USING btree (exchange);


--
-- Name: idx_exec_cache_executed_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_exec_cache_executed_at ON public.execution_cache USING btree (credential_id, executed_at DESC);


--
-- Name: idx_exec_cache_side; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_exec_cache_side ON public.execution_cache USING btree (side);


--
-- Name: idx_exec_cache_symbol; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_exec_cache_symbol ON public.execution_cache USING btree (symbol);


--
-- Name: idx_exec_cache_unique; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_exec_cache_unique ON public.execution_cache USING btree (credential_id, exchange, order_id, COALESCE(trade_id, ''::character varying));


--
-- Name: idx_execution_cache_credential_exchange_trade; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_execution_cache_credential_exchange_trade ON public.execution_cache USING btree (credential_id, exchange, trade_id);


--
-- Name: idx_execution_cache_executed_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_execution_cache_executed_at ON public.execution_cache USING btree (credential_id, executed_at DESC);


--
-- Name: idx_execution_cache_symbol; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_execution_cache_symbol ON public.execution_cache USING btree (credential_id, symbol);


--
-- Name: idx_klines_symbol_time; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_klines_symbol_time ON public.klines USING btree (symbol_id, "time" DESC);


--
-- Name: idx_mv_latest_prices_close; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_mv_latest_prices_close ON public.mv_latest_prices USING btree (close);


--
-- Name: idx_mv_latest_prices_symbol; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_mv_latest_prices_symbol ON public.mv_latest_prices USING btree (symbol);


--
-- Name: idx_mv_latest_prices_time; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_mv_latest_prices_time ON public.mv_latest_prices USING btree (open_time DESC);


--
-- Name: idx_ohlcv_last_fetch; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_ohlcv_last_fetch ON public.ohlcv USING btree (symbol, timeframe, fetched_at DESC);


--
-- Name: idx_ohlcv_symbol_tf_time; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_ohlcv_symbol_tf_time ON public.ohlcv USING btree (symbol, timeframe, open_time);


--
-- Name: idx_orders_exchange; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_orders_exchange ON public.orders USING btree (exchange, exchange_order_id);


--
-- Name: idx_orders_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_orders_status ON public.orders USING btree (status) WHERE (status = ANY (ARRAY['pending'::public.order_status, 'open'::public.order_status, 'partially_filled'::public.order_status]));


--
-- Name: idx_orders_strategy; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_orders_strategy ON public.orders USING btree (strategy_id) WHERE (strategy_id IS NOT NULL);


--
-- Name: idx_orders_symbol; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_orders_symbol ON public.orders USING btree (symbol_id, created_at DESC);


--
-- Name: idx_performance_strategy; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_performance_strategy ON public.performance_snapshots USING btree (strategy_id, snapshot_time DESC);


--
-- Name: idx_position_snapshots_credential_time; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_position_snapshots_credential_time ON public.position_snapshots USING btree (credential_id, snapshot_time DESC);


--
-- Name: idx_position_snapshots_latest; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_position_snapshots_latest ON public.position_snapshots USING btree (credential_id, snapshot_time DESC) WHERE (quantity > (0)::numeric);


--
-- Name: idx_position_snapshots_symbol; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_position_snapshots_symbol ON public.position_snapshots USING btree (credential_id, symbol, snapshot_time DESC);


--
-- Name: idx_positions_credential; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_positions_credential ON public.positions USING btree (credential_id) WHERE (closed_at IS NULL);


--
-- Name: idx_positions_open_credential; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_positions_open_credential ON public.positions USING btree (credential_id, exchange, symbol_id) WHERE (closed_at IS NULL);


--
-- Name: idx_positions_strategy; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_positions_strategy ON public.positions USING btree (strategy_id) WHERE (strategy_id IS NOT NULL);


--
-- Name: idx_positions_symbol; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_positions_symbol ON public.positions USING btree (credential_id, symbol) WHERE (closed_at IS NULL);


--
-- Name: idx_price_snapshot_rank; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_price_snapshot_rank ON public.price_snapshot USING btree (recommend_rank) WHERE (recommend_rank <= 10);


--
-- Name: idx_price_snapshot_source; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_price_snapshot_source ON public.price_snapshot USING btree (recommend_source, snapshot_date DESC);


--
-- Name: idx_price_snapshot_symbol; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_price_snapshot_symbol ON public.price_snapshot USING btree (symbol, snapshot_date DESC);


--
-- Name: idx_reality_check_profitable; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_reality_check_profitable ON public.reality_check USING btree (is_profitable, check_date DESC);


--
-- Name: idx_reality_check_recommend_date; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_reality_check_recommend_date ON public.reality_check USING btree (recommend_date, check_date);


--
-- Name: idx_reality_check_return; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_reality_check_return ON public.reality_check USING btree (actual_return DESC);


--
-- Name: idx_reality_check_source; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_reality_check_source ON public.reality_check USING btree (recommend_source, check_date DESC);


--
-- Name: idx_reality_check_symbol; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_reality_check_symbol ON public.reality_check USING btree (symbol, check_date DESC);


--
-- Name: idx_signal_marker_executed; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_signal_marker_executed ON public.signal_marker USING btree (executed);


--
-- Name: idx_signal_marker_indicators; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_signal_marker_indicators ON public.signal_marker USING gin (indicators);


--
-- Name: idx_signal_marker_signal_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_signal_marker_signal_type ON public.signal_marker USING btree (signal_type);


--
-- Name: idx_signal_marker_strategy; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_signal_marker_strategy ON public.signal_marker USING btree (strategy_id, "timestamp" DESC);


--
-- Name: idx_signal_marker_symbol_timestamp; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_signal_marker_symbol_timestamp ON public.signal_marker USING btree (symbol_id, "timestamp" DESC);


--
-- Name: idx_signals_strategy; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_signals_strategy ON public.signals USING btree (strategy_id, created_at DESC);


--
-- Name: idx_signals_unprocessed; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_signals_unprocessed ON public.signals USING btree (created_at) WHERE (processed_at IS NULL);


--
-- Name: idx_strategies_active; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_strategies_active ON public.strategies USING btree (is_active) WHERE (is_active = true);


--
-- Name: idx_strategies_risk_profile; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_strategies_risk_profile ON public.strategies USING btree (risk_profile);


--
-- Name: idx_strategies_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_strategies_type ON public.strategies USING btree (strategy_type);


--
-- Name: idx_strategy_presets_default; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_strategy_presets_default ON public.strategy_presets USING btree (is_default) WHERE (is_default = true);


--
-- Name: idx_strategy_presets_tags; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_strategy_presets_tags ON public.strategy_presets USING gin (tags);


--
-- Name: idx_strategy_presets_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_strategy_presets_type ON public.strategy_presets USING btree (strategy_type);


--
-- Name: idx_symbol_fundamental_dividend_yield; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_symbol_fundamental_dividend_yield ON public.symbol_fundamental USING btree (dividend_yield DESC);


--
-- Name: idx_symbol_fundamental_market_cap; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_symbol_fundamental_market_cap ON public.symbol_fundamental USING btree (market_cap DESC);


--
-- Name: idx_symbol_fundamental_per; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_symbol_fundamental_per ON public.symbol_fundamental USING btree (per);


--
-- Name: idx_symbol_fundamental_regime; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_symbol_fundamental_regime ON public.symbol_fundamental USING btree (regime) WHERE (regime IS NOT NULL);


--
-- Name: idx_symbol_fundamental_roe; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_symbol_fundamental_roe ON public.symbol_fundamental USING btree (roe DESC);


--
-- Name: idx_symbol_fundamental_symbol_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_symbol_fundamental_symbol_id ON public.symbol_fundamental USING btree (symbol_info_id);


--
-- Name: idx_symbol_fundamental_ttm_squeeze; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_symbol_fundamental_ttm_squeeze ON public.symbol_fundamental USING btree (ttm_squeeze, ttm_squeeze_cnt DESC) WHERE (ttm_squeeze = true);


--
-- Name: INDEX idx_symbol_fundamental_ttm_squeeze; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON INDEX public.idx_symbol_fundamental_ttm_squeeze IS 'TTM Squeeze 필터링 성능 향상 (squeeze 상태인 종목만)';


--
-- Name: idx_symbol_fundamental_valuation; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_symbol_fundamental_valuation ON public.symbol_fundamental USING btree (per, pbr, dividend_yield) WHERE ((per IS NOT NULL) AND (pbr IS NOT NULL));


--
-- Name: idx_symbol_info_fail_count; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_symbol_info_fail_count ON public.symbol_info USING btree (fetch_fail_count DESC) WHERE ((is_active = true) AND (fetch_fail_count > 0));


--
-- Name: idx_symbol_info_last_attempt; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_symbol_info_last_attempt ON public.symbol_info USING btree (last_fetch_attempt DESC) WHERE (is_active = true);


--
-- Name: idx_symbol_info_market; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_symbol_info_market ON public.symbol_info USING btree (market);


--
-- Name: idx_symbol_info_name; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_symbol_info_name ON public.symbol_info USING btree (name);


--
-- Name: idx_symbol_info_search; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_symbol_info_search ON public.symbol_info USING gin (to_tsvector('simple'::regconfig, (((((ticker)::text || ' '::text) || (name)::text) || ' '::text) || (COALESCE(name_en, ''::character varying))::text)));


--
-- Name: idx_symbol_info_ticker; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_symbol_info_ticker ON public.symbol_info USING btree (ticker);


--
-- Name: idx_symbol_info_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_symbol_info_type ON public.symbol_info USING btree (symbol_type);


--
-- Name: idx_symbol_info_yahoo; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_symbol_info_yahoo ON public.symbol_info USING btree (yahoo_symbol);


--
-- Name: idx_symbols_active; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_symbols_active ON public.symbols USING btree (is_active) WHERE (is_active = true);


--
-- Name: idx_symbols_exchange; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_symbols_exchange ON public.symbols USING btree (exchange);


--
-- Name: idx_telegram_single_setting; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_telegram_single_setting ON public.telegram_settings USING btree ((1));


--
-- Name: idx_trade_executions_credential_time; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_trade_executions_credential_time ON public.trade_executions USING btree (credential_id, executed_at DESC);


--
-- Name: idx_trade_executions_lookup; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_trade_executions_lookup ON public.trade_executions USING btree (credential_id, exchange, exchange_trade_id);


--
-- Name: idx_trade_executions_strategy; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_trade_executions_strategy ON public.trade_executions USING btree (strategy_id, executed_at DESC) WHERE (strategy_id IS NOT NULL);


--
-- Name: idx_trade_executions_symbol; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_trade_executions_symbol ON public.trade_executions USING btree (credential_id, symbol, executed_at DESC);


--
-- Name: idx_trade_ticks_symbol; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_trade_ticks_symbol ON public.trade_ticks USING btree (symbol_id, "time" DESC);


--
-- Name: idx_trades_executed; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_trades_executed ON public.trades USING btree (executed_at DESC);


--
-- Name: idx_trades_order; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_trades_order ON public.trades USING btree (order_id);


--
-- Name: idx_trades_symbol; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_trades_symbol ON public.trades USING btree (symbol_id, executed_at DESC);


--
-- Name: idx_users_email; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_users_email ON public.users USING btree (email);


--
-- Name: idx_users_username; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_users_username ON public.users USING btree (username);


--
-- Name: idx_watchlist_active; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_watchlist_active ON public.watchlist USING btree (is_active) WHERE (is_active = true);


--
-- Name: idx_watchlist_sort; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_watchlist_sort ON public.watchlist USING btree (sort_order);


--
-- Name: klines_time_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX klines_time_idx ON public.klines USING btree ("time" DESC);


--
-- Name: price_snapshot_snapshot_date_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX price_snapshot_snapshot_date_idx ON public.price_snapshot USING btree (snapshot_date DESC);


--
-- Name: reality_check_check_date_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX reality_check_check_date_idx ON public.reality_check USING btree (check_date DESC);


--
-- Name: trade_ticks_time_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX trade_ticks_time_idx ON public.trade_ticks USING btree ("time" DESC);


--
-- Name: yahoo_candle_cache_open_time_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX yahoo_candle_cache_open_time_idx ON public.ohlcv USING btree (open_time DESC);


--
-- Name: _hyper_6_111_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_111_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_392_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_392_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_393_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_393_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_394_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_394_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_395_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_395_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_396_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_396_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_397_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_397_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_398_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_398_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_399_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_399_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_400_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_400_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_401_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_401_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_402_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_402_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_403_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_403_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_404_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_404_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_405_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_405_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_406_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_406_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_407_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_407_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_408_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_408_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_409_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_409_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_410_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_410_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_411_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_411_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_412_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_412_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_413_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_413_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_414_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_414_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_415_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_415_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_547_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_547_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: _hyper_6_549_chunk trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: _timescaledb_internal; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON _timescaledb_internal._hyper_6_549_chunk FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: exchange_credentials trigger_exchange_credentials_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER trigger_exchange_credentials_updated_at BEFORE UPDATE ON public.exchange_credentials FOR EACH ROW EXECUTE FUNCTION public.update_exchange_credentials_updated_at();


--
-- Name: strategy_presets trigger_strategy_presets_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER trigger_strategy_presets_updated_at BEFORE UPDATE ON public.strategy_presets FOR EACH ROW EXECUTE FUNCTION public.update_exchange_credentials_updated_at();


--
-- Name: telegram_settings trigger_telegram_settings_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER trigger_telegram_settings_updated_at BEFORE UPDATE ON public.telegram_settings FOR EACH ROW EXECUTE FUNCTION public.update_exchange_credentials_updated_at();


--
-- Name: ohlcv trigger_update_ohlcv_metadata; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER trigger_update_ohlcv_metadata AFTER INSERT ON public.ohlcv FOR EACH ROW EXECUTE FUNCTION public.update_ohlcv_metadata();


--
-- Name: api_keys update_api_keys_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_api_keys_updated_at BEFORE UPDATE ON public.api_keys FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: orders update_orders_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_orders_updated_at BEFORE UPDATE ON public.orders FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: positions update_positions_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_positions_updated_at BEFORE UPDATE ON public.positions FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: strategies update_strategies_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_strategies_updated_at BEFORE UPDATE ON public.strategies FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: symbols update_symbols_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_symbols_updated_at BEFORE UPDATE ON public.symbols FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: trade_executions update_trade_executions_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_trade_executions_updated_at BEFORE UPDATE ON public.trade_executions FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: users update_users_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON public.users FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: _hyper_1_100_chunk 100_200_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_100_chunk
    ADD CONSTRAINT "100_200_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_101_chunk 101_202_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_101_chunk
    ADD CONSTRAINT "101_202_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_102_chunk 102_204_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_102_chunk
    ADD CONSTRAINT "102_204_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_103_chunk 103_206_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_103_chunk
    ADD CONSTRAINT "103_206_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_104_chunk 104_208_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_104_chunk
    ADD CONSTRAINT "104_208_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_105_chunk 105_210_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_105_chunk
    ADD CONSTRAINT "105_210_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_106_chunk 106_212_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_106_chunk
    ADD CONSTRAINT "106_212_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_107_chunk 107_214_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_107_chunk
    ADD CONSTRAINT "107_214_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_108_chunk 108_216_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_108_chunk
    ADD CONSTRAINT "108_216_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_109_chunk 109_218_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_109_chunk
    ADD CONSTRAINT "109_218_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_10_chunk 10_20_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_10_chunk
    ADD CONSTRAINT "10_20_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_110_chunk 110_220_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_110_chunk
    ADD CONSTRAINT "110_220_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_11_chunk 11_22_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_11_chunk
    ADD CONSTRAINT "11_22_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_12_chunk 12_24_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_12_chunk
    ADD CONSTRAINT "12_24_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_13_chunk 13_26_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_13_chunk
    ADD CONSTRAINT "13_26_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_14_chunk 14_28_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_14_chunk
    ADD CONSTRAINT "14_28_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_15_chunk 15_30_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_15_chunk
    ADD CONSTRAINT "15_30_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_16_chunk 16_32_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_16_chunk
    ADD CONSTRAINT "16_32_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_17_chunk 17_34_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_17_chunk
    ADD CONSTRAINT "17_34_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_18_chunk 18_36_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_18_chunk
    ADD CONSTRAINT "18_36_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_19_chunk 19_38_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_19_chunk
    ADD CONSTRAINT "19_38_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_1_chunk 1_2_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_1_chunk
    ADD CONSTRAINT "1_2_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_20_chunk 20_40_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_20_chunk
    ADD CONSTRAINT "20_40_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_21_chunk 21_42_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_21_chunk
    ADD CONSTRAINT "21_42_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_22_chunk 22_44_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_22_chunk
    ADD CONSTRAINT "22_44_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_23_chunk 23_46_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_23_chunk
    ADD CONSTRAINT "23_46_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_24_chunk 24_48_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_24_chunk
    ADD CONSTRAINT "24_48_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_25_chunk 25_50_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_25_chunk
    ADD CONSTRAINT "25_50_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_26_chunk 26_52_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_26_chunk
    ADD CONSTRAINT "26_52_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_27_chunk 27_54_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_27_chunk
    ADD CONSTRAINT "27_54_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_28_chunk 28_56_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_28_chunk
    ADD CONSTRAINT "28_56_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_29_chunk 29_58_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_29_chunk
    ADD CONSTRAINT "29_58_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_2_chunk 2_4_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_2_chunk
    ADD CONSTRAINT "2_4_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_30_chunk 30_60_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_30_chunk
    ADD CONSTRAINT "30_60_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_31_chunk 31_62_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_31_chunk
    ADD CONSTRAINT "31_62_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_32_chunk 32_64_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_32_chunk
    ADD CONSTRAINT "32_64_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_33_chunk 33_66_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_33_chunk
    ADD CONSTRAINT "33_66_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_34_chunk 34_68_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_34_chunk
    ADD CONSTRAINT "34_68_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_35_chunk 35_70_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_35_chunk
    ADD CONSTRAINT "35_70_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_36_chunk 36_72_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_36_chunk
    ADD CONSTRAINT "36_72_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_37_chunk 37_74_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_37_chunk
    ADD CONSTRAINT "37_74_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_38_chunk 38_76_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_38_chunk
    ADD CONSTRAINT "38_76_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_39_chunk 39_78_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_39_chunk
    ADD CONSTRAINT "39_78_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_3_chunk 3_6_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_3_chunk
    ADD CONSTRAINT "3_6_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_40_chunk 40_80_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_40_chunk
    ADD CONSTRAINT "40_80_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_41_chunk 41_82_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_41_chunk
    ADD CONSTRAINT "41_82_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_42_chunk 42_84_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_42_chunk
    ADD CONSTRAINT "42_84_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_43_chunk 43_86_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_43_chunk
    ADD CONSTRAINT "43_86_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_44_chunk 44_88_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_44_chunk
    ADD CONSTRAINT "44_88_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_45_chunk 45_90_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_45_chunk
    ADD CONSTRAINT "45_90_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_46_chunk 46_92_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_46_chunk
    ADD CONSTRAINT "46_92_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_47_chunk 47_94_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_47_chunk
    ADD CONSTRAINT "47_94_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_48_chunk 48_96_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_48_chunk
    ADD CONSTRAINT "48_96_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_49_chunk 49_98_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_49_chunk
    ADD CONSTRAINT "49_98_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_4_chunk 4_8_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_4_chunk
    ADD CONSTRAINT "4_8_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_50_chunk 50_100_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_50_chunk
    ADD CONSTRAINT "50_100_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_51_chunk 51_102_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_51_chunk
    ADD CONSTRAINT "51_102_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_52_chunk 52_104_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_52_chunk
    ADD CONSTRAINT "52_104_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_53_chunk 53_106_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_53_chunk
    ADD CONSTRAINT "53_106_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_54_chunk 54_108_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_54_chunk
    ADD CONSTRAINT "54_108_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_55_chunk 55_110_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_55_chunk
    ADD CONSTRAINT "55_110_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_56_chunk 56_112_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_56_chunk
    ADD CONSTRAINT "56_112_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_57_chunk 57_114_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_57_chunk
    ADD CONSTRAINT "57_114_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_5_chunk 5_10_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_5_chunk
    ADD CONSTRAINT "5_10_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_62_chunk 62_124_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_62_chunk
    ADD CONSTRAINT "62_124_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_63_chunk 63_126_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_63_chunk
    ADD CONSTRAINT "63_126_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_64_chunk 64_128_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_64_chunk
    ADD CONSTRAINT "64_128_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_65_chunk 65_130_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_65_chunk
    ADD CONSTRAINT "65_130_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_66_chunk 66_132_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_66_chunk
    ADD CONSTRAINT "66_132_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_67_chunk 67_134_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_67_chunk
    ADD CONSTRAINT "67_134_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_68_chunk 68_136_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_68_chunk
    ADD CONSTRAINT "68_136_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_69_chunk 69_138_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_69_chunk
    ADD CONSTRAINT "69_138_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_6_chunk 6_12_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_6_chunk
    ADD CONSTRAINT "6_12_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_70_chunk 70_140_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_70_chunk
    ADD CONSTRAINT "70_140_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_71_chunk 71_142_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_71_chunk
    ADD CONSTRAINT "71_142_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_72_chunk 72_144_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_72_chunk
    ADD CONSTRAINT "72_144_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_73_chunk 73_146_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_73_chunk
    ADD CONSTRAINT "73_146_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_74_chunk 74_148_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_74_chunk
    ADD CONSTRAINT "74_148_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_75_chunk 75_150_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_75_chunk
    ADD CONSTRAINT "75_150_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_76_chunk 76_152_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_76_chunk
    ADD CONSTRAINT "76_152_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_77_chunk 77_154_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_77_chunk
    ADD CONSTRAINT "77_154_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_78_chunk 78_156_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_78_chunk
    ADD CONSTRAINT "78_156_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_79_chunk 79_158_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_79_chunk
    ADD CONSTRAINT "79_158_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_7_chunk 7_14_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_7_chunk
    ADD CONSTRAINT "7_14_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_80_chunk 80_160_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_80_chunk
    ADD CONSTRAINT "80_160_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_81_chunk 81_162_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_81_chunk
    ADD CONSTRAINT "81_162_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_82_chunk 82_164_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_82_chunk
    ADD CONSTRAINT "82_164_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_83_chunk 83_166_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_83_chunk
    ADD CONSTRAINT "83_166_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_84_chunk 84_168_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_84_chunk
    ADD CONSTRAINT "84_168_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_85_chunk 85_170_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_85_chunk
    ADD CONSTRAINT "85_170_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_86_chunk 86_172_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_86_chunk
    ADD CONSTRAINT "86_172_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_87_chunk 87_174_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_87_chunk
    ADD CONSTRAINT "87_174_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_88_chunk 88_176_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_88_chunk
    ADD CONSTRAINT "88_176_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_89_chunk 89_178_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_89_chunk
    ADD CONSTRAINT "89_178_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_8_chunk 8_16_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_8_chunk
    ADD CONSTRAINT "8_16_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_90_chunk 90_180_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_90_chunk
    ADD CONSTRAINT "90_180_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_91_chunk 91_182_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_91_chunk
    ADD CONSTRAINT "91_182_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_92_chunk 92_184_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_92_chunk
    ADD CONSTRAINT "92_184_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_93_chunk 93_186_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_93_chunk
    ADD CONSTRAINT "93_186_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_94_chunk 94_188_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_94_chunk
    ADD CONSTRAINT "94_188_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_95_chunk 95_190_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_95_chunk
    ADD CONSTRAINT "95_190_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_96_chunk 96_192_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_96_chunk
    ADD CONSTRAINT "96_192_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_97_chunk 97_194_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_97_chunk
    ADD CONSTRAINT "97_194_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_98_chunk 98_196_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_98_chunk
    ADD CONSTRAINT "98_196_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_99_chunk 99_198_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_99_chunk
    ADD CONSTRAINT "99_198_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: _hyper_1_9_chunk 9_18_klines_symbol_id_fkey; Type: FK CONSTRAINT; Schema: _timescaledb_internal; Owner: -
--

ALTER TABLE ONLY _timescaledb_internal._hyper_1_9_chunk
    ADD CONSTRAINT "9_18_klines_symbol_id_fkey" FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: api_keys api_keys_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.api_keys
    ADD CONSTRAINT api_keys_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id);


--
-- Name: execution_cache execution_cache_credential_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.execution_cache
    ADD CONSTRAINT execution_cache_credential_id_fkey FOREIGN KEY (credential_id) REFERENCES public.exchange_credentials(id) ON DELETE CASCADE;


--
-- Name: execution_cache_meta execution_cache_meta_credential_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.execution_cache_meta
    ADD CONSTRAINT execution_cache_meta_credential_id_fkey FOREIGN KEY (credential_id) REFERENCES public.exchange_credentials(id) ON DELETE CASCADE;


--
-- Name: orders orders_symbol_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.orders
    ADD CONSTRAINT orders_symbol_id_fkey FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: performance_snapshots performance_snapshots_strategy_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.performance_snapshots
    ADD CONSTRAINT performance_snapshots_strategy_id_fkey FOREIGN KEY (strategy_id) REFERENCES public.strategies(id);


--
-- Name: portfolio_equity_history portfolio_equity_history_credential_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.portfolio_equity_history
    ADD CONSTRAINT portfolio_equity_history_credential_id_fkey FOREIGN KEY (credential_id) REFERENCES public.exchange_credentials(id) ON DELETE CASCADE;


--
-- Name: position_snapshots position_snapshots_credential_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.position_snapshots
    ADD CONSTRAINT position_snapshots_credential_id_fkey FOREIGN KEY (credential_id) REFERENCES public.exchange_credentials(id) ON DELETE CASCADE;


--
-- Name: positions positions_credential_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.positions
    ADD CONSTRAINT positions_credential_id_fkey FOREIGN KEY (credential_id) REFERENCES public.exchange_credentials(id);


--
-- Name: positions positions_symbol_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.positions
    ADD CONSTRAINT positions_symbol_id_fkey FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: signal_marker signal_marker_symbol_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.signal_marker
    ADD CONSTRAINT signal_marker_symbol_id_fkey FOREIGN KEY (symbol_id) REFERENCES public.symbol_info(id) ON DELETE CASCADE;


--
-- Name: signals signals_order_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.signals
    ADD CONSTRAINT signals_order_id_fkey FOREIGN KEY (order_id) REFERENCES public.orders(id);


--
-- Name: signals signals_symbol_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.signals
    ADD CONSTRAINT signals_symbol_id_fkey FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- Name: symbol_fundamental symbol_fundamental_symbol_info_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.symbol_fundamental
    ADD CONSTRAINT symbol_fundamental_symbol_info_id_fkey FOREIGN KEY (symbol_info_id) REFERENCES public.symbol_info(id) ON DELETE CASCADE;


--
-- Name: trade_executions trade_executions_credential_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.trade_executions
    ADD CONSTRAINT trade_executions_credential_id_fkey FOREIGN KEY (credential_id) REFERENCES public.exchange_credentials(id) ON DELETE CASCADE;


--
-- Name: trade_executions trade_executions_order_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.trade_executions
    ADD CONSTRAINT trade_executions_order_id_fkey FOREIGN KEY (order_id) REFERENCES public.orders(id) ON DELETE SET NULL;


--
-- Name: trades trades_order_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.trades
    ADD CONSTRAINT trades_order_id_fkey FOREIGN KEY (order_id) REFERENCES public.orders(id);


--
-- Name: trades trades_symbol_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.trades
    ADD CONSTRAINT trades_symbol_id_fkey FOREIGN KEY (symbol_id) REFERENCES public.symbols(id);


--
-- PostgreSQL database dump complete
--

