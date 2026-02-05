import { createMemo, For, Show } from 'solid-js'
import { createStore } from 'solid-js/store'
import { createQuery, createMutation } from '@tanstack/solid-query'
import {
  ListFilter, Search, TrendingUp, TrendingDown,
  ChevronUp, ChevronDown, Loader2, RefreshCw, Sparkles, Target,
  DollarSign, Percent, Building2, Zap, Settings
} from 'lucide-solid'
import { PageHeader, PageLoader, EmptyState } from '../components/ui'
import { OpportunityMap, KanbanBoard } from '../components/charts'
import type { OpportunitySymbol, KanbanSymbol } from '../components/charts'
import { SymbolDetailModal, WatchlistSelectModal, StrategyLinkModal, PresetModal } from '../components/screening'
import { useToast } from '../components/Toast'
import {
  runScreening,
  getScreeningPresets,
  runPresetScreening,
  runMomentumScreening,
  type ScreeningRequest,
  type ScreeningResultDto,
} from '../api/client'

// ==================== 타입 ====================

type ScreeningTab = 'preset' | 'custom' | 'momentum'
type SortField = 'ticker' | 'name' | 'market_cap' | 'per' | 'pbr' | 'roe' | 'dividend_yield' | 'change_pct'
type SortOrder = 'asc' | 'desc'
type Ma20Position = 'all' | 'above' | 'below'
type FilterMode = 'and' | 'or'
type MacdCross = 'all' | 'golden' | 'dead'
type ViewMode = 'table' | 'map' | 'kanban'

// 커스텀 스크리닝 필터 (서버 전송용)
interface CustomFilterState {
  market: string
  min_per: string
  max_per: string
  min_pbr: string
  max_pbr: string
  min_roe: string
  max_roe: string
  min_dividend_yield: string
  max_debt_ratio: string
  min_revenue_growth: string
  min_earnings_growth: string
  max_distance_from_52w_high: string
  sort_by: string
  sort_order: string
  limit: number
}

// 클라이언트 사이드 필터 상태
interface ClientFilterState {
  presetMarket: string
  momentumDays: number
  momentumMinChange: string
  momentumMarket: string
  selectedRouteStates: string[]
  rsiMin: string
  rsiMax: string
  selectedSectors: string[]
  marketCapMin: string
  marketCapMax: string
  distMa20Min: string
  distMa20Max: string
  ma20Position: Ma20Position
  filterMode: FilterMode
  macdCrossFilter: MacdCross
  sortField: SortField
  sortOrder: SortOrder
  currentPage: number
}

// UI 상태
interface UIState {
  activeTab: ScreeningTab
  selectedPreset: string
  viewMode: ViewMode
  showSectorPanel: boolean
}

// 모달 상태
interface ModalState {
  symbolDetail: {
    open: boolean
    symbol: ScreeningResultDto | null
  }
  watchlist: {
    open: boolean
    ticker: string
    market: string
  }
  strategyLink: {
    open: boolean
    symbol: string
  }
  preset: {
    open: boolean
  }
}

// ==================== 초기 상태 ====================

const DEFAULT_CUSTOM_FILTER: CustomFilterState = {
  market: '',
  min_per: '',
  max_per: '',
  min_pbr: '',
  max_pbr: '',
  min_roe: '',
  max_roe: '',
  min_dividend_yield: '',
  max_debt_ratio: '',
  min_revenue_growth: '',
  min_earnings_growth: '',
  max_distance_from_52w_high: '',
  sort_by: 'market_cap',
  sort_order: 'desc',
  limit: 50,
}

const initialClientFilter: ClientFilterState = {
  presetMarket: '',
  momentumDays: 5,
  momentumMinChange: '5',
  momentumMarket: '',
  selectedRouteStates: [],
  rsiMin: '',
  rsiMax: '',
  selectedSectors: [],
  marketCapMin: '',
  marketCapMax: '',
  distMa20Min: '',
  distMa20Max: '',
  ma20Position: 'all',
  filterMode: 'and',
  macdCrossFilter: 'all',
  sortField: 'market_cap',
  sortOrder: 'desc',
  currentPage: 1,
}

const initialUIState: UIState = {
  activeTab: 'preset',
  selectedPreset: 'basic',
  viewMode: 'table',
  showSectorPanel: false,
}

const initialModalState: ModalState = {
  symbolDetail: { open: false, symbol: null },
  watchlist: { open: false, ticker: '', market: '' },
  strategyLink: { open: false, symbol: '' },
  preset: { open: false },
}

// 프리셋 ID -> 표시 이름 매핑
const PRESET_LABELS: Record<string, { name: string; icon: typeof DollarSign; description: string }> = {
  basic: { name: '전체', icon: ListFilter, description: '필터 없이 모든 종목 조회' },
  value: { name: '가치주', icon: DollarSign, description: '저 PER, 저 PBR 종목' },
  dividend: { name: '배당주', icon: Percent, description: '고배당 수익률 종목' },
  growth: { name: '성장주', icon: TrendingUp, description: '높은 매출/이익 성장률' },
  snowball: { name: '스노우볼', icon: Sparkles, description: '저 PBR + 고배당' },
  large_cap: { name: '대형주', icon: Building2, description: '시가총액 상위 종목' },
  near_52w_low: { name: '52주 저점', icon: TrendingDown, description: '52주 저점 근접 종목' },
}

// 시장 필터 옵션
const MARKET_OPTIONS: { value: string; label: string; emoji: string; indent?: boolean }[] = [
  { value: '', label: '전체', emoji: '🌐' },
  { value: 'KR', label: '한국 전체', emoji: '🇰🇷' },
  { value: 'KR-KOSPI', label: 'KOSPI', emoji: '📈', indent: true },
  { value: 'KR-KOSDAQ', label: 'KOSDAQ', emoji: '📊', indent: true },
  { value: 'US', label: '미국', emoji: '🇺🇸' },
  { value: 'CRYPTO', label: '암호화폐', emoji: '₿' },
]

// RouteState 필터 옵션
const ROUTE_STATE_OPTIONS = [
  { value: 'Attack', label: 'ATTACK', bg: 'bg-red-500/20', text: 'text-red-400', emoji: '🚀' },
  { value: 'Armed', label: 'ARMED', bg: 'bg-orange-500/20', text: 'text-orange-400', emoji: '⚡' },
  { value: 'Watch', label: 'WATCH', bg: 'bg-yellow-500/20', text: 'text-yellow-400', emoji: '👀' },
  { value: 'Rest', label: 'REST', bg: 'bg-gray-500/20', text: 'text-gray-400', emoji: '😴' },
] as const

// RouteState 뱃지 스타일
const ROUTE_STATE_STYLES: Record<string, { bg: string; text: string; label: string }> = {
  Attack: { bg: 'bg-red-500/20', text: 'text-red-400', label: 'ATTACK' },
  Armed: { bg: 'bg-orange-500/20', text: 'text-orange-400', label: 'ARMED' },
  Watch: { bg: 'bg-yellow-500/20', text: 'text-yellow-400', label: 'WATCH' },
  Rest: { bg: 'bg-gray-500/20', text: 'text-gray-400', label: 'REST' },
}

// 등급 뱃지 스타일
const GRADE_STYLES: Record<string, { bg: string; text: string }> = {
  S: { bg: 'bg-purple-500/20', text: 'text-purple-400' },
  A: { bg: 'bg-blue-500/20', text: 'text-blue-400' },
  B: { bg: 'bg-green-500/20', text: 'text-green-400' },
  C: { bg: 'bg-yellow-500/20', text: 'text-yellow-400' },
  D: { bg: 'bg-orange-500/20', text: 'text-orange-400' },
  F: { bg: 'bg-red-500/20', text: 'text-red-400' },
}

// 한국 시장 섹터 목록
const SECTOR_OPTIONS = [
  { value: '반도체', label: '반도체', emoji: '💻' },
  { value: '2차전지', label: '2차전지', emoji: '🔋' },
  { value: '바이오', label: '바이오', emoji: '🧬' },
  { value: '자동차', label: '자동차', emoji: '🚗' },
  { value: 'IT', label: 'IT', emoji: '🌐' },
  { value: '금융', label: '금융', emoji: '🏦' },
  { value: '건설', label: '건설', emoji: '🏗️' },
  { value: '화학', label: '화학', emoji: '⚗️' },
  { value: '철강', label: '철강', emoji: '⚙️' },
  { value: '유통', label: '유통', emoji: '🛒' },
  { value: '음식료', label: '음식료', emoji: '🍔' },
  { value: '기계', label: '기계', emoji: '🔧' },
  { value: '전기전자', label: '전기전자', emoji: '⚡' },
  { value: '의약품', label: '의약품', emoji: '💊' },
  { value: '섬유', label: '섬유', emoji: '👕' },
  { value: '통신', label: '통신', emoji: '📡' },
] as const

// 시가총액 프리셋 (억 단위)
const MARKET_CAP_PRESETS = [
  { label: '소형주', min: 0, max: 1000, description: '1,000억 미만' },
  { label: '중소형', min: 1000, max: 5000, description: '1,000~5,000억' },
  { label: '중형주', min: 5000, max: 10000, description: '5,000억~1조' },
  { label: '대형주', min: 10000, max: null, description: '1조 이상' },
] as const

// ==================== 메인 컴포넌트 ====================

export function Screening() {
  const toast = useToast()
  const pageSize = 20

  // ==================== Store 기반 상태 관리 ====================
  const [customFilter, setCustomFilter] = createStore<CustomFilterState>({ ...DEFAULT_CUSTOM_FILTER })
  const [filters, setFilters] = createStore<ClientFilterState>({ ...initialClientFilter })
  const [ui, setUI] = createStore<UIState>({ ...initialUIState })
  const [modal, setModal] = createStore<ModalState>({ ...initialModalState })

  // ==================== 헬퍼 함수 ====================

  // 필터 업데이트 헬퍼
  const updateFilter = <K extends keyof ClientFilterState>(key: K, value: ClientFilterState[K]) => {
    setFilters({ [key]: value, currentPage: 1 } as Partial<ClientFilterState>)
  }

  // 커스텀 필터 업데이트 헬퍼
  const updateCustomFilter = <K extends keyof CustomFilterState>(key: K, value: CustomFilterState[K]) => {
    setCustomFilter({ [key]: value } as Partial<CustomFilterState>)
  }

  // 모달 헬퍼
  const openSymbolDetailModal = (symbol: ScreeningResultDto) => {
    setModal('symbolDetail', { open: true, symbol })
  }

  const closeSymbolDetailModal = () => {
    setModal('symbolDetail', { open: false, symbol: null })
  }

  const openWatchlistModal = (ticker: string, market: string) => {
    setModal('watchlist', { open: true, ticker, market })
  }

  const closeWatchlistModal = () => {
    setModal('watchlist', { open: false, ticker: '', market: '' })
  }

  const openStrategyLinkModal = (symbol: string) => {
    setModal('strategyLink', { open: true, symbol })
  }

  const closeStrategyLinkModal = () => {
    setModal('strategyLink', { open: false, symbol: '' })
  }

  const openPresetModal = () => {
    setModal('preset', { open: true })
  }

  const closePresetModal = () => {
    setModal('preset', { open: false })
  }

  // 필터 초기화
  const resetAllFilters = () => {
    setFilters({
      selectedRouteStates: [],
      rsiMin: '',
      rsiMax: '',
      selectedSectors: [],
      marketCapMin: '',
      marketCapMax: '',
      distMa20Min: '',
      distMa20Max: '',
      ma20Position: 'all',
      macdCrossFilter: 'all',
      filterMode: 'and',
      currentPage: 1,
    })
  }

  const resetCustomFilter = () => {
    setCustomFilter({ ...DEFAULT_CUSTOM_FILTER })
  }

  // 섹터 토글
  const toggleSector = (sector: string) => {
    const current = filters.selectedSectors
    const newSectors = current.includes(sector)
      ? current.filter(s => s !== sector)
      : [...current, sector]
    setFilters({ selectedSectors: newSectors, currentPage: 1 })
  }

  // RouteState 토글
  const toggleRouteState = (state: string) => {
    const current = filters.selectedRouteStates
    const newStates = current.includes(state)
      ? current.filter(s => s !== state)
      : [...current, state]
    setFilters({ selectedRouteStates: newStates, currentPage: 1 })
  }

  // ==================== 쿼리 ====================

  // 프리셋 목록 조회
  const presetsQuery = createQuery(() => ({
    queryKey: ['screening-presets'],
    queryFn: getScreeningPresets,
    staleTime: 1000 * 60 * 5, // 5분
  }))

  // 프리셋 스크리닝 결과
  const presetScreeningQuery = createQuery(() => ({
    queryKey: ['screening-preset', ui.selectedPreset, filters.presetMarket],
    queryFn: () => runPresetScreening(ui.selectedPreset, filters.presetMarket || undefined, 100),
    enabled: ui.activeTab === 'preset',
  }))

  // 커스텀 스크리닝 뮤테이션
  const customScreeningMutation = createMutation(() => ({
    mutationFn: (request: ScreeningRequest) => runScreening(request),
    onSuccess: () => {
      toast.success('스크리닝 완료', '필터 조건에 맞는 종목을 조회했습니다.')
    },
    onError: (error: Error) => {
      toast.error('스크리닝 실패', error.message)
    },
  }))

  // 모멘텀 스크리닝 쿼리
  const momentumQuery = createQuery(() => ({
    queryKey: ['screening-momentum', filters.momentumDays, filters.momentumMinChange, filters.momentumMarket],
    queryFn: () => runMomentumScreening({
      days: filters.momentumDays,
      min_change_pct: filters.momentumMinChange,
      market: filters.momentumMarket || undefined,
      limit: 100,
    }),
    enabled: ui.activeTab === 'momentum',
  }))

  // ==================== 계산된 값 ====================

  // 현재 활성 데이터
  const currentResults = createMemo((): ScreeningResultDto[] => {
    if (ui.activeTab === 'preset') {
      return presetScreeningQuery.data?.results || []
    } else if (ui.activeTab === 'custom') {
      return customScreeningMutation.data?.results || []
    }
    return []
  })

  // 정렬된 결과 (모든 필터 적용)
  const sortedResults = createMemo(() => {
    let results = [...currentResults()]
    const { sortField, sortOrder, selectedRouteStates, filterMode, rsiMin, rsiMax,
      selectedSectors, marketCapMin, marketCapMax, distMa20Min, distMa20Max,
      ma20Position, macdCrossFilter } = filters

    // 필터 조건들을 함수 배열로 수집
    const filterConditions: ((r: ScreeningResultDto) => boolean)[] = []

    // RouteState 필터
    if (selectedRouteStates.length > 0) {
      filterConditions.push(r => r.route_state ? selectedRouteStates.includes(r.route_state) : false)
    }

    // RSI 필터 (클라이언트 사이드)
    const minRsi = rsiMin ? parseFloat(rsiMin) : null
    const maxRsi = rsiMax ? parseFloat(rsiMax) : null
    if (minRsi !== null || maxRsi !== null) {
      filterConditions.push(r => {
        const rsi = r.rsi_14 ? parseFloat(r.rsi_14) : null
        if (rsi === null) return false
        if (minRsi !== null && rsi < minRsi) return false
        if (maxRsi !== null && rsi > maxRsi) return false
        return true
      })
    }

    // 섹터 필터
    if (selectedSectors.length > 0) {
      filterConditions.push(r => r.sector ? selectedSectors.some(s => r.sector?.includes(s)) : false)
    }

    // 시가총액 필터 (억 단위로 입력, 원화 기준)
    const minCap = marketCapMin ? parseFloat(marketCapMin) * 100000000 : null
    const maxCap = marketCapMax ? parseFloat(marketCapMax) * 100000000 : null
    if (minCap !== null || maxCap !== null) {
      filterConditions.push(r => {
        const cap = r.market_cap ? parseFloat(r.market_cap) : null
        if (cap === null) return false
        if (minCap !== null && cap < minCap) return false
        if (maxCap !== null && cap > maxCap) return false
        return true
      })
    }

    // 20일선 이격도 필터
    const minDist = distMa20Min ? parseFloat(distMa20Min) : null
    const maxDist = distMa20Max ? parseFloat(distMa20Max) : null
    if (minDist !== null || maxDist !== null || ma20Position !== 'all') {
      filterConditions.push(r => {
        const dist = r.dist_ma20 ? parseFloat(r.dist_ma20) : null
        if (dist === null) return ma20Position === 'all' // 데이터 없으면 all일때만 통과
        // 위치 필터
        if (ma20Position === 'above' && dist < 0) return false
        if (ma20Position === 'below' && dist > 0) return false
        // 범위 필터
        if (minDist !== null && dist < minDist) return false
        if (maxDist !== null && dist > maxDist) return false
        return true
      })
    }

    // MACD 크로스 필터
    if (macdCrossFilter !== 'all') {
      filterConditions.push(r => r.macd_cross === macdCrossFilter)
    }

    // AND/OR 조건 적용
    if (filterConditions.length > 0) {
      if (filterMode === 'and') {
        results = results.filter(r => filterConditions.every(fn => fn(r)))
      } else {
        results = results.filter(r => filterConditions.some(fn => fn(r)))
      }
    }

    results.sort((a, b) => {
      let aVal: number | string = 0
      let bVal: number | string = 0

      switch (sortField) {
        case 'ticker':
          aVal = a.ticker
          bVal = b.ticker
          break
        case 'name':
          aVal = a.name
          bVal = b.name
          break
        case 'market_cap':
          aVal = parseFloat(a.market_cap || '0')
          bVal = parseFloat(b.market_cap || '0')
          break
        case 'per':
          aVal = parseFloat(a.per || '9999')
          bVal = parseFloat(b.per || '9999')
          break
        case 'pbr':
          aVal = parseFloat(a.pbr || '9999')
          bVal = parseFloat(b.pbr || '9999')
          break
        case 'roe':
          aVal = parseFloat(a.roe || '-9999')
          bVal = parseFloat(b.roe || '-9999')
          break
        case 'dividend_yield':
          aVal = parseFloat(a.dividend_yield || '0')
          bVal = parseFloat(b.dividend_yield || '0')
          break
      }

      if (typeof aVal === 'string' && typeof bVal === 'string') {
        return sortOrder === 'asc' ? aVal.localeCompare(bVal) : bVal.localeCompare(aVal)
      }
      return sortOrder === 'asc' ? (aVal as number) - (bVal as number) : (bVal as number) - (aVal as number)
    })

    return results
  })

  // 페이지네이션된 결과
  const paginatedResults = createMemo(() => {
    const start = (filters.currentPage - 1) * pageSize
    return sortedResults().slice(start, start + pageSize)
  })

  const totalPages = createMemo(() => Math.ceil(sortedResults().length / pageSize))

  // 로딩 상태
  const isLoading = createMemo(() => {
    if (ui.activeTab === 'preset') return presetScreeningQuery.isLoading
    if (ui.activeTab === 'custom') return customScreeningMutation.isPending
    if (ui.activeTab === 'momentum') return momentumQuery.isLoading
    return false
  })

  // OpportunityMap용 데이터 변환
  const opportunityMapData = createMemo((): OpportunitySymbol[] => {
    return sortedResults().map(r => {
      // RouteState 변환 (DB: ATTACK/ARMED/WAIT/OVERHEAT/NEUTRAL → UI)
      let routeState: 'ATTACK' | 'ARMED' | 'WATCH' | 'AVOID' | 'UNKNOWN' = 'UNKNOWN'
      const dbState = r.route_state?.toUpperCase()
      if (dbState === 'ATTACK') routeState = 'ATTACK'
      else if (dbState === 'ARMED') routeState = 'ARMED'
      else if (dbState === 'WAIT' || dbState === 'WATCH') routeState = 'WATCH'
      else if (dbState === 'OVERHEAT' || dbState === 'REST' || dbState === 'NEUTRAL') routeState = 'AVOID'

      return {
        symbol: r.ticker,
        totalScore: r.global_score ? parseFloat(r.global_score) : 50,
        triggerScore: r.trigger_score ? parseFloat(r.trigger_score) : 50,
        routeState,
        name: r.name,
        size: r.market_cap ? parseFloat(r.market_cap) / 1e11 : 1, // 천억 단위로 정규화
      }
    })
  })

  // KanbanBoard용 데이터 변환
  const kanbanBoardData = createMemo((): KanbanSymbol[] => {
    return sortedResults()
      .filter(r => {
        const dbState = r.route_state?.toUpperCase()
        return dbState && ['ATTACK', 'ARMED', 'WAIT', 'WATCH'].includes(dbState)
      })
      .map(r => {
        const dbState = r.route_state?.toUpperCase()
        let routeState: 'ATTACK' | 'ARMED' | 'WATCH' = 'WATCH'
        if (dbState === 'ATTACK') routeState = 'ATTACK'
        else if (dbState === 'ARMED') routeState = 'ARMED'

        return {
          symbol: r.ticker,
          name: r.name,
          routeState,
          score: r.global_score ? parseFloat(r.global_score) : 0,
          price: r.current_price ? parseFloat(r.current_price) : undefined,
          changeRate: r.change_pct ? parseFloat(r.change_pct) : undefined,
        }
      })
  })

  // ==================== 핸들러 ====================

  const handleSort = (field: SortField) => {
    if (filters.sortField === field) {
      setFilters('sortOrder', filters.sortOrder === 'asc' ? 'desc' : 'asc')
    } else {
      setFilters({ sortField: field, sortOrder: 'desc' })
    }
    setFilters('currentPage', 1)
  }

  const handlePresetChange = (preset: string) => {
    setUI('selectedPreset', preset)
    setFilters('currentPage', 1)
  }

  const handleCustomScreening = () => {
    const request: ScreeningRequest = {
      market: customFilter.market || undefined,
      min_per: customFilter.min_per || undefined,
      max_per: customFilter.max_per || undefined,
      min_pbr: customFilter.min_pbr || undefined,
      max_pbr: customFilter.max_pbr || undefined,
      min_roe: customFilter.min_roe || undefined,
      max_roe: customFilter.max_roe || undefined,
      min_dividend_yield: customFilter.min_dividend_yield || undefined,
      max_debt_ratio: customFilter.max_debt_ratio || undefined,
      min_revenue_growth: customFilter.min_revenue_growth || undefined,
      min_earnings_growth: customFilter.min_earnings_growth || undefined,
      max_distance_from_52w_high: customFilter.max_distance_from_52w_high || undefined,
      sort_by: customFilter.sort_by || undefined,
      sort_order: customFilter.sort_order || undefined,
      limit: customFilter.limit,
    }
    customScreeningMutation.mutate(request)
    setFilters('currentPage', 1)
  }

  // 숫자 포맷팅
  const formatNumber = (value: string | null | undefined, decimals: number = 2): string => {
    if (!value) return '-'
    const num = parseFloat(value)
    if (isNaN(num)) return '-'
    return num.toLocaleString('ko-KR', { maximumFractionDigits: decimals })
  }

  const formatMarketCap = (value: string | null | undefined): string => {
    if (!value) return '-'
    const num = parseFloat(value)
    if (isNaN(num)) return '-'
    if (num >= 1e12) return `${(num / 1e12).toFixed(1)}조`
    if (num >= 1e8) return `${(num / 1e8).toFixed(0)}억`
    if (num >= 1e4) return `${(num / 1e4).toFixed(0)}만`
    return num.toLocaleString()
  }

  const formatPercent = (value: string | null | undefined): string => {
    if (!value) return '-'
    const num = parseFloat(value)
    if (isNaN(num)) return '-'
    return `${num >= 0 ? '+' : ''}${num.toFixed(2)}%`
  }

  // ==================== 렌더링 ====================

  return (
    <div class="h-full flex flex-col">
      {/* 헤더 - 공통 컴포넌트 사용 */}
      <PageHeader
        title="종목 스크리닝"
        icon="🔍"
        description="펀더멘털 및 모멘텀 기반 종목 필터링"
      />

      {/* 탭 선택 */}
      <div class="flex gap-1 bg-[var(--color-surface)] rounded-lg p-1 mb-4 w-fit">
        <button
          onClick={() => setUI('activeTab', 'preset')}
          class={`px-4 py-2 text-sm rounded-md flex items-center gap-2 transition
                  ${ui.activeTab === 'preset'
                    ? 'bg-[var(--color-primary)] text-white'
                    : 'text-[var(--color-text-muted)] hover:bg-[var(--color-surface-light)]'}`}
        >
          <Sparkles class="w-4 h-4" />
          프리셋
        </button>
        <button
          onClick={() => setUI('activeTab', 'custom')}
          class={`px-4 py-2 text-sm rounded-md flex items-center gap-2 transition
                  ${ui.activeTab === 'custom'
                    ? 'bg-[var(--color-primary)] text-white'
                    : 'text-[var(--color-text-muted)] hover:bg-[var(--color-surface-light)]'}`}
        >
          <ListFilter class="w-4 h-4" />
          커스텀 필터
        </button>
        <button
          onClick={() => setUI('activeTab', 'momentum')}
          class={`px-4 py-2 text-sm rounded-md flex items-center gap-2 transition
                  ${ui.activeTab === 'momentum'
                    ? 'bg-[var(--color-primary)] text-white'
                    : 'text-[var(--color-text-muted)] hover:bg-[var(--color-surface-light)]'}`}
        >
          <Zap class="w-4 h-4" />
          모멘텀
        </button>
      </div>

      {/* 프리셋 탭 */}
      <Show when={ui.activeTab === 'preset'}>
        <div class="bg-[var(--color-surface)] rounded-xl p-4 mb-4">
          <div class="flex items-center gap-4 mb-4">
            <span class="text-sm text-[var(--color-text-muted)]">프리셋 선택:</span>
            <div class="flex flex-wrap gap-2">
              <For each={presetsQuery.data?.presets || Object.keys(PRESET_LABELS).map(id => ({ id, name: PRESET_LABELS[id].name, description: PRESET_LABELS[id].description }))}>
                {(preset) => {
                  const info = PRESET_LABELS[preset.id] || { name: preset.name, icon: Target, description: preset.description }
                  const Icon = info.icon
                  return (
                    <button
                      onClick={() => handlePresetChange(preset.id)}
                      class={`px-4 py-2 rounded-lg flex items-center gap-2 transition text-sm
                              ${ui.selectedPreset === preset.id
                                ? 'bg-[var(--color-primary)] text-white'
                                : 'bg-[var(--color-surface-light)] text-[var(--color-text)] hover:bg-[var(--color-primary)]/20'}`}
                      title={info.description}
                    >
                      <Icon class="w-4 h-4" />
                      {info.name}
                    </button>
                  )
                }}
              </For>
              {/* 프리셋 관리 버튼 */}
              <button
                onClick={openPresetModal}
                class="px-3 py-2 rounded-lg flex items-center gap-2 transition text-sm bg-[var(--color-surface-light)] text-[var(--color-text-muted)] hover:text-[var(--color-text)] hover:bg-[var(--color-surface-light)]/80"
                title="프리셋 저장/삭제"
              >
                <Settings class="w-4 h-4" />
              </button>
            </div>
          </div>
          {/* 필터 행: 시장 + RouteState + 새로고침 */}
          <div class="flex items-center gap-6 flex-wrap">
            {/* 시장 필터 (버튼 그룹) */}
            <div class="flex items-center gap-2">
              <span class="text-sm text-[var(--color-text-muted)]">시장:</span>
              <div class="flex gap-1">
                <For each={MARKET_OPTIONS}>
                  {(option) => (
                    <button
                      onClick={() => setFilters('presetMarket', option.value)}
                      class={`px-3 py-1.5 text-sm rounded-lg transition flex items-center gap-1.5
                              ${option.indent ? 'ml-1' : ''}
                              ${filters.presetMarket === option.value
                                ? 'bg-[var(--color-primary)] text-white'
                                : 'bg-[var(--color-surface-light)] text-[var(--color-text-muted)] hover:bg-[var(--color-primary)]/20'}`}
                    >
                      <span>{option.emoji}</span>
                      <span>{option.label}</span>
                    </button>
                  )}
                </For>
              </div>
            </div>

            {/* RouteState 필터 (다중 선택) */}
            <div class="flex items-center gap-2">
              <span class="text-sm text-[var(--color-text-muted)]">상태:</span>
              <div class="flex gap-1">
                <For each={ROUTE_STATE_OPTIONS}>
                  {(option) => {
                    const isSelected = () => filters.selectedRouteStates.includes(option.value)
                    return (
                      <button
                        onClick={() => toggleRouteState(option.value)}
                        class={`px-3 py-1.5 text-sm rounded-lg transition flex items-center gap-1.5
                                ${isSelected()
                                  ? `${option.bg} ${option.text} ring-1 ring-current`
                                  : 'bg-[var(--color-surface-light)] text-[var(--color-text-muted)] hover:bg-[var(--color-surface)]'}`}
                      >
                        <span>{option.emoji}</span>
                        <span>{option.label}</span>
                      </button>
                    )
                  }}
                </For>
                <Show when={filters.selectedRouteStates.length > 0}>
                  <button
                    onClick={() => setFilters({ selectedRouteStates: [], currentPage: 1 })}
                    class="px-2 py-1.5 text-xs text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition"
                    title="필터 초기화"
                  >
                    ✕
                  </button>
                  <span class="text-xs text-[var(--color-primary)]">
                    {filters.selectedRouteStates.length}개 선택
                  </span>
                </Show>
              </div>
            </div>

            {/* RSI 필터 */}
            <div class="flex items-center gap-2">
              <span class="text-sm text-[var(--color-text-muted)]">RSI:</span>
              <input
                type="number"
                value={filters.rsiMin}
                onInput={(e) => updateFilter('rsiMin', e.currentTarget.value)}
                placeholder="최소"
                min="0"
                max="100"
                class="w-16 px-2 py-1.5 text-sm bg-[var(--color-bg)] text-[var(--color-text)]
                       rounded-lg border border-[var(--color-surface-light)]"
              />
              <span class="text-[var(--color-text-muted)]">~</span>
              <input
                type="number"
                value={filters.rsiMax}
                onInput={(e) => updateFilter('rsiMax', e.currentTarget.value)}
                placeholder="최대"
                min="0"
                max="100"
                class="w-16 px-2 py-1.5 text-sm bg-[var(--color-bg)] text-[var(--color-text)]
                       rounded-lg border border-[var(--color-surface-light)]"
              />
              <Show when={filters.rsiMin || filters.rsiMax}>
                <button
                  onClick={() => setFilters({ rsiMin: '', rsiMax: '', currentPage: 1 })}
                  class="px-2 py-1.5 text-xs text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition"
                  title="RSI 필터 초기화"
                >
                  ✕
                </button>
              </Show>
            </div>

            {/* AND/OR 토글 */}
            <div class="flex items-center gap-2">
              <button
                onClick={() => setFilters('filterMode', filters.filterMode === 'and' ? 'or' : 'and')}
                class={`px-3 py-1.5 text-sm rounded-lg transition flex items-center gap-1.5
                        ${filters.filterMode === 'and'
                          ? 'bg-blue-500/20 text-blue-400 ring-1 ring-blue-500/50'
                          : 'bg-orange-500/20 text-orange-400 ring-1 ring-orange-500/50'}`}
                title={filters.filterMode === 'and' ? '모든 조건 만족' : '하나라도 만족'}
              >
                <span class="font-medium">{filters.filterMode.toUpperCase()}</span>
              </button>
            </div>

            {/* 새로고침 버튼 */}
            <button
              onClick={() => presetScreeningQuery.refetch()}
              disabled={presetScreeningQuery.isFetching}
              class="px-3 py-1.5 bg-[var(--color-primary)] text-white rounded-lg text-sm
                     hover:bg-[var(--color-primary-dark)] transition flex items-center gap-2 disabled:opacity-50"
            >
              <RefreshCw class={`w-4 h-4 ${presetScreeningQuery.isFetching ? 'animate-spin' : ''}`} />
              새로고침
            </button>
          </div>
          {/* 확장 필터 행 (섹터, 시가총액, 이격도) */}
          <div class="flex items-center gap-6 flex-wrap mt-4 pt-4 border-t border-[var(--color-surface-light)]">
            {/* 섹터 multi_select */}
            <div class="flex items-center gap-2 relative">
              <span class="text-sm text-[var(--color-text-muted)]">섹터:</span>
              <button
                onClick={() => setUI('showSectorPanel', !ui.showSectorPanel)}
                class={`px-3 py-1.5 text-sm rounded-lg transition flex items-center gap-2
                        ${filters.selectedSectors.length > 0
                          ? 'bg-[var(--color-primary)]/20 text-[var(--color-primary)] ring-1 ring-[var(--color-primary)]/50'
                          : 'bg-[var(--color-surface-light)] text-[var(--color-text-muted)] hover:bg-[var(--color-surface)]'}`}
              >
                <span>{filters.selectedSectors.length > 0 ? `${filters.selectedSectors.length}개 선택` : '전체'}</span>
                <ChevronDown class={`w-3 h-3 transition-transform ${ui.showSectorPanel ? 'rotate-180' : ''}`} />
              </button>
              {/* 섹터 선택 패널 */}
              <Show when={ui.showSectorPanel}>
                <div class="absolute top-full left-0 mt-2 z-50 bg-[var(--color-surface)] border border-[var(--color-surface-light)] rounded-lg shadow-lg p-3 min-w-[280px]">
                  <div class="grid grid-cols-2 gap-2">
                    <For each={SECTOR_OPTIONS}>
                      {(option) => {
                        const isSelected = () => filters.selectedSectors.includes(option.value)
                        return (
                          <label class="flex items-center gap-2 cursor-pointer hover:bg-[var(--color-surface-light)] rounded px-2 py-1.5">
                            <input
                              type="checkbox"
                              checked={isSelected()}
                              onChange={() => toggleSector(option.value)}
                              class="w-4 h-4 accent-[var(--color-primary)]"
                            />
                            <span class="text-sm">{option.emoji} {option.label}</span>
                          </label>
                        )
                      }}
                    </For>
                  </div>
                  <div class="flex justify-between mt-3 pt-2 border-t border-[var(--color-surface-light)]">
                    <button
                      onClick={() => setFilters({ selectedSectors: [], currentPage: 1 })}
                      class="text-xs text-[var(--color-text-muted)] hover:text-[var(--color-text)]"
                    >
                      초기화
                    </button>
                    <button
                      onClick={() => setUI('showSectorPanel', false)}
                      class="text-xs text-[var(--color-primary)] hover:text-[var(--color-primary-light)]"
                    >
                      닫기
                    </button>
                  </div>
                </div>
              </Show>
              <Show when={filters.selectedSectors.length > 0}>
                <button
                  onClick={() => setFilters({ selectedSectors: [], currentPage: 1 })}
                  class="text-xs text-[var(--color-text-muted)] hover:text-[var(--color-text)]"
                >
                  ✕
                </button>
              </Show>
            </div>

            {/* 시가총액 range (억 단위) */}
            <div class="flex items-center gap-2">
              <span class="text-sm text-[var(--color-text-muted)]">시총(억):</span>
              <div class="flex gap-1">
                <For each={MARKET_CAP_PRESETS}>
                  {(preset) => {
                    const isActive = () => {
                      const min = filters.marketCapMin ? parseFloat(filters.marketCapMin) : null
                      const max = filters.marketCapMax ? parseFloat(filters.marketCapMax) : null
                      return min === preset.min && (preset.max === null ? max === null : max === preset.max)
                    }
                    return (
                      <button
                        onClick={() => {
                          setFilters({
                            marketCapMin: preset.min.toString(),
                            marketCapMax: preset.max !== null ? preset.max.toString() : '',
                            currentPage: 1
                          })
                        }}
                        class={`px-2 py-1 text-xs rounded transition
                                ${isActive()
                                  ? 'bg-[var(--color-primary)] text-white'
                                  : 'bg-[var(--color-surface-light)] text-[var(--color-text-muted)] hover:bg-[var(--color-surface)]'}`}
                        title={preset.description}
                      >
                        {preset.label}
                      </button>
                    )
                  }}
                </For>
              </div>
              <input
                type="number"
                value={filters.marketCapMin}
                onInput={(e) => updateFilter('marketCapMin', e.currentTarget.value)}
                placeholder="최소"
                class="w-20 px-2 py-1.5 text-sm bg-[var(--color-bg)] text-[var(--color-text)]
                       rounded-lg border border-[var(--color-surface-light)]"
              />
              <span class="text-[var(--color-text-muted)]">~</span>
              <input
                type="number"
                value={filters.marketCapMax}
                onInput={(e) => updateFilter('marketCapMax', e.currentTarget.value)}
                placeholder="최대"
                class="w-20 px-2 py-1.5 text-sm bg-[var(--color-bg)] text-[var(--color-text)]
                       rounded-lg border border-[var(--color-surface-light)]"
              />
              <Show when={filters.marketCapMin || filters.marketCapMax}>
                <button
                  onClick={() => setFilters({ marketCapMin: '', marketCapMax: '', currentPage: 1 })}
                  class="text-xs text-[var(--color-text-muted)] hover:text-[var(--color-text)]"
                >
                  ✕
                </button>
              </Show>
            </div>

            {/* 20일선 이격도 필터 */}
            <div class="flex items-center gap-2">
              <span class="text-sm text-[var(--color-text-muted)]">20일선:</span>
              <div class="flex gap-1">
                <button
                  onClick={() => setFilters({ ma20Position: 'all', currentPage: 1 })}
                  class={`px-2 py-1 text-xs rounded transition
                          ${filters.ma20Position === 'all'
                            ? 'bg-[var(--color-primary)] text-white'
                            : 'bg-[var(--color-surface-light)] text-[var(--color-text-muted)]'}`}
                >
                  전체
                </button>
                <button
                  onClick={() => setFilters({ ma20Position: 'above', currentPage: 1 })}
                  class={`px-2 py-1 text-xs rounded transition
                          ${filters.ma20Position === 'above'
                            ? 'bg-green-500/30 text-green-400'
                            : 'bg-[var(--color-surface-light)] text-[var(--color-text-muted)]'}`}
                >
                  위 ↑
                </button>
                <button
                  onClick={() => setFilters({ ma20Position: 'below', currentPage: 1 })}
                  class={`px-2 py-1 text-xs rounded transition
                          ${filters.ma20Position === 'below'
                            ? 'bg-red-500/30 text-red-400'
                            : 'bg-[var(--color-surface-light)] text-[var(--color-text-muted)]'}`}
                >
                  아래 ↓
                </button>
              </div>
              <input
                type="number"
                step="0.1"
                value={filters.distMa20Min}
                onInput={(e) => updateFilter('distMa20Min', e.currentTarget.value)}
                placeholder="최소%"
                class="w-16 px-2 py-1.5 text-sm bg-[var(--color-bg)] text-[var(--color-text)]
                       rounded-lg border border-[var(--color-surface-light)]"
              />
              <span class="text-[var(--color-text-muted)]">~</span>
              <input
                type="number"
                step="0.1"
                value={filters.distMa20Max}
                onInput={(e) => updateFilter('distMa20Max', e.currentTarget.value)}
                placeholder="최대%"
                class="w-16 px-2 py-1.5 text-sm bg-[var(--color-bg)] text-[var(--color-text)]
                       rounded-lg border border-[var(--color-surface-light)]"
              />
              <Show when={filters.distMa20Min || filters.distMa20Max || filters.ma20Position !== 'all'}>
                <button
                  onClick={() => setFilters({ distMa20Min: '', distMa20Max: '', ma20Position: 'all', currentPage: 1 })}
                  class="text-xs text-[var(--color-text-muted)] hover:text-[var(--color-text)]"
                >
                  ✕
                </button>
              </Show>
            </div>

            {/* MACD 크로스 필터 */}
            <div class="flex items-center gap-2">
              <span class="text-sm text-[var(--color-text-muted)]">MACD:</span>
              <div class="flex gap-1">
                <button
                  onClick={() => setFilters({ macdCrossFilter: 'all', currentPage: 1 })}
                  class={`px-2 py-1 text-xs rounded transition
                          ${filters.macdCrossFilter === 'all'
                            ? 'bg-[var(--color-primary)] text-white'
                            : 'bg-[var(--color-surface-light)] text-[var(--color-text-muted)]'}`}
                >
                  전체
                </button>
                <button
                  onClick={() => setFilters({ macdCrossFilter: 'golden', currentPage: 1 })}
                  class={`px-2 py-1 text-xs rounded transition flex items-center gap-1
                          ${filters.macdCrossFilter === 'golden'
                            ? 'bg-yellow-500/30 text-yellow-300 ring-1 ring-yellow-500/50'
                            : 'bg-[var(--color-surface-light)] text-[var(--color-text-muted)]'}`}
                  title="MACD가 시그널을 상향 돌파"
                >
                  <span>✨</span> 골든
                </button>
                <button
                  onClick={() => setFilters({ macdCrossFilter: 'dead', currentPage: 1 })}
                  class={`px-2 py-1 text-xs rounded transition flex items-center gap-1
                          ${filters.macdCrossFilter === 'dead'
                            ? 'bg-gray-500/30 text-gray-300 ring-1 ring-gray-500/50'
                            : 'bg-[var(--color-surface-light)] text-[var(--color-text-muted)]'}`}
                  title="MACD가 시그널을 하향 돌파"
                >
                  <span>💀</span> 데드
                </button>
              </div>
              <Show when={filters.macdCrossFilter !== 'all'}>
                <button
                  onClick={() => setFilters({ macdCrossFilter: 'all', currentPage: 1 })}
                  class="text-xs text-[var(--color-text-muted)] hover:text-[var(--color-text)]"
                >
                  ✕
                </button>
              </Show>
            </div>
          </div>

          {/* 활성 필터 요약 */}
          <Show when={filters.selectedSectors.length > 0 || filters.marketCapMin || filters.marketCapMax || filters.distMa20Min || filters.distMa20Max || filters.ma20Position !== 'all' || filters.selectedRouteStates.length > 0 || filters.rsiMin || filters.rsiMax || filters.macdCrossFilter !== 'all'}>
            <div class="mt-3 flex items-center gap-2 flex-wrap">
              <span class="text-xs text-[var(--color-text-muted)]">활성 필터:</span>
              <Show when={filters.selectedSectors.length > 0}>
                <span class="text-xs px-2 py-0.5 rounded bg-[var(--color-primary)]/10 text-[var(--color-primary)]">
                  섹터: {filters.selectedSectors.join(', ')}
                </span>
              </Show>
              <Show when={filters.marketCapMin || filters.marketCapMax}>
                <span class="text-xs px-2 py-0.5 rounded bg-green-500/10 text-green-400">
                  시총: {filters.marketCapMin || '0'}~{filters.marketCapMax || '∞'}억
                </span>
              </Show>
              <Show when={filters.ma20Position !== 'all' || filters.distMa20Min || filters.distMa20Max}>
                <span class="text-xs px-2 py-0.5 rounded bg-orange-500/10 text-orange-400">
                  20일선: {filters.ma20Position === 'above' ? '위' : filters.ma20Position === 'below' ? '아래' : ''} {filters.distMa20Min || filters.distMa20Max ? `${filters.distMa20Min || ''}~${filters.distMa20Max || ''}%` : ''}
                </span>
              </Show>
              <Show when={filters.rsiMin || filters.rsiMax}>
                <span class="text-xs px-2 py-0.5 rounded bg-purple-500/10 text-purple-400">
                  RSI: {filters.rsiMin || '0'}~{filters.rsiMax || '100'}
                </span>
              </Show>
              <Show when={filters.selectedRouteStates.length > 0}>
                <span class="text-xs px-2 py-0.5 rounded bg-yellow-500/10 text-yellow-400">
                  상태: {filters.selectedRouteStates.join(', ')}
                </span>
              </Show>
              <Show when={filters.macdCrossFilter !== 'all'}>
                <span class={`text-xs px-2 py-0.5 rounded ${filters.macdCrossFilter === 'golden' ? 'bg-yellow-500/10 text-yellow-300' : 'bg-gray-500/10 text-gray-400'}`}>
                  MACD: {filters.macdCrossFilter === 'golden' ? '✨ 골든크로스' : '💀 데드크로스'}
                </span>
              </Show>
              <span class={`text-xs px-2 py-0.5 rounded ${filters.filterMode === 'and' ? 'bg-blue-500/10 text-blue-400' : 'bg-orange-500/10 text-orange-400'}`}>
                조건: {filters.filterMode.toUpperCase()}
              </span>
              <button
                onClick={resetAllFilters}
                class="text-xs text-red-400 hover:text-red-300 ml-2"
              >
                전체 초기화
              </button>
            </div>
          </Show>

          <Show when={presetScreeningQuery.data}>
            <div class="mt-3 text-sm text-[var(--color-text-muted)]">
              {presetScreeningQuery.data?.filter_summary}
            </div>
          </Show>
        </div>
      </Show>

      {/* 커스텀 필터 탭 */}
      <Show when={ui.activeTab === 'custom'}>
        <div class="bg-[var(--color-surface)] rounded-xl p-4 mb-4">
          <div class="grid grid-cols-6 gap-4 mb-4">
            {/* 시장 */}
            <div>
              <label class="block text-xs text-[var(--color-text-muted)] mb-1">시장</label>
              <select
                value={customFilter.market}
                onChange={(e) => updateCustomFilter('market', e.currentTarget.value)}
                style={{ "background-color": "#1a1a2e" }}
                class="w-full px-3 py-2 text-sm text-[var(--color-text)] rounded-lg border border-[var(--color-surface-light)]"
              >
                <For each={MARKET_OPTIONS}>
                  {(option) => (
                    <option value={option.value}>
                      {option.indent ? '└ ' : ''}{option.emoji} {option.label}
                    </option>
                  )}
                </For>
              </select>
            </div>

            {/* PER */}
            <div>
              <label class="block text-xs text-[var(--color-text-muted)] mb-1">PER (최소~최대)</label>
              <div class="flex gap-1">
                <input
                  type="number"
                  value={customFilter.min_per}
                  onInput={(e) => updateCustomFilter('min_per', e.currentTarget.value)}
                  placeholder="0"
                  class="w-1/2 px-2 py-2 text-sm bg-[var(--color-bg)] text-[var(--color-text)]
                         rounded-lg border border-[var(--color-surface-light)]"
                />
                <input
                  type="number"
                  value={customFilter.max_per}
                  onInput={(e) => updateCustomFilter('max_per', e.currentTarget.value)}
                  placeholder="20"
                  class="w-1/2 px-2 py-2 text-sm bg-[var(--color-bg)] text-[var(--color-text)]
                         rounded-lg border border-[var(--color-surface-light)]"
                />
              </div>
            </div>

            {/* PBR */}
            <div>
              <label class="block text-xs text-[var(--color-text-muted)] mb-1">PBR (최소~최대)</label>
              <div class="flex gap-1">
                <input
                  type="number"
                  step="0.1"
                  value={customFilter.min_pbr}
                  onInput={(e) => updateCustomFilter('min_pbr', e.currentTarget.value)}
                  placeholder="0"
                  class="w-1/2 px-2 py-2 text-sm bg-[var(--color-bg)] text-[var(--color-text)]
                         rounded-lg border border-[var(--color-surface-light)]"
                />
                <input
                  type="number"
                  step="0.1"
                  value={customFilter.max_pbr}
                  onInput={(e) => updateCustomFilter('max_pbr', e.currentTarget.value)}
                  placeholder="1.5"
                  class="w-1/2 px-2 py-2 text-sm bg-[var(--color-bg)] text-[var(--color-text)]
                         rounded-lg border border-[var(--color-surface-light)]"
                />
              </div>
            </div>

            {/* ROE */}
            <div>
              <label class="block text-xs text-[var(--color-text-muted)] mb-1">ROE 최소 (%)</label>
              <input
                type="number"
                step="0.1"
                value={customFilter.min_roe}
                onInput={(e) => updateCustomFilter('min_roe', e.currentTarget.value)}
                placeholder="10"
                class="w-full px-2 py-2 text-sm bg-[var(--color-bg)] text-[var(--color-text)]
                       rounded-lg border border-[var(--color-surface-light)]"
              />
            </div>

            {/* 배당수익률 */}
            <div>
              <label class="block text-xs text-[var(--color-text-muted)] mb-1">배당수익률 최소 (%)</label>
              <input
                type="number"
                step="0.1"
                value={customFilter.min_dividend_yield}
                onInput={(e) => updateCustomFilter('min_dividend_yield', e.currentTarget.value)}
                placeholder="3"
                class="w-full px-2 py-2 text-sm bg-[var(--color-bg)] text-[var(--color-text)]
                       rounded-lg border border-[var(--color-surface-light)]"
              />
            </div>

            {/* 부채비율 */}
            <div>
              <label class="block text-xs text-[var(--color-text-muted)] mb-1">부채비율 최대 (%)</label>
              <input
                type="number"
                value={customFilter.max_debt_ratio}
                onInput={(e) => updateCustomFilter('max_debt_ratio', e.currentTarget.value)}
                placeholder="100"
                class="w-full px-2 py-2 text-sm bg-[var(--color-bg)] text-[var(--color-text)]
                       rounded-lg border border-[var(--color-surface-light)]"
              />
            </div>
          </div>

          <div class="grid grid-cols-6 gap-4 mb-4">
            {/* 매출성장률 */}
            <div>
              <label class="block text-xs text-[var(--color-text-muted)] mb-1">매출성장률 최소 (%)</label>
              <input
                type="number"
                value={customFilter.min_revenue_growth}
                onInput={(e) => updateCustomFilter('min_revenue_growth', e.currentTarget.value)}
                placeholder="10"
                class="w-full px-2 py-2 text-sm bg-[var(--color-bg)] text-[var(--color-text)]
                       rounded-lg border border-[var(--color-surface-light)]"
              />
            </div>

            {/* 이익성장률 */}
            <div>
              <label class="block text-xs text-[var(--color-text-muted)] mb-1">이익성장률 최소 (%)</label>
              <input
                type="number"
                value={customFilter.min_earnings_growth}
                onInput={(e) => updateCustomFilter('min_earnings_growth', e.currentTarget.value)}
                placeholder="10"
                class="w-full px-2 py-2 text-sm bg-[var(--color-bg)] text-[var(--color-text)]
                       rounded-lg border border-[var(--color-surface-light)]"
              />
            </div>

            {/* 52주 고점 이격 */}
            <div>
              <label class="block text-xs text-[var(--color-text-muted)] mb-1">52주 고점 이격 최대 (%)</label>
              <input
                type="number"
                value={customFilter.max_distance_from_52w_high}
                onInput={(e) => updateCustomFilter('max_distance_from_52w_high', e.currentTarget.value)}
                placeholder="20"
                class="w-full px-2 py-2 text-sm bg-[var(--color-bg)] text-[var(--color-text)]
                       rounded-lg border border-[var(--color-surface-light)]"
              />
            </div>

            {/* 정렬 기준 */}
            <div>
              <label class="block text-xs text-[var(--color-text-muted)] mb-1">정렬 기준</label>
              <select
                value={customFilter.sort_by}
                onChange={(e) => updateCustomFilter('sort_by', e.currentTarget.value)}
                style={{ "background-color": "#1a1a2e" }}
                class="w-full px-2 py-2 text-sm text-[var(--color-text)] rounded-lg border border-[var(--color-surface-light)]"
              >
                <option value="market_cap">시가총액</option>
                <option value="per">PER</option>
                <option value="pbr">PBR</option>
                <option value="roe">ROE</option>
                <option value="dividend_yield">배당수익률</option>
              </select>
            </div>

            {/* 결과 수 */}
            <div>
              <label class="block text-xs text-[var(--color-text-muted)] mb-1">결과 수</label>
              <select
                value={customFilter.limit}
                onChange={(e) => updateCustomFilter('limit', parseInt(e.currentTarget.value))}
                style={{ "background-color": "#1a1a2e" }}
                class="w-full px-2 py-2 text-sm text-[var(--color-text)] rounded-lg border border-[var(--color-surface-light)]"
              >
                <option value={20}>20개</option>
                <option value={50}>50개</option>
                <option value={100}>100개</option>
              </select>
            </div>

            {/* 액션 버튼 */}
            <div class="flex items-end gap-2">
              <button
                onClick={handleCustomScreening}
                disabled={customScreeningMutation.isPending}
                class="flex-1 px-4 py-2 bg-[var(--color-primary)] text-white rounded-lg text-sm
                       hover:bg-[var(--color-primary-dark)] transition flex items-center justify-center gap-2 disabled:opacity-50"
              >
                <Show when={customScreeningMutation.isPending} fallback={<Search class="w-4 h-4" />}>
                  <Loader2 class="w-4 h-4 animate-spin" />
                </Show>
                스크리닝
              </button>
              <button
                onClick={resetCustomFilter}
                class="px-3 py-2 bg-[var(--color-surface-light)] text-[var(--color-text)] rounded-lg text-sm
                       hover:bg-[var(--color-surface)] transition"
                title="필터 초기화"
              >
                <RefreshCw class="w-4 h-4" />
              </button>
            </div>
          </div>

          <Show when={customScreeningMutation.data}>
            <div class="text-sm text-[var(--color-text-muted)]">
              {customScreeningMutation.data?.filter_summary}
            </div>
          </Show>
        </div>
      </Show>

      {/* 모멘텀 탭 */}
      <Show when={ui.activeTab === 'momentum'}>
        <div class="bg-[var(--color-surface)] rounded-xl p-4 mb-4">
          <div class="flex items-center gap-4 flex-wrap">
            <div>
              <label class="block text-xs text-[var(--color-text-muted)] mb-1">시장</label>
              <select
                value={filters.momentumMarket}
                onChange={(e) => setFilters('momentumMarket', e.currentTarget.value)}
                style={{ "background-color": "#1a1a2e" }}
                class="px-3 py-2 text-sm text-[var(--color-text)] rounded-lg border border-[var(--color-surface-light)]"
              >
                <option value="">전체</option>
                <option value="KR">한국</option>
                <option value="US">미국</option>
              </select>
            </div>
            <div>
              <label class="block text-xs text-[var(--color-text-muted)] mb-1">기간 (일)</label>
              <select
                value={filters.momentumDays}
                onChange={(e) => setFilters('momentumDays', parseInt(e.currentTarget.value))}
                style={{ "background-color": "#1a1a2e" }}
                class="px-3 py-2 text-sm text-[var(--color-text)] rounded-lg border border-[var(--color-surface-light)]"
              >
                <option value={1}>1일</option>
                <option value={3}>3일</option>
                <option value={5}>5일</option>
                <option value={10}>10일</option>
                <option value={20}>20일</option>
              </select>
            </div>
            <div>
              <label class="block text-xs text-[var(--color-text-muted)] mb-1">최소 변동률 (%)</label>
              <input
                type="number"
                value={filters.momentumMinChange}
                onInput={(e) => setFilters('momentumMinChange', e.currentTarget.value)}
                class="w-20 px-3 py-2 text-sm bg-[var(--color-bg)] text-[var(--color-text)]
                       rounded-lg border border-[var(--color-surface-light)]"
              />
            </div>
            <div class="flex items-end">
              <button
                onClick={() => momentumQuery.refetch()}
                disabled={momentumQuery.isFetching}
                class="px-4 py-2 bg-[var(--color-primary)] text-white rounded-lg text-sm
                       hover:bg-[var(--color-primary-dark)] transition flex items-center gap-2 disabled:opacity-50"
              >
                <RefreshCw class={`w-4 h-4 ${momentumQuery.isFetching ? 'animate-spin' : ''}`} />
                조회
              </button>
            </div>
          </div>
          <Show when={momentumQuery.data}>
            <div class="mt-3 text-sm text-[var(--color-text-muted)]">
              {momentumQuery.data?.days}일간 {momentumQuery.data?.min_change_pct}% 이상 변동 종목: {momentumQuery.data?.total}개
            </div>
          </Show>
        </div>
      </Show>

      {/* 결과 영역 */}
      <div class="flex-1 bg-[var(--color-surface)] rounded-xl overflow-hidden flex flex-col min-h-0">
        {/* 뷰 모드 토글 + 결과 요약 */}
        <Show when={!isLoading() && ui.activeTab !== 'momentum' && sortedResults().length > 0}>
          <div class="flex items-center justify-between px-4 py-3 border-b border-[var(--color-surface-light)]">
            <div class="text-sm text-[var(--color-text-muted)]">
              총 {sortedResults().length}개 종목
            </div>
            <div class="flex items-center gap-1 bg-[var(--color-surface-light)] rounded-lg p-1">
              <button
                onClick={() => setUI('viewMode', 'table')}
                class={`px-3 py-1.5 text-xs rounded-md transition flex items-center gap-1.5 ${
                  ui.viewMode === 'table'
                    ? 'bg-[var(--color-primary)] text-white'
                    : 'text-[var(--color-text-muted)] hover:text-[var(--color-text)]'
                }`}
              >
                📋 테이블
              </button>
              <button
                onClick={() => setUI('viewMode', 'map')}
                class={`px-3 py-1.5 text-xs rounded-md transition flex items-center gap-1.5 ${
                  ui.viewMode === 'map'
                    ? 'bg-[var(--color-primary)] text-white'
                    : 'text-[var(--color-text-muted)] hover:text-[var(--color-text)]'
                }`}
              >
                🗺️ 기회맵
              </button>
              <button
                onClick={() => setUI('viewMode', 'kanban')}
                class={`px-3 py-1.5 text-xs rounded-md transition flex items-center gap-1.5 ${
                  ui.viewMode === 'kanban'
                    ? 'bg-[var(--color-primary)] text-white'
                    : 'text-[var(--color-text-muted)] hover:text-[var(--color-text)]'
                }`}
              >
                📊 칸반
              </button>
            </div>
          </div>
        </Show>

        {/* 로딩 상태 - 공통 컴포넌트 사용 */}
        <Show when={isLoading()}>
          <div class="flex-1 flex items-center justify-center">
            <PageLoader message="스크리닝 중..." />
          </div>
        </Show>

        {/* 결과 없음 - 공통 컴포넌트 사용 */}
        {/* sortedResults 사용: 클라이언트 필터 적용 후 결과가 0인 경우도 포함 */}
        <Show when={!isLoading() && (ui.activeTab !== 'momentum'
          ? (currentResults().length === 0 || sortedResults().length === 0)
          : (momentumQuery.data?.results?.length || 0) === 0)}>
          <div class="flex-1 flex items-center justify-center">
            <EmptyState
              icon="📭"
              title={ui.activeTab === 'custom' && !customScreeningMutation.data
                ? '필터를 설정하세요'
                : currentResults().length > 0 && sortedResults().length === 0
                  ? '클라이언트 필터 조건에 맞는 종목이 없습니다'
                  : '조건에 맞는 종목이 없습니다'}
              description={ui.activeTab === 'custom' && !customScreeningMutation.data
                ? '필터를 설정하고 스크리닝 버튼을 클릭하세요.'
                : currentResults().length > 0 && sortedResults().length === 0
                  ? `API 결과 ${currentResults().length}개 중 추가 필터(RouteState, RSI 등)에 맞는 종목이 없습니다. 일부 데이터가 아직 계산되지 않았을 수 있습니다.`
                  : '다른 필터 조건을 시도해보세요.'}
            />
          </div>
        </Show>

        {/* 기회맵 뷰 */}
        <Show when={!isLoading() && ui.activeTab !== 'momentum' && sortedResults().length > 0 && ui.viewMode === 'map'}>
          <div class="flex-1 p-4 overflow-auto">
            <OpportunityMap
              symbols={opportunityMapData()}
              onSymbolClick={(symbol) => {
                const result = sortedResults().find(r => r.ticker === symbol)
                if (result) openSymbolDetailModal(result)
              }}
              height={500}
              showQuadrantLabels={true}
              threshold={50}
              title="종목 기회 분석"
            />
          </div>
        </Show>

        {/* 칸반 뷰 */}
        <Show when={!isLoading() && ui.activeTab !== 'momentum' && sortedResults().length > 0 && ui.viewMode === 'kanban'}>
          <div class="flex-1 p-4 overflow-auto">
            <Show
              when={kanbanBoardData().length > 0}
              fallback={
                <EmptyState
                  icon="📊"
                  title="칸반 데이터 없음"
                  description="ATTACK, ARMED, WATCH 상태의 종목이 없습니다"
                />
              }
            >
              <KanbanBoard
                symbols={kanbanBoardData()}
                onCardClick={(symbol) => {
                  const result = sortedResults().find(r => r.ticker === symbol)
                  if (result) openSymbolDetailModal(result)
                }}
                enableDragDrop={false}
              />
            </Show>
          </div>
        </Show>

        {/* 펀더멘털 결과 테이블 (프리셋/커스텀) */}
        <Show when={!isLoading() && ui.activeTab !== 'momentum' && sortedResults().length > 0 && ui.viewMode === 'table'}>
          <div class="overflow-auto flex-1">
            <table class="w-full text-sm table-fixed">
              <thead class="sticky top-0 bg-[var(--color-surface-light)]">
                <tr>
                  <th class="w-[10%] px-4 py-3 text-left font-medium text-[var(--color-text-muted)]">
                    <button onClick={() => handleSort('ticker')} class="flex items-center gap-1 hover:text-[var(--color-text)]">
                      티커
                      <Show when={filters.sortField === 'ticker'}>
                        {filters.sortOrder === 'asc' ? <ChevronUp class="w-3 h-3" /> : <ChevronDown class="w-3 h-3" />}
                      </Show>
                    </button>
                  </th>
                  <th class="w-[20%] px-4 py-3 text-left font-medium text-[var(--color-text-muted)]">
                    <button onClick={() => handleSort('name')} class="flex items-center gap-1 hover:text-[var(--color-text)]">
                      종목명
                      <Show when={filters.sortField === 'name'}>
                        {filters.sortOrder === 'asc' ? <ChevronUp class="w-3 h-3" /> : <ChevronDown class="w-3 h-3" />}
                      </Show>
                    </button>
                  </th>
                  <th class="w-[10%] px-4 py-3 text-right font-medium text-[var(--color-text-muted)]">
                    <button onClick={() => handleSort('market_cap')} class="flex items-center gap-1 justify-end hover:text-[var(--color-text)]">
                      시가총액
                      <Show when={filters.sortField === 'market_cap'}>
                        {filters.sortOrder === 'asc' ? <ChevronUp class="w-3 h-3" /> : <ChevronDown class="w-3 h-3" />}
                      </Show>
                    </button>
                  </th>
                  <th class="w-[8%] px-4 py-3 text-right font-medium text-[var(--color-text-muted)]">
                    <button onClick={() => handleSort('per')} class="flex items-center gap-1 justify-end hover:text-[var(--color-text)]">
                      PER
                      <Show when={filters.sortField === 'per'}>
                        {filters.sortOrder === 'asc' ? <ChevronUp class="w-3 h-3" /> : <ChevronDown class="w-3 h-3" />}
                      </Show>
                    </button>
                  </th>
                  <th class="w-[8%] px-4 py-3 text-right font-medium text-[var(--color-text-muted)]">
                    <button onClick={() => handleSort('pbr')} class="flex items-center gap-1 justify-end hover:text-[var(--color-text)]">
                      PBR
                      <Show when={filters.sortField === 'pbr'}>
                        {filters.sortOrder === 'asc' ? <ChevronUp class="w-3 h-3" /> : <ChevronDown class="w-3 h-3" />}
                      </Show>
                    </button>
                  </th>
                  <th class="w-[10%] px-4 py-3 text-right font-medium text-[var(--color-text-muted)]">
                    <button onClick={() => handleSort('roe')} class="flex items-center gap-1 justify-end hover:text-[var(--color-text)]">
                      ROE
                      <Show when={filters.sortField === 'roe'}>
                        {filters.sortOrder === 'asc' ? <ChevronUp class="w-3 h-3" /> : <ChevronDown class="w-3 h-3" />}
                      </Show>
                    </button>
                  </th>
                  <th class="w-[10%] px-4 py-3 text-right font-medium text-[var(--color-text-muted)]">
                    <button onClick={() => handleSort('dividend_yield')} class="flex items-center gap-1 justify-end hover:text-[var(--color-text)]">
                      배당률
                      <Show when={filters.sortField === 'dividend_yield'}>
                        {filters.sortOrder === 'asc' ? <ChevronUp class="w-3 h-3" /> : <ChevronDown class="w-3 h-3" />}
                      </Show>
                    </button>
                  </th>
                  <th class="w-[8%] px-4 py-3 text-center font-medium text-[var(--color-text-muted)]">상태</th>
                  <th class="w-[6%] px-4 py-3 text-center font-medium text-[var(--color-text-muted)]">등급</th>
                  <th class="w-[10%] px-4 py-3 text-right font-medium text-[var(--color-text-muted)]">점수</th>
                </tr>
              </thead>
              <tbody>
                <For each={paginatedResults()}>
                  {(result, idx) => (
                    <tr
                      class={`border-t border-[var(--color-surface-light)] hover:bg-[var(--color-surface-light)]/50 transition cursor-pointer
                              ${idx() % 2 === 0 ? '' : 'bg-[var(--color-surface-light)]/20'}`}
                      onClick={() => openSymbolDetailModal(result)}
                    >
                      <td class="w-[10%] px-4 py-3">
                        <div class="flex items-center gap-2">
                          <span class="font-mono font-medium text-[var(--color-text)]">{result.ticker}</span>
                          <span class="text-xs px-1.5 py-0.5 rounded bg-[var(--color-primary)]/10 text-[var(--color-primary)]">
                            {result.market}
                          </span>
                        </div>
                      </td>
                      <td class="w-[20%] px-4 py-3 text-[var(--color-text)] truncate">{result.name}</td>
                      <td class="w-[10%] px-4 py-3 text-right font-mono text-[var(--color-text)]">
                        {formatMarketCap(result.market_cap)}
                      </td>
                      <td class="px-4 py-3 text-right font-mono text-[var(--color-text)]">
                        {formatNumber(result.per, 1)}
                      </td>
                      <td class="px-4 py-3 text-right font-mono text-[var(--color-text)]">
                        {formatNumber(result.pbr, 2)}
                      </td>
                      <td class="px-4 py-3 text-right font-mono">
                        <span class={parseFloat(result.roe || '0') >= 15 ? 'text-green-400' : 'text-[var(--color-text)]'}>
                          {result.roe ? `${formatNumber(result.roe, 1)}%` : '-'}
                        </span>
                      </td>
                      <td class="px-4 py-3 text-right font-mono">
                        <span class={parseFloat(result.dividend_yield || '0') >= 3 ? 'text-blue-400' : 'text-[var(--color-text)]'}>
                          {result.dividend_yield ? `${formatNumber(result.dividend_yield, 2)}%` : '-'}
                        </span>
                      </td>
                      <td class="px-4 py-3 text-center">
                        <Show when={result.route_state} fallback={<span class="text-[var(--color-text-muted)]">-</span>}>
                          {(() => {
                            const style = ROUTE_STATE_STYLES[result.route_state!] || ROUTE_STATE_STYLES.Rest
                            return (
                              <span class={`text-xs px-2 py-1 rounded font-medium ${style.bg} ${style.text}`}>
                                {style.label}
                              </span>
                            )
                          })()}
                        </Show>
                      </td>
                      <td class="px-4 py-3 text-center">
                        <Show when={result.grade} fallback={<span class="text-[var(--color-text-muted)]">-</span>}>
                          {(() => {
                            const style = GRADE_STYLES[result.grade!] || { bg: 'bg-gray-500/20', text: 'text-gray-400' }
                            return (
                              <span class={`text-xs px-2 py-1 rounded font-bold ${style.bg} ${style.text}`}>
                                {result.grade}
                              </span>
                            )
                          })()}
                        </Show>
                      </td>
                      <td class="px-4 py-3 text-right font-mono">
                        <Show when={result.overall_score} fallback={<span class="text-[var(--color-text-muted)]">-</span>}>
                          <span class={parseFloat(result.overall_score || '0') >= 70 ? 'text-green-400' : parseFloat(result.overall_score || '0') >= 50 ? 'text-yellow-400' : 'text-[var(--color-text)]'}>
                            {formatNumber(result.overall_score, 1)}
                          </span>
                        </Show>
                      </td>
                    </tr>
                  )}
                </For>
              </tbody>
            </table>
          </div>

          {/* 페이지네이션 */}
          <Show when={totalPages() > 1}>
            <div class="flex items-center justify-between px-4 py-3 border-t border-[var(--color-surface-light)]">
              <span class="text-sm text-[var(--color-text-muted)]">
                총 {sortedResults().length}개 중 {(filters.currentPage - 1) * pageSize + 1}-{Math.min(filters.currentPage * pageSize, sortedResults().length)}
              </span>
              <div class="flex items-center gap-2">
                <button
                  onClick={() => setFilters('currentPage', Math.max(1, filters.currentPage - 1))}
                  disabled={filters.currentPage === 1}
                  class="px-3 py-1 rounded bg-[var(--color-surface-light)] text-[var(--color-text)]
                         disabled:opacity-50 hover:bg-[var(--color-primary)]/20 transition"
                >
                  이전
                </button>
                <span class="text-sm text-[var(--color-text)]">
                  {filters.currentPage} / {totalPages()}
                </span>
                <button
                  onClick={() => setFilters('currentPage', Math.min(totalPages(), filters.currentPage + 1))}
                  disabled={filters.currentPage === totalPages()}
                  class="px-3 py-1 rounded bg-[var(--color-surface-light)] text-[var(--color-text)]
                         disabled:opacity-50 hover:bg-[var(--color-primary)]/20 transition"
                >
                  다음
                </button>
              </div>
            </div>
          </Show>
        </Show>

        {/* 모멘텀 결과 테이블 */}
        <Show when={!isLoading() && ui.activeTab === 'momentum' && (momentumQuery.data?.results?.length || 0) > 0}>
          <div class="overflow-auto flex-1">
            <table class="w-full text-sm" style={{ "table-layout": "fixed" }}>
              <thead class="sticky top-0 bg-[var(--color-surface-light)]">
                <tr>
                  <th class="px-4 py-3 text-left font-medium text-[var(--color-text-muted)]" style={{ width: "10%" }}>티커</th>
                  <th class="px-4 py-3 text-left font-medium text-[var(--color-text-muted)]" style={{ width: "18%" }}>종목명</th>
                  <th class="px-4 py-3 text-center font-medium text-[var(--color-text-muted)]" style={{ width: "8%" }}>시장</th>
                  <th class="px-4 py-3 text-right font-medium text-[var(--color-text-muted)]" style={{ width: "10%" }}>시작가</th>
                  <th class="px-4 py-3 text-right font-medium text-[var(--color-text-muted)]" style={{ width: "10%" }}>종가</th>
                  <th class="px-4 py-3 text-right font-medium text-[var(--color-text-muted)]" style={{ width: "10%" }}>변동률</th>
                  <th class="px-4 py-3 text-right font-medium text-[var(--color-text-muted)]" style={{ width: "12%" }}>평균거래량</th>
                  <th class="px-4 py-3 text-right font-medium text-[var(--color-text-muted)]" style={{ width: "12%" }}>현재거래량</th>
                  <th class="px-4 py-3 text-right font-medium text-[var(--color-text-muted)]" style={{ width: "10%" }}>거래량 배율</th>
                </tr>
              </thead>
              <tbody>
                <For each={momentumQuery.data?.results || []}>
                  {(result, idx) => (
                    <tr class={`border-t border-[var(--color-surface-light)] hover:bg-[var(--color-surface-light)]/50 transition
                                ${idx() % 2 === 0 ? '' : 'bg-[var(--color-surface-light)]/20'}`}>
                      <td class="px-4 py-3">
                        <span class="font-mono font-medium text-[var(--color-text)]">{result.symbol}</span>
                      </td>
                      <td class="px-4 py-3 text-[var(--color-text)]">{result.name}</td>
                      <td class="px-4 py-3 text-center">
                        <span class="text-xs px-1.5 py-0.5 rounded bg-[var(--color-primary)]/10 text-[var(--color-primary)]">
                          {result.market}
                        </span>
                      </td>
                      <td class="px-4 py-3 text-right font-mono text-[var(--color-text)]">
                        {formatNumber(result.start_price)}
                      </td>
                      <td class="px-4 py-3 text-right font-mono text-[var(--color-text)]">
                        {formatNumber(result.end_price)}
                      </td>
                      <td class="px-4 py-3 text-right font-mono">
                        <span class={parseFloat(result.change_pct) >= 0 ? 'text-green-400' : 'text-red-400'}>
                          {formatPercent(result.change_pct)}
                        </span>
                      </td>
                      <td class="px-4 py-3 text-right font-mono text-[var(--color-text)]">
                        {formatNumber(result.avg_volume, 0)}
                      </td>
                      <td class="px-4 py-3 text-right font-mono text-[var(--color-text)]">
                        {formatNumber(result.current_volume, 0)}
                      </td>
                      <td class="px-4 py-3 text-right font-mono">
                        <span class={parseFloat(result.volume_ratio) >= 2 ? 'text-yellow-400' : 'text-[var(--color-text)]'}>
                          {formatNumber(result.volume_ratio, 1)}x
                        </span>
                      </td>
                    </tr>
                  )}
                </For>
              </tbody>
            </table>
          </div>
        </Show>
      </div>

      {/* 종목 상세 모달 */}
      <SymbolDetailModal
        isOpen={modal.symbolDetail.open}
        symbol={modal.symbolDetail.symbol}
        onClose={closeSymbolDetailModal}
        onAddWatchlist={(ticker) => {
          const market = modal.symbolDetail.symbol?.market || 'KR'
          openWatchlistModal(ticker, market)
        }}
        onLinkStrategy={(ticker) => {
          openStrategyLinkModal(ticker)
        }}
      />

      {/* 관심종목 선택 모달 */}
      <WatchlistSelectModal
        isOpen={modal.watchlist.open}
        symbol={modal.watchlist.ticker}
        market={modal.watchlist.market}
        onClose={closeWatchlistModal}
      />

      {/* 전략 연결 모달 */}
      <StrategyLinkModal
        isOpen={modal.strategyLink.open}
        symbol={modal.strategyLink.symbol}
        onClose={closeStrategyLinkModal}
      />

      {/* 프리셋 관리 모달 */}
      <PresetModal
        isOpen={modal.preset.open}
        currentFilters={customFilter}
        onClose={closePresetModal}
        onSuccess={() => presetsQuery.refetch()}
      />
    </div>
  )
}

export default Screening
