import { get } from 'svelte/store'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  createOnboardingController,
  type OnboardingControllerContext,
} from './onboardingController'

function harness(overrides: Partial<OnboardingControllerContext> = {}) {
  const context = {
    locale: () => 'en' as const,
    isCompleted: () => true,
    isAvailable: () => true,
    startup: { start: vi.fn(async () => true) },
    managedSessions: { openNewManagedSession: vi.fn(async () => true) },
    closeSettingsTab: vi.fn(async () => undefined),
    updates: { available: true, check: vi.fn(async () => undefined) },
    reset: vi.fn(),
    ...overrides,
  } as OnboardingControllerContext
  return { controller: createOnboardingController(context), context }
}

afterEach(() => {
  vi.useRealTimers()
})

describe('onboarding controller', () => {
  it('starts the workbench when onboarding is already complete', () => {
    const { controller, context } = harness()
    controller.begin()
    expect(context.startup.start).toHaveBeenCalled()
    expect(get(controller).open).toBe(false)
  })

  it('opens the wizard when onboarding is incomplete and available', () => {
    const { controller, context } = harness({ isCompleted: () => false })
    controller.begin()
    expect(get(controller).open).toBe(true)
    expect(context.startup.start).not.toHaveBeenCalled()
  })

  it('starts the workbench when the wizard is unavailable', () => {
    const { controller, context } = harness({ isCompleted: () => false, isAvailable: () => false })
    controller.begin()
    expect(get(controller).open).toBe(false)
    expect(context.startup.start).toHaveBeenCalled()
  })

  it('defers the launch update check while the wizard is open', async () => {
    vi.useFakeTimers()
    const { controller, context } = harness({ isCompleted: () => false })
    controller.begin()
    controller.scheduleLaunchCheck(10)
    await vi.advanceTimersByTimeAsync(10)
    expect(get(controller).launchCheckDue).toBe(true)
    expect(context.updates.check).not.toHaveBeenCalled()

    controller.close()
    expect(context.updates.check).toHaveBeenCalledWith({ prompt: true, forcePrompt: false })
    expect(context.startup.start).toHaveBeenCalled()
  })

  it('checks for updates straight away when the wizard is closed', async () => {
    vi.useFakeTimers()
    const { controller, context } = harness()
    controller.begin()
    controller.scheduleLaunchCheck(10)
    await vi.advanceTimersByTimeAsync(10)
    expect(context.updates.check).toHaveBeenCalledWith({ prompt: true, forcePrompt: false })
  })

  it('skips the scheduled check when updates are unavailable', async () => {
    vi.useFakeTimers()
    const { controller, context } = harness({ updates: { available: false, check: vi.fn() } })
    controller.scheduleLaunchCheck(10)
    await vi.advanceTimersByTimeAsync(10)
    expect(context.updates.check).not.toHaveBeenCalled()
  })

  it('starts a session and reports a failed handoff', async () => {
    const { controller } = harness()
    await expect(controller.startSession('config-1')).resolves.toBeUndefined()

    const failedStartup = harness({ startup: { start: vi.fn(async () => false) } })
    await expect(failedStartup.controller.startSession()).rejects.toThrow('Initialization did not finish')

    const failedSession = harness({
      managedSessions: { openNewManagedSession: vi.fn(async () => false) },
    })
    await expect(failedSession.controller.startSession()).rejects.toThrow('Could not open a new session')
  })

  it('restarts by resetting, closing the settings tab, and reopening', async () => {
    const { controller, context } = harness()
    controller.restart()
    await Promise.resolve()
    expect(context.reset).toHaveBeenCalled()
    expect(context.closeSettingsTab).toHaveBeenCalled()
    expect(get(controller).open).toBe(true)
  })
})
