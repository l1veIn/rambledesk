import { invoke } from '@tauri-apps/api/core'
import type { ClientDiagnosticEvent } from '../diagnostics/clientDiagnostics'

export type DesktopShellInstrumentation = Readonly<{
  reportFrontendError: (context: string, message: string) => Promise<void>
  recordClientDiagnostic: (input: ClientDiagnosticEvent) => Promise<void>
  openMainDevtools: () => Promise<void>
}>

export type DesktopShellCommandApi = Readonly<{
  invoke<Result>(command: string, args?: Record<string, unknown>): Promise<Result>
}>

const DEFAULT_DESKTOP_SHELL_COMMAND_API: DesktopShellCommandApi = { invoke }

/** Owns commands used to instrument and inspect the Desktop shell itself. */
export function createTauriDesktopShellInstrumentation(
  api: DesktopShellCommandApi = DEFAULT_DESKTOP_SHELL_COMMAND_API,
): DesktopShellInstrumentation {
  return {
    // The recorder owns failure handling and loss counters; preserve rejection to that boundary.
    recordClientDiagnostic: input => api.invoke<void>('record_client_diagnostic', { input }),
    reportFrontendError: (context, message) =>
      api
        .invoke<void>('log_frontend_error', { context, message })
        .catch(() => undefined),
    openMainDevtools: () => api.invoke<void>('open_main_devtools'),
  }
}

export const TAURI_DESKTOP_SHELL_INSTRUMENTATION =
  createTauriDesktopShellInstrumentation()
