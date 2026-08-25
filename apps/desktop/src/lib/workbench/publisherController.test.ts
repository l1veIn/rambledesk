import { beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))

import type { FeedbackRequestView, FeedbackWorkspaceView } from '../feedback'
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
      body_markdown: 'Edited cooked draft.',
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
      package_uri: 'file:///tmp/package',
      directory_path: '/tmp/package',
      markdown_path: '/tmp/package/feedback.md',
      manifest_path: '/tmp/package/manifest.json',
    },
    resolution: 'feedback_submitted',
    allow_finish: false,
    final_summary: null,
  }
}

describe('publisherController', () => {
  beforeEach(() => {
    mocks.invoke.mockReset()
  })

  it('submits an edited cooked draft without cooking again', async () => {
    let workspace = workspaceView()
    const setPreview = vi.fn()
    const cookAndPublish = vi.fn()
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'submit_feedback') return completedRequest()
      if (command === 'read_published_feedback') {
        return {
          markdown: '## Operator Feedback\n\nEdited cooked draft.',
          uncooked_markdown: 'Original uncooked ramble.',
        }
      }
      return undefined
    })
    const controller = createPublisherController({
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
    awaitCaptureWork: vi.fn(),
      saveDraftNow: vi.fn(async () => true),
      getDraftBody: () => 'Edited cooked draft.',
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
    expect(mocks.invoke).toHaveBeenCalledWith('submit_feedback', {
      input: {
        request_id: 'request-1',
        expected_revision: 4,
        cooked_markdown: 'Edited cooked draft.',
        cooking_model: 'deepseek/deepseek-chat',
        uncooked_markdown: 'Original uncooked ramble.',
      },
    })
    expect(setPreview).toHaveBeenCalledWith(null)
  })
})
