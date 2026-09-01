import { describe, expect, it } from 'vitest'
import { Sha256 } from './sha256'

describe('incremental SHA-256', () => {
  it.each([
    ['', 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'],
    ['abc', 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad'],
  ])('matches the standard vector for %j', (input, expected) => {
    expect(new Sha256().update(new TextEncoder().encode(input)).digestHex()).toBe(expected)
  })

  it('is invariant to response chunk boundaries', () => {
    const input = new TextEncoder().encode('RambleDesk browser speech '.repeat(200))
    const whole = new Sha256().update(input).digestHex()
    const chunked = new Sha256()
    for (let offset = 0; offset < input.length; offset += 37) {
      chunked.update(input.subarray(offset, offset + 37))
    }
    expect(chunked.digestHex()).toBe(whole)
  })
})
