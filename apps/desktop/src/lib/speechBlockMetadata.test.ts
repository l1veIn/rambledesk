import type { JSONContent } from '@tiptap/core'
import { describe, expect, it } from 'vitest'

import {
  CLEANUP_STATE_ATTR,
  INPUT_SOURCE_ATTR,
  SPEECH_SEGMENT_ID_ATTR,
  asrParagraphAttrs,
  speechCleanupCandidates,
} from './speechBlockMetadata'

describe('speech block metadata', () => {
  it('selects only pending ASR paragraphs and preserves document order', () => {
    const doc: JSONContent = {
      type: 'doc',
      content: [
        {
          type: 'paragraph',
          attrs: asrParagraphAttrs('asr-1', 'pending'),
          content: [{ type: 'text', text: '第一段' }],
        },
        {
          type: 'paragraph',
          content: [{ type: 'text', text: '手打内容' }],
        },
        {
          type: 'paragraph',
          attrs: asrParagraphAttrs('asr-2', 'cleaned'),
          content: [{ type: 'text', text: '已经整理' }],
        },
        {
          type: 'paragraph',
          attrs: asrParagraphAttrs('asr-3', 'pending'),
          content: [{ type: 'text', text: '第三段' }],
        },
      ],
    }

    expect(speechCleanupCandidates(doc)).toEqual([
      { segmentId: 'asr-1', text: '第一段' },
      { segmentId: 'asr-3', text: '第三段' },
    ])
  })

  it('uses explicit independent attributes for provenance and cleanup state', () => {
    expect(asrParagraphAttrs('segment-1', 'pending')).toEqual({
      [SPEECH_SEGMENT_ID_ATTR]: 'segment-1',
      [INPUT_SOURCE_ATTR]: 'asr',
      [CLEANUP_STATE_ATTR]: 'pending',
    })
  })
})
