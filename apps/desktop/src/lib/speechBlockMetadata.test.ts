import { describe, expect, it } from 'vitest'

import {
  ASR_INPUT_SOURCE,
  CLEANUP_STATE_ATTR,
  INPUT_SOURCE_ATTR,
  SPEECH_SEGMENT_ID_ATTR,
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
})
