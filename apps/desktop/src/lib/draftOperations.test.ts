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

  it('does not stack empty Action Blockquotes when the same Action is clicked again', () => {
    const first = applyDraftOperation(
      { type: 'doc', content: [] },
      { kind: 'startActionGroup', action: actionA },
    )
    const again = applyDraftOperation(first, { kind: 'startActionGroup', action: actionA })
    const third = applyDraftOperation(again, { kind: 'startActionGroup', action: actionA })
    expect(third.content).toHaveLength(1)
    expect(third.content?.[0].attrs?.[ACTION_ID_ATTR]).toBe('login')
  })

  it('replaces a trailing empty Action Blockquote instead of leaving both', () => {
    const emptyA = applyDraftOperation(
      { type: 'doc', content: [] },
      { kind: 'startActionGroup', action: actionA },
    )
    const switched = applyDraftOperation(emptyA, { kind: 'startActionGroup', action: actionB })
    expect(switched.content).toHaveLength(1)
    expect(switched.content?.[0].attrs?.[ACTION_ID_ATTR]).toBe('toast')
  })

  it('keeps a filled Action Blockquote when opening a different Action', () => {
    const filled = applyDraftOperation(
      applyDraftOperation(
        { type: 'doc', content: [] },
        { kind: 'startActionGroup', action: actionA },
      ),
      { kind: 'appendSpeech', segmentId: 'a1', text: '有内容', action: actionA },
    )
    const next = applyDraftOperation(filled, { kind: 'startActionGroup', action: actionB })
    expect(next.content?.map((node) => node.attrs?.[ACTION_ID_ATTR])).toEqual(['login', 'toast'])
  })

  it('puts consecutive speech into the same Action group and ignores trailing empty paragraphs', () => {
    const opened = applyDraftOperation(
      { type: 'doc', content: [] },
      { kind: 'startActionGroup', action: actionA },
    )
    const withGap = {
      type: 'doc' as const,
      content: [...(opened.content ?? []), { type: 'paragraph' }],
    }
    const first = applyDraftOperation(withGap, {
      kind: 'appendSpeech',
      segmentId: 'a1',
      text: '喂。',
      action: actionA,
    })
    const second = applyDraftOperation(first, {
      kind: 'appendSpeech',
      segmentId: 'a2',
      text: '能听到吗?',
      action: actionA,
    })
    expect(second.content).toHaveLength(1)
    expect(second.content?.[0].attrs?.[ACTION_ID_ATTR]).toBe('login')
    expect(
      second.content?.[0].content?.filter((node) => node.attrs?.[SPEECH_SEGMENT_ID_ATTR]).map(
        (node) => node.content?.[0]?.text,
      ),
    ).toEqual(['喂。', '能听到吗?'])
  })

  it('drops an unused empty Action when returning to a filled one', () => {
    const filledA = applyDraftOperation(
      applyDraftOperation(
        { type: 'doc', content: [] },
        { kind: 'startActionGroup', action: actionA },
      ),
      { kind: 'appendSpeech', segmentId: 'a1', text: '有内容', action: actionA },
    )
    const emptyB = applyDraftOperation(filledA, { kind: 'startActionGroup', action: actionB })
    const backToA = applyDraftOperation(emptyB, { kind: 'startActionGroup', action: actionA })
    expect(backToA.content?.map((node) => node.attrs?.[ACTION_ID_ATTR])).toEqual(['login'])
  })
})
