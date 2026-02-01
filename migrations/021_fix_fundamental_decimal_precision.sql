-- =====================================================
-- Fundamental 테이블 Decimal 정밀도 수정
--
-- 문제: DECIMAL(8, 4)는 정수부 4자리까지만 허용
--       스타트업/바이오텍 등에서 극단적인 성장률 발생 시 오버플로우
--
-- 예시:
--   - earnings_growth_yoy = 21033.3% (DAVE)
--   - revenue_growth_yoy = 80521.4% (JOBY)
--   - operating_margin = -14854.78% (CRSP)
--
-- 해결: DECIMAL(8, 4) → DECIMAL(12, 4) (정수부 8자리까지 허용)
-- =====================================================

-- 1. 뷰 삭제 (컬럼 타입 변경을 위해 필요)
DROP VIEW IF EXISTS v_symbol_with_fundamental;

-- 2. 수익성 지표 정밀도 확장
ALTER TABLE symbol_fundamental
    ALTER COLUMN roe TYPE DECIMAL(12, 4),
    ALTER COLUMN roa TYPE DECIMAL(12, 4),
    ALTER COLUMN operating_margin TYPE DECIMAL(12, 4),
    ALTER COLUMN net_profit_margin TYPE DECIMAL(12, 4),
    ALTER COLUMN gross_margin TYPE DECIMAL(12, 4);

-- 3. 성장성 지표 정밀도 확장 (극단적 성장률 허용)
ALTER TABLE symbol_fundamental
    ALTER COLUMN revenue_growth_yoy TYPE DECIMAL(12, 4),
    ALTER COLUMN earnings_growth_yoy TYPE DECIMAL(12, 4),
    ALTER COLUMN revenue_growth_3y TYPE DECIMAL(12, 4),
    ALTER COLUMN earnings_growth_3y TYPE DECIMAL(12, 4);

-- 4. 배당 관련 정밀도 확장
ALTER TABLE symbol_fundamental
    ALTER COLUMN dividend_yield TYPE DECIMAL(12, 4),
    ALTER COLUMN dividend_payout_ratio TYPE DECIMAL(12, 4);

-- 5. 뷰 재생성
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

-- 6. 코멘트 업데이트
COMMENT ON COLUMN symbol_fundamental.roe IS 'ROE (자기자본이익률) % - 극단적 값 허용';
COMMENT ON COLUMN symbol_fundamental.operating_margin IS '영업이익률 % - 적자 기업의 큰 음수 허용';
COMMENT ON COLUMN symbol_fundamental.revenue_growth_yoy IS '매출 성장률 YoY % - 스타트업 고성장 허용';
COMMENT ON COLUMN symbol_fundamental.earnings_growth_yoy IS '이익 성장률 YoY % - 적자→흑자 전환 시 극단값 허용';
COMMENT ON VIEW v_symbol_with_fundamental IS '심볼 기본정보와 펀더멘털 통합 조회용 뷰';
