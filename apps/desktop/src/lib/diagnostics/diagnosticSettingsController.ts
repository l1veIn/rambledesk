import { get, writable } from 'svelte/store'
import type { DiagnosticsCapability } from '../capabilities/workbenchCapabilities'

type Action = 'loading' | 'saving' | 'clearing'
type DiagnosticSettingsState = {
  enabled: boolean | null
  busy: Action | null
  error: { action: Action; message: string } | null
  cleared: boolean
}

/** Native acknowledgement owns the setting; failed operations keep the last confirmed value. */
export function createDiagnosticSettingsController(
  api: Pick<DiagnosticsCapability, 'readSettings' | 'setEnabled' | 'clear'>,
  client: { setClientEnabled: (enabled: boolean) => void; flush: () => Promise<void> },
) {
  const state = writable<DiagnosticSettingsState>({ enabled: null, busy: null, error: null, cleared: false })

  async function run(action: Action, operation: () => Promise<void>) {
    if (get(state).busy) return false
    state.update(current => ({ ...current, busy: action, error: null, cleared: false }))
    try {
      await operation()
      return true
    } catch (cause) {
      state.update(current => ({
        ...current,
        error: { action, message: cause instanceof Error ? cause.message : String(cause) },
      }))
      return false
    } finally {
      state.update(current => ({ ...current, busy: null }))
    }
  }

  function acknowledge(enabled: boolean) {
    state.update(current => ({ ...current, enabled }))
    client.setClientEnabled(enabled)
  }

  return {
    subscribe: state.subscribe,
    load: () => run('loading', async () => { acknowledge((await api.readSettings()).enabled) }),
    setEnabled: (enabled: boolean) => run('saving', async () => { acknowledge((await api.setEnabled(enabled)).enabled) }),
    clear: () => run('clearing', async () => {
      await client.flush()
      await api.clear()
      state.update(current => ({ ...current, cleared: true }))
    }),
  }
}
