import { get } from 'svelte/store'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { ApplicationTransport } from '../application/applicationTransport'
import type { FeedbackRequestSummary, FeedbackWorkspaceView } from '../feedback'
import type { ResumePrompt } from '../domain/resumePrompt'
import {
  createResumePromptController,
  RESUME_PROMPT_STREAM,
  type ResumePromptControllerContext,
} from './resumePromptController'

const prompt: ResumePrompt = {
  request_id: 'request-1',
  host_id: 'codex',
  host_label: 'Codex',
  title: 'Return to Codex',
  body: 'The package is ready.',
  resume_prompt: 'Continue the original task.',
  reason: 'completed',
}

function request(overrides: Partial<FeedbackRequestSummary> = {}): FeedbackRequestSummary {
  return {
    request_id: 'request-1',
    host_id: 'codex',
    host_session_id: 'alpha',
    source_hint: 'Workbench',
    title: 'Review',
    what_happened: 'Something happened.',
    status: 'completed',
    resolution: 'feedback_submitted',
    allow_finish: false,
    final_summary: null,
    revision: 1,
    created_at: '2026-09-08T00:00:00Z',
    updated_at: '2026-09-08T00:01:00Z',
    ...overrides,
  }
}

function workspace(overrides: Partial<FeedbackRequestSummary> = {}): FeedbackWorkspaceView {
  return {
    request: request(overrides),
    actions: [],
    context_refs: [],
    request_attachments: [],
    draft: { document_json: null, body_markdown: 'body', saved_revision: 1, updated_at: null },
    attachments: [],
    feedback: null,
  } as FeedbackWorkspaceView
}

function harness(overrides: Partial<ResumePromptControllerContext> = {}) {
  let streamHandler: ((prompt: ResumePrompt) => void) | undefined
  const transport = {
    call: vi.fn(async () => workspace()),
    subscribe: vi.fn((_stream: unknown, handler: (prompt: ResumePrompt) => void) => {
      streamHandler = handler
      return () => {}
    }),
  } as unknown as ApplicationTransport
  const send = vi.fn(async () => undefined)
  const context = {
    transport,
    tr: (source: string) => source,
    messageFrom: (cause: unknown) => String(cause),
    getCurrentRequest: () => null,
    getKnownRequests: () => [],
    getWorkspace: () => workspace(),
    resolveHostProfile: (hostId: string) => ({ id: hostId, label: 'Codex' }) as never,
    canOpenFromWorkspace: () => true,
    notifications: {
      available: true,
      isMac: true,
      getPopupEnabled: () => true,
      getState: () => 'enabled' as const,
      send,
    },
    setPageError: vi.fn(),
    copyText: vi.fn(async () => undefined),
    ...overrides,
  } as ResumePromptControllerContext
  return {
    controller: createResumePromptController(context),
    context,
    transport,
    send,
    emit: (value: ResumePrompt) => streamHandler?.(value),
  }
}

afterEach(() => {
  vi.useRealTimers()
})

describe('resume prompt controller', () => {
  it('presents a prompt after loading an unknown request', async () => {
    const { controller, emit, transport, send } = harness()
    const unsubscribe = controller.subscribeStream()
    expect(transport.subscribe).toHaveBeenCalledWith(
      RESUME_PROMPT_STREAM,
      expect.any(Function),
      expect.any(Function),
    )

    emit(prompt)
    await vi.waitFor(() => expect(get(controller).prompt).toEqual(prompt))
    expect(send).toHaveBeenCalledWith({
      title: 'Return to Codex',
      body: 'Return to {host} and use the resume prompt to continue the host session.',
    })
    unsubscribe()
  })

  it('ignores prompts for managed sessions', async () => {
    const { controller, emit } = harness({
      getKnownRequests: () => [request({ managed_session_id: 'managed-1' })],
    })
    controller.subscribeStream()
    emit(prompt)
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(get(controller).prompt).toBeNull()
  })

  it('reports a failed lookup without showing the dialog', async () => {
    const { controller, context, emit } = harness()
    ;(context.transport.call as ReturnType<typeof vi.fn>).mockRejectedValue(
      new Error('request missing'),
    )
    controller.subscribeStream()
    emit(prompt)
    await vi.waitFor(() => expect(context.setPageError).toHaveBeenCalledWith('Error: request missing'))
    expect(get(controller).prompt).toBeNull()
  })

  it('builds a prompt from the open workspace', () => {
    const { controller } = harness()
    controller.open()
    expect(get(controller).prompt).toMatchObject({ request_id: 'request-1', reason: 'completed' })
  })

  it('does not open without a workspace or when the button is hidden', () => {
    const { controller } = harness({ getWorkspace: () => null })
    controller.open()
    expect(get(controller).prompt).toBeNull()

    const hidden = harness({ canOpenFromWorkspace: () => false })
    hidden.controller.open()
    expect(get(hidden.controller).prompt).toBeNull()
  })

  it('copies the prompt and resets the copy state', async () => {
    vi.useFakeTimers()
    const copyText = vi.fn(async () => undefined)
    const { controller } = harness({ copyText })
    controller.show(prompt)

    await controller.copy()
    expect(copyText).toHaveBeenCalledWith('Continue the original task.')
    expect(get(controller).copyState).toBe('copied')

    await vi.advanceTimersByTimeAsync(2_000)
    expect(get(controller).copyState).toBe('idle')
  })

  it('reports a failed copy', async () => {
    const { controller } = harness({ copyText: vi.fn(async () => { throw new Error('denied') }) })
    controller.show(prompt)
    await controller.copy()
    expect(get(controller).copyState).toBe('failed')
  })

  it('dismisses and disposes cleanly', () => {
    const { controller } = harness()
    controller.show(prompt)
    controller.dismiss()
    expect(get(controller)).toEqual({ prompt: null, copyState: 'idle' })
    controller.dispose()
  })
})
