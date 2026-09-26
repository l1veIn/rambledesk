import { describe, expect, it, vi } from 'vitest'

import type { DraftView, FeedbackWorkspaceView, SaveDraftInput } from './feedback'
import { ACTION_ID_ATTR } from './actionBlockquote'
import { collectActionGroupContent } from './actionGroupContent'
import { writeBackgroundDraftOperation, writeBackgroundSpeech } from './backgroundDraftWriter'
import type { SpeechWriteInput } from './speech/speechWriteback'
import { restoreFeedbackDraftDocument, snapshotFeedbackDraftDocument } from './feedbackDraftDocument'

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

function reviewWorkspace(body = 'Existing'): FeedbackWorkspaceView {
  const value = workspace({ document_json: empty.documentJson, body_markdown: '', saved_revision: 1, updated_at: '' })
  value.workbench = { type: 'document_review', version: 1, data: { title: 'Script', source_version: 'v1', paragraphs: [{ id: 'p', text: 'Immutable source' }] } }
  value.draft.document_json = JSON.stringify({ ...JSON.parse(empty.documentJson),
    workbenchState: { type: 'document_review', verdict: null, paragraph_marks: [], annotations: [
      { id: 'a', paragraph_id: 'p', start: null, end: null, quote: null, kind: 'comment', body, replacement: null },
    ] },
  })
  return value
}
const reviewSpeech: SpeechWriteInput = { requestId: 'request-a', requestTitle: 'Title', id: 'speech-1', text: 'Spoken comment', destination: {
  kind: 'review_annotation', annotationId: 'a', field: 'body', sourceVersion: 'v1', paragraphLabel: 'Opening',
} }

describe('background annotation speech writer', () => {
  it('reapplies against concurrent edits after a CAS conflict without overwriting them', async () => {
    let current = reviewWorkspace()
    const save = vi.fn(async (input: SaveDraftInput): Promise<DraftView> => {
      if (save.mock.calls.length === 1) {
        current = reviewWorkspace('Concurrent manual note')
        current.draft.saved_revision = 2
        throw { code: 'DRAFT_CONFLICT', message: 'stale' }
      }
      current.draft = { ...current.draft, document_json: input.document_json, body_markdown: input.body_markdown, saved_revision: input.expected_revision + 1 }
      return current.draft
    })
    const load = vi.fn(async () => current)
    const saved = await writeBackgroundSpeech(reviewSpeech, { load, save })
    expect(JSON.parse(saved.document_json!).workbenchState.annotations[0].body).toBe('Concurrent manual note\nSpoken comment')
    expect(save.mock.calls.map(([input]) => input.expected_revision)).toEqual([1, 2])
    expect(load).toHaveBeenCalledTimes(2)
  })

  it('deduplicates a committed annotation write after its acknowledgement was lost and the writer reloaded', async () => {
    const current = reviewWorkspace()
    const save = vi.fn(async (input: SaveDraftInput): Promise<DraftView> => {
      current.draft = { ...current.draft, document_json: input.document_json, body_markdown: input.body_markdown, saved_revision: input.expected_revision + 1 }
      throw new Error('Acknowledgement lost')
    })
    await expect(writeBackgroundSpeech(reviewSpeech, { load: async () => current, save })).rejects.toThrow('Acknowledgement lost')
    const restored = JSON.parse(JSON.stringify(current)) as FeedbackWorkspaceView
    const saved = await writeBackgroundSpeech(reviewSpeech, { load: async () => restored, save })
    expect(saved.saved_revision).toBe(2)
    expect(JSON.parse(saved.document_json!).workbenchState.annotations[0].body).toBe('Existing\nSpoken comment')
    expect(save).toHaveBeenCalledTimes(1)
  })

  it('rejects a removed annotation after conflict instead of falling back to the document', async () => {
    let current = reviewWorkspace()
    const save = vi.fn(async () => {
      current = reviewWorkspace()
      const envelope = JSON.parse(current.draft.document_json!)
      envelope.workbenchState.annotations = []
      current.draft.document_json = JSON.stringify(envelope)
      current.draft.saved_revision = 2
      throw { code: 'DRAFT_CONFLICT', message: 'stale' }
    })
    await expect(writeBackgroundSpeech(reviewSpeech, { load: async () => current, save })).rejects.toThrow('no longer exists')
    expect(save).toHaveBeenCalledTimes(1)
    expect(current.draft.body_markdown).toBe('')
  })
})

describe('background draft writer', () => {
  it('persists a Task tab action and its feedback for the Session editor to reload', async () => {
    let persisted: DraftView = {
      document_json: empty.documentJson,
      body_markdown: empty.bodyMarkdown,
      saved_revision: 1,
      updated_at: '',
    }
    const writer = {
      load: async () => workspace(persisted),
      save: vi.fn(async (input: SaveDraftInput): Promise<DraftView> => {
        persisted = {
          document_json: input.document_json,
          body_markdown: input.body_markdown,
          saved_revision: input.expected_revision + 1,
          updated_at: '',
        }
        return persisted
      }),
    }
    const action = { actionId: 'verify-startup', actionIndex: 0, title: 'Verify startup' }

    await writeBackgroundDraftOperation('request-a', { kind: 'startActionGroup', action }, writer)
    const selected = restoreFeedbackDraftDocument(persisted.document_json, persisted.body_markdown)
    expect(selected.content?.[0].attrs?.[ACTION_ID_ATTR]).toBe(action.actionId)

    await writeBackgroundDraftOperation('request-a', {
      kind: 'appendSpeech', segmentId: 'task-tab-speech', text: 'Startup works.', action,
    }, writer)
    await writeBackgroundDraftOperation('request-a', {
      kind: 'clearActionGroup', actionId: action.actionId,
    }, writer)

    const reopened = await writer.load()
    const document = restoreFeedbackDraftDocument(reopened.draft.document_json, reopened.draft.body_markdown)
    const feedback = collectActionGroupContent(document).get(action.actionId)
    expect(feedback?.groupCount).toBe(1)
    expect(JSON.stringify(feedback?.document)).toContain('Startup works.')
    expect(writer.save.mock.calls.map(([input]) => input.request_id)).toEqual(['request-a', 'request-a'])
  })

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
