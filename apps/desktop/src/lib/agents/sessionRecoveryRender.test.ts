import { describe, expect, it, vi } from 'vitest'
import { render } from 'svelte/server'
import type { SessionRecovery } from '$lib/generated/feedback'
import ManagedSessionWorkspace from './ManagedSessionWorkspace.svelte'
import SessionRecoveryNotice from './SessionRecoveryNotice.svelte'
import type { ManagedSessionViewSnapshot } from './managedSessionUi'

vi.mock('$lib/preferences', async () => {
  const { writable } = await import('svelte/store')
  return { locale: writable('en') }
})

function snapshot(connection: ManagedSessionViewSnapshot['runtime']['connection'] = 'stopped'): ManagedSessionViewSnapshot {
  return {
    deleting: false,
    session: { session_id: 'local', host_id: 'dsh', host_session_id: 'feedback', title: 'Interrupted work',
      created_at: 'today', updated_at: 'today',
      management: { kind: 'managed', protocol: 'acp', agent_config_id: 'config', cwd: '/repo', remote_session_id: 'original' } },
    runtime: { configuration: { options: [], modes: null, models: null }, connection, activity: 'idle', instance_id: null, config_updated_at: null,
      capabilities: { prompt: { image: false, audio: false, embedded_context: false, resource_links: true }, load_session: true, resume_session: false, http_mcp: true }, last_error: null },
  }
}

function recovery(status: SessionRecovery['status'] = 'interrupted'): SessionRecovery {
  return { session_id: 'local', status, run_id: 'run', active_turn_id: null,
    interrupted_turn_id: 'turn', last_error: 'Connection ended: private-token', updated_at: 'today' }
}

describe('managed session recovery rendering', () => {
  it('preserves history, redacts recovery errors, and offers explicit original-session recovery without any automatic work', () => {
    const action = vi.fn()
    const { body } = render(ManagedSessionWorkspace, { props: {
      snapshot: snapshot(), recovery: recovery(),
      config: { id: 'config', name: 'Agent', host_id: 'dsh', protocol: 'acp', enabled: true,
        command: 'deepseek-acp', args: [], env: { TOKEN: 'private-token' }, created_at: 'today', updated_at: 'today' },
      activities: [{ id: 'past', session_id: 'local', kind: 'agent_message', text: 'Preserved output', tool_call_id: null, created_at: 'today' }],
      onPrompt: action, onStart: action, onStop: action, onCancel: action, onRespondPermission: action, onOpenFeedback: action,
    } })
    expect(body).toContain('previous agent turn was interrupted')
    expect(body).toContain('will not be sent again automatically')
    expect(body).toContain('original agent session')
    expect(body).toContain('Resume session')
    expect(body).toContain('Stopped')
    expect(body).toContain('Preserved output')
    expect(body).not.toContain('private-token')
    expect(action).not.toHaveBeenCalled()
  })

  it('uses Start when no remote session exists and never suggests replacing the original session', () => {
    const state = snapshot()
    if (state.session.management.kind === 'managed') state.session.management.remote_session_id = null
    const { body } = render(SessionRecoveryNotice, { props: { snapshot: state, recovery: recovery() } })
    expect(body).toContain('No agent session was established')
    expect(body).toContain('Start agent')
    expect(body).not.toContain('Resume session')
  })

  it('treats an unclosed checkpoint as history and suppresses stale notices during a live connection', () => {
    const { body } = render(SessionRecoveryNotice, { props: { snapshot: snapshot(), recovery: recovery('unclosed') } })
    expect(body).toContain('previous agent run did not finish closing')
    for (const connection of ['connected', 'connecting'] as const) {
      const result = render(SessionRecoveryNotice, { props: { snapshot: snapshot(connection), recovery: recovery('unclosed') } })
      expect(result.body).not.toContain('previous agent run')
    }
  })

  it('does not expose another session checkpoint or offer recovery during deletion or a clean stop', () => {
    for (const props of [
      { snapshot: snapshot(), recovery: { ...recovery(), session_id: 'another-session' } },
      { snapshot: { ...snapshot(), deleting: true }, recovery: recovery() },
      { snapshot: snapshot(), recovery: recovery('stopped') },
      { snapshot: snapshot(), recovery: recovery('never_started') },
    ]) {
      const { body } = render(SessionRecoveryNotice, { props })
      expect(body).not.toContain('Resume session')
      expect(body).not.toContain('private-token')
    }
  })
})
