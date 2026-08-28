import { describe, expect, it, vi } from 'vitest'

import type { DraftView, FeedbackWorkspaceView } from './feedback'
import { writeBackgroundDraftOperation } from './backgroundDraftWriter'
import { snapshotFeedbackDraftDocument } from './feedbackDraftDocument'

function workspace(draft: DraftView): FeedbackWorkspaceView {
  return {
    request: {
      request_id: 'request-a',
      host_id: 'host',
      host_session_id: 'session',
      source_hint: null,
      title: 'Title',
      what_happened: 'Context',
      status: 'in_progress',
      resolution: null,
      allow_finish: false,
      final_summary: null,
      revision: draft.saved_revision,
      created_at: '',
      updated_at: '',
    },
    actions: [],
    context_refs: [],
    attachments: [],
    request_attachments: [],
    draft,
    feedback: null,
  }
}

const empty = snapshotFeedbackDraftDocument({ type: 'doc', content: [] })

describe('background draft writer', () => {
  it('reloads and reapplies an idempotent operation after a CAS conflict', async () => {
    const initial = workspace({
      document_json: empty.documentJson,
      body_markdown: empty.bodyMarkdown,
      saved_revision: 1,
      updated_at: '',
    })
    const concurrent = workspace({ ...initial.draft, saved_revision: 2 })
    const load = vi.fn()
      .mockResolvedValueOnce(initial)
      .mockResolvedValueOnce(concurrent)
    const save = vi.fn()
      .mockRejectedValueOnce({ code: 'DRAFT_CONFLICT', message: 'stale' })
      .mockImplementationOnce(async (input) => ({
        document_json: input.document_json,
        body_markdown: input.body_markdown,
        saved_revision: 3,
        updated_at: '',
      }))

    const saved = await writeBackgroundDraftOperation(
      'request-a',
      { kind: 'appendSpeech', segmentId: 'asr-session-0', text: '内容', action: null },
      { load, save },
    )
    expect(saved.saved_revision).toBe(3)
    expect(load).toHaveBeenCalledTimes(2)
    expect(save).toHaveBeenCalledTimes(2)
    expect(save.mock.calls[1]![0].expected_revision).toBe(2)
  })

  it('does not save again when the retried operation is already present', async () => {
    const operation = {
      kind: 'appendSpeech' as const,
      segmentId: 'asr-session-0',
      text: '内容',
      action: null,
    }
    const firstSave = vi.fn()
    let persisted: DraftView | null = null
    const writer = {
      load: async () =>
        workspace(
          persisted ?? {
            document_json: empty.documentJson,
            body_markdown: empty.bodyMarkdown,
            saved_revision: 1,
            updated_at: '',
          },
        ),
      save: firstSave.mockImplementation(async (input) => {
        persisted = {
          document_json: input.document_json,
          body_markdown: input.body_markdown,
          saved_revision: 2,
          updated_at: '',
        }
        return persisted
      }),
    }
    await writeBackgroundDraftOperation('request-a', operation, writer)
    await writeBackgroundDraftOperation('request-a', operation, writer)
    expect(firstSave).toHaveBeenCalledTimes(1)
  })
})
