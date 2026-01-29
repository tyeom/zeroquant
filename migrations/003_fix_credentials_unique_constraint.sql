-- =============================================================================
-- 003: Fix Credentials Unique Constraint
-- =============================================================================
-- 동일 거래소에서 여러 계좌 지원 (예: KIS 일반계좌, ISA계좌)
-- exchange_name을 unique 조합에 포함하여 구분
-- =============================================================================

-- 기존 unique constraint 삭제
ALTER TABLE exchange_credentials
DROP CONSTRAINT IF EXISTS unique_active_exchange;

-- 새 unique constraint 추가 (exchange_name 포함)
-- 같은 거래소, 같은 시장, 같은 테스트넷 여부라도 이름이 다르면 허용
ALTER TABLE exchange_credentials
ADD CONSTRAINT unique_exchange_account
UNIQUE (exchange_id, market_type, is_testnet, exchange_name);

-- 코멘트 업데이트
COMMENT ON CONSTRAINT unique_exchange_account ON exchange_credentials
IS '거래소별 계좌 구분 (동일 거래소에서 일반/ISA 등 여러 계좌 허용)';
