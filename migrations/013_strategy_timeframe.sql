-- =====================================================
-- 전략별 타임프레임 설정
--
-- 목적: 각 전략의 권장/사용 타임프레임 저장
-- 값: 1m, 5m, 15m, 30m, 1h, 4h, 1d, 1w, 1M
-- =====================================================

-- timeframe 컬럼 추가
ALTER TABLE strategies ADD COLUMN IF NOT EXISTS timeframe VARCHAR(10) DEFAULT '1d';

-- 코멘트 추가
COMMENT ON COLUMN strategies.timeframe IS '전략 실행 타임프레임 (1m, 5m, 15m, 30m, 1h, 4h, 1d, 1w, 1M)';

-- 전략 타입별 기본 타임프레임 업데이트 (기존 데이터가 있을 경우)
-- 실시간 전략: 1m
UPDATE strategies SET timeframe = '1m' WHERE strategy_type IN ('grid_trading', 'magic_split', 'infinity_bot', 'trailing_stop') AND timeframe = '1d';

-- 분봉 전략: 15m (기본)
UPDATE strategies SET timeframe = '15m' WHERE strategy_type IN ('rsi_mean_reversion', 'bollinger_bands', 'sma_crossover', 'candle_pattern') AND timeframe = '1d';

-- 일봉 전략: 1d
UPDATE strategies SET timeframe = '1d' WHERE strategy_type IN ('volatility_breakout', 'snow', 'stock_rotation', 'market_interest_day') AND timeframe = '1d';

-- 월봉 전략 (자산배분): 1d (리밸런싱은 월 1회지만 일봉 데이터 사용)
UPDATE strategies SET timeframe = '1d' WHERE strategy_type IN ('simple_power', 'haa', 'xaa', 'all_weather', 'market_cap_top') AND timeframe = '1d';
