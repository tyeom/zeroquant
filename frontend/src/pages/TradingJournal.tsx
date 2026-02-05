/**
 * 매매 일지 페이지
 *
 * PRD 2.6에 따라 체결 내역, 보유 현황, 손익 분석 기능을 제공합니다.
 * 컴포넌트가 journal/ 폴더로 분리되어 모듈화되었습니다.
 *
 * 상태 관리: createStore를 사용하여 관련 상태를 그룹화
 * - filters: 필터 및 페이지네이션 상태
 * - loading: 로딩 상태
 * - modal: 모달 상태
 */
import { createResource, Show, createMemo } from 'solid-js'
import { createStore } from 'solid-js/store'
import { BookOpen, BarChart3, RefreshCw, LineChart, PieChart, Lightbulb } from 'lucide-solid'
import {
  PageHeader,
  StatCard,
  StatCardGrid,
  Button,
  Card,
  CardHeader,
  CardContent,
  formatCurrency,
  getPnLColor,
} from '../components/ui'
import {
  getJournalPositions,
  getJournalExecutions,
  getJournalPnLSummary,
  getJournalDailyPnL,
  getJournalSymbolPnL,
  getJournalWeeklyPnL,
  getJournalMonthlyPnL,
  getJournalYearlyPnL,
  getJournalCumulativePnL,
  getJournalInsights,
  getJournalStrategyPerformance,
  syncJournalExecutions,
  clearJournalCache,
} from '../api/client'
import type { ExecutionFilter } from '../api/client'

// 분리된 컴포넌트 import
import {
  PositionsTable,
  ExecutionsTable,
  SymbolPnLTable,
  PnLAnalysisPanel,
  StrategyInsightsPanel,
  PositionDonutChart,
  PositionDetailModal,
} from '../components/journal'
import type { JournalPosition } from '../api/client'

// ==================== 타입 정의 ====================

/** 탭 타입 (5개로 통합) */
type TabType = 'positions' | 'executions' | 'pnl-analysis' | 'symbols' | 'strategy-insights'

/** 필터 상태 타입 */
interface FilterState {
  symbol: string
  side: string
  startDate: string
  endDate: string
  currentPage: number
  pageSize: number
}

/** 로딩 상태 타입 */
interface LoadingState {
  isRefreshing: boolean
  isSyncing: boolean
}

/** 모달 상태 타입 */
interface ModalState {
  position: {
    open: boolean
    data: JournalPosition | null
  }
}

/** UI 상태 타입 */
interface UIState {
  activeTab: TabType
}

// ==================== 초기 상태 ====================

const initialFilterState: FilterState = {
  symbol: '',
  side: '',
  startDate: '',
  endDate: '',
  currentPage: 1,
  pageSize: 50,
}

const initialLoadingState: LoadingState = {
  isRefreshing: false,
  isSyncing: false,
}

const initialModalState: ModalState = {
  position: { open: false, data: null },
}

const initialUIState: UIState = {
  activeTab: 'positions',
}

// ==================== 유틸리티 함수 ====================

/** API 에러 발생 시에도 UI가 동작하도록 안전한 wrapper */
const safeFetch = <T,>(fetcher: () => Promise<T>, fallback: T) => async (): Promise<T> => {
  try {
    return await fetcher()
  } catch (error) {
    console.warn('API fetch failed:', error)
    return fallback
  }
}

/** 필터가 있는 경우의 안전한 wrapper */
const safeFetchWithArg = <T, A>(fetcher: (arg: A) => Promise<T>, fallback: T) => async (arg: A): Promise<T> => {
  try {
    return await fetcher(arg)
  } catch (error) {
    console.warn('API fetch failed:', error)
    return fallback
  }
}

export function TradingJournal() {
  // ==================== createStore 기반 상태 관리 ====================
  const [filters, setFilters] = createStore<FilterState>(initialFilterState)
  const [loading, setLoading] = createStore<LoadingState>(initialLoadingState)
  const [modal, setModal] = createStore<ModalState>(initialModalState)
  const [ui, setUI] = createStore<UIState>(initialUIState)

  // ==================== 모달 헬퍼 함수 ====================

  /** 포지션 상세 모달 열기 */
  const openPositionModal = (position: JournalPosition) => {
    setModal('position', { open: true, data: position })
  }

  /** 포지션 상세 모달 닫기 */
  const closePositionModal = () => {
    setModal('position', { open: false, data: null })
  }

  // 데이터 로드 (에러 발생 시 빈 데이터 반환)
  const [positions, { refetch: refetchPositions }] = createResource(
    safeFetch(getJournalPositions, { positions: [], summary: null })
  )
  const [pnlSummary, { refetch: refetchPnL }] = createResource(
    safeFetch(getJournalPnLSummary, null)
  )
  const [dailyPnL, { refetch: refetchDaily }] = createResource(
    safeFetch(() => getJournalDailyPnL(), { daily: [] })
  )
  const [symbolPnL, { refetch: refetchSymbols }] = createResource(
    safeFetch(getJournalSymbolPnL, { symbols: [] })
  )

  // 기간별 손익 데이터
  const [weeklyPnL, { refetch: refetchWeekly }] = createResource(
    safeFetch(getJournalWeeklyPnL, { weekly: [] })
  )
  const [monthlyPnL, { refetch: refetchMonthly }] = createResource(
    safeFetch(getJournalMonthlyPnL, { monthly: [] })
  )
  const [yearlyPnL, { refetch: refetchYearly }] = createResource(
    safeFetch(getJournalYearlyPnL, { yearly: [] })
  )
  const [cumulativePnL, { refetch: refetchCumulative }] = createResource(
    safeFetch(getJournalCumulativePnL, { curve: [] })
  )

  // 전략 성과 및 인사이트
  const [strategyPerformance, { refetch: refetchStrategies }] = createResource(
    safeFetch(getJournalStrategyPerformance, { strategies: [] })
  )
  const [insights, { refetch: refetchInsights }] = createResource(
    safeFetch(getJournalInsights, null)
  )

  // ==================== 파생 상태 (createMemo) ====================

  /** 체결 내역 필터 (페이지네이션 + 날짜 필터 포함) */
  const executionFilter = createMemo<ExecutionFilter>(() => ({
    symbol: filters.symbol || undefined,
    side: filters.side || undefined,
    start_date: filters.startDate || undefined,
    end_date: filters.endDate || undefined,
    limit: filters.pageSize,
    offset: (filters.currentPage - 1) * filters.pageSize,
  }))

  // ==================== 필터 핸들러 ====================

  /** 필터 변경 시 페이지 자동 초기화 */
  const updateFilter = <K extends keyof FilterState>(key: K, value: FilterState[K]) => {
    setFilters({ [key]: value, currentPage: 1 } as Partial<FilterState>)
  }

  /** 심볼 필터 변경 */
  const handleSymbolFilterChange = (value: string) => updateFilter('symbol', value)

  /** 매매 방향 필터 변경 */
  const handleSideFilterChange = (value: string) => updateFilter('side', value)

  /** 시작일 필터 변경 */
  const handleStartDateChange = (value: string) => updateFilter('startDate', value)

  /** 종료일 필터 변경 */
  const handleEndDateChange = (value: string) => updateFilter('endDate', value)

  /** 페이지 변경 */
  const handlePageChange = (page: number) => setFilters('currentPage', page)

  /** 필터 초기화 */
  const resetFilters = () => setFilters(initialFilterState)

  const [executions, { refetch: refetchExecutions }] = createResource(
    executionFilter,
    safeFetchWithArg(getJournalExecutions, { executions: [] })
  )

  // ==================== 데이터 로드 핸들러 ====================

  /** 새로고침 */
  const handleRefresh = async () => {
    setLoading('isRefreshing', true)
    try {
      await Promise.all([
        refetchPositions(),
        refetchPnL(),
        refetchDaily(),
        refetchSymbols(),
        refetchExecutions(),
        refetchWeekly(),
        refetchMonthly(),
        refetchYearly(),
        refetchCumulative(),
        refetchStrategies(),
        refetchInsights(),
      ])
    } finally {
      setLoading('isRefreshing', false)
    }
  }

  /** 동기화 */
  const handleSync = async (forceFullSync: boolean = false) => {
    setLoading('isSyncing', true)
    try {
      if (forceFullSync) {
        // 강제 동기화: 캐시 초기화 후 전체 내역 조회
        console.log('강제 동기화 시작: 캐시 초기화 포함')
      }
      const result = await syncJournalExecutions(undefined, undefined, forceFullSync)
      if (result.success) {
        await handleRefresh()
      }
    } catch (error) {
      console.error('Sync failed:', error)
    } finally {
      setLoading('isSyncing', false)
    }
  }

  /** 캐시 초기화 */
  const handleClearCache = async () => {
    if (!confirm('캐시를 초기화하시겠습니까?\n\n초기화 후 다음 동기화 시 전체 체결 내역을 다시 조회합니다.')) {
      return
    }
    try {
      const result = await clearJournalCache()
      console.log('캐시 초기화 완료:', result.message)
      alert(`캐시 초기화 완료: ${result.deleted_count}건 삭제`)
    } catch (error) {
      console.error('캐시 초기화 실패:', error)
      alert('캐시 초기화 실패')
    }
  }

  // ==================== UI 컴포넌트 ====================

  /** 액션 버튼 컴포넌트 */
  const HeaderActions = () => (
    <div class="flex items-center gap-3">
      <Button variant="primary" onClick={() => handleSync(false)} disabled={loading.isSyncing} loading={loading.isSyncing}>
        🔄 동기화
      </Button>
      <Button
        variant="secondary"
        onClick={() => handleSync(true)}
        disabled={loading.isSyncing}
        title="캐시를 초기화하고 전체 체결 내역을 다시 조회합니다 (ISA 계좌 등)"
      >
        🔄 강제 동기화
      </Button>
      <Button variant="ghost" onClick={handleClearCache} disabled={loading.isSyncing}>
        🗑️ 캐시 초기화
      </Button>
      <Button variant="secondary" onClick={handleRefresh} disabled={loading.isRefreshing} loading={loading.isRefreshing}>
        🔃 새로고침
      </Button>
    </div>
  )

  return (
    <div class="space-y-6">
      {/* 헤더 - 공통 컴포넌트 사용 */}
      <PageHeader
        title="매매일지"
        icon="📘"
        description="체결 내역과 손익을 분석합니다"
        actions={<HeaderActions />}
      />

      {/* PnL 요약 카드 - 공통 컴포넌트 사용 */}
      <StatCardGrid columns={4}>
        <StatCard
          label="총 실현손익"
          value={pnlSummary() ? formatCurrency(pnlSummary()!.net_pnl) : '-'}
          icon="💰"
          valueColor={getPnLColor(pnlSummary()?.net_pnl || '0')}
        />
        <StatCard
          label="총 거래"
          value={pnlSummary()?.total_trades || 0}
          icon="📊"
        />
        <StatCard
          label="승률"
          value={`${pnlSummary()?.win_rate || '0.00'}%`}
          icon="📈"
        />
        <StatCard
          label="총 수수료"
          value={pnlSummary() ? formatCurrency(pnlSummary()!.total_fees) : '-'}
          icon="⚠️"
          valueColor="text-orange-400"
        />
      </StatCardGrid>

      {/* 탭 네비게이션 (5개로 통합) */}
      <div class="bg-gray-800 rounded-xl">
        <div class="flex overflow-x-auto border-b border-gray-700 scrollbar-thin scrollbar-thumb-gray-700">
          <button
            type="button"
            onClick={() => setUI('activeTab', 'positions')}
            class={`flex items-center gap-2 px-5 py-4 text-sm font-medium transition-colors whitespace-nowrap ${
              ui.activeTab === 'positions'
                ? 'text-blue-400 border-b-2 border-blue-400'
                : 'text-gray-400 hover:text-gray-300'
            }`}
          >
            <BookOpen class="w-4 h-4" />
            보유 현황
          </button>
          <button
            type="button"
            onClick={() => setUI('activeTab', 'executions')}
            class={`flex items-center gap-2 px-5 py-4 text-sm font-medium transition-colors whitespace-nowrap ${
              ui.activeTab === 'executions'
                ? 'text-blue-400 border-b-2 border-blue-400'
                : 'text-gray-400 hover:text-gray-300'
            }`}
          >
            <BarChart3 class="w-4 h-4" />
            체결 내역
          </button>
          <button
            type="button"
            onClick={() => setUI('activeTab', 'pnl-analysis')}
            class={`flex items-center gap-2 px-5 py-4 text-sm font-medium transition-colors whitespace-nowrap ${
              ui.activeTab === 'pnl-analysis'
                ? 'text-green-400 border-b-2 border-green-400'
                : 'text-gray-400 hover:text-gray-300'
            }`}
          >
            <LineChart class="w-4 h-4" />
            손익 분석
          </button>
          <button
            type="button"
            onClick={() => setUI('activeTab', 'symbols')}
            class={`flex items-center gap-2 px-5 py-4 text-sm font-medium transition-colors whitespace-nowrap ${
              ui.activeTab === 'symbols'
                ? 'text-purple-400 border-b-2 border-purple-400'
                : 'text-gray-400 hover:text-gray-300'
            }`}
          >
            <PieChart class="w-4 h-4" />
            종목별
          </button>
          <button
            type="button"
            onClick={() => setUI('activeTab', 'strategy-insights')}
            class={`flex items-center gap-2 px-5 py-4 text-sm font-medium transition-colors whitespace-nowrap ${
              ui.activeTab === 'strategy-insights'
                ? 'text-yellow-400 border-b-2 border-yellow-400'
                : 'text-gray-400 hover:text-gray-300'
            }`}
          >
            <Lightbulb class="w-4 h-4" />
            전략 분석
          </button>
        </div>

        {/* 탭 컨텐츠 */}
        <div class="p-4">
          <Show when={ui.activeTab === 'positions'}>
            <div class="space-y-4">
              {/* 포지션 비중 도넛 차트 (클릭 시 상세 모달) */}
              <PositionDonutChart
                positions={positions()?.positions || []}
                onSymbolClick={openPositionModal}
              />
              {/* 포지션 테이블 (클릭 시 상세 모달) */}
              <PositionsTable
                positions={positions()?.positions || []}
                onRowClick={openPositionModal}
              />
            </div>
          </Show>
          <Show when={ui.activeTab === 'executions'}>
            <ExecutionsTable
              executions={executions()?.executions || []}
              onRefetch={refetchExecutions}
              symbolFilter={filters.symbol}
              setSymbolFilter={handleSymbolFilterChange}
              sideFilter={filters.side}
              setSideFilter={handleSideFilterChange}
              total={executions()?.total || 0}
              currentPage={filters.currentPage}
              pageSize={filters.pageSize}
              onPageChange={handlePageChange}
              startDate={filters.startDate}
              endDate={filters.endDate}
              setStartDate={handleStartDateChange}
              setEndDate={handleEndDateChange}
            />
          </Show>
          <Show when={ui.activeTab === 'pnl-analysis'}>
            <PnLAnalysisPanel
              cumulativeData={cumulativePnL()?.curve || []}
              dailyData={dailyPnL()?.daily || []}
              weeklyData={weeklyPnL()?.weekly || []}
              monthlyData={monthlyPnL()?.monthly || []}
              yearlyData={yearlyPnL()?.yearly || []}
              insights={insights()}
            />
          </Show>
          <Show when={ui.activeTab === 'symbols'}>
            <SymbolPnLTable symbols={symbolPnL()?.symbols || []} />
          </Show>
          <Show when={ui.activeTab === 'strategy-insights'}>
            <StrategyInsightsPanel
              insights={insights() || null}
              strategies={strategyPerformance()?.strategies || []}
            />
          </Show>
        </div>
      </div>

      {/* 포지션 요약 (보유 현황 탭에서만) */}
      <Show when={ui.activeTab === 'positions' && positions()?.summary}>
        <Card padding="lg">
          <CardHeader>
            <h3 class="text-lg font-semibold text-gray-900 dark:text-white">포지션 요약</h3>
          </CardHeader>
          <CardContent>
            <div class="grid grid-cols-2 md:grid-cols-5 gap-4">
              <div>
                <div class="text-gray-500 dark:text-gray-400 text-sm mb-1">보유 종목 수</div>
                <div class="text-gray-900 dark:text-white font-medium">{positions()?.summary.total_positions || 0}</div>
              </div>
              <div>
                <div class="text-gray-500 dark:text-gray-400 text-sm mb-1">총 매입금액</div>
                <div class="text-gray-900 dark:text-white font-medium">
                  {positions()?.summary ? formatCurrency(positions()!.summary.total_cost_basis) : '-'}
                </div>
              </div>
              <div>
                <div class="text-gray-500 dark:text-gray-400 text-sm mb-1">총 평가금액</div>
                <div class="text-gray-900 dark:text-white font-medium">
                  {positions()?.summary ? formatCurrency(positions()!.summary.total_market_value) : '-'}
                </div>
              </div>
              <div>
                <div class="text-gray-500 dark:text-gray-400 text-sm mb-1">평가손익</div>
                <div class={`font-medium ${getPnLColor(parseFloat(positions()?.summary?.total_unrealized_pnl || '0'))}`}>
                  {positions()?.summary ? formatCurrency(positions()!.summary.total_unrealized_pnl) : '-'}
                </div>
              </div>
              <div>
                <div class="text-gray-500 dark:text-gray-400 text-sm mb-1">수익률</div>
                <div class={`font-medium ${getPnLColor(parseFloat(positions()?.summary?.total_unrealized_pnl_pct || '0'))}`}>
                  {positions()?.summary ? `${positions()!.summary.total_unrealized_pnl_pct}%` : '-'}
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      </Show>

      {/* 포지션 상세 모달 */}
      <PositionDetailModal
        isOpen={modal.position.open}
        position={modal.position.data}
        onClose={closePositionModal}
      />
    </div>
  )
}

export default TradingJournal
