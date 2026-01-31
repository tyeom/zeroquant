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
  displayName?: string;  // "005930(삼성전자)" 형식
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

/** 전략 복제 요청 타입 */
export interface CloneStrategyRequest {
  new_name: string;
}

/** 전략 복제 응답 타입 */
export interface CloneStrategyResponse {
  success: boolean;
  message: string;
  strategy_id: string;
  name: string;
}

/** 전략 복제 */
export const cloneStrategy = async (strategyId: string, newName: string): Promise<CloneStrategyResponse> => {
  const response = await api.post(`/strategies/${strategyId}/clone`, { new_name: newName });
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
export const MULTI_ASSET_STRATEGIES = [
  'simple_power',
  'haa',
  'xaa',
  'stock_rotation',
  // 추가 다중 자산 전략들
  'all_weather',
  'all_weather_us',
  'all_weather_kr',
  'snow',
  'snow_us',
  'snow_kr',
  'baa',
  'sector_momentum',
  'dual_momentum',
  'pension_bot',
  'market_cap_top',
];

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

/** 백테스트 결과 저장 요청 */
export interface SaveBacktestResultRequest {
  strategy_id: string;
  strategy_type: string;
  symbol: string;
  start_date: string;
  end_date: string;
  initial_capital: number;
  slippage_rate?: number;
  metrics: BacktestMetrics;
  config_summary: BacktestConfigSummary;
  equity_curve: EquityCurvePoint[];
  trades: TradeHistoryItem[];
  success: boolean;
}

/** 백테스트 결과 저장 응답 */
export interface SaveBacktestResultResponse {
  id: string;
  message: string;
}

/** 백테스트 결과 목록 쿼리 파라미터 */
export interface ListBacktestResultsQuery {
  strategy_id?: string;
  strategy_type?: string;
  limit?: number;
  offset?: number;
}

/** 백테스트 결과 저장 */
export const saveBacktestResult = async (request: SaveBacktestResultRequest): Promise<SaveBacktestResultResponse> => {
  const response = await api.post('/backtest/results', request);
  return response.data;
};

/** 백테스트 결과 삭제 */
export const deleteBacktestResult = async (id: string): Promise<void> => {
  await api.delete(`/backtest/results/${id}`);
};

/** 저장된 백테스트 결과 목록 조회 (쿼리 파라미터 지원) */
export const listBacktestResults = async (query?: ListBacktestResultsQuery): Promise<{ results: BacktestResult[]; total: number }> => {
  const response = await api.get('/backtest/results', { params: query });
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
  /** 시뮬레이션(백테스트) 시작 날짜 (YYYY-MM-DD) */
  start_date?: string;
  /** 시뮬레이션(백테스트) 종료 날짜 (YYYY-MM-DD) */
  end_date?: string;
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
  /** 현재 시뮬레이션 시간 (배속 적용된 가상 시간) */
  current_simulation_time: string | null;
  /** 시뮬레이션(백테스트) 시작 날짜 (YYYY-MM-DD) */
  simulation_start_date: string | null;
  /** 시뮬레이션(백테스트) 종료 날짜 (YYYY-MM-DD) */
  simulation_end_date: string | null;
}

/** 시뮬레이션 포지션 */
export interface SimulationPosition {
  symbol: string;
  displayName?: string;  // "005930(삼성전자)" 형식
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
  displayName?: string;  // "005930(삼성전자)" 형식
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
  // 포지션 기반 지표 (실제 투자 원금 대비)
  totalCostBasis?: string;      // 총 투자 원금
  positionPnl?: string;         // 포지션 손익 금액
  positionPnlPct?: string;      // 포지션 손익률 (%)
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

export const getPerformance = async (period?: string, credentialId?: string): Promise<PerformanceResponse> => {
  const params: Record<string, string> = {};
  if (period) params.period = period;
  if (credentialId) params.credential_id = credentialId;
  const response = await api.get('/analytics/performance', { params });
  return response.data;
};

export const getEquityCurve = async (period?: string, credentialId?: string): Promise<EquityCurveResponse> => {
  const params: Record<string, string> = {};
  if (period) params.period = period;
  if (credentialId) params.credential_id = credentialId;
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
  use_market_prices?: boolean;  // 시장가 기반 자산 계산 (기본값: true)
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
  // 기본값: 시장가 기반 자산 계산 (현재 보유 포지션의 주식 가치만 추적)
  const requestWithDefaults = {
    ...request,
    use_market_prices: request.use_market_prices ?? true,
  };
  const response = await api.post('/analytics/sync-equity', requestWithDefaults);
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

// ==================== 심볼 검색 ====================

/** 심볼 검색 결과 */
export interface SymbolSearchResult {
  ticker: string;
  name: string;
  market: string;
  yahooSymbol: string | null;
}

/** 심볼 검색 응답 */
export interface SymbolSearchResponse {
  results: SymbolSearchResult[];
  total: number;
}

/**
 * 심볼/회사명 검색
 * @param query 검색어 (티커 또는 회사명)
 * @param limit 최대 결과 수 (기본값: 10)
 */
export const searchSymbols = async (query: string, limit: number = 10): Promise<SymbolSearchResult[]> => {
  if (!query.trim()) return [];

  const params = new URLSearchParams({ q: query, limit: limit.toString() });
  const response = await api.get(`/dataset/search?${params}`);
  return response.data?.results || [];
};

// ==================== 매매일지 (Journal) ====================

/** 매매일지 포지션 */
export interface JournalPosition {
  id: string;
  exchange: string;
  symbol: string;
  symbol_name: string | null;
  side: string;
  quantity: string;
  entry_price: string;
  current_price: string | null;
  cost_basis: string;
  market_value: string | null;
  unrealized_pnl: string | null;
  unrealized_pnl_pct: string | null;
  realized_pnl: string | null;
  weight_pct: string | null;
  first_trade_at: string | null;
  last_trade_at: string | null;
  trade_count: number | null;
  strategy_id: string | null;
  snapshot_time: string;
}

/** 포지션 요약 */
export interface PositionsSummary {
  total_positions: number;
  total_cost_basis: string;
  total_market_value: string;
  total_unrealized_pnl: string;
  total_unrealized_pnl_pct: string;
}

/** 포지션 목록 응답 */
export interface JournalPositionsResponse {
  positions: JournalPosition[];
  total: number;
  summary: PositionsSummary;
}

/** 체결 내역 */
export interface JournalExecution {
  id: string;
  exchange: string;
  symbol: string;
  symbol_name: string | null;
  side: string;
  order_type: string;
  quantity: string;
  price: string;
  notional_value: string;
  fee: string | null;
  fee_currency: string | null;
  position_effect: string | null;
  realized_pnl: string | null;
  strategy_id: string | null;
  strategy_name: string | null;
  executed_at: string;
  memo: string | null;
  tags: string[] | null;
}

/** 체결 내역 목록 응답 */
export interface JournalExecutionsResponse {
  executions: JournalExecution[];
  total: number;
  limit: number;
  offset: number;
}

/** 체결 내역 조회 필터 */
export interface ExecutionFilter {
  symbol?: string;
  side?: string;
  strategy_id?: string;
  start_date?: string;
  end_date?: string;
  limit?: number;
  offset?: number;
}

/** PnL 요약 응답 */
export interface JournalPnLSummary {
  total_realized_pnl: string;
  total_fees: string;
  net_pnl: string;
  total_trades: number;
  buy_trades: number;
  sell_trades: number;
  winning_trades: number;
  losing_trades: number;
  win_rate: string;
  total_volume: string;
  first_trade_at: string | null;
  last_trade_at: string | null;
}

/** 일별 손익 항목 */
export interface DailyPnLItem {
  date: string;
  total_trades: number;
  buy_count: number;
  sell_count: number;
  total_volume: string;
  total_fees: string;
  realized_pnl: string;
  symbol_count: number;
}

/** 일별 손익 응답 */
export interface DailyPnLResponse {
  daily: DailyPnLItem[];
  total_days: number;
}

/** 종목별 손익 항목 */
export interface SymbolPnLItem {
  symbol: string;
  symbol_name: string | null;
  total_trades: number;
  total_buy_qty: string;
  total_sell_qty: string;
  total_buy_value: string;
  total_sell_value: string;
  total_fees: string;
  realized_pnl: string;
  first_trade_at: string | null;
  last_trade_at: string | null;
}

/** 종목별 손익 응답 */
export interface SymbolPnLResponse {
  symbols: SymbolPnLItem[];
  total: number;
}

/** 동기화 응답 */
export interface JournalSyncResponse {
  success: boolean;
  inserted: number;
  skipped: number;
  message: string;
}

/** 매매일지 포지션 조회 */
export const getJournalPositions = async (): Promise<JournalPositionsResponse> => {
  const response = await api.get('/journal/positions');
  return response.data;
};

/** 매매일지 체결 내역 조회 */
export const getJournalExecutions = async (filter?: ExecutionFilter): Promise<JournalExecutionsResponse> => {
  const response = await api.get('/journal/executions', { params: filter });
  return response.data;
};

/** PnL 요약 조회 */
export const getJournalPnLSummary = async (): Promise<JournalPnLSummary> => {
  const response = await api.get('/journal/pnl');
  return response.data;
};

/** 일별 손익 조회 */
export const getJournalDailyPnL = async (startDate?: string, endDate?: string): Promise<DailyPnLResponse> => {
  const params: Record<string, string> = {};
  if (startDate) params.start_date = startDate;
  if (endDate) params.end_date = endDate;
  const response = await api.get('/journal/pnl/daily', { params });
  return response.data;
};

/** 종목별 손익 조회 */
export const getJournalSymbolPnL = async (): Promise<SymbolPnLResponse> => {
  const response = await api.get('/journal/pnl/symbol');
  return response.data;
};

/** 체결 내역 메모/태그 수정 */
export const updateJournalExecution = async (
  id: string,
  data: { memo?: string; tags?: string[] }
): Promise<JournalExecution> => {
  const response = await api.patch(`/journal/executions/${id}`, data);
  return response.data;
};

/** 거래소 체결 내역 동기화 */
export const syncJournalExecutions = async (exchange?: string, startDate?: string): Promise<JournalSyncResponse> => {
  const response = await api.post('/journal/sync', { exchange, start_date: startDate });
  return response.data;
};

// ==================== 기간별 손익 API ====================

/** 주별 손익 항목 */
export interface WeeklyPnLItem {
  week_start: string;
  total_trades: number;
  buy_count: number;
  sell_count: number;
  total_volume: string;
  total_fees: string;
  realized_pnl: string;
  symbol_count: number;
  trading_days: number;
}

/** 주별 손익 응답 */
export interface WeeklyPnLResponse {
  weekly: WeeklyPnLItem[];
  total_weeks: number;
}

/** 월별 손익 항목 */
export interface MonthlyPnLItem {
  year: number;
  month: number;
  total_trades: number;
  buy_count: number;
  sell_count: number;
  total_volume: string;
  total_fees: string;
  realized_pnl: string;
  symbol_count: number;
  trading_days: number;
}

/** 월별 손익 응답 */
export interface MonthlyPnLResponse {
  monthly: MonthlyPnLItem[];
  total_months: number;
}

/** 연도별 손익 항목 */
export interface YearlyPnLItem {
  year: number;
  total_trades: number;
  buy_count: number;
  sell_count: number;
  total_volume: string;
  total_fees: string;
  realized_pnl: string;
  symbol_count: number;
  trading_days: number;
  trading_months: number;
}

/** 연도별 손익 응답 */
export interface YearlyPnLResponse {
  yearly: YearlyPnLItem[];
  total_years: number;
}

/** 누적 손익 포인트 */
export interface CumulativePnLPoint {
  date: string;
  cumulative_pnl: string;
  cumulative_fees: string;
  cumulative_trades: number;
  daily_pnl: string;
}

/** 누적 손익 응답 */
export interface CumulativePnLResponse {
  curve: CumulativePnLPoint[];
  total_points: number;
}

/** 투자 인사이트 응답 */
export interface TradingInsightsResponse {
  total_trades: number;
  buy_trades: number;
  sell_trades: number;
  unique_symbols: number;
  total_realized_pnl: string;
  total_fees: string;
  winning_trades: number;
  losing_trades: number;
  win_rate_pct: string;
  profit_factor: string | null;
  avg_win: string;
  avg_loss: string;
  largest_win: string;
  largest_loss: string;
  trading_period_days: number;
  active_trading_days: number;
  first_trade_at: string | null;
  last_trade_at: string | null;
}

/** 전략별 성과 항목 */
export interface StrategyPerformanceItem {
  strategy_id: string;
  strategy_name: string;
  total_trades: number;
  buy_trades: number;
  sell_trades: number;
  unique_symbols: number;
  total_volume: string;
  total_fees: string;
  realized_pnl: string;
  winning_trades: number;
  losing_trades: number;
  win_rate_pct: string;
  profit_factor: string | null;
  avg_win: string;
  avg_loss: string;
  largest_win: string;
  largest_loss: string;
  active_trading_days: number;
  first_trade_at: string | null;
  last_trade_at: string | null;
}

/** 전략별 성과 응답 */
export interface StrategyPerformanceResponse {
  strategies: StrategyPerformanceItem[];
  total: number;
}

/** 주별 손익 조회 */
export const getJournalWeeklyPnL = async (): Promise<WeeklyPnLResponse> => {
  const response = await api.get('/journal/pnl/weekly');
  return response.data;
};

/** 월별 손익 조회 */
export const getJournalMonthlyPnL = async (): Promise<MonthlyPnLResponse> => {
  const response = await api.get('/journal/pnl/monthly');
  return response.data;
};

/** 연도별 손익 조회 */
export const getJournalYearlyPnL = async (): Promise<YearlyPnLResponse> => {
  const response = await api.get('/journal/pnl/yearly');
  return response.data;
};

/** 누적 손익 곡선 조회 */
export const getJournalCumulativePnL = async (): Promise<CumulativePnLResponse> => {
  const response = await api.get('/journal/pnl/cumulative');
  return response.data;
};

/** 투자 인사이트 조회 */
export const getJournalInsights = async (): Promise<TradingInsightsResponse> => {
  const response = await api.get('/journal/insights');
  return response.data;
};

/** 전략별 성과 조회 */
export const getJournalStrategyPerformance = async (): Promise<StrategyPerformanceResponse> => {
  const response = await api.get('/journal/strategies');
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
