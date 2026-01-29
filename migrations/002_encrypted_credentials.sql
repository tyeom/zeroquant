-- =============================================================================
-- 002: Encrypted Credentials Management
-- =============================================================================
-- 거래소 API 키, 텔레그램 토큰 등 민감한 자격증명을 암호화하여 저장
-- 환경변수 대신 대시보드에서 관리
-- =============================================================================

-- -----------------------------------------------------------------------------
-- 거래소 자격증명 테이블
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS exchange_credentials (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- 거래소 정보
    exchange_id VARCHAR(50) NOT NULL,           -- 'binance', 'kis', 'coinbase' 등
    exchange_name VARCHAR(100) NOT NULL,        -- 표시용 이름
    market_type VARCHAR(20) NOT NULL,           -- 'crypto', 'stock_kr', 'stock_us', 'forex'

    -- 암호화된 자격증명 (JSON 형태로 암호화)
    -- 예: {"api_key": "xxx", "api_secret": "yyy", "passphrase": "zzz"}
    encrypted_credentials BYTEA NOT NULL,

    -- 암호화 메타데이터
    encryption_version INT NOT NULL DEFAULT 1,  -- 암호화 버전 (키 로테이션용)
    encryption_nonce BYTEA NOT NULL,            -- AES-GCM nonce (12바이트)

    -- 상태 및 메타데이터
    is_active BOOLEAN NOT NULL DEFAULT true,
    is_testnet BOOLEAN NOT NULL DEFAULT false,  -- 테스트넷 여부
    permissions JSONB,                          -- 권한 정보: ["read", "trade", "withdraw"]

    -- 추가 설정
    settings JSONB DEFAULT '{}',                -- 거래소별 추가 설정

    -- 감사 정보
    last_used_at TIMESTAMPTZ,
    last_verified_at TIMESTAMPTZ,               -- 마지막 연결 테스트 시간
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- 유니크 제약: 거래소당 하나의 활성 자격증명
    CONSTRAINT unique_active_exchange UNIQUE (exchange_id, market_type, is_testnet)
);

-- 인덱스
CREATE INDEX idx_exchange_credentials_active ON exchange_credentials(is_active) WHERE is_active = true;
CREATE INDEX idx_exchange_credentials_exchange ON exchange_credentials(exchange_id);

-- 업데이트 트리거
CREATE OR REPLACE FUNCTION update_exchange_credentials_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_exchange_credentials_updated_at
    BEFORE UPDATE ON exchange_credentials
    FOR EACH ROW
    EXECUTE FUNCTION update_exchange_credentials_updated_at();

-- -----------------------------------------------------------------------------
-- 텔레그램 설정 테이블
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS telegram_settings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- 암호화된 Bot Token
    encrypted_bot_token BYTEA NOT NULL,
    encryption_nonce_token BYTEA NOT NULL,

    -- 암호화된 Chat ID (개인 또는 그룹)
    encrypted_chat_id BYTEA NOT NULL,
    encryption_nonce_chat BYTEA NOT NULL,

    -- 암호화 버전
    encryption_version INT NOT NULL DEFAULT 1,

    -- 알림 설정
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    notification_settings JSONB DEFAULT '{
        "trade_executed": true,
        "order_filled": true,
        "position_opened": true,
        "position_closed": true,
        "stop_loss_triggered": true,
        "take_profit_triggered": true,
        "daily_summary": true,
        "error_alerts": true,
        "risk_warnings": true
    }',

    -- 메타데이터
    bot_username VARCHAR(100),                  -- @username (연결 테스트 후 저장)
    chat_type VARCHAR(20),                      -- 'private', 'group', 'supergroup'

    -- 감사 정보
    last_message_at TIMESTAMPTZ,
    last_verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 단일 텔레그램 설정만 허용 (필요시 여러 개 허용 가능하도록 변경)
CREATE UNIQUE INDEX idx_telegram_single_setting ON telegram_settings((1));

-- 업데이트 트리거
CREATE TRIGGER trigger_telegram_settings_updated_at
    BEFORE UPDATE ON telegram_settings
    FOR EACH ROW
    EXECUTE FUNCTION update_exchange_credentials_updated_at();

-- -----------------------------------------------------------------------------
-- 전략 파라미터 프리셋 테이블
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS strategy_presets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- 전략 정보
    strategy_type VARCHAR(100) NOT NULL,        -- 'grid_trading', 'ma_crossover', 'rsi_mean_reversion' 등
    preset_name VARCHAR(200) NOT NULL,          -- 사용자 정의 이름
    description TEXT,

    -- 파라미터 (JSON)
    -- 예: {"symbol": "BTC/USDT", "grid_levels": 10, "grid_spacing_pct": 1.0, ...}
    parameters JSONB NOT NULL,

    -- 파라미터 스키마 버전 (호환성 관리)
    schema_version INT NOT NULL DEFAULT 1,

    -- 상태
    is_default BOOLEAN NOT NULL DEFAULT false,  -- 기본 프리셋 여부
    is_public BOOLEAN NOT NULL DEFAULT false,   -- 공유 프리셋 여부

    -- 메타데이터
    tags VARCHAR(50)[] DEFAULT '{}',            -- 태그: ["btc", "aggressive", "tested"]
    performance_metrics JSONB,                  -- 백테스트 결과 저장

    -- 감사 정보
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 인덱스
CREATE INDEX idx_strategy_presets_type ON strategy_presets(strategy_type);
CREATE INDEX idx_strategy_presets_default ON strategy_presets(is_default) WHERE is_default = true;
CREATE INDEX idx_strategy_presets_tags ON strategy_presets USING GIN(tags);

-- 업데이트 트리거
CREATE TRIGGER trigger_strategy_presets_updated_at
    BEFORE UPDATE ON strategy_presets
    FOR EACH ROW
    EXECUTE FUNCTION update_exchange_credentials_updated_at();

-- -----------------------------------------------------------------------------
-- 자격증명 접근 로그 (감사용)
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS credential_access_logs (
    id BIGSERIAL PRIMARY KEY,

    -- 대상 정보
    credential_type VARCHAR(50) NOT NULL,       -- 'exchange', 'telegram'
    credential_id UUID NOT NULL,

    -- 접근 정보
    action VARCHAR(50) NOT NULL,                -- 'create', 'read', 'update', 'delete', 'verify', 'use'
    accessor_ip VARCHAR(45),                    -- IPv4/IPv6
    user_agent TEXT,

    -- 결과
    success BOOLEAN NOT NULL DEFAULT true,
    error_message TEXT,

    -- 타임스탬프
    accessed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 시계열 최적화 (TimescaleDB)
SELECT create_hypertable('credential_access_logs', 'accessed_at', if_not_exists => TRUE);

-- 인덱스
CREATE INDEX idx_credential_access_logs_type_id ON credential_access_logs(credential_type, credential_id);
CREATE INDEX idx_credential_access_logs_action ON credential_access_logs(action);

-- 90일 이후 데이터 자동 삭제 정책
SELECT add_retention_policy('credential_access_logs', INTERVAL '90 days', if_not_exists => TRUE);

-- -----------------------------------------------------------------------------
-- 코멘트
-- -----------------------------------------------------------------------------
COMMENT ON TABLE exchange_credentials IS '거래소 API 자격증명 (AES-256-GCM 암호화)';
COMMENT ON TABLE telegram_settings IS '텔레그램 봇 설정 (암호화)';
COMMENT ON TABLE strategy_presets IS '전략 파라미터 프리셋';
COMMENT ON TABLE credential_access_logs IS '자격증명 접근 감사 로그';

COMMENT ON COLUMN exchange_credentials.encrypted_credentials IS 'AES-256-GCM으로 암호화된 JSON (api_key, api_secret 등)';
COMMENT ON COLUMN exchange_credentials.encryption_nonce IS 'AES-GCM nonce (12바이트, 각 암호화마다 고유)';
