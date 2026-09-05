import { get, writable } from 'svelte/store'
import type { ApplicationTransport } from '$lib/application/applicationTransport'
import { APPLICATION_EVENTS_STREAM } from '$lib/application/applicationEvents'
import { readApplicationSnapshot } from '$lib/application/readApplicationSnapshot'
import { applicationResourcesAffectManagedSession } from '$lib/application/applicationSnapshotRefetch'
import type { AgentCatalogEntry, AgentConfig, AgentInspection, ManagedSessionSnapshot, SessionConfigChange } from '$lib/generated/feedback'
import { isAbsoluteAgentDirectory, redactAgentMessage } from './agentConfigForm'
import { readPromptFiles, validatePromptAttachments, type PromptAttachment } from './attachments/promptAttachments'
import { sessionPromptDrafts } from './managedSessionUi'
import type { ManagedSessionDraftStorage } from './managedSessionDrafts'

export type DraftAgentChoice = Readonly<{ key: string; name: string; config?: AgentConfig; catalogId?: string }>
export function draftAgentChoices(configs: readonly AgentConfig[], catalog: readonly AgentCatalogEntry[], inspections: readonly AgentInspection[]): DraftAgentChoice[] {
  const profiles = configs.map((config) => ({ key: `config:${config.id}`, name: config.name, config }))
  const installed = catalog.filter((entry) => !configs.some((config) => config.catalog_id === entry.id)
    && inspections.some((inspection) => inspection.agent_id === entry.id && Boolean(inspection.command)))
    .map((entry) => ({ key: `catalog:${entry.id}`, name: entry.name, catalogId: entry.id }))
  return [...profiles, ...installed]
}

export type DraftManagedSessionState = Readonly<{
  choice: string; cwd: string; text: string; attachments: readonly PromptAttachment[]
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
  const state = writable<DraftManagedSessionState>({ ...initial, attachments: [], choices: [], loadingChoices: true, choicesError: '', awaitingAcknowledgement: false, phase: 'idle', snapshot: null, error: '' })
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
    const remainingAttachments = latest.attachments
    sessionPromptDrafts.writeAttachments(snapshot.session.session_id, remainingAttachments)
    patch({ phase: 'promoted', snapshot, text: remainingText, attachments: remainingAttachments, awaitingAcknowledgement: false, error: '' })
    storage.remove(draftId)
    unsubscribe?.(); unsubscribe = null
    await onPromoted(snapshot)
  }

  let sent: { text: string; attachments: readonly PromptAttachment[]; editRevision: number } | null = null
  function restoreRejectedSubmission() {
    if (!sent) return
    if (draftEditRevision === sent.editRevision) { patch({ text: sent.text, attachments: sent.attachments }); persist() }
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
          const resolved = { key: `config:${config.id}`, name: config.name, config }
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

  async function refreshChoices() {
    if (closed || promoted) return
    const intent = ++choicesRevision
    patch({ loadingChoices: true, choicesError: '' })
    try {
      await transport.waitUntilReady()
      const [configs, catalog] = await Promise.all([transport.call('listAgentConfigs', undefined), transport.call('listAvailableAgents', undefined)])
      const inspected = await Promise.allSettled(catalog.map((entry) => transport.call('inspectAgentInstallation', { agent_id: entry.id })))
      if (intent !== choicesRevision || closed || promoted) return
      const choices = draftAgentChoices(configs, catalog, inspected.flatMap((result) => result.status === 'fulfilled' ? [result.value] : []))
      const value = get(state)
      const previousProfile = value.choices.find((item) => item.key === value.choice)?.config
      const selectedProfile = choices.find((item) => item.key === value.choice)?.config
      const selectionChanged = previousProfile && (!selectedProfile || selectedProfile.updated_at !== previousProfile.updated_at)
      if (selectionChanged && !sent && !closing) {
        revision += 1
        readRevision += 1
        patch({ snapshot: null, phase: 'idle', error: '' })
      }
      // Catalog materialization requires choosing that row. A saved profile can
      // be the default; discovering an installed catalog entry alone cannot save it.
      const choice = value.choice || choices.find((item) => item.config?.enabled)?.key || ''
      patch({ choices, choice, loadingChoices: false, choicesError: inspected.some((result) => result.status === 'rejected') ? 'Some installed agents could not be checked.' : '' })
      persist()
      if ((!prepared || selectionChanged) && !closing && !sent) void schedule()
    } catch (cause) {
      if (intent === choicesRevision && !closed) patch({ loadingChoices: false, choicesError: message(cause) })
    }
  }
  function start() {
    if (closed || promoted) return
    if (started) { void refreshChoices(); return }
    started = true
    unsubscribe = transport.subscribe(APPLICATION_EVENTS_STREAM, (event) => {
      if (event.type === 'ready') { if (prepared) void refreshPrepared(); return }
      // Installation inspection also emits this resource. Refresh saved profile state
      // on explicit UI activation, avoiding an inspect -> event -> inspect loop.
      if (prepared && applicationResourcesAffectManagedSession(event.resources, prepared.session.session_id)) void refreshPrepared()
    }, (cause) => { if (!closed && !promoted) patch({ error: message(cause) }) })
    void refreshChoices()
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
    const attachments = value.attachments
    const trimmed = text.trim()
    if (!trimmed && attachments.length === 0) return
    validatePromptAttachments(trimmed, attachments, prepared.runtime.capabilities.prompt)
    const target = prepared.session.session_id
    const intent = revision
    sent = { text, attachments, editRevision: draftEditRevision }
    sendPending = true
    // The submission stays recoverable until accepted; the composer is available
    // for the next draft without retaining already submitted attachments.
    patch({ phase: 'sending', text: '', attachments: [], awaitingAcknowledgement: true, error: '' })
    await enqueue(async () => {
      try {
        const snapshot = await transport.call('sendManagedPromptContent', { session_id: target, text: trimmed, content: attachments.map((attachment) => attachment.content) })
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
  async function addFiles(files: readonly File[]) {
    const value = get(state)
    if (!value.snapshot || closing || closed || promoted) return
    const intent = revision
    const attachments = await readPromptFiles(files, value.snapshot.runtime.capabilities.prompt)
    if (!current(intent)) return
    const combined = [...get(state).attachments, ...attachments]
    validatePromptAttachments(get(state).text.trim(), combined, value.snapshot.runtime.capabilities.prompt)
    draftEditRevision += 1
    patch({ attachments: combined })
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
  return { subscribe: state.subscribe, start, select, edit, send, configure, addFiles, close, refreshChoices,
    retry: () => schedule(true),
    removeAttachment: (id: string) => { if (!closing && !promoted) { draftEditRevision += 1; patch({ attachments: get(state).attachments.filter((item) => item.id !== id) }) } },
  }
}

export type DraftManagedSessionController = ReturnType<typeof createDraftManagedSessionController>
