import { createSignal, For, Show, createResource, onCleanup } from 'solid-js'
import {
  Brain,
  Play,
  Square,
  Trash2,
  Download,
  RefreshCw,
  CheckCircle,
  XCircle,
  Clock,
  Loader2,
  Plus,
  ChevronDown,
  ChevronUp,
  Zap,
  Database,
  TrendingUp,
  BarChart3,
} from 'lucide-solid'
import {
  startTraining,
  getTrainingJobs,
  getTrainedModels,
  cancelTraining,
  deleteModel,
  activateModel,
  MODEL_TYPE_NAMES,
  TRAINING_STATUS_NAMES,
  PERIOD_OPTIONS,
  HORIZON_OPTIONS,
  type TrainingRequest,
  type TrainingJob,
  type TrainedModel,
  type ModelType,
  type TrainingStatus,
} from '../api/ml'
import { useToast } from '../components/Toast'

// 인기 심볼 카테고리 (프론트엔드에서 정의)
const POPULAR_SYMBOLS = {
  us_index: { name: 'US 지수 ETF', symbols: ['SPY', 'QQQ', 'DIA', 'IWM'] },
  us_leverage: { name: 'US 레버리지', symbols: ['TQQQ', 'SQQQ', 'UPRO', 'SPXU'] },
  us_sector: { name: 'US 섹터', symbols: ['XLK', 'XLF', 'XLE', 'XLV'] },
  us_bond: { name: 'US 채권', symbols: ['TLT', 'IEF', 'SHY', 'BND'] },
  crypto: { name: '암호화폐', symbols: ['BTC-USD', 'ETH-USD'] },
}

export function MLTraining() {
  const toast = useToast()

  // ==================== 리소스 ====================
  const [jobs, { refetch: refetchJobs }] = createResource(async () => {
    try {
      return await getTrainingJobs()
    } catch {
      return []
    }
  })

  const [models, { refetch: refetchModels }] = createResource(async () => {
    try {
      const response = await getTrainedModels()
      return response.models
    } catch {
      return []
    }
  })

  // ==================== 폼 상태 ====================
  const [showForm, setShowForm] = createSignal(false)
  const [modelType, setModelType] = createSignal<ModelType>('xgboost')
  const [selectedSymbols, setSelectedSymbols] = createSignal<string[]>(['SPY'])
  const [customSymbol, setCustomSymbol] = createSignal('')
  const [period, setPeriod] = createSignal('5y')
  const [horizon, setHorizon] = createSignal(5)
  const [modelName, setModelName] = createSignal('')
  const [isSubmitting, setIsSubmitting] = createSignal(false)

  // ==================== 자동 새로고침 ====================
  let refreshInterval: number | undefined

  const startAutoRefresh = () => {
    refreshInterval = setInterval(() => {
      const runningJobs = jobs()?.filter(j => j.status === 'running' || j.status === 'pending')
      if (runningJobs && runningJobs.length > 0) {
        refetchJobs()
      }
    }, 5000)
  }

  startAutoRefresh()
  onCleanup(() => {
    if (refreshInterval) clearInterval(refreshInterval)
  })

  // ==================== 핸들러 ====================
  const addSymbol = (symbol: string) => {
    if (!selectedSymbols().includes(symbol)) {
      setSelectedSymbols([...selectedSymbols(), symbol])
    }
  }

  const removeSymbol = (symbol: string) => {
    setSelectedSymbols(selectedSymbols().filter(s => s !== symbol))
  }

  const addCustomSymbol = () => {
    const symbol = customSymbol().trim().toUpperCase()
    if (symbol && !selectedSymbols().includes(symbol)) {
      setSelectedSymbols([...selectedSymbols(), symbol])
      setCustomSymbol('')
    }
  }

  const handleSubmit = async () => {
    if (selectedSymbols().length === 0) {
      toast.warning('심볼 필요', '최소 1개의 심볼을 선택하세요.')
      return
    }

    setIsSubmitting(true)

    try {
      const request: TrainingRequest = {
        modelType: modelType(),
        symbols: selectedSymbols(),
        period: period(),
        horizon: horizon(),
        name: modelName() || undefined,
      }

      const response = await startTraining(request)
      if (response.success) {
        toast.success('훈련 시작', response.message)
        setShowForm(false)
        refetchJobs()
      } else {
        toast.error('훈련 실패', response.message)
      }
    } catch (e) {
      toast.error('오류', '훈련을 시작할 수 없습니다.')
    } finally {
      setIsSubmitting(false)
    }
  }

  const handleCancelJob = async (jobId: string) => {
    try {
      const response = await cancelTraining(jobId)
      if (response.success) {
        toast.info('취소됨', response.message)
        refetchJobs()
      }
    } catch {
      toast.error('오류', '훈련을 취소할 수 없습니다.')
    }
  }

  const handleDeleteModel = async (modelId: string) => {
    if (!confirm('이 모델을 삭제하시겠습니까?')) return

    try {
      const response = await deleteModel(modelId)
      if (response.success) {
        toast.success('삭제됨', response.message)
        refetchModels()
      }
    } catch {
      toast.error('오류', '모델을 삭제할 수 없습니다.')
    }
  }

  const handleActivateModel = async (modelId: string) => {
    try {
      const response = await activateModel(modelId)
      if (response.success) {
        toast.success('활성화됨', response.message)
        refetchModels()
      }
    } catch {
      toast.error('오류', '모델을 활성화할 수 없습니다.')
    }
  }

  // ==================== 상태 아이콘 ====================
  const StatusIcon = (props: { status: TrainingStatus }) => {
    switch (props.status) {
      case 'completed':
        return <CheckCircle class="w-5 h-5 text-green-500" />
      case 'failed':
        return <XCircle class="w-5 h-5 text-red-500" />
      case 'running':
        return <Loader2 class="w-5 h-5 text-blue-500 animate-spin" />
      case 'pending':
        return <Clock class="w-5 h-5 text-yellow-500" />
      default:
        return null
    }
  }

  return (
    <div class="space-y-6">
      {/* 헤더 */}
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-3">
          <Brain class="w-8 h-8 text-purple-500" />
          <div>
            <h1 class="text-2xl font-bold text-[var(--color-text)]">ML 모델 훈련</h1>
            <p class="text-sm text-[var(--color-text-muted)]">
              Yahoo Finance 데이터로 ONNX 모델을 훈련합니다
            </p>
          </div>
        </div>
        <button
          onClick={() => setShowForm(!showForm())}
          class="flex items-center gap-2 px-4 py-2 bg-purple-600 hover:bg-purple-700 text-white rounded-lg transition-colors"
        >
          <Plus class="w-4 h-4" />
          새 훈련
        </button>
      </div>

      {/* 훈련 폼 */}
      <Show when={showForm()}>
        <div class="card p-6 space-y-6">
          <h2 class="text-lg font-semibold text-[var(--color-text)]">새 모델 훈련</h2>

          {/* 모델 유형 */}
          <div>
            <label class="block text-sm font-medium text-[var(--color-text-muted)] mb-2">
              모델 유형
            </label>
            <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
              <For each={Object.entries(MODEL_TYPE_NAMES)}>
                {([type, name]) => (
                  <button
                    onClick={() => setModelType(type as ModelType)}
                    class={`p-3 rounded-lg border-2 transition-all ${
                      modelType() === type
                        ? 'border-purple-500 bg-purple-500/10'
                        : 'border-[var(--color-border)] hover:border-purple-500/50'
                    }`}
                  >
                    <span class="text-sm font-medium text-[var(--color-text)]">{name}</span>
                  </button>
                )}
              </For>
            </div>
          </div>

          {/* 심볼 선택 */}
          <div>
            <label class="block text-sm font-medium text-[var(--color-text-muted)] mb-2">
              훈련 심볼
            </label>

            {/* 선택된 심볼 */}
            <div class="flex flex-wrap gap-2 mb-3">
              <For each={selectedSymbols()}>
                {(symbol) => (
                  <span class="inline-flex items-center gap-1 px-3 py-1 bg-purple-500/20 text-purple-400 rounded-full text-sm">
                    {symbol}
                    <button onClick={() => removeSymbol(symbol)} class="hover:text-purple-300">
                      <XCircle class="w-4 h-4" />
                    </button>
                  </span>
                )}
              </For>
            </div>

            {/* 인기 심볼 */}
            <div class="space-y-2 mb-3">
              <For each={Object.entries(POPULAR_SYMBOLS)}>
                {([_, category]) => (
                  <div class="flex items-center gap-2 flex-wrap">
                    <span class="text-xs text-[var(--color-text-muted)] w-20">{category.name}:</span>
                    <For each={category.symbols}>
                      {(symbol) => (
                        <button
                          onClick={() => addSymbol(symbol)}
                          disabled={selectedSymbols().includes(symbol)}
                          class={`px-2 py-1 text-xs rounded ${
                            selectedSymbols().includes(symbol)
                              ? 'bg-[var(--color-bg-tertiary)] text-[var(--color-text-muted)]'
                              : 'bg-[var(--color-bg-secondary)] hover:bg-purple-500/20 text-[var(--color-text)]'
                          }`}
                        >
                          {symbol}
                        </button>
                      )}
                    </For>
                  </div>
                )}
              </For>
            </div>

            {/* 커스텀 심볼 입력 */}
            <div class="flex gap-2">
              <input
                type="text"
                value={customSymbol()}
                onInput={(e) => setCustomSymbol(e.currentTarget.value)}
                onKeyPress={(e) => e.key === 'Enter' && addCustomSymbol()}
                placeholder="심볼 직접 입력 (예: AAPL)"
                class="flex-1 px-3 py-2 bg-[var(--color-bg-secondary)] border border-[var(--color-border)] rounded-lg text-[var(--color-text)] text-sm"
              />
              <button
                onClick={addCustomSymbol}
                class="px-4 py-2 bg-[var(--color-bg-tertiary)] hover:bg-purple-500/20 text-[var(--color-text)] rounded-lg text-sm"
              >
                추가
              </button>
            </div>
          </div>

          {/* 기간 & 예측 horizon */}
          <div class="grid grid-cols-2 gap-4">
            <div>
              <label class="block text-sm font-medium text-[var(--color-text-muted)] mb-2">
                데이터 기간
              </label>
              <select
                value={period()}
                onChange={(e) => setPeriod(e.currentTarget.value)}
                class="w-full px-3 py-2 bg-[#1e1e2e] border border-[var(--color-border)] rounded-lg text-white"
              >
                <For each={PERIOD_OPTIONS}>
                  {(option) => (
                    <option value={option.value} class="bg-[#1e1e2e] text-white">
                      {option.label}
                    </option>
                  )}
                </For>
              </select>
            </div>
            <div>
              <label class="block text-sm font-medium text-[var(--color-text-muted)] mb-2">
                예측 기간
              </label>
              <select
                value={horizon()}
                onChange={(e) => setHorizon(parseInt(e.currentTarget.value))}
                class="w-full px-3 py-2 bg-[#1e1e2e] border border-[var(--color-border)] rounded-lg text-white"
              >
                <For each={HORIZON_OPTIONS}>
                  {(option) => (
                    <option value={option.value} class="bg-[#1e1e2e] text-white">
                      {option.label}
                    </option>
                  )}
                </For>
              </select>
            </div>
          </div>

          {/* 모델 이름 (선택) */}
          <div>
            <label class="block text-sm font-medium text-[var(--color-text-muted)] mb-2">
              모델 이름 (선택사항)
            </label>
            <input
              type="text"
              value={modelName()}
              onInput={(e) => setModelName(e.currentTarget.value)}
              placeholder="자동 생성됨"
              class="w-full px-3 py-2 bg-[var(--color-bg-secondary)] border border-[var(--color-border)] rounded-lg text-[var(--color-text)]"
            />
          </div>

          {/* 제출 버튼 */}
          <div class="flex justify-end gap-3">
            <button
              onClick={() => setShowForm(false)}
              class="px-4 py-2 text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors"
            >
              취소
            </button>
            <button
              onClick={handleSubmit}
              disabled={isSubmitting() || selectedSymbols().length === 0}
              class="flex items-center gap-2 px-6 py-2 bg-purple-600 hover:bg-purple-700 disabled:bg-purple-600/50 text-white rounded-lg transition-colors"
            >
              <Show when={isSubmitting()} fallback={<Play class="w-4 h-4" />}>
                <Loader2 class="w-4 h-4 animate-spin" />
              </Show>
              훈련 시작
            </button>
          </div>
        </div>
      </Show>

      {/* 훈련 작업 목록 */}
      <div class="card p-6">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-lg font-semibold text-[var(--color-text)] flex items-center gap-2">
            <BarChart3 class="w-5 h-5 text-blue-500" />
            훈련 작업
          </h2>
          <button
            onClick={() => refetchJobs()}
            class="p-2 hover:bg-[var(--color-bg-secondary)] rounded-lg transition-colors"
          >
            <RefreshCw class="w-4 h-4 text-[var(--color-text-muted)]" />
          </button>
        </div>

        <Show
          when={jobs() && jobs()!.length > 0}
          fallback={
            <div class="text-center py-8 text-[var(--color-text-muted)]">
              <Clock class="w-12 h-12 mx-auto mb-3 opacity-50" />
              <p>훈련 작업이 없습니다</p>
            </div>
          }
        >
          <div class="space-y-3">
            <For each={jobs()}>
              {(job) => (
                <div class="p-4 bg-[var(--color-bg-secondary)] rounded-lg">
                  <div class="flex items-center justify-between">
                    <div class="flex items-center gap-3">
                      <StatusIcon status={job.status} />
                      <div>
                        <p class="font-medium text-[var(--color-text)]">{job.name}</p>
                        <p class="text-sm text-[var(--color-text-muted)]">
                          {MODEL_TYPE_NAMES[job.modelType]} · {job.symbols.join(', ')} · {job.period}
                        </p>
                      </div>
                    </div>
                    <div class="flex items-center gap-2">
                      <span
                        class={`px-2 py-1 rounded text-xs ${
                          job.status === 'completed'
                            ? 'bg-green-500/20 text-green-400'
                            : job.status === 'failed'
                            ? 'bg-red-500/20 text-red-400'
                            : job.status === 'running'
                            ? 'bg-blue-500/20 text-blue-400'
                            : 'bg-yellow-500/20 text-yellow-400'
                        }`}
                      >
                        {TRAINING_STATUS_NAMES[job.status]}
                      </span>
                      <Show when={job.status === 'running' || job.status === 'pending'}>
                        <button
                          onClick={() => handleCancelJob(job.id)}
                          class="p-1 hover:bg-red-500/20 rounded text-red-400"
                        >
                          <Square class="w-4 h-4" />
                        </button>
                      </Show>
                    </div>
                  </div>

                  {/* 진행률 바 */}
                  <Show when={job.status === 'running'}>
                    <div class="mt-3">
                      <div class="h-2 bg-[var(--color-bg-tertiary)] rounded-full overflow-hidden">
                        <div
                          class="h-full bg-blue-500 transition-all duration-300"
                          style={{ width: `${job.progress}%` }}
                        />
                      </div>
                      <p class="text-xs text-[var(--color-text-muted)] mt-1">{job.progress}% 완료</p>
                    </div>
                  </Show>

                  {/* 메트릭 (완료된 경우) */}
                  <Show when={job.status === 'completed' && job.metrics}>
                    <div class="mt-3 grid grid-cols-3 gap-4 text-sm">
                      <div>
                        <p class="text-[var(--color-text-muted)]">정확도</p>
                        <p class="text-[var(--color-text)] font-medium">
                          {(job.metrics!.accuracy * 100).toFixed(1)}%
                        </p>
                      </div>
                      <div>
                        <p class="text-[var(--color-text-muted)]">AUC</p>
                        <p class="text-[var(--color-text)] font-medium">{job.metrics!.auc.toFixed(3)}</p>
                      </div>
                      <div>
                        <p class="text-[var(--color-text-muted)]">CV 정확도</p>
                        <p class="text-[var(--color-text)] font-medium">
                          {(job.metrics!.cvAccuracy * 100).toFixed(1)}%
                        </p>
                      </div>
                    </div>
                  </Show>

                  {/* 에러 메시지 */}
                  <Show when={job.status === 'failed' && job.error}>
                    <div class="mt-3 p-2 bg-red-500/10 border border-red-500/20 rounded text-sm text-red-400">
                      {job.error}
                    </div>
                  </Show>
                </div>
              )}
            </For>
          </div>
        </Show>
      </div>

      {/* 훈련된 모델 목록 */}
      <div class="card p-6">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-lg font-semibold text-[var(--color-text)] flex items-center gap-2">
            <Database class="w-5 h-5 text-green-500" />
            훈련된 모델
          </h2>
          <button
            onClick={() => refetchModels()}
            class="p-2 hover:bg-[var(--color-bg-secondary)] rounded-lg transition-colors"
          >
            <RefreshCw class="w-4 h-4 text-[var(--color-text-muted)]" />
          </button>
        </div>

        <Show
          when={models() && models()!.length > 0}
          fallback={
            <div class="text-center py-8 text-[var(--color-text-muted)]">
              <Database class="w-12 h-12 mx-auto mb-3 opacity-50" />
              <p>훈련된 모델이 없습니다</p>
            </div>
          }
        >
          <div class="space-y-3">
            <For each={models()}>
              {(model) => (
                <div class="p-4 bg-[var(--color-bg-secondary)] rounded-lg">
                  <div class="flex items-center justify-between">
                    <div>
                      <p class="font-medium text-[var(--color-text)]">{model.name}</p>
                      <p class="text-sm text-[var(--color-text-muted)]">
                        {MODEL_TYPE_NAMES[model.modelType]} · {model.symbols.join(', ')}
                      </p>
                      <p class="text-xs text-[var(--color-text-muted)] mt-1">
                        생성: {new Date(model.createdAt).toLocaleDateString('ko-KR')}
                      </p>
                    </div>
                    <div class="flex items-center gap-2">
                      <button
                        onClick={() => handleActivateModel(model.id)}
                        class="flex items-center gap-1 px-3 py-1 bg-green-500/20 hover:bg-green-500/30 text-green-400 rounded text-sm"
                        title="추론에 사용"
                      >
                        <Zap class="w-4 h-4" />
                        활성화
                      </button>
                      <button
                        onClick={() => handleDeleteModel(model.id)}
                        class="p-2 hover:bg-red-500/20 rounded text-red-400"
                        title="삭제"
                      >
                        <Trash2 class="w-4 h-4" />
                      </button>
                    </div>
                  </div>

                  {/* 메트릭 */}
                  <div class="mt-3 grid grid-cols-4 gap-4 text-sm">
                    <div>
                      <p class="text-[var(--color-text-muted)]">정확도</p>
                      <p class="text-[var(--color-text)] font-medium">
                        {(model.metrics.accuracy * 100).toFixed(1)}%
                      </p>
                    </div>
                    <div>
                      <p class="text-[var(--color-text-muted)]">AUC</p>
                      <p class="text-[var(--color-text)] font-medium">{model.metrics.auc.toFixed(3)}</p>
                    </div>
                    <div>
                      <p class="text-[var(--color-text-muted)]">훈련 샘플</p>
                      <p class="text-[var(--color-text)] font-medium">
                        {model.metrics.trainSamples.toLocaleString()}
                      </p>
                    </div>
                    <div>
                      <p class="text-[var(--color-text-muted)]">피처 수</p>
                      <p class="text-[var(--color-text)] font-medium">{model.metrics.features}</p>
                    </div>
                  </div>
                </div>
              )}
            </For>
          </div>
        </Show>
      </div>

      {/* 도움말 */}
      <div class="card p-6 bg-purple-500/5 border border-purple-500/20">
        <h3 class="font-semibold text-[var(--color-text)] mb-2 flex items-center gap-2">
          <TrendingUp class="w-5 h-5 text-purple-500" />
          사용 가이드
        </h3>
        <ul class="text-sm text-[var(--color-text-muted)] space-y-1">
          <li>• Yahoo Finance에서 과거 데이터를 자동으로 다운로드합니다</li>
          <li>• 40개 이상의 기술적 지표를 피처로 사용합니다 (RSI, MACD, 볼린저밴드 등)</li>
          <li>• 훈련된 모델은 ONNX 형식으로 저장되어 Rust에서 추론에 사용됩니다</li>
          <li>• XGBoost를 기본 모델로 추천합니다 (빠르고 정확함)</li>
          <li>• 다양한 심볼로 훈련하면 일반화 성능이 향상됩니다</li>
        </ul>
      </div>
    </div>
  )
}
