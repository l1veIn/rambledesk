import { describe, expect, it } from 'vitest'

import { ACTION_ID_ATTR, ACTION_INDEX_ATTR } from './actionBlockquote'
import { applyDraftOperation } from './draftOperations'
import { SPEECH_SEGMENT_ID_ATTR } from './speechBlockMetadata'

const actionA = { actionId: 'login', actionIndex: 0, title: '修复登录状态' }
const actionB = { actionId: 'toast', actionIndex: 1, title: '检查 toast' }

describe('draft operations', () => {
  it('appends ordinary speech as a top-level pending ASR paragraph', () => {
    const next = applyDraftOperation(
      { type: 'doc', content: [] },
      { kind: 'appendSpeech', segmentId: 'seg-1', text: '登录失败', action: null },
    )
    expect(next.content?.[0]).toMatchObject({
      type: 'paragraph',
      attrs: { [SPEECH_SEGMENT_ID_ATTR]: 'seg-1' },
    })
  })

  it('reuses the open Action Blockquote and creates a new one after reopen', () => {
    const first = applyDraftOperation(
      { type: 'doc', content: [] },
      { kind: 'startActionGroup', action: actionA },
    )
    const withSpeech = applyDraftOperation(first, {
      kind: 'appendSpeech',
      segmentId: 'a1',
      text: '第一次',
      action: actionA,
    })
    const switched = applyDraftOperation(withSpeech, {
      kind: 'appendSpeech',
      segmentId: 'b1',
      text: '中间',
      action: actionB,
    })
    const reopened = applyDraftOperation(switched, {
      kind: 'appendSpeech',
      segmentId: 'a2',
      text: '再次打开',
      action: actionA,
    })

    expect(reopened.content?.map((node) => node.type)).toEqual([
      'blockquote',
      'blockquote',
      'blockquote',
    ])
    expect(reopened.content?.map((node) => node.attrs?.[ACTION_ID_ATTR])).toEqual([
      'login',
      'toast',
      'login',
    ])
    expect(reopened.content?.[0].attrs?.[ACTION_INDEX_ATTR]).toBe(0)
    expect(reopened.content?.[2].content?.some((node) => node.attrs?.[SPEECH_SEGMENT_ID_ATTR] === 'a2')).toBe(
      true,
    )
  })
})
