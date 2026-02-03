-- =====================================================
-- 04_symbol_metadata.sql
-- 심볼 메타데이터 및 펀더멘털 정보 관리
-- =====================================================
--
-- 원본 마이그레이션: 012_symbol_info.sql, 020_symbol_fundamental.sql,
--                  021_fix_fundamental_decimal_precision.sql,
--                  023_symbol_fetch_failure_tracking.sql,
--                  024_add_symbol_type.sql
--
-- 포함 내용:
-- - symbol_info: 심볼 기본 정보 (티커, 회사명, 시장)
-- - symbol_fundamental: 펀더멘털 데이터 (PER, PBR, 재무 지표 등)
-- - 데이터 수집 실패 추적 (fetch_fail_count, last_fetch_error)
-- - 심볼 타입 구분 (STOCK, ETF, ETN, WARRANT 등)
-- - 펀더멘털 통합 뷰 (v_symbol_with_fundamental)
-- - 수집 실패 관리 함수 및 뷰
--
-- =====================================================

-- =====================================================
-- SYMBOL_INFO TABLE
-- 심볼 기본 정보 (티커, 회사명, 시장)
-- =====================================================

CREATE TABLE IF NOT EXISTS symbol_info (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- 심볼 식별
    ticker VARCHAR(20) NOT NULL,                    -- 티커 코드 (AAPL, 005930)
    name VARCHAR(200) NOT NULL,                     -- 회사명 (Apple Inc., 삼성전자)
    name_en VARCHAR(200),                           -- 영문 회사명 (Samsung Electronics)

    -- 시장 정보
    market VARCHAR(20) NOT NULL,                    -- KR, US, JP 등
    exchange VARCHAR(50),                           -- NYSE, NASDAQ, KRX, KOSDAQ
    sector VARCHAR(100),                            -- 섹터/업종

    -- Yahoo Finance 호환 심볼
    yahoo_symbol VARCHAR(30),                       -- 005930.KS, AAPL

    -- 심볼 타입 (024에서 추가)
    symbol_type VARCHAR(20) DEFAULT 'STOCK',        -- STOCK, ETF, ETN, WARRANT, REIT, PREFERRED

    -- 상태
    is_active BOOLEAN DEFAULT true,

    -- 데이터 수집 실패 추적 (023에서 추가)
    fetch_fail_count INTEGER DEFAULT 0,             -- 연속 데이터 수집 실패 횟수
    last_fetch_error TEXT,                          -- 마지막 수집 실패 오류 메시지
    last_fetch_attempt TIMESTAMPTZ,                 -- 마지막 데이터 수집 시도 시간

    -- 시스템 필드
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- 유일 제약
    CONSTRAINT unique_symbol_market UNIQUE (ticker, market)
);

-- 기본 인덱스
CREATE INDEX IF NOT EXISTS idx_symbol_info_ticker ON symbol_info(ticker);
CREATE INDEX IF NOT EXISTS idx_symbol_info_name ON symbol_info(name);
CREATE INDEX IF NOT EXISTS idx_symbol_info_market ON symbol_info(market);
CREATE INDEX IF NOT EXISTS idx_symbol_info_yahoo ON symbol_info(yahoo_symbol);
CREATE INDEX IF NOT EXISTS idx_symbol_info_type ON symbol_info(symbol_type);

-- 전문 검색용 인덱스 (티커 + 회사명)
CREATE INDEX IF NOT EXISTS idx_symbol_info_search
ON symbol_info USING gin(to_tsvector('simple', ticker || ' ' || name || ' ' || COALESCE(name_en, '')));

-- 실패 추적용 인덱스
CREATE INDEX IF NOT EXISTS idx_symbol_info_fail_count
    ON symbol_info(fetch_fail_count DESC)
    WHERE is_active = true AND fetch_fail_count > 0;

CREATE INDEX IF NOT EXISTS idx_symbol_info_last_attempt
    ON symbol_info(last_fetch_attempt DESC)
    WHERE is_active = true;

COMMENT ON TABLE symbol_info IS '심볼 메타데이터 - 티커와 회사명 매핑';
COMMENT ON COLUMN symbol_info.ticker IS '티커 코드 (예: AAPL, 005930)';
COMMENT ON COLUMN symbol_info.name IS '회사명 (예: Apple Inc., 삼성전자)';
COMMENT ON COLUMN symbol_info.yahoo_symbol IS 'Yahoo Finance 호환 심볼 (예: 005930.KS, AAPL)';
COMMENT ON COLUMN symbol_info.symbol_type IS '종목 유형 (STOCK, ETF, ETN, WARRANT, REIT, PREFERRED)';
COMMENT ON COLUMN symbol_info.fetch_fail_count IS '연속 데이터 수집 실패 횟수';
COMMENT ON COLUMN symbol_info.last_fetch_error IS '마지막 수집 실패 오류 메시지';
COMMENT ON COLUMN symbol_info.last_fetch_attempt IS '마지막 데이터 수집 시도 시간';

-- =====================================================
-- SYMBOL_FUNDAMENTAL TABLE
-- 심볼 펀더멘털 정보 (PER, PBR, 재무 지표 등)
-- 주의: DECIMAL 정밀도는 021의 수정사항 반영 (극단값 허용)
-- =====================================================

CREATE TABLE IF NOT EXISTS symbol_fundamental (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- symbol_info 연계 (FK)
    symbol_info_id UUID NOT NULL REFERENCES symbol_info(id) ON DELETE CASCADE,

    -- 기본 시장 데이터
    market_cap DECIMAL(20, 2),                      -- 시가총액 (원/달러)
    shares_outstanding BIGINT,                      -- 발행주식수
    float_shares BIGINT,                            -- 유통주식수

    -- 가격 관련
    week_52_high DECIMAL(20, 4),                    -- 52주 최고가
    week_52_low DECIMAL(20, 4),                     -- 52주 최저가
    avg_volume_10d BIGINT,                          -- 10일 평균 거래량
    avg_volume_3m BIGINT,                           -- 3개월 평균 거래량

    -- 밸류에이션 지표
    per DECIMAL(12, 4),                             -- PER (주가수익비율)
    forward_per DECIMAL(12, 4),                     -- Forward PER
    pbr DECIMAL(12, 4),                             -- PBR (주가순자산비율)
    psr DECIMAL(12, 4),                             -- PSR (주가매출비율)
    pcr DECIMAL(12, 4),                             -- PCR (주가현금흐름비율)
    ev_ebitda DECIMAL(12, 4),                       -- EV/EBITDA

    -- 주당 지표
    eps DECIMAL(20, 4),                             -- EPS (주당순이익)
    bps DECIMAL(20, 4),                             -- BPS (주당순자산)
    dps DECIMAL(20, 4),                             -- DPS (주당배당금)
    sps DECIMAL(20, 4),                             -- SPS (주당매출)

    -- 배당 관련 (021: DECIMAL(12, 4)로 확장)
    dividend_yield DECIMAL(12, 4),                  -- 배당수익률 (%)
    dividend_payout_ratio DECIMAL(12, 4),           -- 배당성향 (%)
    ex_dividend_date DATE,                          -- 배당락일

    -- 재무제표 요약 (최근 연간)
    revenue DECIMAL(20, 2),                         -- 매출액
    operating_income DECIMAL(20, 2),                -- 영업이익
    net_income DECIMAL(20, 2),                      -- 순이익
    total_assets DECIMAL(20, 2),                    -- 총자산
    total_liabilities DECIMAL(20, 2),               -- 총부채
    total_equity DECIMAL(20, 2),                    -- 자기자본

    -- 수익성 지표 (021: DECIMAL(12, 4)로 확장 - 극단값 허용)
    roe DECIMAL(12, 4),                             -- ROE (자기자본이익률) %
    roa DECIMAL(12, 4),                             -- ROA (총자산이익률) %
    operating_margin DECIMAL(12, 4),                -- 영업이익률 % (적자 기업의 큰 음수 허용)
    net_profit_margin DECIMAL(12, 4),               -- 순이익률 %
    gross_margin DECIMAL(12, 4),                    -- 매출총이익률 %

    -- 안정성 지표
    debt_ratio DECIMAL(12, 4),                      -- 부채비율 (%)
    current_ratio DECIMAL(12, 4),                   -- 유동비율 (%)
    quick_ratio DECIMAL(12, 4),                     -- 당좌비율 (%)
    interest_coverage DECIMAL(12, 4),               -- 이자보상배율

    -- 성장성 지표 (021: DECIMAL(12, 4)로 확장 - 스타트업 고성장 허용)
    revenue_growth_yoy DECIMAL(12, 4),              -- 매출 성장률 YoY %
    earnings_growth_yoy DECIMAL(12, 4),             -- 이익 성장률 YoY % (적자→흑자 전환 시 극단값 허용)
    revenue_growth_3y DECIMAL(12, 4),               -- 매출 3년 CAGR %
    earnings_growth_3y DECIMAL(12, 4),              -- 이익 3년 CAGR %

    -- 메타데이터
    data_source VARCHAR(50),                        -- 데이터 소스 (KRX, Yahoo, etc.)
    fiscal_year_end VARCHAR(10),                    -- 회계연도 종료월 (예: "12")
    currency VARCHAR(10) DEFAULT 'KRW',             -- 통화 (KRW, USD, etc.)

    -- 시스템 필드
    fetched_at TIMESTAMPTZ DEFAULT NOW(),           -- 데이터 수집 시점
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- 유일 제약 (한 심볼당 하나의 Fundamental 레코드)
    CONSTRAINT unique_symbol_fundamental UNIQUE (symbol_info_id)
);

-- 인덱스
CREATE INDEX IF NOT EXISTS idx_symbol_fundamental_symbol_id ON symbol_fundamental(symbol_info_id);
CREATE INDEX IF NOT EXISTS idx_symbol_fundamental_market_cap ON symbol_fundamental(market_cap DESC);
CREATE INDEX IF NOT EXISTS idx_symbol_fundamental_per ON symbol_fundamental(per);
CREATE INDEX IF NOT EXISTS idx_symbol_fundamental_roe ON symbol_fundamental(roe DESC);
CREATE INDEX IF NOT EXISTS idx_symbol_fundamental_dividend_yield ON symbol_fundamental(dividend_yield DESC);

-- 스크리닝용 복합 인덱스
CREATE INDEX IF NOT EXISTS idx_symbol_fundamental_valuation
ON symbol_fundamental(per, pbr, dividend_yield)
WHERE per IS NOT NULL AND pbr IS NOT NULL;

COMMENT ON TABLE symbol_fundamental IS '심볼 펀더멘털 데이터 - 시가총액, PER, PBR, 재무 지표 등';
COMMENT ON COLUMN symbol_fundamental.symbol_info_id IS 'symbol_info 테이블 FK';
COMMENT ON COLUMN symbol_fundamental.market_cap IS '시가총액 (통화 단위)';
COMMENT ON COLUMN symbol_fundamental.per IS '주가수익비율 (Price to Earnings Ratio)';
COMMENT ON COLUMN symbol_fundamental.pbr IS '주가순자산비율 (Price to Book Ratio)';
COMMENT ON COLUMN symbol_fundamental.roe IS 'ROE (자기자본이익률) % - 극단적 값 허용';
COMMENT ON COLUMN symbol_fundamental.operating_margin IS '영업이익률 % - 적자 기업의 큰 음수 허용';
COMMENT ON COLUMN symbol_fundamental.revenue_growth_yoy IS '매출 성장률 YoY % - 스타트업 고성장 허용';
COMMENT ON COLUMN symbol_fundamental.earnings_growth_yoy IS '이익 성장률 YoY % - 적자→흑자 전환 시 극단값 허용';

-- =====================================================
-- VIEWS
-- =====================================================

-- 심볼 정보 + Fundamental 통합 뷰
CREATE OR REPLACE VIEW v_symbol_with_fundamental AS
SELECT
    si.id,
    si.ticker,
    si.name,
    si.name_en,
    si.market,
    si.exchange,
    si.sector,
    si.yahoo_symbol,
    si.is_active,
    -- Fundamental 데이터
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
    sf.data_source AS fundamental_source,
    sf.fetched_at AS fundamental_fetched_at,
    sf.updated_at AS fundamental_updated_at
FROM symbol_info si
LEFT JOIN symbol_fundamental sf ON si.id = sf.symbol_info_id
WHERE si.is_active = true;

COMMENT ON VIEW v_symbol_with_fundamental IS '심볼 기본정보와 펀더멘털 통합 조회용 뷰';

-- 실패 심볼 현황 뷰 (023)
CREATE OR REPLACE VIEW v_symbol_fetch_failures AS
SELECT
    si.id,
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
        WHEN si.fetch_fail_count >= 3 THEN 'CRITICAL'
        WHEN si.fetch_fail_count >= 2 THEN 'WARNING'
        WHEN si.fetch_fail_count >= 1 THEN 'MINOR'
        ELSE 'OK'
    END AS failure_level
FROM symbol_info si
WHERE si.fetch_fail_count > 0
ORDER BY si.fetch_fail_count DESC, si.last_fetch_attempt DESC;

COMMENT ON VIEW v_symbol_fetch_failures IS '데이터 수집 실패 심볼 현황 뷰. 실패 횟수별 레벨 표시';

-- =====================================================
-- FUNCTIONS (023)
-- 데이터 수집 실패 추적 함수
-- =====================================================

-- 심볼 데이터 수집 실패 기록 함수
CREATE OR REPLACE FUNCTION record_symbol_fetch_failure(
    p_symbol_info_id UUID,
    p_error_message TEXT,
    p_max_failures INTEGER DEFAULT 3
)
RETURNS BOOLEAN AS $$
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

    -- N회 이상 실패 시 자동 비활성화
    IF v_new_count >= p_max_failures THEN
        UPDATE symbol_info
        SET is_active = FALSE,
            updated_at = NOW()
        WHERE id = p_symbol_info_id;
        v_deactivated := TRUE;
    END IF;

    RETURN v_deactivated;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION record_symbol_fetch_failure(UUID, TEXT, INTEGER) IS
'심볼 데이터 수집 실패 기록. max_failures 초과 시 자동 비활성화하고 TRUE 반환';

-- 심볼 데이터 수집 성공 시 실패 카운트 초기화 함수
CREATE OR REPLACE FUNCTION reset_symbol_fetch_failure(
    p_symbol_info_id UUID
)
RETURNS void AS $$
BEGIN
    UPDATE symbol_info
    SET fetch_fail_count = 0,
        last_fetch_error = NULL,
        last_fetch_attempt = NOW(),
        updated_at = NOW()
    WHERE id = p_symbol_info_id;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION reset_symbol_fetch_failure(UUID) IS
'심볼 데이터 수집 성공 시 실패 카운트 초기화';
