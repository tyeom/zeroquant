import { createSignal, For, Show, createEffect, onCleanup, createResource } from 'solid-js'
import { useSearchParams } from '@solidjs/router'
import {
  Play,
  Pause,
  RotateCcw,
  FastForward,
  Clock,
  TrendingUp,
  TrendingDown,
  Activity,
  DollarSign,
  Square,
  RefreshCw,
} from 'lucide-solid'
import { EquityCurve } from '../components/charts'
import type { EquityDataPoint } from '../components/charts'
import {
  startSimulation,
  stopSimulation,
  pauseSimulation,
  resetSimulation,
  getSimulationStatus,
  getSimulationPositions,
  getSimulationTrades,
  getStrategies,
  type SimulationStatusResponse,
  type SimulationPosition,
  type SimulationTrade,
} from '../api/client'
import type { Strategy } from '../types'

function formatCurrency(value: number | string): string {
  const numValue = typeof value === 'string' ? parseFloat(value) : value
  return new Intl.NumberFormat('ko-KR', {
    style: 'currency',
    currency: 'KRW',
    minimumFractionDigits: 0,
    maximumFractionDigits: 0,
  }).format(numValue)
}

function formatDecimal(value: string | number, decimals: number = 2): string {
  const numValue = typeof value === 'string' ? parseFloat(value) : value
  return numValue.toFixed(decimals)
}

export function Simulation() {
  // URL 파라미터 읽기 (전략 페이지에서 바로 이동 시)
  const [searchParams] = useSearchParams()

  // 등록된 전략 목록 로드
  const [strategies] = createResource(async () => {
    try {
      return await getStrategies()
    } catch {
      return [] as Strategy[]
    }
  })

  // 상태 관리
  const [status, setStatus] = createSignal<SimulationStatusResponse | null>(null)
  const [positions, setPositions] = createSignal<SimulationPosition[]>([])
  const [trades, setTrades] = createSignal<SimulationTrade[]>([])
  const [isLoading, setIsLoading] = createSignal(false)
  const [error, setError] = createSignal<string | null>(null)

  // 폼 상태
  const [selectedStrategy, setSelectedStrategy] = createSignal('')

  // URL에서 전략 ID가 있으면 자동 선택
  createEffect(() => {
    const strategyId = searchParams.strategy
    if (strategyId && strategies() && strategies()!.length > 0) {
      const found = strategies()!.find(s => s.id === strategyId)
      if (found) {
        setSelectedStrategy(found.strategyType)
      }
    }
  })
  const [initialBalance, setInitialBalance] = createSignal('10000000')
  const [speed, setSpeed] = createSignal(1)

  // 자산 곡선 데이터
  const [equityCurve, setEquityCurve] = createSignal<EquityDataPoint[]>([])

  // 폴링 인터벌
  let pollInterval: ReturnType<typeof setInterval> | undefined

  // 초기 상태 로드
  const loadStatus = async () => {
    try {
      const statusData = await getSimulationStatus()
      setStatus(statusData)

      // 전략 선택 초기화
      if (statusData.strategy_id && !selectedStrategy()) {
        setSelectedStrategy(statusData.strategy_id)
      }

      // 포지션/거래 로드
      const positionsData = await getSimulationPositions()
      setPositions(positionsData.positions)

      const tradesData = await getSimulationTrades()
      setTrades(tradesData.trades)

      // 자산 곡선에 데이터 추가 (실행 중일 때)
      if (statusData.state === 'running') {
        const timestamp = Math.floor(Date.now() / 1000)
        const equity = parseFloat(statusData.total_equity)
        setEquityCurve(prev => {
          // 중복 방지
          if (prev.length > 0 && prev[prev.length - 1].time === timestamp) {
            return prev
          }
          return [...prev, { time: timestamp, value: equity }]
        })
      }

      setError(null)
    } catch (err) {
      console.error('Failed to load simulation status:', err)
      setError('시뮬레이션 상태를 불러오는데 실패했습니다')
    }
  }

  // 컴포넌트 마운트 시 상태 로드
  createEffect(() => {
    loadStatus()
  })

  // 실행 중일 때 폴링
  createEffect(() => {
    const currentStatus = status()

    if (currentStatus?.state === 'running') {
      // 1초마다 상태 업데이트
      pollInterval = setInterval(() => {
        loadStatus()
      }, 1000)
    } else {
      if (pollInterval) {
        clearInterval(pollInterval)
        pollInterval = undefined
      }
    }
  })

  // 클린업
  onCleanup(() => {
    if (pollInterval) {
      clearInterval(pollInterval)
    }
  })

  // 시뮬레이션 시작
  const handleStart = async () => {
    if (!selectedStrategy()) {
      setError('전략을 선택해주세요')
      return
    }

    setIsLoading(true)
    setError(null)

    try {
      await startSimulation({
        strategy_id: selectedStrategy(),
        initial_balance: parseInt(initialBalance(), 10),
        speed: speed(),
      })

      // 자산 곡선 초기화
      setEquityCurve([])

      await loadStatus()
    } catch (err) {
      console.error('Failed to start simulation:', err)
      setError('시뮬레이션 시작에 실패했습니다')
    } finally {
      setIsLoading(false)
    }
  }

  // 시뮬레이션 중지
  const handleStop = async () => {
    setIsLoading(true)
    try {
      await stopSimulation()
      await loadStatus()
    } catch (err) {
      console.error('Failed to stop simulation:', err)
      setError('시뮬레이션 중지에 실패했습니다')
    } finally {
      setIsLoading(false)
    }
  }

  // 일시정지/재개
  const handlePause = async () => {
    setIsLoading(true)
    try {
      await pauseSimulation()
      await loadStatus()
    } catch (err) {
      console.error('Failed to pause simulation:', err)
      setError('시뮬레이션 일시정지에 실패했습니다')
    } finally {
      setIsLoading(false)
    }
  }

  // 리셋
  const handleReset = async () => {
    setIsLoading(true)
    try {
      await resetSimulation()
      setEquityCurve([])
      await loadStatus()
    } catch (err) {
      console.error('Failed to reset simulation:', err)
      setError('시뮬레이션 리셋에 실패했습니다')
    } finally {
      setIsLoading(false)
    }
  }

  // 계산된 값
  const isRunning = () => status()?.state === 'running'
  const isPaused = () => status()?.state === 'paused'
  const isStopped = () => status()?.state === 'stopped'

  const totalPnl = () => {
    const s = status()
    if (!s) return 0
    return parseFloat(s.realized_pnl) + parseFloat(s.unrealized_pnl)
  }

  const totalPnlPercent = () => {
    const s = status()
    if (!s) return 0
    return parseFloat(s.return_pct)
  }

  return (
    <div class="space-y-6">
      {/* 에러 표시 */}
      <Show when={error()}>
        <div class="bg-red-500/20 border border-red-500/50 rounded-lg p-4 text-red-400">
          {error()}
        </div>
      </Show>

      {/* Simulation Controls */}
      <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-6">
        <div class="flex flex-wrap items-center justify-between gap-4">
          {/* Strategy & Settings */}
          <div class="flex items-center gap-6">
            <div>
              <label class="block text-sm text-[var(--color-text-muted)] mb-1">전략</label>
              <select
                value={selectedStrategy()}
                onChange={(e) => setSelectedStrategy(e.currentTarget.value)}
                disabled={!isStopped()}
                class="px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)] disabled:opacity-50"
              >
                <option value="">전략 선택...</option>
                <For each={strategies()}>
                  {(strategy) => (
                    <option value={strategy.strategyType}>
                      {strategy.name} ({strategy.strategyType})
                    </option>
                  )}
                </For>
              </select>
            </div>

            <div>
              <label class="block text-sm text-[var(--color-text-muted)] mb-1">초기 자본</label>
              <input
                type="number"
                value={initialBalance()}
                onInput={(e) => setInitialBalance(e.currentTarget.value)}
                disabled={!isStopped()}
                class="w-40 px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)] disabled:opacity-50"
              />
            </div>

            <Show when={status()?.started_at}>
              <div class="flex items-center gap-2 px-4 py-2 bg-[var(--color-surface-light)] rounded-lg">
                <Clock class="w-5 h-5 text-[var(--color-text-muted)]" />
                <span class="text-[var(--color-text)] font-mono text-sm">
                  {new Date(status()!.started_at!).toLocaleString('ko-KR')}
                </span>
              </div>
            </Show>

            <Show when={status()}>
              <div class="text-sm text-[var(--color-text-muted)]">
                거래: <span class="text-[var(--color-text)] font-semibold">{status()?.trade_count}건</span>
              </div>
            </Show>
          </div>

          {/* Controls */}
          <div class="flex items-center gap-2">
            {/* Speed Control */}
            <div class="flex items-center gap-1 mr-4">
              <FastForward class="w-4 h-4 text-[var(--color-text-muted)]" />
              <For each={[1, 2, 5, 10]}>
                {(spd) => (
                  <button
                    class={`px-3 py-1 text-sm rounded ${
                      speed() === spd
                        ? 'bg-[var(--color-primary)] text-white'
                        : 'bg-[var(--color-surface-light)] text-[var(--color-text-muted)] hover:text-[var(--color-text)]'
                    } transition-colors`}
                    onClick={() => setSpeed(spd)}
                    disabled={!isStopped()}
                  >
                    {spd}x
                  </button>
                )}
              </For>
            </div>

            {/* Start/Pause/Stop Buttons */}
            <Show when={isStopped()}>
              <button
                class="p-3 rounded-lg bg-green-500 hover:bg-green-600 text-white transition-colors disabled:opacity-50"
                onClick={handleStart}
                disabled={isLoading() || !selectedStrategy()}
              >
                <Play class="w-5 h-5" />
              </button>
            </Show>

            <Show when={isRunning() || isPaused()}>
              <button
                class={`p-3 rounded-lg ${
                  isPaused()
                    ? 'bg-green-500 hover:bg-green-600'
                    : 'bg-yellow-500 hover:bg-yellow-600'
                } text-white transition-colors disabled:opacity-50`}
                onClick={handlePause}
                disabled={isLoading()}
              >
                <Show when={isPaused()} fallback={<Pause class="w-5 h-5" />}>
                  <Play class="w-5 h-5" />
                </Show>
              </button>

              <button
                class="p-3 rounded-lg bg-red-500 hover:bg-red-600 text-white transition-colors disabled:opacity-50"
                onClick={handleStop}
                disabled={isLoading()}
              >
                <Square class="w-5 h-5" />
              </button>
            </Show>

            {/* Reset */}
            <button
              class="p-3 rounded-lg bg-[var(--color-surface-light)] text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors disabled:opacity-50"
              onClick={handleReset}
              disabled={isLoading() || isRunning()}
              title="초기화"
            >
              <RotateCcw class="w-5 h-5" />
            </button>

            {/* Refresh */}
            <button
              class="p-3 rounded-lg bg-[var(--color-surface-light)] text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors disabled:opacity-50"
              onClick={loadStatus}
              disabled={isLoading()}
              title="새로고침"
            >
              <RefreshCw class={`w-5 h-5 ${isLoading() ? 'animate-spin' : ''}`} />
            </button>
          </div>
        </div>
      </div>

      {/* Stats Cards */}
      <div class="grid grid-cols-1 md:grid-cols-4 gap-4">
        <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-4">
          <div class="flex items-center gap-3">
            <div class="p-2 rounded-lg bg-blue-500/20">
              <DollarSign class="w-5 h-5 text-blue-500" />
            </div>
            <div>
              <div class="text-sm text-[var(--color-text-muted)]">초기 자본</div>
              <div class="text-lg font-semibold text-[var(--color-text)]">
                {formatCurrency(status()?.initial_balance || initialBalance())}
              </div>
            </div>
          </div>
        </div>

        <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-4">
          <div class="flex items-center gap-3">
            <div class="p-2 rounded-lg bg-purple-500/20">
              <Activity class="w-5 h-5 text-purple-500" />
            </div>
            <div>
              <div class="text-sm text-[var(--color-text-muted)]">총 자산</div>
              <div class="text-lg font-semibold text-[var(--color-text)]">
                {formatCurrency(status()?.total_equity || '0')}
              </div>
            </div>
          </div>
        </div>

        <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-4">
          <div class="flex items-center gap-3">
            <div
              class={`p-2 rounded-lg ${
                totalPnl() >= 0 ? 'bg-green-500/20' : 'bg-red-500/20'
              }`}
            >
              <Show
                when={totalPnl() >= 0}
                fallback={<TrendingDown class="w-5 h-5 text-red-500" />}
              >
                <TrendingUp class="w-5 h-5 text-green-500" />
              </Show>
            </div>
            <div>
              <div class="text-sm text-[var(--color-text-muted)]">총 손익</div>
              <div
                class={`text-lg font-semibold ${
                  totalPnl() >= 0 ? 'text-green-500' : 'text-red-500'
                }`}
              >
                {totalPnl() >= 0 ? '+' : ''}
                {formatCurrency(totalPnl())}
              </div>
            </div>
          </div>
        </div>

        <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-4">
          <div class="flex items-center gap-3">
            <div
              class={`p-2 rounded-lg ${
                totalPnlPercent() >= 0 ? 'bg-green-500/20' : 'bg-red-500/20'
              }`}
            >
              <Show
                when={totalPnlPercent() >= 0}
                fallback={<TrendingDown class="w-5 h-5 text-red-500" />}
              >
                <TrendingUp class="w-5 h-5 text-green-500" />
              </Show>
            </div>
            <div>
              <div class="text-sm text-[var(--color-text-muted)]">수익률</div>
              <div
                class={`text-lg font-semibold ${
                  totalPnlPercent() >= 0 ? 'text-green-500' : 'text-red-500'
                }`}
              >
                {totalPnlPercent() >= 0 ? '+' : ''}
                {formatDecimal(totalPnlPercent())}%
              </div>
            </div>
          </div>
        </div>
      </div>

      <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Positions */}
        <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-6">
          <h3 class="text-lg font-semibold text-[var(--color-text)] mb-4">
            보유 포지션 ({positions().length})
          </h3>

          <Show
            when={positions().length > 0}
            fallback={
              <div class="text-center py-8 text-[var(--color-text-muted)]">
                포지션 없음
              </div>
            }
          >
            <div class="space-y-3">
              <For each={positions()}>
                {(position) => {
                  const pnl = parseFloat(position.unrealized_pnl)
                  const pnlPct = parseFloat(position.return_pct)
                  return (
                    <div class="flex items-center justify-between p-3 bg-[var(--color-surface-light)] rounded-lg">
                      <div>
                        <div class="flex items-center gap-2">
                          <span class="font-semibold text-[var(--color-text)]">
                            {position.displayName || position.symbol}
                          </span>
                          <span
                            class={`px-2 py-0.5 text-xs rounded ${
                              position.side === 'Long'
                                ? 'bg-green-500/20 text-green-400'
                                : 'bg-red-500/20 text-red-400'
                            }`}
                          >
                            {position.side}
                          </span>
                        </div>
                        <div class="text-sm text-[var(--color-text-muted)] mt-1">
                          {formatDecimal(position.quantity, 4)} @ {formatCurrency(position.entry_price)}
                        </div>
                      </div>
                      <div class="text-right">
                        <div
                          class={`font-semibold ${
                            pnl >= 0 ? 'text-green-500' : 'text-red-500'
                          }`}
                        >
                          {pnl >= 0 ? '+' : ''}{formatCurrency(pnl)}
                        </div>
                        <div
                          class={`text-sm ${
                            pnlPct >= 0 ? 'text-green-500' : 'text-red-500'
                          }`}
                        >
                          {pnlPct >= 0 ? '+' : ''}
                          {formatDecimal(pnlPct)}%
                        </div>
                      </div>
                    </div>
                  )
                }}
              </For>
            </div>
          </Show>
        </div>

        {/* Trade History */}
        <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-6">
          <h3 class="text-lg font-semibold text-[var(--color-text)] mb-4">
            거래 내역 ({trades().length})
          </h3>

          <Show
            when={trades().length > 0}
            fallback={
              <div class="text-center py-8 text-[var(--color-text-muted)]">
                거래 내역 없음
              </div>
            }
          >
            <div class="space-y-2 max-h-80 overflow-y-auto">
              <For each={[...trades()].reverse().slice(0, 20)}>
                {(trade) => {
                  const realizedPnl = trade.realized_pnl ? parseFloat(trade.realized_pnl) : null
                  return (
                    <div class="flex items-center justify-between p-3 bg-[var(--color-surface-light)] rounded-lg">
                      <div class="flex items-center gap-3">
                        <span class="text-sm text-[var(--color-text-muted)] font-mono">
                          {new Date(trade.timestamp).toLocaleTimeString('ko-KR')}
                        </span>
                        <span
                          class={`px-2 py-0.5 text-xs rounded font-medium ${
                            trade.side === 'Buy'
                              ? 'bg-green-500/20 text-green-400'
                              : 'bg-red-500/20 text-red-400'
                          }`}
                        >
                          {trade.side === 'Buy' ? '매수' : '매도'}
                        </span>
                        <span class="text-[var(--color-text)]">{trade.displayName || trade.symbol}</span>
                      </div>
                      <div class="text-right">
                        <div class="text-sm text-[var(--color-text)]">
                          {formatDecimal(trade.quantity, 4)} @ {formatCurrency(trade.price)}
                        </div>
                        <Show when={realizedPnl !== null}>
                          <div
                            class={`text-sm ${
                              realizedPnl! >= 0 ? 'text-green-500' : 'text-red-500'
                            }`}
                          >
                            {realizedPnl! >= 0 ? '+' : ''}{formatCurrency(realizedPnl!)}
                          </div>
                        </Show>
                      </div>
                    </div>
                  )
                }}
              </For>
            </div>
          </Show>
        </div>
      </div>

      {/* Equity Curve Chart */}
      <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-6">
        <h3 class="text-lg font-semibold text-[var(--color-text)] mb-4">자산 곡선</h3>
        <Show
          when={equityCurve().length > 1}
          fallback={
            <div class="h-[300px] flex items-center justify-center text-[var(--color-text-muted)]">
              시뮬레이션을 시작하면 자산 곡선이 표시됩니다
            </div>
          }
        >
          <EquityCurve data={equityCurve()} height={300} />
        </Show>
      </div>

      {/* Additional Stats */}
      <Show when={status() && (status()!.realized_pnl !== '0' || status()!.trade_count > 0)}>
        <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-6">
          <h3 class="text-lg font-semibold text-[var(--color-text)] mb-4">시뮬레이션 통계</h3>
          <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
            <div>
              <div class="text-sm text-[var(--color-text-muted)]">실현 손익</div>
              <div class={`text-lg font-semibold ${parseFloat(status()!.realized_pnl) >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                {formatCurrency(status()!.realized_pnl)}
              </div>
            </div>
            <div>
              <div class="text-sm text-[var(--color-text-muted)]">미실현 손익</div>
              <div class={`text-lg font-semibold ${parseFloat(status()!.unrealized_pnl) >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                {formatCurrency(status()!.unrealized_pnl)}
              </div>
            </div>
            <div>
              <div class="text-sm text-[var(--color-text-muted)]">현재 잔고</div>
              <div class="text-lg font-semibold text-[var(--color-text)]">
                {formatCurrency(status()!.current_balance)}
              </div>
            </div>
            <div>
              <div class="text-sm text-[var(--color-text-muted)]">포지션 수</div>
              <div class="text-lg font-semibold text-[var(--color-text)]">
                {status()!.position_count}개
              </div>
            </div>
          </div>
        </div>
      </Show>
    </div>
  )
}
