import { get } from 'svelte/store'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { TestApplicationTransport } from '../application/testApplicationTransport'
import type { DraftView, FeedbackRequestView, FeedbackWorkspaceView } from '../feedback'
import { snapshotFeedbackDraftDocument } from '../feedbackDraftDocument'
import { previewFixtures } from '../preview/previewFixtures'
import { createDraftController } from './draftController'
import { createDraftSession } from './draftSession'
import { createSubmissionController, type SubmissionControllerContext } from './submissionController'
import { createWorkspaceSession } from './workspaceSession'
import { workbenchPreviewWorkspace } from '../../dev/workbenchPreviewFixtures'
import { readWorkbenchState, withWorkbenchState } from '../workbenchState'
import type { WorkbenchState } from '../generated/feedback'
import { createWorkbenchLifecycle } from './workbenchLifecycle'

const cleanups: Array<() => void> = []
afterEach(() => { for (const cleanup of cleanups.splice(0)) cleanup(); vi.unstubAllGlobals() })
type Intent = 'approve' | 'cancel'
const commandFor = (intent: Intent) => intent === 'approve' ? 'approveFeedbackRequest' : 'cancelFeedbackRequest'

function workspace(requestId = 'request-1'): FeedbackWorkspaceView {
  return { ...previewFixtures.workspace,
    request: { ...previewFixtures.workspace.request, request_id: requestId, allow_finish: true },
  }
}

function terminal(requestId: string, intent: Intent): FeedbackRequestView {
  return {
    request_id: requestId, host_id: 'codex', host_session_id: 'host-session', execution_mode: 'wait',
    status: intent === 'approve' ? 'completed' : 'cancelled',
    resolution: intent === 'approve' ? 'approved' : null, allow_finish: true,
    feedback: null, final_summary: null, created_at: '2026-09-09T10:00:00Z', updated_at: '2026-09-09T11:00:00Z',
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (cause: unknown) => void
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail })
  return { promise, resolve, reject }
}

/** Real request/draft sessions and save controller; only backend/input boundaries are controlled. */
function harness(options: Partial<SubmissionControllerContext> = {}) {
  const transport = new TestApplicationTransport()
    .handle('saveFeedbackDraft', (input) => ({
      document_json: input.document_json, body_markdown: input.body_markdown,
      saved_revision: input.expected_revision + 1, updated_at: '2026-09-09T10:30:00Z',
    }))
    .handle('approveFeedbackRequest', ({ request_id }) => terminal(request_id, 'approve'))
    .handle('cancelFeedbackRequest', ({ request_id }) => terminal(request_id, 'cancel'))
  const session = createWorkspaceSession()
  const draftSession = createDraftSession()
  const first = workspace()
  session.open(first)
  draftSession.adopt(first.draft)
  const drafts = createDraftController({
    transport, session: draftSession, messageFrom: String,
    isInteractionLocked: () => get(session).interactionLocked,
    isWorkspaceTerminal: () => get(session).terminal,
    getWorkspace: () => get(session).workspace,
    setWorkspaceDraft: session.setDraft,
  })
  cleanups.push(drafts.cancelPendingSave)
  const setPageError = vi.fn()
  const notifyApproved = vi.fn()
  const notifyCancelled = vi.fn()
  const refreshNavigation = vi.fn(async () => {})
  const prepareFeedback = vi.fn(async () => ({ kind: 'ready' as const }))
  const controller = createSubmissionController({
    transport, session, draftSession,
    publishedFeedbackAction: { label: 'Open feedback package', run: vi.fn(async () => {}) },
    tr: (source, values) => source.replace(/\{(\w+)\}/g, (_, key) => String(values?.[key] ?? key)),
    messageFrom: (cause) => cause instanceof Error ? cause.message : String(cause),
    canCancel: () => true,
    prepareFeedback, saveDraftNow: drafts.saveDraftNow, confirmApproval: () => true,
    setPageError, notifyApproved, notifyCancelled, refreshNavigation,
    ...options,
  })
  function edit(body: string) {
    drafts.updateDraft(snapshotFeedbackDraftDocument({
      type: 'doc', content: [{ type: 'paragraph', content: [{ type: 'text', text: body }] }],
    }))
  }
  function open(requestId: string) {
    session.close()
    const next = workspace(requestId)
    session.open(next)
    draftSession.adopt(next.draft)
  }
  return {
    transport, session, draftSession, drafts, controller, setPageError,
    notifyApproved, notifyCancelled, refreshNavigation, prepareFeedback, edit, open,
    run: (intent: Intent) => intent === 'approve' ? controller.approveFeedback() : controller.cancelFeedback(),
  }
}

describe('submission controller', () => {
  it.each(['empty-text', 'unavailable-image'] as const)('cancels visual feedback with %s without generating a publication artifact', async (scenario) => {
    let lifecycle: ReturnType<typeof createWorkbenchLifecycle>
    const h = harness({ prepareWorkbench: (requestId, intent) => lifecycle.prepareSubmission(requestId, intent) })
    const original = workspace()
    const data = { title: 'Sketch', source_version: 'snapshot-1', width: 640, height: 400,
      image_file_name: scenario === 'unavailable-image' ? 'original.png' : null }
    h.session.replace({ ...original, workbench: { type: 'visual_feedback', version: 1, data },
      request_attachments: scenario === 'unavailable-image' ? [{ attachment_id: 'original_image', file_name: 'original.png', media_type: 'image/png', byte_size: 4, sha256: 'hash', position: 0 }] : [] })
    const state: WorkbenchState = { type: 'visual_feedback', composite_attachment_id: null, annotations: scenario === 'empty-text' ? [{
      id: 'text_one', kind: 'text', points: [{ x: 20, y: 30 }], color: '#e5484d', stroke_width: 24, text: '', body: '',
    }] : [] }
    h.drafts.updateDraft(withWorkbenchState(h.draftSession.snapshot(), state))
    h.transport.handle('readRequestAttachment', () => { throw new Error('Original image is unavailable') })
    const persistGeneratedAttachment = vi.fn(async () => { throw new Error('Image upload must not run for cancellation') })
    lifecycle = createWorkbenchLifecycle({ transport: h.transport, getWorkspace: () => get(h.session).workspace,
      getState: () => readWorkbenchState(h.draftSession.snapshot().documentJson),
      updateState: (next) => h.drafts.updateDraft(withWorkbenchState(h.draftSession.snapshot(), next)),
      isEditable: () => !h.session.isTerminal() && !get(h.session).interactionLocked, onBusy: () => {}, persistGeneratedAttachment })
    cleanups.push(lifecycle.dispose)
    await h.controller.cancelFeedback()
    expect(h.transport.callsFor('cancelFeedbackRequest')).toHaveLength(1)
    expect(h.transport.callsFor('readRequestAttachment')).toHaveLength(0)
    expect(persistGeneratedAttachment).not.toHaveBeenCalled()
    expect(h.setPageError).toHaveBeenLastCalledWith('')
    expect(readWorkbenchState(get(h.draftSession).documentJson)).toEqual(state)
  })

  it('still retires a recorded terminal session before cancellation', async () => {
    let lifecycle: ReturnType<typeof createWorkbenchLifecycle>
    const h = harness({ prepareWorkbench: (requestId, intent) => lifecycle.prepareSubmission(requestId, intent) })
    h.session.replace({ ...workspace(), workbench: { type: 'terminal', version: 1, data: { cwd: '/preview/cli-demo' } } })
    h.drafts.updateDraft(withWorkbenchState(h.draftSession.snapshot(), { type: 'terminal', sessions: [{
      id: 'trial_one', cwd: '/preview/cli-demo', shell: 'preview', cols: 80, rows: 24, status: 'running', exit_code: null,
      output: 'Preserved output', screen: 'Preserved output', truncated: false,
    }] }))
    h.transport.handle('stopTerminalSession', () => { throw { code: 'INVALID_ARGUMENT', message: 'The terminal session was not found for this request.' } })
    lifecycle = createWorkbenchLifecycle({ transport: h.transport, getWorkspace: () => get(h.session).workspace,
      getState: () => readWorkbenchState(h.draftSession.snapshot().documentJson),
      updateState: (next) => h.drafts.updateDraft(withWorkbenchState(h.draftSession.snapshot(), next)),
      isEditable: () => !h.session.isTerminal() && !get(h.session).interactionLocked, onBusy: () => {} })
    cleanups.push(lifecycle.dispose)
    await h.controller.cancelFeedback()
    expect(h.transport.callsFor('stopTerminalSession')).toHaveLength(1)
    expect(h.transport.callsFor('cancelFeedbackRequest')).toHaveLength(1)
    const names = h.transport.calls.map((call) => call.name)
    expect(names.indexOf('stopTerminalSession')).toBeLessThan(names.indexOf('cancelFeedbackRequest'))
    expect(readWorkbenchState(get(h.draftSession).documentJson)).toMatchObject({ type: 'terminal', sessions: [{ status: 'stopped', output: 'Preserved output' }] })
  })

  it.each(['approve', 'cancel'] as const)('passes explicit %s intent to workbench preparation', async (intent) => {
    const prepareWorkbench = vi.fn(async () => {})
    const h = harness({ prepareWorkbench }); await h.run(intent)
    expect(prepareWorkbench).toHaveBeenCalledExactlyOnceWith('request-1', intent)
  })

  it.each(['approve', 'cancel'] as const)('does not freeze or %s when input arrives after preparation resolves', async (intent) => {
    const ready = deferred<{ kind: 'ready' }>()
    let busy = false
    const prepareFeedback = vi.fn(() => ready.promise)
    const h = harness({ prepareFeedback, isInputBusy: () => busy })
    h.edit('Preserve the current feedback')
    const lock = vi.spyOn(h.session, intent === 'approve' ? 'beginApprove' : 'beginCancel')
    const finishing = h.run(intent)
    await vi.waitFor(() => expect(prepareFeedback).toHaveBeenCalledOnce())

    ready.resolve({ kind: 'ready' })
    busy = true
    await finishing
    expect(lock).not.toHaveBeenCalled()
    expect(h.transport.calls).toHaveLength(0)
    expect(get(h.draftSession)).toMatchObject({ body: 'Preserve the current feedback', dirty: true })
    expect(h.setPageError).toHaveBeenLastCalledWith('Input is still being received. Finish the current input and try again.')

    busy = false
    await h.run(intent)
    expect(h.transport.callsFor(commandFor(intent))).toHaveLength(1)
  })

  it.each([0, 1, 2, 3])('only approves valid Ramble despite a historical allow_finish flag (type %s)', async (index) => {
    const h = harness()
    h.session.replace({ ...workspace(), workbench: workbenchPreviewWorkspace(index).workbench })
    await h.controller.approveFeedback()
    expect(h.transport.callsFor('approveFeedbackRequest')).toHaveLength(index === 0 ? 1 : 0)
    if (index !== 0) {
      expect(h.prepareFeedback).not.toHaveBeenCalled()
      await h.controller.cancelFeedback()
      expect(h.transport.callsFor('cancelFeedbackRequest')).toHaveLength(1)
    }
  })

  it('cancels an unsupported request without preparing speech or rewriting its opaque draft, but never approves it', async () => {
    const h = harness()
    const original = workspace()
    const documentJson = JSON.stringify({ schemaVersion: 2, doc: { type: 'doc', content: [] }, workbenchState: { type: 'future_workbench', value: { keep: true } } })
    const draft = { ...original.draft, document_json: documentJson, body_markdown: '' }
    h.session.replace({ ...original, draft, workbench: { type: 'future_workbench', version: 99, data: { future: true } } })
    h.draftSession.adopt(draft)
    const before = h.draftSession.snapshot()
    await h.controller.approveFeedback()
    expect(h.transport.calls).toEqual([])
    await h.controller.cancelFeedback()
    expect(h.prepareFeedback).not.toHaveBeenCalled()
    expect(h.transport.callsFor('saveFeedbackDraft')).toHaveLength(0)
    expect(h.transport.callsFor('cancelFeedbackRequest')).toHaveLength(1)
    expect(h.draftSession.snapshot()).toEqual(before)
  })

  it('honors external approval locks before preparing input', async () => {
    const h = harness({ canApprove: () => false })
    await h.controller.approveFeedback()
    expect(h.prepareFeedback).not.toHaveBeenCalled()
    expect(h.transport.calls).toEqual([])
  })

  it('keeps the confirmed document locked until its save finishes', async () => {
    const saved = deferred<DraftView>()
    const h = harness()
    h.edit('The confirmed document')
    h.transport.handle('saveFeedbackDraft', () => saved.promise)
    const cancelling = h.controller.cancelFeedback()
    await vi.waitFor(() => expect(h.transport.callsFor('saveFeedbackDraft')).toHaveLength(1))
    expect(get(h.session).interactionLocked).toBe(true)
    expect(h.transport.callsFor('cancelFeedbackRequest')).toHaveLength(0)
    h.edit('A late UI edit must remain blocked')
    expect(get(h.draftSession).body).toBe('The confirmed document')
    const input = h.transport.callsFor('saveFeedbackDraft')[0].input
    saved.resolve({
      document_json: input.document_json, body_markdown: input.body_markdown,
      saved_revision: input.expected_revision + 1, updated_at: '2026-09-09T10:30:00Z',
    })
    await cancelling
    expect(h.transport.callsFor('cancelFeedbackRequest')).toHaveLength(1)
    expect(get(h.session).interactionLocked).toBe(false)
  })

  it('preserves the browser approval confirmation and does nothing when declined', async () => {
    const confirm = vi.fn(() => false)
    vi.stubGlobal('window', { confirm })
    const h = harness({ confirmApproval: undefined })
    await h.controller.approveFeedback()
    expect(confirm).toHaveBeenCalledWith('Approve this final summary and end Pi’s Ramble flow?')
    expect(h.prepareFeedback).not.toHaveBeenCalled()
    expect(h.transport.calls).toEqual([])
  })

  it('does not prepare input for an ineligible cancel or approval', async () => {
    const h = harness({ canCancel: () => false })
    await h.controller.cancelFeedback()
    const view = workspace()
    h.session.replace({ ...view, request: { ...view.request, allow_finish: false } })
    await h.controller.approveFeedback()
    expect(h.prepareFeedback).not.toHaveBeenCalled()
    expect(h.transport.calls).toEqual([])
  })

  it.each(['Open feedback package', 'Download published feedback'] as const)('delegates %s for the current package and reports failures', async (label) => {
    const run = vi.fn(async () => { throw new Error('Package missing') })
    const h = harness({ publishedFeedbackAction: { label, run } })
    await h.controller.openFeedbackPackage()
    expect(run).not.toHaveBeenCalled()
    h.session.setCompleted({ ...terminal('request-1', 'approve'), feedback: { available: true } })
    await h.controller.openFeedbackPackage()
    expect(run).toHaveBeenCalledWith('request-1')
    expect(h.setPageError).toHaveBeenLastCalledWith(label === 'Open feedback package'
      ? 'Could not open Feedback Package: Package missing'
      : 'Could not download published feedback: Package missing')
  })

  it.each(['approve', 'cancel'] as const)('retains dirty content after a failed save and only %ss after explicit retry', async (intent) => {
    const h = harness()
    const savedBody = get(h.draftSession).savedBody
    h.edit('Unsaved review must survive')
    h.transport.reject('saveFeedbackDraft', new Error('Storage full'))
    await h.run(intent)
    expect(h.transport.callsFor(commandFor(intent))).toHaveLength(0)
    expect(get(h.draftSession)).toMatchObject({ body: 'Unsaved review must survive', savedBody, dirty: true, phase: 'error' })
    expect(h.session.isTerminal()).toBe(false)
    expect(get(h.session).interactionLocked).toBe(false)
    expect(h.notifyApproved).not.toHaveBeenCalled()
    expect(h.notifyCancelled).not.toHaveBeenCalled()
    h.transport.handle('saveFeedbackDraft', (input) => ({
      document_json: input.document_json, body_markdown: input.body_markdown,
      saved_revision: input.expected_revision + 1, updated_at: '2026-09-09T10:30:00Z',
    }))
    await h.run(intent)
    expect(h.transport.callsFor(commandFor(intent))).toHaveLength(1)
    expect(get(h.draftSession).dirty).toBe(false)
  })

  it.each(['approve', 'cancel'] as const)('stops input before freezing and saving the final speech for %s', async (intent) => {
    const h = harness({ prepareFeedback: async () => {
      h.edit('The final speech segment')
      return { kind: 'ready' }
    } })
    await h.run(intent)
    expect(h.transport.callsFor('saveFeedbackDraft')[0].input.body_markdown).toBe('The final speech segment')
    expect(get(h.draftSession).savedBody).toBe('The final speech segment')
  })

  it.each(['approve', 'cancel'] as const)('blocks %s while input needs review or failed to finish', async (intent) => {
    for (const kind of ['pending-speech', 'failed', 'thrown'] as const) {
      const h = harness({ prepareFeedback: async () => {
        if (kind === 'thrown') throw new Error('Microphone stopped unexpectedly')
        return kind === 'failed' ? { kind, message: 'Input could not finish' } : { kind }
      } })
      h.edit('Keep local feedback')
      await h.run(intent)
      expect(h.transport.calls).toEqual([])
      expect(get(h.draftSession)).toMatchObject({ body: 'Keep local feedback', dirty: true, phase: 'unsaved' })
      expect(get(h.session).interactionLocked).toBe(false)
      expect(h.setPageError.mock.lastCall?.[0]).toContain(kind === 'pending-speech' ? 'pending speech' : kind === 'failed' ? 'Input could not finish' : 'Microphone stopped unexpectedly')
    }
  })

  it('does not save or cancel a new request selected while input is finishing', async () => {
    const ready = deferred<{ kind: 'ready' }>()
    const h = harness({ prepareFeedback: () => ready.promise })
    const cancelling = h.controller.cancelFeedback()
    await Promise.resolve()
    h.open('request-2')
    h.edit('Another request is still unsaved')
    ready.resolve({ kind: 'ready' })
    await cancelling
    expect(h.transport.calls).toEqual([])
    expect(get(h.draftSession)).toMatchObject({ body: 'Another request is still unsaved', dirty: true, phase: 'unsaved' })
    expect(h.session.requestId()).toBe('request-2')
  })

  it.each(['resolve', 'reject'] as const)('does not touch the new request when a terminal response %ss late', async (outcome) => {
    const response = deferred<FeedbackRequestView>()
    const h = harness()
    h.transport.handle('cancelFeedbackRequest', () => response.promise)
    const cancelling = h.controller.cancelFeedback()
    await vi.waitFor(() => expect(h.transport.callsFor('cancelFeedbackRequest')).toHaveLength(1))
    h.open('request-2')
    h.edit('Local work on another request')
    const errors = h.setPageError.mock.calls.length
    if (outcome === 'resolve') response.resolve(terminal('request-1', 'cancel'))
    else response.reject(new Error('Old request failed'))
    await cancelling
    expect(h.session.requestId()).toBe('request-2')
    expect(h.session.isTerminal()).toBe(false)
    expect(get(h.draftSession)).toMatchObject({ body: 'Local work on another request', dirty: true, phase: 'unsaved' })
    expect(h.setPageError).toHaveBeenCalledTimes(errors)
    expect(h.notifyCancelled).not.toHaveBeenCalled()
    expect(get(h.session).interactionLocked).toBe(false)
  })

  it.each(['approve', 'cancel'] as const)('does not replay an unknown %s result until an explicit retry', async (intent) => {
    const h = harness()
    h.edit('Feedback was saved before the uncertain response')
    let attempts = 0
    h.transport.handle(commandFor(intent), ({ request_id }) => {
      if (++attempts === 1) throw new Error('The terminal response was lost')
      return terminal(request_id, intent)
    })
    await h.run(intent)
    expect(attempts).toBe(1)
    expect(h.session.isTerminal()).toBe(false)
    expect(get(h.draftSession).body).toBe('Feedback was saved before the uncertain response')
    expect(h.notifyApproved).not.toHaveBeenCalled()
    expect(h.notifyCancelled).not.toHaveBeenCalled()
    await h.run(intent)
    expect(attempts).toBe(2)
    expect(h.session.isTerminal()).toBe(true)
    expect(h.transport.callsFor('saveFeedbackDraft')).toHaveLength(1)
  })

  it.each(['approve', 'cancel'] as const)('keeps confirmed %s distinct from a failed navigation refresh and never sends again', async (intent) => {
    const h = harness({ refreshNavigation: async () => { throw new Error('Navigation unavailable') } })
    await h.run(intent)
    expect(h.session.isTerminal()).toBe(true)
    expect(intent === 'approve' ? h.notifyApproved : h.notifyCancelled).toHaveBeenCalledOnce()
    expect(h.setPageError).toHaveBeenLastCalledWith('The request was updated, but navigation could not be refreshed: Navigation unavailable')
    await h.run(intent)
    await h.run(intent === 'approve' ? 'cancel' : 'approve')
    expect(h.transport.callsFor(commandFor(intent))).toHaveLength(1)
    expect(h.transport.callsFor(commandFor(intent === 'approve' ? 'cancel' : 'approve'))).toHaveLength(0)
  })

  it.each(['approve', 'cancel'] as const)('reserves one %s flight before input preparation and blocks the opposite terminal action', async (intent) => {
    const ready = deferred<{ kind: 'ready' }>()
    const prepareFeedback = vi.fn(() => ready.promise)
    const h = harness({ prepareFeedback })
    const first = h.run(intent)
    const second = h.run(intent)
    const opposite = h.run(intent === 'approve' ? 'cancel' : 'approve')
    expect(second).toBe(first)
    expect(opposite).not.toBe(first)
    await opposite
    expect(prepareFeedback).toHaveBeenCalledTimes(1)
    ready.resolve({ kind: 'ready' })
    await first
    expect(h.transport.calls.map((call) => call.name)).toEqual([commandFor(intent)])
  })

  it.each(['approve', 'cancel'] as const)('saves the current document before %s and keeps a real saved baseline', async (intent) => {
    const h = harness()
    h.edit('Keep this feedback even when ending the request')
    await h.run(intent)
    expect(h.transport.calls.map((call) => call.name)).toEqual(['saveFeedbackDraft', commandFor(intent)])
    expect(h.transport.callsFor('saveFeedbackDraft')[0].input.body_markdown).toBe('Keep this feedback even when ending the request')
    expect(get(h.draftSession)).toMatchObject({ dirty: false, phase: 'saved', savedBody: 'Keep this feedback even when ending the request' })
    expect(h.session.request()?.status).toBe(intent === 'approve' ? 'completed' : 'cancelled')
    expect(get(h.session).interactionLocked).toBe(false)
  })
})
