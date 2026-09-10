import { afterEach, vi } from 'vitest'

import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
import type {
  AgentCatalogEntry,
  AgentConfig,
  AgentInspection,
  ManagedSessionSnapshot,
} from '$lib/generated/feedback'
import { configureClientDiagnostics, type ClientDiagnosticEvent } from '$lib/diagnostics/clientDiagnostics'
import { createDraftManagedSessionController } from './draftManagedSessionController'
import { createManagedSessionDraftStorage } from './managedSessionDrafts'

export const config: AgentConfig = { id: 'config', name: 'Pi', host_id: 'pi', protocol: 'acp', enabled: true, command: 'pi-acp', args: [], env: {}, created_at: '', updated_at: '' }
export const catalog: AgentCatalogEntry = { id: 'pi', name: 'Pi', host_id: 'pi', description: '', connection_kind: 'bridge', distribution: { kind: 'npm', package: 'pi-acp', command: 'pi-acp', pinned_version: '1.0.0', node_required: '22.0.0' }, args: [], dependencies: [], verification: { status: 'unverified', versions: [], note: '' } }
export const inspection: AgentInspection = { agent_id: 'pi', command: 'pi-acp', args: [], source: 'system', version: '1.0.0', checks: [], dependencies: [] }

export function snapshot(id: string, lifecycle: 'prepared' | 'active' = 'prepared', connection: 'connected' | 'failed' = 'connected'): ManagedSessionSnapshot {
  return { session: { session_id: id, host_id: 'pi', host_session_id: id, title: 'Task', lifecycle,
    management: { kind: 'managed', protocol: 'acp', agent_config_id: 'config', cwd: '/repo', remote_session_id: `remote-${id}` }, created_at: '', updated_at: '' },
    runtime: { configuration: { options: [] }, connection, activity: 'idle', instance_id: 'runtime', config_updated_at: null,
      capabilities: { prompt: { image: false, audio: false, embedded_context: true, resource_links: true }, load_session: false, resume_session: false, http_mcp: false }, last_error: connection === 'failed' ? 'Connection failed' : null },
    activities: [], interactions: [], deliveries: [], deleting: false, recovery: null }
}

export function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>((done) => { resolve = done }); return { promise, resolve } }

export async function flush() { for (let i = 0; i < 35; i++) await Promise.resolve() }

export function setup() {
  const data = new Map<string, string>()
  const storage = createManagedSessionDraftStorage({ getItem: (key) => data.get(key) ?? null, setItem: (key, value) => { data.set(key, value) } })
  storage.save('draft', { choice: 'config:config', cwd: '/repo', text: 'Draft task' })
  const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
    .resolve('listAgentConfigs', [config]).resolve('listAvailableAgents', []).resolve('discardPreparedSession', undefined)
    .resolve('prepareManagedSession', snapshot('one')).resolve('getManagedSession', snapshot('one'))
  const promoted = vi.fn()
  const controller = createDraftManagedSessionController(transport, 'draft', storage, promoted)
  return { transport, controller, promoted, storage, data }
}

let stopDiagnostics: (() => void) | undefined

export function captureDiagnostics() {
  const events: ClientDiagnosticEvent[] = []
  stopDiagnostics = configureClientDiagnostics(event => { events.push(event) })
  return events
}

export function releaseDiagnostics() {
  stopDiagnostics?.()
  stopDiagnostics = undefined
}

/** Replaces the sink with one that never settles, to prove nothing awaits it. */
export function stallDiagnostics() {
  stopDiagnostics?.()
  stopDiagnostics = configureClientDiagnostics(() => new Promise<void>(() => {}))
}
