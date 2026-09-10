import { describe, expect, it } from 'vitest'

import { isNewerReleaseVersion, normalizeUpdateNotes, parseReleaseVersion } from './updateVersion'

describe('isNewerReleaseVersion', () => {
  it('treats a higher patch as an update', () => {
    expect(isNewerReleaseVersion('0.0.2', '0.0.1')).toBe(true)
    expect(isNewerReleaseVersion('0.0.1', '0.0.2')).toBe(false)
    expect(isNewerReleaseVersion('0.0.2', '0.0.2')).toBe(false)
  })

  it('treats a stable release as newer than its release candidate', () => {
    expect(isNewerReleaseVersion('0.0.2', '0.0.2-rc.18')).toBe(true)
    expect(isNewerReleaseVersion('0.0.2-rc.18', '0.0.2')).toBe(false)
    expect(isNewerReleaseVersion('0.0.2-rc.18', '0.0.2-rc.17')).toBe(true)
  })

  it('accepts a v prefix', () => {
    expect(parseReleaseVersion('v1.2.3-rc.1')).toEqual({
      major: 1,
      minor: 2,
      patch: 3,
      prerelease: 'rc.1',
    })
    expect(isNewerReleaseVersion('v0.0.3', 'v0.0.2')).toBe(true)
  })
})

describe('normalizeUpdateNotes', () => {
  it('trims empty notes to the fallback', () => {
    expect(normalizeUpdateNotes('  \n  ', 'See GitHub Release Notes.')).toBe(
      'See GitHub Release Notes.',
    )
  })

  it('keeps changelog text and caps very long notes', () => {
    expect(normalizeUpdateNotes('## What\'s Changed\r\n* Fix updater dialog\r\n')).toBe(
      "## What's Changed\n* Fix updater dialog",
    )
    const long = 'n'.repeat(8_100)
    expect(normalizeUpdateNotes(long).endsWith('…')).toBe(true)
    expect(normalizeUpdateNotes(long).length).toBeLessThan(8_100)
  })
})
