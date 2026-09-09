import { describe, expect, it, vi } from 'vitest'

import { openReleases, PROJECT_URL, RELEASES_URL } from './releases'

describe('releases link', () => {
  it('opens the release page through the external-link capability', async () => {
    const open = vi.fn(async () => undefined)
    await openReleases({ implementation: { open } } as never)
    expect(open).toHaveBeenCalledWith(RELEASES_URL)
    expect(RELEASES_URL).toBe(`${PROJECT_URL}/releases`)
  })
})
