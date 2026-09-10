import { afterEach, describe, expect, it, vi } from 'vitest'
import { describeStartupFailure, runFrontendBootstrap } from './startupFallback'

afterEach(() => vi.useRealTimers())

describe('frontend bootstrap recovery', () => {
  it('catches synchronous initialization failures without exposing their contents', async () => {
    const failed = vi.fn()
    await runFrontendBootstrap(() => { throw new TypeError('secret preference value') }, failed)
    expect(failed).toHaveBeenCalledExactlyOnceWith({ code: 'STARTUP_FAILED', category: 'type_error' })
    expect(JSON.stringify(failed.mock.calls)).not.toContain('secret')
  })

  it('reports rejected component imports and arbitrary thrown values safely', async () => {
    const failed = vi.fn()
    await runFrontendBootstrap(async () => { throw 'file:///private/path' }, failed)
    expect(failed).toHaveBeenCalledExactlyOnceWith({ code: 'STARTUP_FAILED', category: 'unknown' })
    expect(describeStartupFailure(new SyntaxError('private source'))).toEqual({ code: 'STARTUP_FAILED', category: 'syntax_error' })
  })

  it('bounds a stalled import and prevents its eventual mount', async () => {
    vi.useFakeTimers()
    let loaded!: () => void
    const module = new Promise<void>(resolve => { loaded = resolve })
    const mount = vi.fn()
    const failed = vi.fn()
    const ready = runFrontendBootstrap(async signal => {
      await module
      signal.throwIfAborted()
      mount()
    }, failed, 100)
    await vi.advanceTimersByTimeAsync(100)
    await ready
    expect(failed).toHaveBeenCalledExactlyOnceWith({ code: 'STARTUP_TIMEOUT', category: 'timeout' })
    loaded()
    await vi.runAllTimersAsync()
    expect(mount).not.toHaveBeenCalled()
    expect(failed).toHaveBeenCalledTimes(1)
  })

  it('clears the deadline after successful startup', async () => {
    vi.useFakeTimers()
    const failed = vi.fn()
    await runFrontendBootstrap(async () => {}, failed, 100)
    await vi.runAllTimersAsync()
    expect(failed).not.toHaveBeenCalled()
    expect(vi.getTimerCount()).toBe(0)
  })
})
