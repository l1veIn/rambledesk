import { describe, expect, it } from 'vitest'

import {
  DEFAULT_CLEANUP_CHAR_THRESHOLD,
  DEFAULT_CLEANUP_SEGMENT_THRESHOLD,
  acceptCleanupResult,
  alignCleanupParts,
  normalizeCleanupNewlines,
  shouldStartCleanup,
} from './speechCleanupPolicy'

const thresholds = {
  segmentThreshold: DEFAULT_CLEANUP_SEGMENT_THRESHOLD,
  charThreshold: DEFAULT_CLEANUP_CHAR_THRESHOLD,
}

describe('shouldStartCleanup', () => {
  it('does nothing when disabled, busy, or empty', () => {
    expect(
      shouldStartCleanup({
        enabled: false,
        busy: false,
        pendingCount: 3,
        pendingChars: 12,
        trigger: 'settle',
        thresholds,
      }),
    ).toBe(false)
    expect(
      shouldStartCleanup({
        enabled: true,
        busy: true,
        pendingCount: 3,
        pendingChars: 12,
        trigger: 'settle',
        thresholds,
      }),
    ).toBe(false)
    expect(
      shouldStartCleanup({
        enabled: true,
        busy: false,
        pendingCount: 0,
        pendingChars: 0,
        trigger: 'settle',
        thresholds,
      }),
    ).toBe(false)
  })

  it('starts after three uncleaned speech nodes or 500 characters', () => {
    expect(
      shouldStartCleanup({
        enabled: true,
        busy: false,
        pendingCount: DEFAULT_CLEANUP_SEGMENT_THRESHOLD - 1,
        pendingChars: 8,
        trigger: 'segment-count',
        thresholds,
      }),
    ).toBe(false)
    expect(
      shouldStartCleanup({
        enabled: true,
        busy: false,
        pendingCount: DEFAULT_CLEANUP_SEGMENT_THRESHOLD,
        pendingChars: 12,
        trigger: 'segment-count',
        thresholds,
      }),
    ).toBe(true)
    expect(
      shouldStartCleanup({
        enabled: true,
        busy: false,
        pendingCount: 1,
        pendingChars: DEFAULT_CLEANUP_CHAR_THRESHOLD,
        trigger: 'char-count',
        thresholds,
      }),
    ).toBe(true)
  })

  it('starts after the configured idle period; non-speech insert or settle also flush pending', () => {
    expect(
      shouldStartCleanup({
        enabled: true,
        busy: false,
        pendingCount: 1,
        pendingChars: 8,
        trigger: 'idle',
        thresholds,
      }),
    ).toBe(true)
    expect(
      shouldStartCleanup({
        enabled: true,
        busy: false,
        pendingCount: 1,
        pendingChars: 8,
        trigger: 'non-speech',
        thresholds,
      }),
    ).toBe(true)
    expect(
      shouldStartCleanup({
        enabled: true,
        busy: false,
        pendingCount: 1,
        pendingChars: 8,
        trigger: 'settle',
        thresholds,
      }),
    ).toBe(true)
  })
})

describe('alignCleanupParts', () => {
  it('keeps a one-to-one mapping onto speech nodes', () => {
    expect(alignCleanupParts(['啊按钮太小了', '列表还行'], '按钮太小了。\n\n列表还行。')).toEqual([
      '按钮太小了。',
      '列表还行。',
    ])
  })

  it('rejects a blob that does not match the node count', () => {
    expect(alignCleanupParts(['one', 'two'], 'one two three four')).toBeNull()
    expect(alignCleanupParts(['one', 'two'], null)).toBeNull()
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

  it('normalizes JSON-escaped newlines so batch splitting still works', () => {
    const escaped = '第一句。\\n\\n第二句。'
    expect(normalizeCleanupNewlines(escaped)).toBe('第一句。\n\n第二句。')
    expect(
      alignCleanupParts(['第一句', '第二句'], acceptCleanupResult('第一句\n\n第二句', escaped)),
    ).toEqual(['第一句。', '第二句。'])
  })
})
