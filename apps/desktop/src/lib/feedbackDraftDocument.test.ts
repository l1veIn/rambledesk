import type { JSONContent } from '@tiptap/core'
import { describe, expect, it } from 'vitest'

import {
  decodeFeedbackDraftDocument,
  restoreFeedbackDraftDocument,
  snapshotFeedbackDraftDocument,
  snapshotFeedbackDraftMarkdown,
} from './feedbackDraftDocument'
import {
  CLEANUP_STATE_ATTR,
  INPUT_SOURCE_ATTR,
  SPEECH_SEGMENT_ID_ATTR,
} from './speechBlockMetadata'

describe('persisted feedback draft document', () => {
  it('restores node types, attrs, and marks that Markdown cannot represent', () => {
    const doc: JSONContent = {
      type: 'doc',
      content: [
        {
          type: 'paragraph',
          attrs: {
            [SPEECH_SEGMENT_ID_ATTR]: 'segment-1',
            [INPUT_SOURCE_ATTR]: 'asr',
            [CLEANUP_STATE_ATTR]: 'pending',
            actionIndex: 2,
          },
          content: [{ type: 'text', text: '按钮', marks: [{ type: 'bold' }] }],
        },
        {
          type: 'paragraph',
          attrs: {
            [SPEECH_SEGMENT_ID_ATTR]: 'segment-2',
            [INPUT_SOURCE_ATTR]: 'asr',
            [CLEANUP_STATE_ATTR]: 'cleaned',
            actionIndex: 3,
          },
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
    expect(snapshot.bodyMarkdown).not.toContain(SPEECH_SEGMENT_ID_ATTR)
    expect(snapshot.bodyMarkdown).not.toContain(CLEANUP_STATE_ATTR)
  })

  it('migrates v1 speech workflow nodes into ordinary ASR paragraphs', () => {
    const restored = decodeFeedbackDraftDocument(
      JSON.stringify({
        schemaVersion: 1,
        doc: {
          type: 'doc',
          content: [
            {
              type: 'pendingSpeech',
              attrs: { status: 'cleaning', actionIndex: 2 },
              content: [{ type: 'text', text: '还没整理' }],
            },
            {
              type: 'cleanedSpeech',
              content: [{ type: 'text', text: '已经整理' }],
            },
          ],
        },
      }),
    )

    expect(restored?.content).toMatchObject([
      {
        type: 'paragraph',
        attrs: {
          actionIndex: 2,
          [SPEECH_SEGMENT_ID_ATTR]: 'legacy-asr-1',
          [INPUT_SOURCE_ATTR]: 'asr',
          [CLEANUP_STATE_ATTR]: 'pending',
        },
      },
      {
        type: 'paragraph',
        attrs: {
          [SPEECH_SEGMENT_ID_ATTR]: 'legacy-asr-2',
          [INPUT_SOURCE_ATTR]: 'asr',
          [CLEANUP_STATE_ATTR]: 'cleaned',
        },
      },
    ])
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

  it('never rehydrates action dividers as visible text', () => {
    const markdown =
      '先说结论。\n\n------------------------ Action 2 ------------------------\n\n保存失败。'
    const restored = restoreFeedbackDraftDocument(null, markdown)

    const texts = (restored.content ?? []).map((node) =>
      (node.content ?? []).map((child) => child.text ?? '').join(''),
    )
    expect(texts).not.toContain('------------------------ Action 2 ------------------------')
    expect(restored.content?.[0]?.attrs?.actionIndex).toBeUndefined()
    expect(restored.content?.[1]?.content?.[0]?.text).toBe('保存失败。')
    expect(restored.content?.[1]?.attrs?.actionIndex).toBe(2)
  })

  it('also strips dividers left inside a persisted document JSON', () => {
    const markdown =
      '------------------------ Action 2 ------------------------\n\n保存失败。'
    const snapshot = snapshotFeedbackDraftMarkdown(markdown)
    const restored = decodeFeedbackDraftDocument(snapshot.documentJson)

    expect((restored?.content ?? []).map((node) => node.attrs?.actionIndex ?? null)).toEqual([
      2,
    ])
    expect(JSON.stringify(restored?.content)).not.toContain('Action 2')
  })
})
