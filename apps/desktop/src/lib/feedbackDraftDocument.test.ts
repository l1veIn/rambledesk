import type { JSONContent } from '@tiptap/core'
import { describe, expect, it } from 'vitest'

import {
  decodeFeedbackDraftDocument,
  restoreFeedbackDraftDocument,
  snapshotFeedbackDraftDocument,
} from './feedbackDraftDocument'
import { CLEANED_SPEECH_NODE, PENDING_SPEECH_NODE } from './pendingSpeech'

describe('persisted feedback draft document', () => {
  it('restores node types, attrs, and marks that Markdown cannot represent', () => {
    const doc: JSONContent = {
      type: 'doc',
      content: [
        {
          type: PENDING_SPEECH_NODE,
          attrs: { status: 'pending', actionIndex: 2 },
          content: [{ type: 'text', text: '按钮', marks: [{ type: 'bold' }] }],
        },
        {
          type: CLEANED_SPEECH_NODE,
          attrs: { actionIndex: 3 },
          content: [{ type: 'text', text: '按钮太小了。' }],
        },
      ],
    }

    const snapshot = snapshotFeedbackDraftDocument(doc)
    const restored = restoreFeedbackDraftDocument(snapshot.documentJson, 'wrong fallback')

    expect(restored).toEqual(doc)
    expect(snapshot.bodyMarkdown).toContain(
      '------------------------ Action 2 ------------------------',
    )
    expect(snapshot.bodyMarkdown).not.toContain(PENDING_SPEECH_NODE)
    expect(snapshot.bodyMarkdown).not.toContain(CLEANED_SPEECH_NODE)
  })

  it('stores canonical attachment identities instead of ephemeral preview URLs', () => {
    const snapshot = snapshotFeedbackDraftDocument({
      type: 'doc',
      content: [
        {
          type: 'image',
          attrs: {
            src: 'blob:http://localhost/ephemeral-preview',
            attachmentId: 'abc-123',
            alt: 'shot.png',
          },
        },
      ],
    })

    const restored = decodeFeedbackDraftDocument(snapshot.documentJson)
    expect(restored?.content?.[0].attrs?.src).toBe('attachment://abc-123')
    expect(snapshot.documentJson).not.toContain('ephemeral-preview')
  })

  it('does not persist the current Action staging attr on an empty paragraph', () => {
    const snapshot = snapshotFeedbackDraftDocument({
      type: 'doc',
      content: [{ type: 'paragraph', attrs: { actionIndex: 2 } }],
    })

    expect(decodeFeedbackDraftDocument(snapshot.documentJson)?.content?.[0].attrs).toBeUndefined()
  })

  it('hydrates legacy and unsupported documents from their Markdown projection', () => {
    expect(restoreFeedbackDraftDocument(null, '**Legacy**').content?.[0].content?.[0]).toMatchObject({
      text: 'Legacy',
      marks: [{ type: 'bold' }],
    })
    expect(
      restoreFeedbackDraftDocument(
        JSON.stringify({ schemaVersion: 999, doc: { type: 'doc' } }),
        'Fallback',
      ).content?.[0].content?.[0]?.text,
    ).toBe('Fallback')
  })
})
