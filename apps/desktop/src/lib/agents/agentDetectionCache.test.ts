import { describe, expect, it } from 'vitest'
import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
import type { AgentConfig, AgentConnectionCheck } from '$lib/generated/feedback'
import { readAgentDetectionCache, rememberAgentConnection } from './agentDetectionCache'

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
