-- =====================================================
-- 09_strategy_system.sql
-- 전략 시스템 및 신호 관리
-- =====================================================
--
-- 원본 마이그레이션: 008_strategies_type_and_symbols.sql,
--                  013_strategy_timeframe.sql, 014_strategy_risk_capital.sql,
--                  025_add_route_state.sql, 026_add_ttm_squeeze.sql,
--                  027_add_market_regime.sql, 029_signal_marker.sql,
--                  034_signal_alert_rules.sql
--
-- 포함 내용:
-- - strategies 테이블 확장 (strategy_type, symbols, market, timeframe, risk 설정)
-- - symbol_fundamental 테이블 확장 (route_state, ttm_squeeze, market_regime)
-- - signal_marker 테이블 생성 (백테스트 및 실거래 신호)
-- - signal_alert_rule 테이블 생성 (신호 알림 규칙 관리)
-- - route_state ENUM 타입
--
-- =====================================================

-- =====================================================
-- ENUM TYPES
-- =====================================================

-- RouteState ENUM (025): 진입 적기 판단용
CREATE TYPE route_state AS ENUM (
    'ATTACK',                                       -- 진입 적기 (강한 매수 신호)
    'ARMED',                                        -- 대기 준비 (조건 충족 임박)
    'WAIT',                                         -- 관찰 중 (중립)
    'OVERHEAT',                                     -- 과열 (매수 회피)
    'NEUTRAL'                                       -- 중립 (기본값)
);

COMMENT ON TYPE route_state IS '전략 진입 상태: ATTACK(진입 적기), ARMED(대기), WAIT(관찰), OVERHEAT(과열), NEUTRAL(중립)';

-- =====================================================
-- STRATEGIES TABLE 확장
-- =====================================================

-- 008: strategy_type, symbols, market 컬럼 추가
ALTER TABLE strategies ADD COLUMN IF NOT EXISTS strategy_type VARCHAR(50);
ALTER TABLE strategies ADD COLUMN IF NOT EXISTS symbols JSONB DEFAULT '[]';
ALTER TABLE strategies ADD COLUMN IF NOT EXISTS market VARCHAR(20) DEFAULT 'KR';

-- 013: timeframe 컬럼 추가
ALTER TABLE strategies ADD COLUMN IF NOT EXISTS timeframe VARCHAR(10) DEFAULT '1d';

-- 014: allocated_capital, risk_profile 컬럼 추가
ALTER TABLE strategies ADD COLUMN IF NOT EXISTS allocated_capital DECIMAL(30, 15);
ALTER TABLE strategies ADD COLUMN IF NOT EXISTS risk_profile VARCHAR(20) DEFAULT 'default';

-- 인덱스
CREATE INDEX IF NOT EXISTS idx_strategies_type ON strategies(strategy_type);
CREATE INDEX IF NOT EXISTS idx_strategies_active ON strategies(is_active) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_strategies_risk_profile ON strategies(risk_profile);

COMMENT ON COLUMN strategies.strategy_type IS '전략 구현 타입 (grid_trading, rsi_mean_reversion, sma_crossover 등)';
COMMENT ON COLUMN strategies.symbols IS '전략이 운영하는 심볼 배열 (JSONB)';
COMMENT ON COLUMN strategies.market IS '시장 타입: KR, US, CRYPTO';
COMMENT ON COLUMN strategies.timeframe IS '전략 실행 타임프레임 (1m, 5m, 15m, 30m, 1h, 4h, 1d, 1w, 1M)';
COMMENT ON COLUMN strategies.allocated_capital IS '전략에 할당된 자본 (NULL = 전체 계좌 잔고 사용)';
COMMENT ON COLUMN strategies.risk_limits IS 'RiskConfig 설정을 담은 JSON 객체';
COMMENT ON COLUMN strategies.risk_profile IS '리스크 프로파일: conservative, default, aggressive, custom';

-- =====================================================
-- SYMBOL_FUNDAMENTAL TABLE 확장
-- =====================================================

-- 025: route_state 컬럼 추가
ALTER TABLE symbol_fundamental
ADD COLUMN IF NOT EXISTS route_state route_state DEFAULT 'NEUTRAL';

CREATE INDEX IF NOT EXISTS idx_symbol_fundamental_route_state
ON symbol_fundamental(route_state)
WHERE route_state IN ('ATTACK', 'ARMED');

COMMENT ON COLUMN symbol_fundamental.route_state IS 'RouteState 진입 신호: ATTACK(강매수), ARMED(대기), WAIT(관찰), OVERHEAT(과열), NEUTRAL(중립)';

-- 026: TTM Squeeze 지표 추가
ALTER TABLE symbol_fundamental
ADD COLUMN IF NOT EXISTS ttm_squeeze BOOLEAN DEFAULT FALSE,
ADD COLUMN IF NOT EXISTS ttm_squeeze_cnt INTEGER DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_symbol_fundamental_ttm_squeeze
ON symbol_fundamental(ttm_squeeze, ttm_squeeze_cnt DESC)
WHERE ttm_squeeze = TRUE;

COMMENT ON COLUMN symbol_fundamental.ttm_squeeze IS 'TTM Squeeze 상태 (BB가 KC 내부에 있으면 true - 에너지 응축)';
COMMENT ON COLUMN symbol_fundamental.ttm_squeeze_cnt IS 'TTM Squeeze 연속 카운트 (에너지 응축 기간, 높을수록 큰 변동성 예상)';

-- 027: Market Regime 컬럼 추가
ALTER TABLE symbol_fundamental
ADD COLUMN IF NOT EXISTS regime VARCHAR(20);

CREATE INDEX IF NOT EXISTS idx_symbol_fundamental_regime
ON symbol_fundamental(regime)
WHERE regime IS NOT NULL;

COMMENT ON COLUMN symbol_fundamental.regime IS '시장 레짐: STRONG_UPTREND, CORRECTION, SIDEWAYS, BOTTOM_BOUNCE, DOWNTREND';

-- 027: v_symbol_with_fundamental 뷰 업데이트 (regime 추가)
-- 주의: 이 뷰는 04_symbol_metadata.sql에서 이미 생성되었으므로 DROP 후 재생성
DROP VIEW IF EXISTS v_symbol_with_fundamental;
CREATE VIEW v_symbol_with_fundamental AS
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
    -- 전략 관련 컬럼 (025, 026, 027)
    sf.route_state,
    sf.ttm_squeeze,
    sf.ttm_squeeze_cnt,
    sf.regime,
    -- 메타데이터
    sf.data_source AS fundamental_source,
    sf.fetched_at AS fundamental_fetched_at,
    sf.updated_at AS fundamental_updated_at
FROM symbol_info si
LEFT JOIN symbol_fundamental sf ON si.id = sf.symbol_info_id
WHERE si.is_active = true;

COMMENT ON VIEW v_symbol_with_fundamental IS '심볼 기본정보와 펀더멘털 통합 조회용 뷰 (route_state, ttm_squeeze, regime 포함)';

-- =====================================================
-- SIGNAL_MARKER TABLE (029)
-- 백테스트 및 실거래 신호 저장
-- =====================================================

CREATE TABLE IF NOT EXISTS signal_marker (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- 심볼 정보
    symbol_id UUID NOT NULL REFERENCES symbol_info(id) ON DELETE CASCADE,

    -- 신호 정보
    timestamp TIMESTAMPTZ NOT NULL,                 -- 신호 발생 시간
    signal_type VARCHAR(20) NOT NULL,               -- Entry, Exit, Alert, AddToPosition, ReducePosition, Scale
    side VARCHAR(10),                               -- Buy, Sell (Alert의 경우 nullable)
    price NUMERIC(20, 8) NOT NULL,                  -- 신호 발생 시 가격
    strength DOUBLE PRECISION NOT NULL DEFAULT 0.0, -- 신호 강도 (0.0 ~ 1.0)

    -- 지표 정보 (JSON)
    indicators JSONB NOT NULL DEFAULT '{}'::jsonb,  -- RSI, MACD, BB, RouteState 등

    -- 설명 정보
    reason TEXT NOT NULL DEFAULT '',                -- 신호 발생 이유 (사람이 읽을 수 있는 형태)

    -- 전략 정보
    strategy_id VARCHAR(100) NOT NULL,
    strategy_name VARCHAR(200) NOT NULL,

    -- 실행 여부
    executed BOOLEAN NOT NULL DEFAULT false,        -- 백테스트에서 실제 체결 여부

    -- 메타데이터
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,    -- 확장용 메타데이터

    -- 타임스탬프
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 인덱스
CREATE INDEX idx_signal_marker_symbol_timestamp
ON signal_marker(symbol_id, timestamp DESC);

CREATE INDEX idx_signal_marker_strategy
ON signal_marker(strategy_id, timestamp DESC);

CREATE INDEX idx_signal_marker_signal_type
ON signal_marker(signal_type);

CREATE INDEX idx_signal_marker_executed
ON signal_marker(executed);

-- GIN 인덱스 (지표 검색용)
CREATE INDEX idx_signal_marker_indicators
ON signal_marker USING GIN (indicators);

COMMENT ON TABLE signal_marker IS '백테스트 및 실거래 신호 마커 (차트 표시 및 분석용)';
COMMENT ON COLUMN signal_marker.indicators IS '기술적 지표 값 (JSONB): RSI, MACD, BB, RouteState 등';
COMMENT ON COLUMN signal_marker.reason IS '신호 발생 이유 (예: "RSI 과매도 + MACD 골든크로스")';
COMMENT ON COLUMN signal_marker.executed IS '백테스트에서 실제 체결 여부';
COMMENT ON COLUMN signal_marker.metadata IS '확장용 메타데이터 (슬리피지, 수수료, 거부 사유 등)';

-- =====================================================
-- SIGNAL_ALERT_RULE TABLE (034)
-- 신호 알림 규칙 관리
-- =====================================================

CREATE TABLE IF NOT EXISTS signal_alert_rule (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- 규칙 정보
    rule_name VARCHAR(100) NOT NULL,
    description TEXT,
    enabled BOOLEAN NOT NULL DEFAULT true,

    -- 필터 조건 (JSONB)
    -- {
    --   "min_strength": 0.7,
    --   "strategy_ids": ["rsi_strategy", "macd_strategy"],
    --   "symbols": ["BTC", "ETH"],
    --   "entry_only": false
    -- }
    filter_conditions JSONB NOT NULL DEFAULT '{}',

    -- 메타데이터
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- 인덱스용 제약조건
    CONSTRAINT unique_rule_name UNIQUE(rule_name)
);

-- 인덱스
CREATE INDEX IF NOT EXISTS idx_signal_alert_rule_enabled
ON signal_alert_rule(enabled);

CREATE INDEX IF NOT EXISTS idx_signal_alert_rule_created_at
ON signal_alert_rule(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_signal_alert_rule_filter_conditions
ON signal_alert_rule USING GIN(filter_conditions);

-- updated_at 자동 업데이트 트리거
CREATE OR REPLACE FUNCTION update_signal_alert_rule_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_signal_alert_rule_timestamp
    BEFORE UPDATE ON signal_alert_rule
    FOR EACH ROW
    EXECUTE FUNCTION update_signal_alert_rule_timestamp();

-- 기본 규칙 삽입 (예시)
INSERT INTO signal_alert_rule (rule_name, description, filter_conditions)
VALUES
    (
        'high_strength_signals',
        '강도 80% 이상 모든 신호',
        '{"min_strength": 0.8}'::jsonb
    ),
    (
        'entry_signals_only',
        '진입 신호만 (강도 70% 이상)',
        '{"min_strength": 0.7, "entry_only": true}'::jsonb
    )
ON CONFLICT (rule_name) DO NOTHING;

COMMENT ON TABLE signal_alert_rule IS '신호 마커 알림 규칙';
COMMENT ON COLUMN signal_alert_rule.rule_name IS '규칙 이름 (고유)';
COMMENT ON COLUMN signal_alert_rule.filter_conditions IS '필터 조건 (JSONB: min_strength, strategy_ids, symbols, entry_only)';

-- =====================================================
-- 사용 예시
-- =====================================================
--
-- 1. ATTACK 상태 종목 스크리닝:
--    SELECT ticker, name, route_state, ttm_squeeze, regime
--    FROM v_symbol_with_fundamental
--    WHERE route_state = 'ATTACK' AND ttm_squeeze = TRUE;
--
-- 2. 전략별 신호 조회:
--    SELECT * FROM signal_marker
--    WHERE strategy_id = 'rsi_mean_reversion'
--    ORDER BY timestamp DESC LIMIT 100;
--
-- 3. 특정 지표 조건 검색 (GIN 인덱스 활용):
--    SELECT * FROM signal_marker
--    WHERE indicators @> '{"RSI": 30}'::jsonb;
--
-- 4. 알림 규칙 추가:
--    INSERT INTO signal_alert_rule (rule_name, description, filter_conditions)
--    VALUES ('macd_crossover', 'MACD 골든크로스 신호', '{"min_strength": 0.75, "strategy_ids": ["macd_strategy"]}'::jsonb);
--
-- 5. 활성화된 알림 규칙 조회:
--    SELECT * FROM signal_alert_rule WHERE enabled = true;
--
-- =====================================================
