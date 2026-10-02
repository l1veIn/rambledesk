import { get } from 'svelte/store'
import { describe, expect, it, vi } from 'vitest'
import { TestApplicationTransport } from '../../application/testApplicationTransport'
import { createUnavailableWorkbenchCapabilities } from '../../capabilities/unavailableCapabilities'
import type { FeedbackWorkspaceView } from '../../feedback'
import { snapshotFeedbackDraftMarkdown } from '../../feedbackDraftDocument'
import { previewFixtures } from '../../preview/previewFixtures'
import { readWorkbenchState, withWorkbenchState } from '../../workbenchState'
import { createAttachmentController } from '../attachmentController'
import { createAttachmentSession } from '../attachmentSession'
import { createCookingSession } from '../cookingSession'
import { createDraftController } from '../draftController'
import { createDraftSession } from '../draftSession'
import { createPublisherController } from '../publisherController'
import { createWorkbenchLifecycle } from '../workbenchLifecycle'
import { createWorkspaceSession } from '../workspaceSession'

// Canvas encoding is the browser boundary; all publication, state and queue controllers are real.
const { exportPng } = vi.hoisted(() => ({ exportPng: vi.fn(async () => new Uint8Array([137, 80, 78, 71]).buffer) }))
vi.mock('./visualExport', () => ({ exportVisualPng: exportPng, loadVisualImage: vi.fn() }))

describe('notes-only visual publication', () => {
  it('prepares a missing state through the publisher and CAS attachment queue before its final save', async () => {
    let backend: FeedbackWorkspaceView = { ...structuredClone(previewFixtures.workspace),
      workbench: { type: 'visual_feedback', version: 1, data: { title: 'Blank canvas', source_version: 'v1', width: 640, height: 400, image_file_name: null } },
      request_attachments: [], attachments: [], feedback: null,
      draft: { document_json: null, body_markdown: '', saved_revision: 0, updated_at: null } }
    const transport = new TestApplicationTransport()
      .handle('getFeedbackWorkspace', () => structuredClone(backend))
      .handle('saveFeedbackDraft', (input) => {
        expect(input.expected_revision).toBe(backend.draft.saved_revision)
        backend = { ...backend, draft: { document_json: input.document_json, body_markdown: input.body_markdown,
          saved_revision: input.expected_revision + 1, updated_at: '2026-10-02T00:00:00Z' } }
        return structuredClone(backend.draft)
      })
      .handle('addFeedbackAttachment', (input) => {
        expect(input.expected_revision).toBe(backend.draft.saved_revision)
        backend = { ...backend, draft: { ...backend.draft, saved_revision: input.expected_revision + 1 }, attachments: [{
          attachment_id: 'composite_png', file_name: input.file_name, media_type: 'image/png',
          byte_size: input.contents.byteLength, sha256: 'preview', position: 0 }] }
        return structuredClone(backend)
      })
      .handle('submitFeedback', (input) => {
        expect(input.expected_revision).toBe(backend.draft.saved_revision)
        expect(readWorkbenchState(backend.draft.document_json)).toEqual({ type: 'visual_feedback', annotations: [], composite_attachment_id: 'composite_png' })
        return { request_id: input.request_id, host_id: backend.request.host_id, host_session_id: backend.request.host_session_id,
          status: 'completed', execution_mode: 'wait', created_at: backend.request.created_at, updated_at: '2026-10-02T00:01:00Z',
          feedback: { available: true }, resolution: 'feedback_submitted', allow_finish: false, final_summary: null }
      }).resolve('readPublishedFeedback', null)
    const session = createWorkspaceSession(), draft = createDraftSession(), attachments = createAttachmentSession()
    session.open(backend); draft.adopt(backend.draft)
    const workspace = () => get(session).workspace
    const drafts = createDraftController({ transport, session: draft, messageFrom: String,
      isInteractionLocked: () => get(session).interactionLocked, isWorkspaceTerminal: () => get(session).terminal,
      getWorkspace: workspace, setWorkspaceDraft: session.setDraft })
    const attachmentController = createAttachmentController({ transport, capabilities: createUnavailableWorkbenchCapabilities(),
      tr: (text) => text, messageFrom: String, getWorkspace: workspace, getEditor: () => undefined,
      getRambleRequestId: () => '', getInteractionLocked: () => get(session).interactionLocked,
      getSavedRevision: () => get(draft).savedRevision, session: attachments, saveDraftNow: drafts.saveDraftNow,
      waitForRambleMarkdown: async () => {}, routeDraftOperation: async () => {}, activeActionFor: () => null,
      applyWorkspaceMutation: (next) => { session.replace(next); draft.reconcile(next.draft) } })
    const lifecycle = createWorkbenchLifecycle({ transport, getWorkspace: workspace,
      getState: () => readWorkbenchState(get(draft).documentJson),
      updateState: (state) => drafts.updateDraft(withWorkbenchState(snapshotFeedbackDraftMarkdown(get(draft).body), state)),
      isEditable: () => !get(session).terminal && !get(session).interactionLocked, onBusy: () => {},
      persistGeneratedAttachment: attachmentController.persistGeneratedAttachment })
    const error = vi.fn(), publisher = createPublisherController({ transport, session, draft, cooking: createCookingSession(),
      tr: (text) => text, messageFrom: String, setPageError: error, isReadOnly: () => false,
      prepareFeedback: async (requestId) => {
        const result = await attachmentController.prepareFeedback(requestId)
        return result.kind === 'pending-capture' ? { kind: 'failed', message: 'Capture pending' } : result
      }, prepareWorkbench: lifecycle.prepareSubmission,
      saveDraftNow: drafts.saveDraftNow, getCookingEnabled: () => false, cookSubmission: async () => { throw new Error('unused') },
      refreshNavigation: async () => {}, showSubmittedToast: () => {} })
    try {
      drafts.updateDraft(snapshotFeedbackDraftMarkdown('Keep the layout and increase its spacing.'))
      expect(readWorkbenchState(get(draft).documentJson)).toBeNull()
      await publisher.submitFeedback()
      expect(error.mock.calls.every(([message]) => message === '')).toBe(true); expect(transport.callsFor('submitFeedback')).toHaveLength(1)
      expect(exportPng).toHaveBeenCalledWith(backend.workbench!.data, [], null, [])
      expect(backend.draft.body_markdown).toBe('Keep the layout and increase its spacing.')
      expect(transport.calls.filter(({ name }) => ['saveFeedbackDraft', 'addFeedbackAttachment', 'submitFeedback'].includes(name)).map(({ name }) => name))
        .toEqual(['saveFeedbackDraft', 'addFeedbackAttachment', 'saveFeedbackDraft', 'submitFeedback'])
      expect(session.isTerminal()).toBe(true)
    } finally { drafts.cancelPendingSave(); lifecycle.dispose() }
  })
})
