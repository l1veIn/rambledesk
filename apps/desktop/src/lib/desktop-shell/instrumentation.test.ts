import { describe, expect, it, vi } from 'vitest'

import {
  createTauriDesktopShellInstrumentation,
  type DesktopShellCommandApi,
} from './instrumentation'

describe('Tauri Desktop Shell instrumentation', () => {
  it('sends structured local diagnostics to the native receiver', async () => {
    const invoke = vi.fn(async () => undefined)
    const instrumentation = createTauriDesktopShellInstrumentation({ invoke: invoke as DesktopShellCommandApi['invoke'] })
    const input = { activity: 'agent_detection', outcome: 'ok' as const, operationId: '00000000-0000-4000-8000-000000000001', details: { checked_count: 2 } }
    await instrumentation.recordClientDiagnostic(input)
    expect(invoke).toHaveBeenCalledWith('record_client_diagnostic', { input })
  })
  it('lets the recorder observe a rejected metadata write', async () => {
    const instrumentation = createTauriDesktopShellInstrumentation({ invoke: async () => { throw Error('receiver unavailable') } })
    await expect(instrumentation.recordClientDiagnostic({ activity: 'onboarding', outcome: 'ok', operationId: '00000000-0000-4000-8000-000000000001' }))
      .rejects.toThrow('receiver unavailable')
  })
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
