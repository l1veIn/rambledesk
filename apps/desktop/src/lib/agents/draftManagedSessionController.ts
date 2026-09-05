import { get, writable } from 'svelte/store'
import type { ApplicationTransport } from '$lib/application/applicationTransport'
import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import { readApplicationSnapshot } from '$lib/application/readApplicationSnapshot'
import { applicationResourcesAffectManagedSession } from '$lib/application/applicationSnapshotRefetch'
import type { AgentCatalogEntry, AgentConfig, AgentInspection, ManagedSessionSnapshot, SessionConfigChange } from '$lib/generated/feedback'
import { isAbsoluteAgentDirectory, redactAgentMessage } from './agentConfigForm'
import { sessionPromptDrafts } from './managedSessionUi'
import type { ManagedSessionDraftStorage } from './managedSessionDrafts'

export type DraftAgentChoice = Readonly<{ key: string; name: string; hostId: string; config?: AgentConfig; catalogId?: string }>
export function draftAgentChoices(configs: readonly AgentConfig[], catalog: readonly AgentCatalogEntry[], inspections: readonly AgentInspection[]): DraftAgentChoice[] {
  const profiles = configs.map((config) => ({ key: `config:${config.id}`, name: config.name, hostId: config.host_id, config }))
  const installed = catalog.filter((entry) => !configs.some((config) => config.catalog_id === entry.id)
    && inspections.some((inspection) => inspection.agent_id === entry.id && Boolean(inspection.command)))
    .map((entry) => ({ key: `catalog:${entry.id}`, name: entry.name, hostId: entry.host_id, catalogId: entry.id }))
  return [...profiles, ...installed]
}

export type DraftManagedSessionState = Readonly<{
  choice: string; cwd: string; text: string
  choices: readonly DraftAgentChoice[]; loadingChoices: boolean; choicesError: string
  awaitingAcknowledgement: boolean
  phase: 'idle' | 'preparing' | 'ready' | 'failed' | 'sending' | 'closing' | 'promoted'
  snapshot: ManagedSessionSnapshot | null; error: string
}>

export function createDraftManagedSessionController(
  transport: ApplicationTransport,
  draftId: string,
  storage: ManagedSessionDraftStorage,
  onPromoted: (snapshot: ManagedSessionSnapshot) => Promise<void> | void,
) {
  const initial = storage.load(draftId)
  const state = writable<DraftManagedSessionState>({ ...initial, choices: [], loadingChoices: true, choicesError: '', awaitingAcknowledgement: false, phase: 'idle', snapshot: null, error: '' })
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
  let operations: Promise<void> = Promise.resolve()
  let unsubscribe: (() => void) | null = null
  let preparationTimer: ReturnType<typeof setTimeout> | null = null
  let choicesTask: Promise<void> | null = null
  let inspectedCatalog = ''
  let catalogEntries: AgentCatalogEntry[] = []
  const inspections = new Map<string, AgentInspection>()

  function patch(next: Partial<DraftManagedSessionState>) { state.update((value) => ({ ...value, ...next })) }
  function persist() { const { choice, cwd, text } = get(state); storage.save(draftId, { choice, cwd, text }) }
  function message(cause: unknown) {
    const text = cause instanceof Error ? cause.message : typeof cause === 'object' && cause && 'message' in cause ? String(cause.message) : 'Could not connect to the agent.'
    const env = get(state).choices.flatMap((choice) => Object.entries(choice.config?.env ?? {}).map(([key, value]) => `${key}=${value}`)).join('\n')
    return redactAgentMessage(text, env)
  }
  function enqueue(operation: () => Promise<void>) {
    const pending = operations.then(operation)
    operations = pending.catch(() => {})
    return pending
  }
  function current(intent: number) { return !closed && !closing && !promoted && intent === revision }
  function ready(snapshot: ManagedSessionSnapshot) { return snapshot.runtime.connection === 'connected' }
  function assertSession(snapshot: ManagedSessionSnapshot, id: string) {
    if (snapshot.session.session_id !== id) throw new Error('The agent returned an invalid session snapshot.')
  }

  async function acceptPromotion(snapshot: ManagedSessionSnapshot) {
    if (promoted || snapshot.session.lifecycle === 'prepared') return
    promoted = true
    prepared = snapshot
    const latest = get(state)
    const remainingText = latest.text
    sessionPromptDrafts.write(snapshot.session.session_id, remainingText)
    patch({ phase: 'promoted', snapshot, text: remainingText, awaitingAcknowledgement: false, error: '' })
    storage.remove(draftId)
    unsubscribe?.(); unsubscribe = null
    await onPromoted(snapshot)
  }

  let sent: { text: string; editRevision: number } | null = null
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
      const snapshot = await readApplicationSnapshot(transport, 'getManagedSession', { session_id: target.session.session_id })
      assertSession(snapshot, target.session.session_id)
      if (read !== readRevision || prepared?.session.session_id !== target.session.session_id || revision !== intent || closed || promoted) return false
      prepared = snapshot
      if (sent && snapshot.session.lifecycle !== 'prepared') await acceptPromotion(snapshot)
      else {
        if (sent && !sendPending) restoreRejectedSubmission()
        if (!closing) patch({ snapshot, ...(sendPending ? {} : {
          phase: ready(snapshot) ? 'ready' : snapshot.runtime.connection === 'connecting' ? 'preparing' : 'failed',
          error: snapshot.runtime.last_error ? message(new Error(snapshot.runtime.last_error)) : '',
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
    if (!choice || !isAbsoluteAgentDirectory(value.cwd.trim())) { patch({ phase: 'idle' }); return }
    if (prepared && !retry) return
    patch({ phase: 'preparing', error: '' })
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
        if (!current(intent)) return
        if (config !== previous) {
          const resolved = { key: `config:${config.id}`, name: config.name, hostId: config.host_id, config }
          choicesRevision += 1
          patch({ choice: resolved.key, choices: [...get(state).choices.filter((item) => item.key !== choice.key && item.key !== resolved.key), resolved], loadingChoices: false })
          persist()
        }
        snapshot = await transport.call('prepareManagedSession', { agent_config_id: config.id, cwd: value.cwd.trim() })
      }
      prepared = snapshot
      preparedRevision = intent
      if (!current(intent)) {
        await transport.call('discardPreparedSession', { session_id: snapshot.session.session_id })
        prepared = null
        return
      }
      patch({ snapshot, phase: ready(snapshot) ? 'ready' : 'failed', error: snapshot.runtime.last_error ? message(new Error(snapshot.runtime.last_error)) : '' })
    } catch (cause) {
      if (current(intent)) patch({ phase: 'failed', error: message(cause) })
      throw cause
    }
  }
  function schedule(retry = false) {
    const intent = revision
    return enqueue(() => reconcile(intent, retry)).catch((cause) => {
      if (!closed && !promoted) patch({ phase: closing ? 'closing' : 'failed', error: message(cause) })
    })
  }

  function publishChoices(configs: readonly AgentConfig[], catalog: readonly AgentCatalogEntry[]) {
    const choices = draftAgentChoices(configs, catalog, [...inspections.values()])
    const value = get(state)
    const previousProfile = value.choices.find((item) => item.key === value.choice)?.config
    const selectedProfile = choices.find((item) => item.key === value.choice)?.config
    const selectionChanged = previousProfile && (!selectedProfile || selectedProfile.updated_at !== previousProfile.updated_at)
    if (selectionChanged && !sent) {
      revision += 1
      readRevision += 1
      patch({ snapshot: null, phase: 'idle', error: '' })
    }
    // Discovery never selects or materializes an installed catalog entry.
    const choice = value.choice || choices.find((item) => item.config?.enabled)?.key || ''
    patch({ choices, choice, loadingChoices: false })
    persist()
    if ((!prepared || selectionChanged) && !sent) void schedule()
  }
  function refreshChoices(rescan = true): Promise<void> {
    if (closed || closing || promoted) return Promise.resolve()
    if (choicesTask) return choicesTask
    const intent = ++choicesRevision
    patch({ loadingChoices: true, choicesError: '' })
    choicesTask = (async () => {
      try {
        await transport.waitUntilReady()
        if (closed || closing || promoted) return
        const configs = await transport.call('listAgentConfigs', undefined)
        if (intent !== choicesRevision || closed || closing || promoted) return
        // Configured agents connect immediately, independent of slow version probes.
        publishChoices(configs, catalogEntries)
        const catalog = await transport.call('listAvailableAgents', undefined)
        if (closed || closing || promoted) return
        catalogEntries = catalog
        publishChoices(get(state).choices.flatMap(choice => choice.config ? [choice.config] : []), catalog)
        const catalogKey = catalog.map(entry => entry.id).join('|')
        if (!rescan && inspectedCatalog === catalogKey) return
        let next = 0
        let failed = false
        async function inspectNext() {
          while (!closed && !closing && !promoted && next < catalog.length) {
            const entry = catalog[next++]
            try {
              const inspection = await transport.call('inspectAgentInstallation', { agent_id: entry.id })
              if (closed || closing || promoted) return
              inspections.set(entry.id, inspection)
            } catch {
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
        if (!closed && !closing && !promoted && !failed) inspectedCatalog = catalogKey
      } catch (cause) {
        if (!closed && !closing && !promoted) patch({ loadingChoices: false, choicesError: message(cause) })
      } finally { choicesTask = null }
    })()
    return choicesTask
  }
  function start() {
    if (closed || promoted) return
    if (started) { void refreshChoices(false); return }
    started = true
    unsubscribe = transport.subscribe(APPLICATION_EVENTS_STREAM, (event) => {
      if (event.type === 'ready') { if (prepared) void refreshPrepared(); return }
      // Installation inspection also emits this resource. Refresh saved profile state
      // on explicit UI activation, avoiding an inspect -> event -> inspect loop.
      if (prepared && applicationResourcesAffectManagedSession(event.resources, prepared.session.session_id)) void refreshPrepared()
    }, (cause) => { if (!closed && !promoted) patch({ error: message(cause) }) })
    void refreshChoices(false)
  }
  function select(choice: string, cwd: string, delayMs = 0) {
    if (closed || closing || promoted || sent) return
    const value = get(state)
    if (choice === value.choice && cwd === value.cwd) return
    revision += 1
    readRevision += 1
    patch({ choice, cwd, snapshot: null, phase: 'idle', error: '' })
    persist()
    if (preparationTimer) clearTimeout(preparationTimer)
    if (delayMs > 0) preparationTimer = setTimeout(() => { preparationTimer = null; void schedule() }, delayMs)
    else void schedule()
  }
  function edit(text: string) { if (!closed && !closing && !promoted) { draftEditRevision += 1; patch({ text }); persist() } }

  async function send(text: string) {
    const value = get(state)
    if (closed || closing || promoted || sent || value.phase !== 'ready' || !prepared) return
    const trimmed = text.trim()
    if (!trimmed) return
    const target = prepared.session.session_id
    const intent = revision
    sent = { text, editRevision: draftEditRevision }
    sendPending = true
    // The submission stays recoverable until accepted; the composer is available
    // for the next draft while the first message is being accepted.
    patch({ phase: 'sending', text: '', awaitingAcknowledgement: true, error: '' })
    await enqueue(async () => {
      try {
        const snapshot = await transport.call('sendManagedPrompt', { session_id: target, text: trimmed })
        assertSession(snapshot, target)
        await acceptPromotion(snapshot)
        if (!promoted && current(intent)) { prepared = snapshot; restoreRejectedSubmission(); patch({ snapshot, phase: ready(snapshot) ? 'ready' : 'failed' }) }
      } catch (cause) {
        // A lost acknowledgement may still have persisted the real user message.
        sendPending = false
        await refreshPrepared()
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
      await transport.call('setManagedSessionConfig', { session_id: target.session.session_id, change })
      if (current(intent)) await refreshPrepared()
    })
  }
  async function close(): Promise<string | null> {
    if (preparationTimer) clearTimeout(preparationTimer)
    preparationTimer = null
    closing = true
    readRevision += 1
    patch({ phase: promoted ? 'promoted' : 'closing' })
    try {
      await enqueue(async () => {
        if (sent && !promoted && !await refreshPrepared() && !promoted) throw new Error('Could not confirm whether the first message was accepted. Retry to check the session.')
        if (!promoted && prepared) {
          await transport.call('discardPreparedSession', { session_id: prepared.session.session_id })
          prepared = null
        }
      })
      closed = true
      choicesRevision += 1
      unsubscribe?.(); unsubscribe = null
      return promoted ? prepared?.session.session_id ?? null : null
    } catch (cause) {
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
