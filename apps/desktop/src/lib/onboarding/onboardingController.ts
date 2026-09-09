import { get, writable } from 'svelte/store'

import type { Locale } from '../preferences'

export type OnboardingControllerState = Readonly<{
  open: boolean
  /** A launch update check was due while the wizard was open. */
  launchCheckDue: boolean
}>

export type OnboardingControllerContext = {
  locale: () => Locale
  isCompleted: () => boolean
  isAvailable: () => boolean
  startup: { start(): Promise<boolean> }
  managedSessions: { openNewManagedSession(configId?: string): Promise<boolean> }
  closeSettingsTab: () => Promise<void>
  updates: {
    available: boolean
    check: (input: { prompt: boolean; forcePrompt: boolean }) => Promise<void>
  }
  reset: () => void
}

/**
 * First-run onboarding: whether the wizard is open, the launch update check that
 * has to wait for it, and the "start a managed session" handoff.
 */
export function createOnboardingController(context: OnboardingControllerContext) {
  const store = writable<OnboardingControllerState>({ open: false, launchCheckDue: false })
  let updateCheckTimer: ReturnType<typeof setTimeout> | undefined

  function patch(next: Partial<OnboardingControllerState>) {
    store.update((current) => ({ ...current, ...next }))
  }

  /** Runs at mount: either starts the workbench or opens the wizard. */
  function begin() {
    if (context.isCompleted() || !context.isAvailable()) void context.startup.start()
    else patch({ open: true })
  }

  /** Desktop only: checks for an update shortly after launch, unless the wizard is open. */
  function scheduleLaunchCheck(delayMs = 4_000) {
    if (!context.updates.available) return
    updateCheckTimer = setTimeout(() => {
      patch({ launchCheckDue: true })
      if (!get(store).open) void context.updates.check({ prompt: true, forcePrompt: false })
    }, delayMs)
  }

  async function startSession(configId?: string) {
    if (!(await context.startup.start())) {
      throw new Error(
        context.locale() === 'zh-CN'
          ? '初始化未完成，请检查连接后重试。'
          : 'Initialization did not finish. Check the connection and retry.',
      )
    }
    if (!(await context.managedSessions.openNewManagedSession(configId))) {
      throw new Error(
        context.locale() === 'zh-CN'
          ? '暂时无法打开新会话，请稍后重试。'
          : 'Could not open a new session. Please retry.',
      )
    }
  }

  function close() {
    patch({ open: false })
    void context.startup.start()
    if (context.updates.available && get(store).launchCheckDue) {
      void context.updates.check({ prompt: true, forcePrompt: false })
    }
  }

  function restart() {
    context.reset()
    void context.closeSettingsTab()
    patch({ open: true })
  }

  function dispose() {
    if (updateCheckTimer !== undefined) clearTimeout(updateCheckTimer)
  }

  return {
    subscribe: store.subscribe,
    /** Svelte `bind:openWizard={$onboarding.open}` writes through the store. */
    set: store.set,
    begin,
    scheduleLaunchCheck,
    startSession,
    close,
    restart,
    dispose,
  }
}

export type OnboardingController = ReturnType<typeof createOnboardingController>
