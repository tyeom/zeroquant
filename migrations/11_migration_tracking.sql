-- =====================================================
-- 11_migration_tracking.sql
-- 마이그레이션 적용 이력 추적 시스템
-- =====================================================
--
-- 원본 마이그레이션: 033_migration_tracking.sql
--
-- 포함 내용:
-- - schema_migrations: 마이그레이션 적용 이력 추적
-- - 기존 34개 마이그레이션 기록 (이미 적용된 것으로 표시)
--
-- 기능:
-- - 어떤 마이그레이션이 적용되었는지 추적
-- - 실패한 마이그레이션 기록
-- - 실행 시간 측정
-- - SHA-256 체크섬 (향후 무결성 검증용)
--
-- =====================================================

-- =====================================================
-- SCHEMA_MIGRATIONS TABLE
-- 마이그레이션 적용 이력 추적
-- =====================================================

CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,                    -- 마이그레이션 버전 번호
    filename VARCHAR(255) NOT NULL,                 -- 마이그레이션 파일명
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),  -- 적용 시간
    checksum VARCHAR(64),                           -- SHA-256 해시 (향후 구현)
    execution_time_ms INTEGER,                      -- 실행 시간 (ms)
    success BOOLEAN NOT NULL DEFAULT true,          -- 적용 성공 여부
    error_message TEXT                              -- 실패 시 에러 메시지
);

-- 인덱스
CREATE INDEX idx_schema_migrations_applied ON schema_migrations(applied_at DESC);
CREATE INDEX idx_schema_migrations_failed ON schema_migrations(success) WHERE success = false;

COMMENT ON TABLE schema_migrations IS '마이그레이션 적용 이력 추적';
COMMENT ON COLUMN schema_migrations.version IS '마이그레이션 버전 번호 (파일명의 숫자)';
COMMENT ON COLUMN schema_migrations.filename IS '마이그레이션 파일명 (예: 001_initial_schema.sql)';
COMMENT ON COLUMN schema_migrations.checksum IS 'SHA-256 체크섬 (무결성 검증용, 향후 구현)';
COMMENT ON COLUMN schema_migrations.success IS '적용 성공 여부 (실패 시 false)';

-- =====================================================
-- 기존 마이그레이션 기록
-- 현재까지 적용된 마이그레이션을 기록합니다
-- =====================================================

INSERT INTO schema_migrations (version, filename, success, applied_at) VALUES
(1, '001_initial_schema.sql', true, '2025-01-27 00:00:00'),
(2, '002_encrypted_credentials.sql', true, '2025-01-29 00:00:00'),
(3, '003_fix_credentials_unique_constraint.sql', true, '2025-01-29 00:00:00'),
(4, '004_watchlist.sql', true, '2025-01-29 00:00:00'),
(5, '005_yahoo_candle_cache.sql', true, '2025-01-30 00:00:00'),
(6, '006_app_settings.sql', true, '2025-01-29 00:00:00'),
(7, '007_portfolio_equity_history.sql', true, '2025-01-30 00:00:00'),
(8, '008_strategies_type_and_symbols.sql', true, '2025-01-30 00:00:00'),
(9, '009_rename_candle_cache.sql', true, '2025-01-30 00:00:00'),
(10, '010_backtest_results.sql', true, '2025-01-30 00:00:00'),
(11, '011_execution_cache.sql', true, '2025-01-30 00:00:00'),
(12, '012_symbol_info.sql', true, '2025-01-30 00:00:00'),
(13, '013_strategy_timeframe.sql', true, '2025-01-30 00:00:00'),
(14, '014_strategy_risk_capital.sql', true, '2025-01-31 00:00:00'),
(15, '015_trading_journal.sql', true, '2025-01-31 00:00:00'),
(16, '016_positions_credential_id.sql', true, '2025-01-31 00:00:00'),
(17, '017_journal_views.sql', true, '2025-01-31 00:00:00'),
(18, '018_journal_period_views.sql', true, '2025-01-31 00:00:00'),
(19, '019_fix_cumulative_pnl_types.sql', true, '2025-01-31 00:00:00'),
(20, '020_symbol_fundamental.sql', true, '2025-01-31 00:00:00'),
(21, '021_fix_fundamental_decimal_precision.sql', true, '2025-02-01 00:00:00'),
(22, '022_latest_prices_materialized_view.sql', true, '2025-02-01 00:00:00'),
(23, '023_symbol_fetch_failure_tracking.sql', true, '2025-02-01 00:00:00'),
(24, '024_add_symbol_type.sql', true, '2025-02-03 00:00:00'),
(25, '025_add_route_state.sql', true, '2025-02-03 00:00:00'),
(26, '026_add_ttm_squeeze.sql', true, '2025-02-03 00:00:00'),
(27, '027_add_market_regime.sql', true, '2025-02-03 00:00:00'),
(28, '028_reality_check_system.sql', true, '2025-02-03 00:00:00'),
(29, '029_signal_marker.sql', true, '2025-02-03 00:00:00'),
(30, '030_add_missing_views.sql', true, NOW()),
(31, '031_add_strategy_presets.sql', true, NOW()),
(32, '032_fix_hypertable_declarations.sql', true, NOW()),
(33, '033_migration_tracking.sql', true, NOW()),
(34, '034_signal_alert_rules.sql', true, NOW())
ON CONFLICT (version) DO NOTHING;

-- =====================================================
-- 사용 예시
-- =====================================================
--
-- 1. 적용된 마이그레이션 조회:
--    SELECT * FROM schema_migrations WHERE success = true ORDER BY version;
--
-- 2. 실패한 마이그레이션 조회:
--    SELECT * FROM schema_migrations WHERE success = false;
--
-- 3. 최근 적용된 마이그레이션:
--    SELECT * FROM schema_migrations ORDER BY applied_at DESC LIMIT 10;
--
-- 4. 새 마이그레이션 기록:
--    INSERT INTO schema_migrations (version, filename, success, execution_time_ms)
--    VALUES (34, '034_new_feature.sql', true, 1250);
--
-- =====================================================
