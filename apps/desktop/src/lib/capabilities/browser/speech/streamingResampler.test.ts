import { describe, expect, it } from 'vitest'
import { downmixToMono, StreamingResampler } from './streamingResampler'

describe('browser speech audio transforms', () => {
  it.each([44_100, 48_000, 96_000])('resamples %i Hz to 16 kHz independently of chunks', (rate) => {
    const input = Float32Array.from({ length: rate }, (_, index) => Math.sin(index / 31))
    const whole = new StreamingResampler(rate)
    const expected = [...whole.push(input), ...whole.flush()]
    const chunked = new StreamingResampler(rate)
    const actual: number[] = []
    for (let offset = 0; offset < input.length; offset += 997) {
      actual.push(...chunked.push(input.subarray(offset, offset + 997)))
    }
    actual.push(...chunked.flush())
    expect(actual).toHaveLength(expected.length)
    expect(actual).toEqual(expected)
    expect(actual.every(Number.isFinite)).toBe(true)
    expect(actual.length).toBeGreaterThanOrEqual(15_999)
    expect(actual.length).toBeLessThanOrEqual(16_001)
  })

  it('downmixes every input channel instead of selecting the first', () => {
    expect([...downmixToMono([new Float32Array([1, -1]), new Float32Array([-1, 1])])]).toEqual([0, 0])
  })
})
