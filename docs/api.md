# Trader API Documentation

## Overview

Multi-Market Trading Bot REST API 및 WebSocket 서버 문서입니다.

**Base URL:** `http://localhost:3000`
**API Version:** v1

---

## Authentication

### JWT Token

모든 보호된 엔드포인트는 JWT Bearer 토큰을 요구합니다.

```
Authorization: Bearer <token>
```

### Roles

| Role | Level | Description |
|------|-------|-------------|
| `admin` | 100 | 모든 권한 (시스템 관리 포함) |
| `trader` | 50 | 거래 및 전략 관리 |
| `viewer` | 10 | 읽기 전용 |

### Token Endpoints

#### POST /api/v1/auth/login
로그인 및 토큰 발급

**Request:**
```json
{
  "username": "string",
  "password": "string"
}
```

**Response:**
```json
{
  "access_token": "string",
  "refresh_token": "string",
  "expires_in": 1800,
  "token_type": "Bearer"
}
```

---

## Health Check

### GET /health
Liveness check

**Response:** `200 OK`
```json
{
  "status": "healthy",
  "timestamp": "2026-01-28T12:00:00Z"
}
```

### GET /health/ready
Readiness check (모든 컴포넌트 상태)

**Response:**
```json
{
  "status": "healthy",
  "components": {
    "database": { "status": "healthy", "latency_ms": 5 },
    "redis": { "status": "healthy", "latency_ms": 2 },
    "exchange": { "status": "healthy" }
  },
  "timestamp": "2026-01-28T12:00:00Z"
}
```

---

## Strategies API

### GET /api/v1/strategies
전략 목록 조회

**Response:**
```json
{
  "strategies": [
    {
      "id": "grid_btc",
      "name": "BTC Grid Trading",
      "version": "1.0.0",
      "running": true,
      "signals_generated": 150
    }
  ],
  "total": 1,
  "running": 1
}
```

### GET /api/v1/strategies/:id
특정 전략 상세 조회

**Response:**
```json
{
  "id": "grid_btc",
  "name": "BTC Grid Trading",
  "version": "1.0.0",
  "running": true,
  "stats": {
    "signals_generated": 150,
    "orders_submitted": 120,
    "orders_filled": 115
  }
}
```

### POST /api/v1/strategies/:id/start
전략 시작

**Response:**
```json
{
  "success": true,
  "strategy_id": "grid_btc",
  "action": "start",
  "message": "Strategy 'grid_btc' started successfully"
}
```

### POST /api/v1/strategies/:id/stop
전략 중지

### PUT /api/v1/strategies/:id/config
전략 설정 변경

**Request:**
```json
{
  "config": {
    "grid_spacing_pct": 1.5,
    "grid_levels": 10
  }
}
```

### GET /api/v1/strategies/stats
엔진 통계 조회

**Response:**
```json
{
  "total_strategies": 5,
  "running_strategies": 2,
  "total_signals_generated": 500,
  "total_orders_filled": 450,
  "total_market_data_processed": 100000
}
```

---

## Orders API

### GET /api/v1/orders
활성 주문 목록 조회

**Response:**
```json
{
  "orders": [
    {
      "id": "uuid",
      "exchange_order_id": "12345",
      "symbol": "BTC/USDT",
      "side": "Buy",
      "order_type": "Limit",
      "quantity": "0.1",
      "filled_quantity": "0.05",
      "price": "50000",
      "average_fill_price": "49950",
      "status": "PartiallyFilled",
      "strategy_id": "grid_btc",
      "created_at": "2026-01-28T12:00:00Z",
      "updated_at": "2026-01-28T12:05:00Z"
    }
  ],
  "total": 1
}
```

### GET /api/v1/orders/:id
특정 주문 상세 조회

### DELETE /api/v1/orders/:id
주문 취소

**Request Body (optional):**
```json
{
  "reason": "Manual cancellation"
}
```

**Response:**
```json
{
  "success": true,
  "order_id": "uuid",
  "message": "Order cancelled successfully"
}
```

### GET /api/v1/orders/stats
주문 통계

**Response:**
```json
{
  "total": 10,
  "by_status": {
    "pending": 2,
    "open": 5,
    "partially_filled": 3
  },
  "by_side": {
    "buy": 6,
    "sell": 4
  }
}
```

---

## Positions API

### GET /api/v1/positions
열린 포지션 목록 조회

**Response:**
```json
{
  "positions": [
    {
      "id": "uuid",
      "exchange": "binance",
      "symbol": "BTC/USDT",
      "side": "Buy",
      "quantity": "0.5",
      "entry_price": "50000",
      "current_price": "51000",
      "unrealized_pnl": "500",
      "realized_pnl": "0",
      "notional_value": "25500",
      "return_pct": "2.0",
      "strategy_id": "grid_btc",
      "opened_at": "2026-01-28T10:00:00Z",
      "updated_at": "2026-01-28T12:00:00Z"
    }
  ],
  "total": 1,
  "summary": {
    "total_positions": 1,
    "total_unrealized_pnl": "500",
    "total_realized_pnl": "0",
    "total_notional_value": "25500",
    "long_count": 1,
    "short_count": 0
  }
}
```

### GET /api/v1/positions/:symbol
특정 심볼 포지션 조회

### GET /api/v1/positions/summary
포지션 요약 통계

---

## WebSocket API

**Endpoint:** `ws://localhost:3000/ws`

### Connection Flow

1. WebSocket 연결 수립
2. 서버에서 `welcome` 메시지 전송
3. 클라이언트에서 `auth` 메시지로 인증 (선택)
4. 클라이언트에서 `subscribe` 메시지로 채널 구독
5. 서버에서 실시간 데이터 스트리밍

### Client → Server Messages

#### Subscribe
```json
{
  "type": "subscribe",
  "channels": ["market:BTC-USDT", "orders", "positions"]
}
```

#### Unsubscribe
```json
{
  "type": "unsubscribe",
  "channels": ["market:BTC-USDT"]
}
```

#### Ping
```json
{
  "type": "ping"
}
```

#### Auth
```json
{
  "type": "auth",
  "token": "jwt_token_here"
}
```

### Server → Client Messages

#### Welcome
```json
{
  "type": "welcome",
  "version": "0.1.0",
  "timestamp": 1706436000000
}
```

#### Subscribed
```json
{
  "type": "subscribed",
  "channels": ["market:BTC-USDT", "orders"]
}
```

#### Pong
```json
{
  "type": "pong",
  "timestamp": 1706436000000
}
```

#### Ticker
```json
{
  "type": "ticker",
  "symbol": "BTC-USDT",
  "price": "50000.00",
  "change_24h": "2.5",
  "volume_24h": "1000000",
  "high_24h": "51000",
  "low_24h": "49000",
  "timestamp": 1706436000000
}
```

#### Trade
```json
{
  "type": "trade",
  "symbol": "BTC-USDT",
  "trade_id": "123456",
  "price": "50000.00",
  "quantity": "0.1",
  "side": "Buy",
  "timestamp": 1706436000000
}
```

#### Order Update
```json
{
  "type": "order_update",
  "order_id": "uuid",
  "symbol": "BTC/USDT",
  "status": "Filled",
  "side": "Buy",
  "order_type": "Limit",
  "quantity": "0.1",
  "filled_quantity": "0.1",
  "price": "50000",
  "average_price": "49990",
  "timestamp": 1706436000000
}
```

#### Position Update
```json
{
  "type": "position_update",
  "symbol": "BTC/USDT",
  "side": "Buy",
  "quantity": "0.5",
  "entry_price": "50000",
  "current_price": "51000",
  "unrealized_pnl": "500",
  "return_pct": "2.0",
  "timestamp": 1706436000000
}
```

#### Strategy Update
```json
{
  "type": "strategy_update",
  "strategy_id": "grid_btc",
  "name": "BTC Grid Trading",
  "running": true,
  "event": "signal_generated",
  "data": { "signal_type": "Entry", "side": "Buy" },
  "timestamp": 1706436000000
}
```

#### Error
```json
{
  "type": "error",
  "code": "INVALID_CHANNEL",
  "message": "Unknown channel: xyz"
}
```

### Subscription Channels

| Channel | Description |
|---------|-------------|
| `market:{symbol}` | 특정 심볼의 시장 데이터 (ticker, trades) |
| `orders` | 주문 상태 업데이트 |
| `positions` | 포지션 업데이트 |
| `strategies` | 전략 상태 변경 |
| `all_markets` | 모든 시장 요약 데이터 |

---

## Error Responses

모든 에러는 동일한 형식으로 반환됩니다:

```json
{
  "code": "ERROR_CODE",
  "message": "Human readable error message"
}
```

### Common Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `MISSING_TOKEN` | 401 | 인증 토큰 누락 |
| `INVALID_TOKEN` | 401 | 유효하지 않은 토큰 |
| `TOKEN_EXPIRED` | 401 | 토큰 만료 |
| `INSUFFICIENT_PERMISSION` | 403 | 권한 부족 |
| `STRATEGY_NOT_FOUND` | 404 | 전략을 찾을 수 없음 |
| `ORDER_NOT_FOUND` | 404 | 주문을 찾을 수 없음 |
| `POSITION_NOT_FOUND` | 404 | 포지션을 찾을 수 없음 |
| `INVALID_ORDER_ID` | 400 | 잘못된 주문 ID 형식 |
| `ALREADY_RUNNING` | 400 | 전략이 이미 실행 중 |
| `NOT_RUNNING` | 400 | 전략이 실행 중이 아님 |
| `CANCEL_FAILED` | 500 | 주문 취소 실패 |

---

## Rate Limiting

- REST API: 1200 requests/minute
- WebSocket: 100 messages/second

Rate limit 초과 시 `429 Too Many Requests` 반환

---

## Testing

### Unit Tests
```bash
cargo test --workspace
```

### Integration Tests
```bash
cargo test --workspace --test '*'
```

### Current Test Status
- **Total Tests:** 237
- **Passed:** 237
- **Failed:** 0

---

## Changelog

### v0.1.0 (2026-01-28)
- Initial Phase 8 implementation
- REST API for strategies, orders, positions
- JWT authentication with role-based access control
- WebSocket real-time data streaming
- 237 unit tests passing
