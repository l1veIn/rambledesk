import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
import type { AgentConfig, AgentConnectionCheck } from '$lib/generated/feedback'
import { beginAgentConnection, beginAgentInspection, forgetAgentConnection, observeAgentRuntime, readAgentDetectionCache, reconcileAgentConnections, rememberAgentConnection, rememberAgentInspection, resetAgentDetectionCache } from './agentDetectionCache'
import { config, inspection } from './draftManagedSessionControllerTestHarness'

describe('Shared agent detection results', () => {
  it('redacts nested failure evidence before publishing it to another view', () => {
    const secret = 'nested-failure-canary'
    const config: AgentConfig = {
      id: 'private', name: 'Private', host_id: 'generic', protocol: 'acp', enabled: true,
      command: 'agent', args: [], env: { API_KEY: secret }, created_at: '', updated_at: '',
    }
    const result: AgentConnectionCheck = {
      ok: false, connection: 'connected', message: `Check ${secret}`, details: [`Detail ${secret}`],
      failure: { stage: 'initialize', reason: 'authentication', message: `Authentication ${secret}` },
    }
    const transport = new TestApplicationTransport()
    rememberAgentConnection(transport, config, result)
    const cached = readAgentDetectionCache(transport).connections[config.id].result
    expect(JSON.stringify(cached)).not.toContain(secret)
    expect(cached.failure).toEqual({ stage: 'initialize', reason: 'authentication', message: 'Authentication [redacted]' })
    expect(result.failure?.message).toContain(secret)
  })
})

describe('persisted explicit detection', () => {
  const connected = { ok: true, message: 'ACP connected', details: [] }
  const data = new Map<string, string>()
  const client = (scope = 'desktop') => Object.assign(new TestApplicationTransport(), { persistenceScope: scope })
  beforeEach(() => {
    data.clear()
    vi.stubGlobal('localStorage', { getItem: (key: string) => data.get(key) ?? null, setItem: (key: string, value: string) => data.set(key, value) })
  })
  afterEach(() => vi.unstubAllGlobals())

  it('restores results in a new application instance only after matching saved revisions', () => {
    const original = client()
    rememberAgentInspection(original, inspection)
    rememberAgentConnection(original, config, connected)
    const reopened = client()
    expect(readAgentDetectionCache(reopened).inspections.pi).toEqual(inspection)
    expect(readAgentDetectionCache(reopened).connections).toEqual({})
    reconcileAgentConnections(reopened, [config])
    expect(readAgentDetectionCache(reopened).connections[config.id]?.result).toEqual(connected)
    expect(reopened.calls).toHaveLength(0)
  })
  it('invalidates edits and deletions across restarts without reviving an older revision', () => {
    const original = client()
    rememberAgentConnection(original, config, connected)
    const reopened = client()
    reconcileAgentConnections(reopened, [{ ...config, updated_at: 'later', command: 'changed' }])
    expect(readAgentDetectionCache(reopened).connections).toEqual({})
    const restored = client()
    reconcileAgentConnections(restored, [config])
    expect(readAgentDetectionCache(restored).connections).toEqual({})
    rememberAgentConnection(restored, config, connected)
    reconcileAgentConnections(restored, [])
    const deleted = client()
    reconcileAgentConnections(deleted, [config])
    expect(readAgentDetectionCache(deleted).connections).toEqual({})
  })
  it('persists failures and never serializes launch signatures or account secrets', () => {
    const transport = client()
    const privateConfig = { ...config, command: 'private-command', args: ['private-arg'], env: { TOKEN: 'private-env' } }
    rememberAgentConnection(transport, privateConfig, { ok: false, message: 'Failed private-env', details: ['private-env'], failure: { stage: 'initialize', reason: 'authentication', message: 'private-env' } })
    const serialized = [...data.values()].join('')
    for (const privateValue of ['private-command', 'private-arg', 'private-env']) expect(serialized).not.toContain(privateValue)
    const reopened = client()
    reconcileAgentConnections(reopened, [privateConfig])
    expect(readAgentDetectionCache(reopened).connections[config.id].result).toMatchObject({ ok: false, failure: { reason: 'authentication' } })
  })
  it('retains completed results on restart and rejects pending results from the previous runtime', () => {
    const transport = client()
    observeAgentRuntime(transport, 'first')
    rememberAgentConnection(transport, config, connected)
    const other = { ...config, id: 'other' }
    const completeCheck = beginAgentConnection(transport, other)
    const completeInspection = beginAgentInspection(transport, 'pi')
    observeAgentRuntime(transport, 'second')
    expect(completeCheck(connected)).toBe(false)
    expect(completeInspection(inspection)).toBe(false)
    expect(readAgentDetectionCache(transport).connections[config.id].result).toEqual(connected)
  })
  it('keeps explicit clearing and client scopes separate', () => {
    const transport = client()
    rememberAgentConnection(transport, config, connected)
    const web = client('web')
    reconcileAgentConnections(web, [config])
    expect(readAgentDetectionCache(web).connections).toEqual({})
    forgetAgentConnection(transport, config.id)
    const reopened = client()
    reconcileAgentConnections(reopened, [config])
    expect(readAgentDetectionCache(reopened).connections).toEqual({})
    rememberAgentInspection(reopened, inspection)
    resetAgentDetectionCache(reopened)
    expect(readAgentDetectionCache(client()).inspections).toEqual({})
  })
  it('ignores corrupt storage and falls back to memory when writes fail', () => {
    data.set('rambledesk.agent-detection.v1:desktop', '{invalid')
    expect(readAgentDetectionCache(client())).toEqual({ inspections: {}, connections: {} })
    data.set('rambledesk.agent-detection.v1:desktop', JSON.stringify({ version: 1, inspections: {}, connections: { config: { revision: '', result: { ok: 'yes' } } } }))
    expect(readAgentDetectionCache(client()).connections).toEqual({})
    vi.stubGlobal('localStorage', { getItem: () => { throw new Error('Blocked') }, setItem: () => { throw new Error('Full') } })
    const transport = client()
    rememberAgentConnection(transport, config, connected)
    expect(readAgentDetectionCache(transport).connections[config.id].result).toEqual(connected)
  })
})
