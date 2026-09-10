import { afterEach, describe, expect, it, vi } from 'vitest'
import { createTauriHostIntegrationCapability } from '$lib/capabilities/tauri/administrationCapabilities'
import type { TauriCapabilityApi } from '$lib/capabilities/tauri/tauriCapabilityApi'
import { createExternalAdapterSettingsAccess } from './externalAdapterSettingsAccess'
import { configureClientDiagnostics, type ClientDiagnosticEvent } from '$lib/diagnostics/clientDiagnostics'

let stopDiagnostics: (() => void) | undefined
afterEach(() => { stopDiagnostics?.(); stopDiagnostics = undefined })

function harness() {
  const invoke = vi.fn(async (_command: string) => undefined)
  const capability = createTauriHostIntegrationCapability({ invoke } as unknown as TauriCapabilityApi)
  const access = createExternalAdapterSettingsAccess([
    capability.detectGenericMcpHosts, capability.piStatus, capability.dshStatus, capability.genericMcpConfiguration,
  ])
  return { invoke, access }
}

describe('external adapter settings access', () => {
  it('records explicit access and bounded probe outcomes while suppressing repeated page state and raw failures', async () => {
    const events: ClientDiagnosticEvent[] = []
    stopDiagnostics = configureClientDiagnostics(event => { events.push(event) })
    const reads = [vi.fn(async () => { throw new Error('private-adapter-error C:/private/path') }), vi.fn(async () => ['private-host-name'])]
    const access = createExternalAdapterSettingsAccess(reads)
    await access.setSection('adapters', false)
    expect(events).toHaveLength(1)
    expect(events[0]).toMatchObject({ outcome: 'blocked', details: { action: 'visit', available: false } })
    await access.setSection('adapters', true)
    expect(events.find(event => event.details?.action === 'probe' && event.outcome === 'failed')).toMatchObject({ durationMs: expect.any(Number), details: { target_count: 2, checked_count: 2, failed_count: 1 } })
    const count = events.length
    await access.setSection('adapters', true)
    expect(events).toHaveLength(count)
    expect(reads[0]).toHaveBeenCalledTimes(1)
    expect(JSON.stringify(events)).not.toMatch(/private-adapter-error|private-host-name|private\/path/u)
  })
  it('does not inspect or configure integrations when capabilities are constructed or other settings are visited', async () => {
    const { invoke, access } = harness()
    expect(invoke).not.toHaveBeenCalled()
    for (const section of ['general', 'agents', 'voice', 'about'] as const) await access.setSection(section, true)
    expect(access.isActive()).toBe(false)
    expect(invoke).not.toHaveBeenCalled()
  })

  it('reads all external adapter statuses only after entering their settings page and never installs as part of discovery', async () => {
    const { invoke, access } = harness()
    await access.setSection('adapters', true)
    expect(access.isActive()).toBe(true)
    expect(invoke.mock.calls.map(call => call[0])).toEqual([
      'detect_generic_mcp_hosts', 'get_pi_package_status', 'detect_dsh_host', 'get_generic_mcp_configuration',
    ])
    await access.setSection('adapters', true)
    expect(invoke).toHaveBeenCalledTimes(4)
    await access.setSection('agents', true)
    expect(access.isActive()).toBe(false)
    expect(invoke).toHaveBeenCalledTimes(4)
    await access.setSection('adapters', true)
    expect(invoke).toHaveBeenCalledTimes(8)
    expect(invoke.mock.calls.some(call => String(call[0]).includes('install'))).toBe(false)
  })

  it('does not scan unavailable adapters and does not start background retries when a probe fails', async () => {
    const reads = [vi.fn(async () => { throw new Error('Pi unavailable') }), vi.fn(async () => [])]
    const access = createExternalAdapterSettingsAccess(reads)
    await access.setSection('adapters', false)
    expect(reads[0]).not.toHaveBeenCalled()
    expect(access.isActive()).toBe(false)
    await access.setSection('adapters', true)
    expect(reads[0]).toHaveBeenCalledTimes(1)
    expect(reads[1]).toHaveBeenCalledTimes(1)
    await access.setSection('general', true)
    await Promise.resolve()
    expect(reads[0]).toHaveBeenCalledTimes(1)
    expect(access.isActive()).toBe(false)
  })
})
