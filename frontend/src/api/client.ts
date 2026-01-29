import axios from 'axios';
import type {
  Position,
  Order,
  Strategy,
  PortfolioSummary,
  MarketStatus,
  SupportedExchange,
  ExchangeCredential,
  TelegramSettings,
} from '../types';

const api = axios.create({
  baseURL: '/api/v1',
  headers: {
    'Content-Type': 'application/json',
  },
});

// Add auth token to requests
api.interceptors.request.use((config) => {
  const token = localStorage.getItem('auth_token');
  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  return config;
});

// ==================== 헬스 체크 ====================

export const healthCheck = async () => {
  const response = await api.get('/health');
  return response.data;
};

// ==================== 활성 계정 관리 ====================

/** 활성 계정 정보 */
export interface ActiveAccount {
  credential_id: string | null;
  exchange_id: string | null;
  display_name: string | null;
  is_testnet: boolean;
}

/** 활성 계정 조회 */
export const getActiveAccount = async (): Promise<ActiveAccount> => {
  const response = await api.get('/credentials/active');
  return response.data;
};

/** 활성 계정 설정 */
export const setActiveAccount = async (credentialId: string | null): Promise<{ success: boolean; message: string }> => {
  const response = await api.put('/credentials/active', { credential_id: credentialId });
  return response.data;
};

// ==================== 포트폴리오 ====================

/** 포트폴리오 요약 조회 (활성 계정 기준) */
export const getPortfolioSummary = async (credentialId?: string): Promise<PortfolioSummary> => {
  const params = credentialId ? { credential_id: credentialId } : {};
  const response = await api.get('/portfolio/summary', { params });
  return response.data;
};

export interface BalanceInfo {
  kr?: {
    cashBalance: string;
    totalEvalAmount: string;
    totalProfitLoss: string;
    holdingsCount: number;
  };
  us?: {
    totalEvalAmount?: string;
    totalProfitLoss?: string;
    holdingsCount: number;
  };
  totalValue: string;
}

/** 잔고 조회 (활성 계정 기준) */
export const getBalance = async (credentialId?: string): Promise<BalanceInfo> => {
  const params = credentialId ? { credential_id: credentialId } : {};
  const response = await api.get('/portfolio/balance', { params });
  return response.data;
};

export interface HoldingInfo {
  symbol: string;
  name: string;
  quantity: string;
  avgPrice: string;
  currentPrice: string;
  evalAmount: string;
  profitLoss: string;
  profitLossRate: string;
  market: string;
}

export interface HoldingsResponse {
  krHoldings: HoldingInfo[];
  usHoldings: HoldingInfo[];
  totalCount: number;
}

/** 보유 종목 조회 (활성 계정 기준) */
export const getHoldings = async (credentialId?: string): Promise<HoldingsResponse> => {
  const params = credentialId ? { credential_id: credentialId } : {};
  const response = await api.get('/portfolio/holdings', { params });
  return response.data;
};

// ==================== 시장 상태 ====================

export const getMarketStatus = async (market: 'KR' | 'US'): Promise<MarketStatus> => {
  const response = await api.get(`/market/${market}/status`);
  return response.data;
};

// ==================== 캔들스틱 데이터 ====================

export interface CandleData {
  time: string;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

export interface KlinesResponse {
  symbol: string;
  timeframe: string;
  data: CandleData[];
}

export const getKlines = async (params: {
  symbol: string;
  timeframe?: string;
  limit?: number;
}): Promise<KlinesResponse> => {
  const response = await api.get('/market/klines', { params });
  return response.data;
};

// ==================== 현재가 (Ticker) ====================

export interface TickerResponse {
  symbol: string;
  price: string;
  change24h: string;
  change24hPercent: string;
  high24h: string;
  low24h: string;
  volume24h: string;
  timestamp: number;
}

export const getTicker = async (symbol: string): Promise<TickerResponse> => {
  const response = await api.get('/market/ticker', { params: { symbol } });
  return response.data;
};

// ==================== 포지션 & 주문 ====================

export const getPositions = async (): Promise<Position[]> => {
  const response = await api.get('/positions');
  return response.data;
};

export const getOrders = async (): Promise<Order[]> => {
  const response = await api.get('/orders');
  return response.data;
};

export const placeOrder = async (order: {
  symbol: string;
  side: 'Buy' | 'Sell';
  type: 'Market' | 'Limit';
  quantity: number;
  price?: number;
}) => {
  const response = await api.post('/orders', order);
  return response.data;
};

export const cancelOrder = async (orderId: string) => {
  const response = await api.delete(`/orders/${orderId}`);
  return response.data;
};

// ==================== 전략 ====================

export const getStrategies = async (): Promise<Strategy[]> => {
  const response = await api.get('/strategies');
  // API returns { strategies: [...], total: N, running: N }
  return response.data.strategies || [];
};

export const startStrategy = async (strategyId: string) => {
  const response = await api.post(`/strategies/${strategyId}/start`);
  return response.data;
};

export const stopStrategy = async (strategyId: string) => {
  const response = await api.post(`/strategies/${strategyId}/stop`);
  return response.data;
};

export interface CreateStrategyRequest {
  strategy_type: string;
  name?: string;
  parameters: Record<string, unknown>;
}

export interface CreateStrategyResponse {
  success: boolean;
  strategy_id: string;
  name: string;
  message: string;
}

export const createStrategy = async (request: CreateStrategyRequest): Promise<CreateStrategyResponse> => {
  const response = await api.post('/strategies', request);
  return response.data;
};

export const deleteStrategy = async (strategyId: string) => {
  const response = await api.delete(`/strategies/${strategyId}`);
  return response.data;
};

// 전략 상세 응답 타입
export interface StrategyDetailResponse {
  id: string;
  strategy_type: string;
  name: string;
  version: string;
  description: string;
  running: boolean;
  stats: {
    signals_generated: number;
    orders_filled: number;
    market_data_processed: number;
    last_signal_time: string | null;
    last_error: string | null;
    started_at: string | null;
    total_runtime_secs: number;
  };
  state: Record<string, unknown>;
  config: Record<string, unknown>;
}

// 전략 상세 조회
export const getStrategy = async (strategyId: string): Promise<StrategyDetailResponse> => {
  const response = await api.get(`/strategies/${strategyId}`);
  return response.data;
};

// 전략 설정 업데이트 요청 타입
export interface UpdateStrategyConfigRequest {
  config: Record<string, unknown>;
}

// 전략 설정 업데이트
export const updateStrategyConfig = async (
  strategyId: string,
  config: Record<string, unknown>
): Promise<{ success: boolean; strategy_id: string; action: string; message: string }> => {
  const response = await api.put(`/strategies/${strategyId}/config`, { config });
  return response.data;
};

// ==================== 백테스트 ====================

export interface BacktestRequest {
  strategy_id: string;
  symbol: string;
  start_date: string;
  end_date: string;
  initial_capital: number;
  commission_rate?: number;
  slippage_rate?: number;
  parameters?: Record<string, unknown>;
}

// 다중 자산 백테스트 요청 (Simple Power, HAA, XAA, Stock Rotation 등)
export interface BacktestMultiRequest {
  strategy_id: string;
  symbols: string[];
  start_date: string;
  end_date: string;
  initial_capital: number;
  commission_rate?: number;
  slippage_rate?: number;
  parameters?: Record<string, unknown>;
}

// 다중 자산 백테스트 결과 (심볼별 데이터 포인트 포함)
export interface BacktestMultiResult extends Omit<BacktestResult, 'symbol'> {
  symbols: string[];
  data_points_by_symbol?: Record<string, number>;
}

// 다중 자산 전략 ID 목록
export const MULTI_ASSET_STRATEGIES = ['simple_power', 'haa', 'xaa', 'stock_rotation'];

// ==================== SDUI (Server Driven UI) 타입 ====================

/** UI 필드 타입 */
export type UiFieldType =
  | 'number'
  | 'text'
  | 'select'
  | 'boolean'
  | 'symbol_picker'
  | 'range'
  | 'split_levels'
  | 'symbol_category_group'
  | 'date'
  | 'timeframe';

/** 유효성 검사 규칙 */
export interface UiValidation {
  required?: boolean;
  min?: number;
  max?: number;
  step?: number;
  min_length?: number;
  max_length?: number;
  pattern?: string;
  min_items?: number;
  max_items?: number;
}

/** 선택 옵션 */
export interface UiSelectOption {
  label: string;
  value: unknown;
  description?: string;
}

/** 심볼 카테고리 정의 (자산배분 전략용) */
export interface SymbolCategory {
  /** 카테고리 키 (예: "canary_assets") */
  key: string;
  /** 카테고리 표시 이름 (예: "카나리아 자산") */
  label: string;
  /** 카테고리 설명 */
  description?: string;
  /** 기본 심볼 목록 */
  default_symbols: string[];
  /** 추천 심볼 목록 */
  suggested_symbols: string[];
  /** 최소 선택 수 */
  min_items?: number;
  /** 최대 선택 수 */
  max_items?: number;
  /** 표시 순서 */
  order: number;
}

/** 조건 연산자 */
export type UiConditionOperator = 'equals' | 'not_equals' | 'greater_than' | 'less_than' | 'contains';

/** 조건부 표시 규칙 */
export interface UiCondition {
  field: string;
  operator: UiConditionOperator;
  value: unknown;
}

/** UI 필드 정의 */
export interface UiField {
  key: string;
  label: string;
  field_type: UiFieldType;
  default_value?: unknown;
  placeholder?: string;
  help_text?: string;
  validation: UiValidation;
  options?: UiSelectOption[];
  /** 심볼 카테고리 목록 (symbol_category_group 타입용) */
  symbol_categories?: SymbolCategory[];
  group?: string;
  order: number;
  show_when?: UiCondition;
  unit?: string;
}

/** 필드 그룹 */
export interface UiFieldGroup {
  id: string;
  label: string;
  description?: string;
  order: number;
  collapsed?: boolean;
}

/** 레이아웃 힌트 */
export interface UiLayout {
  columns: number;
}

/** SDUI 스키마 */
export interface UiSchema {
  fields: UiField[];
  groups: UiFieldGroup[];
  layout?: UiLayout;
}

// ==================== 백테스트 전략 ====================

/** 전략 실행 주기 */
export type ExecutionSchedule = 'realtime' | 'on_candle_close' | 'daily' | 'weekly' | 'monthly';

/** 실행 주기 표시명 */
export const ExecutionScheduleLabel: Record<ExecutionSchedule, string> = {
  realtime: '실시간',
  on_candle_close: '캔들 완성 시',
  daily: '일 1회',
  weekly: '주 1회',
  monthly: '월 1회',
};

export interface BacktestStrategy {
  id: string;
  name: string;
  description: string;
  supported_symbols: string[];
  default_params: Record<string, unknown>;
  /** SDUI 스키마 (동적 폼 렌더링용) */
  ui_schema?: UiSchema;
  /** 전략 카테고리 */
  category?: string;
  /** 전략 태그 */
  tags?: string[];
  /** 실행 주기 */
  execution_schedule?: ExecutionSchedule;
  /** 실행 주기 상세 설명 (예: "장 시작 5분 후") */
  schedule_detail?: string;
  /** 작동 방식 상세 설명 */
  how_it_works?: string;
}

export interface BacktestStrategiesResponse {
  strategies: BacktestStrategy[];
  total: number;
}

export interface BacktestMetrics {
  total_return_pct: string;
  annualized_return_pct: string;
  net_profit: string;
  total_trades: number;
  win_rate_pct: string;
  profit_factor: string;
  sharpe_ratio: string;
  sortino_ratio: string;
  max_drawdown_pct: string;
  calmar_ratio: string;
  avg_win: string;
  avg_loss: string;
  largest_win: string;
  largest_loss: string;
}

export interface EquityCurvePoint {
  timestamp: number;
  equity: string;
  drawdown_pct: string;
}

export interface TradeHistoryItem {
  symbol: string;
  entry_time: string;
  exit_time: string;
  entry_price: string;
  exit_price: string;
  quantity: string;
  side: string;
  pnl: string;
  return_pct: string;
}

export interface BacktestConfigSummary {
  initial_capital: string;
  commission_rate: string;
  slippage_rate: string;
  total_commission: string;
  total_slippage: string;
  data_points: number;
}

export interface BacktestResult {
  id: string;
  success: boolean;
  strategy_id: string;
  symbol: string;
  start_date: string;
  end_date: string;
  metrics: BacktestMetrics;
  equity_curve: EquityCurvePoint[];
  trades: TradeHistoryItem[];
  config_summary: BacktestConfigSummary;
}

export const runBacktest = async (request: BacktestRequest): Promise<BacktestResult> => {
  const response = await api.post('/backtest/run', request);
  return response.data;
};

// 다중 자산 백테스트 실행 (Simple Power, HAA, XAA, Stock Rotation 등)
export const runMultiBacktest = async (request: BacktestMultiRequest): Promise<BacktestMultiResult> => {
  const response = await api.post('/backtest/run-multi', request);
  return response.data;
};

export const getBacktestStrategies = async (): Promise<BacktestStrategiesResponse> => {
  const response = await api.get('/backtest/strategies');
  return response.data;
};

export const getBacktestResults = async (): Promise<BacktestResult[]> => {
  const response = await api.get('/backtest/results');
  return response.data.results || [];
};

export const getBacktestResult = async (id: string): Promise<BacktestResult> => {
  const response = await api.get(`/backtest/results/${id}`);
  return response.data;
};

// ==================== 시뮬레이션 ====================

/** 시뮬레이션 상태 enum */
export type SimulationStateEnum = 'stopped' | 'running' | 'paused';

/** 시뮬레이션 시작 요청 */
export interface SimulationStartRequest {
  strategy_id: string;
  initial_balance?: number;
  speed?: number;
}

/** 시뮬레이션 시작 응답 */
export interface SimulationStartResponse {
  success: boolean;
  message: string;
  started_at: string;
}

/** 시뮬레이션 중지 응답 */
export interface SimulationStopResponse {
  success: boolean;
  message: string;
  final_equity: string;
  total_return_pct: string;
  total_trades: number;
}

/** 시뮬레이션 상태 응답 */
export interface SimulationStatusResponse {
  state: SimulationStateEnum;
  strategy_id: string | null;
  initial_balance: string;
  current_balance: string;
  total_equity: string;
  unrealized_pnl: string;
  realized_pnl: string;
  return_pct: string;
  position_count: number;
  trade_count: number;
  started_at: string | null;
  speed: number;
}

/** 시뮬레이션 포지션 */
export interface SimulationPosition {
  symbol: string;
  side: string;  // "Long" | "Short"
  quantity: string;
  entry_price: string;
  current_price: string;
  unrealized_pnl: string;
  return_pct: string;
  entry_time: string;
}

/** 시뮬레이션 포지션 응답 */
export interface SimulationPositionsResponse {
  positions: SimulationPosition[];
  total_unrealized_pnl: string;
}

/** 시뮬레이션 거래 */
export interface SimulationTrade {
  id: string;
  symbol: string;
  side: string;  // "Buy" | "Sell"
  quantity: string;
  price: string;
  commission: string;
  realized_pnl: string | null;
  timestamp: string;
}

/** 시뮬레이션 거래 내역 응답 */
export interface SimulationTradesResponse {
  trades: SimulationTrade[];
  total: number;
  total_realized_pnl: string;
  total_commission: string;
}

/** 시뮬레이션 주문 요청 */
export interface SimulationOrderRequest {
  symbol: string;
  side: string;  // "Buy" | "Sell"
  quantity: number;
  price?: number;
}

/** 시뮬레이션 주문 응답 */
export interface SimulationOrderResponse {
  success: boolean;
  trade?: SimulationTrade;
  error?: string;
}

/** 시뮬레이션 일시정지/재개 응답 */
export interface SimulationPauseResponse {
  success: boolean;
  state: SimulationStateEnum;
  message: string;
}

/** 시뮬레이션 리셋 응답 */
export interface SimulationResetResponse {
  success: boolean;
  message: string;
}

export const startSimulation = async (request: SimulationStartRequest): Promise<SimulationStartResponse> => {
  const response = await api.post('/simulation/start', request);
  return response.data;
};

export const stopSimulation = async (): Promise<SimulationStopResponse> => {
  const response = await api.post('/simulation/stop');
  return response.data;
};

export const pauseSimulation = async (): Promise<SimulationPauseResponse> => {
  const response = await api.post('/simulation/pause');
  return response.data;
};

export const resetSimulation = async (): Promise<SimulationResetResponse> => {
  const response = await api.post('/simulation/reset');
  return response.data;
};

export const getSimulationStatus = async (): Promise<SimulationStatusResponse> => {
  const response = await api.get('/simulation/status');
  return response.data;
};

export const getSimulationPositions = async (): Promise<SimulationPositionsResponse> => {
  const response = await api.get('/simulation/positions');
  return response.data;
};

export const getSimulationTrades = async (): Promise<SimulationTradesResponse> => {
  const response = await api.get('/simulation/trades');
  return response.data;
};

export const placeSimulationOrder = async (order: SimulationOrderRequest): Promise<SimulationOrderResponse> => {
  const response = await api.post('/simulation/order', order);
  return response.data;
};

// ==================== 분석 (Analytics) ====================

export interface PerformanceResponse {
  currentEquity: string;
  initialCapital: string;
  totalPnl: string;
  totalReturnPct: string;
  cagrPct: string;
  maxDrawdownPct: string;
  currentDrawdownPct: string;
  peakEquity: string;
  periodDays: number;
  periodReturns: { period: string; returnPct: string }[];
  lastUpdated: string;
}

export interface ChartPointResponse {
  x: number;
  y: string;
  label?: string;
}

export interface EquityCurveResponse {
  data: ChartPointResponse[];
  count: number;
  period: string;
  startTime: string;
  endTime: string;
}

export interface ChartResponse {
  name: string;
  data: ChartPointResponse[];
  count: number;
  period: string;
}

export interface MonthlyReturnCell {
  year: number;
  month: number;
  returnPct: string;
  intensity: number;
}

export interface MonthlyReturnsResponse {
  data: MonthlyReturnCell[];
  count: number;
  yearRange: [number, number];
}

export const getPerformance = async (period?: string): Promise<PerformanceResponse> => {
  const params = period ? { period } : {};
  const response = await api.get('/analytics/performance', { params });
  return response.data;
};

export const getEquityCurve = async (period?: string): Promise<EquityCurveResponse> => {
  const params = period ? { period } : {};
  const response = await api.get('/analytics/equity-curve', { params });
  return response.data;
};

export const getCagrChart = async (period?: string, windowDays?: number): Promise<ChartResponse> => {
  const params: Record<string, string | number> = {};
  if (period) params.period = period;
  if (windowDays) params.window_days = windowDays;
  const response = await api.get('/analytics/charts/cagr', { params });
  return response.data;
};

export const getMddChart = async (period?: string, windowDays?: number): Promise<ChartResponse> => {
  const params: Record<string, string | number> = {};
  if (period) params.period = period;
  if (windowDays) params.window_days = windowDays;
  const response = await api.get('/analytics/charts/mdd', { params });
  return response.data;
};

export const getDrawdownChart = async (period?: string): Promise<ChartResponse> => {
  const params = period ? { period } : {};
  const response = await api.get('/analytics/charts/drawdown', { params });
  return response.data;
};

export const getMonthlyReturns = async (): Promise<MonthlyReturnsResponse> => {
  const response = await api.get('/analytics/monthly-returns');
  return response.data;
};

// 자산 곡선 동기화 요청
export interface SyncEquityCurveRequest {
  credential_id: string;
  start_date: string;  // YYYYMMDD
  end_date: string;    // YYYYMMDD
}

// 자산 곡선 동기화 응답
export interface SyncEquityCurveResponse {
  success: boolean;
  message: string;
  synced_count: number;
  execution_count: number;
  start_date: string;
  end_date: string;
  synced_at: string;
}

// 자산 곡선 동기화 (거래소 체결 내역 기반)
export const syncEquityCurve = async (request: SyncEquityCurveRequest): Promise<SyncEquityCurveResponse> => {
  const response = await api.post('/analytics/sync-equity', request);
  return response.data;
};

// ==================== 알림 (Notifications) ====================

/** 알림 설정 응답 */
export interface NotificationSettingsResponse {
  telegram_enabled: boolean;
  telegram_configured: boolean;
}

/** 텔레그램 테스트 요청 */
export interface TelegramTestRequest {
  bot_token: string;
  chat_id: string;
}

/** 텔레그램 테스트 응답 */
export interface TelegramTestResponse {
  success: boolean;
  message: string;
}

/** 템플릿 정보 */
export interface TemplateInfo {
  id: string;
  name: string;
  description: string;
  priority: string;
}

/** 템플릿 목록 응답 */
export interface TemplateListResponse {
  templates: TemplateInfo[];
}

/** 템플릿 테스트 요청 */
export interface TemplateTestRequest {
  template_type: string;
}

export const getNotificationSettings = async (): Promise<NotificationSettingsResponse> => {
  const response = await api.get('/notifications/settings');
  return response.data;
};

export const getNotificationTemplates = async (): Promise<TemplateListResponse> => {
  const response = await api.get('/notifications/templates');
  return response.data;
};

export const testTelegram = async (request: TelegramTestRequest): Promise<TelegramTestResponse> => {
  const response = await api.post('/notifications/telegram/test', request);
  return response.data;
};

export const testTelegramEnv = async (): Promise<TelegramTestResponse> => {
  const response = await api.post('/notifications/telegram/test-env');
  return response.data;
};

export const testTelegramTemplate = async (request: TemplateTestRequest): Promise<TelegramTestResponse> => {
  const response = await api.post('/notifications/telegram/test-template', request);
  return response.data;
};

export const testAllTelegramTemplates = async (): Promise<TelegramTestResponse> => {
  const response = await api.post('/notifications/telegram/test-all-templates');
  return response.data;
};

// ==================== 자격증명 관리 (Credentials) ====================

/** 지원되는 거래소 목록 응답 */
export interface SupportedExchangesResponse {
  exchanges: SupportedExchange[];
}

/** 등록된 자격증명 목록 응답 */
export interface CredentialsListResponse {
  credentials: ExchangeCredential[];
  total: number;
}

/** 자격증명 생성/수정 요청 */
export interface CredentialRequest {
  exchange_id: string;
  display_name: string;
  fields: Record<string, string>;
  /** 모의투자/테스트넷 여부 */
  is_testnet?: boolean;
}

/** 자격증명 응답 */
export interface CredentialResponse {
  success: boolean;
  message: string;
  credential?: ExchangeCredential;
}

/** 자격증명 테스트 요청 */
export interface CredentialTestRequest {
  exchange_id: string;
  fields: Record<string, string>;
}

/** 자격증명 테스트 응답 */
export interface CredentialTestResponse {
  success: boolean;
  message: string;
  details?: {
    balance_check?: boolean;
    permissions?: string[];
  };
}

/** 텔레그램 설정 요청 */
export interface TelegramSettingsRequest {
  bot_token: string;
  chat_id: string;
  display_name?: string;
}

/** 텔레그램 설정 응답 */
export interface TelegramSettingsResponse {
  success: boolean;
  message: string;
  settings?: TelegramSettings;
}

/** 지원되는 거래소 목록 조회 (필드 정보 포함) */
export const getSupportedExchanges = async (): Promise<SupportedExchangesResponse> => {
  const response = await api.get('/credentials/exchanges');
  return response.data;
};

/** 등록된 자격증명 목록 조회 */
export const listCredentials = async (): Promise<CredentialsListResponse> => {
  const response = await api.get('/credentials/exchanges/list');
  return response.data;
};

/** 새 자격증명 등록 */
export const createCredential = async (request: CredentialRequest): Promise<CredentialResponse> => {
  const response = await api.post('/credentials/exchanges', request);
  return response.data;
};

/** 기존 자격증명 수정 */
export const updateCredential = async (id: string, request: CredentialRequest): Promise<CredentialResponse> => {
  const response = await api.put(`/credentials/exchanges/${id}`, request);
  return response.data;
};

/** 자격증명 삭제 */
export const deleteCredential = async (id: string): Promise<{ success: boolean; message: string }> => {
  const response = await api.delete(`/credentials/exchanges/${id}`);
  return response.data;
};

/** 새 자격증명 테스트 (저장 전) */
export const testNewCredential = async (request: CredentialTestRequest): Promise<CredentialTestResponse> => {
  const response = await api.post('/credentials/exchanges/test', request);
  return response.data;
};

/** 기존 자격증명 테스트 */
export const testExistingCredential = async (id: string): Promise<CredentialTestResponse> => {
  const response = await api.post(`/credentials/exchanges/${id}/test`);
  return response.data;
};

/** 텔레그램 설정 조회 */
export const getTelegramSettings = async (): Promise<TelegramSettings> => {
  const response = await api.get('/credentials/telegram');
  return response.data;
};

/** 텔레그램 설정 저장 */
export const saveTelegramSettings = async (request: TelegramSettingsRequest): Promise<TelegramSettingsResponse> => {
  const response = await api.post('/credentials/telegram', request);
  return response.data;
};

/** 텔레그램 설정 삭제 */
export const deleteTelegramSettings = async (): Promise<{ success: boolean; message: string }> => {
  const response = await api.delete('/credentials/telegram');
  return response.data;
};

// ==================== 인증 ====================

export const login = async (username: string, password: string) => {
  const response = await api.post('/auth/login', { username, password });
  const { token } = response.data;
  localStorage.setItem('auth_token', token);
  return response.data;
};

export const logout = () => {
  localStorage.removeItem('auth_token');
};

export default api;
