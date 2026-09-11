import { get, writable } from 'svelte/store'
import type { ApplicationTransport } from '$lib/application/applicationTransport'
import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import { diagnosticAgentId, diagnosticErrorCategory, recordClientDiagnostic, startClientDiagnostic } from '$lib/diagnostics/clientDiagnostics'
import { readApplicationSnapshot } from '$lib/application/readApplicationSnapshot'
import { applicationResourcesAffectAgentConfigurations, applicationResourcesAffectManagedSession } from '$lib/application/applicationSnapshotRefetch'
import type { AgentCatalogEntry, AgentConfig, AgentFailure, AgentInspection, ManagedSessionSnapshot, SessionConfigChange } from '$lib/generated/feedback'
import { isAbsoluteAgentDirectory, redactAgentMessage } from './agentConfigForm'
import { agentListItems, createAgentCatalogController } from './agentCatalogController'
import { agentLaunchSignature, observeAgentRuntime, readAgentDetectionCache, reconcileAgentConnections, subscribeAgentDetectionCache, type CachedAgentConnection } from './agentDetectionCache'
import { agentFailureFrom } from './agentFailure'
import { promptRejectionConfirmed } from './promptAdmission'
import { sessionPromptDrafts } from './managedSessionUi'
import type { ManagedSessionDraftStorage } from './managedSessionDrafts'

export type DraftAgentChoice = Readonly<{
  key: string; name: string; hostId: string; config?: AgentConfig; catalogId?: string
  entry?: AgentCatalogEntry; inspection?: AgentInspection; profiles: readonly AgentConfig[]
}>
export function draftAgentChoices(configs: readonly AgentConfig[], catalog: readonly AgentCatalogEntry[], inspections: readonly AgentInspection[], connections: Record<string, CachedAgentConnection> = {}, selectedChoice = ''): DraftAgentChoice[] {
  const checked = configs.filter(config => config.enabled && connections[config.id]?.result.ok
    && connections[config.id].signature === agentLaunchSignature(config))
  // Use the same agent identities and names as Settings. Historical aliases and
  // additional launch profiles belong to that agent's advanced settings.
  return agentListItems(catalog, checked, connections)
    .filter(row => row.config && (row.entry || !row.config.catalog_id))
    .map(row => {
      // A restored draft keeps its explicitly selected account/launch profile.
      const config = row.configs.find(config => `config:${config.id}` === selectedChoice) ?? row.config!
      return { key: `config:${config.id}`, name: row.name, hostId: config.host_id, config,
        catalogId: config.catalog_id, entry: row.entry, profiles: row.configs,
        inspection: inspections.find(inspection => inspection.agent_id === config.catalog_id) }
    })
}

export type DraftManagedSessionState = Readonly<{
  choice: string; cwd: string; text: string
  choices: readonly DraftAgentChoice[]; loadingChoices: boolean; choicesError: string
  awaitingAcknowledgement: boolean
  phase: 'idle' | 'preparing' | 'ready' | 'failed' | 'sending' | 'closing' | 'promoted'
  snapshot: ManagedSessionSnapshot | null; error: string; failure?: AgentFailure | null
}>

export function createDraftManagedSessionController(
  transport: ApplicationTransport,
  draftId: string,
  storage: ManagedSessionDraftStorage,
  onPromoted: (snapshot: ManagedSessionSnapshot) => Promise<void> | void,
) {
  const initial = storage.load(draftId)
  const state = writable<DraftManagedSessionState>({ ...initial, choices: [], loadingChoices: true, choicesError: '', awaitingAcknowledgement: false, phase: 'idle', snapshot: null, error: '', failure: null })
  let started = false
  let closed = false
  let closing = false
  let promoted = false
  let revision = 0
  let choicesRevision = 0
  let readRevision = 0
  let draftEditRevision = 0
  let prepared: ManagedSessionSnapshot | null = null
  let preparedRevision = -1
  let preparedEnvironment = ''
  let operations: Promise<void> = Promise.resolve()
  let unsubscribe: (() => void) | null = null
  let unsubscribeCache: (() => void) | null = null
  let preparationTimer: ReturnType<typeof setTimeout> | null = null
  let choicesTask: Promise<void> | null = null
  let refreshAfterChoices = false
  let scanningChoices = false
  let lastChoicesDiagnosticSummary = ''
  let catalogEntries: AgentCatalogEntry[] = []
  let catalogLoaded = false
  let savedConfigs: AgentConfig[] = []
  const inspections = new Map<string, AgentInspection>(Object.entries(readAgentDetectionCache(transport).inspections))

  function patch(next: Partial<DraftManagedSessionState>) { state.update((value) => ({ ...value, ...next })) }
  function persist() { const { choice, cwd, text } = get(state); storage.save(draftId, { choice, cwd, text }) }
  function environmentText() {
    return get(state).choices.flatMap(choice => Object.entries(choice.config?.env ?? {}).map(([key, value]) => key + '=' + value)).join('\n')
  }
  function message(cause: unknown, env = environmentText()) {
    const text = cause instanceof Error ? cause.message : typeof cause === 'object' && cause && 'message' in cause ? String(cause.message) : 'Could not connect to the agent.'
    return redactAgentMessage(text, env)
  }
  function safeFailure(cause: unknown, stage: AgentFailure['stage'], env = environmentText()): AgentFailure {
    return agentFailureFrom(cause, stage, env)
  }
  function safeSnapshot(snapshot: ManagedSessionSnapshot, env: string): ManagedSessionSnapshot {
    return { ...snapshot, runtime: { ...snapshot.runtime,
      last_error: snapshot.runtime.last_error ? redactAgentMessage(snapshot.runtime.last_error, env) : null,
      ...(snapshot.runtime.failure ? { failure: safeFailure(snapshot.runtime.failure, snapshot.runtime.failure.stage, env) } : {}),
    } }
  }
  function enqueue(operation: () => Promise<void>) {
    const pending = operations.then(operation)
    operations = pending.catch(() => {})
    return pending
  }
  function current(intent: number) { return !closed && !closing && !promoted && intent === revision }
  function ready(snapshot: ManagedSessionSnapshot) { return snapshot.runtime.connection === 'connected' }
  function directoryError(cwd: string) {
    return !cwd.trim() ? 'Choose a project directory before connecting.'
      : !isAbsoluteAgentDirectory(cwd.trim()) ? 'Enter an absolute project directory.' : ''
  }
  function selectedDiagnosticAgent() {
    const value = get(state)
    const choice = value.choices.find(item => item.key === value.choice)
    return diagnosticAgentId(choice?.config?.catalog_id ?? choice?.catalogId)
  }
  function assertSession(snapshot: ManagedSessionSnapshot, id: string) {
    if (snapshot.session.session_id !== id) throw new Error('The agent returned an invalid session snapshot.')
  }

  async function acceptPromotion(snapshot: ManagedSessionSnapshot) {
    if (promoted || snapshot.session.lifecycle === 'prepared') return
    const finish = startClientDiagnostic('session_draft', { action: 'promote', source: 'draft', agent: selectedDiagnosticAgent() })
    try {
      promoted = true
      prepared = snapshot
      const latest = get(state)
      const remainingText = latest.text
      sessionPromptDrafts.write(snapshot.session.session_id, remainingText)
      patch({ phase: 'promoted', snapshot, text: remainingText, awaitingAcknowledgement: false, error: '' })
      storage.remove(draftId)
      unsubscribe?.(); unsubscribe = null
      unsubscribeCache?.(); unsubscribeCache = null
      await onPromoted(snapshot)
      finish('ok')
    } catch (cause) { finish('failed', { error_category: diagnosticErrorCategory(cause) }); throw cause }
  }

  let sent: { text: string; editRevision: number; rejected: boolean } | null = null
  function restoreRejectedSubmission() {
    if (!sent) return
    if (draftEditRevision === sent.editRevision) { patch({ text: sent.text }); persist() }
    sent = null
    patch({ awaitingAcknowledgement: false })
  }
  let sendPending = false
  async function refreshPrepared(): Promise<boolean> {
    const target = prepared
    if (!target || closed || promoted || preparedRevision !== revision) return false
    const intent = revision
    const read = ++readRevision
    try {
      const snapshot = safeSnapshot(await readApplicationSnapshot(transport, 'getManagedSession', { session_id: target.session.session_id }), preparedEnvironment)
      assertSession(snapshot, target.session.session_id)
      if (read !== readRevision || prepared?.session.session_id !== target.session.session_id || revision !== intent || closed || promoted) return false
      prepared = snapshot
      if (sent && snapshot.session.lifecycle !== 'prepared') await acceptPromotion(snapshot)
      else {
        if (sent && !sendPending && sent.rejected) restoreRejectedSubmission()
        if (!closing) patch({ snapshot, ...(sendPending ? {} : {
          phase: ready(snapshot) ? 'ready' : snapshot.runtime.connection === 'connecting' ? 'preparing' : 'failed',
          error: snapshot.runtime.last_error ? message(new Error(snapshot.runtime.last_error)) : '',
          failure: snapshot.runtime.failure ?? null,
        }) })
      }
      return true
    } catch (cause) { if (read === readRevision && current(intent)) patch({ error: message(cause) }); return false }
  }

  async function reconcile(intent: number, retry = false) {
    if (promoted) return
    if (sent) {
      if (!await refreshPrepared() && !promoted) throw new Error('Could not confirm whether the first message was accepted. Retry to check the session.')
      if (promoted) return
      if (sent) throw new Error('Could not confirm whether the first message was accepted. Retry to check the session.')
    }
    // A failed discard keeps ownership here. A later retry cannot silently orphan it.
    if (prepared && (closed || closing || preparedRevision !== revision)) {
      await transport.call('discardPreparedSession', { session_id: prepared.session.session_id })
      prepared = null
      patch({ snapshot: null })
    }
    if (!current(intent)) return
    const value = get(state)
    const choice = value.choices.find((item) => item.key === value.choice)
    const invalidDirectory = directoryError(value.cwd)
    if (invalidDirectory) { patch({ phase: 'idle', ...(retry ? { error: invalidDirectory } : {}) }); return }
    if (!choice) { patch({ phase: 'idle' }); return }
    if (prepared && !retry) return
    patch({ phase: 'preparing', error: '', failure: null })
    let operationEnvironment = prepared ? preparedEnvironment : environmentText()
    const finish = startClientDiagnostic('session_draft', { action: 'prepare', source: 'draft', retry, agent: diagnosticAgentId(choice.config?.catalog_id ?? choice.catalogId),
      agent_kind: choice.entry?.connection_kind === 'native' ? 'native' : choice.entry?.connection_kind === 'bridge' ? 'bridge' : 'custom' })
    try {
      let snapshot: ManagedSessionSnapshot
      if (prepared) {
        snapshot = await transport.call('startManagedSession', { session_id: prepared.session.session_id })
        assertSession(snapshot, prepared.session.session_id)
      } else {
        const config = choice.config!
        operationEnvironment += '\n' + Object.entries(config.env).map(([key, value]) => key + '=' + value).join('\n')
        snapshot = await transport.call('prepareManagedSession', { agent_config_id: config.id, cwd: value.cwd.trim() })
      }
      snapshot = safeSnapshot(snapshot, operationEnvironment)
      prepared = snapshot
      preparedEnvironment = operationEnvironment
      preparedRevision = intent
      if (!current(intent)) {
        await transport.call('discardPreparedSession', { session_id: snapshot.session.session_id })
        prepared = null
        finish('cancelled', { reason: 'stale' })
        return
      }
      patch({ snapshot, phase: ready(snapshot) ? 'ready' : 'failed', error: snapshot.runtime.last_error ? message(new Error(snapshot.runtime.last_error)) : '', failure: snapshot.runtime.failure ?? null })
      finish(ready(snapshot) ? 'ok' : 'failed', { status: snapshot.runtime.connection })
    } catch (cause) {
      finish(current(intent) ? 'failed' : 'cancelled', { error_category: diagnosticErrorCategory(cause) })
      if (current(intent)) patch({ phase: 'failed', error: message(cause, operationEnvironment), failure: safeFailure(cause, 'session', operationEnvironment) })
      throw cause
    }
  }
  function schedule(retry = false) {
    const intent = revision
    return enqueue(() => reconcile(intent, retry)).catch((cause) => {
      if (!closed && !promoted) patch({ phase: closing ? 'closing' : 'failed', error: message(cause, preparedEnvironment + '\n' + environmentText()) })
    })
  }

  function publishChoices(configs: readonly AgentConfig[], catalog: readonly AgentCatalogEntry[]) {
    const value = get(state)
    const choices = draftAgentChoices(configs, catalog, [...inspections.values()], readAgentDetectionCache(transport).connections, value.choice)
    const previousChoice = value.choices.find((item) => item.key === value.choice)
    const previousProfile = previousChoice?.config
    const selectedProfile = choices.find((item) => item.key === value.choice)?.config
    const selectionChanged = previousProfile && (!selectedProfile || selectedProfile.updated_at !== previousProfile.updated_at)
    if (selectionChanged && !sent) {
      revision += 1
      readRevision += 1
      patch({ snapshot: null, phase: 'idle', error: '', failure: null })
    }
    const choice = !catalogLoaded || choices.some(item => item.key === value.choice) ? value.choice : ''
    patch({ choices, choice, loadingChoices: false })
    persist()
    if ((!prepared || selectionChanged) && !sent) void schedule()
  }
  function refreshChoices(rescan = false, reason: 'mount' | 'refresh' | 'ready' | 'invalidation' | 'manual' | 'post_install' = rescan ? 'manual' : 'refresh'): Promise<void> {
    if (closed || closing || promoted) return Promise.resolve()
    if (choicesTask) {
      if (reason === 'ready' || reason === 'invalidation') { refreshAfterChoices = true; choicesRevision += 1 }
      return rescan && !scanningChoices ? choicesTask.then(() => refreshChoices(true, reason)) : choicesTask
    }
    scanningChoices = rescan
    const diagnosticActivity = rescan ? 'agent_detection' : 'agent_catalog_refresh'
    const diagnosticDetails = { action: rescan ? 'scan' : 'refresh', source: 'draft', reason, rescan }
    const background = !rescan && (reason === 'ready' || reason === 'invalidation')
    const startedAt = Date.now()
    const finish = background ? undefined : startClientDiagnostic(diagnosticActivity, diagnosticDetails)
    let checkedCount = 0
    let failedCount = 0
    let successful = false
    let failureCategory: string | undefined
    const intent = ++choicesRevision
    patch({ loadingChoices: true, choicesError: '' })
    choicesTask = (async () => {
      try {
        await transport.waitUntilReady()
        if (closed || closing || promoted) return
        if (rescan) {
          // Use the same explicit discovery + ACP checks as Agents settings.
          const detector = createAgentCatalogController(transport)
          const dispose = detector.start()
          try {
            await detector.detectAll()
            const detected = get(detector)
            checkedCount = Object.keys(detected.connections).length
            failedCount = Object.values(detected.connections).filter(check => !check.result.ok).length
            if (detected.error) patch({ choicesError: detected.error })
          } finally { dispose() }
          if (closed || closing || promoted) return
        }
        const configs = await transport.call('listAgentConfigs', undefined)
        if (intent !== choicesRevision || closed || closing || promoted) return
        savedConfigs = configs
        reconcileAgentConnections(transport, configs)
        inspections.clear()
        for (const [id, inspection] of Object.entries(readAgentDetectionCache(transport).inspections)) inspections.set(id, inspection)
        // Previously checked profiles are available without launching new probes.
        publishChoices(configs, catalogEntries)
        const catalog = await transport.call('listAvailableAgents', undefined)
        if (intent !== choicesRevision || closed || closing || promoted) return
        catalogEntries = catalog
        catalogLoaded = true
        publishChoices(savedConfigs, catalog)
        successful = true
      } catch (cause) {
        failureCategory = diagnosticErrorCategory(cause)
        if (!closed && !closing && !promoted) patch({ loadingChoices: false, choicesError: message(cause) })
      } finally {
        const outcome = failureCategory ? 'failed' : !successful || closed || closing || promoted ? 'cancelled' : failedCount ? 'failed' : 'ok'
        const summary = { checked_count: checkedCount, failed_count: failedCount,
          entry_count: catalogEntries.length, config_count: get(state).choices.filter(choice => choice.config).length,
          cache_count: inspections.size, cache_hit: !rescan && inspections.size > 0 }
        const summaryKey = JSON.stringify(summary)
        const finalDetails = { ...summary, ...(failureCategory ? { error_category: failureCategory } : {}) }
        if (finish) finish(outcome, finalDetails)
        else if (failureCategory || summaryKey !== lastChoicesDiagnosticSummary || Date.now() - startedAt >= 1000) {
          recordClientDiagnostic({ activity: diagnosticActivity, outcome, durationMs: Date.now() - startedAt, details: { ...diagnosticDetails, ...finalDetails } })
        }
        lastChoicesDiagnosticSummary = summaryKey
        choicesTask = null; scanningChoices = false
        if (refreshAfterChoices) { refreshAfterChoices = false; void refreshChoices(false, 'invalidation') }
      }
    })()
    return choicesTask
  }
  function start() {
    if (closed || promoted) return
    if (started) { void refreshChoices(false); return }
    started = true
    unsubscribeCache = subscribeAgentDetectionCache(transport, snapshot => {
      inspections.clear()
      for (const [id, inspection] of Object.entries(snapshot.inspections)) inspections.set(id, inspection)
      if (savedConfigs.length && !closed && !closing && !promoted) publishChoices(savedConfigs, catalogEntries)
    })
    unsubscribe = transport.subscribe(APPLICATION_EVENTS_STREAM, (event) => {
      observeAgentRuntime(transport, event.runtime_generation)
      if (event.type === 'ready') { void refreshChoices(false, 'ready'); if (prepared) void refreshPrepared(); return }
      // Only fetch saved records and cached detection results on invalidation.
      if (applicationResourcesAffectAgentConfigurations(event.resources)) void refreshChoices(false, 'invalidation')
      if (prepared && applicationResourcesAffectManagedSession(event.resources, prepared.session.session_id)) void refreshPrepared()
    }, (cause) => { if (!closed && !promoted) patch({ error: message(cause) }) })
    void refreshChoices(false, 'mount')
  }
  function select(choice: string, cwd: string, delayMs = 0) {
    if (closed || closing || promoted || sent) return
    const value = get(state)
    if (choice === value.choice && cwd === value.cwd) return
    const choices = started ? draftAgentChoices(savedConfigs, catalogEntries, [...inspections.values()], readAgentDetectionCache(transport).connections, choice) : value.choices
    if (started && choice && choice !== value.choice && !choices.some(item => item.key === choice)) return
    revision += 1
    readRevision += 1
    patch({ choice, choices, cwd, snapshot: null, phase: 'idle', error: '', failure: null })
    persist()
    if (preparationTimer) clearTimeout(preparationTimer)
    if (delayMs > 0) preparationTimer = setTimeout(() => { preparationTimer = null; void schedule() }, delayMs)
    else void schedule()
  }
  function edit(text: string) { if (!closed && !closing && !promoted) { draftEditRevision += 1; patch({ text }); persist() } }

  async function send(text: string) {
    const value = get(state)
    if (!closed && !closing && !promoted && !sent && directoryError(value.cwd)) {
      patch({ error: directoryError(value.cwd) })
      recordClientDiagnostic({ activity: 'session_draft', outcome: 'blocked', details: { action: 'send', source: 'draft', reason: 'not_ready', agent: selectedDiagnosticAgent() } })
      return
    }
    if (closed || closing || promoted || sent || value.phase !== 'ready' || !prepared) {
      recordClientDiagnostic({ activity: 'session_draft', outcome: 'blocked', details: { action: 'send', source: 'draft', reason: 'not_ready', agent: selectedDiagnosticAgent() } })
      return
    }
    const trimmed = text.trim()
    if (!trimmed) return
    const finish = startClientDiagnostic('session_draft', { action: 'send', source: 'draft', agent: selectedDiagnosticAgent() })
    const target = prepared.session.session_id
    const intent = revision
    sent = { text, editRevision: draftEditRevision, rejected: false }
    sendPending = true
    // The submission stays recoverable until accepted; the composer is available
    // for the next draft while the first message is being accepted.
    patch({ phase: 'sending', text: '', awaitingAcknowledgement: true, error: '' })
    await enqueue(async () => {
      try {
        const snapshot = safeSnapshot(await transport.call('sendManagedPrompt', { session_id: target, text: trimmed }), preparedEnvironment)
        assertSession(snapshot, target)
        await acceptPromotion(snapshot)
        if (!promoted && current(intent)) { prepared = snapshot; restoreRejectedSubmission(); patch({ snapshot, phase: ready(snapshot) ? 'ready' : 'failed' }) }
        finish(promoted ? 'ok' : 'failed', { promoted })
      } catch (cause) {
        // A lost acknowledgement may still have persisted the real user message.
        sendPending = false
        if (sent) sent.rejected = promptRejectionConfirmed(cause)
        if (sent?.rejected && !promoted) restoreRejectedSubmission()
        await refreshPrepared()
        finish(promoted ? 'ok' : 'failed', { promoted, reason: 'acknowledgement_lost', ...(!promoted ? { error_category: diagnosticErrorCategory(cause) } : {}) })
        if (!promoted && !closed) patch({ phase: closing ? 'closing' : 'failed', error: message(cause) })
        if (!promoted) throw cause
      } finally { sendPending = false }
    })
  }
  async function configure(change: SessionConfigChange) {
    const target = prepared
    if (!target || get(state).phase !== 'ready' || closing || promoted) return
    const intent = revision
    await enqueue(async () => {
      if (!current(intent)) return
      const finish = startClientDiagnostic('session_draft', { action: 'configure', source: 'draft', agent: selectedDiagnosticAgent() })
      try {
        await transport.call('setManagedSessionConfig', { session_id: target.session.session_id, change })
        finish('ok')
        if (current(intent)) await refreshPrepared()
      } catch (cause) {
        finish('failed', { error_category: diagnosticErrorCategory(cause) })
        if (current(intent)) {
          await refreshPrepared()
          patch({ error: message(cause, preparedEnvironment), failure: get(state).snapshot?.runtime.failure ?? safeFailure(cause, 'configuration', preparedEnvironment) })
        }
        throw cause
      }
    })
  }
  async function close(): Promise<string | null> {
    const finish = startClientDiagnostic('session_draft', { action: 'close', source: 'draft', has_prepared: Boolean(prepared), agent: selectedDiagnosticAgent() })
    if (preparationTimer) clearTimeout(preparationTimer)
    preparationTimer = null
    closing = true
    readRevision += 1
    patch({ phase: promoted ? 'promoted' : 'closing' })
    try {
      await enqueue(async () => {
        if (sent && !promoted) {
          await refreshPrepared()
          if (sent && !promoted) throw new Error('Could not confirm whether the first message was accepted. Retry to check the session.')
        }
        if (!promoted && prepared) {
          await transport.call('discardPreparedSession', { session_id: prepared.session.session_id })
          prepared = null
        }
      })
      closed = true
      choicesRevision += 1
      unsubscribe?.(); unsubscribe = null
      unsubscribeCache?.(); unsubscribeCache = null
      finish('ok', { promoted })
      return promoted ? prepared?.session.session_id ?? null : null
    } catch (cause) {
      finish('failed', { error_category: diagnosticErrorCategory(cause) })
      closing = false
      patch({ phase: 'failed', error: message(cause) })
      throw cause
    }
  }
  return { subscribe: state.subscribe, start, select, edit, send, configure, close, refreshChoices,
    retry: () => schedule(true),
  }
}

export type DraftManagedSessionController = ReturnType<typeof createDraftManagedSessionController>
