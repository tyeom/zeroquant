-- =====================================================
-- 심볼 메타데이터 테이블
--
-- 목적: 티커와 회사명 매핑으로 자동완성 검색 지원
-- 데이터 소스: KRX (한국), Yahoo Finance (해외)
-- =====================================================

CREATE TABLE IF NOT EXISTS symbol_info (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- 심볼 식별
    ticker VARCHAR(20) NOT NULL,              -- 티커 코드 (AAPL, 005930)
    name VARCHAR(200) NOT NULL,               -- 회사명 (Apple Inc., 삼성전자)
    name_en VARCHAR(200),                     -- 영문 회사명 (Samsung Electronics)

    -- 시장 정보
    market VARCHAR(20) NOT NULL,              -- KR, US, JP 등
    exchange VARCHAR(50),                     -- NYSE, NASDAQ, KRX, KOSDAQ
    sector VARCHAR(100),                      -- 섹터/업종

    -- Yahoo Finance 호환 심볼
    yahoo_symbol VARCHAR(30),                 -- 005930.KS, AAPL

    -- 상태
    is_active BOOLEAN DEFAULT true,

    -- 시스템 필드
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- 유일 제약
    CONSTRAINT unique_symbol_market UNIQUE (ticker, market)
);

-- 인덱스
CREATE INDEX IF NOT EXISTS idx_symbol_info_ticker ON symbol_info(ticker);
CREATE INDEX IF NOT EXISTS idx_symbol_info_name ON symbol_info(name);
CREATE INDEX IF NOT EXISTS idx_symbol_info_market ON symbol_info(market);
CREATE INDEX IF NOT EXISTS idx_symbol_info_yahoo ON symbol_info(yahoo_symbol);

-- 전문 검색용 인덱스 (티커 + 회사명)
CREATE INDEX IF NOT EXISTS idx_symbol_info_search
ON symbol_info USING gin(to_tsvector('simple', ticker || ' ' || name || ' ' || COALESCE(name_en, '')));

-- 코멘트
COMMENT ON TABLE symbol_info IS '심볼 메타데이터 - 티커와 회사명 매핑';
COMMENT ON COLUMN symbol_info.ticker IS '티커 코드 (예: AAPL, 005930)';
COMMENT ON COLUMN symbol_info.name IS '회사명 (예: Apple Inc., 삼성전자)';
COMMENT ON COLUMN symbol_info.yahoo_symbol IS 'Yahoo Finance 호환 심볼 (예: 005930.KS, AAPL)';
