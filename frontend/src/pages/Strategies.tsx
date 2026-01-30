import { createSignal, createResource, For, Show, createEffect } from 'solid-js'
import { useNavigate } from '@solidjs/router'
import { Play, Pause, Settings, TrendingUp, TrendingDown, AlertCircle, RefreshCw, X, ChevronRight, Search, BarChart3, Activity } from 'lucide-solid'
import { getStrategies, startStrategy, stopStrategy, getBacktestStrategies, createStrategy, getStrategy, updateStrategyConfig } from '../api/client'
import type { Strategy } from '../types'
import type { BacktestStrategy, UiSchema } from '../api/client'
import { DynamicForm } from '../components/DynamicForm'
import { useToast } from '../components/Toast'

function formatCurrency(value: number): string {
  return new Intl.NumberFormat('ko-KR', {
    style: 'currency',
    currency: 'KRW',
    maximumFractionDigits: 0,
  }).format(value)
}

// 전략 타입별 기본 타임프레임
function getDefaultTimeframe(strategyType: string): string {
  switch (strategyType) {
    // 실시간 전략: 1m
    case 'grid':
    case 'grid_trading':
    case 'magic_split':
    case 'split':
    case 'infinity_bot':
    case 'trailing_stop':
      return '1m'
    // 분봉 전략: 15m
    case 'rsi':
    case 'rsi_mean_reversion':
    case 'bollinger':
    case 'bollinger_bands':
    case 'sma':
    case 'sma_crossover':
    case 'ma_crossover':
    case 'candle_pattern':
      return '15m'
    // 일봉 전략: 1d
    case 'volatility_breakout':
    case 'volatility':
    case 'snow':
    case 'snow_us':
    case 'snow_kr':
    case 'stock_rotation':
    case 'rotation':
    case 'market_interest_day':
    case 'simple_power':
    case 'haa':
    case 'xaa':
    case 'all_weather':
    case 'all_weather_us':
    case 'all_weather_kr':
    case 'market_cap_top':
      return '1d'
    default:
      return '1d'
  }
}

export function Strategies() {
  const toast = useToast()
  const navigate = useNavigate()
  const [filter, setFilter] = createSignal<'all' | 'running' | 'stopped'>('all')
  const [togglingId, setTogglingId] = createSignal<string | null>(null)

  // ==================== 전략 추가 모달 상태 ====================
  const [showAddModal, setShowAddModal] = createSignal(false)
  const [modalStep, setModalStep] = createSignal<'select' | 'configure'>('select')
  const [selectedStrategy, setSelectedStrategy] = createSignal<BacktestStrategy | null>(null)
  const [strategyParams, setStrategyParams] = createSignal<Record<string, unknown>>({})
  const [formErrors, setFormErrors] = createSignal<Record<string, string>>({})
  const [customName, setCustomName] = createSignal('')  // 전략 이름 커스터마이징
  const [searchQuery, setSearchQuery] = createSignal('')
  const [selectedCategory, setSelectedCategory] = createSignal<string | null>(null)

  // ==================== 전략 편집 모달 상태 ====================
  const [showEditModal, setShowEditModal] = createSignal(false)
  const [editingStrategyId, setEditingStrategyId] = createSignal<string | null>(null)
  const [editingStrategyType, setEditingStrategyType] = createSignal<string | null>(null)
  const [editingStrategyName, setEditingStrategyName] = createSignal('')
  const [editingParams, setEditingParams] = createSignal<Record<string, unknown>>({})
  const [editFormErrors, setEditFormErrors] = createSignal<Record<string, string>>({})
  const [isLoadingStrategy, setIsLoadingStrategy] = createSignal(false)
  const [isUpdating, setIsUpdating] = createSignal(false)
  const [updateError, setUpdateError] = createSignal<string | null>(null)

  // 전략 템플릿 목록 가져오기
  const [strategyTemplates] = createResource(async () => {
    const response = await getBacktestStrategies()
    return response.strategies
  })

  // 전략 목록 가져오기
  const [strategies, { refetch }] = createResource(getStrategies)

  // 카테고리 목록
  const categories = () => {
    const cats = new Set<string>()
    strategyTemplates()?.forEach(s => {
      if (s.category) cats.add(s.category)
    })
    return Array.from(cats)
  }

  // 필터링된 전략 템플릿
  const filteredTemplates = () => {
    let templates = strategyTemplates() || []

    // 카테고리 필터
    if (selectedCategory()) {
      templates = templates.filter(s => s.category === selectedCategory())
    }

    // 검색 필터
    const query = searchQuery().toLowerCase()
    if (query) {
      templates = templates.filter(s =>
        s.name.toLowerCase().includes(query) ||
        s.description.toLowerCase().includes(query) ||
        s.tags?.some(t => t.toLowerCase().includes(query))
      )
    }

    return templates
  }

  // 전략 선택
  const selectStrategy = (template: BacktestStrategy) => {
    setSelectedStrategy(template)

    // 기본값으로 파라미터 초기화
    const initialParams: Record<string, unknown> = { ...(template.default_params || {}) }

    // ui_schema의 default_value도 적용 (default_params에 없는 필드의 경우)
    if (template.ui_schema) {
      for (const field of template.ui_schema.fields) {
        if (initialParams[field.key] === undefined && field.default_value !== undefined) {
          initialParams[field.key] = field.default_value
        }
      }
    }

    setStrategyParams(initialParams)
    setFormErrors({})
    setCustomName(template.name)  // 기본 이름으로 초기화
    setModalStep('configure')
  }

  // 파라미터 변경
  const handleParamChange = (key: string, value: unknown) => {
    setStrategyParams(prev => ({ ...prev, [key]: value }))
    // 에러 지우기
    setFormErrors(prev => {
      const next = { ...prev }
      delete next[key]
      return next
    })
  }

  // 폼 유효성 검사
  const validateForm = (): boolean => {
    const template = selectedStrategy()
    if (!template?.ui_schema) return true

    const errors: Record<string, string> = {}
    const params = strategyParams()

    for (const field of template.ui_schema.fields) {
      const value = params[field.key]

      // 필수 필드 검사
      if (field.validation.required) {
        if (value === undefined || value === null || value === '') {
          errors[field.key] = '필수 항목입니다'
          continue
        }
        if (Array.isArray(value) && value.length === 0) {
          errors[field.key] = '최소 하나 이상 선택해주세요'
          continue
        }
      }

      // 숫자 범위 검사
      if (field.field_type === 'number' || field.field_type === 'range') {
        const numValue = value as number
        if (field.validation.min !== undefined && numValue < field.validation.min) {
          errors[field.key] = `최소값은 ${field.validation.min}입니다`
        }
        if (field.validation.max !== undefined && numValue > field.validation.max) {
          errors[field.key] = `최대값은 ${field.validation.max}입니다`
        }
      }

      // 심볼 개수 검사
      if (field.field_type === 'symbol_picker' && Array.isArray(value)) {
        if (field.validation.min_items && value.length < field.validation.min_items) {
          errors[field.key] = `최소 ${field.validation.min_items}개를 선택해주세요`
        }
        if (field.validation.max_items && value.length > field.validation.max_items) {
          errors[field.key] = `최대 ${field.validation.max_items}개까지 선택 가능합니다`
        }
      }
    }

    setFormErrors(errors)
    return Object.keys(errors).length === 0
  }

  // 전략 생성
  const [isCreating, setIsCreating] = createSignal(false)
  const [createError, setCreateError] = createSignal<string | null>(null)

  const handleCreateStrategy = async () => {
    if (!validateForm()) return

    const template = selectedStrategy()
    if (!template) return

    setIsCreating(true)
    setCreateError(null)

    try {
      const response = await createStrategy({
        strategy_type: template.id,
        name: customName() || template.name,  // 커스텀 이름 사용
        parameters: strategyParams(),
      })

      console.log('Strategy created:', response)

      // 모달 닫기 및 상태 초기화
      closeModal()
      // 전략 목록 새로고침
      refetch()
      // 성공 토스트
      toast.success('전략 생성 완료', `"${customName() || template.name}" 전략이 생성되었습니다`)
    } catch (error) {
      console.error('Failed to create strategy:', error)
      const errorMsg = error instanceof Error ? error.message : '전략 생성에 실패했습니다'
      setCreateError(errorMsg)
      toast.error('전략 생성 실패', errorMsg)
    } finally {
      setIsCreating(false)
    }
  }

  // 모달 닫기
  const closeModal = () => {
    setShowAddModal(false)
    setModalStep('select')
    setSelectedStrategy(null)
    setStrategyParams({})
    setFormErrors({})
    setCustomName('')
    setSearchQuery('')
    setSelectedCategory(null)
  }

  // 뒤로가기
  const goBack = () => {
    setModalStep('select')
    setSelectedStrategy(null)
    setStrategyParams({})
    setFormErrors({})
    setCustomName('')
  }

  // ==================== 전략 편집 기능 ====================

  // 편집 모달 열기
  const handleEditStrategy = async (strategy: Strategy) => {
    setEditingStrategyId(strategy.id)
    setIsLoadingStrategy(true)
    setShowEditModal(true)
    setUpdateError(null)
    setEditFormErrors({})

    try {
      // API에서 전략 상세 정보 가져오기
      const detail = await getStrategy(strategy.id)
      setEditingStrategyType(detail.strategy_type)
      setEditingStrategyName(detail.name)
      setEditingParams(detail.config as Record<string, unknown>)
    } catch (error) {
      console.error('Failed to load strategy:', error)
      const errorMsg = error instanceof Error ? error.message : '전략 정보를 불러오는데 실패했습니다'
      setUpdateError(errorMsg)
      toast.error('전략 로드 실패', errorMsg)
    } finally {
      setIsLoadingStrategy(false)
    }
  }

  // 편집 모달에서 사용할 전략 템플릿 가져오기
  const getEditingTemplate = () => {
    const strategyType = editingStrategyType()
    if (!strategyType) return null
    return strategyTemplates()?.find(t => t.id === strategyType) || null
  }

  // 편집 파라미터 변경
  const handleEditParamChange = (key: string, value: unknown) => {
    setEditingParams(prev => ({ ...prev, [key]: value }))
    setEditFormErrors(prev => {
      const next = { ...prev }
      delete next[key]
      return next
    })
  }

  // 편집 폼 유효성 검사
  const validateEditForm = (): boolean => {
    const template = getEditingTemplate()
    if (!template?.ui_schema) return true

    const errors: Record<string, string> = {}
    const params = editingParams()

    for (const field of template.ui_schema.fields) {
      const value = params[field.key]

      // 필수 필드 검사
      if (field.validation.required) {
        if (value === undefined || value === null || value === '') {
          errors[field.key] = '필수 항목입니다'
          continue
        }
        if (Array.isArray(value) && value.length === 0) {
          errors[field.key] = '최소 하나 이상 선택해주세요'
          continue
        }
      }

      // 숫자 범위 검사
      if (field.field_type === 'number' || field.field_type === 'range') {
        const numValue = value as number
        if (field.validation.min !== undefined && numValue < field.validation.min) {
          errors[field.key] = `최소값은 ${field.validation.min}입니다`
        }
        if (field.validation.max !== undefined && numValue > field.validation.max) {
          errors[field.key] = `최대값은 ${field.validation.max}입니다`
        }
      }

      // 심볼 개수 검사
      if (field.field_type === 'symbol_picker' && Array.isArray(value)) {
        if (field.validation.min_items && value.length < field.validation.min_items) {
          errors[field.key] = `최소 ${field.validation.min_items}개를 선택해주세요`
        }
        if (field.validation.max_items && value.length > field.validation.max_items) {
          errors[field.key] = `최대 ${field.validation.max_items}개까지 선택 가능합니다`
        }
      }
    }

    setEditFormErrors(errors)
    return Object.keys(errors).length === 0
  }

  // 전략 업데이트
  const handleUpdateStrategy = async () => {
    if (!validateEditForm()) return

    const strategyId = editingStrategyId()
    if (!strategyId) return

    setIsUpdating(true)
    setUpdateError(null)

    try {
      // 이름도 config에 포함시켜서 전송
      const configWithName = {
        ...editingParams(),
        name: editingStrategyName(),
      }

      const response = await updateStrategyConfig(strategyId, configWithName)
      console.log('Strategy updated:', response)

      // 모달 닫기 및 목록 새로고침
      closeEditModal()
      refetch()
      // 성공 토스트
      toast.success('전략 업데이트 완료', `"${editingStrategyName()}" 설정이 저장되었습니다`)
    } catch (error) {
      console.error('Failed to update strategy:', error)
      const errorMsg = error instanceof Error ? error.message : '전략 업데이트에 실패했습니다'
      setUpdateError(errorMsg)
      toast.error('전략 업데이트 실패', errorMsg)
    } finally {
      setIsUpdating(false)
    }
  }

  // 편집 모달 닫기
  const closeEditModal = () => {
    setShowEditModal(false)
    setEditingStrategyId(null)
    setEditingStrategyType(null)
    setEditingStrategyName('')
    setEditingParams({})
    setEditFormErrors({})
    setUpdateError(null)
  }

  const filteredStrategies = () => {
    const data = strategies()
    if (!data) return []
    const f = filter()
    if (f === 'all') return data
    if (f === 'running') return data.filter((s) => s.status === 'Running')
    return data.filter((s) => s.status === 'Stopped' || s.status === 'Error')
  }

  const toggleStrategy = async (strategy: Strategy) => {
    setTogglingId(strategy.id)
    const isRunning = strategy.status === 'Running'
    try {
      if (isRunning) {
        await stopStrategy(strategy.id)
        toast.info('전략 중지됨', `"${strategy.name}" 전략이 중지되었습니다`)
      } else {
        await startStrategy(strategy.id)
        toast.success('전략 시작됨', `"${strategy.name}" 전략이 실행되었습니다`)
      }
      // 목록 새로고침
      refetch()
    } catch (error) {
      console.error('Failed to toggle strategy:', error)
      const errorMsg = error instanceof Error ? error.message : '전략 상태 변경에 실패했습니다'
      toast.error(isRunning ? '전략 중지 실패' : '전략 시작 실패', errorMsg)
    } finally {
      setTogglingId(null)
    }
  }

  const runningCount = () => strategies()?.filter((s) => s.status === 'Running').length || 0
  const stoppedCount = () => strategies()?.filter((s) => s.status !== 'Running').length || 0

  return (
    <div class="space-y-6">
      {/* Header */}
      <div class="flex items-center justify-between">
        <div class="flex gap-2">
          <button
            class={`px-4 py-2 rounded-lg font-medium transition-colors ${
              filter() === 'all'
                ? 'bg-[var(--color-primary)] text-white'
                : 'bg-[var(--color-surface)] text-[var(--color-text-muted)] hover:text-[var(--color-text)]'
            }`}
            onClick={() => setFilter('all')}
          >
            전체 ({strategies()?.length || 0})
          </button>
          <button
            class={`px-4 py-2 rounded-lg font-medium transition-colors ${
              filter() === 'running'
                ? 'bg-[var(--color-primary)] text-white'
                : 'bg-[var(--color-surface)] text-[var(--color-text-muted)] hover:text-[var(--color-text)]'
            }`}
            onClick={() => setFilter('running')}
          >
            실행 중 ({runningCount()})
          </button>
          <button
            class={`px-4 py-2 rounded-lg font-medium transition-colors ${
              filter() === 'stopped'
                ? 'bg-[var(--color-primary)] text-white'
                : 'bg-[var(--color-surface)] text-[var(--color-text-muted)] hover:text-[var(--color-text)]'
            }`}
            onClick={() => setFilter('stopped')}
          >
            중지됨 ({stoppedCount()})
          </button>
        </div>

        <div class="flex gap-2">
          <button
            class="px-4 py-2 bg-[var(--color-primary)] text-white rounded-lg font-medium hover:bg-[var(--color-primary)]/90 transition-colors"
            onClick={() => setShowAddModal(true)}
          >
            + 전략 추가
          </button>
          <button
            class="px-4 py-2 bg-[var(--color-surface)] text-[var(--color-text-muted)] rounded-lg font-medium hover:text-[var(--color-text)] transition-colors flex items-center gap-2"
            onClick={() => refetch()}
          >
            <RefreshCw class={`w-4 h-4 ${strategies.loading ? 'animate-spin' : ''}`} />
            새로고침
          </button>
        </div>
      </div>

      {/* Loading State */}
      <Show when={strategies.loading && !strategies()}>
        <div class="flex items-center justify-center py-12">
          <RefreshCw class="w-8 h-8 animate-spin text-[var(--color-primary)]" />
        </div>
      </Show>

      {/* Error State */}
      <Show when={strategies.error}>
        <div class="flex items-center justify-center py-12 text-red-500">
          <AlertCircle class="w-6 h-6 mr-2" />
          전략 목록을 불러오는데 실패했습니다
        </div>
      </Show>

      {/* Empty State */}
      <Show when={!strategies.loading && !strategies.error && (!strategies() || strategies()?.length === 0)}>
        <div class="flex flex-col items-center justify-center py-12 text-[var(--color-text-muted)]">
          <Settings class="w-12 h-12 mb-4 opacity-50" />
          <p class="text-lg mb-2">등록된 전략이 없습니다</p>
          <p class="text-sm">새로운 전략을 추가해 자동 매매를 시작하세요</p>
        </div>
      </Show>

      {/* Strategies Grid */}
      <Show when={filteredStrategies().length > 0}>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <For each={filteredStrategies()}>
            {(strategy) => (
              <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-6">
                {/* Header */}
                <div class="flex items-start justify-between mb-4">
                  <div>
                    <div class="flex items-center gap-2 mb-1">
                      <h3 class="text-lg font-semibold text-[var(--color-text)]">
                        {strategy.name}
                      </h3>
                      <span
                        class={`px-2 py-0.5 text-xs rounded ${
                          strategy.market === 'KR'
                            ? 'bg-blue-500/20 text-blue-400'
                            : strategy.market === 'US'
                            ? 'bg-green-500/20 text-green-400'
                            : 'bg-orange-500/20 text-orange-400'
                        }`}
                      >
                        {strategy.market}
                      </span>
                    </div>
                    <div class="flex items-center gap-2">
                      <div
                        class={`w-2 h-2 rounded-full ${
                          strategy.status === 'Running'
                            ? 'bg-green-500 animate-pulse'
                            : strategy.status === 'Error'
                            ? 'bg-red-500'
                            : 'bg-gray-500'
                        }`}
                      />
                      <span class="text-sm text-[var(--color-text-muted)]">
                        {strategy.status === 'Running'
                          ? '실행 중'
                          : strategy.status === 'Error'
                          ? '오류'
                          : '중지됨'}
                      </span>
                    </div>
                  </div>
                  <div class="flex gap-2">
                    <button
                      class="p-2 rounded-lg hover:bg-[var(--color-surface-light)] transition-colors disabled:opacity-50"
                      onClick={() => toggleStrategy(strategy)}
                      disabled={togglingId() === strategy.id}
                    >
                      <Show when={togglingId() === strategy.id}>
                        <RefreshCw class="w-5 h-5 animate-spin text-[var(--color-text-muted)]" />
                      </Show>
                      <Show when={togglingId() !== strategy.id}>
                        <Show
                          when={strategy.status === 'Running'}
                          fallback={<Play class="w-5 h-5 text-green-500" />}
                        >
                          <Pause class="w-5 h-5 text-yellow-500" />
                        </Show>
                      </Show>
                    </button>
                    <button
                      class="p-2 rounded-lg hover:bg-[var(--color-surface-light)] transition-colors"
                      onClick={() => handleEditStrategy(strategy)}
                      title="전략 설정"
                    >
                      <Settings class="w-5 h-5 text-[var(--color-text-muted)]" />
                    </button>
                  </div>
                </div>

                {/* Symbols & Timeframe */}
                <div class="flex flex-wrap items-center gap-1 mb-4">
                  {/* 타임프레임 배지 */}
                  <span class="px-2 py-0.5 text-xs bg-[var(--color-primary)]/20 text-[var(--color-primary)] rounded font-medium">
                    {strategy.timeframe || getDefaultTimeframe(strategy.strategyType)}
                  </span>
                  {/* 심볼 목록 */}
                  <For each={strategy.symbols}>
                    {(symbol) => (
                      <span class="px-2 py-0.5 text-xs bg-[var(--color-surface-light)] text-[var(--color-text-muted)] rounded">
                        {symbol}
                      </span>
                    )}
                  </For>
                </div>

                {/* Stats */}
                <Show
                  when={strategy.status !== 'Error'}
                  fallback={
                    <div class="flex items-center gap-2 p-3 bg-red-500/10 rounded-lg">
                      <AlertCircle class="w-5 h-5 text-red-500" />
                      <span class="text-sm text-red-500">
                        전략 실행 중 오류가 발생했습니다
                      </span>
                    </div>
                  }
                >
                  <div class="grid grid-cols-3 gap-4">
                    <div>
                      <div class="text-sm text-[var(--color-text-muted)] mb-1">손익</div>
                      <div
                        class={`font-semibold flex items-center gap-1 ${
                          strategy.pnl >= 0 ? 'text-green-500' : 'text-red-500'
                        }`}
                      >
                        <Show
                          when={strategy.pnl >= 0}
                          fallback={<TrendingDown class="w-4 h-4" />}
                        >
                          <TrendingUp class="w-4 h-4" />
                        </Show>
                        {formatCurrency(strategy.pnl)}
                      </div>
                    </div>
                    <div>
                      <div class="text-sm text-[var(--color-text-muted)] mb-1">승률</div>
                      <div class="font-semibold text-[var(--color-text)]">
                        {strategy.winRate.toFixed(1)}%
                      </div>
                    </div>
                    <div>
                      <div class="text-sm text-[var(--color-text-muted)] mb-1">거래</div>
                      <div class="font-semibold text-[var(--color-text)]">
                        {strategy.tradesCount}회
                      </div>
                    </div>
                  </div>
                </Show>

                {/* 빠른 액션 버튼 */}
                <div class="flex gap-2 mt-4 pt-4 border-t border-[var(--color-surface-light)]">
                  <button
                    class="flex-1 flex items-center justify-center gap-2 px-3 py-2 text-sm bg-[var(--color-surface-light)] hover:bg-[var(--color-primary)]/20 text-[var(--color-text-muted)] hover:text-[var(--color-primary)] rounded-lg transition-colors"
                    onClick={() => navigate(`/backtest?strategy=${strategy.id}`)}
                    title="이 전략으로 백테스트"
                  >
                    <BarChart3 class="w-4 h-4" />
                    백테스트
                  </button>
                  <button
                    class="flex-1 flex items-center justify-center gap-2 px-3 py-2 text-sm bg-[var(--color-surface-light)] hover:bg-purple-500/20 text-[var(--color-text-muted)] hover:text-purple-400 rounded-lg transition-colors"
                    onClick={() => navigate(`/simulation?strategy=${strategy.id}`)}
                    title="이 전략으로 시뮬레이션"
                  >
                    <Activity class="w-4 h-4" />
                    시뮬레이션
                  </button>
                </div>
              </div>
            )}
          </For>
        </div>
      </Show>

      {/* ==================== 전략 편집 모달 ==================== */}
      <Show when={showEditModal()}>
        <div class="fixed inset-0 z-50 flex items-center justify-center p-4">
          {/* 배경 오버레이 */}
          <div
            class="absolute inset-0 bg-black/50"
            onClick={closeEditModal}
          />

          {/* 모달 컨텐츠 */}
          <div class="relative w-full max-w-2xl max-h-[90vh] bg-[var(--color-bg)] rounded-2xl shadow-2xl overflow-hidden flex flex-col">
            {/* 헤더 */}
            <div class="flex items-center justify-between p-6 border-b border-[var(--color-surface-light)]">
              <div>
                <h2 class="text-xl font-semibold text-[var(--color-text)]">
                  전략 설정
                </h2>
                <p class="text-sm text-[var(--color-text-muted)]">
                  전략 파라미터를 수정하세요
                </p>
              </div>
              <button
                onClick={closeEditModal}
                class="p-2 hover:bg-[var(--color-surface)] rounded-lg transition-colors"
              >
                <X class="w-5 h-5" />
              </button>
            </div>

            {/* 본문 */}
            <div class="flex-1 overflow-y-auto p-6 space-y-6">
              {/* 로딩 상태 */}
              <Show when={isLoadingStrategy()}>
                <div class="flex items-center justify-center py-12">
                  <RefreshCw class="w-8 h-8 animate-spin text-[var(--color-primary)]" />
                </div>
              </Show>

              {/* 로딩 완료 후 폼 표시 */}
              <Show when={!isLoadingStrategy() && getEditingTemplate()}>
                {/* 전략 정보 카드 */}
                <div class="p-4 bg-[var(--color-surface)] rounded-lg space-y-3">
                  {/* 실행 주기 배지 */}
                  <Show when={getEditingTemplate()?.execution_schedule}>
                    <div class="flex items-center gap-2">
                      <span class="px-2 py-1 text-xs bg-blue-500/20 text-blue-400 rounded-lg font-medium">
                        ⏰ {getEditingTemplate()?.schedule_detail || getEditingTemplate()?.execution_schedule}
                      </span>
                      <Show when={getEditingTemplate()?.category}>
                        <span class="px-2 py-1 text-xs bg-[var(--color-primary)]/20 text-[var(--color-primary)] rounded-lg font-medium">
                          {getEditingTemplate()?.category}
                        </span>
                      </Show>
                    </div>
                  </Show>

                  {/* 기본 설명 */}
                  <p class="text-sm text-[var(--color-text-muted)]">
                    {getEditingTemplate()?.description}
                  </p>

                  {/* 작동 방식 상세 설명 */}
                  <Show when={getEditingTemplate()?.how_it_works}>
                    <div class="pt-3 border-t border-[var(--color-surface-light)]">
                      <h4 class="text-xs font-semibold text-[var(--color-text)] mb-1.5">📖 작동 방식</h4>
                      <p class="text-xs text-[var(--color-text-muted)] leading-relaxed">
                        {getEditingTemplate()?.how_it_works}
                      </p>
                    </div>
                  </Show>

                  {/* 태그 */}
                  <Show when={getEditingTemplate()?.tags?.length}>
                    <div class="flex flex-wrap gap-1 pt-2">
                      <For each={getEditingTemplate()?.tags}>
                        {(tag) => (
                          <span class="px-2 py-0.5 text-xs bg-[var(--color-bg)] text-[var(--color-text-muted)] rounded">
                            #{tag}
                          </span>
                        )}
                      </For>
                    </div>
                  </Show>
                </div>

                {/* 전략 이름 */}
                <div>
                  <label class="block text-sm font-medium text-[var(--color-text)] mb-2">
                    전략 이름
                  </label>
                  <input
                    type="text"
                    value={editingStrategyName()}
                    onInput={(e) => setEditingStrategyName(e.currentTarget.value)}
                    placeholder="전략 이름을 입력하세요"
                    class="w-full px-4 py-2.5 bg-[var(--color-surface)] border border-[var(--color-surface-light)] rounded-lg text-[var(--color-text)] placeholder:text-[var(--color-text-muted)] focus:outline-none focus:ring-2 focus:ring-[var(--color-primary)]"
                  />
                </div>

                {/* 동적 폼 */}
                <Show
                  when={getEditingTemplate()?.ui_schema}
                  fallback={
                    <div class="text-center py-8 text-[var(--color-text-muted)]">
                      <p>이 전략은 추가 설정이 필요하지 않습니다</p>
                    </div>
                  }
                >
                  <DynamicForm
                    schema={getEditingTemplate()!.ui_schema!}
                    values={editingParams()}
                    onChange={handleEditParamChange}
                    errors={editFormErrors()}
                  />
                </Show>
              </Show>

              {/* 템플릿을 찾을 수 없는 경우 */}
              <Show when={!isLoadingStrategy() && !getEditingTemplate() && !updateError()}>
                <div class="text-center py-8 text-[var(--color-text-muted)]">
                  <AlertCircle class="w-12 h-12 mx-auto mb-4 opacity-50" />
                  <p>전략 템플릿을 찾을 수 없습니다</p>
                </div>
              </Show>
            </div>

            {/* 푸터 */}
            <div class="flex items-center justify-between p-6 border-t border-[var(--color-surface-light)]">
              {/* 에러 메시지 */}
              <Show when={updateError()}>
                <div class="flex items-center gap-2 text-red-500 text-sm">
                  <AlertCircle class="w-4 h-4" />
                  <span>{updateError()}</span>
                </div>
              </Show>
              <Show when={!updateError()}>
                <div />
              </Show>

              <div class="flex items-center gap-3">
                <button
                  onClick={closeEditModal}
                  class="px-4 py-2 text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors"
                  disabled={isUpdating()}
                >
                  취소
                </button>
                <button
                  onClick={handleUpdateStrategy}
                  disabled={isUpdating() || isLoadingStrategy() || !getEditingTemplate()}
                  class="px-6 py-2 bg-[var(--color-primary)] text-white rounded-lg font-medium hover:bg-[var(--color-primary)]/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
                >
                  <Show when={isUpdating()}>
                    <RefreshCw class="w-4 h-4 animate-spin" />
                  </Show>
                  {isUpdating() ? '저장 중...' : '변경 저장'}
                </button>
              </div>
            </div>
          </div>
        </div>
      </Show>

      {/* ==================== 전략 추가 모달 ==================== */}
      <Show when={showAddModal()}>
        <div class="fixed inset-0 z-50 flex items-center justify-center p-4">
          {/* 배경 오버레이 */}
          <div
            class="absolute inset-0 bg-black/50"
            onClick={closeModal}
          />

          {/* 모달 컨텐츠 */}
          <div class="relative w-full max-w-4xl max-h-[90vh] bg-[var(--color-bg)] rounded-2xl shadow-2xl overflow-hidden flex flex-col">
            {/* 헤더 */}
            <div class="flex items-center justify-between p-6 border-b border-[var(--color-surface-light)]">
              <div class="flex items-center gap-3">
                <Show when={modalStep() === 'configure'}>
                  <button
                    onClick={goBack}
                    class="p-2 hover:bg-[var(--color-surface)] rounded-lg transition-colors"
                  >
                    <ChevronRight class="w-5 h-5 rotate-180" />
                  </button>
                </Show>
                <div>
                  <h2 class="text-xl font-semibold text-[var(--color-text)]">
                    {modalStep() === 'select' ? '전략 선택' : selectedStrategy()?.name}
                  </h2>
                  <p class="text-sm text-[var(--color-text-muted)]">
                    {modalStep() === 'select'
                      ? '자동 매매에 사용할 전략을 선택하세요'
                      : '전략 파라미터를 설정하세요'}
                  </p>
                </div>
              </div>
              <button
                onClick={closeModal}
                class="p-2 hover:bg-[var(--color-surface)] rounded-lg transition-colors"
              >
                <X class="w-5 h-5" />
              </button>
            </div>

            {/* 본문 */}
            <div class="flex-1 overflow-y-auto">
              {/* Step 1: 전략 선택 */}
              <Show when={modalStep() === 'select'}>
                <div class="p-6 space-y-6">
                  {/* 검색 및 필터 */}
                  <div class="flex gap-4">
                    {/* 검색 */}
                    <div class="flex-1 relative">
                      <Search class="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-[var(--color-text-muted)]" />
                      <input
                        type="text"
                        value={searchQuery()}
                        onInput={(e) => setSearchQuery(e.currentTarget.value)}
                        placeholder="전략 검색..."
                        class="w-full pl-10 pr-4 py-2.5 bg-[var(--color-surface)] border border-[var(--color-surface-light)] rounded-lg text-[var(--color-text)] placeholder:text-[var(--color-text-muted)] focus:outline-none focus:ring-2 focus:ring-[var(--color-primary)]"
                      />
                    </div>
                  </div>

                  {/* 카테고리 필터 */}
                  <div class="flex flex-wrap gap-2">
                    <button
                      class={`px-3 py-1.5 rounded-lg text-sm transition-colors ${
                        selectedCategory() === null
                          ? 'bg-[var(--color-primary)] text-white'
                          : 'bg-[var(--color-surface)] text-[var(--color-text-muted)] hover:text-[var(--color-text)]'
                      }`}
                      onClick={() => setSelectedCategory(null)}
                    >
                      전체
                    </button>
                    <For each={categories()}>
                      {(category) => (
                        <button
                          class={`px-3 py-1.5 rounded-lg text-sm transition-colors ${
                            selectedCategory() === category
                              ? 'bg-[var(--color-primary)] text-white'
                              : 'bg-[var(--color-surface)] text-[var(--color-text-muted)] hover:text-[var(--color-text)]'
                          }`}
                          onClick={() => setSelectedCategory(category)}
                        >
                          {category}
                        </button>
                      )}
                    </For>
                  </div>

                  {/* 전략 목록 */}
                  <Show
                    when={!strategyTemplates.loading}
                    fallback={
                      <div class="flex items-center justify-center py-12">
                        <RefreshCw class="w-8 h-8 animate-spin text-[var(--color-primary)]" />
                      </div>
                    }
                  >
                    <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                      <For each={filteredTemplates()}>
                        {(template) => (
                          <button
                            class="text-left p-4 bg-[var(--color-surface)] border border-[var(--color-surface-light)] rounded-xl hover:border-[var(--color-primary)] hover:bg-[var(--color-surface-light)] transition-all group"
                            onClick={() => selectStrategy(template)}
                          >
                            <div class="flex items-start justify-between mb-2">
                              <h3 class="font-semibold text-[var(--color-text)] group-hover:text-[var(--color-primary)]">
                                {template.name}
                              </h3>
                              <div class="flex gap-1">
                                <Show when={template.execution_schedule}>
                                  <span class="px-2 py-0.5 text-xs bg-blue-500/20 text-blue-400 rounded">
                                    {template.schedule_detail || template.execution_schedule}
                                  </span>
                                </Show>
                                <Show when={template.category}>
                                  <span class="px-2 py-0.5 text-xs bg-[var(--color-primary)]/20 text-[var(--color-primary)] rounded">
                                    {template.category}
                                  </span>
                                </Show>
                              </div>
                            </div>
                            <p class="text-sm text-[var(--color-text-muted)] mb-3 line-clamp-2">
                              {template.description}
                            </p>
                            <div class="flex flex-wrap gap-1">
                              <For each={template.tags?.slice(0, 3)}>
                                {(tag) => (
                                  <span class="px-2 py-0.5 text-xs bg-[var(--color-bg)] text-[var(--color-text-muted)] rounded">
                                    #{tag}
                                  </span>
                                )}
                              </For>
                            </div>
                          </button>
                        )}
                      </For>
                    </div>
                  </Show>

                  {/* 빈 결과 */}
                  <Show when={filteredTemplates().length === 0 && !strategyTemplates.loading}>
                    <div class="text-center py-12 text-[var(--color-text-muted)]">
                      <p class="mb-2">검색 결과가 없습니다</p>
                      <p class="text-sm">다른 검색어를 시도해보세요</p>
                    </div>
                  </Show>
                </div>
              </Show>

              {/* Step 2: 파라미터 설정 */}
              <Show when={modalStep() === 'configure' && selectedStrategy()}>
                <div class="p-6 space-y-6">
                  {/* 전략 정보 카드 */}
                  <div class="p-4 bg-[var(--color-surface)] rounded-lg space-y-3">
                    {/* 실행 주기 배지 */}
                    <Show when={selectedStrategy()?.execution_schedule}>
                      <div class="flex items-center gap-2">
                        <span class="px-2 py-1 text-xs bg-blue-500/20 text-blue-400 rounded-lg font-medium">
                          ⏰ {selectedStrategy()?.schedule_detail || selectedStrategy()?.execution_schedule}
                        </span>
                        <Show when={selectedStrategy()?.category}>
                          <span class="px-2 py-1 text-xs bg-[var(--color-primary)]/20 text-[var(--color-primary)] rounded-lg font-medium">
                            {selectedStrategy()?.category}
                          </span>
                        </Show>
                      </div>
                    </Show>

                    {/* 기본 설명 */}
                    <p class="text-sm text-[var(--color-text-muted)]">
                      {selectedStrategy()?.description}
                    </p>

                    {/* 작동 방식 상세 설명 */}
                    <Show when={selectedStrategy()?.how_it_works}>
                      <div class="pt-3 border-t border-[var(--color-surface-light)]">
                        <h4 class="text-xs font-semibold text-[var(--color-text)] mb-1.5">📖 작동 방식</h4>
                        <p class="text-xs text-[var(--color-text-muted)] leading-relaxed">
                          {selectedStrategy()?.how_it_works}
                        </p>
                      </div>
                    </Show>

                    {/* 태그 */}
                    <Show when={selectedStrategy()?.tags?.length}>
                      <div class="flex flex-wrap gap-1 pt-2">
                        <For each={selectedStrategy()?.tags}>
                          {(tag) => (
                            <span class="px-2 py-0.5 text-xs bg-[var(--color-bg)] text-[var(--color-text-muted)] rounded">
                              #{tag}
                            </span>
                          )}
                        </For>
                      </div>
                    </Show>
                  </div>

                  {/* 전략 이름 커스터마이징 */}
                  <div>
                    <label class="block text-sm font-medium text-[var(--color-text)] mb-2">
                      전략 이름
                    </label>
                    <input
                      type="text"
                      value={customName()}
                      onInput={(e) => setCustomName(e.currentTarget.value)}
                      placeholder="전략 이름을 입력하세요"
                      class="w-full px-4 py-2.5 bg-[var(--color-surface)] border border-[var(--color-surface-light)] rounded-lg text-[var(--color-text)] placeholder:text-[var(--color-text-muted)] focus:outline-none focus:ring-2 focus:ring-[var(--color-primary)]"
                    />
                    <p class="mt-1 text-xs text-[var(--color-text-muted)]">
                      동일한 전략을 다른 종목이나 설정으로 여러 개 등록할 수 있습니다.
                    </p>
                  </div>

                  {/* 타임프레임 선택 */}
                  <div>
                    <label class="block text-sm font-medium text-[var(--color-text)] mb-2">
                      타임프레임
                    </label>
                    <select
                      value={(strategyParams() as Record<string, unknown>).timeframe as string || getDefaultTimeframe(selectedStrategy()?.id || '')}
                      onChange={(e) => handleParamChange('timeframe', e.currentTarget.value)}
                      class="w-full px-4 py-2.5 bg-[var(--color-surface)] border border-[var(--color-surface-light)] rounded-lg text-[var(--color-text)] focus:outline-none focus:ring-2 focus:ring-[var(--color-primary)]"
                    >
                      <optgroup label="실시간/분봉">
                        <option value="1m">1분 (실시간)</option>
                        <option value="5m">5분</option>
                        <option value="15m">15분</option>
                        <option value="30m">30분</option>
                        <option value="1h">1시간</option>
                        <option value="4h">4시간</option>
                      </optgroup>
                      <optgroup label="일봉/주봉">
                        <option value="1d">일봉</option>
                        <option value="1w">주봉</option>
                        <option value="1M">월봉</option>
                      </optgroup>
                    </select>
                    <p class="mt-1 text-xs text-[var(--color-text-muted)]">
                      전략 실행에 사용할 캔들 주기를 선택하세요.
                    </p>
                  </div>

                  {/* 동적 폼 */}
                  <Show
                    when={selectedStrategy()?.ui_schema}
                    fallback={
                      <div class="text-center py-8 text-[var(--color-text-muted)]">
                        <p>이 전략은 추가 설정이 필요하지 않습니다</p>
                      </div>
                    }
                  >
                    <DynamicForm
                      schema={selectedStrategy()!.ui_schema!}
                      values={strategyParams()}
                      onChange={handleParamChange}
                      errors={formErrors()}
                    />
                  </Show>
                </div>
              </Show>
            </div>

            {/* 푸터 */}
            <div class="flex items-center justify-between p-6 border-t border-[var(--color-surface-light)]">
              {/* 에러 메시지 */}
              <Show when={createError()}>
                <div class="flex items-center gap-2 text-red-500 text-sm">
                  <AlertCircle class="w-4 h-4" />
                  <span>{createError()}</span>
                </div>
              </Show>
              <Show when={!createError()}>
                <div />
              </Show>

              <div class="flex items-center gap-3">
                <button
                  onClick={closeModal}
                  class="px-4 py-2 text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors"
                  disabled={isCreating()}
                >
                  취소
                </button>
                <Show when={modalStep() === 'configure'}>
                  <button
                    onClick={handleCreateStrategy}
                    disabled={isCreating()}
                    class="px-6 py-2 bg-[var(--color-primary)] text-white rounded-lg font-medium hover:bg-[var(--color-primary)]/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
                  >
                    <Show when={isCreating()}>
                      <RefreshCw class="w-4 h-4 animate-spin" />
                    </Show>
                    {isCreating() ? '생성 중...' : '전략 생성'}
                  </button>
                </Show>
              </div>
            </div>
          </div>
        </div>
      </Show>
    </div>
  )
}
