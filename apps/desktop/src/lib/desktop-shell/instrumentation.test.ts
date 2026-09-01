import { describe, expect, it, vi } from 'vitest'

import {
  createTauriDesktopShellInstrumentation,
  type DesktopShellCommandApi,
} from './instrumentation'

describe('Tauri Desktop Shell instrumentation', () => {
  it('owns the frontend error logging command envelope', async () => {
    const invoke = vi.fn(async () => undefined)
    const instrumentation = createTauriDesktopShellInstrumentation({
      invoke: invoke as DesktopShellCommandApi['invoke'],
    })

    await instrumentation.reportFrontendError('updater', 'download failed')

    expect(invoke).toHaveBeenCalledWith('log_frontend_error', {
      context: 'updater',
      message: 'download failed',
    })
  })

  it('keeps diagnostic logging best-effort', async () => {
    const instrumentation = createTauriDesktopShellInstrumentation({
      invoke: vi.fn(async () => {
        throw new Error('backend unavailable')
      }),
    })

    await expect(
      instrumentation.reportFrontendError('window', 'render failed'),
    ).resolves.toBeUndefined()
  })

  it('owns the main-window DevTools command', async () => {
    const invoke = vi.fn(async () => undefined)
    const instrumentation = createTauriDesktopShellInstrumentation({
      invoke: invoke as DesktopShellCommandApi['invoke'],
    })

    await instrumentation.openMainDevtools()

    expect(invoke).toHaveBeenCalledWith('open_main_devtools')
  })
})
