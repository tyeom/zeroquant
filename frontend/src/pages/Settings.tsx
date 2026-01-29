import { createSignal, For, Show, createResource, createEffect } from 'solid-js'
import { Save, Key, Bell, Shield, Database, Globe, Moon, Sun, Send, MessageCircle, CheckCircle, XCircle, RefreshCw, Play, Plus, Trash2, TestTube, Building2, Bot, ChevronDown, ChevronRight, BellRing } from 'lucide-solid'
import { useToast } from '../components/Toast'
import {
  getNotificationSettings,
  getNotificationTemplates,
  testTelegram,
  testTelegramEnv,
  testTelegramTemplate,
  testAllTelegramTemplates,
  getSupportedExchanges,
  listCredentials,
  createCredential,
  deleteCredential,
  testNewCredential,
  testExistingCredential,
  getTelegramSettings,
  saveTelegramSettings,
  deleteTelegramSettings,
  getActiveAccount,
  setActiveAccount,
  type TelegramTestResponse,
  type CredentialTestResponse,
  type ActiveAccount,
} from '../api/client'
import type { SupportedExchange } from '../types'

// 알림 서비스 프로바이더 타입
interface NotificationProvider {
  id: string
  name: string
  icon: string
  description: string
  fields: Array<{
    name: string
    label: string
    type: 'text' | 'password'
    placeholder: string
    helpText?: string
  }>
}

// 지원되는 알림 서비스 목록
const NOTIFICATION_PROVIDERS: NotificationProvider[] = [
  {
    id: 'telegram',
    name: 'Telegram',
    icon: '📱',
    description: '텔레그램 봇을 통한 알림',
    fields: [
      {
        name: 'bot_token',
        label: 'Bot Token',
        type: 'password',
        placeholder: '123456789:ABCdefGHIjklMNOpqrsTUVwxyz',
        helpText: '@BotFather에서 발급받은 Bot Token'
      },
      {
        name: 'chat_id',
        label: 'Chat ID',
        type: 'text',
        placeholder: '-1001234567890',
        helpText: '@userinfobot 또는 @getidsbot에서 확인'
      }
    ]
  },
  // 추후 추가 가능: Slack, Discord, Email 등
]

export function Settings() {
  // Toast 알림
  const toast = useToast()

  // 알림 설정 리소스
  const [notificationSettings, { refetch: refetchNotificationSettings }] = createResource(async () => {
    try {
      return await getNotificationSettings()
    } catch {
      return { telegram_enabled: false, telegram_configured: false }
    }
  })

  // 템플릿 목록 리소스
  const [templates] = createResource(async () => {
    try {
      const response = await getNotificationTemplates()
      return response.templates
    } catch {
      return []
    }
  })

  // ==================== 거래소 자격증명 관리 ====================
  // 지원되는 거래소 목록
  const [exchanges] = createResource(async () => {
    try {
      const response = await getSupportedExchanges()
      return response.exchanges
    } catch {
      return []
    }
  })

  // 등록된 자격증명 목록
  const [credentials, { refetch: refetchCredentials }] = createResource(async () => {
    try {
      const response = await listCredentials()
      return response.credentials
    } catch {
      return []
    }
  })

  // 활성 계정 상태
  const [activeAccount, { refetch: refetchActiveAccount }] = createResource(async () => {
    try {
      return await getActiveAccount()
    } catch {
      return { credential_id: null, exchange_id: null, display_name: null, is_testnet: false }
    }
  })
  const [isSettingActiveAccount, setIsSettingActiveAccount] = createSignal(false)

  // 활성 계정 변경
  const handleSetActiveAccount = async (credentialId: string | null) => {
    setIsSettingActiveAccount(true)
    try {
      const result = await setActiveAccount(credentialId)
      if (result.success) {
        refetchActiveAccount()
      } else {
        toast.error('계정 변경 실패', result.message)
      }
    } catch {
      toast.error('계정 변경 실패', '서버 연결 오류')
    } finally {
      setIsSettingActiveAccount(false)
    }
  }

  // 자격증명 폼 상태
  const [showCredentialForm, setShowCredentialForm] = createSignal(false)
  const [selectedExchange, setSelectedExchange] = createSignal<SupportedExchange | null>(null)
  const [credentialFields, setCredentialFields] = createSignal<Record<string, string>>({})
  const [credentialDisplayName, setCredentialDisplayName] = createSignal('')
  const [isTestnet, setIsTestnet] = createSignal(false)  // 모의투자/테스트넷 여부
  const [isCredentialTesting, setIsCredentialTesting] = createSignal(false)
  const [isCredentialSaving, setIsCredentialSaving] = createSignal(false)
  const [credentialTestResult, setCredentialTestResult] = createSignal<CredentialTestResponse | null>(null)
  const [deletingCredentialId, setDeletingCredentialId] = createSignal<string | null>(null)

  // 거래소 선택 시 필드 초기화
  const handleExchangeSelect = (exchangeId: string) => {
    const exchange = exchanges()?.find(e => e.exchange_id === exchangeId)
    setSelectedExchange(exchange || null)
    setCredentialFields({})
    setCredentialDisplayName(exchange?.display_name || '')
    setIsTestnet(false)  // 모의투자 선택 초기화
    setCredentialTestResult(null)
  }

  // 필드 값 업데이트
  const updateField = (fieldName: string, value: string) => {
    setCredentialFields(prev => ({ ...prev, [fieldName]: value }))
  }

  // 자격증명 테스트
  const handleCredentialTest = async () => {
    const exchange = selectedExchange()
    if (!exchange) return

    setIsCredentialTesting(true)
    setCredentialTestResult(null)

    try {
      const result = await testNewCredential({
        exchange_id: exchange.exchange_id,
        fields: credentialFields()
      })
      setCredentialTestResult(result)
    } catch (err) {
      setCredentialTestResult({
        success: false,
        message: '테스트 실패: 서버 연결 오류'
      })
    } finally {
      setIsCredentialTesting(false)
    }
  }

  // 자격증명 저장
  const handleCredentialSave = async () => {
    const exchange = selectedExchange()
    if (!exchange) return

    setIsCredentialSaving(true)

    try {
      const result = await createCredential({
        exchange_id: exchange.exchange_id,
        display_name: credentialDisplayName() || exchange.display_name,
        fields: credentialFields(),
        is_testnet: isTestnet()  // 모의투자 여부 포함
      })

      if (result.success) {
        setShowCredentialForm(false)
        setSelectedExchange(null)
        setCredentialFields({})
        setCredentialDisplayName('')
        setIsTestnet(false)  // 초기화
        setCredentialTestResult(null)
        refetchCredentials()
      } else {
        setCredentialTestResult({
          success: false,
          message: result.message || '저장 실패'
        })
      }
    } catch (err) {
      setCredentialTestResult({
        success: false,
        message: '저장 실패: 서버 연결 오류'
      })
    } finally {
      setIsCredentialSaving(false)
    }
  }

  // 자격증명 삭제
  const handleCredentialDelete = async (id: string) => {
    if (!confirm('이 API 키를 삭제하시겠습니까?')) return

    setDeletingCredentialId(id)

    try {
      await deleteCredential(id)
      refetchCredentials()
      toast.success('삭제 완료', 'API 키가 삭제되었습니다.')
    } catch (err) {
      toast.error('삭제 실패', '서버 연결 오류')
    } finally {
      setDeletingCredentialId(null)
    }
  }

  // 기존 자격증명 테스트
  const handleExistingCredentialTest = async (id: string) => {
    try {
      const result = await testExistingCredential(id)
      if (result.success) {
        toast.success('연결 테스트 성공', '거래소와 정상적으로 연결되었습니다.')
      } else {
        toast.error('테스트 실패', result.message)
      }
    } catch {
      toast.error('테스트 실패', '서버 연결 오류')
    }
  }

  // ==================== API 키 관리 탭 ====================
  type ApiKeyTab = 'exchange' | 'notification'
  const [activeApiTab, setActiveApiTab] = createSignal<ApiKeyTab>('exchange')

  // ==================== 알림 서비스 관리 ====================
  // 등록된 알림 서비스 목록
  const [notificationServices, { refetch: refetchNotificationServices }] = createResource(async () => {
    try {
      const response = await getTelegramSettings()
      // 텔레그램이 설정되어 있으면 목록에 포함
      if (response.configured) {
        return [{
          id: 'telegram-default',
          provider_id: 'telegram',
          display_name: response.display_name || 'Telegram',
          is_active: true,
          created_at: response.created_at || new Date().toISOString(),
          last_tested_at: response.last_tested_at,
          masked_token: response.masked_token || '****',
          masked_chat_id: response.masked_chat_id || '****',
        }]
      }
      return []
    } catch {
      return []
    }
  })

  // 알림 서비스 추가 폼 상태
  const [showNotificationForm, setShowNotificationForm] = createSignal(false)
  const [selectedProvider, setSelectedProvider] = createSignal<NotificationProvider | null>(null)
  const [notificationFields, setNotificationFields] = createSignal<Record<string, string>>({})
  const [notificationDisplayName, setNotificationDisplayName] = createSignal('')
  const [isNotificationTesting, setIsNotificationTesting] = createSignal(false)
  const [isNotificationSaving, setIsNotificationSaving] = createSignal(false)
  const [notificationTestResult, setNotificationTestResult] = createSignal<TelegramTestResponse | null>(null)
  const [deletingNotificationId, setDeletingNotificationId] = createSignal<string | null>(null)

  // 알림 프로바이더 선택
  const handleProviderSelect = (providerId: string) => {
    const provider = NOTIFICATION_PROVIDERS.find(p => p.id === providerId)
    setSelectedProvider(provider || null)
    setNotificationFields({})
    setNotificationDisplayName(provider?.name || '')
    setNotificationTestResult(null)
  }

  // 알림 필드 값 업데이트
  const updateNotificationField = (fieldName: string, value: string) => {
    setNotificationFields(prev => ({ ...prev, [fieldName]: value }))
  }

  // 알림 서비스 테스트
  const handleNotificationTest = async () => {
    const provider = selectedProvider()
    if (!provider) return

    setIsNotificationTesting(true)
    setNotificationTestResult(null)

    try {
      if (provider.id === 'telegram') {
        const fields = notificationFields()
        const result = await testTelegram({
          bot_token: fields['bot_token'] || '',
          chat_id: fields['chat_id'] || ''
        })
        setNotificationTestResult(result)
      }
    } catch (err) {
      setNotificationTestResult({
        success: false,
        message: '테스트 실패: 서버 연결 오류'
      })
    } finally {
      setIsNotificationTesting(false)
    }
  }

  // 알림 서비스 저장
  const handleNotificationSave = async () => {
    const provider = selectedProvider()
    if (!provider) return

    setIsNotificationSaving(true)

    try {
      if (provider.id === 'telegram') {
        const fields = notificationFields()
        const result = await saveTelegramSettings({
          bot_token: fields['bot_token'] || '',
          chat_id: fields['chat_id'] || '',
          display_name: notificationDisplayName() || 'Telegram'
        })

        if (result.success) {
          setShowNotificationForm(false)
          setSelectedProvider(null)
          setNotificationFields({})
          setNotificationDisplayName('')
          setNotificationTestResult(null)
          refetchNotificationServices()
          refetchNotificationSettings()
        } else {
          setNotificationTestResult({
            success: false,
            message: result.message || '저장 실패'
          })
        }
      }
    } catch (err) {
      setNotificationTestResult({
        success: false,
        message: '저장 실패: 서버 연결 오류'
      })
    } finally {
      setIsNotificationSaving(false)
    }
  }

  // 알림 서비스 삭제
  const handleNotificationDelete = async (id: string) => {
    if (!confirm('이 알림 서비스를 삭제하시겠습니까?')) return

    setDeletingNotificationId(id)

    try {
      await deleteTelegramSettings()
      refetchNotificationServices()
      refetchNotificationSettings()
      toast.success('삭제 완료', '알림 서비스가 삭제되었습니다.')
    } catch (err) {
      toast.error('삭제 실패', '서버 연결 오류')
    } finally {
      setDeletingNotificationId(null)
    }
  }

  // 기존 알림 서비스 테스트
  const handleExistingNotificationTest = async (id: string) => {
    try {
      const result = await testTelegramEnv()
      if (result.success) {
        toast.success('연결 테스트 성공', '텔레그램과 정상적으로 연결되었습니다.')
      } else {
        toast.error('테스트 실패', result.message)
      }
    } catch {
      toast.error('테스트 실패', '서버 연결 오류')
    }
  }

  // ==================== 기타 설정 ====================
  const [isDarkMode, setIsDarkMode] = createSignal(true)
  const [notifications, setNotifications] = createSignal({
    tradeExecution: true,
    priceAlerts: true,
    dailyReport: false,
    errorAlerts: true,
  })
  const [riskSettings, setRiskSettings] = createSignal({
    maxDailyLoss: '3',
    maxPositionSize: '10',
    stopLossDefault: '2',
    takeProfitDefault: '5',
  })
  const [telegramSettings, setTelegramSettings] = createSignal({
    botToken: '',
    chatId: '',
    isConnected: false,
  })
  const [isTelegramTesting, setIsTelegramTesting] = createSignal(false)
  const [telegramTestResult, setTelegramTestResult] = createSignal<TelegramTestResponse | null>(null)
  const [selectedTemplate, setSelectedTemplate] = createSignal<string>('')
  const [isTemplateTesting, setIsTemplateTesting] = createSignal(false)
  const [isSaving, setIsSaving] = createSignal(false)

  // 서버에 저장된 텔레그램 설정이 있으면 연결 상태 업데이트
  createEffect(() => {
    const settings = notificationSettings()
    if (settings?.telegram_configured) {
      setTelegramSettings(prev => ({ ...prev, isConnected: true }))
    }
  })

  // 텔레그램 연결 테스트 (직접 입력한 토큰으로)
  const handleTelegramTest = async () => {
    const { botToken, chatId } = telegramSettings()

    if (!botToken || !chatId) {
      setTelegramTestResult({
        success: false,
        message: 'Bot Token과 Chat ID를 모두 입력해주세요.'
      })
      return
    }

    setIsTelegramTesting(true)
    setTelegramTestResult(null)

    try {
      const result = await testTelegram({ bot_token: botToken, chat_id: chatId })
      setTelegramTestResult(result)
      setTelegramSettings(prev => ({ ...prev, isConnected: result.success }))
    } catch (err) {
      setTelegramTestResult({
        success: false,
        message: '서버 연결에 실패했습니다. 나중에 다시 시도해주세요.'
      })
    } finally {
      setIsTelegramTesting(false)
    }
  }

  // 환경변수로 설정된 텔레그램 테스트
  const handleTelegramEnvTest = async () => {
    setIsTelegramTesting(true)
    setTelegramTestResult(null)

    try {
      const result = await testTelegramEnv()
      setTelegramTestResult(result)
      if (result.success) {
        setTelegramSettings(prev => ({ ...prev, isConnected: true }))
        refetchNotificationSettings()
      }
    } catch (err) {
      setTelegramTestResult({
        success: false,
        message: '서버 연결에 실패했습니다.'
      })
    } finally {
      setIsTelegramTesting(false)
    }
  }

  // 템플릿 테스트 전송
  const handleTemplateTest = async () => {
    const templateType = selectedTemplate()
    if (!templateType) return

    setIsTemplateTesting(true)
    setTelegramTestResult(null)

    try {
      const result = await testTelegramTemplate({ template_type: templateType })
      setTelegramTestResult(result)
    } catch (err) {
      setTelegramTestResult({
        success: false,
        message: '템플릿 테스트 전송에 실패했습니다.'
      })
    } finally {
      setIsTemplateTesting(false)
    }
  }

  // 모든 템플릿 테스트
  const handleAllTemplatesTest = async () => {
    setIsTemplateTesting(true)
    setTelegramTestResult(null)

    try {
      const result = await testAllTelegramTemplates()
      setTelegramTestResult(result)
    } catch (err) {
      setTelegramTestResult({
        success: false,
        message: '템플릿 테스트에 실패했습니다.'
      })
    } finally {
      setIsTemplateTesting(false)
    }
  }

  const handleSave = () => {
    setIsSaving(true)
    // TODO: 백엔드 설정 저장 API 구현 시 연동
    setTimeout(() => {
      setIsSaving(false)
      // 로컬 스토리지에 설정 저장 (임시)
      localStorage.setItem('trader_settings', JSON.stringify({
        notifications: notifications(),
        riskSettings: riskSettings(),
        isDarkMode: isDarkMode(),
      }))
    }, 500)
  }

  return (
    <div class="space-y-6 max-w-4xl">
      {/* API 키 관리 (통합 섹션) */}
      <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-6">
        <div class="flex items-center justify-between mb-6">
          <h3 class="text-lg font-semibold text-[var(--color-text)] flex items-center gap-2">
            <Key class="w-5 h-5" />
            API 키 관리
          </h3>
        </div>

        {/* 탭 네비게이션 */}
        <div class="flex gap-2 mb-6 border-b border-[var(--color-surface-light)]">
          <button
            onClick={() => setActiveApiTab('exchange')}
            class={`flex items-center gap-2 px-4 py-3 text-sm font-medium transition-colors border-b-2 -mb-px ${
              activeApiTab() === 'exchange'
                ? 'text-[var(--color-primary)] border-[var(--color-primary)]'
                : 'text-[var(--color-text-muted)] border-transparent hover:text-[var(--color-text)]'
            }`}
          >
            <Building2 class="w-4 h-4" />
            거래소
          </button>
          <button
            onClick={() => setActiveApiTab('notification')}
            class={`flex items-center gap-2 px-4 py-3 text-sm font-medium transition-colors border-b-2 -mb-px ${
              activeApiTab() === 'notification'
                ? 'text-[var(--color-primary)] border-[var(--color-primary)]'
                : 'text-[var(--color-text-muted)] border-transparent hover:text-[var(--color-text)]'
            }`}
          >
            <BellRing class="w-4 h-4" />
            알림 서비스
            <Show when={(notificationServices() || []).length > 0}>
              <span class="w-2 h-2 rounded-full bg-green-500" />
            </Show>
          </button>
        </div>

        {/* 거래소 API 탭 내용 */}
        <Show when={activeApiTab() === 'exchange'}>
          <div class="flex items-center justify-between mb-4">
            <p class="text-sm text-[var(--color-text-muted)]">
              거래소 API 키를 등록하여 자동 매매를 활성화하세요.
            </p>
            <button
              onClick={() => setShowCredentialForm(true)}
              class="px-4 py-2 bg-[var(--color-primary)] text-white rounded-lg text-sm font-medium hover:bg-[var(--color-primary)]/90 transition-colors flex items-center gap-2"
            >
              <Plus class="w-4 h-4" />
              API 키 추가
            </button>
          </div>

          {/* 활성 계정 선택 */}
          <Show when={(credentials() || []).length > 0}>
            <div class="mb-6 p-4 bg-[var(--color-surface-light)] rounded-lg border border-[var(--color-primary)]/30">
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-3">
                  <div class="w-10 h-10 rounded-full bg-[var(--color-primary)]/20 flex items-center justify-center">
                    <Building2 class="w-5 h-5 text-[var(--color-primary)]" />
                  </div>
                  <div>
                    <div class="text-sm font-medium text-[var(--color-text)]">활성 계정</div>
                    <div class="text-xs text-[var(--color-text-muted)]">
                      대시보드에 표시될 자산 정보의 계정을 선택합니다
                    </div>
                  </div>
                </div>
                <div class="flex items-center gap-2">
                  <select
                    value={activeAccount()?.credential_id || ''}
                    onChange={(e) => handleSetActiveAccount(e.currentTarget.value || null)}
                    disabled={isSettingActiveAccount()}
                    class="px-4 py-2 rounded-lg bg-[var(--color-surface)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)] min-w-[200px] disabled:opacity-50"
                  >
                    <option value="">계정 선택 안함</option>
                    <For each={credentials()}>
                      {(cred) => (
                        <option value={cred.id}>
                          {cred.display_name} ({cred.exchange_id}){cred.is_testnet ? ' [모의투자]' : ''}
                        </option>
                      )}
                    </For>
                  </select>
                  <Show when={isSettingActiveAccount()}>
                    <div class="w-5 h-5 border-2 border-[var(--color-primary)] border-t-transparent rounded-full animate-spin" />
                  </Show>
                </div>
              </div>

              {/* 현재 선택된 활성 계정 정보 표시 */}
              <Show when={activeAccount()?.credential_id}>
                <div class="mt-3 pt-3 border-t border-[var(--color-surface)]">
                  <div class="flex items-center gap-2">
                    <div class="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
                    <span class="text-sm text-[var(--color-text)]">
                      {activeAccount()?.display_name}
                    </span>
                    <Show when={activeAccount()?.is_testnet}>
                      <span class="px-2 py-0.5 text-xs rounded bg-yellow-500/20 text-yellow-500">
                        모의투자
                      </span>
                    </Show>
                    <span class="text-xs text-[var(--color-text-muted)]">
                      ({activeAccount()?.exchange_id})
                    </span>
                  </div>
                </div>
              </Show>
            </div>
          </Show>

        {/* 등록된 자격증명 목록 */}
        <Show
          when={(credentials() || []).length > 0}
          fallback={
            <div class="text-center py-8 text-[var(--color-text-muted)]">
              <Key class="w-12 h-12 mx-auto mb-3 opacity-30" />
              <p>등록된 API 키가 없습니다.</p>
              <p class="text-sm mt-2">거래소 API 키를 추가하여 자동 매매를 활성화하세요.</p>
            </div>
          }
        >
          <div class="space-y-3 mb-6">
            <For each={credentials()}>
              {(cred) => (
                <div class="flex items-center justify-between p-4 bg-[var(--color-surface-light)] rounded-lg">
                  <div class="flex items-center gap-4">
                    <div
                      class={`w-3 h-3 rounded-full ${
                        cred.is_active ? 'bg-green-500' : 'bg-gray-500'
                      }`}
                    />
                    <div>
                      <div class="flex items-center gap-2">
                        <span class="font-medium text-[var(--color-text)]">{cred.display_name}</span>
                        <Show when={cred.is_testnet}>
                          <span class="px-2 py-0.5 text-xs rounded bg-yellow-500/20 text-yellow-500">
                            모의투자
                          </span>
                        </Show>
                      </div>
                      <div class="text-sm text-[var(--color-text-muted)]">
                        {cred.exchange_id}
                        <Show when={cred.masked_api_key}>
                          {' '}· API: {cred.masked_api_key}
                        </Show>
                      </div>
                      <div class="text-xs text-[var(--color-text-muted)]">
                        등록: {new Date(cred.created_at).toLocaleDateString()}
                        {cred.last_tested_at && ` · 마지막 테스트: ${new Date(cred.last_tested_at).toLocaleDateString()}`}
                      </div>
                    </div>
                  </div>
                  <div class="flex gap-2">
                    <button
                      onClick={() => handleExistingCredentialTest(cred.id)}
                      class="px-3 py-1 text-sm text-blue-500 hover:text-blue-400 transition-colors flex items-center gap-1"
                    >
                      <TestTube class="w-4 h-4" />
                      테스트
                    </button>
                    <button
                      onClick={() => handleCredentialDelete(cred.id)}
                      disabled={deletingCredentialId() === cred.id}
                      class="px-3 py-1 text-sm text-red-500 hover:text-red-400 transition-colors flex items-center gap-1 disabled:opacity-50"
                    >
                      <Show when={deletingCredentialId() === cred.id} fallback={<Trash2 class="w-4 h-4" />}>
                        <div class="w-4 h-4 border-2 border-red-500 border-t-transparent rounded-full animate-spin" />
                      </Show>
                      삭제
                    </button>
                  </div>
                </div>
              )}
            </For>
          </div>
        </Show>

        {/* 새 자격증명 추가 폼 */}
        <Show when={showCredentialForm()}>
          <div class="border-t border-[var(--color-surface-light)] pt-6 mt-4">
            <h4 class="text-sm font-semibold text-[var(--color-text)] mb-4">새 API 키 등록</h4>

            {/* 거래소 선택 */}
            <div class="mb-4">
              <label class="block text-sm text-[var(--color-text-muted)] mb-1">거래소 선택</label>
              <select
                onChange={(e) => handleExchangeSelect(e.currentTarget.value)}
                class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
              >
                <option value="">거래소를 선택하세요...</option>
                <For each={exchanges()}>
                  {(exchange) => (
                    <option value={exchange.exchange_id}>{exchange.display_name}</option>
                  )}
                </For>
              </select>
            </div>

            {/* 선택된 거래소의 필드들 */}
            <Show when={selectedExchange()}>
              <div class="space-y-4">
                {/* 표시 이름 */}
                <div>
                  <label class="block text-sm text-[var(--color-text-muted)] mb-1">표시 이름</label>
                  <input
                    type="text"
                    value={credentialDisplayName()}
                    onInput={(e) => setCredentialDisplayName(e.currentTarget.value)}
                    class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
                    placeholder="예: 메인 계정, 테스트 계정"
                  />
                </div>

                {/* 필수 필드 */}
                <For each={selectedExchange()!.required_fields}>
                  {(field) => (
                    <div>
                      <label class="block text-sm text-[var(--color-text-muted)] mb-1">
                        {field.label} <span class="text-red-500">*</span>
                      </label>
                      <input
                        type={field.field_type === 'password' ? 'password' : 'text'}
                        value={credentialFields()[field.name] || ''}
                        onInput={(e) => updateField(field.name, e.currentTarget.value)}
                        class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
                        placeholder={field.placeholder || ''}
                      />
                    </div>
                  )}
                </For>

                {/* 선택 필드 */}
                <Show when={selectedExchange()!.optional_fields.length > 0}>
                  <div class="pt-2 border-t border-[var(--color-surface-light)]">
                    <p class="text-xs text-[var(--color-text-muted)] mb-3">선택 항목</p>
                    <For each={selectedExchange()!.optional_fields}>
                      {(field) => (
                        <div class="mb-3">
                          <label class="block text-sm text-[var(--color-text-muted)] mb-1">{field.label}</label>
                          <input
                            type={field.field_type === 'password' ? 'password' : 'text'}
                            value={credentialFields()[field.name] || ''}
                            onInput={(e) => updateField(field.name, e.currentTarget.value)}
                            class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
                            placeholder={field.placeholder || ''}
                          />
                        </div>
                      )}
                    </For>
                  </div>
                </Show>

                {/* 모의투자/테스트넷 체크박스 */}
                <Show when={selectedExchange()?.supports_testnet}>
                  <div class="pt-3 border-t border-[var(--color-surface-light)]">
                    <label class="flex items-center gap-3 cursor-pointer">
                      <input
                        type="checkbox"
                        checked={isTestnet()}
                        onChange={(e) => setIsTestnet(e.currentTarget.checked)}
                        class="w-5 h-5 rounded border-[var(--color-surface-light)] bg-[var(--color-surface-light)] text-[var(--color-primary)] focus:ring-[var(--color-primary)] cursor-pointer"
                      />
                      <div>
                        <div class="text-[var(--color-text)] font-medium">
                          {selectedExchange()?.market_type === 'crypto' ? '테스트넷 API' : '모의투자 계좌'}
                        </div>
                        <div class="text-sm text-[var(--color-text-muted)]">
                          {selectedExchange()?.market_type === 'crypto'
                            ? '실제 자산을 사용하지 않는 테스트 환경입니다.'
                            : '모의투자 계좌의 API 키입니다. 실제 주문이 체결되지 않습니다.'
                          }
                        </div>
                      </div>
                    </label>
                  </div>
                </Show>

                {/* 테스트 결과 */}
                <Show when={credentialTestResult()}>
                  <div
                    class={`p-3 rounded-lg flex items-center gap-2 ${
                      credentialTestResult()!.success
                        ? 'bg-green-500/20 text-green-400'
                        : 'bg-red-500/20 text-red-400'
                    }`}
                  >
                    <Show when={credentialTestResult()!.success} fallback={<XCircle class="w-5 h-5" />}>
                      <CheckCircle class="w-5 h-5" />
                    </Show>
                    <span>{credentialTestResult()!.message}</span>
                  </div>
                </Show>

                {/* 버튼들 */}
                <div class="flex gap-3 pt-2">
                  <button
                    onClick={handleCredentialTest}
                    disabled={isCredentialTesting()}
                    class="flex-1 px-4 py-2 bg-[var(--color-surface-light)] text-[var(--color-text)] rounded-lg font-medium hover:bg-[var(--color-surface-light)]/80 transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
                  >
                    <Show when={isCredentialTesting()} fallback={<TestTube class="w-4 h-4" />}>
                      <div class="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin" />
                    </Show>
                    연결 테스트
                  </button>
                  <button
                    onClick={handleCredentialSave}
                    disabled={isCredentialSaving() || !credentialTestResult()?.success}
                    class="flex-1 px-4 py-2 bg-[var(--color-primary)] text-white rounded-lg font-medium hover:bg-[var(--color-primary)]/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
                  >
                    <Show when={isCredentialSaving()} fallback={<Save class="w-4 h-4" />}>
                      <div class="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
                    </Show>
                    저장
                  </button>
                  <button
                    onClick={() => {
                      setShowCredentialForm(false)
                      setSelectedExchange(null)
                      setCredentialFields({})
                      setIsTestnet(false)
                      setCredentialTestResult(null)
                    }}
                    class="px-4 py-2 text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors"
                  >
                    취소
                  </button>
                </div>
              </div>
            </Show>
          </div>
        </Show>
        </Show>

        {/* 알림 서비스 탭 내용 */}
        <Show when={activeApiTab() === 'notification'}>
          <div class="flex items-center justify-between mb-4">
            <p class="text-sm text-[var(--color-text-muted)]">
              알림 서비스를 등록하여 거래 알림을 받으세요.
            </p>
            <button
              onClick={() => setShowNotificationForm(true)}
              class="px-4 py-2 bg-[var(--color-primary)] text-white rounded-lg text-sm font-medium hover:bg-[var(--color-primary)]/90 transition-colors flex items-center gap-2"
            >
              <Plus class="w-4 h-4" />
              알림 서비스 추가
            </button>
          </div>

          {/* 등록된 알림 서비스 목록 */}
          <Show
            when={(notificationServices() || []).length > 0}
            fallback={
              <div class="text-center py-8 text-[var(--color-text-muted)]">
                <BellRing class="w-12 h-12 mx-auto mb-3 opacity-30" />
                <p>등록된 알림 서비스가 없습니다.</p>
                <p class="text-sm mt-2">알림 서비스를 추가하여 거래 알림을 받으세요.</p>
              </div>
            }
          >
            <div class="space-y-3 mb-6">
              <For each={notificationServices()}>
                {(service) => (
                  <div class="flex items-center justify-between p-4 bg-[var(--color-surface-light)] rounded-lg">
                    <div class="flex items-center gap-4">
                      <div
                        class={`w-3 h-3 rounded-full ${
                          service.is_active ? 'bg-green-500' : 'bg-gray-500'
                        }`}
                      />
                      <div class="flex items-center gap-3">
                        <span class="text-2xl">📱</span>
                        <div>
                          <div class="font-medium text-[var(--color-text)]">{service.display_name}</div>
                          <div class="text-sm text-[var(--color-text-muted)]">
                            Token: {service.masked_token} · Chat ID: {service.masked_chat_id}
                          </div>
                          <div class="text-xs text-[var(--color-text-muted)]">
                            등록: {new Date(service.created_at).toLocaleDateString()}
                            {service.last_tested_at && ` · 마지막 테스트: ${new Date(service.last_tested_at).toLocaleDateString()}`}
                          </div>
                        </div>
                      </div>
                    </div>
                    <div class="flex gap-2">
                      <button
                        onClick={() => handleExistingNotificationTest(service.id)}
                        class="px-3 py-1 text-sm text-blue-500 hover:text-blue-400 transition-colors flex items-center gap-1"
                      >
                        <TestTube class="w-4 h-4" />
                        테스트
                      </button>
                      <button
                        onClick={() => handleNotificationDelete(service.id)}
                        disabled={deletingNotificationId() === service.id}
                        class="px-3 py-1 text-sm text-red-500 hover:text-red-400 transition-colors flex items-center gap-1 disabled:opacity-50"
                      >
                        <Show when={deletingNotificationId() === service.id} fallback={<Trash2 class="w-4 h-4" />}>
                          <div class="w-4 h-4 border-2 border-red-500 border-t-transparent rounded-full animate-spin" />
                        </Show>
                        삭제
                      </button>
                    </div>
                  </div>
                )}
              </For>
            </div>

            {/* 템플릿 테스트 섹션 */}
            <div class="pt-4 border-t border-[var(--color-surface-light)]">
              <h4 class="text-sm font-semibold text-[var(--color-text)] mb-3">
                알림 템플릿 테스트
              </h4>

              <div class="flex gap-3 mb-3">
                <select
                  value={selectedTemplate()}
                  onChange={(e) => setSelectedTemplate(e.currentTarget.value)}
                  class="flex-1 px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
                >
                  <option value="">템플릿 선택...</option>
                  <For each={templates()}>
                    {(template) => (
                      <option value={template.id}>
                        {template.name} ({template.priority})
                      </option>
                    )}
                  </For>
                </select>

                <button
                  onClick={handleTemplateTest}
                  disabled={isTemplateTesting() || !selectedTemplate()}
                  class="px-4 py-2 bg-[var(--color-primary)] text-white rounded-lg font-medium hover:bg-[var(--color-primary)]/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
                >
                  <Show when={isTemplateTesting()} fallback={<Send class="w-4 h-4" />}>
                    <div class="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
                  </Show>
                  전송
                </button>
              </div>

              <button
                onClick={handleAllTemplatesTest}
                disabled={isTemplateTesting()}
                class="w-full px-4 py-2 bg-[var(--color-surface-light)] text-[var(--color-text)] rounded-lg font-medium hover:bg-[var(--color-surface-light)]/80 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
              >
                <Show when={isTemplateTesting()} fallback={<Play class="w-4 h-4" />}>
                  <div class="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin" />
                </Show>
                모든 템플릿 테스트 전송
              </button>

              <Show when={templates()?.length}>
                <div class="mt-4 space-y-2">
                  <p class="text-xs text-[var(--color-text-muted)]">사용 가능한 템플릿:</p>
                  <div class="grid grid-cols-2 gap-2">
                    <For each={templates()}>
                      {(template) => (
                        <div class="text-xs p-2 rounded bg-[var(--color-surface-light)]">
                          <div class="font-medium text-[var(--color-text)]">{template.name}</div>
                          <div class="text-[var(--color-text-muted)]">{template.description}</div>
                        </div>
                      )}
                    </For>
                  </div>
                </div>
              </Show>
            </div>
          </Show>

          {/* 새 알림 서비스 추가 폼 */}
          <Show when={showNotificationForm()}>
            <div class="border-t border-[var(--color-surface-light)] pt-6 mt-4">
              <h4 class="text-sm font-semibold text-[var(--color-text)] mb-4">새 알림 서비스 등록</h4>

              {/* 프로바이더 선택 */}
              <div class="mb-4">
                <label class="block text-sm text-[var(--color-text-muted)] mb-1">알림 서비스 선택</label>
                <select
                  onChange={(e) => handleProviderSelect(e.currentTarget.value)}
                  class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
                >
                  <option value="">알림 서비스를 선택하세요...</option>
                  <For each={NOTIFICATION_PROVIDERS}>
                    {(provider) => (
                      <option value={provider.id}>{provider.icon} {provider.name}</option>
                    )}
                  </For>
                </select>
              </div>

              {/* 선택된 프로바이더의 필드들 */}
              <Show when={selectedProvider()}>
                <div class="space-y-4">
                  <p class="text-sm text-[var(--color-text-muted)]">
                    {selectedProvider()!.description}
                  </p>

                  {/* 표시 이름 */}
                  <div>
                    <label class="block text-sm text-[var(--color-text-muted)] mb-1">표시 이름</label>
                    <input
                      type="text"
                      value={notificationDisplayName()}
                      onInput={(e) => setNotificationDisplayName(e.currentTarget.value)}
                      class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
                      placeholder="예: 메인 알림, 긴급 알림"
                    />
                  </div>

                  {/* 동적 필드 */}
                  <For each={selectedProvider()!.fields}>
                    {(field) => (
                      <div>
                        <label class="block text-sm text-[var(--color-text-muted)] mb-1">
                          {field.label} <span class="text-red-500">*</span>
                        </label>
                        <input
                          type={field.type}
                          value={notificationFields()[field.name] || ''}
                          onInput={(e) => updateNotificationField(field.name, e.currentTarget.value)}
                          class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
                          placeholder={field.placeholder}
                        />
                        <Show when={field.helpText}>
                          <p class="text-xs text-[var(--color-text-muted)] mt-1">{field.helpText}</p>
                        </Show>
                      </div>
                    )}
                  </For>

                  {/* 테스트 결과 */}
                  <Show when={notificationTestResult()}>
                    <div
                      class={`p-3 rounded-lg flex items-center gap-2 ${
                        notificationTestResult()!.success
                          ? 'bg-green-500/20 text-green-400'
                          : 'bg-red-500/20 text-red-400'
                      }`}
                    >
                      <Show when={notificationTestResult()!.success} fallback={<XCircle class="w-5 h-5" />}>
                        <CheckCircle class="w-5 h-5" />
                      </Show>
                      <span>{notificationTestResult()!.message}</span>
                    </div>
                  </Show>

                  {/* 버튼들 */}
                  <div class="flex gap-3 pt-2">
                    <button
                      onClick={handleNotificationTest}
                      disabled={isNotificationTesting()}
                      class="flex-1 px-4 py-2 bg-[var(--color-surface-light)] text-[var(--color-text)] rounded-lg font-medium hover:bg-[var(--color-surface-light)]/80 transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
                    >
                      <Show when={isNotificationTesting()} fallback={<TestTube class="w-4 h-4" />}>
                        <div class="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin" />
                      </Show>
                      연결 테스트
                    </button>
                    <button
                      onClick={handleNotificationSave}
                      disabled={isNotificationSaving() || !notificationTestResult()?.success}
                      class="flex-1 px-4 py-2 bg-[var(--color-primary)] text-white rounded-lg font-medium hover:bg-[var(--color-primary)]/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
                    >
                      <Show when={isNotificationSaving()} fallback={<Save class="w-4 h-4" />}>
                        <div class="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
                      </Show>
                      저장
                    </button>
                    <button
                      onClick={() => {
                        setShowNotificationForm(false)
                        setSelectedProvider(null)
                        setNotificationFields({})
                        setNotificationDisplayName('')
                        setNotificationTestResult(null)
                      }}
                      class="px-4 py-2 text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors"
                    >
                      취소
                    </button>
                  </div>
                </div>
              </Show>
            </div>
          </Show>
        </Show>
      </div>

      {/* Risk Management */}
      <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-6">
        <h3 class="text-lg font-semibold text-[var(--color-text)] mb-4 flex items-center gap-2">
          <Shield class="w-5 h-5" />
          리스크 관리
        </h3>

        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div>
            <label class="block text-sm text-[var(--color-text-muted)] mb-1">
              일일 최대 손실 (%)
            </label>
            <input
              type="number"
              value={riskSettings().maxDailyLoss}
              onInput={(e) =>
                setRiskSettings((prev) => ({
                  ...prev,
                  maxDailyLoss: e.currentTarget.value,
                }))
              }
              class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
            />
          </div>

          <div>
            <label class="block text-sm text-[var(--color-text-muted)] mb-1">
              최대 포지션 크기 (%)
            </label>
            <input
              type="number"
              value={riskSettings().maxPositionSize}
              onInput={(e) =>
                setRiskSettings((prev) => ({
                  ...prev,
                  maxPositionSize: e.currentTarget.value,
                }))
              }
              class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
            />
          </div>

          <div>
            <label class="block text-sm text-[var(--color-text-muted)] mb-1">
              기본 손절가 (%)
            </label>
            <input
              type="number"
              value={riskSettings().stopLossDefault}
              onInput={(e) =>
                setRiskSettings((prev) => ({
                  ...prev,
                  stopLossDefault: e.currentTarget.value,
                }))
              }
              class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
            />
          </div>

          <div>
            <label class="block text-sm text-[var(--color-text-muted)] mb-1">
              기본 익절가 (%)
            </label>
            <input
              type="number"
              value={riskSettings().takeProfitDefault}
              onInput={(e) =>
                setRiskSettings((prev) => ({
                  ...prev,
                  takeProfitDefault: e.currentTarget.value,
                }))
              }
              class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
            />
          </div>
        </div>
      </div>

      {/* Notifications */}
      <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-6">
        <h3 class="text-lg font-semibold text-[var(--color-text)] mb-4 flex items-center gap-2">
          <Bell class="w-5 h-5" />
          알림 설정
        </h3>

        <div class="space-y-4">
          <label class="flex items-center justify-between">
            <div>
              <div class="text-[var(--color-text)]">거래 실행 알림</div>
              <div class="text-sm text-[var(--color-text-muted)]">
                주문이 체결될 때 알림
              </div>
            </div>
            <input
              type="checkbox"
              checked={notifications().tradeExecution}
              onChange={(e) =>
                setNotifications((prev) => ({
                  ...prev,
                  tradeExecution: e.currentTarget.checked,
                }))
              }
              class="w-5 h-5 rounded border-[var(--color-surface-light)] bg-[var(--color-surface-light)] text-[var(--color-primary)] focus:ring-[var(--color-primary)]"
            />
          </label>

          <label class="flex items-center justify-between">
            <div>
              <div class="text-[var(--color-text)]">가격 알림</div>
              <div class="text-sm text-[var(--color-text-muted)]">
                설정한 가격에 도달할 때 알림
              </div>
            </div>
            <input
              type="checkbox"
              checked={notifications().priceAlerts}
              onChange={(e) =>
                setNotifications((prev) => ({
                  ...prev,
                  priceAlerts: e.currentTarget.checked,
                }))
              }
              class="w-5 h-5 rounded border-[var(--color-surface-light)] bg-[var(--color-surface-light)] text-[var(--color-primary)] focus:ring-[var(--color-primary)]"
            />
          </label>

          <label class="flex items-center justify-between">
            <div>
              <div class="text-[var(--color-text)]">일일 리포트</div>
              <div class="text-sm text-[var(--color-text-muted)]">
                매일 거래 요약 리포트
              </div>
            </div>
            <input
              type="checkbox"
              checked={notifications().dailyReport}
              onChange={(e) =>
                setNotifications((prev) => ({
                  ...prev,
                  dailyReport: e.currentTarget.checked,
                }))
              }
              class="w-5 h-5 rounded border-[var(--color-surface-light)] bg-[var(--color-surface-light)] text-[var(--color-primary)] focus:ring-[var(--color-primary)]"
            />
          </label>

          <label class="flex items-center justify-between">
            <div>
              <div class="text-[var(--color-text)]">오류 알림</div>
              <div class="text-sm text-[var(--color-text-muted)]">
                시스템 오류 발생 시 알림
              </div>
            </div>
            <input
              type="checkbox"
              checked={notifications().errorAlerts}
              onChange={(e) =>
                setNotifications((prev) => ({
                  ...prev,
                  errorAlerts: e.currentTarget.checked,
                }))
              }
              class="w-5 h-5 rounded border-[var(--color-surface-light)] bg-[var(--color-surface-light)] text-[var(--color-primary)] focus:ring-[var(--color-primary)]"
            />
          </label>
        </div>
      </div>

      {/* Appearance */}
      <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-6">
        <h3 class="text-lg font-semibold text-[var(--color-text)] mb-4 flex items-center gap-2">
          <Globe class="w-5 h-5" />
          외관 설정
        </h3>

        <div class="flex items-center justify-between">
          <div>
            <div class="text-[var(--color-text)]">다크 모드</div>
            <div class="text-sm text-[var(--color-text-muted)]">
              어두운 테마 사용
            </div>
          </div>
          <button
            class={`relative w-14 h-8 rounded-full transition-colors ${
              isDarkMode() ? 'bg-[var(--color-primary)]' : 'bg-[var(--color-surface-light)]'
            }`}
            onClick={() => setIsDarkMode(!isDarkMode())}
          >
            <div
              class={`absolute top-1 w-6 h-6 rounded-full bg-white flex items-center justify-center transition-transform ${
                isDarkMode() ? 'translate-x-7' : 'translate-x-1'
              }`}
            >
              <Show when={isDarkMode()} fallback={<Sun class="w-4 h-4 text-yellow-500" />}>
                <Moon class="w-4 h-4 text-gray-700" />
              </Show>
            </div>
          </button>
        </div>
      </div>

      {/* Database */}
      <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-6">
        <h3 class="text-lg font-semibold text-[var(--color-text)] mb-4 flex items-center gap-2">
          <Database class="w-5 h-5" />
          데이터 관리
        </h3>

        <div class="flex flex-wrap gap-3">
          <button class="px-4 py-2 bg-[var(--color-surface-light)] text-[var(--color-text)] rounded-lg text-sm font-medium hover:bg-[var(--color-surface-light)]/80 transition-colors">
            데이터 내보내기
          </button>
          <button class="px-4 py-2 bg-[var(--color-surface-light)] text-[var(--color-text)] rounded-lg text-sm font-medium hover:bg-[var(--color-surface-light)]/80 transition-colors">
            거래 내역 다운로드
          </button>
          <button class="px-4 py-2 bg-red-500/20 text-red-500 rounded-lg text-sm font-medium hover:bg-red-500/30 transition-colors">
            캐시 초기화
          </button>
        </div>
      </div>

      {/* Save Button */}
      <div class="flex justify-end">
        <button
          onClick={handleSave}
          disabled={isSaving()}
          class="px-6 py-3 bg-[var(--color-primary)] text-white rounded-lg font-medium hover:bg-[var(--color-primary)]/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
        >
          <Show
            when={isSaving()}
            fallback={<Save class="w-5 h-5" />}
          >
            <div class="w-5 h-5 border-2 border-white border-t-transparent rounded-full animate-spin" />
          </Show>
          {isSaving() ? '저장 중...' : '설정 저장'}
        </button>
      </div>
    </div>
  )
}
