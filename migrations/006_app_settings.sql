-- 애플리케이션 설정 테이블
-- 활성 계정, 기타 앱 설정 저장용

CREATE TABLE IF NOT EXISTS app_settings (
    setting_key VARCHAR(100) PRIMARY KEY,
    setting_value TEXT NOT NULL DEFAULT '',
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- 기본 설정값 삽입
INSERT INTO app_settings (setting_key, setting_value, description)
VALUES
    ('active_credential_id', '', '대시보드에 표시할 활성 거래소 계정 ID'),
    ('default_currency', 'KRW', '기본 통화'),
    ('theme', 'dark', 'UI 테마')
ON CONFLICT (setting_key) DO NOTHING;

-- 인덱스
CREATE INDEX IF NOT EXISTS idx_app_settings_key ON app_settings(setting_key);

COMMENT ON TABLE app_settings IS '애플리케이션 전역 설정';
COMMENT ON COLUMN app_settings.setting_key IS '설정 키';
COMMENT ON COLUMN app_settings.setting_value IS '설정 값';
COMMENT ON COLUMN app_settings.description IS '설정 설명';
