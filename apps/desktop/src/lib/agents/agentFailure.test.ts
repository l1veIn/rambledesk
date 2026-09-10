import { describe, expect, it, vi } from 'vitest'
import { render } from 'svelte/server'
import type { AgentFailure } from '$lib/generated/feedback'
import { agentFailureFrom } from './agentFailure'
import AgentFailureNotice from './AgentFailureNotice.svelte'

vi.mock('$lib/preferences', async () => ({ locale: (await import('svelte/store')).writable('en') }))

describe('action-specific agent failures', () => {
  it('does not infer authentication from unclassified prose', () => {
    const failure = agentFailureFrom(new Error('Authentication, network, or another unknown failure'), 'session')
    expect(failure.reason).toBe('unknown')
    const body = render(AgentFailureNotice, { props: { failure } }).body
    expect(body).toContain('Could not prepare this session')
    expect(body).not.toContain('data-agent-setup-guide')
    expect(body).not.toContain('Run in a terminal')
  })

  it('shows native authentication guidance only for an explicit authentication failure, with current connection context', () => {
    const failure = agentFailureFrom({ failure: { stage: 'session', reason: 'authentication', message: 'API key credential-secret rejected' } }, 'session', 'API_KEY=credential-secret')
    const body = render(AgentFailureNotice, { props: {
      failure, name: 'Work Claude', hostId: 'claude', onConfigure: vi.fn(),
      inspection: { agent_id: 'claude-acp', source: 'managed', command: '/opt/node', args: [], version: null, checks: [],
        dependencies: [{ command: 'claude', path: '/opt/claude', version: null, required: false }] },
    } }).body
    expect(body).toContain('data-agent-setup-guide')
    expect(body).toContain('Work Claude')
    expect(body).toContain('same machine and user account')
    expect(body).toContain('Configure an API key in advanced settings')
    expect(body).toContain('/opt/claude')
    expect(body).not.toContain('credential-secret')
  })

  it.each(['model', 'configuration', 'rate_limit', 'network', 'connection', 'unknown'] as const)('does not route %s failures to sign-in', reason => {
    const failure: AgentFailure = { stage: 'prompt', reason, message: 'Provider failure details' }
    const body = render(AgentFailureNotice, { props: { failure, hostId: 'claude', onConfigure: vi.fn() } }).body
    expect(body).toContain('The agent could not complete this message')
    expect(body).not.toContain('data-agent-setup-guide')
    expect(body).not.toContain('Authenticate')
  })
})
