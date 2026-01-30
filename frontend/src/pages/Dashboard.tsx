import { createSignal, onMount, For, Show, createResource, createEffect } from 'solid-js'
import {
  TrendingUp,
  TrendingDown,
  DollarSign,
  Activity,
  BarChart3,
  ArrowUpRight,
  ArrowDownRight,
  RefreshCw,
  AlertCircle,
  Bell,
  Bot,
  Play,
  Pause,
  Building2,
  Settings,
} from 'lucide-solid'
import { createWebSocket } from '../hooks/createWebSocket'
import { getPortfolioSummary, getHoldings, getMarketStatus, getStrategies, getActiveAccount } from '../api/client'
import { PortfolioEquityChart } from '../components/charts'
import type { WsOrderUpdate, WsPositionUpdate, Strategy } from '../types'
import type { HoldingInfo, ActiveAccount } from '../api/client'

function formatCurrency(value: number | string, currency: 'KRW' | 'USD' = 'KRW'): string {
  const numValue = typeof value === 'string' ? parseFloat(value) : value
  if (isNaN(numValue)) return '₩0'

  if (currency === 'KRW') {
    return new Intl.NumberFormat('ko-KR', {
      style: 'currency',
      currency: 'KRW',
      maximumFractionDigits: 0,
    }).format(numValue)
  }
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
  }).format(numValue)
}

function formatPercent(value: number | string): string {
  const numValue = typeof value === 'string' ? parseFloat(value) : value
  if (isNaN(numValue)) return '+0.00%'
  const sign = numValue >= 0 ? '+' : ''
  return `${sign}${numValue.toFixed(2)}%`
}

export function Dashboard() {
  // 활성 계정 조회
  const [activeAccount, { refetch: refetchActiveAccount }] = createResource(async () => {
    try {
      return await getActiveAccount()
    } catch {
      return { credential_id: null, exchange_id: null, display_name: null, is_testnet: false } as ActiveAccount
    }
  })

  // API 데이터 로딩 - 활성 계정의 credential_id를 사용
  const [portfolio, { refetch: refetchPortfolio }] = createResource(
    () => activeAccount()?.credential_id,
    async (credentialId) => {
      return getPortfolioSummary(credentialId || undefined)
    }
  )
  const [holdings, { refetch: refetchHoldings }] = createResource(
    () => activeAccount()?.credential_id,
    async (credentialId) => {
      return getHoldings(credentialId || undefined)
    }
  )
  const [strategies, { refetch: refetchStrategies }] = createResource(getStrategies)
  const [krMarketStatus] = createResource(() => getMarketStatus('KR'))
  const [usMarketStatus] = createResource(() => getMarketStatus('US'))

  const [isRefreshing, setIsRefreshing] = createSignal(false)

  // 실시간 주문/포지션 알림
  const [recentOrderUpdates, setRecentOrderUpdates] = createSignal<WsOrderUpdate[]>([])
  const [recentPositionUpdates, setRecentPositionUpdates] = createSignal<WsPositionUpdate[]>([])
  const [showNotifications, setShowNotifications] = createSignal(false)

  const { isConnected, subscribeChannels } = createWebSocket({
    onOrderUpdate: (order) => {
      setRecentOrderUpdates((prev) => [order, ...prev].slice(0, 10))
      refetchHoldings()
    },
    onPositionUpdate: (position) => {
      setRecentPositionUpdates((prev) => [position, ...prev].slice(0, 10))
      refetchPortfolio()
      refetchHoldings()
    },
  })

  onMount(() => {
    subscribeChannels(['orders', 'positions'])
  })

  // 데이터 새로고침
  const handleRefresh = async () => {
    setIsRefreshing(true)
    try {
      await Promise.all([refetchActiveAccount(), refetchPortfolio(), refetchHoldings(), refetchStrategies()])
    } finally {
      setIsRefreshing(false)
    }
  }

  // 실행 중인 전략 필터링
  const runningStrategies = () => {
    const all = strategies() || []
    return all.filter((s: Strategy) => s.status === 'running')
  }

  // 포지션 데이터 변환
  const positions = () => {
    const h = holdings()
    if (!h) return []

    const allHoldings = [...(h.krHoldings || []), ...(h.usHoldings || [])]
    return allHoldings.map((holding: HoldingInfo, index: number) => ({
      id: `${holding.market}-${index}`,
      symbol: holding.displayName || holding.name || holding.symbol,
      side: 'Long' as const,
      quantity: parseFloat(holding.quantity) || 0,
      entryPrice: parseFloat(holding.avgPrice) || 0,
      currentPrice: parseFloat(holding.currentPrice) || 0,
      unrealizedPnl: parseFloat(holding.profitLoss) || 0,
      unrealizedPnlPercent: parseFloat(holding.profitLossRate) || 0,
      market: holding.market,
    }))
  }

  return (
    <div class="space-y-6">
      {/* Header with Connection Status & Refresh */}
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-4">
          {/* Connection Status */}
          <div class="flex items-center gap-2 text-sm">
            <div
              class={`w-2 h-2 rounded-full ${
                isConnected() ? 'bg-green-500' : 'bg-red-500'
              }`}
            />
            <span class="text-[var(--color-text-muted)]">
              {isConnected() ? '실시간 연결됨' : '연결 끊김'}
            </span>
          </div>

          {/* Market Status */}
          <Show when={krMarketStatus()}>
            <div class="flex items-center gap-2 text-sm">
              <span class={`px-2 py-0.5 rounded ${krMarketStatus()?.isOpen ? 'bg-green-500/20 text-green-400' : 'bg-gray-500/20 text-gray-400'}`}>
                KR {krMarketStatus()?.isOpen ? '개장' : '폐장'}
              </span>
            </div>
          </Show>
          <Show when={usMarketStatus()}>
            <div class="flex items-center gap-2 text-sm">
              <span class={`px-2 py-0.5 rounded ${usMarketStatus()?.isOpen ? 'bg-green-500/20 text-green-400' : 'bg-gray-500/20 text-gray-400'}`}>
                US {usMarketStatus()?.isOpen ? (usMarketStatus()?.session || '개장') : '폐장'}
              </span>
            </div>
          </Show>

          {/* Active Account Display */}
          <Show when={activeAccount()}>
            <div class="flex items-center gap-2 text-sm border-l border-[var(--color-surface-light)] pl-4">
              <Building2 class="w-4 h-4 text-[var(--color-text-muted)]" />
              <Show
                when={activeAccount()?.credential_id}
                fallback={
                  <a href="/settings" class="text-[var(--color-text-muted)] hover:text-[var(--color-primary)] transition-colors flex items-center gap-1">
                    <span>계정 선택 안됨</span>
                    <Settings class="w-3 h-3" />
                  </a>
                }
              >
                <span class="text-[var(--color-text)]">{activeAccount()?.display_name}</span>
                <Show when={activeAccount()?.is_testnet}>
                  <span class="px-1.5 py-0.5 text-xs rounded bg-yellow-500/20 text-yellow-500">
                    모의
                  </span>
                </Show>
              </Show>
            </div>
          </Show>
        </div>

        <div class="flex items-center gap-2">
          {/* Notifications Button */}
          <div class="relative">
            <button
              onClick={() => setShowNotifications(!showNotifications())}
              class="relative flex items-center gap-2 px-3 py-1.5 rounded-lg bg-[var(--color-surface-light)] text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors"
            >
              <Bell class="w-4 h-4" />
              <Show when={recentOrderUpdates().length > 0 || recentPositionUpdates().length > 0}>
                <span class="absolute -top-1 -right-1 w-4 h-4 bg-red-500 rounded-full text-xs text-white flex items-center justify-center">
                  {recentOrderUpdates().length + recentPositionUpdates().length}
                </span>
              </Show>
            </button>

            {/* Notifications Dropdown */}
            <Show when={showNotifications()}>
              <div class="absolute right-0 top-full mt-2 w-80 bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] shadow-xl z-50 max-h-96 overflow-y-auto">
                <div class="p-3 border-b border-[var(--color-surface-light)]">
                  <h4 class="text-sm font-semibold text-[var(--color-text)]">실시간 알림</h4>
                </div>

                <Show when={recentOrderUpdates().length > 0}>
                  <div class="p-2">
                    <div class="text-xs text-[var(--color-text-muted)] px-2 mb-1">주문 업데이트</div>
                    <For each={recentOrderUpdates().slice(0, 5)}>
                      {(order) => (
                        <div class="p-2 rounded-lg hover:bg-[var(--color-surface-light)] transition-colors">
                          <div class="flex items-center justify-between">
                            <span class="text-sm font-medium text-[var(--color-text)]">{order.symbol}</span>
                            <span class={`text-xs px-1.5 py-0.5 rounded ${
                              order.status === 'filled' ? 'bg-green-500/20 text-green-400' :
                              order.status === 'cancelled' ? 'bg-red-500/20 text-red-400' :
                              'bg-yellow-500/20 text-yellow-400'
                            }`}>
                              {order.status}
                            </span>
                          </div>
                          <div class="text-xs text-[var(--color-text-muted)] mt-1">
                            {order.side === 'buy' ? '매수' : '매도'} {order.filled_quantity}/{order.quantity}
                            {order.price && ` @ ${order.price}`}
                          </div>
                        </div>
                      )}
                    </For>
                  </div>
                </Show>

                <Show when={recentPositionUpdates().length > 0}>
                  <div class="p-2 border-t border-[var(--color-surface-light)]">
                    <div class="text-xs text-[var(--color-text-muted)] px-2 mb-1">포지션 업데이트</div>
                    <For each={recentPositionUpdates().slice(0, 5)}>
                      {(position) => (
                        <div class="p-2 rounded-lg hover:bg-[var(--color-surface-light)] transition-colors">
                          <div class="flex items-center justify-between">
                            <span class="text-sm font-medium text-[var(--color-text)]">{position.symbol}</span>
                            <span class={`text-xs ${parseFloat(position.unrealized_pnl) >= 0 ? 'text-green-400' : 'text-red-400'}`}>
                              {parseFloat(position.return_pct) >= 0 ? '+' : ''}{position.return_pct}%
                            </span>
                          </div>
                          <div class="text-xs text-[var(--color-text-muted)] mt-1">
                            {position.side === 'long' ? '롱' : '숏'} {position.quantity} @ {position.current_price}
                          </div>
                        </div>
                      )}
                    </For>
                  </div>
                </Show>

                <Show when={recentOrderUpdates().length === 0 && recentPositionUpdates().length === 0}>
                  <div class="p-4 text-center text-[var(--color-text-muted)] text-sm">
                    새로운 알림이 없습니다.
                  </div>
                </Show>
              </div>
            </Show>
          </div>

          {/* Refresh Button */}
          <button
            onClick={handleRefresh}
            disabled={isRefreshing()}
            class="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-[var(--color-surface-light)] text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors disabled:opacity-50"
          >
            <RefreshCw class={`w-4 h-4 ${isRefreshing() ? 'animate-spin' : ''}`} />
            새로고침
          </button>
        </div>
      </div>

      {/* Loading State */}
      <Show when={portfolio.loading}>
        <div class="flex items-center justify-center py-8">
          <RefreshCw class="w-6 h-6 animate-spin text-[var(--color-primary)]" />
          <span class="ml-2 text-[var(--color-text-muted)]">데이터 로딩 중...</span>
        </div>
      </Show>

      {/* Error State */}
      <Show when={portfolio.error}>
        <div class="flex items-center gap-2 p-4 bg-red-500/10 rounded-lg">
          <AlertCircle class="w-5 h-5 text-red-500" />
          <span class="text-red-500">포트폴리오 데이터를 불러오는데 실패했습니다.</span>
          <button onClick={handleRefresh} class="ml-auto text-sm text-blue-500 hover:underline">
            다시 시도
          </button>
        </div>
      </Show>

      {/* Portfolio Summary Cards */}
      <Show when={!portfolio.loading && !portfolio.error}>
        {/* Testnet/Mock Account Banner */}
        <Show when={activeAccount()?.is_testnet}>
          <div class="flex items-center gap-2 p-3 bg-yellow-500/10 border border-yellow-500/30 rounded-lg">
            <AlertCircle class="w-5 h-5 text-yellow-500" />
            <span class="text-yellow-500 text-sm font-medium">
              모의투자 계정의 자산 정보입니다. 실제 자산과 다를 수 있습니다.
            </span>
          </div>
        </Show>

        {/* No Account Selected Info */}
        <Show when={!activeAccount()?.credential_id}>
          <div class="flex items-center justify-between p-4 bg-[var(--color-surface-light)] rounded-lg">
            <div class="flex items-center gap-2">
              <Building2 class="w-5 h-5 text-[var(--color-text-muted)]" />
              <span class="text-[var(--color-text-muted)]">
                거래소 계정이 선택되지 않았습니다. 샘플 데이터를 표시합니다.
              </span>
            </div>
            <a
              href="/settings"
              class="px-3 py-1.5 bg-[var(--color-primary)] text-white rounded-lg text-sm font-medium hover:bg-[var(--color-primary)]/90 transition-colors flex items-center gap-2"
            >
              <Settings class="w-4 h-4" />
              계정 설정
            </a>
          </div>
        </Show>

        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          {/* Total Value */}
          <div class="bg-[var(--color-surface)] rounded-xl p-6 border border-[var(--color-surface-light)]">
            <div class="flex items-center justify-between mb-4">
              <span class="text-[var(--color-text-muted)]">총 자산</span>
              <DollarSign class="w-5 h-5 text-[var(--color-primary)]" />
            </div>
            <div class="text-2xl font-bold text-[var(--color-text)]">
              {formatCurrency(portfolio()?.totalValue || 0)}
            </div>
            <div class="flex items-center gap-1 mt-2">
              <Show
                when={(portfolio()?.totalPnlPercent || 0) >= 0}
                fallback={<ArrowDownRight class="w-4 h-4 text-red-500" />}
              >
                <ArrowUpRight class="w-4 h-4 text-green-500" />
              </Show>
              <span
                class={(portfolio()?.totalPnlPercent || 0) >= 0 ? 'text-green-500' : 'text-red-500'}
              >
                {formatPercent(portfolio()?.totalPnlPercent || 0)}
              </span>
            </div>
          </div>

          {/* Daily P&L */}
          <div class="bg-[var(--color-surface)] rounded-xl p-6 border border-[var(--color-surface-light)]">
            <div class="flex items-center justify-between mb-4">
              <span class="text-[var(--color-text-muted)]">일일 손익</span>
              <Activity class="w-5 h-5 text-[var(--color-primary)]" />
            </div>
            <div
              class={`text-2xl font-bold ${
                (portfolio()?.dailyPnl || 0) >= 0 ? 'text-green-500' : 'text-red-500'
              }`}
            >
              {(portfolio()?.dailyPnl || 0) >= 0 ? '+' : ''}
              {formatCurrency(portfolio()?.dailyPnl || 0)}
            </div>
            <div class="flex items-center gap-1 mt-2">
              <Show
                when={(portfolio()?.dailyPnlPercent || 0) >= 0}
                fallback={<TrendingDown class="w-4 h-4 text-red-500" />}
              >
                <TrendingUp class="w-4 h-4 text-green-500" />
              </Show>
              <span
                class={(portfolio()?.dailyPnlPercent || 0) >= 0 ? 'text-green-500' : 'text-red-500'}
              >
                {formatPercent(portfolio()?.dailyPnlPercent || 0)}
              </span>
            </div>
          </div>

          {/* Total P&L */}
          <div class="bg-[var(--color-surface)] rounded-xl p-6 border border-[var(--color-surface-light)]">
            <div class="flex items-center justify-between mb-4">
              <span class="text-[var(--color-text-muted)]">총 손익</span>
              <BarChart3 class="w-5 h-5 text-[var(--color-primary)]" />
            </div>
            <div
              class={`text-2xl font-bold ${
                (portfolio()?.totalPnl || 0) >= 0 ? 'text-green-500' : 'text-red-500'
              }`}
            >
              {(portfolio()?.totalPnl || 0) >= 0 ? '+' : ''}
              {formatCurrency(portfolio()?.totalPnl || 0)}
            </div>
            <div class="text-sm text-[var(--color-text-muted)] mt-2">
              누적 수익
            </div>
          </div>

          {/* Cash Balance */}
          <div class="bg-[var(--color-surface)] rounded-xl p-6 border border-[var(--color-surface-light)]">
            <div class="flex items-center justify-between mb-4">
              <span class="text-[var(--color-text-muted)]">현금 잔고</span>
              <DollarSign class="w-5 h-5 text-[var(--color-primary)]" />
            </div>
            <div class="text-2xl font-bold text-[var(--color-text)]">
              {formatCurrency(portfolio()?.cashBalance || 0)}
            </div>
            <div class="text-sm text-[var(--color-text-muted)] mt-2">
              거래 가능 금액
            </div>
          </div>
        </div>
      </Show>

      {/* Equity Curve - 실제 데이터 + 백테스트 데이터 지원 */}
      <PortfolioEquityChart
        height={280}
        showControls={true}
        defaultPeriod="3m"
        defaultSource="portfolio"
      />

      {/* Main Content Grid */}
      <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Positions */}
        <div class="lg:col-span-2 bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)]">
          <div class="p-4 border-b border-[var(--color-surface-light)] flex items-center justify-between">
            <h3 class="text-lg font-semibold text-[var(--color-text)]">
              보유 포지션
            </h3>
            <Show when={holdings()}>
              <span class="text-sm text-[var(--color-text-muted)]">
                {holdings()?.totalCount || 0}개 종목
              </span>
            </Show>
          </div>

          <Show when={holdings.loading}>
            <div class="flex items-center justify-center py-8">
              <RefreshCw class="w-5 h-5 animate-spin text-[var(--color-primary)]" />
            </div>
          </Show>

          <Show when={!holdings.loading && positions().length === 0}>
            <div class="p-8 text-center text-[var(--color-text-muted)]">
              보유 중인 종목이 없습니다.
            </div>
          </Show>

          <Show when={!holdings.loading && positions().length > 0}>
            <div class="overflow-x-auto">
              <table class="w-full">
                <thead>
                  <tr class="border-b border-[var(--color-surface-light)]">
                    <th class="text-left p-4 text-sm font-medium text-[var(--color-text-muted)]">
                      종목
                    </th>
                    <th class="text-right p-4 text-sm font-medium text-[var(--color-text-muted)]">
                      수량
                    </th>
                    <th class="text-right p-4 text-sm font-medium text-[var(--color-text-muted)]">
                      매입가
                    </th>
                    <th class="text-right p-4 text-sm font-medium text-[var(--color-text-muted)]">
                      현재가
                    </th>
                    <th class="text-right p-4 text-sm font-medium text-[var(--color-text-muted)]">
                      손익
                    </th>
                  </tr>
                </thead>
                <tbody>
                  <For each={positions()}>
                    {(position) => (
                      <tr class="border-b border-[var(--color-surface-light)] hover:bg-[var(--color-surface-light)]/50 transition-colors">
                        <td class="p-4">
                          <div class="flex items-center gap-2">
                            <span
                              class={`px-2 py-0.5 text-xs rounded ${
                                position.market === 'KR'
                                  ? 'bg-blue-500/20 text-blue-400'
                                  : position.market === 'US'
                                  ? 'bg-green-500/20 text-green-400'
                                  : 'bg-orange-500/20 text-orange-400'
                              }`}
                            >
                              {position.market}
                            </span>
                            <span class="font-medium text-[var(--color-text)]">
                              {position.symbol}
                            </span>
                          </div>
                        </td>
                        <td class="p-4 text-right text-[var(--color-text)]">
                          {position.quantity.toLocaleString()}
                        </td>
                        <td class="p-4 text-right text-[var(--color-text)]">
                          {position.market === 'KR'
                            ? formatCurrency(position.entryPrice)
                            : formatCurrency(position.entryPrice, 'USD')}
                        </td>
                        <td class="p-4 text-right text-[var(--color-text)]">
                          {position.market === 'KR'
                            ? formatCurrency(position.currentPrice)
                            : formatCurrency(position.currentPrice, 'USD')}
                        </td>
                        <td class="p-4 text-right">
                          <div
                            class={
                              position.unrealizedPnl >= 0 ? 'text-green-500' : 'text-red-500'
                            }
                          >
                            <div class="font-medium">
                              {formatPercent(position.unrealizedPnlPercent)}
                            </div>
                            <div class="text-sm">
                              {position.market === 'KR'
                                ? formatCurrency(position.unrealizedPnl)
                                : formatCurrency(position.unrealizedPnl, 'USD')}
                            </div>
                          </div>
                        </td>
                      </tr>
                    )}
                  </For>
                </tbody>
              </table>
            </div>
          </Show>
        </div>

        {/* Running Strategies */}
        <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)]">
          <div class="p-4 border-b border-[var(--color-surface-light)] flex items-center justify-between">
            <h3 class="text-lg font-semibold text-[var(--color-text)]">
              실행 중인 전략
            </h3>
            <Show when={strategies()}>
              <span class="text-sm text-[var(--color-text-muted)]">
                {runningStrategies().length}개 실행 중
              </span>
            </Show>
          </div>

          <Show when={strategies.loading}>
            <div class="flex items-center justify-center py-8">
              <RefreshCw class="w-5 h-5 animate-spin text-[var(--color-primary)]" />
            </div>
          </Show>

          <Show when={!strategies.loading && runningStrategies().length === 0}>
            <div class="p-8 text-center text-[var(--color-text-muted)]">
              <Bot class="w-12 h-12 mx-auto mb-3 opacity-50" />
              <p>실행 중인 전략이 없습니다.</p>
              <a href="/strategies" class="mt-2 text-[var(--color-primary)] hover:underline inline-block">
                전략 시작하기
              </a>
            </div>
          </Show>

          <Show when={!strategies.loading && runningStrategies().length > 0}>
            <div class="divide-y divide-[var(--color-surface-light)]">
              <For each={runningStrategies()}>
                {(strategy: Strategy) => (
                  <div class="p-4 hover:bg-[var(--color-surface-light)]/50 transition-colors">
                    <div class="flex items-center justify-between">
                      <div class="flex items-center gap-3">
                        <div class="p-2 rounded-lg bg-green-500/20">
                          <Play class="w-4 h-4 text-green-400" />
                        </div>
                        <div>
                          <div class="font-medium text-[var(--color-text)]">{strategy.name}</div>
                          <div class="text-sm text-[var(--color-text-muted)]">
                            {strategy.symbols?.join(', ') || '심볼 없음'}
                          </div>
                        </div>
                      </div>
                      <Show when={strategy.metrics}>
                        <div class="text-right">
                          <div class={`font-medium ${(strategy.metrics?.totalPnlPercent || 0) >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                            {formatPercent(strategy.metrics?.totalPnlPercent || 0)}
                          </div>
                          <div class="text-xs text-[var(--color-text-muted)]">
                            {strategy.metrics?.tradeCount || 0}회 거래
                          </div>
                        </div>
                      </Show>
                    </div>
                  </div>
                )}
              </For>
            </div>
          </Show>
        </div>
      </div>
    </div>
  )
}
