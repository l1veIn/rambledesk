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

  it('checks whether it may start before taking the terminal lock', async () => {
    // The lock this operation takes is itself part of what the app reports as
    // "another terminal action is running". Consulting that after locking makes
    // submission invalidate itself and silently publish nothing, so the check
    // has to happen first — this asserts the order, not just the outcome.
    let workspace = workspaceView()
    const calls: string[] = []
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'submit_feedback') return completedRequest()
      if (command === 'read_published_feedback') return null
      return undefined
    })
    const base = {
      tr: (source: string) => source,
      messageFrom: (cause: unknown) => String(cause),
      isPreviewMode: () => false,
      setCompletedResult: vi.fn(),
      setPublishedFeedback: vi.fn(),
      setSavePhase: vi.fn(),
      setPageError: vi.fn(),
      getCanSubmit: () => true,
      getRambleCanExit: () => false,
      exitRamble: vi.fn(),
      awaitCaptureWork: vi.fn().mockResolvedValue(true),
      saveDraftNow: vi.fn(async () => true),
      getDraftBody: () => 'Ramble body.',
      getSavedRevision: () => 2,
      getCookingEnabled: () => false,
      getPreview: () => null,
      setPreview: vi.fn(),
      setCooking: vi.fn(),
      cookAndPublish: vi.fn(),
      setSubmitting: vi.fn(),
      setSubmitStage: vi.fn(),
      refreshNavigation: vi.fn(async () => undefined),
      showSubmittedToast: vi.fn(),
    }
    const controller = createPublisherController({
      ...base,
      getWorkspace: () => workspace,
      setWorkspace: (next) => {
        workspace = next
      },
      canStartTerminal: () => {
        calls.push('canStartTerminal')
        return true
      },
      lockTerminal: () => calls.push('lockTerminal'),
      unlockTerminal: () => calls.push('unlockTerminal'),
    })

    await controller.submitFeedback()

    expect(calls.indexOf('canStartTerminal')).toBeLessThan(calls.indexOf('lockTerminal'))
    expect(mocks.invoke).toHaveBeenCalledWith('submit_feedback', expect.anything())

    // And it declines without locking when another terminal action holds it.
    mocks.invoke.mockClear()
    let blockedWorkspace = workspaceView()
    const blockedCalls: string[] = []
    const blocked = createPublisherController({
      ...base,
      getWorkspace: () => blockedWorkspace,
      setWorkspace: (next) => {
        blockedWorkspace = next
      },
      canStartTerminal: () => false,
      lockTerminal: () => blockedCalls.push('lockTerminal'),
      unlockTerminal: () => blockedCalls.push('unlockTerminal'),
    })

    await blocked.submitFeedback()

    expect(blockedCalls).toEqual([])
    expect(mocks.invoke).not.toHaveBeenCalledWith('submit_feedback', expect.anything())
  })

  it('does not publish when a capture failed to persist', async () => {
    // A capture that never reached the store would be dropped for good once the
    // request goes terminal, so submission has to stop instead.
    let workspace = workspaceView()
    const setPageError = vi.fn()
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
      setPageError,
      getCanSubmit: () => true,
      getRambleCanExit: () => false,
      exitRamble: vi.fn(),
      awaitCaptureWork: vi.fn().mockResolvedValue(false),
      canStartTerminal: () => true,
      lockTerminal: vi.fn(),
      unlockTerminal: vi.fn(),
      saveDraftNow: vi.fn(async () => true),
      getDraftBody: () => 'Draft with a recording that never saved.',
      getSavedRevision: () => 4,
      getCookingEnabled: () => false,
      getPreview: () => null,
      setPreview: vi.fn(),
      setCooking: vi.fn(),
      cookAndPublish: vi.fn(),
      setSubmitting: vi.fn(),
      setSubmitStage: vi.fn(),
      refreshNavigation: vi.fn(async () => undefined),
      showSubmittedToast: vi.fn(),
    })

    await controller.submitFeedback()

    expect(mocks.invoke).not.toHaveBeenCalledWith('submit_feedback', expect.anything())
    expect(workspace.request.status).not.toBe('completed')
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
      awaitCaptureWork: vi.fn().mockResolvedValue(true),
      canStartTerminal: () => true,
      lockTerminal: vi.fn(),
      unlockTerminal: vi.fn(),
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
