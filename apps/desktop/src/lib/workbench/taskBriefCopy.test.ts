import { describe, expect, it } from 'vitest'

import type { FeedbackWorkspaceView } from '../feedback'
import { buildTaskBriefText } from './taskBriefCopy'

function workspace(overrides: Partial<FeedbackWorkspaceView> = {}): FeedbackWorkspaceView {
  return {
    request: {
      request_id: 'req-1',
      title: 'Test the ramble',
      status: 'in_progress',
      what_happened: 'Press the record button.',
      created_at: '2026-08-27T00:00:00Z',
      updated_at: '2026-08-27T00:00:00Z',
      host_id: 'dsh',
      host_session_id: 'sess-1',
      source_hint: '',
      resolution: null,
      allow_finish: false,
      final_summary: null,
      revision: 1,
    },
    actions: [
      { id: 'a1', instruction: 'Start a voice Ramble' },
      { id: 'a2', instruction: 'Insert a screenshot' },
    ],
    context_refs: [{ label: 'Spec', uri: 'file:///tmp/spec.md' }],
    request_attachments: [
      { attachment_id: 'att-1', file_name: 'notes.md', media_type: 'text/markdown', byte_size: 2048 },
    ],
    ...overrides,
  } as FeedbackWorkspaceView
}

describe('buildTaskBriefText', () => {
  it('includes title, what happened, numbered actions, refs, and files', () => {
    const text = buildTaskBriefText(workspace())
    expect(text).toContain('Test the ramble')
    expect(text).toContain('Press the record button.')
    expect(text).toContain('1. Start a voice Ramble')
    expect(text).toContain('2. Insert a screenshot')
    expect(text).toContain('Spec: file:///tmp/spec.md')
    expect(text).toContain('notes.md (2.0 KiB)')
  })

  it('strips empty sections', () => {
    const text = buildTaskBriefText(
      workspace({
        actions: [],
        context_refs: [],
        request_attachments: [],
        request: {
          request_id: 'req-1',
          title: 'Only a title',
          status: 'in_progress',
          what_happened: '',
          created_at: '2026-08-27T00:00:00Z',
          updated_at: '2026-08-27T00:00:00Z',
          host_id: 'dsh',
          host_session_id: 'sess-1',
          source_hint: '',
          resolution: null,
          allow_finish: false,
          final_summary: null,
          revision: 1,
        },
      }),
    )
    expect(text).toBe('Only a title')
  })
})
