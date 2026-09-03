import { describe, expect, it, vi } from 'vitest'

import type { FeedbackRequestView, FeedbackWorkspaceView } from '../feedback'
import { TestApplicationTransport } from '../application/testApplicationTransport'
import { createPublisherController } from './publisherController'

function workspaceView(): FeedbackWorkspaceView {
  return {
    request: {
      request_id: 'request-1',
      host_id: 'codex',
      host_session_id: 'session-1',
      source_hint: null,
      title: 'Review the work',
      what_happened: 'Need human feedback.',
      status: 'in_progress',
      resolution: null,
      allow_finish: false,
      final_summary: null,
      revision: 4,
      created_at: '2026-08-22T00:00:00Z',
      updated_at: '2026-08-22T00:00:00Z',
    },
    actions: [],
    context_refs: [],
    request_attachments: [],
    draft: {
      document_json: null,
      body_markdown: 'Original uncooked ramble.',
      saved_revision: 4,
      updated_at: '2026-08-22T00:00:00Z',
    },
    attachments: [],
    feedback: null,
  }
}

function completedRequest(): FeedbackRequestView {
  return {
    request_id: 'request-1',
    host_id: 'codex',
    host_session_id: 'session-1',
    status: 'completed',
    execution_mode: 'wait',
    created_at: '2026-08-22T00:00:00Z',
    updated_at: '2026-08-22T00:01:00Z',
    feedback: {
      available: true,
    },
    resolution: 'feedback_submitted',
    allow_finish: false,
    final_summary: null,
  }
}

describe('publisherController', () => {
  it('checks pending speech after stopping recording, before saving or publishing', async () => {
    let pending = false
    const transport = new TestApplicationTransport(undefined)
    const saveDraftNow = vi.fn(async () => true)
    const setPageError = vi.fn()
    const controller = createPublisherController({
      transport, tr: (source) => source, messageFrom: String,
      isPreviewMode: () => false, getWorkspace: workspaceView,
      setWorkspace: vi.fn(), setCompletedResult: vi.fn(), setPublishedFeedback: vi.fn(),
      setSavePhase: vi.fn(), setPageError,
      getCanSubmit: () => true, getRambleCanExit: () => true,
      exitRamble: async () => { pending = true },
      hasPendingSpeech: (requestId) => requestId === 'request-1' && pending,
      saveDraftNow, getDraftBody: () => 'Existing draft', getSavedRevision: () => 4,
      getCookingEnabled: () => false, getPreview: () => null, setPreview: vi.fn(),
      setCooking: vi.fn(), cookAndPublish: vi.fn(), setSubmitting: vi.fn(), setSubmitStage: vi.fn(),
      refreshNavigation: vi.fn(async () => {}), showSubmittedToast: vi.fn(),
    })
    await controller.submitFeedback()
    expect(setPageError).toHaveBeenCalledWith('Review the pending speech in the capsule before submitting feedback.')
    expect(saveDraftNow).not.toHaveBeenCalled()
    expect(transport.callsFor('submitFeedback')).toEqual([])
  })
  it('submits the read-only cooked preview without replacing the canonical draft', async () => {
    let workspace = workspaceView()
    const setPreview = vi.fn()
    const cookAndPublish = vi.fn()
    const transport = new TestApplicationTransport(undefined)
      .resolve('submitFeedback', completedRequest())
      .handle('readPublishedFeedback', () => {
        return {
          manifest: {
            schema_version: 1,
            request_id: 'request-1',
            title: 'Review the work',
            host_id: 'codex',
            host_session_id: 'session-1',
            source_hint: null,
            submitted_at: '2026-08-22T00:01:00Z',
            source_revision: 4,
            draft_revision: 4,
            feedback_markdown: 'feedback.md',
            feedback_sha256: 'sha256',
            attachments: [],
          },
          markdown: '## Operator Feedback\n\nEdited cooked draft.',
          uncooked_markdown: 'Original uncooked ramble.',
          attachment_paths: [],
        }
      })
    const controller = createPublisherController({
      transport,
      tr: (source) => source,
      messageFrom: (cause) => String(cause),
      isPreviewMode: () => false,
      getWorkspace: () => workspace,
      setWorkspace: (next) => {
        workspace = next
      },
      setCompletedResult: vi.fn(),
      setPublishedFeedback: vi.fn(),
      setSavePhase: vi.fn(),
      setPageError: vi.fn(),
      getCanSubmit: () => true,
      getRambleCanExit: () => false,
      exitRamble: vi.fn(),
      saveDraftNow: vi.fn(async () => true),
      getDraftBody: () => 'Original uncooked ramble.',
      getSavedRevision: () => 4,
      getCookingEnabled: () => true,
      getPreview: () => ({
        markdown: 'Cooked draft before operator edits.',
        original: 'Original uncooked ramble.',
        model: 'deepseek/deepseek-chat',
      }),
      setPreview,
      setCooking: vi.fn(),
      cookAndPublish,
      setSubmitting: vi.fn(),
      setSubmitStage: vi.fn(),
      refreshNavigation: vi.fn(async () => undefined),
      showSubmittedToast: vi.fn(),
    })

    await controller.submitFeedback()

    expect(cookAndPublish).not.toHaveBeenCalled()
    expect(workspace.request.status).toBe('completed')
    expect(workspace.request.resolution).toBe('feedback_submitted')
    expect(transport.callsFor('submitFeedback')).toEqual([
      {
        name: 'submitFeedback',
        input: {
        request_id: 'request-1',
        expected_revision: 4,
        cooked_markdown: 'Cooked draft before operator edits.',
        cooking_model: 'deepseek/deepseek-chat',
        uncooked_markdown: 'Original uncooked ramble.',
        },
      },
    ])
    expect(setPreview).toHaveBeenCalledWith(null)
  })
})
