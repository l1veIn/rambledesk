import {
  RAMBLE_CONSOLE_COMMAND_EVENT,
  RAMBLE_CONSOLE_HIDE_EVENT,
  RAMBLE_CONSOLE_LABEL,
  RAMBLE_CONSOLE_READY_EVENT,
  RAMBLE_CONSOLE_SHOW_EVENT,
  RAMBLE_CONSOLE_STATE_EVENT,
} from '$lib/rambleConsole'

import type { RambleConsoleCapability } from '../workbenchCapabilities'
import { subscribeToTauriEvent } from './subscription'
import type { TauriCapabilityApi } from './tauriCapabilityApi'

export function createTauriRambleConsoleCapability(
  api: TauriCapabilityApi,
): RambleConsoleCapability {
  return {
    async show() {
      await api.invoke<void>('show_ramble_console')
      await api.emitTo(RAMBLE_CONSOLE_LABEL, RAMBLE_CONSOLE_SHOW_EVENT)
    },
    restoreVisibility: () => api.invoke<void>('show_ramble_console'),
    async hide() {
      await api.invoke<void>('hide_ramble_console')
      await api.emitTo(RAMBLE_CONSOLE_LABEL, RAMBLE_CONSOLE_HIDE_EVENT)
    },
    publish: (state) =>
      api.emitTo(RAMBLE_CONSOLE_LABEL, RAMBLE_CONSOLE_STATE_EVENT, state),
    onCommand: (handler, onError) =>
      subscribeToTauriEvent(api, RAMBLE_CONSOLE_COMMAND_EVENT, handler, onError),
    onReady: (handler, onError) =>
      subscribeToTauriEvent(api, RAMBLE_CONSOLE_READY_EVENT, handler, onError),
    recordDiagnostic: (activity, caseId) =>
      api.invoke<void>('record_diagnostic_event', { activity, caseId }),
  }
}
