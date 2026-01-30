import { createSignal, createEffect, createMemo, For, Show, onCleanup, batch } from 'solid-js'
import { createQuery, createMutation, useQueryClient } from '@tanstack/solid-query'
import {
  Database, Download, Trash2, RefreshCw, TrendingUp, BarChart3,
  Search, Zap, LineChart, Table, Loader2, ArrowUp, ArrowDown,
  X, Grid2x2, Square, Settings2
} from 'lucide-solid'
import { useToast } from '../components/Toast'
import { type CandlestickDataPoint, type IndicatorOverlay, type LineDataPoint } from '../components/charts/PriceChart'
import { type SeparateIndicatorData } from '../components/charts/SubPriceChart'
import { SyncedChartPanel } from '../components/charts/SyncedChartPanel'
import { MultiPanelGrid, type LayoutMode, type PanelConfig } from '../components/MultiPanelGrid'

// ==================== 지표 설정 타입 ====================

/** 오버레이 지표 타입 (메인 차트 위에 그려짐) */
type OverlayIndicatorType = 'sma' | 'ema' | 'bb'

/** 서브 차트 지표 타입 (별도 패널에 그려짐) */
type SubIndicatorType = 'volume' | 'rsi' | 'macd' | 'stochastic' | 'atr' | 'atr_percent' | 'momentum'

/** 전체 지표 타입 */
type IndicatorType = OverlayIndicatorType | SubIndicatorType

/** 지표별 파라미터 정의 */
interface IndicatorParams {
  // 오버레이 지표
  sma: { period: number }
  ema: { period: number }
  bb: { period: number; stdDev: number }
  // 서브 차트 지표
  volume: Record<string, never>
  rsi: { period: number }
  macd: { fastPeriod: number; slowPeriod: number; signalPeriod: number }
  stochastic: { kPeriod: number; dPeriod: number }
  atr: { period: number }
  atr_percent: { period: number }
  momentum: { periods: number[] }
}

/** 활성 지표 설정 */
interface ActiveIndicator<T extends IndicatorType = IndicatorType> {
  id: string
  type: T
  params: IndicatorParams[T]
  enabled: boolean
  /** 오버레이 지표인지 (메인 차트에 그려지는지) */
  isOverlay: boolean
}

/** 지표 메타데이터 (UI용) */
interface IndicatorMeta {
  type: IndicatorType
  name: string
  description: string
  defaultParams: IndicatorParams[IndicatorType]
  paramLabels: Record<string, string>
  scaleRange?: { min: number; max: number; levels?: number[] }
  color: string
  /** 오버레이 지표인지 */
  isOverlay: boolean
}

/** 지표 메타데이터 정의 */
const INDICATOR_META: Record<IndicatorType, IndicatorMeta> = {
  // ========== 오버레이 지표 (메인 차트 위에 그려짐) ==========
  sma: {
    type: 'sma',
    name: 'SMA',
    description: '단순이동평균 (Simple Moving Average)',
    defaultParams: { period: 20 },
    paramLabels: { period: '기간' },
    color: '#f59e0b',
    isOverlay: true,
  },
  ema: {
    type: 'ema',
    name: 'EMA',
    description: '지수이동평균 (Exponential Moving Average)',
    defaultParams: { period: 12 },
    paramLabels: { period: '기간' },
    color: '#ec4899',
    isOverlay: true,
  },
  bb: {
    type: 'bb',
    name: 'BB',
    description: '볼린저 밴드 (Bollinger Bands)',
    defaultParams: { period: 20, stdDev: 2 },
    paramLabels: { period: '기간', stdDev: '표준편차 배수' },
    color: '#06b6d4',
    isOverlay: true,
  },
  // ========== 서브 차트 지표 (별도 패널에 그려짐) ==========
  volume: {
    type: 'volume',
    name: 'Volume',
    description: '거래량',
    defaultParams: {},
    paramLabels: {},
    color: '#6b7280',
    isOverlay: false,
  },
  rsi: {
    type: 'rsi',
    name: 'RSI',
    description: '상대강도지수 (Relative Strength Index)',
    defaultParams: { period: 14 },
    paramLabels: { period: '기간' },
    scaleRange: { min: 0, max: 100, levels: [30, 70] },
    color: '#8b5cf6',
    isOverlay: false,
  },
  macd: {
    type: 'macd',
    name: 'MACD',
    description: '이동평균수렴확산 (Moving Average Convergence Divergence)',
    defaultParams: { fastPeriod: 12, slowPeriod: 26, signalPeriod: 9 },
    paramLabels: { fastPeriod: '빠른 기간', slowPeriod: '느린 기간', signalPeriod: '시그널 기간' },
    color: '#3b82f6',
    isOverlay: false,
  },
  stochastic: {
    type: 'stochastic',
    name: 'Stochastic',
    description: '스토캐스틱 오실레이터 (%K, %D)',
    defaultParams: { kPeriod: 14, dPeriod: 3 },
    paramLabels: { kPeriod: '%K 기간', dPeriod: '%D 기간' },
    scaleRange: { min: 0, max: 100, levels: [20, 80] },
    color: '#f59e0b',
    isOverlay: false,
  },
  atr: {
    type: 'atr',
    name: 'ATR',
    description: '평균진정범위 (Average True Range)',
    defaultParams: { period: 14 },
    paramLabels: { period: '기간' },
    color: '#ef4444',
    isOverlay: false,
  },
  atr_percent: {
    type: 'atr_percent',
    name: 'ATR%',
    description: '평균진정범위 백분율 (ATR / 종가 × 100)',
    defaultParams: { period: 14 },
    paramLabels: { period: '기간' },
    color: '#ec4899',
    isOverlay: false,
  },
  momentum: {
    type: 'momentum',
    name: 'Momentum',
    description: '다중 기간 모멘텀 점수',
    defaultParams: { periods: [5, 10, 20] },
    paramLabels: { periods: '기간들 (쉼표 구분)' },
    color: '#22c55e',
    isOverlay: false,
  },
}

// ==================== 타입 ====================

interface DatasetSummary {
  symbol: string
  displayName?: string  // "005930(삼성전자)" 형식
  timeframe: string
  firstTime: string | null
  lastTime: string | null
  candleCount: number
  lastUpdated: string | null
}

interface DatasetListResponse {
  datasets: DatasetSummary[]
  totalCount: number
}

interface CandleItem {
  time: string
  open: string
  high: string
  low: string
  close: string
  volume: string
}

interface CandleDataResponse {
  symbol: string
  timeframe: string
  candles: CandleItem[]
  totalCount: number
}

interface FetchDatasetRequest {
  symbol: string
  timeframe: string
  limit: number
  startDate?: string  // YYYY-MM-DD 형식
  endDate?: string    // YYYY-MM-DD 형식
}

interface Strategy {
  id: string
  name: string
  strategyType: string
  symbols: string[]
}

// ==================== API ====================

const API_BASE = 'http://localhost:3000/api/v1'

async function fetchDatasets(): Promise<DatasetListResponse> {
  const res = await fetch(`${API_BASE}/dataset`)
  if (!res.ok) throw new Error('데이터셋 목록 조회 실패')
  return res.json()
}

type SortColumnType = 'time' | 'close' | 'change' | 'volume' | 'open' | 'high' | 'low'
type SortOrderType = 'desc' | 'asc'

async function fetchCandles(
  symbol: string,
  timeframe: string,
  limit: number,
  sortBy: SortColumnType = 'time',
  sortOrder: SortOrderType = 'desc'
): Promise<CandleDataResponse> {
  const serverSortBy = sortBy === 'change' ? 'time' : sortBy
  const params = new URLSearchParams({
    timeframe,
    limit: limit.toString(),
    sortBy: serverSortBy,
    sortOrder,
  })
  const res = await fetch(`${API_BASE}/dataset/${encodeURIComponent(symbol)}?${params}`)
  if (!res.ok) throw new Error('캔들 데이터 조회 실패')
  return res.json()
}

async function downloadDataset(req: FetchDatasetRequest): Promise<{ fetchedCount: number; message: string }> {
  const res = await fetch(`${API_BASE}/dataset/fetch`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(req),
  })
  if (!res.ok) throw new Error('데이터셋 다운로드 실패')
  return res.json()
}

async function deleteDataset(symbol: string, timeframe?: string): Promise<void> {
  const url = timeframe
    ? `${API_BASE}/dataset/${encodeURIComponent(symbol)}?timeframe=${timeframe}`
    : `${API_BASE}/dataset/${encodeURIComponent(symbol)}`
  const res = await fetch(url, { method: 'DELETE' })
  if (!res.ok) throw new Error('데이터셋 삭제 실패')
}

async function fetchStrategies(): Promise<Strategy[]> {
  const res = await fetch(`${API_BASE}/strategies`)
  if (!res.ok) throw new Error('전략 목록 조회 실패')
  const data = await res.json()
  return data.strategies || []
}

// 심볼 검색 결과 타입
interface SymbolSearchResult {
  ticker: string
  name: string
  market: string
  yahooSymbol: string | null
}

interface SymbolSearchResponse {
  results: SymbolSearchResult[]
  total: number
}

// 심볼 검색 API
async function searchSymbols(query: string, limit: number = 10): Promise<SymbolSearchResult[]> {
  if (!query.trim()) return []
  const params = new URLSearchParams({ q: query, limit: limit.toString() })
  const res = await fetch(`${API_BASE}/dataset/search?${params}`)
  if (!res.ok) return []
  const data: SymbolSearchResponse = await res.json()
  return data.results || []
}

// ==================== 지표 계산 유틸 ====================

/** 타임프레임에 따라 적절한 시간 키 반환 (Lightweight Charts 호환) */
function getTimeKey(time: string, isDailyOrHigher: boolean): string | number {
  if (isDailyOrHigher) {
    // 일봉 이상: "YYYY-MM-DD" 형식
    return time.split(' ')[0]
  } else {
    // 시간봉 이하: Unix timestamp (초)로 변환
    // "2025-10-30 04:00:00" 형식을 파싱
    const date = new Date(time.replace(' ', 'T'))
    return Math.floor(date.getTime() / 1000)
  }
}

function calculateSMA(data: CandleItem[], period: number, isDailyOrHigher = true): LineDataPoint[] {
  if (data.length < period) return []
  const result: LineDataPoint[] = []
  for (let i = period - 1; i < data.length; i++) {
    let sum = 0
    for (let j = 0; j < period; j++) {
      sum += parseFloat(data[i - j].close)
    }
    result.push({
      time: getTimeKey(data[i].time, isDailyOrHigher),
      value: sum / period,
    })
  }
  return result
}

function calculateEMA(data: CandleItem[], period: number, isDailyOrHigher = true): LineDataPoint[] {
  if (data.length < period) return []
  const k = 2 / (period + 1)
  const result: LineDataPoint[] = []

  // 첫 EMA는 SMA로 시작
  let sum = 0
  for (let i = 0; i < period; i++) {
    sum += parseFloat(data[i].close)
  }
  let ema = sum / period
  result.push({ time: getTimeKey(data[period - 1].time, isDailyOrHigher), value: ema })

  for (let i = period; i < data.length; i++) {
    ema = parseFloat(data[i].close) * k + ema * (1 - k)
    result.push({ time: getTimeKey(data[i].time, isDailyOrHigher), value: ema })
  }
  return result
}

function calculateBollingerBands(data: CandleItem[], period: number = 20, stdDev: number = 2, isDailyOrHigher = true) {
  if (data.length < period) return { upper: [], middle: [], lower: [] }

  const middle: LineDataPoint[] = []
  const upper: LineDataPoint[] = []
  const lower: LineDataPoint[] = []

  for (let i = period - 1; i < data.length; i++) {
    let sum = 0
    const values: number[] = []
    for (let j = 0; j < period; j++) {
      const val = parseFloat(data[i - j].close)
      sum += val
      values.push(val)
    }
    const mean = sum / period
    const variance = values.reduce((acc, v) => acc + Math.pow(v - mean, 2), 0) / period
    const std = Math.sqrt(variance)

    const time = getTimeKey(data[i].time, isDailyOrHigher)
    middle.push({ time, value: mean })
    upper.push({ time, value: mean + stdDev * std })
    lower.push({ time, value: mean - stdDev * std })
  }

  return { upper, middle, lower }
}

/** RSI 계산 (Relative Strength Index) */
function calculateRSI(data: CandleItem[], period: number = 14, isDailyOrHigher = true): LineDataPoint[] {
  if (data.length < period + 1) return []

  const result: LineDataPoint[] = []
  const gains: number[] = []
  const losses: number[] = []

  // 변화량 계산
  for (let i = 1; i < data.length; i++) {
    const change = parseFloat(data[i].close) - parseFloat(data[i - 1].close)
    gains.push(change > 0 ? change : 0)
    losses.push(change < 0 ? Math.abs(change) : 0)
  }

  // 첫 번째 평균 계산
  let avgGain = gains.slice(0, period).reduce((a, b) => a + b, 0) / period
  let avgLoss = losses.slice(0, period).reduce((a, b) => a + b, 0) / period

  for (let i = period; i < gains.length; i++) {
    if (i === period) {
      // 첫 RSI 값
      const rs = avgLoss === 0 ? 100 : avgGain / avgLoss
      const rsi = 100 - (100 / (1 + rs))
      result.push({ time: getTimeKey(data[i + 1].time, isDailyOrHigher), value: rsi })
    } else {
      // Smoothed 이동 평균
      avgGain = (avgGain * (period - 1) + gains[i]) / period
      avgLoss = (avgLoss * (period - 1) + losses[i]) / period
      const rs = avgLoss === 0 ? 100 : avgGain / avgLoss
      const rsi = 100 - (100 / (1 + rs))
      result.push({ time: getTimeKey(data[i + 1].time, isDailyOrHigher), value: rsi })
    }
  }

  return result
}

/** MACD 계산 (Moving Average Convergence Divergence) */
function calculateMACD(data: CandleItem[], fastPeriod: number = 12, slowPeriod: number = 26, signalPeriod: number = 9, isDailyOrHigher = true) {
  if (data.length < slowPeriod + signalPeriod) return { macd: [], signal: [], histogram: [] }

  // EMA 계산 헬퍼
  const getEMA = (values: number[], period: number): number[] => {
    const k = 2 / (period + 1)
    const ema: number[] = []
    let sum = 0
    for (let i = 0; i < period; i++) {
      sum += values[i]
    }
    ema.push(sum / period)
    for (let i = period; i < values.length; i++) {
      ema.push(values[i] * k + ema[ema.length - 1] * (1 - k))
    }
    return ema
  }

  const closes = data.map(d => parseFloat(d.close))
  const fastEMA = getEMA(closes, fastPeriod)
  const slowEMA = getEMA(closes, slowPeriod)

  // MACD 라인 (fastEMA - slowEMA)
  const macdLine: number[] = []
  const offset = slowPeriod - fastPeriod
  for (let i = 0; i < slowEMA.length; i++) {
    macdLine.push(fastEMA[i + offset] - slowEMA[i])
  }

  // Signal 라인 (MACD의 9일 EMA)
  const signalLine = getEMA(macdLine, signalPeriod)

  // 결과 생성
  const macd: LineDataPoint[] = []
  const signal: LineDataPoint[] = []
  const histogram: LineDataPoint[] = []

  const startIdx = slowPeriod - 1 + signalPeriod - 1
  for (let i = 0; i < signalLine.length; i++) {
    const dataIdx = startIdx + i
    if (dataIdx >= data.length) break
    const time = getTimeKey(data[dataIdx].time, isDailyOrHigher)
    const macdVal = macdLine[i + signalPeriod - 1]
    const signalVal = signalLine[i]

    macd.push({ time, value: macdVal })
    signal.push({ time, value: signalVal })
    histogram.push({ time, value: macdVal - signalVal })
  }

  return { macd, signal, histogram }
}

/** Volume 데이터 생성 */
function calculateVolume(data: CandleItem[], isDailyOrHigher = true): { data: LineDataPoint[], colors: string[] } {
  const result: LineDataPoint[] = []
  const colors: string[] = []

  for (const candle of data) {
    const isUp = parseFloat(candle.close) >= parseFloat(candle.open)
    result.push({
      time: getTimeKey(candle.time, isDailyOrHigher),
      value: parseInt(candle.volume),
    })
    colors.push(isUp ? '#22c55e' : '#ef4444')
  }

  return { data: result, colors }
}

/** Stochastic Oscillator 계산 (%K, %D) */
function calculateStochastic(data: CandleItem[], kPeriod: number = 14, dPeriod: number = 3, isDailyOrHigher = true): { k: LineDataPoint[], d: LineDataPoint[] } {
  if (data.length < kPeriod) return { k: [], d: [] }

  const kValues: LineDataPoint[] = []

  // %K 계산: (현재 종가 - N기간 최저가) / (N기간 최고가 - N기간 최저가) * 100
  for (let i = kPeriod - 1; i < data.length; i++) {
    let lowestLow = Infinity
    let highestHigh = -Infinity

    for (let j = 0; j < kPeriod; j++) {
      const high = parseFloat(data[i - j].high)
      const low = parseFloat(data[i - j].low)
      if (high > highestHigh) highestHigh = high
      if (low < lowestLow) lowestLow = low
    }

    const close = parseFloat(data[i].close)
    const range = highestHigh - lowestLow
    const k = range === 0 ? 50 : ((close - lowestLow) / range) * 100

    kValues.push({
      time: getTimeKey(data[i].time, isDailyOrHigher),
      value: k,
    })
  }

  // %D 계산: %K의 dPeriod 이동평균
  const dValues: LineDataPoint[] = []
  if (kValues.length >= dPeriod) {
    for (let i = dPeriod - 1; i < kValues.length; i++) {
      let sum = 0
      for (let j = 0; j < dPeriod; j++) {
        sum += kValues[i - j].value
      }
      dValues.push({
        time: kValues[i].time,
        value: sum / dPeriod,
      })
    }
  }

  return { k: kValues, d: dValues }
}

/** ATR (Average True Range) 계산 */
function calculateATR(data: CandleItem[], period: number = 14, isDailyOrHigher = true): LineDataPoint[] {
  if (data.length < period + 1) return []

  const trValues: number[] = []

  // True Range 계산
  for (let i = 1; i < data.length; i++) {
    const high = parseFloat(data[i].high)
    const low = parseFloat(data[i].low)
    const prevClose = parseFloat(data[i - 1].close)

    const tr = Math.max(
      high - low,                    // 당일 고가 - 저가
      Math.abs(high - prevClose),    // |당일 고가 - 전일 종가|
      Math.abs(low - prevClose)      // |당일 저가 - 전일 종가|
    )
    trValues.push(tr)
  }

  // ATR 계산 (첫 번째는 단순 평균, 이후 지수 이동 평균)
  const result: LineDataPoint[] = []
  let atr = trValues.slice(0, period).reduce((a, b) => a + b, 0) / period

  result.push({
    time: getTimeKey(data[period].time, isDailyOrHigher),
    value: atr,
  })

  for (let i = period; i < trValues.length; i++) {
    atr = (atr * (period - 1) + trValues[i]) / period
    result.push({
      time: getTimeKey(data[i + 1].time, isDailyOrHigher),
      value: atr,
    })
  }

  return result
}

/** ATR Percent 계산 (ATR을 종가 대비 백분율로) */
function calculateATRPercent(data: CandleItem[], period: number = 14, isDailyOrHigher = true): LineDataPoint[] {
  const atrData = calculateATR(data, period, isDailyOrHigher)
  if (atrData.length === 0) return []

  // ATR 데이터와 매칭되는 종가 찾기
  const result: LineDataPoint[] = []
  for (const atrPoint of atrData) {
    const candle = data.find(d => getTimeKey(d.time, isDailyOrHigher) === atrPoint.time)
    if (candle) {
      const close = parseFloat(candle.close)
      result.push({
        time: atrPoint.time,
        value: (atrPoint.value / close) * 100,
      })
    }
  }

  return result
}

/** Momentum Score 계산 (다중 기간 수익률 합산) */
function calculateMomentumScore(data: CandleItem[], periods: number[] = [5, 10, 20], isDailyOrHigher = true): LineDataPoint[] {
  const maxPeriod = Math.max(...periods)
  if (data.length < maxPeriod + 1) return []

  const result: LineDataPoint[] = []

  for (let i = maxPeriod; i < data.length; i++) {
    let score = 0
    const currentClose = parseFloat(data[i].close)

    for (const period of periods) {
      const pastClose = parseFloat(data[i - period].close)
      const returns = ((currentClose - pastClose) / pastClose) * 100
      score += returns
    }

    // 정규화: 기간 수로 나눠서 평균 수익률로 표현
    result.push({
      time: getTimeKey(data[i].time, isDailyOrHigher),
      value: score / periods.length,
    })
  }

  return result
}

// ==================== 타임프레임 유틸 ====================

const timeframeText: Record<string, string> = {
  '1m': '1분', '5m': '5분', '15m': '15분', '30m': '30분',
  '1h': '1시간', '2h': '2시간', '4h': '4시간',
  '1d': '일봉', '1wk': '주봉', '1mo': '월봉',
}

const columnText: Record<SortColumnType, string> = {
  time: '시간', open: '시가', high: '고가', low: '저가',
  close: '종가', change: '변동', volume: '거래량',
}

// ==================== 패널 콘텐츠 컴포넌트 ====================

interface SymbolPanelProps {
  symbol?: string
  timeframe: string
  datasets: DatasetSummary[]
  cachedSymbols: string[]
  onSymbolChange: (symbol: string) => void
  onTimeframeChange: (tf: string) => void
  onRefresh: () => void
  onDelete: () => void
  isRefreshing: boolean
  compact?: boolean
}

function SymbolPanel(props: SymbolPanelProps) {
  const [viewMode, setViewMode] = createSignal<'chart' | 'table'>('chart')
  const [sortColumn, setSortColumn] = createSignal<SortColumnType>('time')
  const [sortOrder, setSortOrder] = createSignal<SortOrderType>('desc')

  // 통합 지표 목록 (오버레이 + 서브차트)
  const [activeIndicators, setActiveIndicators] = createSignal<ActiveIndicator[]>([
    { id: 'vol-1', type: 'volume', params: {}, enabled: true, isOverlay: false },
  ])

  // 지표 추가 모달 상태
  const [showIndicatorModal, setShowIndicatorModal] = createSignal(false)
  const [newIndicatorType, setNewIndicatorType] = createSignal<IndicatorType>('rsi')
  const [newIndicatorParams, setNewIndicatorParams] = createSignal<Record<string, unknown>>({})

  // 패널 내 심볼 검색 (자동완성)
  const [panelSearch, setPanelSearch] = createSignal('')
  const [showAutocomplete, setShowAutocomplete] = createSignal(false)
  const [selectedIndex, setSelectedIndex] = createSignal(-1)

  // 테이블 무한 스크롤 상태
  const [visibleRows, setVisibleRows] = createSignal(50)
  const ROWS_PER_LOAD = 50
  let tableEndRef: HTMLDivElement | undefined

  // 테이블 뷰로 전환 시 표시 행 수 리셋
  createEffect(() => {
    if (viewMode() === 'table') {
      setVisibleRows(50)
    }
  })

  // Intersection Observer로 무한 스크롤 구현
  createEffect(() => {
    if (viewMode() !== 'table' || !tableEndRef) return

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0].isIntersecting) {
          const totalRows = tableData().length
          if (visibleRows() < totalRows) {
            setVisibleRows(prev => Math.min(prev + ROWS_PER_LOAD, totalRows))
          }
        }
      },
      { threshold: 0.1 }
    )

    observer.observe(tableEndRef)

    onCleanup(() => observer.disconnect())
  })

  // 새 지표 타입 선택 시 기본 파라미터 설정
  createEffect(() => {
    const type = newIndicatorType()
    const meta = INDICATOR_META[type]
    setNewIndicatorParams({ ...meta.defaultParams })
  })

  // 지표 추가 함수
  const addIndicator = () => {
    const type = newIndicatorType()
    const params = newIndicatorParams()
    const meta = INDICATOR_META[type]
    const newIndicator: ActiveIndicator = {
      id: `${type}-${Date.now()}`,
      type,
      params: params as IndicatorParams[typeof type],
      enabled: true,
      isOverlay: meta.isOverlay,
    }
    setActiveIndicators(prev => [...prev, newIndicator])
    setShowIndicatorModal(false)
  }

  // 지표 제거 함수
  const removeIndicator = (id: string) => {
    setActiveIndicators(prev => prev.filter(ind => ind.id !== id))
  }

  // 지표 토글 함수
  const toggleIndicator = (id: string) => {
    setActiveIndicators(prev => prev.map(ind =>
      ind.id === id ? { ...ind, enabled: !ind.enabled } : ind
    ))
  }

  // 자동완성 심볼 목록 (캐시된 심볼만 필터링)
  const autocompleteSymbols = createMemo(() => {
    const term = panelSearch().toUpperCase().trim()
    if (!term) return []
    // 캐시된 심볼 중 검색어와 매칭되는 것만 표시 (최대 8개)
    return props.cachedSymbols
      .filter(s => s.toUpperCase().includes(term))
      .slice(0, 8)
  })

  // 캔들 데이터 쿼리
  const candlesQuery = createQuery(() => ({
    queryKey: ['candles', props.symbol, props.timeframe, 500, sortColumn(), sortOrder()],
    queryFn: () => fetchCandles(props.symbol!, props.timeframe, 500, sortColumn(), sortOrder()),
    enabled: !!props.symbol && !!props.timeframe,
    staleTime: 30000,
  }))

  // 타임프레임 목록
  const availableTimeframes = createMemo(() => {
    const order = ['1m', '5m', '15m', '30m', '1h', '2h', '4h', '1d', '1wk', '1mo']
    return props.datasets
      .filter(d => d.symbol === props.symbol)
      .map(d => d.timeframe)
      .sort((a, b) => order.indexOf(a) - order.indexOf(b))
  })

  // 현재 데이터셋 정보
  const currentDataset = createMemo(() =>
    props.datasets.find(d => d.symbol === props.symbol && d.timeframe === props.timeframe)
  )

  // 일봉 이상 타임프레임인지 확인
  const isDailyOrHigher = createMemo(() => {
    const tf = props.timeframe
    return tf === '1d' || tf === '3d' || tf === '1wk' || tf === '1mo' || tf === 'd' || tf === 'w' || tf === 'M'
  })

  // 차트 데이터 (오름차순, 타임프레임에 따라 시간 형식 결정)
  const chartData = createMemo((): CandlestickDataPoint[] => {
    const candles = candlesQuery.data?.candles || []
    const daily = isDailyOrHigher()
    const uniqueMap = new Map<string | number, CandlestickDataPoint>()

    candles.forEach(c => {
      // 일봉 이상이면 날짜만 사용, 시간봉 이하면 Unix timestamp로 변환
      const timeKey = getTimeKey(c.time, daily)
      uniqueMap.set(timeKey, {
        time: timeKey,
        open: parseFloat(c.open),
        high: parseFloat(c.high),
        low: parseFloat(c.low),
        close: parseFloat(c.close),
      })
    })

    // 정렬: 일봉은 문자열 비교, 시간봉은 숫자 비교
    return Array.from(uniqueMap.values()).sort((a, b) => {
      if (typeof a.time === 'number' && typeof b.time === 'number') {
        return a.time - b.time
      }
      return (a.time as string).localeCompare(b.time as string)
    })
  })

  // 오버레이 지표 계산 (동적 시스템)
  const indicators = createMemo((): IndicatorOverlay[] => {
    const candles = candlesQuery.data?.candles || []
    if (candles.length === 0) return []

    // 오름차순 정렬 (지표 계산용)
    const sortedCandles = [...candles].sort((a, b) => a.time.localeCompare(b.time))
    const result: IndicatorOverlay[] = []
    const daily = isDailyOrHigher()

    // 활성화된 오버레이 지표만 계산
    for (const indicator of activeIndicators()) {
      if (!indicator.enabled || !indicator.isOverlay) continue

      const meta = INDICATOR_META[indicator.type]

      switch (indicator.type) {
        case 'sma': {
          const params = indicator.params as IndicatorParams['sma']
          const smaData = calculateSMA(sortedCandles, params.period, daily)
          if (smaData.length > 0) {
            result.push({
              id: indicator.id,
              name: `SMA ${params.period}`,
              data: smaData,
              color: meta.color,
              lineWidth: 1,
            })
          }
          break
        }

        case 'ema': {
          const params = indicator.params as IndicatorParams['ema']
          const emaData = calculateEMA(sortedCandles, params.period, daily)
          if (emaData.length > 0) {
            result.push({
              id: indicator.id,
              name: `EMA ${params.period}`,
              data: emaData,
              color: meta.color,
              lineWidth: 1,
            })
          }
          break
        }

        case 'bb': {
          const params = indicator.params as IndicatorParams['bb']
          const bb = calculateBollingerBands(sortedCandles, params.period, params.stdDev, daily)
          if (bb.middle.length > 0) {
            result.push({ id: `${indicator.id}-upper`, name: 'BB Upper', data: bb.upper, color: meta.color, lineWidth: 1 })
            result.push({ id: `${indicator.id}-middle`, name: 'BB Middle', data: bb.middle, color: meta.color, lineWidth: 1 })
            result.push({ id: `${indicator.id}-lower`, name: 'BB Lower', data: bb.lower, color: meta.color, lineWidth: 1 })
          }
          break
        }
      }
    }

    return result
  })

  // 서브 차트 지표 데이터 (동적 생성)
  const subIndicators = createMemo((): SeparateIndicatorData[] => {
    const candles = candlesQuery.data?.candles || []
    if (candles.length === 0) return []

    const sortedCandles = [...candles].sort((a, b) => a.time.localeCompare(b.time))
    const result: SeparateIndicatorData[] = []
    const daily = isDailyOrHigher()

    // 서브 차트 지표만 필터링 (isOverlay가 false인 것)
    for (const indicator of activeIndicators()) {
      if (!indicator.enabled || indicator.isOverlay) continue

      const meta = INDICATOR_META[indicator.type]

      switch (indicator.type) {
        case 'volume': {
          const volumeData = calculateVolume(sortedCandles, daily)
          result.push({
            id: indicator.id,
            type: 'volume',
            name: 'Volume',
            series: [{
              name: 'Volume',
              data: volumeData.data,
              color: meta.color,
              seriesType: 'bar',
            }],
          })
          break
        }

        case 'rsi': {
          const params = indicator.params as IndicatorParams['rsi']
          const rsiData = calculateRSI(sortedCandles, params.period, daily)
          if (rsiData.length > 0) {
            result.push({
              id: indicator.id,
              type: 'rsi',
              name: `RSI (${params.period})`,
              series: [{
                name: 'RSI',
                data: rsiData,
                color: meta.color,
                seriesType: 'line',
                lineWidth: 2,
              }],
              scaleRange: meta.scaleRange,
            })
          }
          break
        }

        case 'macd': {
          const params = indicator.params as IndicatorParams['macd']
          const macdData = calculateMACD(sortedCandles, params.fastPeriod, params.slowPeriod, params.signalPeriod, daily)
          if (macdData.macd.length > 0) {
            result.push({
              id: indicator.id,
              type: 'macd',
              name: `MACD (${params.fastPeriod}, ${params.slowPeriod}, ${params.signalPeriod})`,
              series: [
                { name: 'MACD', data: macdData.macd, color: '#3b82f6', seriesType: 'line', lineWidth: 2 },
                { name: 'Signal', data: macdData.signal, color: '#f97316', seriesType: 'line', lineWidth: 1 },
                { name: 'Histogram', data: macdData.histogram, color: '#22c55e', seriesType: 'bar' },
              ],
            })
          }
          break
        }

        case 'stochastic': {
          const params = indicator.params as IndicatorParams['stochastic']
          const stochData = calculateStochastic(sortedCandles, params.kPeriod, params.dPeriod, daily)
          if (stochData.k.length > 0) {
            result.push({
              id: indicator.id,
              type: 'stochastic',
              name: `Stochastic (${params.kPeriod}, ${params.dPeriod})`,
              series: [
                { name: '%K', data: stochData.k, color: '#f59e0b', seriesType: 'line', lineWidth: 2 },
                { name: '%D', data: stochData.d, color: '#a855f7', seriesType: 'line', lineWidth: 1 },
              ],
              scaleRange: meta.scaleRange,
            })
          }
          break
        }

        case 'atr': {
          const params = indicator.params as IndicatorParams['atr']
          const atrData = calculateATR(sortedCandles, params.period, daily)
          if (atrData.length > 0) {
            result.push({
              id: indicator.id,
              type: 'atr',
              name: `ATR (${params.period})`,
              series: [{
                name: 'ATR',
                data: atrData,
                color: meta.color,
                seriesType: 'line',
                lineWidth: 2,
              }],
            })
          }
          break
        }

        case 'atr_percent': {
          const params = indicator.params as IndicatorParams['atr_percent']
          const atrPctData = calculateATRPercent(sortedCandles, params.period, daily)
          if (atrPctData.length > 0) {
            result.push({
              id: indicator.id,
              type: 'atr_percent',
              name: `ATR% (${params.period})`,
              series: [{
                name: 'ATR%',
                data: atrPctData,
                color: meta.color,
                seriesType: 'line',
                lineWidth: 2,
              }],
            })
          }
          break
        }

        case 'momentum': {
          const params = indicator.params as IndicatorParams['momentum']
          const momData = calculateMomentumScore(sortedCandles, params.periods, daily)
          if (momData.length > 0) {
            result.push({
              id: indicator.id,
              type: 'momentum',
              name: `Momentum (${params.periods.join(', ')})`,
              series: [{
                name: 'Momentum',
                data: momData,
                color: meta.color,
                seriesType: 'line',
                lineWidth: 2,
              }],
            })
          }
          break
        }
      }
    }

    return result
  })

  // 테이블 데이터
  const tableData = createMemo(() => {
    const candles = candlesQuery.data?.candles || []
    if (sortColumn() === 'change') {
      return [...candles].sort((a, b) => {
        const changeA = (parseFloat(a.close) - parseFloat(a.open)) / parseFloat(a.open)
        const changeB = (parseFloat(b.close) - parseFloat(b.open)) / parseFloat(b.open)
        return sortOrder() === 'desc' ? changeB - changeA : changeA - changeB
      })
    }
    return candles
  })

  // 컬럼 정렬 핸들러
  const handleColumnSort = (column: SortColumnType) => {
    if (sortColumn() === column) {
      setSortOrder(prev => prev === 'desc' ? 'asc' : 'desc')
    } else {
      setSortColumn(column)
      setSortOrder('desc')
    }
  }

  // 날짜 포맷
  const formatDate = (dateStr: string | null) => {
    if (!dateStr) return '-'
    return new Date(dateStr).toLocaleDateString('ko-KR', { year: 'numeric', month: '2-digit', day: '2-digit' })
  }

  // 서브 차트 개수에 따른 메인 차트 높이 조절
  const subChartCount = () => subIndicators().length
  const chartHeight = () => {
    const base = props.compact ? 160 : 240
    // 서브차트가 있으면 메인 차트 높이를 줄임
    if (subChartCount() > 0) return Math.max(120, base - subChartCount() * 20)
    return base
  }
  const subChartHeight = () => props.compact ? 80 : 100

  // 심볼 선택 핸들러
  const handleSelectSymbol = (symbol: string) => {
    props.onSymbolChange(symbol)
    setPanelSearch('')
    setShowAutocomplete(false)
    setSelectedIndex(-1)
  }

  // 키보드 네비게이션 핸들러
  const handleKeyDown = (e: KeyboardEvent) => {
    const symbols = autocompleteSymbols()
    const len = symbols.length

    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setSelectedIndex(prev => (prev + 1) % len)
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      setSelectedIndex(prev => (prev - 1 + len) % len)
    } else if (e.key === 'Enter') {
      e.preventDefault()
      const idx = selectedIndex()
      if (idx >= 0 && idx < len) {
        handleSelectSymbol(symbols[idx])
      } else if (panelSearch().trim()) {
        // 검색어가 있으면 그대로 사용 (새 심볼 다운로드용)
        handleSelectSymbol(panelSearch().trim().toUpperCase())
      }
    } else if (e.key === 'Escape') {
      setShowAutocomplete(false)
      setSelectedIndex(-1)
    }
  }

  // 심볼 자동완성 UI
  const SymbolSearchUI = () => (
    <div class="h-full flex flex-col items-center justify-center p-4">
      {/* 심볼 자동완성 입력 */}
      <div class="w-full max-w-md">
        <div class="relative">
          <Search class="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-[var(--color-text-muted)]" />
          <input
            type="text"
            value={panelSearch()}
            onInput={(e) => {
              setPanelSearch(e.currentTarget.value)
              setShowAutocomplete(true)
              setSelectedIndex(-1)
            }}
            onFocus={() => setShowAutocomplete(true)}
            onBlur={() => setTimeout(() => setShowAutocomplete(false), 200)}
            onKeyDown={handleKeyDown}
            placeholder="심볼 검색 (예: AAPL, 005930)..."
            class="w-full pl-10 pr-4 py-3 text-base bg-[var(--color-bg)] text-[var(--color-text)]
                   rounded-xl border-2 border-[var(--color-surface-light)]
                   focus:outline-none focus:border-[var(--color-primary)]
                   placeholder:text-[var(--color-text-muted)]"
          />

          {/* 자동완성 드롭다운 */}
          <Show when={showAutocomplete() && panelSearch().trim() && autocompleteSymbols().length > 0}>
            <div class="absolute top-full left-0 right-0 mt-1 bg-[var(--color-surface)]
                        border border-[var(--color-surface-light)] rounded-lg shadow-xl z-50
                        max-h-64 overflow-auto">
              <For each={autocompleteSymbols()}>
                {(symbol, index) => (
                  <button
                    onMouseDown={(e) => {
                      e.preventDefault()
                      handleSelectSymbol(symbol)
                    }}
                    class={`w-full px-4 py-2.5 text-left text-sm font-mono flex items-center gap-2
                            transition hover:bg-[var(--color-surface-light)]
                            ${index() === selectedIndex()
                              ? 'bg-[var(--color-primary)]/20 text-[var(--color-primary)]'
                              : 'text-[var(--color-text)]'}`}
                  >
                    <TrendingUp class="w-4 h-4 text-[var(--color-primary)]" />
                    <span>{symbol}</span>
                    <Show when={props.cachedSymbols.includes(symbol)}>
                      <span class="ml-auto text-xs text-[var(--color-text-muted)] bg-[var(--color-bg)] px-1.5 py-0.5 rounded">
                        캐시됨
                      </span>
                    </Show>
                  </button>
                )}
              </For>
            </div>
          </Show>
        </div>

        {/* 힌트 텍스트 */}
        <p class="text-center text-xs text-[var(--color-text-muted)] mt-3">
          심볼을 입력하여 검색하거나 새 심볼을 입력 후 Enter
        </p>
      </div>
    </div>
  )

  return (
    <Show when={props.symbol} fallback={<SymbolSearchUI />}>
    <div class="h-full flex flex-col gap-2 overflow-hidden">
      {/* 심볼 + 타임프레임 + 액션 */}
      <div class="flex items-center justify-between flex-shrink-0">
        <div class="flex items-center gap-2">
          {/* 심볼 표시 및 변경 */}
          <button
            onClick={() => props.onSymbolChange('')}
            class="px-2 py-1 text-xs font-mono font-semibold bg-[var(--color-primary)]/20
                   text-[var(--color-primary)] rounded hover:bg-[var(--color-primary)]/30
                   transition flex items-center gap-1"
            title="심볼 변경"
          >
            <TrendingUp class="w-3 h-3" />
            {props.symbol}
            <X class="w-3 h-3 ml-1 opacity-60" />
          </button>
          {/* 타임프레임 */}
          <div class="flex items-center gap-0.5">
            <For each={availableTimeframes()}>
              {(tf) => (
                <button
                  onClick={() => props.onTimeframeChange(tf)}
                  class={`px-1.5 py-0.5 text-xs rounded transition
                          ${props.timeframe === tf
                            ? 'bg-[var(--color-primary)] text-white'
                            : 'bg-[var(--color-bg)] text-[var(--color-text-muted)] hover:text-[var(--color-text)]'}`}
                >
                  {timeframeText[tf] || tf}
                </button>
              )}
            </For>
          </div>
        </div>
        <div class="flex items-center gap-1">
          <button
            onClick={props.onRefresh}
            disabled={props.isRefreshing}
            class="p-1 hover:bg-[var(--color-surface-light)] rounded"
            title="새로고침"
          >
            <RefreshCw class={`w-3.5 h-3.5 text-[var(--color-text-muted)] ${props.isRefreshing ? 'animate-spin' : ''}`} />
          </button>
          <button
            onClick={props.onDelete}
            class="p-1 hover:bg-red-500/20 rounded"
            title="삭제"
          >
            <Trash2 class="w-3.5 h-3.5 text-red-400" />
          </button>
        </div>
      </div>

      {/* 데이터셋 정보 (컴팩트 모드 아닐 때만) */}
      <Show when={!props.compact && currentDataset()}>
        {(dataset) => (
          <div class="grid grid-cols-4 gap-2 text-xs flex-shrink-0">
            <div>
              <span class="text-[var(--color-text-muted)]">시작</span>
              <p class="text-[var(--color-text)]">{formatDate(dataset().firstTime)}</p>
            </div>
            <div>
              <span class="text-[var(--color-text-muted)]">종료</span>
              <p class="text-[var(--color-text)]">{formatDate(dataset().lastTime)}</p>
            </div>
            <div>
              <span class="text-[var(--color-text-muted)]">캔들</span>
              <p class="text-[var(--color-text)]">{dataset().candleCount.toLocaleString()}</p>
            </div>
            <div>
              <span class="text-[var(--color-text-muted)]">업데이트</span>
              <p class="text-[var(--color-text)]">{formatDate(dataset().lastUpdated)}</p>
            </div>
          </div>
        )}
      </Show>

      {/* 뷰 모드 + 지표 토글 */}
      <div class="flex items-center justify-between flex-shrink-0 flex-wrap gap-1">
        <div class="flex gap-1">
          <button
            onClick={() => setViewMode('chart')}
            class={`px-2 py-1 text-xs rounded flex items-center gap-1 transition
                    ${viewMode() === 'chart'
                      ? 'bg-[var(--color-primary)] text-white'
                      : 'bg-[var(--color-bg)] text-[var(--color-text-muted)]'}`}
          >
            <LineChart class="w-3 h-3" />
            차트
          </button>
          <button
            onClick={() => setViewMode('table')}
            class={`px-2 py-1 text-xs rounded flex items-center gap-1 transition
                    ${viewMode() === 'table'
                      ? 'bg-[var(--color-primary)] text-white'
                      : 'bg-[var(--color-bg)] text-[var(--color-text-muted)]'}`}
          >
            <Table class="w-3 h-3" />
            테이블
          </button>
        </div>

        <Show when={viewMode() === 'chart'}>
          <div class="flex items-center gap-1 flex-wrap">
            {/* 오버레이 지표 토글 */}
            <Show when={activeIndicators().filter(i => i.isOverlay).length > 0}>
              <div class="flex gap-0.5 items-center">
                <For each={activeIndicators().filter(i => i.isOverlay)}>
                  {(ind) => (
                    <div class="flex items-center">
                      <button
                        onClick={() => toggleIndicator(ind.id)}
                        class={`px-1.5 py-0.5 text-xs rounded-l transition
                                ${ind.enabled
                                  ? `bg-opacity-30 text-opacity-100`
                                  : 'bg-[var(--color-bg)] text-[var(--color-text-muted)]'}`}
                        style={{
                          'background-color': ind.enabled ? `${INDICATOR_META[ind.type].color}30` : undefined,
                          color: ind.enabled ? INDICATOR_META[ind.type].color : undefined,
                        }}
                        title={INDICATOR_META[ind.type].description}
                      >
                        {INDICATOR_META[ind.type].name}
                        {Object.keys(ind.params).length > 0 && (
                          <span class="ml-0.5 opacity-70">
                            {Object.values(ind.params)[0]}
                          </span>
                        )}
                      </button>
                      <button
                        onClick={() => removeIndicator(ind.id)}
                        class="px-1 py-0.5 text-xs rounded-r bg-red-500/20 text-red-400 hover:bg-red-500/40 transition"
                        title="제거"
                      >
                        <X class="w-2.5 h-2.5" />
                      </button>
                    </div>
                  )}
                </For>
              </div>
              <span class="text-[var(--color-text-muted)] text-xs">|</span>
            </Show>
            {/* 서브 차트 지표 토글 */}
            <div class="flex gap-0.5 items-center">
              <For each={activeIndicators().filter(i => !i.isOverlay)}>
                {(ind) => (
                  <div class="flex items-center">
                    <button
                      onClick={() => toggleIndicator(ind.id)}
                      class={`px-1.5 py-0.5 text-xs rounded-l transition
                              ${ind.enabled
                                ? `bg-opacity-30 text-opacity-100`
                                : 'bg-[var(--color-bg)] text-[var(--color-text-muted)]'}`}
                      style={{
                        'background-color': ind.enabled ? `${INDICATOR_META[ind.type].color}30` : undefined,
                        color: ind.enabled ? INDICATOR_META[ind.type].color : undefined,
                      }}
                      title={INDICATOR_META[ind.type].description}
                    >
                      {INDICATOR_META[ind.type].name}
                    </button>
                    <button
                      onClick={() => removeIndicator(ind.id)}
                      class="px-1 py-0.5 text-xs rounded-r bg-red-500/20 text-red-400 hover:bg-red-500/40 transition"
                      title="제거"
                    >
                      <X class="w-2.5 h-2.5" />
                    </button>
                  </div>
                )}
              </For>
              {/* 지표 추가 버튼 */}
              <button
                onClick={() => setShowIndicatorModal(true)}
                class="px-1.5 py-0.5 text-xs rounded bg-[var(--color-primary)]/20 text-[var(--color-primary)]
                       hover:bg-[var(--color-primary)]/30 transition flex items-center gap-0.5"
                title="지표 추가"
              >
                <Settings2 class="w-3 h-3" />
                +
              </button>
            </div>
          </div>
        </Show>
      </div>

      {/* 지표 추가 모달 */}
      <Show when={showIndicatorModal()}>
        <div class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center"
             onClick={(e) => { if (e.target === e.currentTarget) setShowIndicatorModal(false) }}>
          <div class="bg-[var(--color-surface)] rounded-xl p-4 w-80 max-w-[90vw] shadow-xl">
            <div class="flex items-center justify-between mb-4">
              <h3 class="text-sm font-semibold text-[var(--color-text)]">지표 추가</h3>
              <button
                onClick={() => setShowIndicatorModal(false)}
                class="p-1 hover:bg-[var(--color-surface-light)] rounded"
              >
                <X class="w-4 h-4 text-[var(--color-text-muted)]" />
              </button>
            </div>

            {/* 지표 타입 선택 */}
            <div class="mb-4">
              <label class="block text-xs text-[var(--color-text-muted)] mb-1">지표 종류</label>
              <select
                value={newIndicatorType()}
                onChange={(e) => setNewIndicatorType(e.currentTarget.value as IndicatorType)}
                style={{ "background-color": "#1a1a2e" }}
                class="w-full px-3 py-2 text-sm text-[var(--color-text)] rounded-lg border border-[var(--color-surface-light)]"
              >
                <optgroup label="📊 오버레이 지표 (메인 차트)">
                  <For each={Object.entries(INDICATOR_META).filter(([_, m]) => m.isOverlay)}>
                    {([type, meta]) => (
                      <option value={type}>{meta.name} - {meta.description}</option>
                    )}
                  </For>
                </optgroup>
                <optgroup label="📈 서브 차트 지표">
                  <For each={Object.entries(INDICATOR_META).filter(([_, m]) => !m.isOverlay)}>
                    {([type, meta]) => (
                      <option value={type}>{meta.name} - {meta.description}</option>
                    )}
                  </For>
                </optgroup>
              </select>
            </div>

            {/* 파라미터 입력 */}
            <div class="mb-4 space-y-3">
              <For each={Object.entries(INDICATOR_META[newIndicatorType()].paramLabels)}>
                {([key, label]) => (
                  <div>
                    <label class="block text-xs text-[var(--color-text-muted)] mb-1">{label}</label>
                    <Show
                      when={key === 'periods'}
                      fallback={
                        <input
                          type="number"
                          value={(newIndicatorParams() as Record<string, number>)[key] || 0}
                          onInput={(e) => setNewIndicatorParams(prev => ({
                            ...prev,
                            [key]: parseInt(e.currentTarget.value) || 0
                          }))}
                          class="w-full px-3 py-2 text-sm bg-[var(--color-bg)] text-[var(--color-text)]
                                 rounded-lg border border-[var(--color-surface-light)]"
                        />
                      }
                    >
                      <input
                        type="text"
                        value={((newIndicatorParams() as Record<string, number[]>)[key] || []).join(', ')}
                        onInput={(e) => setNewIndicatorParams(prev => ({
                          ...prev,
                          [key]: e.currentTarget.value.split(',').map(v => parseInt(v.trim())).filter(v => !isNaN(v))
                        }))}
                        placeholder="5, 10, 20"
                        class="w-full px-3 py-2 text-sm bg-[var(--color-bg)] text-[var(--color-text)]
                               rounded-lg border border-[var(--color-surface-light)]"
                      />
                    </Show>
                  </div>
                )}
              </For>
              <Show when={Object.keys(INDICATOR_META[newIndicatorType()].paramLabels).length === 0}>
                <p class="text-xs text-[var(--color-text-muted)]">설정 가능한 파라미터가 없습니다.</p>
              </Show>
            </div>

            {/* 미리보기 */}
            <div class="mb-4 p-2 bg-[var(--color-bg)] rounded-lg">
              <p class="text-xs text-[var(--color-text-muted)]">미리보기</p>
              <div class="flex items-center gap-2 mt-1">
                <span
                  class="w-3 h-0.5 rounded"
                  style={{ 'background-color': INDICATOR_META[newIndicatorType()].color }}
                />
                <span class="text-sm text-[var(--color-text)]" style={{ color: INDICATOR_META[newIndicatorType()].color }}>
                  {INDICATOR_META[newIndicatorType()].name}
                  <Show when={Object.keys(newIndicatorParams()).length > 0}>
                    {' '}({Object.values(newIndicatorParams()).map(v => Array.isArray(v) ? v.join(', ') : v).join(', ')})
                  </Show>
                </span>
              </div>
            </div>

            {/* 버튼 */}
            <div class="flex gap-2">
              <button
                onClick={() => setShowIndicatorModal(false)}
                class="flex-1 px-3 py-2 text-sm bg-[var(--color-bg)] text-[var(--color-text-muted)]
                       rounded-lg hover:bg-[var(--color-surface-light)] transition"
              >
                취소
              </button>
              <button
                onClick={addIndicator}
                class="flex-1 px-3 py-2 text-sm bg-[var(--color-primary)] text-white
                       rounded-lg hover:bg-[var(--color-primary-dark)] transition"
              >
                추가
              </button>
            </div>
          </div>
        </div>
      </Show>

      {/* 차트 뷰 */}
      <Show when={viewMode() === 'chart'}>
        <div class="flex-1 min-h-0 overflow-auto">
          <Show
            when={!candlesQuery.isLoading && chartData().length > 0}
            fallback={
              <div class="h-full flex items-center justify-center text-[var(--color-text-muted)]">
                {candlesQuery.isLoading ? (
                  <div class="flex items-center gap-2">
                    <Loader2 class="w-5 h-5 animate-spin" />
                    <span class="text-sm">차트 로딩...</span>
                  </div>
                ) : '데이터 없음'}
              </div>
            }
          >
            {/* 동기화된 차트 패널 (메인 + 서브 지표) */}
            <SyncedChartPanel
              data={chartData()}
              type="candlestick"
              mainHeight={chartHeight()}
              subHeight={subChartHeight()}
              indicators={indicators()}
              subIndicators={subIndicators()}
            />
          </Show>
        </div>
      </Show>

      {/* 테이블 뷰 */}
      <Show when={viewMode() === 'table'}>
        <div class="flex-1 min-h-0 overflow-auto">
          <table class="w-full text-xs">
            <thead class="bg-[var(--color-bg)] sticky top-0 z-10">
              <tr class="text-[var(--color-text-muted)] text-left">
                <th class="px-2 py-1.5 cursor-pointer" onClick={() => handleColumnSort('time')}>
                  <div class="flex items-center gap-1">
                    시간
                    <Show when={sortColumn() === 'time'}>
                      {sortOrder() === 'desc' ? <ArrowDown class="w-3 h-3" /> : <ArrowUp class="w-3 h-3" />}
                    </Show>
                  </div>
                </th>
                <th class="px-2 py-1.5 text-right cursor-pointer" onClick={() => handleColumnSort('close')}>
                  <div class="flex items-center justify-end gap-1">
                    종가
                    <Show when={sortColumn() === 'close'}>
                      {sortOrder() === 'desc' ? <ArrowDown class="w-3 h-3" /> : <ArrowUp class="w-3 h-3" />}
                    </Show>
                  </div>
                </th>
                <th class="px-2 py-1.5 text-right cursor-pointer" onClick={() => handleColumnSort('change')}>
                  <div class="flex items-center justify-end gap-1">
                    변동
                    <Show when={sortColumn() === 'change'}>
                      {sortOrder() === 'desc' ? <ArrowDown class="w-3 h-3" /> : <ArrowUp class="w-3 h-3" />}
                    </Show>
                  </div>
                </th>
                <th class="px-2 py-1.5 text-right cursor-pointer" onClick={() => handleColumnSort('volume')}>
                  <div class="flex items-center justify-end gap-1">
                    거래량
                    <Show when={sortColumn() === 'volume'}>
                      {sortOrder() === 'desc' ? <ArrowDown class="w-3 h-3" /> : <ArrowUp class="w-3 h-3" />}
                    </Show>
                  </div>
                </th>
              </tr>
            </thead>
            <tbody class="divide-y divide-[var(--color-surface-light)]">
              <For each={tableData().slice(0, visibleRows())}>
                {(candle) => {
                  const open = parseFloat(candle.open)
                  const close = parseFloat(candle.close)
                  const changePct = ((close - open) / open * 100).toFixed(2)
                  const isUp = close >= open
                  // 타임프레임에 따라 시간 표시 형식 결정
                  const timeDisplay = isDailyOrHigher() ? candle.time.split(' ')[0] : candle.time
                  return (
                    <tr class="hover:bg-[var(--color-surface-light)]">
                      <td class="px-2 py-1 text-[var(--color-text)] font-mono">{timeDisplay}</td>
                      <td class={`px-2 py-1 text-right font-mono ${isUp ? 'text-green-400' : 'text-red-400'}`}>
                        {close.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
                      </td>
                      <td class={`px-2 py-1 text-right font-mono ${isUp ? 'text-green-400' : 'text-red-400'}`}>
                        {isUp ? '+' : ''}{changePct}%
                      </td>
                      <td class="px-2 py-1 text-right text-[var(--color-text-muted)] font-mono">
                        {parseInt(candle.volume).toLocaleString()}
                      </td>
                    </tr>
                  )
                }}
              </For>
            </tbody>
          </table>
          {/* 무한 스크롤 트리거 요소 */}
          <div
            ref={tableEndRef}
            class="h-4 flex items-center justify-center text-xs text-[var(--color-text-muted)]"
          >
            <Show when={visibleRows() < tableData().length}>
              <span class="opacity-50">스크롤하여 더 보기 ({visibleRows()}/{tableData().length})</span>
            </Show>
          </div>
        </div>
      </Show>
    </div>
    </Show>
  )
}

// ==================== 메인 컴포넌트 ====================

export function Dataset() {
  const toast = useToast()
  const queryClient = useQueryClient()

  // ==================== 상태 ====================
  // 뷰 모드: single (탭 방식) / multi (그리드 방식)
  const [viewType, setViewType] = createSignal<'single' | 'multi'>('multi')
  // 그리드 레이아웃
  const [layoutMode, setLayoutMode] = createSignal<LayoutMode>('2x2')

  // 패널 설정 (멀티 뷰용)
  const [panels, setPanels] = createSignal<PanelConfig[]>([])
  // 싱글 뷰용 상태
  const [activeSymbol, setActiveSymbol] = createSignal<string>('')
  const [activeTimeframe, setActiveTimeframe] = createSignal<string>('1d')

  // UI 상태
  const [showDownloadForm, setShowDownloadForm] = createSignal(false)
  const [downloadSymbol, setDownloadSymbol] = createSignal('')
  const [downloadTimeframe, setDownloadTimeframe] = createSignal('1d')
  const [downloadLimit, setDownloadLimit] = createSignal(500)
  // 날짜 범위 다운로드
  const [downloadStartDate, setDownloadStartDate] = createSignal('')
  const [downloadEndDate, setDownloadEndDate] = createSignal('')
  const [useDateRange, setUseDateRange] = createSignal(false)
  // 다운로드 폼 자동완성 상태
  const [showDownloadAutocomplete, setShowDownloadAutocomplete] = createSignal(false)
  const [downloadSelectedIndex, setDownloadSelectedIndex] = createSignal(-1)

  // ==================== 쿼리 ====================
  const datasetsQuery = createQuery(() => ({
    queryKey: ['datasets'],
    queryFn: fetchDatasets,
    refetchInterval: 30000,
  }))

  const strategiesQuery = createQuery(() => ({
    queryKey: ['strategies'],
    queryFn: fetchStrategies,
  }))

  // ==================== 뮤테이션 ====================
  const downloadMutation = createMutation(() => ({
    mutationFn: downloadDataset,
    onSuccess: (data, variables) => {
      toast.success('다운로드 완료', data.message)
      queryClient.invalidateQueries({ queryKey: ['datasets'] })
      queryClient.invalidateQueries({ queryKey: ['candles', variables.symbol] })
      setShowDownloadForm(false)
      setDownloadSymbol('')
      setDownloadStartDate('')
      setDownloadEndDate('')
      setUseDateRange(false)
    },
    onError: (error: Error) => {
      toast.error('다운로드 실패', error.message)
    },
  }))

  const deleteMutation = createMutation(() => ({
    mutationFn: (params: { symbol: string; timeframe?: string }) =>
      deleteDataset(params.symbol, params.timeframe),
    onSuccess: (_, variables) => {
      toast.success('삭제 완료', '데이터셋이 삭제되었습니다')
      queryClient.invalidateQueries({ queryKey: ['datasets'] })

      // 패널에서도 제거
      if (variables.timeframe) {
        setPanels(prev => prev.map(p =>
          p.symbol === variables.symbol && p.timeframe === variables.timeframe
            ? { ...p, symbol: undefined, timeframe: undefined }
            : p
        ))
      } else {
        setPanels(prev => prev.map(p =>
          p.symbol === variables.symbol
            ? { ...p, symbol: undefined, timeframe: undefined }
            : p
        ))
      }
    },
    onError: (error: Error) => {
      toast.error('삭제 실패', error.message)
    },
  }))

  // ==================== 계산된 값 ====================
  const cachedSymbols = createMemo(() => {
    const datasets = datasetsQuery.data?.datasets || []
    return [...new Set(datasets.map(d => d.symbol))].sort()
  })


  const strategySymbols = createMemo(() => {
    const strategies = strategiesQuery.data || []
    const symbolSet = new Set<string>()
    strategies.forEach(s => {
      if (s.symbols) s.symbols.forEach(sym => symbolSet.add(sym))
    })
    return Array.from(symbolSet)
  })

  const totalCandles = () => (datasetsQuery.data?.datasets || []).reduce((sum, d) => sum + d.candleCount, 0)

  // 다운로드 폼 자동완성 심볼 목록
  const downloadAutocompleteSymbols = createMemo(() => {
    const term = downloadSymbol().toUpperCase().trim()
    if (!term) return []
    // 캐시된 심볼 + 전략 심볼 합쳐서 검색
    const allSymbols = [...new Set([...cachedSymbols(), ...strategySymbols()])]
    return allSymbols
      .filter(s => s.toUpperCase().includes(term))
      .slice(0, 8)
  })

  // 다운로드 폼 심볼 선택 핸들러
  const handleDownloadSymbolSelect = (symbol: string) => {
    setDownloadSymbol(symbol)
    setShowDownloadAutocomplete(false)
    setDownloadSelectedIndex(-1)
  }

  // 다운로드 폼 키보드 핸들러
  const handleDownloadKeyDown = (e: KeyboardEvent) => {
    const symbols = downloadAutocompleteSymbols()
    const len = symbols.length

    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setDownloadSelectedIndex(prev => (prev + 1) % len)
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      setDownloadSelectedIndex(prev => (prev - 1 + len) % len)
    } else if (e.key === 'Enter' && downloadSelectedIndex() >= 0 && downloadSelectedIndex() < len) {
      e.preventDefault()
      handleDownloadSymbolSelect(symbols[downloadSelectedIndex()])
    } else if (e.key === 'Escape') {
      setShowDownloadAutocomplete(false)
      setDownloadSelectedIndex(-1)
    }
  }

  // ==================== 핸들러 ====================

  // 패널 심볼 변경
  const changePanelSymbol = (panelId: string, symbol: string) => {
    if (symbol) {
      setPanels(prev => prev.map(p =>
        p.id === panelId ? { ...p, symbol, timeframe: '1d' } : p
      ))
    } else {
      // 심볼 해제 (검색 모드로 전환)
      setPanels(prev => prev.map(p =>
        p.id === panelId ? { ...p, symbol: undefined } : p
      ))
    }
  }

  // 패널 닫기
  const closePanel = (panelId: string) => {
    setPanels(prev => prev.filter(p => p.id !== panelId))
  }

  // 패널 타임프레임 변경
  const changePanelTimeframe = (panelId: string, timeframe: string) => {
    setPanels(prev => prev.map(p =>
      p.id === panelId ? { ...p, timeframe } : p
    ))
  }

  // 빠른 다운로드
  const quickDownload = (symbol: string) => {
    downloadMutation.mutate({ symbol, timeframe: '1d', limit: 500 })
  }

  // 초기 패널 설정
  createEffect(() => {
    if (panels().length === 0 && viewType() === 'multi') {
      // 기본 4개 패널 생성
      setPanels([
        { id: 'panel-1' },
        { id: 'panel-2' },
        { id: 'panel-3' },
        { id: 'panel-4' },
      ])
    }
  })


  // ==================== 렌더링 ====================
  return (
    <div class="h-full flex flex-col">
      {/* 상단 바: 뷰 모드 + 액션 */}
      <div class="flex items-center justify-between gap-4 mb-4">
        <div class="flex items-center gap-3">
          <h1 class="text-lg font-semibold text-[var(--color-text)] flex items-center gap-2">
            <Database class="w-5 h-5" />
            데이터셋
          </h1>
          {/* 뷰 모드 토글 */}
          <div class="flex gap-1 bg-[var(--color-surface)] rounded-lg p-1">
            <button
              onClick={() => setViewType('single')}
              class={`px-3 py-1.5 text-sm rounded flex items-center gap-2 transition
                      ${viewType() === 'single'
                        ? 'bg-[var(--color-primary)] text-white'
                        : 'text-[var(--color-text-muted)] hover:bg-[var(--color-surface-light)]'}`}
            >
              <Square class="w-4 h-4" />
              싱글
            </button>
            <button
              onClick={() => setViewType('multi')}
              class={`px-3 py-1.5 text-sm rounded flex items-center gap-2 transition
                      ${viewType() === 'multi'
                        ? 'bg-[var(--color-primary)] text-white'
                        : 'text-[var(--color-text-muted)] hover:bg-[var(--color-surface-light)]'}`}
            >
              <Grid2x2 class="w-4 h-4" />
              멀티
            </button>
          </div>
        </div>

        {/* 액션 버튼 */}
        <div class="flex items-center gap-2">
          <button
            onClick={() => setShowDownloadForm(!showDownloadForm())}
            class="px-4 py-2 bg-[var(--color-primary)] text-white rounded-lg
                   hover:bg-[var(--color-primary-dark)] transition flex items-center gap-2"
          >
            <Download class="w-4 h-4" />
            다운로드
          </button>
          <button
            onClick={() => datasetsQuery.refetch()}
            class="px-4 py-2 bg-[var(--color-surface)] text-[var(--color-text)] rounded-lg
                   hover:bg-[var(--color-surface-light)] transition"
          >
            <RefreshCw class={`w-4 h-4 ${datasetsQuery.isFetching ? 'animate-spin' : ''}`} />
          </button>
        </div>
      </div>

      {/* 통계 카드 */}
      <div class="grid grid-cols-4 gap-4 mb-4">
        <div class="bg-[var(--color-surface)] rounded-xl p-4 flex items-center gap-3">
          <div class="p-2 bg-blue-500/20 rounded-lg">
            <Database class="w-5 h-5 text-blue-400" />
          </div>
          <div>
            <p class="text-sm text-[var(--color-text-muted)]">캐시 심볼</p>
            <p class="text-xl font-bold text-[var(--color-text)]">{cachedSymbols().length}</p>
          </div>
        </div>
        <div class="bg-[var(--color-surface)] rounded-xl p-4 flex items-center gap-3">
          <div class="p-2 bg-green-500/20 rounded-lg">
            <BarChart3 class="w-5 h-5 text-green-400" />
          </div>
          <div>
            <p class="text-sm text-[var(--color-text-muted)]">전체 캔들</p>
            <p class="text-xl font-bold text-[var(--color-text)]">{totalCandles().toLocaleString()}</p>
          </div>
        </div>
        <div class="bg-[var(--color-surface)] rounded-xl p-4 flex items-center gap-3">
          <div class="p-2 bg-purple-500/20 rounded-lg">
            <TrendingUp class="w-5 h-5 text-purple-400" />
          </div>
          <div>
            <p class="text-sm text-[var(--color-text-muted)]">전략 심볼</p>
            <p class="text-xl font-bold text-[var(--color-text)]">{strategySymbols().length}</p>
          </div>
        </div>
        <div class="bg-[var(--color-surface)] rounded-xl p-4 flex items-center gap-3">
          <div class="p-2 bg-amber-500/20 rounded-lg">
            <Grid2x2 class="w-5 h-5 text-amber-400" />
          </div>
          <div>
            <p class="text-sm text-[var(--color-text-muted)]">활성 패널</p>
            <p class="text-xl font-bold text-[var(--color-text)]">
              {viewType() === 'multi' ? panels().filter(p => p.symbol).length : (activeSymbol() ? 1 : 0)}
            </p>
          </div>
        </div>
      </div>

      {/* 다운로드 폼 */}
      <Show when={showDownloadForm()}>
        <div class="bg-[var(--color-surface)] rounded-xl p-6 mb-4">
          <h2 class="text-lg font-semibold text-[var(--color-text)] mb-4 flex items-center gap-2">
            <Download class="w-5 h-5" />
            데이터 다운로드
          </h2>
          <Show when={strategySymbols().length > 0}>
            <div class="mb-4">
              <label class="block text-sm text-[var(--color-text-muted)] mb-2">
                전략 심볼 (클릭하여 빠른 다운로드)
              </label>
              <div class="flex flex-wrap gap-2">
                <For each={strategySymbols()}>
                  {(symbol) => (
                    <button
                      onClick={() => quickDownload(symbol)}
                      disabled={downloadMutation.isPending}
                      class="px-3 py-1.5 bg-[var(--color-primary)]/20 text-[var(--color-primary)]
                             rounded-lg hover:bg-[var(--color-primary)]/30 transition
                             flex items-center gap-1.5 text-sm"
                    >
                      <Zap class="w-3.5 h-3.5" />
                      {symbol}
                    </button>
                  )}
                </For>
              </div>
            </div>
          </Show>
          <div class="grid grid-cols-4 gap-4">
            <div class="relative">
              <label class="block text-sm text-[var(--color-text-muted)] mb-2">심볼</label>
              <div class="relative">
                <Search class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-[var(--color-text-muted)]" />
                <input
                  type="text"
                  value={downloadSymbol()}
                  onInput={(e) => {
                    setDownloadSymbol(e.currentTarget.value)
                    setShowDownloadAutocomplete(true)
                    setDownloadSelectedIndex(-1)
                  }}
                  onFocus={() => setShowDownloadAutocomplete(true)}
                  onBlur={() => setTimeout(() => setShowDownloadAutocomplete(false), 200)}
                  onKeyDown={handleDownloadKeyDown}
                  placeholder="심볼 검색..."
                  class="w-full pl-9 pr-4 py-2 bg-[var(--color-bg)] text-[var(--color-text)]
                         rounded-lg border border-[var(--color-surface-light)]
                         focus:outline-none focus:border-[var(--color-primary)]"
                />

                {/* 자동완성 드롭다운 */}
                <Show when={showDownloadAutocomplete() && downloadSymbol().trim() && downloadAutocompleteSymbols().length > 0}>
                  <div class="absolute top-full left-0 right-0 mt-1 bg-[var(--color-surface)]
                              border border-[var(--color-surface-light)] rounded-lg shadow-xl z-50
                              max-h-48 overflow-auto">
                    <For each={downloadAutocompleteSymbols()}>
                      {(symbol, index) => (
                        <button
                          onMouseDown={(e) => {
                            e.preventDefault()
                            handleDownloadSymbolSelect(symbol)
                          }}
                          class={`w-full px-3 py-2 text-left text-sm font-mono flex items-center gap-2
                                  transition hover:bg-[var(--color-surface-light)]
                                  ${index() === downloadSelectedIndex()
                                    ? 'bg-[var(--color-primary)]/20 text-[var(--color-primary)]'
                                    : 'text-[var(--color-text)]'}`}
                        >
                          <TrendingUp class="w-3.5 h-3.5 text-[var(--color-primary)]" />
                          <span>{symbol}</span>
                          <Show when={cachedSymbols().includes(symbol)}>
                            <span class="ml-auto text-xs text-green-400">✓</span>
                          </Show>
                        </button>
                      )}
                    </For>
                  </div>
                </Show>
              </div>
            </div>
            <div>
              <label class="block text-sm text-[var(--color-text-muted)] mb-2">타임프레임</label>
              <select
                value={downloadTimeframe()}
                onChange={(e) => setDownloadTimeframe(e.currentTarget.value)}
                style={{ "background-color": "#1a1a2e" }}
                class="w-full px-4 py-2 text-[var(--color-text)] rounded-lg border border-[var(--color-surface-light)]"
              >
                <option value="1m">1분</option>
                <option value="5m">5분</option>
                <option value="15m">15분</option>
                <option value="1h">1시간</option>
                <option value="1d">1일</option>
              </select>
            </div>
            <div>
              <label class="block text-sm text-[var(--color-text-muted)] mb-2">
                <span class="flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={useDateRange()}
                    onChange={(e) => setUseDateRange(e.currentTarget.checked)}
                    class="rounded border-[var(--color-surface-light)]"
                  />
                  날짜 범위 지정
                </span>
              </label>
              <Show when={useDateRange()} fallback={
                <input
                  type="number"
                  value={downloadLimit()}
                  onInput={(e) => setDownloadLimit(parseInt(e.currentTarget.value) || 100)}
                  min="10"
                  max="5000"
                  class="w-full px-4 py-2 bg-[var(--color-bg)] text-[var(--color-text)]
                         rounded-lg border border-[var(--color-surface-light)]"
                  placeholder="캔들 수"
                />
              }>
                <div class="flex gap-2">
                  <input
                    type="date"
                    value={downloadStartDate()}
                    onInput={(e) => setDownloadStartDate(e.currentTarget.value)}
                    class="flex-1 px-3 py-2 bg-[var(--color-bg)] text-[var(--color-text)]
                           rounded-lg border border-[var(--color-surface-light)] text-sm"
                    placeholder="시작일"
                  />
                  <span class="text-[var(--color-text-muted)] self-center">~</span>
                  <input
                    type="date"
                    value={downloadEndDate()}
                    onInput={(e) => setDownloadEndDate(e.currentTarget.value)}
                    class="flex-1 px-3 py-2 bg-[var(--color-bg)] text-[var(--color-text)]
                           rounded-lg border border-[var(--color-surface-light)] text-sm"
                    placeholder="종료일"
                  />
                </div>
              </Show>
            </div>
            <div class="flex items-end">
              <button
                onClick={() => downloadMutation.mutate({
                  symbol: downloadSymbol(),
                  timeframe: downloadTimeframe(),
                  limit: downloadLimit(),
                  ...(useDateRange() && downloadStartDate() ? { startDate: downloadStartDate() } : {}),
                  ...(useDateRange() && downloadEndDate() ? { endDate: downloadEndDate() } : {}),
                })}
                disabled={downloadMutation.isPending || !downloadSymbol() || (useDateRange() && !downloadStartDate())}
                class="w-full px-4 py-2 bg-[var(--color-primary)] text-white rounded-lg
                       hover:bg-[var(--color-primary-dark)] transition disabled:opacity-50
                       flex items-center justify-center gap-2"
              >
                <Show when={downloadMutation.isPending} fallback={<Download class="w-4 h-4" />}>
                  <RefreshCw class="w-4 h-4 animate-spin" />
                </Show>
                다운로드
              </button>
            </div>
          </div>
        </div>
      </Show>

      {/* 메인 컨텐츠 */}
      <div class="flex-1 min-h-0">
        <Show when={viewType() === 'multi'}>
          {/* 멀티 패널 뷰 */}
          <MultiPanelGrid
            panels={panels()}
            layoutMode={layoutMode()}
            onLayoutChange={setLayoutMode}
            onPanelClose={closePanel}
            availableSymbols={[...new Set([...cachedSymbols(), ...strategySymbols()])]}
            onSymbolChange={(panelId, symbol) => changePanelSymbol(panelId, symbol)}
            onSymbolSearch={async (query) => {
              const results = await searchSymbols(query, 10)
              return results.map(r => ({
                ticker: r.ticker,
                name: r.name,
                market: r.market
              }))
            }}
            renderPanel={(panel) => (
              <SymbolPanel
                symbol={panel.symbol}
                timeframe={panel.timeframe || '1d'}
                datasets={datasetsQuery.data?.datasets || []}
                cachedSymbols={cachedSymbols()}
                onSymbolChange={(symbol) => changePanelSymbol(panel.id, symbol)}
                onTimeframeChange={(tf) => changePanelTimeframe(panel.id, tf)}
                onRefresh={() => {
                  if (panel.symbol) {
                    downloadMutation.mutate({
                      symbol: panel.symbol,
                      timeframe: panel.timeframe || '1d',
                      limit: 500,
                    })
                  }
                }}
                onDelete={() => {
                  if (panel.symbol) {
                    deleteMutation.mutate({
                      symbol: panel.symbol,
                      timeframe: panel.timeframe,
                    })
                  }
                }}
                isRefreshing={downloadMutation.isPending}
                compact={layoutMode() !== '1x1'}
              />
            )}
          />
        </Show>

        <Show when={viewType() === 'single'}>
          {/* 싱글 뷰 */}
          <div class="h-full bg-[var(--color-surface)] rounded-xl p-4">
            <SymbolPanel
              symbol={activeSymbol() || undefined}
              timeframe={activeTimeframe()}
              datasets={datasetsQuery.data?.datasets || []}
              cachedSymbols={cachedSymbols()}
              onSymbolChange={(symbol) => {
                setActiveSymbol(symbol)
                setActiveTimeframe('1d')
              }}
              onTimeframeChange={setActiveTimeframe}
              onRefresh={() => {
                if (activeSymbol()) {
                  downloadMutation.mutate({
                    symbol: activeSymbol(),
                    timeframe: activeTimeframe(),
                    limit: 500,
                  })
                }
              }}
              onDelete={() => {
                if (activeSymbol()) {
                  deleteMutation.mutate({
                    symbol: activeSymbol(),
                    timeframe: activeTimeframe(),
                  })
                }
              }}
              isRefreshing={downloadMutation.isPending}
            />
          </div>
        </Show>
      </div>
    </div>
  )
}
