import { get, writable } from 'svelte/store'
import type { ApplicationTransport } from '$lib/application/applicationTransport'
import type { AgentCatalogEntry, AgentInspection, AgentInstallJob, AgentConfig, AgentConnectionCheck, SaveAgentConfigInput } from '$lib/generated/feedback'
import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import { diagnosticAgentId, diagnosticErrorCategory, recordClientDiagnostic, startClientDiagnostic } from '$lib/diagnostics/clientDiagnostics'
import { isAbsoluteAgentDirectory, redactAgentMessage } from './agentConfigForm'
import { agentLaunchSignature as launchSignature, beginAgentConnection, beginAgentInspection, forgetAgentConnection, observeAgentRuntime, readAgentDetectionCache, reconcileAgentConnections, redactAgentConnection, resetAgentDetectionCache, subscribeAgentDetectionCache } from './agentDetectionCache'
export { agentLaunchSignature as launchSignature } from './agentDetectionCache'
import { agentDiagnosis, prefersManagedDeepSeek } from './agentDiagnosis'
export { agentDiagnosis, connectionPreparationAvailable } from './agentDiagnosis'

type ConnectionResult = { signature: string; result: AgentConnectionCheck }
export type AgentCatalogState = {
  entries: AgentCatalogEntry[]; configs: AgentConfig[]; jobs: AgentInstallJob[]
  inspections: Record<string, AgentInspection>; connections: Record<string, ConnectionResult>
  checking: string[]; connecting: string[]; loading: boolean; error: string
}
export function installIsActive(job: AgentInstallJob) { return ['preparing', 'installing', 'verifying'].includes(job.phase) }
export function catalogConfiguration(entry: AgentCatalogEntry, inspection: AgentInspection) {
  if (!inspection.command) throw new Error('Install this agent before using it.')
  if (inspection.checks.some(check => check.status === 'fail')) throw new Error('Resolve the failed checks before using this agent.')
  return { id: null, catalog_id: entry.id, name: entry.name, host_id: entry.host_id, protocol: 'acp' as const, enabled: true, command: inspection.command, args: inspection.args, env: inspection.env ?? {} }
}
/** Explicitly use the detected ACP launch while retaining this profile's account settings. */
export function detectedAgentConfiguration(entry: AgentCatalogEntry, inspection: AgentInspection, previous?: AgentConfig): SaveAgentConfigInput {
  const defaults = catalogConfiguration(entry, inspection)
  return { ...defaults, id: previous?.id ?? null, name: previous?.name ?? defaults.name,
    env: { ...previous?.env, ...defaults.env } }
}
export function configurationsForAgent(entry: AgentCatalogEntry, configs: readonly AgentConfig[]): AgentConfig[] {
  return configs.filter(config => config.catalog_id === entry.id)
}
export type AgentListItem = { key: string; name: string; entry?: AgentCatalogEntry; config?: AgentConfig; configs: AgentConfig[]; legacy?: boolean }
/** One row per known Agent. Every saved profile remains accessible in advanced settings. */
export function agentListItems(entries: readonly AgentCatalogEntry[], configs: readonly AgentConfig[], connections: AgentCatalogState['connections'] = {}): AgentListItem[] {
  return [...entries.map(entry => {
    const profiles = configurationsForAgent(entry, configs)
    const checked = profiles.find(config => config.enabled && connections[config.id]?.result.ok && connections[config.id].signature === launchSignature(config))
    return { key: `catalog:${entry.id}`, name: entry.name, entry, config: checked ?? profiles.find(config => config.enabled) ?? profiles[0], configs: profiles }
  }), ...configs.filter(config => !entries.some(entry => entry.id === config.catalog_id))
    .map(config => ({ key: `config:${config.id}`, name: config.name, config, configs: [config], legacy: Boolean(config.catalog_id) }))]
}
export function agentConnectionResult(config: AgentConfig | undefined, state: AgentCatalogState): AgentConnectionCheck | undefined {
  if (!config) return
  const checked = state.connections[config.id]
  return checked?.signature === launchSignature(config) ? checked.result : undefined
}
export type AgentStatus = 'unchecked' | 'missing' | 'prepare' | 'checking' | 'connected' | 'attention'
export function agentStatus(row: AgentListItem, state: AgentCatalogState): AgentStatus {
  const diagnosis = agentDiagnosis(row, state)
  if (diagnosis.connection === 'failed') return 'attention'
  if (diagnosis.connection === 'connected' && diagnosis.reason !== 'none') return 'attention'
  return diagnosis.connection
}
export function manualAgentConfiguration(entry: AgentCatalogEntry, path: string, previous?: AgentConfig): SaveAgentConfigInput {
  const command = path.trim().replace(/^"(.*)"$/u, '$1')
  if (!isAbsoluteAgentDirectory(command)) throw new Error('Enter the full path to the CLI or ACP executable.')
  if (/\.(?:m?js|cjs)$/iu.test(command)) throw new Error('JavaScript entry points need a runtime command. Use advanced launch settings.')
  return { id: previous?.id ?? null, catalog_id: entry.id, name: previous?.name ?? entry.name,
    host_id: previous?.host_id ?? entry.host_id, protocol: 'acp', enabled: true,
    command, args: [...entry.args], env: { ...previous?.env } }
}

type ConnectionIntent = { configId?: string; signature?: string }
type ConnectionPreparation = { intent: ConnectionIntent; started: Promise<void> }
type ConnectionPreparations = {
  pending: Map<string, ConnectionPreparation>
  jobs: Map<string, AgentInstallJob>
  error: string
  listeners: Set<(saved?: AgentConfig) => void>
}
// A requested connection belongs to the application connection, not its settings view.
// Only connect() registers work here; historical installation jobs remain read-only.
const connectionPreparations = new WeakMap<ApplicationTransport, ConnectionPreparations>()
function preparationsFor(transport: ApplicationTransport): ConnectionPreparations {
  let shared = connectionPreparations.get(transport)
  if (!shared) {
    shared = { pending: new Map(), jobs: new Map(), error: '', listeners: new Set() }
    connectionPreparations.set(transport, shared)
  }
  return shared
}
function prepareConnection(transport: ApplicationTransport, entry: AgentCatalogEntry, intent: ConnectionIntent, knownConfigs: AgentConfig[]): Promise<void> {
  const shared = preparationsFor(transport)
  const existing = shared.pending.get(entry.id)
  if (existing) return existing.started
  let releaseStart!: () => void
  const preparation: ConnectionPreparation = { intent, started: new Promise(resolve => { releaseStart = resolve }) }
  shared.pending.set(entry.id, preparation)
  shared.error = ''
  const publish = (saved?: AgentConfig) => { for (const listener of shared.listeners) listener(saved) }
  publish()
  const details = { source: 'catalog', agent_kind: entry.connection_kind, agent: diagnosticAgentId(entry.id) }
  const finishInstall = startClientDiagnostic('agent_install', { ...details, action: 'install' })
  let finishRecheck: ReturnType<typeof startClientDiagnostic> | undefined
  let environments = knownConfigs
  void (async () => {
    try {
      let job = await transport.call('installAgent', { agent_id: entry.id, version: null })
      shared.jobs.set(entry.id, job)
      publish()
      if (installIsActive(job)) releaseStart()
      while (installIsActive(job)) {
        await new Promise(resolve => setTimeout(resolve, 500))
        const jobs = await transport.call('listAgentInstallJobs', undefined)
        const updated = jobs.find(candidate => candidate.id === job.id)
        if (!updated) throw new Error('Connection preparation status is unavailable. Check Agents and retry.')
        job = updated
        shared.jobs.set(entry.id, job)
        publish()
      }
      finishInstall(job.phase === 'complete' ? 'ok' : job.phase === 'cancelled' ? 'cancelled' : 'failed', { status: job.phase })
      if (job.phase === 'cancelled') return
      if (job.phase !== 'complete') throw new Error('Could not prepare the connection. See the installation details and retry.')
      finishRecheck = startClientDiagnostic('agent_install', { ...details, action: 'recheck', reason: 'post_install', target_count: 1 })
      const rememberInspection = beginAgentInspection(transport, entry.id)
      const found = await transport.call('inspectAgentInstallation', { agent_id: entry.id })
      rememberInspection(found)
      // Re-read after discovery so edits made while the view was closed remain authoritative.
      const configs = await transport.call('listAgentConfigs', undefined)
      environments = configs
      const previous = configs.find(config => config.id === intent.configId)
      if (intent.configId ? !previous || previous.catalog_id !== entry.id || launchSignature(previous) !== intent.signature
        : configs.some(config => config.catalog_id === entry.id)) {
        throw new Error('The configuration changed during installation. Check the saved connection before continuing.')
      }
      const saved = await transport.call('saveAgentConfig', detectedAgentConfiguration(entry, found, previous))
      environments = [...configs, saved]
      for (const config of configs.filter(config => config.catalog_id === entry.id)) forgetAgentConnection(transport, config.id)
      publish(saved)
      const rememberConnection = beginAgentConnection(transport, saved)
      let checked: AgentConnectionCheck
      try { checked = await transport.call('checkAgentConfig', { agent_config_id: saved.id }) }
      catch (error) {
        const message = typeof error === 'object' && error && 'message' in error ? String(error.message) : String(error)
        checked = { ok: false, message, details: [] }
      }
      checked = redactAgentConnection(saved, checked)
      rememberConnection(checked)
      finishRecheck(checked.ok ? 'ok' : 'failed', { checked_count: 1 })
    } catch (error) {
      finishInstall('failed', { error_category: diagnosticErrorCategory(error) })
      finishRecheck?.('failed', { error_category: diagnosticErrorCategory(error) })
      const message = typeof error === 'object' && error && 'message' in error ? String(error.message) : String(error)
      shared.error = redactAgentMessage(message, environments.flatMap(config => Object.entries(config.env).map(([key, value]) => `${key}=${value}`)).join('\n'))
    } finally {
      if (shared.pending.get(entry.id) === preparation) {
        shared.pending.delete(entry.id)
        shared.jobs.delete(entry.id)
      }
      publish()
      releaseStart()
    }
  })()
  return preparation.started
}

export function createAgentCatalogController(transport: ApplicationTransport) {
  const state = writable<AgentCatalogState>({ entries: [], configs: [], jobs: [], ...readAgentDetectionCache(transport), checking: [], connecting: [], loading: true, error: '' })
  let active = false
  let timer: ReturnType<typeof setTimeout> | undefined
  let unsubscribe: (() => void) | undefined
  let unsubscribeCache: (() => void) | undefined
  let unsubscribePreparations: (() => void) | undefined
  let preparingAgents: string[] = []
  const preparations = preparationsFor(transport)
  let fetchingJobs = false
  let refreshing: Promise<void> | undefined
  let detecting: Promise<void> | undefined
  let configGeneration = 0
  let runningConnections = 0
  const pendingConnections: Array<() => void> = []
  const checks = new Map<string, Promise<AgentInspection | undefined>>()
  const connectionChecks = new Map<string, Promise<AgentConnectionCheck | undefined>>()
  const handshakes = new Map<string, Promise<AgentConnectionCheck>>()
  const seenCompleted = new Set<string>()
  const requestedJobs = new Set<string>()
  const installationDiagnostics = new Map<string, ReturnType<typeof startClientDiagnostic>>()
  const attemptedCatalogs = new Set<string>()
  let lastRefreshSummary = ''
  function cacheSummary() {
    const cache = readAgentDetectionCache(transport)
    const cacheCount = Object.keys(cache.inspections).length + Object.keys(cache.connections).length
    return { config_count: get(state).configs.length, cache_count: cacheCount, cache_hit: cacheCount > 0 }
  }
  function patch(value: Partial<AgentCatalogState>) { if (active) state.update(current => ({ ...current, ...value })) }
  function message(error: unknown, configs = get(state).configs) {
    const text = typeof error === 'object' && error && 'message' in error ? String(error.message) : String(error)
    return redactAgentMessage(text, configs.flatMap(config => Object.entries(config.env).map(([key, value]) => `${key}=${value}`)).join('\n'))
  }
  function failure(error: unknown) { patch({ error: message(error) }) }
  function agentDiagnosticDetails(agentId: string | undefined) {
    const kind = get(state).entries.find(entry => entry.id === agentId)?.connection_kind
    return { agent_kind: kind === 'native' || kind === 'bridge' ? kind : 'custom', agent: diagnosticAgentId(agentId) }
  }
  async function inspect(agentId: string, reason: 'manual' | 'onboarding' | 'post_install' = 'manual'): Promise<AgentInspection | undefined> {
    if (!active) return
    if (reason === 'manual') preparations.error = ''
    if (checks.has(agentId)) return checks.get(agentId)
    const finish = startClientDiagnostic('agent_detection', { action: 'inspect', source: 'catalog', reason, ...agentDiagnosticDetails(agentId) })
    const rememberInspection = beginAgentInspection(transport, agentId)
    const task = (async () => {
      patch({ checking: [...get(state).checking, agentId], error: '' })
      try {
        const result = await transport.call('inspectAgentInstallation', { agent_id: agentId })
        const accepted = rememberInspection(result)
        finish(accepted ? 'ok' : 'cancelled', { status: result.source, failed_count: result.checks.filter(check => check.status === 'fail').length })
        return accepted ? result : undefined
      } catch (error) { finish('failed', { error_category: diagnosticErrorCategory(error) }); failure(error); return undefined }
      finally { checks.delete(agentId); patch({ checking: get(state).checking.filter(id => id !== agentId) }) }
    })()
    checks.set(agentId, task)
    return task
  }
  async function refreshConfigs() {
    if (!active) return
    const generation = ++configGeneration
    const configs = await transport.call('listAgentConfigs', undefined)
    if (active && generation === configGeneration) {
      reconcileAgentConnections(transport, configs)
      patch({ configs, ...readAgentDetectionCache(transport) })
    }
  }
  function remember(config: AgentConfig) {
    configGeneration += 1
    const configs = get(state).configs
    const updated = configs.some(item => item.id === config.id) ? configs.map(item => item.id === config.id ? config : item) : [...configs, config]
    reconcileAgentConnections(transport, updated)
    patch({ configs: updated })
  }
  async function save(input: SaveAgentConfigInput) {
    if (!active) throw new Error('Agent settings are no longer open.')
    preparations.error = ''
    const finish = startClientDiagnostic('agent_config', { action: 'save', source: 'catalog', ...agentDiagnosticDetails(input.catalog_id) })
    configGeneration += 1
    try {
      const saved = await transport.call('saveAgentConfig', input)
      remember(saved)
      finish('ok')
      return saved
    } catch (error) { finish('failed', { error_category: diagnosticErrorCategory(error) }); throw error }
  }
  async function remove(id: string) {
    if (!active) return
    const finish = startClientDiagnostic('agent_config', { action: 'delete', source: 'catalog', ...agentDiagnosticDetails(get(state).configs.find(config => config.id === id)?.catalog_id) })
    configGeneration += 1
    try {
      await transport.call('deleteAgentConfig', { agent_config_id: id })
      configGeneration += 1
      patch({ configs: get(state).configs.filter(config => config.id !== id) })
      forgetAgentConnection(transport, id)
      finish('ok')
    } catch (error) { finish('failed', { error_category: diagnosticErrorCategory(error) }); throw error }
  }
  async function resolve(agentId: string) {
    const existing = get(state).configs.find(config => config.catalog_id === agentId && config.enabled)
      ?? get(state).configs.find(config => config.catalog_id === agentId)
    if (existing) return existing
    const finish = startClientDiagnostic('agent_config', { action: 'resolve', source: 'catalog', ...agentDiagnosticDetails(agentId) })
    try {
      const config = await transport.call('resolveCatalogAgent', { agent_id: agentId, enable: false })
      remember(config)
      finish('ok')
      return config
    } catch (error) { finish('failed', { error_category: diagnosticErrorCategory(error) }); throw error }
  }
  function pumpConnections() {
    while (runningConnections < 2 && pendingConnections.length) pendingConnections.shift()!()
  }
  function queuedConnection(key: string, operation: () => Promise<AgentConnectionCheck | undefined>) {
    if (connectionChecks.has(key)) return connectionChecks.get(key)!
    patch({ connecting: [...get(state).connecting, key] })
    const task = new Promise<AgentConnectionCheck | undefined>((resolve) => {
      pendingConnections.push(() => {
        runningConnections += 1
        void (async () => {
          try { resolve(active ? await operation() : undefined) }
          catch (error) { failure(error); resolve(undefined) }
          finally {
            runningConnections -= 1
            connectionChecks.delete(key)
            patch({ connecting: get(state).connecting.filter(item => item !== key) })
            pumpConnections()
          }
        })()
      })
    })
    connectionChecks.set(key, task)
    pumpConnections()
    return task
  }
  async function performCheck(original: AgentConfig, explicit: boolean, reason: 'manual' | 'onboarding' | 'post_install'): Promise<AgentConnectionCheck | undefined> {
    if (!active) return
    let config = get(state).configs.find(item => item.id === original.id)
    if (!config) return
    if (!config.enabled) {
      if (!explicit) return
      config = await transport.call('saveAgentConfig', { ...config, enabled: true })
      remember(config)
    }
    const signature = launchSignature(config)
    const rememberConnection = beginAgentConnection(transport, config)
    let handshake = handshakes.get(signature)
    if (!handshake) {
      const finish = startClientDiagnostic('agent_connection', { action: 'check', source: 'catalog', reason, explicit, ...agentDiagnosticDetails(config.catalog_id) })
      handshake = (async () => {
        try {
          const checked = await transport.call('checkAgentConfig', { agent_config_id: config.id })
          finish(checked.ok ? 'ok' : 'failed')
          return redactAgentConnection(config, checked)
        } catch (error) { finish('failed', { error_category: diagnosticErrorCategory(error) }); return { ok: false, message: message(error, [config]), details: [] } }
      })()
      handshakes.set(signature, handshake)
    }
    const result = await handshake
    handshakes.delete(signature)
    return rememberConnection(result) ? result : undefined
  }
  async function checkConfig(config: AgentConfig, explicit = true, reason: 'manual' | 'onboarding' | 'post_install' = 'manual') {
    if (!active || (!explicit && !config.enabled)) {
      recordClientDiagnostic({ activity: 'agent_connection', outcome: 'skipped', details: { action: 'check', source: 'catalog', reason: active ? 'disabled' : 'inactive', explicit, agent: diagnosticAgentId(config.catalog_id) } })
      return
    }
    if (explicit) preparations.error = ''
    if (get(state).jobs.some(job => job.agent_id === config.catalog_id && installIsActive(job))) return
    if (!explicit && agentConnectionResult(config, get(state))) {
      recordClientDiagnostic({ activity: 'agent_connection', outcome: 'skipped', details: { action: 'check', source: 'catalog', reason: 'cache_hit', cache_hit: true, agent: diagnosticAgentId(config.catalog_id) } })
      return agentConnectionResult(config, get(state))
    }
    return queuedConnection(`config:${config.id}`, () => performCheck(config, explicit, reason))
  }
  async function check(agentConfigId: string) {
    const config = get(state).configs.find(config => config.id === agentConfigId)
    return config ? checkConfig(config) : undefined
  }
  async function checkAgent(agentId: string, explicit = true, reason: 'manual' | 'onboarding' | 'post_install' = 'manual') {
    if (!active) return
    const snapshot = get(state)
    if (snapshot.jobs.some(job => job.agent_id === agentId && installIsActive(job))) return
    const row = agentListItems(snapshot.entries, snapshot.configs).find(item => item.entry?.id === agentId)
    if (!row?.entry) return
    if (row.config) return checkConfig(row.config, explicit, reason)
    const inspection = snapshot.inspections[agentId]
    // Detection is read-only for this recipe until the user chooses managed setup
    // or explicitly selects the discovered launch through advanced settings.
    if (prefersManagedDeepSeek(row, inspection)) return
    if (!inspection?.command || inspection.checks.some(check => check.status === 'fail') || row.entry.verification.status === 'unsupported') return
    const attempt = JSON.stringify([agentId, inspection])
    if (!explicit && attemptedCatalogs.has(attempt)) return
    attemptedCatalogs.add(attempt)
    return queuedConnection(row.key, async () => performCheck(await resolve(agentId), explicit, reason))
  }
  async function checkAll(reason: 'manual' | 'onboarding') {
    if (!active) return
    const snapshot = get(state)
    await Promise.all([
      ...snapshot.configs.filter(config => config.enabled && (!config.catalog_id || snapshot.entries.some(entry => entry.id === config.catalog_id))).map(config => checkConfig(config, false, reason)),
      ...snapshot.entries.filter(entry => !snapshot.configs.some(config => config.catalog_id === entry.id)).map(entry => checkAgent(entry.id, false, reason)),
    ])
  }
  async function completeRequestedInstallation(job: AgentInstallJob) {
    if (!active || job.phase !== 'complete' || !requestedJobs.has(job.id) || seenCompleted.has(job.id)) return
    seenCompleted.add(job.id)
    const finish = startClientDiagnostic('agent_install', { action: 'recheck', source: 'catalog', reason: 'post_install', target_count: 1, ...agentDiagnosticDetails(job.agent_id) })
    try {
      await refreshConfigs()
      await inspect(job.agent_id, 'post_install')
      for (const config of get(state).configs.filter(config => config.catalog_id === job.agent_id)) forgetAgentConnection(transport, config.id)
      patch({ connections: readAgentDetectionCache(transport).connections })
      const result = await checkAgent(job.agent_id, false, 'post_install')
      finish(!active ? 'cancelled' : result ? result.ok ? 'ok' : 'failed' : 'skipped', { checked_count: result ? 1 : 0 })
    } catch (error) { finish('failed', { error_category: diagnosticErrorCategory(error) }); throw error }
  }
  function observeInstallation(job: AgentInstallJob) {
    if (installIsActive(job)) return
    installationDiagnostics.get(job.id)?.(job.phase === 'complete' ? 'ok' : job.phase === 'cancelled' ? 'cancelled' : 'failed', { status: job.phase })
    installationDiagnostics.delete(job.id)
  }
  async function refreshJobs() {
    if (!active || fetchingJobs) return
    fetchingJobs = true
    try {
      const jobs = await transport.call('listAgentInstallJobs', undefined)
      patch({ jobs })
      for (const job of jobs) {
        observeInstallation(job)
        await completeRequestedInstallation(job)
      }
    } catch (error) { failure(error) }
    finally {
      fetchingJobs = false
      if (active && get(state).jobs.some(job => installIsActive(job) && !preparations.pending.has(job.agent_id))) {
        clearTimeout(timer)
        timer = setTimeout(() => void refreshJobs(), 500)
      }
    }
  }
  async function inspectAll(reason: 'manual' | 'onboarding' = 'manual') {
    const entries = get(state).entries
    let failedCount = 0
    for (let index = 0; active && index < entries.length; index += 3) {
      const results = await Promise.all(entries.slice(index, index + 3).map(entry => inspect(entry.id, reason)))
      failedCount += results.filter(result => !result).length
    }
    return failedCount
  }
  /** Read catalog/configuration records and prior results without launching Agent processes. */
  function refresh(reason: 'mount' | 'refresh' | 'manual' | 'onboarding' = 'refresh'): Promise<void> {
    if (!active) return Promise.resolve()
    if (refreshing) return refreshing
    const finish = startClientDiagnostic('agent_catalog_refresh', { action: 'refresh', source: 'catalog', reason, rescan: false })
    refreshing = (async () => {
      patch({ loading: true, error: '' })
      try {
        await transport.waitUntilReady()
        if (!active) { finish('cancelled', { reason: 'inactive' }); return }
        const [entries] = await Promise.all([transport.call('listAvailableAgents', undefined), refreshConfigs()])
        patch({ entries })
        await refreshJobs()
        const summary = cacheSummary()
        lastRefreshSummary = JSON.stringify(summary)
        finish(active ? 'ok' : 'cancelled', { ...summary, entry_count: entries.length })
      } catch (error) { finish('failed', { error_category: diagnosticErrorCategory(error) }); failure(error) }
      finally { refreshing = undefined; patch({ loading: false }) }
    })()
    return refreshing
  }
  /** Only the Detect action or the explicit first-run onboarding step invokes a full scan. */
  function detectAll(reason: 'manual' | 'onboarding' = 'manual'): Promise<void> {
    if (!active) return Promise.resolve()
    if (detecting) return detecting
    const finish = startClientDiagnostic('agent_detection', { action: 'scan', source: 'catalog', reason, rescan: true })
    detecting = (async () => {
      await refresh(reason)
      if (!active) { finish('cancelled', { reason: 'inactive' }); return }
      const refreshFailed = Boolean(get(state).error)
      const inspectionFailures = await inspectAll(reason)
      attemptedCatalogs.clear()
      const snapshotBeforeChecks = get(state)
      for (const config of snapshotBeforeChecks.configs.filter(config => !config.catalog_id || snapshotBeforeChecks.entries.some(entry => entry.id === config.catalog_id))) forgetAgentConnection(transport, config.id)
      patch({ connections: readAgentDetectionCache(transport).connections })
      await checkAll(reason)
      const snapshot = get(state)
      const failedCount = inspectionFailures + Object.values(snapshot.connections).filter(check => !check.result.ok).length
      finish(!active ? 'cancelled' : refreshFailed || failedCount ? 'failed' : 'ok', { entry_count: snapshot.entries.length, checked_count: Object.keys(snapshot.connections).length, failed_count: failedCount })
    })().catch(error => { finish('failed', { error_category: diagnosticErrorCategory(error) }); throw error }).finally(() => { detecting = undefined })
    return detecting
  }
  async function install(agentId: string) {
    if (!active) return
    preparations.error = ''
    patch({ error: '' })
    const finish = startClientDiagnostic('agent_install', { action: 'install', source: 'catalog', ...agentDiagnosticDetails(agentId) })
    try {
      const job = await transport.call('installAgent', { agent_id: agentId, version: null })
      requestedJobs.add(job.id)
      if (active) installationDiagnostics.set(job.id, finish)
      else finish('cancelled', { reason: 'inactive' })
      observeInstallation(job)
      patch({ jobs: [...get(state).jobs.filter(item => item.id !== job.id), job] })
      if (job.phase === 'complete') await completeRequestedInstallation(job)
      else if (installIsActive(job)) {
        // An in-flight cheap job read may predate this new installation.
        clearTimeout(timer)
        timer = setTimeout(() => void refreshJobs(), 500)
      }
      void refreshJobs()
    } catch (error) { finish('failed', { error_category: diagnosticErrorCategory(error) }); failure(error) }
  }
  async function connect(agentId: string, configId?: string) {
    if (!active) return
    const entry = get(state).entries.find(entry => entry.id === agentId)
    if (!entry) return
    const config = get(state).configs.find(config => config.id === configId)
    if (configId && (!config || config.catalog_id !== agentId)) return
    await prepareConnection(transport, entry, { configId, signature: config && launchSignature(config) }, get(state).configs)
  }
  async function cancel(jobId: string) {
    if (!active) return
    const finish = startClientDiagnostic('agent_install', { action: 'cancel', source: 'catalog' })
    try { await transport.call('cancelAgentInstall', { job_id: jobId }); finish('ok'); await refreshJobs() }
    catch (error) { finish('failed', { error_category: diagnosticErrorCategory(error) }); failure(error) }
  }
  function start() {
    if (active) return dispose
    active = true
    const backgroundPreparationError = preparations.error
    unsubscribeCache = subscribeAgentDetectionCache(transport, patch)
    const observePreparations = (saved?: AgentConfig) => {
      if (saved) remember(saved)
      const pending = [...preparations.pending.keys()]
      patch({
        jobs: [...get(state).jobs.filter(job => !preparations.jobs.has(job.agent_id)), ...preparations.jobs.values()],
        checking: [...get(state).checking.filter(id => !preparingAgents.includes(id)), ...pending],
        ...(preparations.error ? { error: preparations.error } : {}),
      })
      preparingAgents = pending
    }
    preparations.listeners.add(observePreparations)
    unsubscribePreparations = () => preparations.listeners.delete(observePreparations)
    observePreparations()
    unsubscribe = transport.subscribe(APPLICATION_EVENTS_STREAM, event => {
      observeAgentRuntime(transport, event.runtime_generation)
      if (event.type === 'ready' || event.resources.some(resource => ['all', 'agent_configurations'].includes(resource.kind))) {
        const details = { action: 'refresh', source: 'catalog', reason: event.type === 'ready' ? 'ready' : 'invalidation', rescan: false }
        const startedAt = Date.now()
        void refreshJobs()
        void refreshConfigs().then(() => {
          const summary = cacheSummary()
          const key = JSON.stringify(summary)
          // Routine invalidations can be frequent. Report changed cache state or
          // a slow read, without producing two events for every cheap snapshot.
          if (active && (key !== lastRefreshSummary || Date.now() - startedAt >= 1000)) {
            recordClientDiagnostic({ activity: 'agent_catalog_refresh', outcome: 'ok', durationMs: Date.now() - startedAt, details: { ...details, ...summary } })
          }
          lastRefreshSummary = key
        }, error => { recordClientDiagnostic({ activity: 'agent_catalog_refresh', outcome: 'failed', durationMs: Date.now() - startedAt,
          details: { ...details, error_category: diagnosticErrorCategory(error) } }); failure(error) })
      }
    }, failure)
    void refresh('mount').then(() => {
      if (backgroundPreparationError && preparations.error === backgroundPreparationError && !get(state).error) patch({ error: backgroundPreparationError })
    })
    return dispose
  }
  function isAgentConfigInUse(error: unknown): boolean {
    return typeof error === 'object' && error !== null && 'code' in error && error.code === 'AGENT_CONFIG_IN_USE'
  }
  async function resetAndDetect(): Promise<void> {
    if (!active) return
    if (detecting) await detecting
    if (!active) return
    const finish = startClientDiagnostic('agent_config', { action: 'delete', source: 'catalog', reason: 'manual' })
    try {
      patch({ error: '' })
      resetAgentDetectionCache(transport)
      for (const config of [...get(state).configs]) {
        try {
          await remove(config.id)
        } catch (error) {
          if (!isAgentConfigInUse(error)) throw error
          await save({
            id: config.id, catalog_id: config.catalog_id, name: config.name, host_id: config.host_id,
            protocol: config.protocol, enabled: false, command: config.command, args: config.args, env: config.env,
          })
          forgetAgentConnection(transport, config.id)
        }
      }
      finish('ok', { config_count: get(state).configs.length })
    } catch (error) {
      finish('failed', { error_category: diagnosticErrorCategory(error) })
      failure(error)
      return
    }
    await detectAll('manual')
  }
  function dispose() {
    active = false; clearTimeout(timer); unsubscribe?.(); unsubscribeCache?.(); unsubscribePreparations?.()
    for (const finish of installationDiagnostics.values()) finish('cancelled', { reason: 'inactive' })
    installationDiagnostics.clear()
  }
  return { subscribe: state.subscribe, start, dispose, refresh, detectAll, resetAndDetect, inspect, inspectAll, install, connect, cancel, save, remove, resolve, check, checkAgent }
}
