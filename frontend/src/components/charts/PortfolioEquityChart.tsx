import { createSignal, createResource, Show, For } from 'solid-js'
import { TrendingUp, TrendingDown, Calendar, Database, TestTube, RefreshCw, Download, X, CheckCircle, AlertCircle } from 'lucide-solid'
import { EquityCurve } from './EquityCurve'
import type { EquityDataPoint } from './EquityCurve'
import { getEquityCurve, getPerformance, getBacktestResults, listCredentials, syncEquityCurve } from '../../api/client'
import type { EquityCurveResponse, PerformanceResponse, BacktestResult, SyncEquityCurveRequest } from '../../api/client'
import type { ExchangeCredential } from '../../types'

// 기간 옵션
const PERIOD_OPTIONS = [
  { value: '1w', label: '1주' },
  { value: '1m', label: '1개월' },
  { value: '3m', label: '3개월' },
  { value: '6m', label: '6개월' },
  { value: '1y', label: '1년' },
  { value: 'all', label: '전체' },
]

// 데이터 소스 타입
type DataSource = 'portfolio' | 'backtest'

interface PortfolioEquityChartProps {
  height?: number
  showControls?: boolean
  defaultPeriod?: string
  defaultSource?: DataSource
  backtestId?: string
}

function formatCurrency(value: number | string): string {
  const numValue = typeof value === 'string' ? parseFloat(value) : value
  if (isNaN(numValue)) return '₩0'
  return new Intl.NumberFormat('ko-KR', {
    style: 'currency',
    currency: 'KRW',
    maximumFractionDigits: 0,
  }).format(numValue)
}

function formatPercent(value: number | string): string {
  const numValue = typeof value === 'string' ? parseFloat(value) : value
  if (isNaN(numValue)) return '0.00%'
  const sign = numValue >= 0 ? '+' : ''
  return `${sign}${numValue.toFixed(2)}%`
}

export function PortfolioEquityChart(props: PortfolioEquityChartProps) {
  const [period, setPeriod] = createSignal(props.defaultPeriod || '3m')
  const [dataSource, setDataSource] = createSignal<DataSource>(props.defaultSource || 'portfolio')
  const [selectedBacktestId, setSelectedBacktestId] = createSignal(props.backtestId || '')

  // 동기화 관련 상태
  const [showSyncModal, setShowSyncModal] = createSignal(false)
  const [syncCredentialId, setSyncCredentialId] = createSignal('')
  const getDefaultStartDate = () => {
    const d = new Date()
    d.setMonth(d.getMonth() - 3)
    return d.toISOString().slice(0, 10).replace(/-/g, '')
  }
  const getDefaultEndDate = () => {
    return new Date().toISOString().slice(0, 10).replace(/-/g, '')
  }
  const [syncStartDate, setSyncStartDate] = createSignal(getDefaultStartDate())
  const [syncEndDate, setSyncEndDate] = createSignal(getDefaultEndDate())
  const [syncLoading, setSyncLoading] = createSignal(false)
  const [syncResult, setSyncResult] = createSignal<{ success: boolean; message: string } | null>(null)

  // 포트폴리오 데이터 조회
  const [equityCurveData, { refetch: refetchEquity }] = createResource(
    () => ({ period: period(), source: dataSource() }),
    async ({ period, source }) => {
      if (source !== 'portfolio') return null
      try {
        return await getEquityCurve(period)
      } catch (e) {
        console.error('Equity curve fetch error:', e)
        return null
      }
    }
  )

  // 성과 지표 조회
  const [performanceData, { refetch: refetchPerformance }] = createResource(
    () => ({ period: period(), source: dataSource() }),
    async ({ period, source }) => {
      if (source !== 'portfolio') return null
      try {
        return await getPerformance(period)
      } catch (e) {
        console.error('Performance fetch error:', e)
        return null
      }
    }
  )

  // 백테스트 결과 목록
  const [backtestResults] = createResource(async () => {
    try {
      return await getBacktestResults()
    } catch {
      return []
    }
  })

  // 자격증명 목록 (KIS만 필터링)
  const [credentials] = createResource(async () => {
    try {
      const res = await listCredentials()
      // KIS 계정만 필터링 (자산 곡선 동기화 지원)
      return res.credentials.filter((c: ExchangeCredential) => c.exchange_id === 'kis')
    } catch {
      return []
    }
  })

  // 동기화 핸들러
  const handleSync = async () => {
    const credId = syncCredentialId()
    if (!credId) {
      setSyncResult({ success: false, message: '자격증명을 선택해주세요.' })
      return
    }

    setSyncLoading(true)
    setSyncResult(null)

    try {
      const request: SyncEquityCurveRequest = {
        credential_id: credId,
        start_date: syncStartDate(),
        end_date: syncEndDate(),
      }
      const result = await syncEquityCurve(request)
      setSyncResult({
        success: result.success,
        message: result.success
          ? `${result.synced_count}일 데이터 동기화 완료 (체결 ${result.execution_count}건)`
          : result.message,
      })
      if (result.success) {
        // 동기화 성공 시 차트 데이터 새로고침
        refetchEquity()
        refetchPerformance()
      }
    } catch (e: unknown) {
      const errorMessage = e instanceof Error ? e.message : '동기화 중 오류가 발생했습니다.'
      setSyncResult({ success: false, message: errorMessage })
    } finally {
      setSyncLoading(false)
    }
  }

  // 동기화 모달 열기
  const openSyncModal = () => {
    setSyncResult(null)
    // 첫 번째 KIS 자격증명 자동 선택
    const creds = credentials()
    if (creds && creds.length > 0 && !syncCredentialId()) {
      setSyncCredentialId(creds[0].id)
    }
    setShowSyncModal(true)
  }

  // 선택된 백테스트 데이터
  const selectedBacktest = () => {
    if (dataSource() !== 'backtest') return null
    const results = backtestResults() || []
    const id = selectedBacktestId()
    return results.find((r: BacktestResult) => r.id === id) || results[0]
  }

  // EquityCurve 컴포넌트용 데이터 변환
  const chartData = (): EquityDataPoint[] => {
    if (dataSource() === 'portfolio') {
      const data = equityCurveData()
      if (!data?.data) return []
      return data.data.map((point) => ({
        time: point.x, // timestamp in milliseconds
        value: parseFloat(point.y),
      }))
    } else {
      const backtest = selectedBacktest()
      if (!backtest?.equity_curve) return []
      return backtest.equity_curve.map((point) => ({
        time: point.timestamp,
        value: parseFloat(point.equity),
      }))
    }
  }

  // 현재 성과 지표
  const metrics = () => {
    if (dataSource() === 'portfolio') {
      const perf = performanceData()
      if (!perf) return null
      return {
        currentEquity: parseFloat(perf.current_equity || perf.currentEquity || '0'),
        totalReturn: parseFloat(perf.total_return_pct || perf.totalReturnPct || '0'),
        maxDrawdown: parseFloat(perf.max_drawdown_pct || perf.maxDrawdownPct || '0'),
        cagr: parseFloat(perf.cagr_pct || perf.cagrPct || '0'),
      }
    } else {
      const backtest = selectedBacktest()
      if (!backtest?.metrics) return null
      return {
        currentEquity: parseFloat(backtest.equity_curve?.[backtest.equity_curve.length - 1]?.equity || '0'),
        totalReturn: parseFloat(backtest.metrics.total_return_pct),
        maxDrawdown: parseFloat(backtest.metrics.max_drawdown_pct),
        cagr: parseFloat(backtest.metrics.annualized_return_pct),
      }
    }
  }

  const handleRefresh = () => {
    if (dataSource() === 'portfolio') {
      refetchEquity()
      refetchPerformance()
    }
  }

  const isLoading = () =>
    (dataSource() === 'portfolio' && (equityCurveData.loading || performanceData.loading)) ||
    (dataSource() === 'backtest' && backtestResults.loading)

  return (
    <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)]">
      {/* Header with Controls */}
      <div class="p-4 border-b border-[var(--color-surface-light)]">
        <div class="flex items-center justify-between flex-wrap gap-3">
          <div class="flex items-center gap-3">
            <h3 class="text-lg font-semibold text-[var(--color-text)]">자산 곡선</h3>

            {/* Data Source Selector */}
            <Show when={props.showControls !== false}>
              <div class="flex items-center gap-1 bg-[var(--color-surface-light)] rounded-lg p-0.5">
                <button
                  onClick={() => setDataSource('portfolio')}
                  class={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm transition-colors ${
                    dataSource() === 'portfolio'
                      ? 'bg-[var(--color-primary)] text-white'
                      : 'text-[var(--color-text-muted)] hover:text-[var(--color-text)]'
                  }`}
                >
                  <Database class="w-3.5 h-3.5" />
                  실제 포트폴리오
                </button>
                <button
                  onClick={() => setDataSource('backtest')}
                  class={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm transition-colors ${
                    dataSource() === 'backtest'
                      ? 'bg-[var(--color-primary)] text-white'
                      : 'text-[var(--color-text-muted)] hover:text-[var(--color-text)]'
                  }`}
                >
                  <TestTube class="w-3.5 h-3.5" />
                  백테스트
                </button>
              </div>
            </Show>
          </div>

          <div class="flex items-center gap-2">
            {/* Backtest Selector (when backtest source is selected) */}
            <Show when={dataSource() === 'backtest' && (backtestResults()?.length || 0) > 0}>
              <select
                value={selectedBacktestId()}
                onChange={(e) => setSelectedBacktestId(e.currentTarget.value)}
                class="bg-[var(--color-surface-light)] text-[var(--color-text)] border border-[var(--color-surface-light)] rounded-lg px-3 py-1.5 text-sm"
              >
                <For each={backtestResults()}>
                  {(result: BacktestResult) => (
                    <option value={result.id}>
                      {result.strategy_id} - {result.symbol} ({result.start_date} ~ {result.end_date})
                    </option>
                  )}
                </For>
              </select>
            </Show>

            {/* Period Selector (for portfolio data) */}
            <Show when={dataSource() === 'portfolio' && props.showControls !== false}>
              <div class="flex items-center gap-1">
                <Calendar class="w-4 h-4 text-[var(--color-text-muted)]" />
                <For each={PERIOD_OPTIONS}>
                  {(option) => (
                    <button
                      onClick={() => setPeriod(option.value)}
                      class={`px-2 py-1 rounded text-sm transition-colors ${
                        period() === option.value
                          ? 'bg-[var(--color-primary)] text-white'
                          : 'text-[var(--color-text-muted)] hover:text-[var(--color-text)] hover:bg-[var(--color-surface-light)]'
                      }`}
                    >
                      {option.label}
                    </button>
                  )}
                </For>
              </div>
            </Show>

            {/* Sync & Refresh Buttons */}
            <Show when={dataSource() === 'portfolio'}>
              <div class="flex items-center gap-1">
                {/* Sync Button */}
                <button
                  onClick={openSyncModal}
                  disabled={isLoading() || !credentials() || credentials()!.length === 0}
                  class="p-1.5 rounded-lg hover:bg-[var(--color-surface-light)] text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors disabled:opacity-50"
                  title="거래소 데이터 동기화"
                >
                  <Download class="w-4 h-4" />
                </button>
                {/* Refresh Button */}
                <button
                  onClick={handleRefresh}
                  disabled={isLoading()}
                  class="p-1.5 rounded-lg hover:bg-[var(--color-surface-light)] text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors disabled:opacity-50"
                  title="새로고침"
                >
                  <RefreshCw class={`w-4 h-4 ${isLoading() ? 'animate-spin' : ''}`} />
                </button>
              </div>
            </Show>
          </div>
        </div>

        {/* Metrics Summary */}
        <Show when={metrics()}>
          <div class="grid grid-cols-4 gap-4 mt-4">
            <div class="text-center">
              <div class="text-xs text-[var(--color-text-muted)] mb-1">현재 자산</div>
              <div class="text-lg font-semibold text-[var(--color-text)]">
                {formatCurrency(metrics()!.currentEquity)}
              </div>
            </div>
            <div class="text-center">
              <div class="text-xs text-[var(--color-text-muted)] mb-1">총 수익률</div>
              <div class={`text-lg font-semibold flex items-center justify-center gap-1 ${
                metrics()!.totalReturn >= 0 ? 'text-green-500' : 'text-red-500'
              }`}>
                {metrics()!.totalReturn >= 0 ? (
                  <TrendingUp class="w-4 h-4" />
                ) : (
                  <TrendingDown class="w-4 h-4" />
                )}
                {formatPercent(metrics()!.totalReturn)}
              </div>
            </div>
            <div class="text-center">
              <div class="text-xs text-[var(--color-text-muted)] mb-1">MDD</div>
              <div class="text-lg font-semibold text-red-500">
                -{Math.abs(metrics()!.maxDrawdown).toFixed(2)}%
              </div>
            </div>
            <div class="text-center">
              <div class="text-xs text-[var(--color-text-muted)] mb-1">CAGR</div>
              <div class={`text-lg font-semibold ${
                metrics()!.cagr >= 0 ? 'text-green-500' : 'text-red-500'
              }`}>
                {formatPercent(metrics()!.cagr)}
              </div>
            </div>
          </div>
        </Show>
      </div>

      {/* Chart Area */}
      <div class="p-4">
        <Show
          when={!isLoading() && chartData().length > 0}
          fallback={
            <div class="flex items-center justify-center" style={{ height: `${props.height || 300}px` }}>
              <Show
                when={isLoading()}
                fallback={
                  <div class="text-center text-[var(--color-text-muted)]">
                    <Show
                      when={dataSource() === 'portfolio'}
                      fallback={
                        <div>
                          <TestTube class="w-12 h-12 mx-auto mb-3 opacity-50" />
                          <p>백테스트 결과가 없습니다.</p>
                          <a href="/backtest" class="text-[var(--color-primary)] hover:underline">
                            백테스트 실행하기
                          </a>
                        </div>
                      }
                    >
                      <Database class="w-12 h-12 mx-auto mb-3 opacity-50" />
                      <p>아직 포트폴리오 데이터가 없습니다.</p>
                      <p class="text-sm mt-1">포트폴리오 조회 시 데이터가 자동으로 축적됩니다.</p>
                    </Show>
                  </div>
                }
              >
                <RefreshCw class="w-6 h-6 animate-spin text-[var(--color-primary)]" />
                <span class="ml-2 text-[var(--color-text-muted)]">데이터 로딩 중...</span>
              </Show>
            </div>
          }
        >
          <EquityCurve
            data={chartData()}
            height={props.height || 300}
            colors={{
              equityColor: dataSource() === 'portfolio' ? '#3b82f6' : '#10b981',
              positiveArea: dataSource() === 'portfolio' ? 'rgba(59, 130, 246, 0.2)' : 'rgba(16, 185, 129, 0.2)',
            }}
          />
        </Show>
      </div>

      {/* Data Source Indicator */}
      <div class="px-4 pb-4">
        <div class="flex items-center justify-between text-xs text-[var(--color-text-muted)]">
          <div class="flex items-center gap-2">
            <div class={`w-2 h-2 rounded-full ${
              dataSource() === 'portfolio' ? 'bg-blue-500' : 'bg-green-500'
            }`} />
            <span>
              {dataSource() === 'portfolio' ? '실제 포트폴리오 데이터' : '백테스트 시뮬레이션 데이터'}
            </span>
          </div>
          <Show when={dataSource() === 'portfolio' && equityCurveData()}>
            <span>
              {equityCurveData()?.count || 0}개 데이터 포인트 •
              {equityCurveData()?.startTime && ` ${new Date(equityCurveData()!.startTime).toLocaleDateString('ko-KR')}`}
              {equityCurveData()?.endTime && ` ~ ${new Date(equityCurveData()!.endTime).toLocaleDateString('ko-KR')}`}
            </span>
          </Show>
          <Show when={dataSource() === 'backtest' && selectedBacktest()}>
            <span>
              {selectedBacktest()?.equity_curve?.length || 0}개 데이터 포인트 •
              {selectedBacktest()?.start_date} ~ {selectedBacktest()?.end_date}
            </span>
          </Show>
        </div>
      </div>

      {/* Sync Modal */}
      <Show when={showSyncModal()}>
        <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onClick={() => setShowSyncModal(false)}>
          <div
            class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-6 w-full max-w-md shadow-xl"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Modal Header */}
            <div class="flex items-center justify-between mb-4">
              <h3 class="text-lg font-semibold text-[var(--color-text)]">거래소 데이터 동기화</h3>
              <button
                onClick={() => setShowSyncModal(false)}
                class="p-1 rounded hover:bg-[var(--color-surface-light)] text-[var(--color-text-muted)]"
              >
                <X class="w-5 h-5" />
              </button>
            </div>

            {/* Modal Body */}
            <div class="space-y-4">
              <p class="text-sm text-[var(--color-text-muted)]">
                거래소 체결 내역을 기반으로 자산 곡선 데이터를 복원합니다.
              </p>

              {/* Credential Selector */}
              <div>
                <label class="block text-sm font-medium text-[var(--color-text)] mb-1">
                  거래소 계정
                </label>
                <select
                  value={syncCredentialId()}
                  onChange={(e) => setSyncCredentialId(e.currentTarget.value)}
                  class="w-full bg-[var(--color-surface-light)] text-[var(--color-text)] border border-[var(--color-surface-light)] rounded-lg px-3 py-2 text-sm"
                >
                  <Show when={!credentials() || credentials()!.length === 0}>
                    <option value="">등록된 KIS 계정이 없습니다</option>
                  </Show>
                  <For each={credentials()}>
                    {(cred: ExchangeCredential) => (
                      <option value={cred.id}>
                        {cred.display_name} {cred.is_testnet ? '(모의투자)' : '(실계좌)'}
                      </option>
                    )}
                  </For>
                </select>
              </div>

              {/* Date Range */}
              <div class="grid grid-cols-2 gap-3">
                <div>
                  <label class="block text-sm font-medium text-[var(--color-text)] mb-1">
                    시작일
                  </label>
                  <input
                    type="date"
                    value={syncStartDate().replace(/(\d{4})(\d{2})(\d{2})/, '$1-$2-$3')}
                    onChange={(e) => setSyncStartDate(e.currentTarget.value.replace(/-/g, ''))}
                    class="w-full bg-[var(--color-surface-light)] text-[var(--color-text)] border border-[var(--color-surface-light)] rounded-lg px-3 py-2 text-sm"
                  />
                </div>
                <div>
                  <label class="block text-sm font-medium text-[var(--color-text)] mb-1">
                    종료일
                  </label>
                  <input
                    type="date"
                    value={syncEndDate().replace(/(\d{4})(\d{2})(\d{2})/, '$1-$2-$3')}
                    onChange={(e) => setSyncEndDate(e.currentTarget.value.replace(/-/g, ''))}
                    class="w-full bg-[var(--color-surface-light)] text-[var(--color-text)] border border-[var(--color-surface-light)] rounded-lg px-3 py-2 text-sm"
                  />
                </div>
              </div>

              {/* Result Message */}
              <Show when={syncResult()}>
                <div class={`flex items-center gap-2 p-3 rounded-lg ${
                  syncResult()!.success
                    ? 'bg-green-500/10 text-green-500'
                    : 'bg-red-500/10 text-red-500'
                }`}>
                  {syncResult()!.success ? (
                    <CheckCircle class="w-5 h-5 flex-shrink-0" />
                  ) : (
                    <AlertCircle class="w-5 h-5 flex-shrink-0" />
                  )}
                  <span class="text-sm">{syncResult()!.message}</span>
                </div>
              </Show>
            </div>

            {/* Modal Footer */}
            <div class="flex justify-end gap-2 mt-6">
              <button
                onClick={() => setShowSyncModal(false)}
                class="px-4 py-2 rounded-lg text-sm text-[var(--color-text-muted)] hover:bg-[var(--color-surface-light)] transition-colors"
              >
                닫기
              </button>
              <button
                onClick={handleSync}
                disabled={syncLoading() || !syncCredentialId()}
                class="flex items-center gap-2 px-4 py-2 rounded-lg text-sm bg-[var(--color-primary)] text-white hover:bg-[var(--color-primary-hover)] transition-colors disabled:opacity-50"
              >
                {syncLoading() ? (
                  <>
                    <RefreshCw class="w-4 h-4 animate-spin" />
                    동기화 중...
                  </>
                ) : (
                  <>
                    <Download class="w-4 h-4" />
                    동기화 시작
                  </>
                )}
              </button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  )
}
