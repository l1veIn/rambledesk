import { get, writable } from 'svelte/store'
import type { ApplicationTransport } from '$lib/application/applicationTransport'
import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import { diagnosticAgentId, diagnosticErrorCategory, recordClientDiagnostic, startClientDiagnostic } from '$lib/diagnostics/clientDiagnostics'
import { readApplicationSnapshot } from '$lib/application/readApplicationSnapshot'
import { applicationResourcesAffectAgentConfigurations, applicationResourcesAffectManagedSession } from '$lib/application/applicationSnapshotRefetch'
import type { AgentCatalogEntry, AgentConfig, AgentFailure, AgentInspection, AgentInstallJob, ManagedSessionSnapshot, SessionConfigChange } from '$lib/generated/feedback'
import { isAbsoluteAgentDirectory, redactAgentMessage } from './agentConfigForm'
import { connectionPreparationAvailable } from './agentCatalogController'
import { beginAgentInspection, observeAgentRuntime, readAgentDetectionCache } from './agentDetectionCache'
import { agentFailureFrom } from './agentFailure'
import { promptRejectionConfirmed } from './promptAdmission'
import { sessionPromptDrafts } from './managedSessionUi'
import type { ManagedSessionDraftStorage } from './managedSessionDrafts'

export type DraftAgentChoice = Readonly<{
  key: string; name: string; hostId: string; config?: AgentConfig; catalogId?: string
  entry?: AgentCatalogEntry; inspection?: AgentInspection; advanced?: boolean
}>
export function agentNeedsPreparation(choice: DraftAgentChoice | undefined): boolean {
  return Boolean(choice?.entry && !choice.config && (!choice.inspection?.command
    || choice.inspection.checks.some(check => check.status === 'fail')
    || choice.entry.verification.status === 'unsupported'))
}
export function canPrepareAgentConnection(choice: DraftAgentChoice | undefined): boolean {
  return Boolean(agentNeedsPreparation(choice) && choice?.entry?.verification.status !== 'unsupported'
    && connectionPreparationAvailable(choice?.entry, choice?.inspection))
}
export function draftAgentChoices(configs: readonly AgentConfig[], catalog: readonly AgentCatalogEntry[], inspections: readonly AgentInspection[]): DraftAgentChoice[] {
  const profiles = configs.map((config) => ({ key: `config:${config.id}`, name: config.name, hostId: config.host_id, config,
    catalogId: config.catalog_id, entry: catalog.find(entry => entry.id === config.catalog_id),
    inspection: inspections.find(inspection => inspection.agent_id === config.catalog_id),
    advanced: Boolean(config.catalog_id && (configs.find(item => item.catalog_id === config.catalog_id && item.enabled)
      ?? configs.find(item => item.catalog_id === config.catalog_id))?.id !== config.id),
  }))
  const installed = catalog.filter((entry) => !configs.some((config) => config.catalog_id === entry.id)
    && inspections.some((inspection) => inspection.agent_id === entry.id))
    .map((entry) => ({ key: `catalog:${entry.id}`, name: entry.name, hostId: entry.host_id, catalogId: entry.id,
      entry, inspection: inspections.find(inspection => inspection.agent_id === entry.id) }))
  return [...profiles, ...installed]
}

export type DraftManagedSessionState = Readonly<{
  choice: string; cwd: string; text: string
  choices: readonly DraftAgentChoice[]; loadingChoices: boolean; choicesError: string
  awaitingAcknowledgement: boolean
  phase: 'idle' | 'preparing' | 'ready' | 'failed' | 'sending' | 'closing' | 'promoted'
  snapshot: ManagedSessionSnapshot | null; error: string; failure?: AgentFailure | null
  preparingConnection: boolean; installationJob: AgentInstallJob | null
}>

export function createDraftManagedSessionController(
  transport: ApplicationTransport,
  draftId: string,
  storage: ManagedSessionDraftStorage,
  onPromoted: (snapshot: ManagedSessionSnapshot) => Promise<void> | void,
) {
  const initial = storage.load(draftId)
  const state = writable<DraftManagedSessionState>({ ...initial, choices: [], loadingChoices: true, choicesError: '', awaitingAcknowledgement: false, phase: 'idle', snapshot: null, error: '', failure: null, preparingConnection: false, installationJob: null })
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
  let preparationTimer: ReturnType<typeof setTimeout> | null = null
  let choicesTask: Promise<void> | null = null
  let scanningChoices = false
  let lastChoicesDiagnosticSummary = ''
  let catalogEntries: AgentCatalogEntry[] = []
  const inspections = new Map<string, AgentInspection>(Object.entries(readAgentDetectionCache(transport).inspections))
  let installationTask: Promise<void> | null = null

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
    if (!choice || agentNeedsPreparation(choice)
      || value.preparingConnection) { patch({ phase: 'idle' }); return }
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
        const previous = choice.config
        const catalogId = previous?.catalog_id ?? choice.catalogId
        // Selecting an agent is the launch intent. Legacy disabled profiles are
        // enabled in this same step while all customized launch fields survive.
        const config = catalogId
          ? await transport.call('resolveCatalogAgent', { agent_id: catalogId, ...(previous ? { agent_config_id: previous.id } : {}), enable: true })
          : previous!.enabled ? previous! : await transport.call('saveAgentConfig', {
            id: previous!.id, name: previous!.name, host_id: previous!.host_id, protocol: previous!.protocol,
            command: previous!.command, args: previous!.args, env: previous!.env, enabled: true,
          })
        if (!current(intent)) { finish('cancelled', { reason: 'stale' }); return }
        if (config !== previous) {
          const resolved = { ...choice, key: `config:${config.id}`, name: config.name, hostId: config.host_id, config }
          choicesRevision += 1
          patch({ choice: resolved.key, choices: [...get(state).choices.filter((item) => item.key !== choice.key && item.key !== resolved.key), resolved], loadingChoices: false })
          persist()
        }
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
    const choices = draftAgentChoices(configs, catalog, [...inspections.values()])
    const value = get(state)
    const previousChoice = value.choices.find((item) => item.key === value.choice)
    const previousProfile = previousChoice?.config
    const selectedProfile = choices.find((item) => item.key === value.choice)?.config
    const selectionChanged = previousProfile && (!selectedProfile || selectedProfile.updated_at !== previousProfile.updated_at)
    if (selectionChanged && !sent) {
      revision += 1
      readRevision += 1
      patch({ snapshot: null, phase: 'idle', error: '', failure: null })
    }
    // Discovery never selects or materializes an installed catalog entry.
    const materialized = previousChoice?.catalogId && !previousProfile && !choices.some(item => item.key === value.choice)
      ? choices.find(item => item.catalogId === previousChoice.catalogId && !item.advanced)?.key : undefined
    const choice = materialized ?? (value.choice || choices.find((item) => item.config?.enabled)?.key || '')
    patch({ choices, choice, loadingChoices: false })
    persist()
    if ((!prepared || selectionChanged) && !sent) void schedule()
  }
  function refreshChoices(rescan = false, reason: 'mount' | 'refresh' | 'ready' | 'invalidation' | 'manual' | 'post_install' = rescan ? 'manual' : 'refresh'): Promise<void> {
    if (closed || closing || promoted) return Promise.resolve()
    if (choicesTask) return rescan && !scanningChoices ? choicesTask.then(() => refreshChoices(true, reason)) : choicesTask
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
        const configs = await transport.call('listAgentConfigs', undefined)
        if (intent !== choicesRevision || closed || closing || promoted) return
        inspections.clear()
        for (const [id, inspection] of Object.entries(readAgentDetectionCache(transport).inspections)) inspections.set(id, inspection)
        // Configured agents connect immediately, independent of slow version probes.
        publishChoices(configs, catalogEntries)
        const catalog = await transport.call('listAvailableAgents', undefined)
        if (closed || closing || promoted) return
        catalogEntries = catalog
        publishChoices(get(state).choices.flatMap(choice => choice.config ? [choice.config] : []), catalog)
        if (!rescan) { successful = true; return }
        let next = 0
        let failed = false
        async function inspectNext() {
          while (!closed && !closing && !promoted && next < catalog.length) {
            const entry = catalog[next++]
            const rememberInspection = beginAgentInspection(transport, entry.id)
            try {
              const inspection = await transport.call('inspectAgentInstallation', { agent_id: entry.id })
              checkedCount += 1
              if (!rememberInspection(inspection)) return
              if (closed || closing || promoted) return
              inspections.set(entry.id, inspection)
            } catch {
              failedCount += 1
              if (closed || closing || promoted) return
              inspections.delete(entry.id)
              failed = true
            }
            // Resolution and user selection can change during a probe. Retain the
            // current saved profiles instead of restoring the scan's older list.
            publishChoices(get(state).choices.flatMap(choice => choice.config ? [choice.config] : []), catalog)
            patch({ choicesError: failed ? 'Some installed agents could not be checked.' : '' })
          }
        }
        await Promise.all(Array.from({ length: Math.min(3, catalog.length) }, inspectNext))
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
      }
    })()
    return choicesTask
  }
  function start() {
    if (closed || promoted) return
    if (started) { void refreshChoices(false); return }
    started = true
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
    if (closed || closing || promoted || sent || get(state).preparingConnection) return
    const value = get(state)
    if (choice === value.choice && cwd === value.cwd) return
    revision += 1
    readRevision += 1
    patch({ choice, cwd, snapshot: null, phase: 'idle', error: '', failure: null })
    persist()
    if (preparationTimer) clearTimeout(preparationTimer)
    if (delayMs > 0) preparationTimer = setTimeout(() => { preparationTimer = null; void schedule() }, delayMs)
    else void schedule()
  }
  function edit(text: string) { if (!closed && !closing && !promoted) { draftEditRevision += 1; patch({ text }); persist() } }

  function prepareConnection(): Promise<void> {
    if (installationTask) return installationTask
    const choice = get(state).choices.find(choice => choice.key === get(state).choice)
    if (closed || closing || promoted || sent || !canPrepareAgentConnection(choice)) return Promise.resolve()
    const agentId = choice!.entry!.id
    const finish = startClientDiagnostic('agent_install', { action: 'install', source: 'draft', agent_kind: 'bridge', agent: diagnosticAgentId(agentId) })
    patch({ preparingConnection: true, installationJob: null, error: '' })
    installationTask = (async () => {
      try {
        let job = await transport.call('installAgent', { agent_id: agentId, version: null })
        while (!closed && !closing && !promoted) {
          patch({ installationJob: job })
          if (job.phase === 'complete') {
            finish('ok', { status: 'complete' })
            inspections.delete(agentId)
            // Explicit preparation checks only the Agent whose components changed.
            if (choicesTask) await choicesTask
            if (closed || closing || promoted) return
            const finishRecheck = startClientDiagnostic('agent_install', { action: 'recheck', source: 'draft', reason: 'post_install', target_count: 1, agent: diagnosticAgentId(agentId) })
            const rememberInspection = beginAgentInspection(transport, agentId)
            let inspection: AgentInspection
            try {
              inspection = await transport.call('inspectAgentInstallation', { agent_id: agentId })
              const accepted = rememberInspection(inspection)
              finishRecheck(accepted ? 'ok' : 'cancelled', { checked_count: 1, status: inspection.source })
              if (!accepted) return
            } catch (cause) { finishRecheck('failed', { error_category: diagnosticErrorCategory(cause) }); throw cause }
            if (closed || closing || promoted) return
            await refreshChoices(false, 'post_install')
            if (!closed && !closing && !promoted) {
              patch({ preparingConnection: false })
              await schedule()
            }
            return
          }
          if (job.phase === 'failed' || job.phase === 'cancelled') {
            finish(job.phase === 'cancelled' ? 'cancelled' : 'failed', { status: job.phase })
            throw new Error(job.phase === 'cancelled' ? 'Connection preparation was cancelled.' : 'Could not prepare the connection. See the installation details and retry.')
          }
          await new Promise(resolve => setTimeout(resolve, 500))
          if (closed || closing || promoted) return
          const jobs = await transport.call('listAgentInstallJobs', undefined)
          const updated = jobs.find(item => item.id === job.id)
          if (!updated) throw new Error('Connection preparation status is unavailable. Check Agents and retry.')
          job = updated
        }
      } catch (cause) {
        finish('failed', { error_category: diagnosticErrorCategory(cause) })
        if (!closed && !closing && !promoted) patch({ error: message(cause) })
      } finally {
        finish('cancelled', { reason: 'inactive' })
        installationTask = null
        // Closing can fail while discarding a previous prepared session. Release
        // the installation UI lock even then so that the retained draft can retry.
        if (!closed && !promoted) patch({ preparingConnection: false })
      }
    })()
    return installationTask
  }

  async function cancelPreparation() {
    const job = get(state).installationJob
    if (!job || !get(state).preparingConnection || job.cancel_requested) return
    const finish = startClientDiagnostic('agent_install', { action: 'cancel', source: 'draft', agent: diagnosticAgentId(job.agent_id) })
    try {
      await transport.call('cancelAgentInstall', { job_id: job.id })
      finish('ok')
      if (get(state).installationJob?.id === job.id) patch({ installationJob: { ...job, cancel_requested: true } })
    } catch (cause) { finish('failed', { error_category: diagnosticErrorCategory(cause) }); if (!closed && !closing && !promoted) patch({ error: message(cause) }) }
  }

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
      finish('ok', { promoted })
      return promoted ? prepared?.session.session_id ?? null : null
    } catch (cause) {
      finish('failed', { error_category: diagnosticErrorCategory(cause) })
      closing = false
      patch({ phase: 'failed', error: message(cause) })
      throw cause
    }
  }
  return { subscribe: state.subscribe, start, select, edit, send, configure, close, refreshChoices, prepareConnection, cancelPreparation,
    retry: () => schedule(true),
  }
}

export type DraftManagedSessionController = ReturnType<typeof createDraftManagedSessionController>
