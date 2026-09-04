import { get, writable } from 'svelte/store'
import type { ApplicationTransport } from '$lib/application/applicationTransport'
import type { AgentCatalogEntry, AgentInspection, AgentInstallJob, AgentConfig } from '$lib/generated/feedback'
import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'

export type AgentCatalogState = { entries: AgentCatalogEntry[]; jobs: AgentInstallJob[]; inspections: Record<string, AgentInspection>; checking: string[]; loading: boolean; error: string }
export function installIsActive(job: AgentInstallJob) { return ['preparing', 'installing', 'verifying'].includes(job.phase) }
export function catalogConfiguration(entry: AgentCatalogEntry, inspection: AgentInspection) {
  if (!inspection.command) throw new Error('Install this agent before using it.')
  if (inspection.checks.some(check => check.status === 'fail')) throw new Error('Resolve the failed checks before using this agent.')
  return { id: null, name: entry.name, host_id: entry.host_id, protocol: 'acp' as const, enabled: true, command: inspection.command, args: inspection.args, env: {} }
}
export function configurationsForAgent(entry: AgentCatalogEntry, configs: readonly AgentConfig[], inspection?: AgentInspection): AgentConfig[] {
  const command = entry.distribution.command.toLowerCase()
  const packagePath = entry.distribution.kind === 'npm' ? `/node_modules/${entry.distribution.package.toLowerCase()}/` : undefined
  const normalized = (value: string) => value.replace(/\\/gu, '/').toLowerCase()
  return configs.filter(config => config.host_id === entry.host_id && (
    normalized(config.command).split('/').at(-1)?.replace(/\.(cmd|exe|bat)$/iu, '') === command ||
    (packagePath && config.args.some(arg => normalized(arg).includes(packagePath))) ||
    (config.command === inspection?.command && config.args.length === inspection.args.length && config.args.every((arg, index) => arg === inspection.args[index]))
  ))
}

export function createAgentCatalogController(transport: ApplicationTransport) {
  const state = writable<AgentCatalogState>({ entries: [], jobs: [], inspections: {}, checking: [], loading: true, error: '' })
  let active = false
  let timer: ReturnType<typeof setTimeout> | undefined
  let unsubscribe: (() => void) | undefined
  let fetchingJobs = false
  const checks = new Map<string, Promise<AgentInspection | undefined>>()
  const seenCompleted = new Set<string>()
  function patch(value: Partial<AgentCatalogState>) { if (active) state.update(current => ({ ...current, ...value })) }
  function failure(error: unknown) { patch({ error: typeof error === 'object' && error && 'message' in error ? String(error.message) : String(error) }) }

  async function inspect(agentId: string): Promise<AgentInspection | undefined> {
    if (!active) return
    if (checks.has(agentId)) return checks.get(agentId)
    const task = (async () => {
      patch({ checking: [...get(state).checking, agentId], error: '' })
      try {
        const result = await transport.call('inspectAgentInstallation', { agent_id: agentId })
        patch({ inspections: { ...get(state).inspections, [agentId]: result } })
        return result
      } catch (error) { failure(error); return undefined }
      finally { checks.delete(agentId); patch({ checking: get(state).checking.filter(id => id !== agentId) }) }
    })()
    checks.set(agentId, task)
    return task
  }

  async function refreshJobs() {
    if (!active || fetchingJobs) return
    fetchingJobs = true
    try {
      const jobs = await transport.call('listAgentInstallJobs', undefined)
      patch({ jobs })
      for (const job of jobs) {
        if (job.phase === 'complete' && !seenCompleted.has(job.id)) {
          seenCompleted.add(job.id)
          void inspect(job.agent_id)
        }
      }
    } catch (error) { failure(error) }
    finally {
      fetchingJobs = false
      if (active && get(state).jobs.some(installIsActive)) {
        clearTimeout(timer)
        timer = setTimeout(() => void refreshJobs(), 500)
      }
    }
  }

  async function refresh() {
    if (!active) return
    patch({ loading: true, error: '' })
    try {
      await transport.waitUntilReady()
      if (!active) return
      const entries = await transport.call('listAvailableAgents', undefined)
      patch({ entries })
      await refreshJobs()
    } catch (error) { failure(error) }
    finally { patch({ loading: false }) }
  }

  async function install(agentId: string) {
    if (!active) return
    patch({ error: '' })
    try {
      const job = await transport.call('installAgent', { agent_id: agentId, version: null })
      patch({ jobs: [...get(state).jobs.filter(item => item.id !== job.id), job] })
      void refreshJobs()
    } catch (error) { failure(error) }
  }
  async function cancel(jobId: string) {
    if (!active) return
    try { await transport.call('cancelAgentInstall', { job_id: jobId }); await refreshJobs() }
    catch (error) { failure(error) }
  }
  function start() {
    if (active) return dispose
    active = true
    unsubscribe = transport.subscribe(APPLICATION_EVENTS_STREAM, event => {
      if (event.type === 'ready' || event.resources.some(resource => ['all', 'agent_configurations'].includes(resource.kind))) void refreshJobs()
    }, failure)
    void refresh()
    return dispose
  }
  function dispose() { active = false; clearTimeout(timer); unsubscribe?.() }
  return { subscribe: state.subscribe, start, dispose, refresh, inspect, install, cancel }
}
