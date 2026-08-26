import { describe, expect, it } from 'vitest'

import {
  CLEANUP_CHAR_THRESHOLD,
  CLEANUP_STABLE_THRESHOLD,
  acceptCleanupResult,
  shouldStartCleanup,
} from './speechCleanupPolicy'

describe('shouldStartCleanup', () => {
  const pending = ['one', 'two', 'three']

  it('does nothing when disabled, busy, or empty', () => {
    expect(
      shouldStartCleanup({ enabled: false, busy: false, pendingPieces: pending, trigger: 'settle' }),
    ).toBe(false)
    expect(
      shouldStartCleanup({ enabled: true, busy: true, pendingPieces: pending, trigger: 'settle' }),
    ).toBe(false)
    expect(
      shouldStartCleanup({ enabled: true, busy: false, pendingPieces: [], trigger: 'settle' }),
    ).toBe(false)
  })

  it('starts after three pending stables or 500 characters', () => {
    expect(
      shouldStartCleanup({
        enabled: true,
        busy: false,
        pendingPieces: pending.slice(0, CLEANUP_STABLE_THRESHOLD - 1),
        trigger: 'stable-count',
      }),
    ).toBe(false)
    expect(
      shouldStartCleanup({
        enabled: true,
        busy: false,
        pendingPieces: pending,
        trigger: 'stable-count',
      }),
    ).toBe(true)
    expect(
      shouldStartCleanup({
        enabled: true,
        busy: false,
        pendingPieces: ['x'.repeat(CLEANUP_CHAR_THRESHOLD)],
        trigger: 'char-count',
      }),
    ).toBe(true)
  })

  it('does not start on a thinking pause; non-speech insert or settle still flush pending', () => {
    expect(
      shouldStartCleanup({
        enabled: true,
        busy: false,
        pendingPieces: ['one line'],
        trigger: 'silence',
      }),
    ).toBe(false)
    expect(
      shouldStartCleanup({
        enabled: true,
        busy: false,
        pendingPieces: ['one line'],
        trigger: 'non-speech',
      }),
    ).toBe(true)
    expect(
      shouldStartCleanup({
        enabled: true,
        busy: false,
        pendingPieces: ['one line'],
        trigger: 'settle',
      }),
    ).toBe(true)
  })
})

describe('acceptCleanupResult', () => {
  it('keeps the original when the model answers instead of tidying', () => {
    const spoken = '呃，跟我说一下当前我们在这个分支上做了哪些工作。'
    const answered =
      '好的，当前这个分支上我们主要做了这些工作：修复了登录页面的一个崩溃问题，优化了数据加载速度。'
    expect(acceptCleanupResult(spoken, answered)).toBe(spoken)
  })

  it('keeps a same-length tidy', () => {
    expect(acceptCleanupResult('呃，按钮太小了。', '按钮太小了。')).toBe('按钮太小了。')
  })
})
