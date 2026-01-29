import { createSignal, createResource, For, Show } from 'solid-js'
import { Play, Calendar, TrendingUp, TrendingDown, ChartBar, Settings2, RefreshCw, AlertCircle } from 'lucide-solid'
import { EquityCurve, DrawdownChart } from '../components/charts'
import type { EquityDataPoint, DrawdownDataPoint, ChartSyncState } from '../components/charts'
import {
  runBacktest,
  runMultiBacktest,
  getBacktestStrategies,
  MULTI_ASSET_STRATEGIES,
  type BacktestRequest,
  type BacktestMultiRequest,
  type BacktestResult,
  type BacktestMultiResult,
  type BacktestStrategy,
} from '../api/client'

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
  strategies: BacktestStrategy[] | undefined
}

function BacktestResultCard(props: BacktestResultCardProps) {
  // 차트 동기화 state
  const [chartSyncState, setChartSyncState] = createSignal<ChartSyncState | null>(null)

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

  return (
    <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-6">
      {/* 헤더 */}
      <div class="flex items-start justify-between mb-4">
        <div>
          <h4 class="text-lg font-semibold text-[var(--color-text)]">
            {props.strategies?.find((s: BacktestStrategy) => s.id === props.result.strategy_id)?.name || props.result.strategy_id}
          </h4>
          <div class="flex items-center gap-3 mt-1 text-sm text-[var(--color-text-muted)]">
            <span>{props.result.symbol}</span>
            <span class="flex items-center gap-1">
              <Calendar class="w-4 h-4" />
              {props.result.start_date} ~ {props.result.end_date}
            </span>
          </div>
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
      </div>

      {/* 성과 지표 그리드 */}
      <div class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-8 gap-4">
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
                      <td class="py-2">{trade.symbol}</td>
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
  )
}

export function Backtest() {
  // 전략 목록 가져오기
  const [strategies] = createResource(async () => {
    const response = await getBacktestStrategies()
    return response.strategies
  })

  // 백테스트 결과 목록 (로컬 상태로 관리)
  const [results, setResults] = createSignal<BacktestResult[]>([])

  // 기본 날짜 설정 (1년 전 ~ 오늘)
  const today = new Date().toISOString().split('T')[0]
  const oneYearAgo = new Date(Date.now() - 365 * 24 * 60 * 60 * 1000).toISOString().split('T')[0]

  // 폼 상태 (날짜는 기본값 설정)
  const [selectedStrategy, setSelectedStrategy] = createSignal('')
  const [symbol, setSymbol] = createSignal('')
  const [startDate, setStartDate] = createSignal(oneYearAgo)
  const [endDate, setEndDate] = createSignal(today)
  const [initialCapital, setInitialCapital] = createSignal('10000000')
  const [isRunning, setIsRunning] = createSignal(false)
  const [error, setError] = createSignal<string | null>(null)

  const handleRunBacktest = async (e: Event) => {
    e.preventDefault()
    setError(null)

    if (!selectedStrategy()) {
      setError('전략을 선택해주세요')
      return
    }
    if (!symbol()) {
      setError('심볼을 입력해주세요')
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
      // 다중 자산 전략인지 확인
      const isMultiAssetStrategy = MULTI_ASSET_STRATEGIES.includes(selectedStrategy())

      if (isMultiAssetStrategy) {
        // 심볼을 콤마로 분리하여 배열로 변환
        const symbols = symbol()
          .split(',')
          .map(s => s.trim())
          .filter(s => s.length > 0)

        if (symbols.length === 0) {
          setError('최소 하나의 심볼을 입력해주세요')
          setIsRunning(false)
          return
        }

        const request: BacktestMultiRequest = {
          strategy_id: selectedStrategy(),
          symbols,
          start_date: startDate(),
          end_date: endDate(),
          initial_capital: parseInt(initialCapital(), 10),
        }

        const result = await runMultiBacktest(request)

        // 다중 자산 결과를 단일 자산 형식으로 변환하여 표시
        const convertedResult: BacktestResult = {
          ...result,
          symbol: result.symbols.join(', '),
        }

        setResults(prev => [convertedResult, ...prev])
      } else {
        // 단일 자산 전략
        const request: BacktestRequest = {
          strategy_id: selectedStrategy(),
          symbol: symbol(),
          start_date: startDate(),
          end_date: endDate(),
          initial_capital: parseInt(initialCapital(), 10),
        }

        const result = await runBacktest(request)
        setResults(prev => [result, ...prev])
      }
    } catch (err) {
      console.error('백테스트 실행 실패:', err)
      setError(err instanceof Error ? err.message : '백테스트 실행에 실패했습니다')
    } finally {
      setIsRunning(false)
    }
  }

  // 전략 선택 시 기본 심볼 설정
  const handleStrategyChange = (strategyId: string) => {
    setSelectedStrategy(strategyId)
    const strategy = strategies()?.find((s: BacktestStrategy) => s.id === strategyId)
    if (strategy && strategy.supported_symbols.length > 0) {
      setSymbol(strategy.supported_symbols[0])
    }
  }

  // 선택된 전략 정보 가져오기
  const getSelectedStrategyInfo = (): BacktestStrategy | undefined => {
    return strategies()?.find((s: BacktestStrategy) => s.id === selectedStrategy())
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
                  {(strategy: BacktestStrategy) => (
                    <option value={strategy.id}>{strategy.name}</option>
                  )}
                </For>
              </select>
            </Show>
            <Show when={getSelectedStrategyInfo()}>
              <p class="mt-1 text-xs text-[var(--color-text-muted)]">
                {getSelectedStrategyInfo()?.description}
              </p>
            </Show>
          </div>

          {/* 심볼 */}
          <div>
            <label class="block text-sm text-[var(--color-text-muted)] mb-1">심볼</label>
            <input
              type="text"
              value={symbol()}
              onInput={(e) => setSymbol(e.currentTarget.value)}
              class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
              placeholder="BTC/USDT, 005930, SPY 등"
            />
            <Show when={getSelectedStrategyInfo()}>
              <p class="mt-1 text-xs text-[var(--color-text-muted)]">
                지원: {getSelectedStrategyInfo()?.supported_symbols.join(', ')}
              </p>
            </Show>
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
              onClick={() => setResults([])}
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
              {(result) => (
                <BacktestResultCard result={result} strategies={strategies()} />
              )}
            </For>
          </div>
        </Show>
      </div>
    </div>
  )
}
