import { describe, expect, it } from 'vitest'
import { TestApplicationTransport } from './testApplicationTransport'
import { StaleHttpApplicationLeaseError, StaleHttpApplicationResponseError } from './httpApplicationTransport'
import { readApplicationSnapshot, type ApplicationSnapshotQuery } from './readApplicationSnapshot'

describe('application snapshot reads', () => {
  it('re-reads a projection invalidated by an event or unstable server snapshot', async () => {
    let reads = 0
    const transport = new TestApplicationTransport().handle('readPublishedFeedback', () => {
      reads += 1
      if (reads === 1) throw new StaleHttpApplicationResponseError()
      if (reads === 2) throw { code: 'SNAPSHOT_UNSTABLE', message: 'Projection changed during the read', retryable: true }
      return null
    })
    await expect(readApplicationSnapshot(transport, 'readPublishedFeedback', { request_id: 'current' })).resolves.toBeNull()
    expect(transport.calls).toEqual(Array.from({ length: 3 }, () => ({ name: 'readPublishedFeedback', input: { request_id: 'current' } })))
  })

  it('bounds repeated invalidation to two re-reads', async () => {
    const error = new StaleHttpApplicationResponseError()
    const transport = new TestApplicationTransport().reject('listHostSessions', error)
    await expect(readApplicationSnapshot(transport, 'listHostSessions', undefined)).rejects.toBe(error)
    expect(transport.calls).toHaveLength(3)
  })

  it('never retries authentication changes or unrelated errors', async () => {
    for (const error of [new StaleHttpApplicationLeaseError(), new Error('Network unavailable'),
      { code: 'RUNTIME_GENERATION_STALE', message: 'Restarted', retryable: false }]) {
      const transport = new TestApplicationTransport().reject('getFeedbackWorkspace', error)
      await expect(readApplicationSnapshot(transport, 'getFeedbackWorkspace', { request_id: 'current' })).rejects.toBe(error)
      expect(transport.calls).toHaveLength(1)
    }
  })

  it('rejects a mutation before dispatch even if a caller bypasses the query type', async () => {
    const transport = new TestApplicationTransport()
    await expect(readApplicationSnapshot(transport, 'sendManagedPrompt' as ApplicationSnapshotQuery, undefined))
      .rejects.toThrow('read-only application queries')
    expect(transport.calls).toEqual([])
  })
})
