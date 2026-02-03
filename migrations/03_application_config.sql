-- =====================================================
-- 03_application_config.sql
-- 애플리케이션 설정 및 관심 종목 관리
-- =====================================================
--
-- 원본 마이그레이션: 004_watchlist.sql, 006_app_settings.sql
--
-- 포함 내용:
-- - watchlist: 사용자 관심 종목 관리
-- - app_settings: 애플리케이션 전역 설정 (key-value)
-- - 기본 데이터: 기본 관심 종목 5개, 기본 설정 3개
--
-- =====================================================

-- =====================================================
-- WATCHLIST TABLE
-- 사용자 관심 종목 관리
-- =====================================================

CREATE TABLE watchlist (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    symbol VARCHAR(50) NOT NULL,                    -- 종목 코드
    market VARCHAR(10) NOT NULL,                    -- 시장: 'KR', 'US', 'crypto'
    display_name VARCHAR(100),                      -- 표시 이름
    sort_order INT DEFAULT 0,                       -- 정렬 순서
    is_active BOOLEAN DEFAULT true,                 -- 활성화 여부
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(symbol, market)                          -- 동일 시장에서 중복 방지
);

CREATE INDEX idx_watchlist_active ON watchlist(is_active) WHERE is_active = true;
CREATE INDEX idx_watchlist_sort ON watchlist(sort_order);

COMMENT ON TABLE watchlist IS '사용자 관심 종목 관리';
COMMENT ON COLUMN watchlist.market IS '시장 구분: KR(한국), US(미국), crypto(암호화폐)';
COMMENT ON COLUMN watchlist.display_name IS 'WebSocket 표시용 이름 (하이픈 사용)';

-- 기본 관심 종목 추가
-- 주의: display_name은 WebSocket 시뮬레이터 심볼과 일치해야 함 (하이픈 사용)
INSERT INTO watchlist (symbol, market, display_name, sort_order) VALUES
    ('069500', 'KR', 'KODEX-200', 1),
    ('122630', 'KR', 'KODEX-레버리지', 2),
    ('SPY', 'US', 'SPY', 3),
    ('QQQ', 'US', 'QQQ', 4),
    ('TQQQ', 'US', 'TQQQ', 5)
ON CONFLICT (symbol, market) DO NOTHING;

-- =====================================================
-- APP_SETTINGS TABLE
-- 애플리케이션 전역 설정 (key-value)
-- =====================================================

CREATE TABLE IF NOT EXISTS app_settings (
    setting_key VARCHAR(100) PRIMARY KEY,           -- 설정 키
    setting_value TEXT NOT NULL DEFAULT '',         -- 설정 값
    description TEXT,                               -- 설정 설명
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_app_settings_key ON app_settings(setting_key);

COMMENT ON TABLE app_settings IS '애플리케이션 전역 설정 (key-value 저장소)';
COMMENT ON COLUMN app_settings.setting_key IS '설정 키 (예: active_credential_id, default_currency)';
COMMENT ON COLUMN app_settings.setting_value IS '설정 값 (문자열, JSONB로 저장 가능)';

-- 기본 설정값 삽입
INSERT INTO app_settings (setting_key, setting_value, description)
VALUES
    ('active_credential_id', '', '대시보드에 표시할 활성 거래소 계정 ID (UUID)'),
    ('default_currency', 'KRW', '기본 통화 (KRW, USD 등)'),
    ('theme', 'dark', 'UI 테마 (dark, light)')
ON CONFLICT (setting_key) DO NOTHING;
