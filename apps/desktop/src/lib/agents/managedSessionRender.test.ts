import { describe, expect, it, vi } from 'vitest'
import { render } from 'svelte/server'
import ManagedSessionWorkspace from './ManagedSessionWorkspace.svelte'
import type { ManagedSessionViewSnapshot } from './managedSessionUi'

vi.mock('$lib/preferences', async () => {
  const { writable } = await import('svelte/store')
  return { locale: writable('en') }
})

describe('Managed session rendering', () => {
  it('keeps history and deletion available while disabling work for a deleting session', () => {
    const snapshot: ManagedSessionViewSnapshot = {
      deleting: true,
      session: { session_id: 'deleting-render', host_id: 'dsh', host_session_id: 'feedback-deleting', title: 'Cleanup incomplete',
        created_at: 'today', updated_at: 'today',
        management: { kind: 'managed', protocol: 'acp', agent_config_id: 'config', cwd: '/repo', remote_session_id: 'remote' } },
      runtime: { connection: 'connected', activity: 'running', instance_id: 'instance', config_updated_at: null,
        capabilities: { load_session: true, resume_session: false, http_mcp: true }, last_error: 'Cleanup failed' },
    }
    const action = vi.fn()
    const { body } = render(ManagedSessionWorkspace, { props: {
      snapshot, onPrompt: action, onStart: action, onStop: action, onCancel: action,
      onRespondPermission: action, onDelete: action, onOpenFeedback: action,
      activities: [{ id: 'past', session_id: 'deleting-render', kind: 'agent_message', text: 'Readable history', tool_call_id: null, created_at: 'today' }],
    } })
    expect(body).toContain('Readable history')
    expect(body).toContain('Retry deletion to finish cleanup')
    expect(body).toContain('Delete session')
    expect(body).not.toContain('Stop agent')
    expect(body).not.toContain('Cancel turn')
    expect(body).toMatch(/<textarea[^>]*disabled/)
    expect(action).not.toHaveBeenCalled()
  })

  it('hides deletion until the owning application provides the deletion operation', () => {
    const snapshot: ManagedSessionViewSnapshot = {
      deleting: false,
      session: { session_id: 'local-empty', host_id: 'dsh', host_session_id: 'feedback-empty', title: 'Empty project',
        created_at: '2026-09-04', updated_at: '2026-09-04',
        management: { kind: 'managed', protocol: 'acp', agent_config_id: 'config', cwd: '/repo', remote_session_id: null } },
      runtime: { connection: 'stopped', activity: 'idle', instance_id: null,
        config_updated_at: null, capabilities: { load_session: false, resume_session: false, http_mcp: false }, last_error: null },
    }
    const runtimeAction = vi.fn()
    const { body } = render(ManagedSessionWorkspace, { props: {
      snapshot, onPrompt: runtimeAction, onCancel: runtimeAction, onStart: runtimeAction,
      onStop: runtimeAction, onRespondPermission: runtimeAction, onOpenFeedback: runtimeAction,
    } })
    expect(body).toContain('Empty project')
    expect(body).toContain('No messages yet')
    expect(body).toContain('Start agent')
    expect(body).not.toContain('Delete session')
    expect(runtimeAction).not.toHaveBeenCalled()
  })

  it('shows only current-session activity and the first permission without starting or stopping anything', () => {
    const snapshot: ManagedSessionViewSnapshot = {
      deleting: false,
      session: { session_id: 'local-one', host_id: 'dsh', host_session_id: 'feedback-one', title: 'Project one',
        created_at: '2026-09-04', updated_at: '2026-09-04',
        management: { kind: 'managed', protocol: 'acp', agent_config_id: 'config-one', cwd: '/repo', remote_session_id: 'remote-one' } },
      runtime: { connection: 'connected', activity: 'waiting_permission', instance_id: 'instance-one',
        config_updated_at: null, capabilities: { load_session: true, resume_session: false, http_mcp: true }, last_error: null },
    }
    const runtimeAction = vi.fn()
    const { body } = render(ManagedSessionWorkspace, { props: {
      snapshot,
      activities: [
        { id: 'one', session_id: 'local-one', kind: 'agent_message', text: 'Current project output', tool_call_id: null, created_at: '2026-09-04' },
        { id: 'two', session_id: 'local-two', kind: 'agent_message', text: 'Foreign project output', tool_call_id: null, created_at: '2026-09-04' },
      ],
      permissions: [
        { request_id: 'first', session_id: 'local-one', title: 'First permission title', options: [{ option_id: 'once', name: 'Allow precisely once', kind: 'allow_once' }] },
        { request_id: 'foreign', session_id: 'local-two', title: 'Foreign permission title', options: [] },
        { request_id: 'second', session_id: 'local-one', title: 'Second permission title', options: [] },
      ],
      onPrompt: runtimeAction, onCancel: runtimeAction, onStart: runtimeAction, onStop: runtimeAction,
      onRespondPermission: runtimeAction, onDelete: runtimeAction, onOpenFeedback: runtimeAction,
    } })
    expect(body).toContain('Current project output')
    expect(body).not.toContain('Foreign project output')
    expect(body).toContain('First permission title')
    expect(body).toContain('Allow precisely once')
    expect(body).not.toContain('Second permission title')
    expect(body).not.toContain('Foreign permission title')
    expect(runtimeAction).not.toHaveBeenCalled()
  })
})
