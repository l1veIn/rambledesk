import { get } from 'svelte/store'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { TestApplicationTransport } from '../application/testApplicationTransport'
import { createUnavailableWorkbenchCapabilities } from '../capabilities/unavailableCapabilities'
import type { FeedbackWorkspaceView, ListFeedbackRequestsOutput } from '../feedback'
import { snapshotFeedbackDraftDocument } from '../feedbackDraftDocument'
import { previewFixtures, previewWorkspaceFor } from '../preview/previewFixtures'
import {
  agentDraftViewDescriptor, agentSessionViewDescriptor, sessionViewDescriptor,
  settingsViewDescriptor, workspaceViewKey,
} from '../workspace/viewDescriptors'
import type { SessionViewResolution } from '../workspace/sessionViewRecovery'
import { createAttachmentSession } from './attachmentSession'
import { createCookingSession } from './cookingSession'
import { createDraftController } from './draftController'
import { createDraftSession } from './draftSession'
import { createNavigationController } from './navigationController'
import { createWorkspaceNavigationController, type WorkspaceNavigationController } from './workspaceNavigationController'
import { createWorkspaceSession } from './workspaceSession'
import { createWorkspaceShellSession } from './workspaceShellSession'

vi.mock('../components/ui/sonner', () => ({ toast: { error: vi.fn(), success: vi.fn(), info: vi.fn() } }))

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (cause: Error) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}

const disposals: (() => void)[] = []
afterEach(() => { for (const dispose of disposals.splice(0)) dispose() })

async function harness() {
  const source = previewWorkspaceFor(previewFixtures.requests[0].request_id)!
  const workspaces = new Map<string, FeedbackWorkspaceView>(['alpha', 'beta', 'gamma'].map(id => [id, {
    ...structuredClone(source),
    request: { ...source.request, request_id: id, host_id: 'codex', host_session_id: id, status: 'in_progress' as const },
    draft: { document_json: null, body_markdown: id + ' draft', saved_revision: 1, updated_at: '2026-09-09T00:00:00Z' },
  } satisfies FeedbackWorkspaceView]))
  const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
    .resolve('listFeedbackInbox', [])
    .resolve('listHostProfiles', [])
    .resolve('listHostSessions', [])
    .handle('listFeedbackRequests', input => ({
      requests: [...workspaces.values()].map(workspace => workspace.request).filter(request => !input?.host_session_id || request.host_session_id === input.host_session_id),
      next_cursor: null,
    }))
    .handle('getFeedbackWorkspace', input => workspaces.get(input.request_id)!)
    .resolve('readPublishedFeedback', null)
    .handle('saveFeedbackDraft', input => {
      const workspace = workspaces.get(input.request_id)!
      const draft = {
        document_json: input.document_json, body_markdown: input.body_markdown,
        saved_revision: input.expected_revision + 1, updated_at: '2026-09-09T01:00:00Z',
      }
      workspaces.set(input.request_id, { ...workspace, draft })
      return draft
    })
  const workspaceShell = createWorkspaceShellSession({ snapshots: { load: () => null, save: () => {} } })
  const workspaceSession = createWorkspaceSession()
  const draftSession = createDraftSession()
  const attachmentSession = createAttachmentSession()
  const cookingSession = createCookingSession()
  let locked = false
  let mounted = true
  let resolution: SessionViewResolution | null = null
  const errors: string[] = []
  const releaseEditor = vi.fn()
  const draftController = createDraftController({
    transport, session: draftSession, messageFrom: String,
    isInteractionLocked: () => locked,
    isWorkspaceTerminal: workspaceSession.isTerminal,
    getWorkspace: () => get(workspaceSession).workspace,
    setWorkspaceDraft: workspaceSession.setDraft,
  })
  let controller!: WorkspaceNavigationController
  const navigation = createNavigationController({
    transport, capabilities: createUnavailableWorkbenchCapabilities(), tr: source => source, messageFrom: String,
    getNotificationState: () => 'disabled', getWorkspaceRequestId: () => workspaceSession.requestId() ?? undefined,
    isDirty: draftSession.isDirty, saveDraftNow: draftController.saveDraftNow,
    openRequest: requestId => controller.openRequest(requestId),
    clearWorkspace: () => controller.clearWorkspace(), onPageError: message => errors.push(message), canSendOsBanners: () => false,
  })
  const managed = {
    closeDraft: vi.fn(async (_draftId: string): Promise<{ skipped: boolean; promotedSessionId: string | null }> => ({ skipped: false, promotedSessionId: null })),
    promotedSessionId: (_draftId: string): string | undefined => undefined,
  }
  controller = createWorkspaceNavigationController({
    navigation, workspaceShell, workspaceSession, draftSession, draftController, attachmentSession, cookingSession,
    attachmentController: { releasePreviews: () => attachmentSession.setPreviews({}), refreshPreviews: vi.fn(async () => {}) },
    startup: () => ({
      patch: next => { if (next.mounted !== undefined) mounted = next.mounted },
      phase: () => 'ready', resolutionFor: () => resolution, refreshSessionViewRecovery: async () => 'applied',
    }),
    managedSessions: () => managed,
    transport, tr: source => source, messageFrom: String, setPageError: message => errors.push(message),
    releaseEditor, refreshNotificationPermission: vi.fn(), isTransitionLocked: () => locked,
    enqueueDocumentTask: task => task(),
    onboardingOpen: () => false, resumePromptOpen: () => false,
  })
  await navigation.initialize(false)
  const alpha = workspaces.get('alpha')!
  workspaceSession.open(alpha)
  draftSession.adopt(alpha.draft)
  workspaceShell.dispatch({ type: 'open', view: sessionViewDescriptor('codex', 'alpha') })
  workspaceShell.bindRequest(workspaceShell.activeViewKey()!, 'alpha')
  await navigation.selectScope('codex', 'alpha')
  disposals.push(() => { controller.dispose(); draftController.cancelPendingSave() })
  function edit(text: string) {
    draftSession.edit(snapshotFeedbackDraftDocument({ type: 'doc', content: [{ type: 'paragraph', content: [{ type: 'text', text }] }] }))
  }
  return {
    controller, transport, navigation, workspaceShell, workspaceSession, draftSession, workspaces, managed, releaseEditor, errors, edit,
    mounted: () => mounted,
    lock: (next: boolean) => { locked = next },
    resolution: (next: SessionViewResolution) => { resolution = next },
    scope: () => get(navigation).selectedHostSessionId,
  }
}

function view(id: string) { return sessionViewDescriptor('codex', id) }

describe('workspace navigation through real sessions and transport', () => {
  it('saves the original draft before accepting the target workspace and rail scope', async () => {
    const run = await harness()
    run.edit('save this before leaving')
    await expect(run.controller.selectRailScope('codex', 'beta')).resolves.toBe('activated')
    expect(run.workspaces.get('alpha')?.draft.body_markdown).toBe('save this before leaving')
    expect(run.workspaceSession.requestId()).toBe('beta')
    expect(run.workspaceShell.activeView()).toEqual(view('beta'))
    expect(run.scope()).toBe('beta')
    expect(get(run.draftSession).body).toBe('beta draft')
    expect(run.mounted()).toBe(true)
  })

  it('keeps draft, tab and scope together on save failure and permits an explicit retry', async () => {
    const run = await harness()
    run.edit('unsaved words')
    run.transport.reject('saveFeedbackDraft', new Error('disk full'))
    await expect(run.controller.selectRailScope('codex', 'beta')).resolves.toBe('blocked')
    expect(run.workspaceSession.requestId()).toBe('alpha')
    expect(run.workspaceShell.activeView()).toEqual(view('alpha'))
    expect(run.scope()).toBe('alpha')
    expect(get(run.draftSession)).toMatchObject({ body: 'unsaved words', phase: 'error', dirty: true })
    expect(run.releaseEditor).not.toHaveBeenCalled()
    run.transport.handle('saveFeedbackDraft', input => ({ document_json: input.document_json, body_markdown: input.body_markdown, saved_revision: 2, updated_at: null }))
    await expect(run.controller.selectRailScope('codex', 'beta')).resolves.toBe('activated')
    expect(run.scope()).toBe('beta')
  })

  it('restores the editor after a target load fails without exposing its candidate scope', async () => {
    const run = await harness()
    const seenScopes: (string | null)[] = []
    const unsubscribe = run.navigation.subscribe(state => seenScopes.push(state.selectedHostSessionId))
    run.transport.reject('getFeedbackWorkspace', new Error('load failed'))
    await expect(run.controller.selectRailScope('codex', 'beta')).resolves.toBe('failed')
    unsubscribe()
    expect(new Set(seenScopes)).toEqual(new Set(['alpha']))
    expect(run.workspaceSession.requestId()).toBe('alpha')
    expect(run.mounted()).toBe(true)
    expect(run.workspaceShell.pendingViewKey()).toBeNull()
  })

  it('does not treat an uncommitted B scope as the rollback destination when C also fails', async () => {
    const run = await harness()
    const beta = deferred<ListFeedbackRequestsOutput>()
    run.transport.handle('listFeedbackRequests', input => {
      if (input.host_session_id === 'beta') return beta.promise
      throw new Error('C scope failed')
    })
    const old = run.controller.selectRailScope('codex', 'beta')
    await expect(run.controller.selectRailScope('codex', 'gamma')).resolves.toBe('failed')
    beta.resolve({ requests: [run.workspaces.get('beta')!.request], next_cursor: null })
    await expect(old).resolves.toBe('stale')
    expect(run.scope()).toBe('alpha')
    expect(run.workspaceSession.requestId()).toBe('alpha')
  })

  it('keeps the committed rail scope when settings supersede a delayed session selection', async () => {
    const run = await harness()
    const beta = deferred<ListFeedbackRequestsOutput>()
    run.transport.handle('listFeedbackRequests', () => beta.promise)
    const old = run.controller.selectRailScope('codex', 'beta')
    await expect(run.controller.openView(settingsViewDescriptor())).resolves.toBe('activated')
    beta.resolve({ requests: [run.workspaces.get('beta')!.request], next_cursor: null })
    await expect(old).resolves.toBe('stale')
    expect(run.workspaceShell.activeView()).toEqual(settingsViewDescriptor())
    expect(run.scope()).toBe('alpha')
    expect(run.workspaceSession.requestId()).toBeNull()
  })

  it('accepts only C when B finishes loading after the user selects C', async () => {
    const run = await harness()
    const beta = deferred<FeedbackWorkspaceView>()
    run.transport.handle('getFeedbackWorkspace', input => input.request_id === 'beta' ? beta.promise : run.workspaces.get(input.request_id)!)
    const old = run.controller.selectRailScope('codex', 'beta')
    await vi.waitFor(() => expect(run.transport.callsFor('getFeedbackWorkspace')).toHaveLength(1))
    const latest = run.controller.selectRailScope('codex', 'gamma')
    beta.resolve(run.workspaces.get('beta')!)
    await expect(old).resolves.toBe('stale')
    await expect(latest).resolves.toBe('activated')
    expect(run.workspaceSession.requestId()).toBe('gamma')
    expect(run.scope()).toBe('gamma')
    expect(run.mounted()).toBe(true)
  })

  it('does not apply a view preparation callback when saving blocks the open', async () => {
    const run = await harness()
    run.edit('keep this')
    run.transport.reject('saveFeedbackDraft', new Error('offline'))
    const prepare = vi.fn()
    await expect(run.controller.openView(settingsViewDescriptor(), { prepare })).resolves.toBe('blocked')
    expect(prepare).not.toHaveBeenCalled()
    expect(run.workspaceShell.activeView()).toEqual(view('alpha'))
  })

  it('closes a background tab without saving, and an active tab through its saved fallback', async () => {
    const run = await harness()
    run.workspaceShell.dispatch({ type: 'open', view: view('beta') })
    run.workspaceShell.bindRequest(workspaceViewKey(view('beta')), 'beta')
    run.workspaceShell.dispatch({ type: 'focus', viewKey: workspaceViewKey(view('alpha')) })
    run.edit('still editing alpha')
    await run.controller.closeWorkspaceTab(workspaceViewKey(view('beta')))
    expect(run.transport.callsFor('saveFeedbackDraft')).toHaveLength(0)
    expect(run.workspaceShell.requestIdFor(view('beta'))).toBeUndefined()
    run.workspaceShell.dispatch({ type: 'open', view: view('beta') })
    run.workspaceShell.dispatch({ type: 'focus', viewKey: workspaceViewKey(view('alpha')) })
    await expect(run.controller.closeWorkspaceTab(workspaceViewKey(view('alpha')))).resolves.toBe('activated')
    expect(run.workspaces.get('alpha')!.draft.body_markdown).toBe('still editing alpha')
    expect(run.workspaceSession.requestId()).toBe('beta')
    expect(run.scope()).toBe('beta')
    await run.controller.closeWorkspaceTab(workspaceViewKey(view('beta')))
    expect(run.workspaceShell.activeView()).toBeNull()
    expect(run.workspaceSession.requestId()).toBeNull()
    expect(run.scope()).toBeNull()
  })

  it('opens a missing session explanation with no obsolete request and a matching global scope', async () => {
    const run = await harness()
    run.workspaceShell.dispatch({ type: 'open', view: view('missing') })
    run.workspaceShell.dispatch({ type: 'focus', viewKey: workspaceViewKey(view('alpha')) })
    run.resolution({ kind: 'missing-session', session: view('missing'), reason: 'unavailable' })
    await expect(run.controller.activateWorkspaceTab(workspaceViewKey(view('missing')))).resolves.toBe('activated')
    expect(run.workspaceShell.activeView()).toEqual(view('missing'))
    expect(run.workspaceSession.requestId()).toBeNull()
    expect(run.scope()).toBeNull()
    expect(run.transport.callsFor('getFeedbackWorkspace')).toHaveLength(0)
  })

  it('closes the promoted view when accepting a prepared draft races with closing it', async () => {
    const run = await harness()
    const draft = agentDraftViewDescriptor('prepared')
    await run.controller.openView(draft)
    run.managed.closeDraft.mockImplementation(async () => {
      run.workspaceShell.dispatch({ type: 'replace', viewKey: workspaceViewKey(draft), view: agentSessionViewDescriptor('promoted') })
      return { skipped: false, promotedSessionId: 'promoted' }
    })
    await expect(run.controller.closeWorkspaceTab(workspaceViewKey(draft))).resolves.toBe('activated')
    expect(run.workspaceShell.activeView()).toEqual(view('alpha'))
    expect(run.workspaceShell.views().some(candidate => candidate.kind === 'agent-session')).toBe(false)
    expect(run.workspaceSession.requestId()).toBe('alpha')
  })

  it('discards a background refetch that was already waiting when the user navigates', async () => {
    const run = await harness()
    const facts = deferred<[]>();
    run.transport.handle('listHostSessions', () => facts.promise)
    run.controller.requestRefetch([{ kind: 'navigation' }])
    await vi.waitFor(() => expect(run.transport.callsFor('listHostSessions')).toHaveLength(2))
    await expect(run.controller.selectRailScope('codex', 'beta')).resolves.toBe('activated')
    facts.resolve([])
    await vi.waitFor(() => expect(get(run.navigation).loadingNavigation).toBe(false))
    expect(run.transport.callsFor('getFeedbackWorkspace').map(call => call.input.request_id)).toEqual(['beta'])
    expect(run.workspaceSession.requestId()).toBe('beta')
    expect(run.scope()).toBe('beta')
  })

  it('reconciles a save invalidation without releasing the current editor or resetting its epoch', async () => {
    const run = await harness()
    const epoch = get(run.draftSession).editorEpoch
    run.edit('An autosaved observation')
    run.controller.requestRefetch([{ kind: 'feedback_workspace', request_id: 'alpha' }])
    await vi.waitFor(() => expect(run.transport.callsFor('getFeedbackWorkspace')).toHaveLength(1))
    await vi.waitFor(() => expect(get(run.draftSession).savedRevision).toBe(2))
    expect(run.releaseEditor).not.toHaveBeenCalled()
    expect(get(run.draftSession).editorEpoch).toBe(epoch)
    expect(run.draftSession.isDirty()).toBe(false)
    expect(get(run.workspaceSession).workspace?.draft.body_markdown).toBe('An autosaved observation')
  })

  it('still loads a genuinely changed remote document through the existing editor transition', async () => {
    const run = await harness()
    const epoch = get(run.draftSession).editorEpoch
    const original = run.workspaces.get('alpha')!
    run.workspaces.set('alpha', { ...original, draft: {
      ...original.draft, body_markdown: 'Edited in another client', saved_revision: 2,
    } })
    run.controller.requestRefetch([{ kind: 'feedback_workspace', request_id: 'alpha' }])
    await vi.waitFor(() => expect(run.draftSession.snapshot().bodyMarkdown).toBe('Edited in another client'))
    expect(run.releaseEditor).toHaveBeenCalledOnce()
    expect(get(run.draftSession).editorEpoch).toBeGreaterThan(epoch)
  })

  it('keeps input arriving while the known saved document is being refetched', async () => {
    const run = await harness()
    const pending = deferred<FeedbackWorkspaceView>()
    const reconcile = vi.spyOn(run.draftSession, 'reconcile')
    run.edit('Saved before the refresh')
    run.transport.handle('getFeedbackWorkspace', () => pending.promise)
    run.controller.requestRefetch([{ kind: 'feedback_workspace', request_id: 'alpha' }])
    await vi.waitFor(() => expect(run.transport.callsFor('getFeedbackWorkspace')).toHaveLength(1))
    const saved = structuredClone(run.workspaces.get('alpha')!)
    run.edit('New input while refreshing')
    pending.resolve(saved)
    await vi.waitFor(() => expect(reconcile).toHaveBeenCalledOnce())
    expect(run.draftSession.snapshot().bodyMarkdown).toBe('New input while refreshing')
    expect(run.draftSession.savedSnapshot().bodyMarkdown).toBe('Saved before the refresh')
    expect(run.draftSession.isDirty()).toBe(true)
    expect(run.releaseEditor).not.toHaveBeenCalled()
  })

  it('drops an in-place refresh response after a newer user navigation', async () => {
    const run = await harness()
    const pending = deferred<FeedbackWorkspaceView>()
    run.transport.handle('getFeedbackWorkspace', input => input.request_id === 'alpha'
      ? pending.promise : run.workspaces.get(input.request_id)!)
    run.controller.requestRefetch([{ kind: 'feedback_workspace', request_id: 'alpha' }])
    await vi.waitFor(() => expect(run.transport.callsFor('getFeedbackWorkspace')).toHaveLength(1))
    await expect(run.controller.openRequest('beta')).resolves.toBe(true)
    pending.resolve(run.workspaces.get('alpha')!)
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(run.workspaceSession.requestId()).toBe('beta')
    expect(run.draftSession.snapshot().bodyMarkdown).toBe('beta draft')
    expect(run.scope()).toBe('beta')
    expect(run.releaseEditor).toHaveBeenCalledOnce()
  })

  it('switches a Ramble page to the newest arriving request for that session', async () => {
    const run = await harness()
    const arrival = {
      ...run.workspaces.get('alpha')!.request,
      request_id: 'arrived',
      status: 'waiting' as const,
      updated_at: '2026-09-09T02:00:00Z',
    }
    run.workspaces.set('arrived', { ...run.workspaces.get('alpha')!, request: arrival })
    await run.controller.autoOpenArrivingRequest([arrival])
    expect(run.workspaceSession.requestId()).toBe('arrived')
    expect(get(run.navigation).requests[0]?.request_id).toBe('arrived')
  })

  it('opens an arriving Ramble from the Agent page being watched even if a background refresh invalidated navigation', async () => {
    const run = await harness()
    const arrival = {
      ...run.workspaces.get('alpha')!.request,
      request_id: 'arrived',
      status: 'waiting' as const,
      managed_session_id: 'agent-one',
    }
    run.workspaces.set('arrived', { ...run.workspaces.get('alpha')!, request: arrival })
    run.transport.resolve('listFeedbackInbox', [arrival])
    await run.navigation.refreshNavigation()
    await run.controller.openView(agentSessionViewDescriptor('agent-one'))
    expect(run.workspaceShell.activeView()).toEqual(agentSessionViewDescriptor('agent-one'))
    run.controller.invalidate()
    run.lock(true)
    await run.controller.autoOpenArrivingRequest([arrival])
    expect(run.workspaceSession.requestId()).toBe('arrived')
    expect(run.workspaceShell.activeView()).toEqual(sessionViewDescriptor('codex', 'alpha'))
    expect(run.workspaceShell.requestIdFor(sessionViewDescriptor('codex', 'alpha'))).toBe('arrived')
    expect(get(run.navigation).requests[0]?.request_id).toBe('arrived')
  })

  it('retries a blocked automatic task open and ignores asynchronous workspace results after disposal', async () => {
    const run = await harness()
    run.lock(true)
    await run.controller.autoOpenTaskView('beta')
    run.lock(false)
    await run.controller.autoOpenTaskView('beta')
    expect(run.workspaceSession.requestId()).toBe('beta')
    const gamma = deferred<FeedbackWorkspaceView>()
    run.transport.handle('getFeedbackWorkspace', () => gamma.promise)
    const pending = run.controller.openRequest('gamma')
    await vi.waitFor(() => expect(run.transport.callsFor('getFeedbackWorkspace')).toHaveLength(2))
    run.controller.dispose()
    gamma.resolve(run.workspaces.get('gamma')!)
    await expect(pending).resolves.toBe(false)
    expect(run.workspaceSession.requestId()).toBe('beta')
  })
})
