import { get } from 'svelte/store'
import { describe, expect, it } from 'vitest'

import { snapshotFeedbackDraftDocument } from '../feedbackDraftDocument'
import { createDraftSession } from './draftSession'

function document(text: string) {
  return { type: 'doc', content: [{ type: 'paragraph', content: [{ type: 'text', text }] }] }
}

function draftFor(text: string, savedRevision = 3, updatedAt: string | null = '2026-09-09T00:00:00Z') {
  const snapshot = snapshotFeedbackDraftDocument(document(text))
  return {
    document_json: snapshot.documentJson,
    body_markdown: snapshot.bodyMarkdown,
    saved_revision: savedRevision,
    updated_at: updatedAt,
  }
}

describe('draft session', () => {
  it('adopts a server draft as both the current and the saved document', () => {
    const session = createDraftSession()
    const snapshot = session.adopt(draftFor('first note'))

    expect(snapshot).toEqual(session.snapshot())
    expect(session.savedSnapshot()).toEqual(snapshot)
    expect(session.isDirty()).toBe(false)
    expect(get(session).savedRevision).toBe(3)
    expect(get(session).phase).toBe('saved')
    expect(get(session).editorDocument).toMatchObject(document('first note'))
    expect(get(session).editorEpoch).toBe(1)
  })

  it('treats a draft that was never saved as idle and can skip the editor load', () => {
    const session = createDraftSession()
    session.adopt(draftFor('typed', 0, null))
    expect(get(session).phase).toBe('idle')
    const epoch = get(session).editorEpoch

    session.adopt(draftFor('server', 1, null), { loadEditor: false })

    expect(get(session).editorEpoch).toBe(epoch)
    expect(get(session).editorDocument).toMatchObject(document('typed'))
    expect(session.snapshot().bodyMarkdown).toContain('server')
  })

  it('marks unsaved edits and returns to saved when the edit matches the saved document', () => {
    const session = createDraftSession()
    const adopted = session.adopt(draftFor('original'))

    session.edit(snapshotFeedbackDraftDocument(document('edited')))
    expect(get(session).phase).toBe('unsaved')
    expect(session.isDirty()).toBe(true)

    session.edit(adopted)
    expect(get(session).phase).toBe('saved')
    expect(session.isDirty()).toBe(false)
  })

  it('accepts a save only when it still matches the current document', () => {
    const session = createDraftSession()
    session.adopt(draftFor('original'))
    const saving = snapshotFeedbackDraftDocument(document('edited'))
    session.edit(saving)

    session.beginSave()
    expect(get(session).phase).toBe('saving')

    session.acceptSaved(saving, 4)
    expect(get(session).phase).toBe('saved')
    expect(get(session).savedRevision).toBe(4)
    expect(session.savedSnapshot()).toEqual(saving)

    // A newer local edit stays unsaved when an older save lands.
    session.edit(snapshotFeedbackDraftDocument(document('newer')))
    session.acceptSaved(saving, 5)
    expect(get(session).phase).toBe('unsaved')
    expect(get(session).savedRevision).toBe(5)
  })

  it('reports a failed save without losing the pending document', () => {
    const session = createDraftSession()
    session.adopt(draftFor('original'))
    const pending = snapshotFeedbackDraftDocument(document('edited'))
    session.edit(pending)

    session.beginSave()
    session.failSave('storage unavailable')

    expect(get(session).phase).toBe('error')
    expect(get(session).message).toBe('storage unavailable')
    expect(session.snapshot()).toEqual(pending)
  })

  it('adopts the server document on reconcile unless local edits diverge', () => {
    const session = createDraftSession()
    session.adopt(draftFor('original', 1))

    // The server accepted exactly what this client already shows: adopt it.
    expect(session.reconcile(draftFor('original', 2))).toBe('adopted')
    expect(get(session).phase).toBe('saved')
    expect(get(session).savedRevision).toBe(2)

    const local = snapshotFeedbackDraftDocument(document('local edit'))
    session.edit(local)
    expect(session.reconcile(draftFor('server moved on', 3))).toBe('kept-local')
    expect(session.snapshot()).toEqual(local)
    expect(session.savedSnapshot().bodyMarkdown).toContain('server moved on')
    expect(get(session).phase).toBe('unsaved')
  })

  it('resets every field', () => {
    const session = createDraftSession()
    session.adopt(draftFor('original'))
    session.edit(snapshotFeedbackDraftDocument(document('edited')))

    session.reset()

    expect(get(session)).toMatchObject({
      body: '',
      documentJson: '',
      savedBody: '',
      savedDocumentJson: '',
      savedRevision: 0,
      phase: 'idle',
      message: '',
      editorDocument: null,
      editorEpoch: 0,
    })
  })
})
