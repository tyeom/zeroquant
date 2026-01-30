// Market data types
export interface Ticker {
  symbol: string;
  price: number;
  change: number;
  changePercent: number;
  high: number;
  low: number;
  volume: number;
  timestamp: number;
}

export interface Position {
  id: string;
  symbol: string;
  displayName?: string;  // "005930(삼성전자)" 형식
  side: 'Long' | 'Short';
  quantity: number;
  entryPrice: number;
  currentPrice: number;
  unrealizedPnl: number;
  unrealizedPnlPercent: number;
  market: 'KR' | 'US' | 'CRYPTO';
}

export interface Order {
  id: string;
  symbol: string;
  displayName?: string;  // "005930(삼성전자)" 형식
  side: 'Buy' | 'Sell';
  type: 'Market' | 'Limit' | 'StopLoss' | 'TakeProfit';
  quantity: number;
  price?: number;
  filledQuantity: number;
  status: 'Pending' | 'PartiallyFilled' | 'Filled' | 'Cancelled' | 'Rejected';
  createdAt: string;
}

export interface Strategy {
  id: string;
  strategyType: string;  // 전략 타입 (예: "rsi", "grid_trading", "sma")
  name: string;
  status: 'Running' | 'Stopped' | 'Error';
  market: 'KR' | 'US' | 'CRYPTO';
  symbols: string[];
  timeframe: string;  // 타임프레임 (예: "1m", "15m", "1d")
  pnl: number;
  winRate: number;
  tradesCount: number;
}

export interface PortfolioSummary {
  totalValue: number;
  totalPnl: number;
  totalPnlPercent: number;
  dailyPnl: number;
  dailyPnlPercent: number;
  cashBalance: number;
  marginUsed: number;
}

export interface MarketStatus {
  market: 'KR' | 'US';
  isOpen: boolean;
  nextOpen?: string;
  nextClose?: string;
  session?: 'Regular' | 'PreMarket' | 'AfterHours';
}

// WebSocket message types
export type WsMessage =
  | WsWelcome
  | WsSubscribed
  | WsUnsubscribed
  | WsPong
  | WsAuthResult
  | WsError
  | WsTicker
  | WsOrderUpdate
  | WsPositionUpdate
  | WsStrategyUpdate;

export interface WsWelcome {
  type: 'welcome';
  version: string;
  timestamp: number;
}

export interface WsSubscribed {
  type: 'subscribed';
  channels: string[];
}

export interface WsUnsubscribed {
  type: 'unsubscribed';
  channels: string[];
}

export interface WsPong {
  type: 'pong';
  timestamp: number;
}

export interface WsAuthResult {
  type: 'auth_result';
  success: boolean;
  message: string;
  user_id?: string;
}

export interface WsError {
  type: 'error';
  code: string;
  message: string;
}

export interface WsTicker {
  type: 'ticker';
  symbol: string;
  price: string;
  change_24h: string;
  volume_24h: string;
  high_24h: string;
  low_24h: string;
  timestamp: number;
}

export interface WsOrderUpdate {
  type: 'order_update';
  order_id: string;
  symbol: string;
  status: string;
  side: string;
  order_type: string;
  quantity: string;
  filled_quantity: string;
  price?: string;
  average_price?: string;
  timestamp: number;
}

export interface WsPositionUpdate {
  type: 'position_update';
  symbol: string;
  side: string;
  quantity: string;
  entry_price: string;
  current_price: string;
  unrealized_pnl: string;
  return_pct: string;
  timestamp: number;
}

export interface WsStrategyUpdate {
  type: 'strategy_update';
  strategy_id: string;
  name: string;
  running: boolean;
  event: string;
  data?: unknown;
  timestamp: number;
}

export interface Notification {
  id: string;
  type: 'info' | 'success' | 'warning' | 'error';
  title: string;
  message: string;
  timestamp: string;
}

// ==================== 자격증명 관리 타입 ====================

/** 자격증명 필드 정의 */
export interface CredentialField {
  name: string;
  label: string;
  field_type: 'text' | 'password' | 'select';
  placeholder?: string;
  options?: string[];
}

/** 지원되는 거래소 정보 */
export interface SupportedExchange {
  exchange_id: string;
  display_name: string;
  market_type: string;  // 'crypto', 'stock_kr', 'stock_us', 'forex'
  supports_testnet: boolean;  // 모의투자/테스트넷 지원 여부
  required_fields: CredentialField[];
  optional_fields: CredentialField[];
}

/** 저장된 거래소 자격증명 */
export interface ExchangeCredential {
  id: string;
  exchange_id: string;
  display_name: string;
  is_active: boolean;
  is_testnet: boolean;
  created_at: string;
  last_tested_at?: string;
  masked_api_key?: string;
}

/** 텔레그램 설정 정보 */
export interface TelegramSettings {
  configured: boolean;
  display_name?: string;
  masked_token?: string;
  masked_chat_id?: string;
  created_at?: string;
  updated_at?: string;
  last_tested_at?: string;
}
