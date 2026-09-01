import { describe, expect, it, vi } from 'vitest'

import {
  WebAccessConnectionError,
  WebAccessTokenRejectedError,
  bootstrapWebAccessSession,
} from './webAccessBootstrap'

describe('bootstrapWebAccessSession', () => {
  it('exchanges the durable token only through same-origin Authorization', async () => {
    const fetch = vi.fn<typeof globalThis.fetch>(async () =>
      new Response(JSON.stringify({ session_token: 'short_session-token' }), {
        status: 200,
        headers: { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' },
      }),
    )

    await expect(
      bootstrapWebAccessSession({
        token: 'a'.repeat(64),
        pageUrl: 'http://127.0.0.1:37643/workbench',
        fetch,
      }),
    ).resolves.toBe('short_session-token')
    const [url, init] = fetch.mock.calls[0]!
    expect(String(url)).toBe('http://127.0.0.1:37643/api/auth/session')
    expect(init).toMatchObject({
      method: 'POST',
      credentials: 'same-origin',
      redirect: 'error',
      headers: { Authorization: `Bearer ${'a'.repeat(64)}` },
    })
    expect(String(url)).not.toContain('a'.repeat(64))
    expect(bootstrapWebAccessSession.toString()).not.toMatch(/localStorage|sessionStorage/u)
  })

  it('reports wrong tokens without reflecting their value', async () => {
    const secret = 'wrong-secret'
    const fetch = vi.fn<typeof globalThis.fetch>(async () => new Response('', { status: 401 }))
    const failure = await bootstrapWebAccessSession({
      token: secret,
      pageUrl: 'http://127.0.0.1:37643/',
      fetch,
    }).catch((cause: unknown) => cause)
    expect(failure).toBeInstanceOf(WebAccessTokenRejectedError)
    expect((failure as Error).message).not.toContain(secret)
  })

  it.each([
    ['rate limit', async () => new Response('', { status: 429 })],
    ['server failure', async () => new Response('', { status: 503 })],
    ['invalid JSON', async () => new Response('not-json', { status: 200 })],
    ['network failure', async () => Promise.reject(new TypeError('offline'))],
  ])('keeps %s distinct from a rejected durable token', async (_label, request) => {
    await expect(
      bootstrapWebAccessSession({
        token: 'a'.repeat(64),
        pageUrl: 'http://127.0.0.1:37643/',
        fetch: vi.fn<typeof globalThis.fetch>(request),
      }),
    ).rejects.toBeInstanceOf(WebAccessConnectionError)
  })
})
