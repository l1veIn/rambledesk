import { describe, expect, it, vi } from 'vitest'

import {
  createTauriWebAccessAdministrationCapability,
  parseWebAccessStatus,
} from './administrationCapabilities'
import type { TauriCapabilityApi } from './tauriCapabilityApi'

function apiReturning(values: Record<string, unknown>) {
  const invoke = vi.fn(async (command: string) => values[command])
  return {
    api: { invoke } as unknown as TauriCapabilityApi,
    invoke,
  }
}

describe('Tauri Web Access administration capability', () => {
  it('parses the backend lifecycle union for status and mutations', async () => {
    const { api, invoke } = apiReturning({
      get_web_access_status: { state: 'stopped', url: null, failure: null },
      start_web_access: {
        state: 'running',
        url: 'http://127.0.0.1:38173',
        failure: null,
      },
      stop_web_access: {
        state: 'failed',
        url: null,
        failure: {
          code: 'shutdown_failed',
          message: 'Web Access could not stop cleanly. Try again.',
        },
      },
    })
    const capability = createTauriWebAccessAdministrationCapability(api)

    await expect(capability.status()).resolves.toEqual({
      state: 'stopped',
      url: null,
      failure: null,
    })
    await expect(capability.setEnabled(true)).resolves.toMatchObject({ state: 'running' })
    await expect(capability.setEnabled(false)).resolves.toMatchObject({
      state: 'failed',
      failure: { code: 'shutdown_failed' },
    })
    expect(invoke.mock.calls.map(([command]) => command)).toEqual([
      'get_web_access_status',
      'start_web_access',
      'stop_web_access',
    ])
  })

  it.each([
    { running: true, url: 'http://127.0.0.1:38173' },
    { state: 'running', url: '', failure: null },
    { state: 'stopped', url: 'http://127.0.0.1:38173', failure: null },
    {
      state: 'failed',
      url: null,
      failure: { code: 'not_a_stable_code', message: 'unsafe details' },
    },
  ])('rejects malformed lifecycle status without exposing its payload', (value) => {
    expect(() => parseWebAccessStatus(value)).toThrowError(
      'Web Access returned an invalid lifecycle status.',
    )
    try {
      parseWebAccessStatus(value)
    } catch (cause) {
      expect(String(cause)).not.toContain('unsafe details')
      expect(String(cause)).not.toContain('127.0.0.1')
    }
  })
})
