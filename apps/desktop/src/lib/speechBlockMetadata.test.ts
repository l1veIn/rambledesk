import { describe, expect, it } from 'vitest'
import { getSchema } from '@tiptap/core'

import { feedbackEditorExtensions } from './feedbackEditorExtensions'
import {
  ASR_INPUT_SOURCE,
  CLEANUP_STATE_ATTR,
  INPUT_SOURCE_ATTR,
  SPEECH_SEGMENT_ID_ATTR,
  applySpeechCleanupResults,
  asrParagraphAttrs,
  speechCleanupCandidates,
} from './speechBlockMetadata'

describe('speech block metadata', () => {
  it('collects pending ASR paragraphs in document order', () => {
    const candidates = speechCleanupCandidates({
      type: 'doc',
      content: [
        {
          type: 'paragraph',
          attrs: asrParagraphAttrs('one', 'pending'),
          content: [{ type: 'text', text: '第一段' }],
        },
        {
          type: 'blockquote',
          content: [
            {
              type: 'paragraph',
              attrs: asrParagraphAttrs('two', 'pending'),
              content: [{ type: 'text', text: '第二段' }],
            },
          ],
        },
        {
          type: 'paragraph',
          attrs: asrParagraphAttrs('done', 'cleaned'),
          content: [{ type: 'text', text: '已整理' }],
        },
      ],
    })

    expect(candidates).toEqual([
      { segmentId: 'one', text: '第一段' },
      { segmentId: 'two', text: '第二段' },
    ])
  })

  it('ignores paragraphs that are not pending ASR', () => {
    expect(
      speechCleanupCandidates({
        type: 'doc',
        content: [
          {
            type: 'paragraph',
            attrs: {
              [INPUT_SOURCE_ATTR]: ASR_INPUT_SOURCE,
              [SPEECH_SEGMENT_ID_ATTR]: 'empty',
              [CLEANUP_STATE_ATTR]: 'pending',
            },
          },
          { type: 'paragraph', content: [{ type: 'text', text: '普通段落' }] },
        ],
      }),
    ).toEqual([])
  })

  it('deletes empty Tidy results and every blank flow paragraph', () => {
    const result = applySpeechCleanupResults(
      {
        type: 'doc',
        content: [
          { type: 'paragraph' },
          {
            type: 'paragraph',
            attrs: asrParagraphAttrs('filler', 'pending'),
            content: [{ type: 'text', text: '嗯，嗯嗯。' }],
          },
          { type: 'paragraph', content: [{ type: 'text', text: '保留正文' }] },
          { type: 'paragraph', content: [{ type: 'hardBreak' }] },
        ],
      },
      [{ segmentId: 'filler', originalText: '嗯，嗯嗯。', nextText: '' }],
    )

    expect(result.replacementsApplied).toBe(1)
    expect(result.document.content).toEqual([
      { type: 'paragraph', content: [{ type: 'text', text: '保留正文' }] },
    ])
  })

  it('writes non-empty Tidy results back as cleaned speech', () => {
    const result = applySpeechCleanupResults(
      {
        type: 'doc',
        content: [
          {
            type: 'paragraph',
            attrs: asrParagraphAttrs('note', 'pending'),
            content: [{ type: 'text', text: '嗯这个按钮没反应' }],
          },
        ],
      },
      [
        {
          segmentId: 'note',
          originalText: '嗯这个按钮没反应',
          nextText: '这个按钮没有反应。',
        },
      ],
    )

    expect(result.document.content).toEqual([
      {
        type: 'paragraph',
        attrs: asrParagraphAttrs('note', 'cleaned'),
        content: [{ type: 'text', text: '这个按钮没有反应。' }],
      },
    ])
  })

  it('removes blank lines inside Action groups without removing the title', () => {
    const result = applySpeechCleanupResults(
      {
        type: 'doc',
        content: [
          {
            type: 'blockquote',
            attrs: { actionId: 'action-1', actionIndex: 0 },
            content: [
              {
                type: 'paragraph',
                content: [{ type: 'text', text: '@Action 1 · Test it' }],
              },
              { type: 'paragraph' },
            ],
          },
        ],
      },
      [{ segmentId: 'missing', originalText: 'missing', nextText: 'missing' }],
    )

    expect(result.document.content?.[0]?.content).toEqual([
      {
        type: 'paragraph',
        content: [{ type: 'text', text: '@Action 1 · Test it' }],
      },
    ])
  })

  it('keeps one required paragraph when Tidy empties the whole document', () => {
    const result = applySpeechCleanupResults(
      {
        type: 'doc',
        content: [
          {
            type: 'paragraph',
            attrs: asrParagraphAttrs('only', 'pending'),
            content: [{ type: 'text', text: '呃。' }],
          },
        ],
      },
      [{ segmentId: 'only', originalText: '呃。', nextText: '' }],
    )

    expect(result.document).toEqual({ type: 'doc', content: [{ type: 'paragraph' }] })
    expect(() => getSchema(feedbackEditorExtensions()).nodeFromJSON(result.document)).not.toThrow()
  })
})
