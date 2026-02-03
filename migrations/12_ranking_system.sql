-- =====================================================
-- 12_ranking_system.sql
-- GlobalScore 랭킹 시스템
-- =====================================================
--
-- 포함 내용:
-- - symbol_global_score: GlobalScore 계산 결과 저장
-- - 인덱스: 랭킹 조회 최적화
-- - 함수: 스코어 업데이트 헬퍼
--
-- =====================================================

-- =====================================================
-- SYMBOL_GLOBAL_SCORE TABLE
-- GlobalScore 계산 결과 저장
-- =====================================================

CREATE TABLE IF NOT EXISTS symbol_global_score (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- symbol_info 연계 (FK)
    symbol_info_id UUID NOT NULL REFERENCES symbol_info(id) ON DELETE CASCADE,

    -- GlobalScore 결과
    overall_score NUMERIC(5, 2) NOT NULL,           -- 0 ~ 100.00
    grade VARCHAR(10) NOT NULL,                     -- BUY, WATCH, HOLD, AVOID
    confidence VARCHAR(10),                         -- HIGH, MEDIUM, LOW

    -- 팩터 점수 (JSONB)
    component_scores JSONB NOT NULL,                -- { "risk_reward": 85.5, "t1": 70.2, ... }

    -- 페널티 (JSONB)
    penalties JSONB,                                -- { "near_52w_high": true, "low_liquidity": true, ... }

    -- 시장 정보 (비정규화 - 조회 최적화)
    market VARCHAR(20) NOT NULL,                    -- KR, US, JP
    ticker VARCHAR(20) NOT NULL,                    -- 005930, AAPL

    -- 계산 시점 (캐시 TTL 용)
    calculated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- 시스템 필드
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- 유일 제약 (심볼당 1개 레코드)
    CONSTRAINT unique_symbol_global_score UNIQUE (symbol_info_id)
);

-- 인덱스: 랭킹 조회 최적화
CREATE INDEX idx_global_score_ranking
    ON symbol_global_score(market, grade, overall_score DESC);

CREATE INDEX idx_global_score_ticker
    ON symbol_global_score(ticker);

CREATE INDEX idx_global_score_calculated
    ON symbol_global_score(calculated_at DESC);

-- JSONB GIN 인덱스 (컴포넌트 필터링용)
CREATE INDEX idx_global_score_components
    ON symbol_global_score USING gin(component_scores);

COMMENT ON TABLE symbol_global_score IS 'GlobalScore 계산 결과 저장 (Phase 1-D.5)';
COMMENT ON COLUMN symbol_global_score.overall_score IS '종합 점수 (0~100)';
COMMENT ON COLUMN symbol_global_score.grade IS '투자 등급 (BUY/WATCH/HOLD/AVOID)';
COMMENT ON COLUMN symbol_global_score.component_scores IS '팩터별 점수 { risk_reward, t1, stop_loss, ... }';
COMMENT ON COLUMN symbol_global_score.penalties IS '페널티 플래그 { near_52w_high, low_liquidity, ... }';
COMMENT ON COLUMN symbol_global_score.calculated_at IS '계산 시점 (캐시 TTL 판단용)';

-- =====================================================
-- 스코어 업데이트 함수 (UPSERT 헬퍼)
-- =====================================================

CREATE OR REPLACE FUNCTION upsert_global_score(
    p_symbol_info_id UUID,
    p_overall_score NUMERIC,
    p_grade VARCHAR,
    p_confidence VARCHAR,
    p_component_scores JSONB,
    p_penalties JSONB,
    p_market VARCHAR,
    p_ticker VARCHAR
) RETURNS UUID AS $$
DECLARE
    v_id UUID;
BEGIN
    INSERT INTO symbol_global_score (
        symbol_info_id,
        overall_score,
        grade,
        confidence,
        component_scores,
        penalties,
        market,
        ticker,
        calculated_at,
        updated_at
    ) VALUES (
        p_symbol_info_id,
        p_overall_score,
        p_grade,
        p_confidence,
        p_component_scores,
        p_penalties,
        p_market,
        p_ticker,
        NOW(),
        NOW()
    )
    ON CONFLICT (symbol_info_id) DO UPDATE SET
        overall_score = EXCLUDED.overall_score,
        grade = EXCLUDED.grade,
        confidence = EXCLUDED.confidence,
        component_scores = EXCLUDED.component_scores,
        penalties = EXCLUDED.penalties,
        calculated_at = NOW(),
        updated_at = NOW()
    RETURNING id INTO v_id;

    RETURN v_id;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION upsert_global_score IS 'GlobalScore UPSERT (있으면 UPDATE, 없으면 INSERT)';

-- =====================================================
-- schema_migrations 기록
-- =====================================================

INSERT INTO schema_migrations (version, filename, success, applied_at)
VALUES (12, '12_ranking_system.sql', true, NOW())
ON CONFLICT (version) DO NOTHING;
