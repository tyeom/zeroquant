-- =====================================================
-- symbol_info 테이블에 symbol_type 컬럼 추가
--
-- 목적: OHLCV 수집 시 특수 증권(ETN, 워런트, 옵션) 필터링
-- =====================================================

-- symbol_type 컬럼 추가
ALTER TABLE symbol_info
ADD COLUMN IF NOT EXISTS symbol_type VARCHAR(20) DEFAULT 'STOCK';

-- 인덱스 추가 (빠른 필터링)
CREATE INDEX IF NOT EXISTS idx_symbol_info_type ON symbol_info(symbol_type);

-- 기존 데이터 업데이트 (ticker 패턴 기반)
-- ETN: 0000X0 형태 (X는 알파벳)
UPDATE symbol_info
SET symbol_type = 'ETN'
WHERE market = 'KR'
  AND ticker ~ '^[0-9]{4}[A-Z][0-9]$'
  AND symbol_type = 'STOCK';

-- WARRANT: 특정 패턴 (추후 추가 가능)
-- UPDATE symbol_info
-- SET symbol_type = 'WARRANT'
-- WHERE ...;

-- 코멘트
COMMENT ON COLUMN symbol_info.symbol_type IS '종목 유형 (STOCK, ETF, ETN, WARRANT, REIT, PREFERRED)';
