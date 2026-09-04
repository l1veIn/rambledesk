import { get, writable } from 'svelte/store'
import type { ApplicationTransport } from '$lib/application/applicationTransport'
import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import { applicationResourcesAffectAgentConfigurations } from '$lib/application/applicationSnapshotRefetch'
import type { AgentConfig, AgentConnectionCheck, SaveAgentConfigInput } from '$lib/generated/feedback'
import { redactAgentMessage } from './agentConfigForm'

export type AgentSettingsState = Readonly<{ configs: AgentConfig[]; loading: boolean; error: string }>

export function createAgentSettingsController(transport: ApplicationTransport) {
  const state = writable<AgentSettingsState>({ configs: [], loading: true, error: '' })
  let active = false
  let generation = 0
  let unsubscribe: (() => void) | null = null

  function patch(next: Partial<AgentSettingsState>) {
    if (active) state.update((current) => ({ ...current, ...next }))
  }

  function message(cause: unknown): string {
    const source = cause instanceof Error ? cause.message
      : typeof cause === 'object' && cause !== null && 'message' in cause ? String(cause.message)
        : 'Could not load agent configurations.'
    const env = get(state).configs.flatMap((config) => Object.entries(config.env).map(([key, value]) => `${key}=${value}`)).join('\n')
    return redactAgentMessage(source, env)
  }

  async function refresh(): Promise<void> {
    if (!active) return
    const intent = ++generation
    patch({ loading: true, error: '' })
    try {
      await transport.waitUntilReady()
      if (!active || intent !== generation) return
      const configs = await transport.call('listAgentConfigs', undefined)
      if (active && intent === generation) patch({ configs, error: '' })
    } catch (cause) {
      if (active && intent === generation) patch({ error: message(cause) })
    } finally {
      if (active && intent === generation) patch({ loading: false })
    }
  }

  function start(): () => void {
    if (active) return dispose
    active = true
    unsubscribe = transport.subscribe(APPLICATION_EVENTS_STREAM, (event) => {
      if (event.type === 'ready' || applicationResourcesAffectAgentConfigurations(event.resources)) {
        void refresh()
      }
    }, (cause) => patch({ error: message(cause) }))
    void refresh()
    return dispose
  }

  function assertActive() {
    if (!active) throw new Error('Agent settings are no longer open.')
  }

  async function save(input: SaveAgentConfigInput): Promise<AgentConfig> {
    assertActive()
    generation += 1
    patch({ loading: false })
    const saved = await transport.call('saveAgentConfig', input)
    generation += 1
    patch({ configs: [...get(state).configs.filter((config) => config.id !== saved.id), saved], loading: false, error: '' })
    return saved
  }

  async function remove(id: string): Promise<void> {
    assertActive()
    generation += 1
    patch({ loading: false })
    await transport.call('deleteAgentConfig', { agent_config_id: id })
    generation += 1
    patch({ configs: get(state).configs.filter((config) => config.id !== id), loading: false, error: '' })
  }

  async function check(id: string): Promise<AgentConnectionCheck> {
    assertActive()
    return transport.call('checkAgentConfig', { agent_config_id: id })
  }

  function dispose(): void {
    active = false
    generation += 1
    unsubscribe?.()
    unsubscribe = null
  }

  return { subscribe: state.subscribe, start, refresh, save, remove, check, dispose }
}
