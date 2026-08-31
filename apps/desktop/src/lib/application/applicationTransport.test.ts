import { describe, expect, expectTypeOf, it } from 'vitest'
import {
  defineApplicationStream,
  type ApplicationStream,
  type ApplicationStreamEvent,
  type ApplicationTransport,
} from './applicationTransport'

describe('ApplicationTransport contracts', () => {
  it('keeps stream event types on an opaque descriptor', () => {
    type RequestChanged = Readonly<{ requestId: string }>
    const stream = defineApplicationStream<RequestChanged>('test:request-changed')

    expect(stream.id).toBe('test:request-changed')
    expect(Object.isFrozen(stream)).toBe(true)
    expectTypeOf(stream).toEqualTypeOf<ApplicationStream<RequestChanged>>()
    expectTypeOf<ApplicationStreamEvent<typeof stream>>().toEqualTypeOf<RequestChanged>()
  })

  it('defaults the capability manifest to unknown', () => {
    expectTypeOf<ApplicationTransport['capabilities']>().returns.toEqualTypeOf<unknown>()
  })
})
