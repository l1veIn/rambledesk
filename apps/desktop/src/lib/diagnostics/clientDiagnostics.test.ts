import { describe, expect, it, vi } from 'vitest'
import { createClientDiagnosticRecorder, diagnosticAgentId, diagnosticErrorCategory, sanitizeDiagnosticDetails } from './clientDiagnostics'

const id = '00000000-0000-4000-8000-000000000001'
describe('local client diagnostics', () => {
  it('pairs a start with exactly one finish and measures elapsed time', () => {
    const sink = vi.fn()
    let now = 10
    const recorder = createClientDiagnosticRecorder(sink, () => now, () => id)
    const finish = recorder.start('agent_detection', { source: 'catalog', reason: 'manual' })
    now = 165
    finish('ok', { checked_count: 3 })
    finish('failed')
    expect(sink.mock.calls.map(([event]) => event)).toEqual([
      { activity: 'agent_detection', outcome: 'started', operationId: id, durationMs: undefined, details: { source: 'catalog', reason: 'manual' } },
      { activity: 'agent_detection', outcome: 'ok', operationId: id, durationMs: 155, details: { source: 'catalog', reason: 'manual', checked_count: 3 } },
    ])
  })
  it('stops new events and outstanding operation finishes when recording is disabled', async () => {
    const events: unknown[] = []
    const recorder = createClientDiagnosticRecorder(event => { events.push(event) }, () => 0, () => id)
    const finish = recorder.start('agent_detection')
    recorder.setEnabled(false)
    expect(recorder.record({ activity: 'onboarding', outcome: 'ok' })).toBe(false)
    const disabledFinish = recorder.start('agent_connection')
    finish('ok')
    await recorder.flush()
    expect(events).toHaveLength(1)
    recorder.setEnabled(true)
    finish('failed')
    disabledFinish('ok')
    recorder.start('session_runtime')('ok')
    expect(events).toEqual([
      expect.objectContaining({ activity: 'agent_detection', outcome: 'started' }),
      expect.objectContaining({ activity: 'session_runtime', outcome: 'started' }),
      expect.objectContaining({ activity: 'session_runtime', outcome: 'ok' }),
    ])
  })
  it('does not revive old loss reports after disabling and enabling recording', async () => {
    let fail!: () => void
    const events: unknown[] = []
    const recorder = createClientDiagnosticRecorder(event => {
      events.push(event)
      return new Promise<void>((_resolve, reject) => { fail = () => reject(Error('late failure')) })
    }, () => 0, () => id)
    recorder.record({ activity: 'onboarding', outcome: 'ok' })
    recorder.setEnabled(false)
    fail()
    await recorder.flush()
    recorder.setEnabled(true)
    await recorder.flush()
    expect(events).toHaveLength(1)
  })
  it('drops payloads, secrets, paths and invalid enum values even if a caller bypasses types', () => {
    expect(diagnosticAgentId('claude-acp')).toBe('claude-acp')
    expect(diagnosticAgentId('My secret project')).toBe('custom')
    const safe = sanitizeDiagnosticDetails({ source: 'catalog', action: 'sk-secret', prompt: 'private text', path: 'D:/private', reason: 'password', entry_count: 2, failed_count: Infinity, selected: true })
    expect(safe).toEqual({ source: 'catalog', entry_count: 2, selected: true })
    const sink = vi.fn()
    const recorder = createClientDiagnosticRecorder(sink, () => 0, () => id)
    recorder.record({ activity: 'private text', outcome: 'failed' })
    recorder.record({ activity: 'agent_detection', outcome: 'failed', operationId: 'sk-secret' })
    expect(sink).not.toHaveBeenCalled()
  })
  it('keeps synchronous and asynchronous sink failures out of application behavior', async () => {
    for (const sink of [() => { throw Error('failed') }, () => Promise.reject(Error('failed'))]) {
      const recorder = createClientDiagnosticRecorder(sink, () => 0, () => id)
      expect(() => recorder.start('agent_detection')('failed')).not.toThrow()
    }
    await Promise.resolve()
  })
  it('bounds outstanding IPC work when persistence is stuck', () => {
    const sink = vi.fn(() => new Promise<void>(() => {}))
    const recorder = createClientDiagnosticRecorder(sink, () => 0, () => id)
    for (let index = 0; index < 1000; index++) recorder.record({ activity: 'onboarding_step', outcome: 'ok' })
    expect(sink).toHaveBeenCalledTimes(128)
  })
  it('does not emit an orphan finish for a rejected start, and reports loss after recovery', async () => {
    const completions: (() => void)[] = []
    const sink = vi.fn(() => new Promise<void>(resolve => { completions.push(resolve) }))
    const recorder = createClientDiagnosticRecorder(sink, () => 0, () => id)
    for (let index = 0; index < 128; index++) recorder.record({ activity: 'onboarding_step', outcome: 'ok' })
    const finish = recorder.start('agent_detection')
    completions.forEach(resolve => resolve())
    await Promise.resolve(); await Promise.resolve(); await Promise.resolve()
    finish('ok')
    expect(sink.mock.calls.flat()).not.toContainEqual(expect.objectContaining({ activity: 'agent_detection' }))
    await recorder.flush(1)
    expect(sink).toHaveBeenLastCalledWith(expect.objectContaining({ activity: 'diagnostic_backpressure', details: { dropped_count: 1 } }))
    completions.at(-1)?.()
  })
  it('classifies stable codes without inspecting sensitive error messages', () => {
    expect(diagnosticErrorCategory({ code: 'TIMEOUT', get message() { throw Error('never read') } })).toBe('timeout')
    expect(diagnosticErrorCategory(Error('token sk-secret'))).toBe('unknown')
    expect(diagnosticErrorCategory({ get code() { throw Error('never serialize') } })).toBe('unknown')
  })
  it('waits for pending metadata before export, but times out a stalled receiver', async () => {
    let complete: () => void = () => {}
    const recorder = createClientDiagnosticRecorder(() => new Promise<void>(resolve => { complete = resolve }), () => 0, () => id)
    recorder.record({ activity: 'onboarding', outcome: 'ok' })
    let flushed = false
    const flush = recorder.flush().then(() => { flushed = true })
    await Promise.resolve()
    expect(flushed).toBe(false)
    complete()
    await flush
    expect(flushed).toBe(true)
    recorder.record({ activity: 'onboarding', outcome: 'ok' })
    await recorder.flush(1)
  })
  it('preserves loss totals when the loss report itself fails', async () => {
    let available = false
    const sink = vi.fn(async () => { if (!available) throw Error('offline') })
    const recorder = createClientDiagnosticRecorder(sink, () => 0, () => id)
    recorder.record({ activity: 'onboarding', outcome: 'ok' })
    recorder.record({ activity: 'onboarding_step', outcome: 'ok' })
    await recorder.flush()
    available = true
    await recorder.flush()
    expect(sink).toHaveBeenLastCalledWith(expect.objectContaining({ activity: 'diagnostic_backpressure', details: { dropped_count: 2 } }))
  })
})
