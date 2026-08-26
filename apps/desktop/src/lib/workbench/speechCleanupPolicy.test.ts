import { describe, expect, it } from 'vitest'

import {
  CLEANUP_CHAR_THRESHOLD,
  CLEANUP_STABLE_THRESHOLD,
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

  it('starts on silence, non-speech insert, or settle whenever anything is pending', () => {
    expect(
      shouldStartCleanup({
        enabled: true,
        busy: false,
        pendingPieces: ['one line'],
        trigger: 'silence',
      }),
    ).toBe(true)
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
