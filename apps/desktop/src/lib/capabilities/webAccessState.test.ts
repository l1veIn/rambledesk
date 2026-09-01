import { describe, expect, it, vi } from 'vitest'

import type { WebAccessStatus } from './workbenchCapabilities'
import {
  settleWebAccessMutation,
  webAccessDisplayState,
  webAccessRunningActionsEnabled,
  webAccessToggleTarget,
} from './webAccessState'

const stopped = {
  state: 'stopped',
  url: null,
  failure: null,
} satisfies WebAccessStatus
const running = {
  state: 'running',
  url: 'http://127.0.0.1:37643',
  failure: null,
} satisfies WebAccessStatus
const failed = {
  state: 'failed',
  url: null,
  failure: {
    code: 'address_in_use',
    message: 'Port 37643 is already in use. Close the other process and try again.',
  },
} satisfies WebAccessStatus

describe('Web Access Settings state', () => {
  it('keeps transient activity separate from the persisted backend fact', () => {
    expect(webAccessDisplayState(null, 'loading')).toBe('loading')
    expect(webAccessDisplayState(running, 'stopping')).toBe('stopping')
    expect(webAccessDisplayState(stopped, null)).toBe('stopped')
    expect(webAccessDisplayState(running, null)).toBe('running')
    expect(webAccessDisplayState(failed, null)).toBe('failed')
    expect(webAccessDisplayState(null, null)).toBe('unavailable')
  })

  it('exposes token and address actions only for a settled running fact', () => {
    expect(webAccessRunningActionsEnabled(running, null)).toBe(true)
    expect(webAccessRunningActionsEnabled(running, 'stopping')).toBe(false)
    expect(webAccessRunningActionsEnabled(stopped, null)).toBe(false)
    expect(webAccessRunningActionsEnabled(failed, null)).toBe(false)
    expect(webAccessRunningActionsEnabled(null, null)).toBe(false)

    expect(webAccessToggleTarget(running)).toBe(false)
    expect(webAccessToggleTarget(stopped)).toBe(true)
    expect(webAccessToggleTarget(failed)).toBe(true)
    expect(webAccessToggleTarget(null)).toBeNull()
  })

  it('refreshes the backend fact when the mutation response is uncertain', async () => {
    const uncertain = new Error('IPC response was lost')
    const implementation = {
      setEnabled: vi.fn(async () => Promise.reject(uncertain)),
      status: vi.fn(async () => running),
    }

    await expect(settleWebAccessMutation(implementation, true)).resolves.toEqual({
      status: running,
      operationError: null,
      refreshError: null,
    })
    expect(implementation.setEnabled).toHaveBeenCalledWith(true)
    expect(implementation.status).toHaveBeenCalledOnce()
  })

  it('keeps the operation error only when the refreshed fact did not reach the target', async () => {
    const rejected = new Error('Could not stop Web Access')
    const implementation = {
      setEnabled: vi.fn(async () => Promise.reject(rejected)),
      status: vi.fn(async () => running),
    }

    await expect(settleWebAccessMutation(implementation, false)).resolves.toEqual({
      status: running,
      operationError: rejected,
      refreshError: null,
    })
  })

  it('does not preserve a stale running fact when both mutation and refresh fail', async () => {
    const operationError = new Error('IPC response was lost')
    const refreshError = new Error('Backend status is unavailable')
    const implementation = {
      setEnabled: vi.fn(async () => Promise.reject(operationError)),
      status: vi.fn(async () => Promise.reject(refreshError)),
    }

    await expect(settleWebAccessMutation(implementation, false)).resolves.toEqual({
      status: null,
      operationError,
      refreshError,
    })
  })
})
