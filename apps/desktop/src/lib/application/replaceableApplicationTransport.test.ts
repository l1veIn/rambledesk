import { describe, expect, it, vi } from 'vitest'

import { defineApplicationStream } from './applicationTransport'
import { ReplaceableApplicationTransport } from './replaceableApplicationTransport'
import { TestApplicationTransport } from './testApplicationTransport'

describe('ReplaceableApplicationTransport', () => {
  it('moves subscriptions and future calls without replaying an in-flight mutation', async () => {
    const stream = defineApplicationStream<string>('events')
    const first = new TestApplicationTransport(undefined)
    const second = new TestApplicationTransport(undefined)
    first.resolve('listFeedbackInbox', [])
    second.resolve('listFeedbackInbox', [])
    const transport = new ReplaceableApplicationTransport(first)
    const handler = vi.fn()
    const unsubscribe = transport.subscribe(stream, handler, vi.fn())

    first.emit(stream, 'first')
    transport.replace(second)
    first.emit(stream, 'stale')
    second.emit(stream, 'second')
    await transport.call('listFeedbackInbox', undefined)

    expect(handler.mock.calls).toEqual([['first'], ['second']])
    expect(first.calls).toHaveLength(0)
    expect(second.calls).toEqual([{ name: 'listFeedbackInbox', input: undefined }])
    unsubscribe()
    second.emit(stream, 'after-unsubscribe')
    expect(handler).toHaveBeenCalledTimes(2)
  })
})
