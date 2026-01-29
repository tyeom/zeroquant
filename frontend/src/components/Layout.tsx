import { type JSX, For } from 'solid-js'
import { A, useLocation } from '@solidjs/router'
import {
  LayoutDashboard,
  Bot,
  Settings,
  Bell,
  LogOut,
  Activity,
  FlaskConical,
  Play,
  Brain,
} from 'lucide-solid'

interface LayoutProps {
  children?: JSX.Element
}

const navItems = [
  { path: '/', icon: LayoutDashboard, label: '대시보드' },
  { path: '/strategies', icon: Bot, label: '전략' },
  { path: '/backtest', icon: FlaskConical, label: '백테스트' },
  { path: '/simulation', icon: Play, label: '시뮬레이션' },
  { path: '/ml-training', icon: Brain, label: 'ML 훈련' },
  { path: '/settings', icon: Settings, label: '설정' },
]

export function Layout(props: LayoutProps) {
  const location = useLocation()

  return (
    <div class="flex h-screen bg-[var(--color-background)]">
      {/* Sidebar */}
      <aside class="w-64 bg-[var(--color-surface)] border-r border-[var(--color-surface-light)] flex flex-col">
        {/* Logo */}
        <div class="p-6 border-b border-[var(--color-surface-light)]">
          <div class="flex items-center gap-3">
            <div class="w-10 h-10 rounded-lg bg-[var(--color-primary)] flex items-center justify-center">
              <Activity class="w-6 h-6 text-white" />
            </div>
            <div>
              <h1 class="text-lg font-bold text-[var(--color-text)]">Trader Bot</h1>
              <p class="text-xs text-[var(--color-text-muted)]">Multi-Market Trading</p>
            </div>
          </div>
        </div>

        {/* Navigation */}
        <nav class="flex-1 p-4">
          <ul class="space-y-2">
            <For each={navItems}>
              {(item) => {
                const Icon = item.icon
                return (
                  <li>
                    <A
                      href={item.path}
                      class={`flex items-center gap-3 px-4 py-3 rounded-lg transition-colors ${
                        location.pathname === item.path
                          ? 'bg-[var(--color-primary)] text-white'
                          : 'text-[var(--color-text-muted)] hover:bg-[var(--color-surface-light)] hover:text-[var(--color-text)]'
                      }`}
                    >
                      <Icon class="w-5 h-5" />
                      <span class="font-medium">{item.label}</span>
                    </A>
                  </li>
                )
              }}
            </For>
          </ul>
        </nav>

        {/* Bottom section */}
        <div class="p-4 border-t border-[var(--color-surface-light)]">
          <button class="flex items-center gap-3 px-4 py-3 w-full rounded-lg text-[var(--color-text-muted)] hover:bg-[var(--color-surface-light)] hover:text-[var(--color-text)] transition-colors">
            <LogOut class="w-5 h-5" />
            <span class="font-medium">로그아웃</span>
          </button>
        </div>
      </aside>

      {/* Main content */}
      <div class="flex-1 flex flex-col overflow-hidden">
        {/* Header */}
        <header class="h-16 bg-[var(--color-surface)] border-b border-[var(--color-surface-light)] flex items-center justify-between px-6">
          <div class="flex items-center gap-4">
            <h2 class="text-xl font-semibold text-[var(--color-text)]">
              {navItems.find((item) => item.path === location.pathname)?.label || '대시보드'}
            </h2>
          </div>

          <div class="flex items-center gap-4">
            {/* Market Status Indicators */}
            <div class="flex items-center gap-3">
              <div class="flex items-center gap-2 px-3 py-1.5 rounded-full bg-[var(--color-surface-light)]">
                <div class="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
                <span class="text-sm text-[var(--color-text)]">KRX</span>
              </div>
              <div class="flex items-center gap-2 px-3 py-1.5 rounded-full bg-[var(--color-surface-light)]">
                <div class="w-2 h-2 rounded-full bg-red-500" />
                <span class="text-sm text-[var(--color-text)]">NYSE</span>
              </div>
              <div class="flex items-center gap-2 px-3 py-1.5 rounded-full bg-[var(--color-surface-light)]">
                <div class="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
                <span class="text-sm text-[var(--color-text)]">Crypto</span>
              </div>
            </div>

            {/* Notifications */}
            <button class="relative p-2 rounded-lg hover:bg-[var(--color-surface-light)] transition-colors">
              <Bell class="w-5 h-5 text-[var(--color-text-muted)]" />
              <span class="absolute top-1 right-1 w-2 h-2 bg-red-500 rounded-full" />
            </button>
          </div>
        </header>

        {/* Page content */}
        <main class="flex-1 overflow-auto p-6">
          {props.children}
        </main>
      </div>
    </div>
  )
}
