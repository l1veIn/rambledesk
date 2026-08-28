import type { JSONContent } from '@tiptap/core'
import { describe, expect, it } from 'vitest'

import { ACTION_ID_ATTR, ACTION_INDEX_ATTR } from './actionBlockquote'
import {
  decodeFeedbackDraftDocument,
  restoreFeedbackDraftDocument,
  snapshotFeedbackDraftDocument,
  snapshotFeedbackDraftMarkdown,
} from './feedbackDraftDocument'
import {
  ASR_INPUT_SOURCE,
  CLEANUP_STATE_ATTR,
  INPUT_SOURCE_ATTR,
  SPEECH_SEGMENT_ID_ATTR,
} from './speechBlockMetadata'

describe('persisted feedback draft document', () => {
  it('round-trips Action attrs, ASR attrs, attachments, tables, and task lists', () => {
    const doc: JSONContent = {
      type: 'doc',
      content: [
        {
          type: 'blockquote',
          attrs: { [ACTION_ID_ATTR]: 'login', [ACTION_INDEX_ATTR]: 0 },
          content: [
            {
              type: 'paragraph',
              content: [
                { type: 'text', text: '@Action 1 · 修复登录状态', marks: [{ type: 'bold' }] },
              ],
            },
            {
              type: 'paragraph',
              attrs: {
                [INPUT_SOURCE_ATTR]: ASR_INPUT_SOURCE,
                [SPEECH_SEGMENT_ID_ATTR]: 'seg-1',
                [CLEANUP_STATE_ATTR]: 'pending',
              },
              content: [{ type: 'text', text: '登录后状态没有更新。' }],
            },
          ],
        },
        {
          type: 'image',
          attrs: {
            src: 'blob:http://localhost/ephemeral-preview',
            attachmentId: 'abc-123',
            alt: 'shot.png',
          },
        },
        {
          type: 'table',
          content: [
            {
              type: 'tableRow',
              content: [
                {
                  type: 'tableHeader',
                  content: [{ type: 'paragraph', content: [{ type: 'text', text: 'Area' }] }],
                },
                {
                  type: 'tableHeader',
                  content: [{ type: 'paragraph', content: [{ type: 'text', text: 'Verdict' }] }],
                },
              ],
            },
          ],
        },
        {
          type: 'taskList',
          content: [
            {
              type: 'taskItem',
              attrs: { checked: false },
              content: [{ type: 'paragraph', content: [{ type: 'text', text: 'Follow up' }] }],
            },
          ],
        },
      ],
    }

    const snapshot = snapshotFeedbackDraftDocument(doc)
    const restored = restoreFeedbackDraftDocument(snapshot.documentJson, 'wrong fallback')

    expect(restored.content?.[0]).toMatchObject({
      type: 'blockquote',
      attrs: { [ACTION_ID_ATTR]: 'login', [ACTION_INDEX_ATTR]: 0 },
    })
    expect(restored.content?.[0].content?.[1].attrs).toMatchObject({
      [SPEECH_SEGMENT_ID_ATTR]: 'seg-1',
      [CLEANUP_STATE_ATTR]: 'pending',
    })
    expect(decodeFeedbackDraftDocument(snapshot.documentJson)?.content?.[1].attrs?.src).toBe(
      'attachment://abc-123',
    )
    expect(snapshot.documentJson).not.toContain('ephemeral-preview')
    expect(snapshot.bodyMarkdown).toContain('@Action 1 · 修复登录状态')
    expect(snapshot.bodyMarkdown).toContain('Follow up')
  })

  it('wraps adjacent v1 Action stamps into separate Blockquote containers', () => {
    const v1 = JSON.stringify({
      schemaVersion: 1,
      doc: {
        type: 'doc',
        content: [
          {
            type: 'paragraph',
            attrs: { actionIndex: 1 },
            content: [{ type: 'text', text: '第一次。' }],
          },
          {
            type: 'paragraph',
            attrs: { actionIndex: 2 },
            content: [{ type: 'text', text: '中间。' }],
          },
          {
            type: 'paragraph',
            attrs: { actionIndex: 1 },
            content: [{ type: 'text', text: '再次打开。' }],
          },
        ],
      },
    })

    const restored = restoreFeedbackDraftDocument(v1, 'fallback')
    expect(restored.content?.map((node) => node.type)).toEqual([
      'blockquote',
      'blockquote',
      'blockquote',
    ])
    expect(restored.content?.map((node) => node.attrs?.[ACTION_INDEX_ATTR])).toEqual([0, 1, 0])
    expect(restored.content?.[0].content?.[0].content?.[0]?.text).toBe('@Action 1')
    expect(restored.content?.[0].content?.[1].content?.[0]?.text).toBe('第一次。')
    expect(restored.content?.[2].content?.[1].content?.[0]?.text).toBe('再次打开。')
  })

  it('hydrates markdown-only drafts and writes v2 on snapshot', () => {
    const snapshot = snapshotFeedbackDraftMarkdown('**Legacy**')
    const parsed = JSON.parse(snapshot.documentJson) as { schemaVersion: number }
    expect(parsed.schemaVersion).toBe(2)
    expect(
      restoreFeedbackDraftDocument(null, '**Legacy**').content?.[0].content?.[0],
    ).toMatchObject({
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
