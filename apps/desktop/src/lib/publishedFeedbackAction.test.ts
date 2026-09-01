import { afterEach, describe, expect, it, vi } from 'vitest'

import { TestApplicationTransport } from './application/testApplicationTransport'
import { createBrowserPublishedFeedbackAction } from './publishedFeedbackAction'

describe('browser published feedback action', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('downloads the safe authenticated projection without a server path', async () => {
    const transport = new TestApplicationTransport(undefined, { initiallyReady: true })
    transport.resolve('readPublishedFeedback', {
      manifest: {
        schema_version: 1,
        request_id: 'request-1',
        title: 'Request',
        host_id: 'codex',
        host_session_id: 'session-1',
        source_hint: null,
        submitted_at: '2026-09-01T00:00:00Z',
        source_revision: 1,
        draft_revision: 1,
        feedback_markdown: 'feedback.md',
        feedback_sha256: 'sha256',
        attachments: [],
      },
      markdown: '# Feedback',
    })
    const click = vi.fn()
    const anchor: Record<string, unknown> = { click }
    const createObjectURL = vi.fn(() => 'blob:safe-feedback')
    const revokeObjectURL = vi.fn()
    vi.stubGlobal('document', { createElement: vi.fn(() => anchor) })
    vi.stubGlobal('URL', { createObjectURL, revokeObjectURL })

    await createBrowserPublishedFeedbackAction(transport).run('request-1')

    expect(transport.calls).toEqual([
      { name: 'readPublishedFeedback', input: { request_id: 'request-1' } },
    ])
    expect(anchor.download).toBe('request-1.rambledesk-feedback.json')
    expect(anchor.href).toBe('blob:safe-feedback')
    expect(click).toHaveBeenCalledOnce()
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:safe-feedback')
  })
})
