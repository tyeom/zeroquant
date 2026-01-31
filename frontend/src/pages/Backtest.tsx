import { createSignal, createResource, createEffect, For, Show, createMemo } from 'solid-js'
import { useSearchParams } from '@solidjs/router'
import { Play, Calendar, TrendingUp, TrendingDown, ChartBar, Settings2, RefreshCw, AlertCircle, Info, X, ChevronDown, ChevronUp, LineChart } from 'lucide-solid'
import { EquityCurve, DrawdownChart, SyncedChartPanel } from '../components/charts'
import type { EquityDataPoint, DrawdownDataPoint, ChartSyncState, CandlestickDataPoint, TradeMarker } from '../components/charts'
import {
  runBacktest,
  runMultiBacktest,
  getStrategies,
  listBacktestResults,
  saveBacktestResult,
  deleteBacktestResult,
  MULTI_ASSET_STRATEGIES,
  type BacktestRequest,
  type BacktestMultiRequest,
  type BacktestResult,
  type BacktestMultiResult,
} from '../api/client'
import type { Strategy } from '../types'
import { SymbolDisplay } from '../components/SymbolDisplay'

function formatCurrency(value: string | number): string {
  const num = typeof value === 'string' ? parseFloat(value) : value
  return new Intl.NumberFormat('ko-KR', {
    style: 'currency',
    currency: 'KRW',
    maximumFractionDigits: 0,
  }).format(num)
}

function formatPercent(value: string | number): string {
  const num = typeof value === 'string' ? parseFloat(value) : value
  const sign = num >= 0 ? '+' : ''
  return `${sign}${num.toFixed(2)}%`
}

// API 기본 URL
const API_BASE = '/api/v1'

// 캔들 데이터 타입
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

// 날짜 간 일수 계산
function daysBetween(startDate: string, endDate: string): number {
  const start = new Date(startDate)
  const end = new Date(endDate)
  const diffTime = Math.abs(end.getTime() - start.getTime())
  return Math.ceil(diffTime / (1000 * 60 * 60 * 24)) + 1 // +1 for inclusive
}

// 캔들 데이터 조회 (백테스트 기간에 해당하는 데이터)
async function fetchCandlesForBacktest(
  symbol: string,
  startDate: string,
  endDate: string
): Promise<CandleDataResponse | null> {
  try {
    // 실제 기간만큼 요청 (여유 있게 20% 추가)
    const days = daysBetween(startDate, endDate)
    const limit = Math.ceil(days * 1.2)

    const params = new URLSearchParams({
      timeframe: '1d',
      limit: limit.toString(),
      sortBy: 'time',
      sortOrder: 'asc',
    })
    const res = await fetch(`${API_BASE}/dataset/${encodeURIComponent(symbol)}?${params}`)
    if (!res.ok) return null
    const data: CandleDataResponse = await res.json()

    // 백테스트 기간에 해당하는 캔들만 필터링
    const filtered = data.candles.filter(c => {
      const date = c.time.split(' ')[0]
      return date >= startDate && date <= endDate
    })

    return { ...data, candles: filtered, totalCount: filtered.length }
  } catch {
    return null
  }
}

// 캔들 데이터를 차트용 형식으로 변환
function convertCandlesToChartData(candles: CandleItem[]): CandlestickDataPoint[] {
  const uniqueMap = new Map<string, CandlestickDataPoint>()

  candles.forEach(c => {
    const timeKey = c.time.split(' ')[0] // 일봉 기준 "YYYY-MM-DD"
    uniqueMap.set(timeKey, {
      time: timeKey,
      open: parseFloat(c.open),
      high: parseFloat(c.high),
      low: parseFloat(c.low),
      close: parseFloat(c.close),
    })
  })

  return Array.from(uniqueMap.values()).sort((a, b) =>
    (a.time as string).localeCompare(b.time as string)
  )
}

// 백테스트 거래 내역을 차트 마커로 변환
function convertTradesToMarkers(trades: BacktestResult['trades']): TradeMarker[] {
  const markers: TradeMarker[] = []

  for (const trade of trades) {
    // entry_time 또는 entry_date 사용 (API 응답 필드명에 따라)
    const entryTime = trade.entry_time || trade.entry_date
    if (!entryTime) continue

    // 진입 마커 (ISO 날짜 형식에서 날짜만 추출)
    const entryDateStr = entryTime.split('T')[0]
    markers.push({
      time: entryDateStr,
      type: trade.side === 'Buy' ? 'buy' : 'sell',
      price: parseFloat(trade.entry_price),
      label: trade.side === 'Buy' ? '매수' : '매도',
    })

    // 청산 마커
    const exitTime = trade.exit_time || trade.exit_date
    if (exitTime) {
      const exitDateStr = exitTime.split('T')[0]
      markers.push({
        time: exitDateStr,
        type: trade.side === 'Buy' ? 'sell' : 'buy', // 청산은 반대 방향
        price: parseFloat(trade.exit_price),
        label: '청산',
      })
    }
  }

  // 시간순 정렬
  return markers.sort((a, b) =>
    (a.time as string).localeCompare(b.time as string)
  )
}

// 자산 곡선 데이터를 차트 컴포넌트 형식으로 변환 (시간순 정렬 + 중복 제거 필수)
function convertEquityCurve(result: BacktestResult): EquityDataPoint[] {
  const sorted = result.equity_curve
    .slice()
    .sort((a, b) => a.timestamp - b.timestamp)
    .map(point => ({
      time: new Date(point.timestamp * 1000).toISOString().split('T')[0],
      value: parseFloat(point.equity),
    }))

  // 중복 타임스탬프 제거 (같은 날짜면 마지막 값 유지)
  const uniqueMap = new Map<string, EquityDataPoint>()
  for (const point of sorted) {
    uniqueMap.set(point.time, point)
  }
  const result_array = Array.from(uniqueMap.values())

  // 디버깅: 급격한 변동 감지 (10% 이상 변동시 로그)
  let hasAnomalies = false
  for (let i = 1; i < result_array.length; i++) {
    const prev = result_array[i - 1]
    const curr = result_array[i]
    const changePercent = ((curr.value - prev.value) / prev.value) * 100
    if (Math.abs(changePercent) > 10) {
      hasAnomalies = true
      console.warn(
        `[EquityCurve] 급격한 변동 감지: ${prev.time} -> ${curr.time}, ` +
        `${prev.value.toLocaleString()} -> ${curr.value.toLocaleString()} (${changePercent.toFixed(2)}%)`
      )
      // 해당 날짜 주변의 원본 데이터 출력
      const anomalyDate = curr.time
      const rawDataAroundAnomaly = sorted.filter(p =>
        p.time >= prev.time && p.time <= curr.time
      )
      console.log(`[EquityCurve] ${anomalyDate} 주변 원본 데이터:`, rawDataAroundAnomaly)
    }
  }

  if (hasAnomalies) {
    console.log('[EquityCurve] 전체 원본 포인트 수:', result.equity_curve.length)
    console.log('[EquityCurve] 중복 제거 후 포인트 수:', result_array.length)
    console.log('[EquityCurve] 같은 날짜에 여러 포인트가 있는 경우가 있는지 확인하세요.')
  }

  return result_array
}

// 드로우다운 곡선 데이터를 차트 컴포넌트 형식으로 변환 (시간순 정렬 + 중복 제거 필수)
function convertDrawdownCurve(result: BacktestResult): DrawdownDataPoint[] {
  const sorted = result.equity_curve
    .slice()
    .sort((a, b) => a.timestamp - b.timestamp)
    .map(point => ({
      time: new Date(point.timestamp * 1000).toISOString().split('T')[0],
      value: parseFloat(point.drawdown_pct),
    }))

  // 중복 타임스탬프 제거 (같은 날짜면 마지막 값 유지)
  const uniqueMap = new Map<string, DrawdownDataPoint>()
  for (const point of sorted) {
    uniqueMap.set(point.time, point)
  }
  return Array.from(uniqueMap.values())
}

// 백테스트 결과 카드 컴포넌트 (차트 동기화를 위해 분리)
interface BacktestResultCardProps {
  result: BacktestResult
  strategies: Strategy[] | undefined
  index: number
  onDelete: (index: number) => void | Promise<void>
}

function BacktestResultCard(props: BacktestResultCardProps) {
  // 카드 확장 상태
  const [isExpanded, setIsExpanded] = createSignal(false)
  // 차트 동기화 state
  const [chartSyncState, setChartSyncState] = createSignal<ChartSyncState | null>(null)

  // 다중 심볼 목록 파싱
  const symbols = createMemo(() =>
    props.result.symbol.split(',').map(s => s.trim()).filter(s => s)
  )
  // 선택된 심볼 (기본값: 첫 번째)
  const [selectedSymbol, setSelectedSymbol] = createSignal<string>('')

  // 선택된 심볼 초기화
  createEffect(() => {
    const syms = symbols()
    if (syms.length > 0 && !selectedSymbol()) {
      setSelectedSymbol(syms[0])
    }
  })

  // 가격 차트 데이터 (심볼별로 캐시)
  const [candleDataCache, setCandleDataCache] = createSignal<Record<string, CandlestickDataPoint[]>>({})
  const [isLoadingCandles, setIsLoadingCandles] = createSignal(false)
  const [showPriceChart, setShowPriceChart] = createSignal(false)

  // 현재 선택된 심볼의 캔들 데이터
  const candleData = createMemo(() => candleDataCache()[selectedSymbol()] || [])

  // 매매 마커 (선택된 심볼만 필터링)
  const tradeMarkers = createMemo(() => {
    const selected = selectedSymbol()
    // 심볼 필터링: 심볼명이 정확히 일치하거나 base 부분이 일치
    const filteredTrades = props.result.trades.filter(t => {
      const tradeSymbol = t.symbol.split('/')[0] // "122630/KRW" → "122630"
      return tradeSymbol === selected || t.symbol === selected
    })
    return convertTradesToMarkers(filteredTrades)
  })

  // 가격 차트 데이터 로드 (선택된 심볼)
  const loadCandleData = async (symbol?: string) => {
    const targetSymbol = symbol || selectedSymbol()
    if (!targetSymbol) return

    // 이미 캐시에 있으면 스킵
    if (candleDataCache()[targetSymbol]?.length > 0) return
    if (isLoadingCandles()) return

    setIsLoadingCandles(true)
    try {
      const data = await fetchCandlesForBacktest(
        targetSymbol,
        props.result.start_date,
        props.result.end_date
      )
      if (data) {
        setCandleDataCache(prev => ({
          ...prev,
          [targetSymbol]: convertCandlesToChartData(data.candles)
        }))
      }
    } catch (err) {
      console.error('캔들 데이터 로드 실패:', err)
    } finally {
      setIsLoadingCandles(false)
    }
  }

  // 심볼 선택 핸들러
  const handleSymbolSelect = (symbol: string) => {
    setSelectedSymbol(symbol)
    // 선택된 심볼의 캔들 데이터 로드
    if (showPriceChart()) {
      loadCandleData(symbol)
    }
  }

  const equityCurve = () => convertEquityCurve(props.result)
  const drawdownCurve = () => convertDrawdownCurve(props.result)

  // 최종 자본 = equity_curve의 마지막 값 (미실현 손익 포함)
  // 차트와 지표가 일관되게 표시되도록 함
  const initialCapitalNum = () => parseFloat(props.result.config_summary.initial_capital)
  const finalCapital = () => {
    const curve = equityCurve()
    if (curve.length > 0) {
      return curve[curve.length - 1].value
    }
    // equity_curve가 없으면 기존 방식 사용
    return initialCapitalNum() + parseFloat(props.result.metrics.net_profit)
  }

  // 총 수익률도 equity_curve 기준으로 재계산
  const totalReturnPct = () => {
    const initial = initialCapitalNum()
    const final = finalCapital()
    if (initial <= 0) return parseFloat(props.result.metrics.total_return_pct)
    return ((final - initial) / initial) * 100
  }

  const handleVisibleRangeChange = (state: ChartSyncState) => {
    setChartSyncState(state)
  }

  const handleDelete = (e: MouseEvent) => {
    e.stopPropagation()
    props.onDelete(props.index)
  }

  const toggleExpand = () => {
    setIsExpanded(!isExpanded())
  }

  return (
    <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] overflow-hidden transition-all duration-200">
      {/* 클릭 가능한 헤더 */}
      <div
        class="p-6 cursor-pointer hover:bg-[var(--color-surface-light)]/30 transition-colors"
        onClick={toggleExpand}
      >
        <div class="flex items-start justify-between">
          <div class="flex items-center gap-3">
            {/* 펼침 아이콘 */}
            <div class="text-[var(--color-text-muted)]">
              <Show when={isExpanded()} fallback={<ChevronDown class="w-5 h-5" />}>
                <ChevronUp class="w-5 h-5" />
              </Show>
            </div>
            <div>
              <h4 class="text-lg font-semibold text-[var(--color-text)]">
                {props.strategies?.find((s: Strategy) => s.id === props.result.strategy_id)?.name || props.result.strategy_id}
              </h4>
              <div class="flex items-center gap-3 mt-1 text-sm text-[var(--color-text-muted)]">
                <div class="flex flex-wrap gap-1">
                  <For each={props.result.symbol.split(',').map(s => s.trim()).filter(s => s)}>
                    {(symbol) => (
                      <SymbolDisplay
                        ticker={symbol}
                        mode="inline"
                        size="sm"
                        autoFetch={true}
                      />
                    )}
                  </For>
                </div>
                <span class="flex items-center gap-1">
                  <Calendar class="w-4 h-4" />
                  {props.result.start_date} ~ {props.result.end_date}
                </span>
              </div>
            </div>
          </div>
          <div class="flex items-center gap-2">
            {/* 요약 수익률 (접혀있을 때도 보임) */}
            <div
              class={`px-3 py-1 rounded-lg text-sm font-semibold ${
                totalReturnPct() >= 0 ? 'bg-green-500/20 text-green-400' : 'bg-red-500/20 text-red-400'
              }`}
            >
              {formatPercent(totalReturnPct())}
            </div>
            <span
              class={`px-3 py-1 rounded-full text-sm font-medium ${
                props.result.success
                  ? 'bg-green-500/20 text-green-400'
                  : 'bg-red-500/20 text-red-400'
              }`}
            >
              {props.result.success ? '완료' : '실패'}
            </span>
            {/* 삭제 버튼 */}
            <button
              onClick={handleDelete}
              class="p-1.5 rounded-lg hover:bg-red-500/20 text-[var(--color-text-muted)] hover:text-red-400 transition-colors"
              title="결과 삭제"
            >
              <X class="w-4 h-4" />
            </button>
          </div>
        </div>

        {/* 접혀있을 때도 보이는 핵심 지표 요약 */}
        <Show when={!isExpanded()}>
          <div class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-8 gap-3 mt-4 ml-8 text-sm">
            <div>
              <span class="text-[var(--color-text-muted)]">초기자본</span>
              <div class="font-medium text-[var(--color-text)]">{formatCurrency(props.result.config_summary.initial_capital)}</div>
            </div>
            <div>
              <span class="text-[var(--color-text-muted)]">최종자본</span>
              <div class="font-medium text-[var(--color-text)]">{formatCurrency(finalCapital())}</div>
            </div>
            <div>
              <span class="text-[var(--color-text-muted)]">수익률</span>
              <div class={`font-medium ${totalReturnPct() >= 0 ? 'text-green-400' : 'text-red-400'}`}>{formatPercent(totalReturnPct())}</div>
            </div>
            <div>
              <span class="text-[var(--color-text-muted)]">CAGR</span>
              <div class={`font-medium ${parseFloat(props.result.metrics.annualized_return_pct) >= 0 ? 'text-green-400' : 'text-red-400'}`}>{formatPercent(props.result.metrics.annualized_return_pct)}</div>
            </div>
            <div>
              <span class="text-[var(--color-text-muted)]">MDD</span>
              <div class="font-medium text-red-400">{parseFloat(props.result.metrics.max_drawdown_pct).toFixed(2)}%</div>
            </div>
            <div>
              <span class="text-[var(--color-text-muted)]">샤프</span>
              <div class="font-medium text-[var(--color-text)]">{parseFloat(props.result.metrics.sharpe_ratio).toFixed(2)}</div>
            </div>
            <div>
              <span class="text-[var(--color-text-muted)]">승률</span>
              <div class="font-medium text-[var(--color-text)]">{parseFloat(props.result.metrics.win_rate_pct).toFixed(1)}%</div>
            </div>
            <div>
              <span class="text-[var(--color-text-muted)]">거래</span>
              <div class="font-medium text-[var(--color-text)]">{props.result.metrics.total_trades}회</div>
            </div>
          </div>
        </Show>
      </div>

      {/* 펼쳐진 상세 내용 */}
      <Show when={isExpanded()}>
        <div class="px-6 pb-6 border-t border-[var(--color-surface-light)]">
          {/* 성과 지표 그리드 */}
          <div class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-8 gap-4 pt-4">
            <div>
              <div class="text-sm text-[var(--color-text-muted)] mb-1">초기 자본</div>
              <div class="font-semibold text-[var(--color-text)]">
                {formatCurrency(props.result.config_summary.initial_capital)}
              </div>
            </div>
            <div>
              <div class="text-sm text-[var(--color-text-muted)] mb-1">최종 자본</div>
              <div class="font-semibold text-[var(--color-text)]">
                {formatCurrency(finalCapital())}
              </div>
            </div>
            <div>
              <div class="text-sm text-[var(--color-text-muted)] mb-1">총 수익률</div>
              <div
                class={`font-semibold flex items-center gap-1 ${
                  totalReturnPct() >= 0 ? 'text-green-500' : 'text-red-500'
                }`}
              >
                <Show
                  when={totalReturnPct() >= 0}
                  fallback={<TrendingDown class="w-4 h-4" />}
                >
                  <TrendingUp class="w-4 h-4" />
                </Show>
                {formatPercent(totalReturnPct())}
              </div>
            </div>
            <div>
              <div class="text-sm text-[var(--color-text-muted)] mb-1">CAGR</div>
              <div
                class={`font-semibold ${
                  parseFloat(props.result.metrics.annualized_return_pct) >= 0 ? 'text-green-500' : 'text-red-500'
                }`}
              >
                {formatPercent(props.result.metrics.annualized_return_pct)}
              </div>
            </div>
            <div>
              <div class="text-sm text-[var(--color-text-muted)] mb-1">MDD</div>
              <div class="font-semibold text-red-500">
                {parseFloat(props.result.metrics.max_drawdown_pct).toFixed(2)}%
              </div>
            </div>
            <div>
              <div class="text-sm text-[var(--color-text-muted)] mb-1">샤프 비율</div>
              <div class="font-semibold text-[var(--color-text)]">
                {parseFloat(props.result.metrics.sharpe_ratio).toFixed(2)}
              </div>
            </div>
            <div>
              <div class="text-sm text-[var(--color-text-muted)] mb-1">승률</div>
              <div class="font-semibold text-[var(--color-text)]">
                {parseFloat(props.result.metrics.win_rate_pct).toFixed(1)}%
              </div>
            </div>
            <div>
              <div class="text-sm text-[var(--color-text-muted)] mb-1">총 거래</div>
              <div class="font-semibold text-[var(--color-text)]">
                {props.result.metrics.total_trades}회
              </div>
            </div>
          </div>

          {/* 추가 성과 지표 (접이식) */}
          <details class="mt-4">
            <summary class="cursor-pointer text-sm text-[var(--color-text-muted)] hover:text-[var(--color-text)]">
              상세 지표 보기
            </summary>
            <div class="grid grid-cols-2 md:grid-cols-4 gap-4 mt-3 pt-3 border-t border-[var(--color-surface-light)]">
              <div>
                <div class="text-sm text-[var(--color-text-muted)] mb-1">순수익</div>
                <div class={`font-semibold ${parseFloat(props.result.metrics.net_profit) >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                  {formatCurrency(props.result.metrics.net_profit)}
                </div>
              </div>
              <div>
                <div class="text-sm text-[var(--color-text-muted)] mb-1">Profit Factor</div>
                <div class="font-semibold text-[var(--color-text)]">
                  {parseFloat(props.result.metrics.profit_factor).toFixed(2)}
                </div>
              </div>
              <div>
                <div class="text-sm text-[var(--color-text-muted)] mb-1">소르티노 비율</div>
                <div class="font-semibold text-[var(--color-text)]">
                  {parseFloat(props.result.metrics.sortino_ratio).toFixed(2)}
                </div>
              </div>
              <div>
                <div class="text-sm text-[var(--color-text-muted)] mb-1">칼마 비율</div>
                <div class="font-semibold text-[var(--color-text)]">
                  {parseFloat(props.result.metrics.calmar_ratio).toFixed(2)}
                </div>
              </div>
              <div>
                <div class="text-sm text-[var(--color-text-muted)] mb-1">평균 수익</div>
                <div class="font-semibold text-green-500">
                  {formatCurrency(props.result.metrics.avg_win)}
                </div>
              </div>
              <div>
                <div class="text-sm text-[var(--color-text-muted)] mb-1">평균 손실</div>
                <div class="font-semibold text-red-500">
                  {formatCurrency(props.result.metrics.avg_loss)}
                </div>
              </div>
              <div>
                <div class="text-sm text-[var(--color-text-muted)] mb-1">총 수수료</div>
                <div class="font-semibold text-[var(--color-text)]">
                  {formatCurrency(props.result.config_summary.total_commission)}
                </div>
              </div>
              <div>
                <div class="text-sm text-[var(--color-text-muted)] mb-1">데이터 포인트</div>
                <div class="font-semibold text-[var(--color-text)]">
                  {props.result.config_summary.data_points}개
                </div>
              </div>
            </div>
          </details>

          {/* 가격 차트 + 매매 태그 (접이식) */}
          <Show when={props.result.trades.length > 0}>
            <details
              class="mt-4"
              onToggle={(e) => {
                if ((e.target as HTMLDetailsElement).open) {
                  setShowPriceChart(true)
                  loadCandleData()
                }
              }}
            >
              <summary class="cursor-pointer text-sm text-[var(--color-text-muted)] hover:text-[var(--color-text)] flex items-center gap-2">
                <LineChart class="w-4 h-4" />
                가격 차트 + 매매 태그
              </summary>
              <div class="mt-3">
                {/* 다중 심볼인 경우 심볼 선택 탭 표시 */}
                <Show when={symbols().length > 1}>
                  <div class="flex flex-wrap gap-1 mb-3 p-1 bg-[var(--color-surface-light)]/30 rounded-lg">
                    <For each={symbols()}>
                      {(symbol) => (
                        <button
                          class={`px-3 py-1.5 text-xs font-medium rounded-md transition-all ${
                            selectedSymbol() === symbol
                              ? 'bg-[var(--color-primary)] text-white shadow-sm'
                              : 'text-[var(--color-text-muted)] hover:bg-[var(--color-surface-light)] hover:text-[var(--color-text)]'
                          }`}
                          onClick={(e) => {
                            e.stopPropagation()
                            handleSymbolSelect(symbol)
                          }}
                        >
                          {symbol}
                        </button>
                      )}
                    </For>
                  </div>
                </Show>
                <Show
                  when={!isLoadingCandles() && candleData().length > 0}
                  fallback={
                    <div class="h-[280px] flex items-center justify-center text-[var(--color-text-muted)]">
                      {isLoadingCandles() ? (
                        <div class="flex items-center gap-2">
                          <RefreshCw class="w-5 h-5 animate-spin" />
                          <span>차트 데이터 로딩 중...</span>
                        </div>
                      ) : (
                        <span>차트 데이터가 없습니다 (데이터셋을 먼저 다운로드하세요)</span>
                      )}
                    </div>
                  }
                >
                  <SyncedChartPanel
                    data={candleData()}
                    type="candlestick"
                    mainHeight={240}
                    markers={tradeMarkers()}
                  />
                </Show>
              </div>
            </details>
          </Show>

          {/* 자산 곡선 & 드로우다운 차트 (동기화됨) */}
          <Show when={equityCurve().length > 0}>
            <div class="mt-4 space-y-4">
              <div>
                <h5 class="text-sm font-medium text-[var(--color-text-muted)] mb-2">자산 곡선</h5>
                <EquityCurve
                  data={equityCurve()}
                  height={200}
                  chartId="equity"
                  syncState={chartSyncState}
                  onVisibleRangeChange={handleVisibleRangeChange}
                />
              </div>
              <Show when={drawdownCurve().length > 0}>
                <div>
                  <h5 class="text-sm font-medium text-[var(--color-text-muted)] mb-2">드로우다운</h5>
                  <DrawdownChart
                    data={drawdownCurve()}
                    height={150}
                    chartId="drawdown"
                    syncState={chartSyncState}
                    onVisibleRangeChange={handleVisibleRangeChange}
                  />
                </div>
              </Show>
            </div>
          </Show>

          {/* 거래 내역 (접이식) */}
          <Show when={props.result.trades.length > 0}>
            <details class="mt-4">
              <summary class="cursor-pointer text-sm text-[var(--color-text-muted)] hover:text-[var(--color-text)]">
                거래 내역 ({props.result.trades.length}건)
              </summary>
              <div class="mt-3 overflow-x-auto">
                <table class="w-full text-sm">
                  <thead>
                    <tr class="text-left text-[var(--color-text-muted)] border-b border-[var(--color-surface-light)]">
                      <th class="pb-2">심볼</th>
                      <th class="pb-2">방향</th>
                      <th class="pb-2">진입가</th>
                      <th class="pb-2">청산가</th>
                      <th class="pb-2">수량</th>
                      <th class="pb-2">손익</th>
                      <th class="pb-2">수익률</th>
                    </tr>
                  </thead>
                  <tbody>
                    <For each={props.result.trades.slice(0, 20)}>
                      {(trade) => (
                        <tr class="border-b border-[var(--color-surface-light)]">
                          <td class="py-2">
                            <SymbolDisplay
                              ticker={trade.symbol}
                              mode="inline"
                              size="sm"
                              autoFetch={true}
                            />
                          </td>
                          <td class={`py-2 ${trade.side === 'Buy' ? 'text-green-500' : 'text-red-500'}`}>
                            {trade.side === 'Buy' ? '매수' : '매도'}
                          </td>
                          <td class="py-2">{formatCurrency(trade.entry_price)}</td>
                          <td class="py-2">{formatCurrency(trade.exit_price)}</td>
                          <td class="py-2">{parseFloat(trade.quantity).toFixed(4)}</td>
                          <td class={`py-2 ${parseFloat(trade.pnl) >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                            {formatCurrency(trade.pnl)}
                          </td>
                          <td class={`py-2 ${parseFloat(trade.return_pct) >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                            {formatPercent(trade.return_pct)}
                          </td>
                        </tr>
                      )}
                    </For>
                  </tbody>
                </table>
                <Show when={props.result.trades.length > 20}>
                  <p class="mt-2 text-xs text-[var(--color-text-muted)]">
                    {props.result.trades.length - 20}건 더 있음
                  </p>
                </Show>
              </div>
            </details>
          </Show>
        </div>
      </Show>
    </div>
  )
}

// 저장된 결과와 DB ID를 매핑하기 위한 인터페이스
interface StoredBacktestResult extends BacktestResult {
  dbId?: string  // DB에 저장된 ID
}

export function Backtest() {
  // 등록된 전략 목록 가져오기 (전략 페이지에서 등록된 전략만 표시)
  const [strategies] = createResource(async () => {
    return await getStrategies()
  })

  // 저장된 백테스트 결과 불러오기
  const [savedResults, { refetch: refetchSavedResults }] = createResource(async () => {
    try {
      const response = await listBacktestResults({ limit: 100 })
      // DB 결과를 StoredBacktestResult 형태로 변환
      return response.results.map(r => ({
        ...r,
        dbId: r.id
      })) as StoredBacktestResult[]
    } catch (err) {
      console.error('저장된 결과 불러오기 실패:', err)
      return []
    }
  })

  // 백테스트 결과 목록 (저장된 결과 + 새로 실행한 결과)
  const [results, setResults] = createSignal<StoredBacktestResult[]>([])

  // 저장된 결과가 로드되면 상태에 반영
  createEffect(() => {
    const saved = savedResults()
    if (saved && saved.length > 0) {
      setResults(saved)
    }
  })

  // URL 파라미터 읽기 (전략 페이지에서 바로 이동 시)
  const [searchParams] = useSearchParams()

  // 기본 날짜 설정 (1년 전 ~ 오늘)
  const today = new Date().toISOString().split('T')[0]
  const oneYearAgo = new Date(Date.now() - 365 * 24 * 60 * 60 * 1000).toISOString().split('T')[0]

  // 폼 상태 (날짜는 기본값 설정)
  const [selectedStrategy, setSelectedStrategy] = createSignal('')

  // URL에서 전략 ID가 있으면 자동 선택
  createEffect(() => {
    const strategyId = searchParams.strategy
    if (strategyId && strategies() && strategies()!.length > 0) {
      const found = strategies()!.find(s => s.id === strategyId)
      if (found) {
        setSelectedStrategy(strategyId)
      }
    }
  })
  // 심볼은 전략에서 가져옴 (signal 제거됨)
  const [startDate, setStartDate] = createSignal(oneYearAgo)
  const [endDate, setEndDate] = createSignal(today)
  const [initialCapital, setInitialCapital] = createSignal('10000000')
  const [slippageRate, setSlippageRate] = createSignal('0.05') // 기본 0.05%
  const [isRunning, setIsRunning] = createSignal(false)
  const [error, setError] = createSignal<string | null>(null)

  const handleRunBacktest = async (e: Event) => {
    e.preventDefault()
    setError(null)

    const strategyType = getSelectedStrategyType()

    const strategyInfo = getSelectedStrategyInfo()
    if (!selectedStrategy() || !strategyType || !strategyInfo) {
      setError('전략을 선택해주세요')
      return
    }
    if (!strategyInfo.symbols || strategyInfo.symbols.length === 0) {
      setError('전략에 심볼이 등록되어 있지 않습니다')
      return
    }
    if (!startDate()) {
      setError('시작일을 선택해주세요')
      return
    }
    if (!endDate()) {
      setError('종료일을 선택해주세요')
      return
    }

    setIsRunning(true)

    try {
      // 다중 자산 전략인지 확인 (strategyType 기준)
      const isMultiAssetStrategy = MULTI_ASSET_STRATEGIES.includes(strategyType)

      // 전략에 등록된 심볼 사용
      const symbols = strategyInfo.symbols

      // 슬리피지를 소수점으로 변환 (0.05% → 0.0005)
      const slippage = parseFloat(slippageRate()) / 100

      let resultToSave: BacktestResult
      let symbolStr: string

      if (isMultiAssetStrategy) {
        const request: BacktestMultiRequest = {
          strategy_id: strategyType,
          symbols,
          start_date: startDate(),
          end_date: endDate(),
          initial_capital: parseInt(initialCapital(), 10),
          slippage_rate: slippage,
        }

        const result = await runMultiBacktest(request)

        // 다중 자산 결과를 단일 자산 형식으로 변환하여 표시
        symbolStr = result.symbols.join(', ')
        resultToSave = {
          ...result,
          symbol: symbolStr,
        }
      } else {
        // 단일 자산 전략 (첫 번째 심볼 사용)
        const request: BacktestRequest = {
          strategy_id: strategyType,
          symbol: symbols[0],
          start_date: startDate(),
          end_date: endDate(),
          initial_capital: parseInt(initialCapital(), 10),
          slippage_rate: slippage,
        }

        resultToSave = await runBacktest(request)
        symbolStr = symbols[0]
      }

      // DB에 결과 저장
      try {
        const saveResponse = await saveBacktestResult({
          strategy_id: selectedStrategy(),  // 등록된 전략 ID
          strategy_type: strategyType,      // 전략 타입 (sma_crossover, bollinger 등)
          symbol: symbolStr,
          start_date: startDate(),
          end_date: endDate(),
          initial_capital: parseInt(initialCapital(), 10),
          slippage_rate: slippage,
          metrics: resultToSave.metrics,
          config_summary: resultToSave.config_summary,
          equity_curve: resultToSave.equity_curve,
          trades: resultToSave.trades,
          success: resultToSave.success,
        })

        // DB ID를 결과에 추가하여 저장
        const storedResult: StoredBacktestResult = {
          ...resultToSave,
          dbId: saveResponse.id,
        }
        setResults(prev => [storedResult, ...prev])
        console.log('백테스트 결과 저장됨:', saveResponse.id)
      } catch (saveErr) {
        console.error('백테스트 결과 저장 실패:', saveErr)
        // 저장 실패해도 결과는 표시 (dbId 없이)
        setResults(prev => [resultToSave, ...prev])
      }
    } catch (err) {
      console.error('백테스트 실행 실패:', err)
      setError(err instanceof Error ? err.message : '백테스트 실행에 실패했습니다')
    } finally {
      setIsRunning(false)
    }
  }

  // 전략 선택
  const handleStrategyChange = (strategyId: string) => {
    setSelectedStrategy(strategyId)
  }

  // 선택된 전략 정보 가져오기
  const getSelectedStrategyInfo = (): Strategy | undefined => {
    return strategies()?.find((s: Strategy) => s.id === selectedStrategy())
  }

  // 선택된 전략의 strategyType 가져오기 (백테스트 API에서 사용)
  const getSelectedStrategyType = (): string | undefined => {
    return getSelectedStrategyInfo()?.strategyType
  }

  return (
    <div class="space-y-6">
      {/* 백테스트 설정 */}
      <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-6">
        <h3 class="text-lg font-semibold text-[var(--color-text)] mb-4 flex items-center gap-2">
          <Settings2 class="w-5 h-5" />
          백테스트 설정
        </h3>

        {/* 에러 메시지 */}
        <Show when={error()}>
          <div class="mb-4 p-3 rounded-lg bg-red-500/20 border border-red-500/30 text-red-400 flex items-center gap-2">
            <AlertCircle class="w-5 h-5 flex-shrink-0" />
            <span>{error()}</span>
          </div>
        </Show>

        {/* 등록된 전략 없음 안내 */}
        <Show when={!strategies.loading && (!strategies() || strategies()!.length === 0)}>
          <div class="mb-4 p-4 rounded-lg bg-blue-500/20 border border-blue-500/30 text-blue-300 flex items-start gap-3">
            <Info class="w-5 h-5 flex-shrink-0 mt-0.5" />
            <div>
              <p class="font-medium">등록된 전략이 없습니다</p>
              <p class="text-sm mt-1 text-blue-300/80">
                백테스트를 실행하려면 먼저 전략 페이지에서 전략을 등록하세요.
                등록된 전략의 파라미터로 백테스트가 실행됩니다.
              </p>
            </div>
          </div>
        </Show>

        <form onSubmit={handleRunBacktest} class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {/* 전략 선택 */}
          <div>
            <label class="block text-sm text-[var(--color-text-muted)] mb-1">전략</label>
            <Show
              when={!strategies.loading}
              fallback={
                <div class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] text-[var(--color-text-muted)]">
                  로딩 중...
                </div>
              }
            >
              <select
                value={selectedStrategy()}
                onChange={(e) => handleStrategyChange(e.currentTarget.value)}
                class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
              >
                <option value="">전략 선택</option>
                <For each={strategies()}>
                  {(strategy: Strategy) => (
                    <option value={strategy.id}>
                      {strategy.name} ({strategy.strategyType})
                    </option>
                  )}
                </For>
              </select>
            </Show>
            <Show when={getSelectedStrategyInfo()}>
              <p class="mt-1 text-xs text-[var(--color-text-muted)]">
                전략 타입: {getSelectedStrategyInfo()?.strategyType}
              </p>
            </Show>
          </div>

          {/* 심볼 (읽기 전용 - 전략에서 가져옴) */}
          <div>
            <label class="block text-sm text-[var(--color-text-muted)] mb-1">심볼</label>
            <div class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)]/50 border border-[var(--color-surface-light)] text-[var(--color-text-muted)]">
              <Show
                when={getSelectedStrategyInfo()?.symbols && getSelectedStrategyInfo()!.symbols.length > 0}
                fallback={<span class="text-[var(--color-text-muted)]/50">전략을 선택하세요</span>}
              >
                <div class="flex flex-wrap gap-1">
                  <For each={getSelectedStrategyInfo()?.symbols}>
                    {(symbol) => (
                      <SymbolDisplay
                        ticker={symbol}
                        mode="inline"
                        size="sm"
                        autoFetch={true}
                      />
                    )}
                  </For>
                </div>
              </Show>
            </div>
          </div>

          {/* 초기 자본 */}
          <div>
            <label class="block text-sm text-[var(--color-text-muted)] mb-1">초기 자본 (KRW)</label>
            <input
              type="number"
              value={initialCapital()}
              onInput={(e) => setInitialCapital(e.currentTarget.value)}
              class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
              placeholder="10000000"
              min="100000"
            />
          </div>

          {/* 슬리피지 */}
          <div>
            <label class="block text-sm text-[var(--color-text-muted)] mb-1">슬리피지 (%)</label>
            <input
              type="number"
              value={slippageRate()}
              onInput={(e) => setSlippageRate(e.currentTarget.value)}
              class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
              placeholder="0.05"
              min="0"
              max="5"
              step="0.01"
            />
            <p class="mt-1 text-xs text-[var(--color-text-muted)]">
              거래 시 예상 체결가와의 차이
            </p>
          </div>

          {/* 시작일 */}
          <div>
            <label class="block text-sm text-[var(--color-text-muted)] mb-1">시작일</label>
            <input
              type="date"
              value={startDate() || oneYearAgo}
              onInput={(e) => setStartDate(e.currentTarget.value)}
              class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
            />
          </div>

          {/* 종료일 */}
          <div>
            <label class="block text-sm text-[var(--color-text-muted)] mb-1">종료일</label>
            <input
              type="date"
              value={endDate() || today}
              onInput={(e) => setEndDate(e.currentTarget.value)}
              class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
            />
          </div>

          {/* 실행 버튼 */}
          <div class="flex items-end">
            <button
              type="submit"
              disabled={isRunning()}
              class="w-full px-4 py-2 bg-[var(--color-primary)] text-white rounded-lg font-medium hover:bg-[var(--color-primary)]/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
            >
              <Show when={isRunning()} fallback={<Play class="w-5 h-5" />}>
                <div class="w-5 h-5 border-2 border-white border-t-transparent rounded-full animate-spin" />
              </Show>
              {isRunning() ? '실행 중...' : '백테스트 실행'}
            </button>
          </div>
        </form>
      </div>

      {/* 결과 */}
      <div class="space-y-4">
        <div class="flex items-center justify-between">
          <h3 class="text-lg font-semibold text-[var(--color-text)] flex items-center gap-2">
            <ChartBar class="w-5 h-5" />
            백테스트 결과
          </h3>
          <Show when={results().length > 0}>
            <button
              onClick={async () => {
                // DB에서 모든 결과 삭제
                const currentResults = results()
                for (const result of currentResults) {
                  if (result.id) {
                    try {
                      await deleteBacktestResult(result.id)
                    } catch (e) {
                      console.error('결과 삭제 실패:', result.id, e)
                    }
                  }
                }
                // 로컬 상태 초기화
                setResults([])
              }}
              class="text-sm text-[var(--color-text-muted)] hover:text-[var(--color-text)] flex items-center gap-1"
            >
              <RefreshCw class="w-4 h-4" />
              결과 초기화
            </button>
          </Show>
        </div>

        <Show
          when={results().length > 0}
          fallback={
            <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-8 text-center text-[var(--color-text-muted)]">
              <ChartBar class="w-12 h-12 mx-auto mb-3 opacity-50" />
              <p>아직 백테스트 결과가 없습니다</p>
              <p class="text-sm mt-1">위에서 전략과 설정을 선택하고 백테스트를 실행해주세요</p>
            </div>
          }
        >
          <div class="grid grid-cols-1 gap-4">
            <For each={results()}>
              {(result, index) => (
                <BacktestResultCard
                  result={result}
                  strategies={strategies()}
                  index={index()}
                  onDelete={async (idx) => {
                    const target = results()[idx] as StoredBacktestResult
                    // DB에 저장된 결과라면 API 호출하여 삭제
                    if (target.dbId) {
                      try {
                        await deleteBacktestResult(target.dbId)
                        console.log('백테스트 결과 삭제됨:', target.dbId)
                      } catch (err) {
                        console.error('백테스트 결과 삭제 실패:', err)
                        // 삭제 실패해도 UI에서는 제거
                      }
                    }
                    setResults(prev => prev.filter((_, i) => i !== idx))
                  }}
                />
              )}
            </For>
          </div>
        </Show>
      </div>
    </div>
  )
}
