import { describe, expect, it, vi } from 'vitest'

import { defineApplicationStream } from './applicationTransport'
import { UnavailableApplicationTransport } from './unavailableApplicationTransport'

describe('UnavailableApplicationTransport', () => {
  it('fails calls and readiness without supplying fixture behavior', async () => {
    const transport = new UnavailableApplicationTransport()

    await expect(transport.call('listFeedbackInbox', undefined)).rejects.toThrow(
      'Application transport is unavailable',
    )
    await expect(transport.waitUntilReady()).rejects.toThrow(
      'Application transport is unavailable',
    )
  })

  it('reports subscription failure unless synchronously unsubscribed', async () => {
    const transport = new UnavailableApplicationTransport()
    const reported = vi.fn()
    transport.subscribe(defineApplicationStream('test:event'), vi.fn(), reported)
    const suppressed = vi.fn()
    const unsubscribe = transport.subscribe(
      defineApplicationStream('test:suppressed'),
      vi.fn(),
      suppressed,
    )
    unsubscribe()
    await Promise.resolve()

    expect(reported).toHaveBeenCalledWith(
      expect.objectContaining({ message: 'Application transport is unavailable in this environment.' }),
    )
    expect(suppressed).not.toHaveBeenCalled()
  })
})
