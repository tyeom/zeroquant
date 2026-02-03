-- =====================================================
-- 06_execution_tracking.sql
-- 거래 체결 내역 캐시 시스템
-- =====================================================
--
-- 원본 마이그레이션: 011_execution_cache.sql
--
-- 포함 내용:
-- - execution_cache: 거래소 체결 내역 캐시 (거래소 중립)
-- - execution_cache_meta: 동기화 메타데이터 (마지막 동기화 정보)
-- - 증분 조회 지원: 마지막 저장일부터만 조회
--
-- 지원 거래소: KIS, Binance, Coinbase 등
--
-- =====================================================

-- =====================================================
-- EXECUTION_CACHE TABLE
-- 거래소 체결 내역 캐시
-- =====================================================

CREATE TABLE IF NOT EXISTS execution_cache (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- 계좌 식별
    credential_id UUID NOT NULL REFERENCES exchange_credentials(id) ON DELETE CASCADE,

    -- 거래소 식별
    exchange VARCHAR(50) NOT NULL,                  -- kis, binance, coinbase 등

    -- 체결 정보 (정규화)
    executed_at TIMESTAMPTZ NOT NULL,               -- 체결 일시

    -- 종목 정보
    symbol VARCHAR(50) NOT NULL,                    -- 거래소별 심볼 (005930, BTCUSDT)
    normalized_symbol VARCHAR(50),                  -- 정규화 심볼 (005930.KS, BTC/USDT)

    -- 체결 상세
    side VARCHAR(10) NOT NULL,                      -- buy, sell
    quantity DECIMAL(30, 15) NOT NULL,              -- 체결 수량
    price DECIMAL(30, 15) NOT NULL,                 -- 체결 단가
    amount DECIMAL(30, 15) NOT NULL,                -- 체결 금액 (quantity * price)

    -- 수수료
    fee DECIMAL(30, 15),                            -- 수수료
    fee_currency VARCHAR(20),                       -- 수수료 통화 (KRW, USDT 등)

    -- 주문/체결 ID
    order_id VARCHAR(100) NOT NULL,                 -- 거래소 주문 ID
    trade_id VARCHAR(100),                          -- 거래소 체결 ID

    -- 주문 유형
    order_type VARCHAR(20),                         -- market, limit, stop_limit 등

    -- 거래소별 원본 데이터
    raw_data JSONB,                                 -- 디버깅/확장용 (거래소 응답 원본)

    -- 시스템 필드
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- 인덱스
CREATE INDEX IF NOT EXISTS idx_exec_cache_credential ON execution_cache(credential_id);
CREATE INDEX IF NOT EXISTS idx_exec_cache_exchange ON execution_cache(exchange);
CREATE INDEX IF NOT EXISTS idx_exec_cache_executed_at ON execution_cache(credential_id, executed_at DESC);
CREATE INDEX IF NOT EXISTS idx_exec_cache_symbol ON execution_cache(symbol);
CREATE INDEX IF NOT EXISTS idx_exec_cache_side ON execution_cache(side);

-- 중복 방지: 거래소 + 계좌 + 주문ID + 체결ID 조합 유일 (함수형 유니크 인덱스)
CREATE UNIQUE INDEX IF NOT EXISTS idx_exec_cache_unique
ON execution_cache(credential_id, exchange, order_id, COALESCE(trade_id, ''));

COMMENT ON TABLE execution_cache IS '체결 내역 캐시 - 거래소 중립적 증분 조회 지원';
COMMENT ON COLUMN execution_cache.exchange IS '거래소 식별자 (kis, binance, coinbase 등)';
COMMENT ON COLUMN execution_cache.normalized_symbol IS '정규화된 심볼 형식 (BTC/USDT, 005930.KS)';
COMMENT ON COLUMN execution_cache.raw_data IS '거래소 API 응답 원본 (디버깅 및 확장 필드 추출용)';

-- =====================================================
-- EXECUTION_CACHE_META TABLE
-- 체결 캐시 메타데이터 (동기화 상태 추적)
-- =====================================================

CREATE TABLE IF NOT EXISTS execution_cache_meta (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    credential_id UUID NOT NULL REFERENCES exchange_credentials(id) ON DELETE CASCADE,
    exchange VARCHAR(50) NOT NULL,                  -- 거래소별 메타데이터

    -- 동기화 범위
    earliest_date DATE,                             -- 가장 오래된 캐시 일자
    latest_date DATE,                               -- 가장 최근 캐시 일자 (다음 조회 시작점)

    -- 동기화 상태
    total_records INT DEFAULT 0,                    -- 총 레코드 수
    last_sync_at TIMESTAMPTZ,                       -- 마지막 동기화 시간
    last_sync_status VARCHAR(20),                   -- success, failed, partial
    last_sync_message TEXT,                         -- 상세 메시지 (에러 로그 등)

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- 계좌당 거래소별 하나의 메타데이터
    CONSTRAINT unique_cache_meta UNIQUE (credential_id, exchange)
);

COMMENT ON TABLE execution_cache_meta IS '체결 캐시 메타데이터 - 거래소별 마지막 동기화 정보';
COMMENT ON COLUMN execution_cache_meta.latest_date IS '가장 최근 캐시된 일자 - 다음 조회 시작점 (증분 조회용)';
COMMENT ON COLUMN execution_cache_meta.last_sync_status IS '마지막 동기화 상태: success, failed, partial';

-- =====================================================
-- 사용 예시
-- =====================================================
--
-- 1. 증분 조회 (마지막 동기화 이후 데이터만):
--    SELECT latest_date FROM execution_cache_meta
--    WHERE credential_id = ? AND exchange = 'kis';
--
-- 2. 특정 계좌의 최근 체결 조회:
--    SELECT * FROM execution_cache
--    WHERE credential_id = ?
--    ORDER BY executed_at DESC LIMIT 100;
--
-- 3. 종목별 체결 통계:
--    SELECT symbol, COUNT(*), SUM(amount)
--    FROM execution_cache
--    WHERE credential_id = ?
--    GROUP BY symbol;
--
-- =====================================================
