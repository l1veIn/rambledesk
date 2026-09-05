import { describe, expect, it, vi } from 'vitest'
import { render } from 'svelte/server'
import ManagedSessionWorkspace from './ManagedSessionWorkspace.svelte'
import type { ManagedSessionViewSnapshot } from './managedSessionUi'

vi.mock('$lib/preferences', async () => {
  const { writable } = await import('svelte/store')
  return { locale: writable('en') }
})

describe('Managed session rendering', () => {
  function pageSnapshot(): ManagedSessionViewSnapshot {
    return {
      deleting: false,
      session: { session_id: 'compact-ui', host_id: 'claude', host_session_id: 'feedback-compact', title: 'Simplify the workspace', created_at: 'today', updated_at: 'today',
        management: { kind: 'managed', protocol: 'acp', agent_config_id: 'config', cwd: '/work/project', remote_session_id: 'remote' } },
      runtime: { configuration: { options: [], modes: null, models: { current_model_id: 'fast', available_models: [{ model_id: 'fast', name: 'Fast model', description: null }] } },
        connection: 'connected', activity: 'idle', instance_id: 'instance', config_updated_at: null, context_usage: { used: 2560, size: 10240 },
        capabilities: { load_session: true, resume_session: false, http_mcp: false, prompt: { image: false, audio: false, embedded_context: false, resource_links: true } }, last_error: null },
    }
  }

  it('keeps the heading focused and moves project details below the shared-width composer', () => {
    const action = vi.fn()
    const { body } = render(ManagedSessionWorkspace, { props: {
      snapshot: pageSnapshot(), branch: 'codex/compact-ui', onOpenRamble: action, onPrompt: action,
      onCancel: action, onStart: action, onRespondPermission: action, onSetConfiguration: action,
    } })
    const heading = body.match(/<header\b[^>]*>[\s\S]*?<\/header>/)?.[0] ?? ''
    expect(heading).toContain('Simplify the workspace')
    expect(heading).toContain('View Ramble')
    expect(heading).not.toContain('/work/project')
    expect(heading).not.toContain('Connected')
    expect(heading).not.toContain('Idle')
    expect(body).toContain('Fast model')
    expect(body).toContain('data-workspace-metadata')
    expect(body).toContain('title="/work/project"')
    expect(body).toContain('title="codex/compact-ui"')
    expect(body).toContain('Context 25%')
    expect(body).toMatch(/max-w-4xl[^>]*data-agent-timeline/)
    expect(body).toMatch(/max-w-4xl[^>]*data-agent-composer/)
    expect(body).not.toContain('Feedback requests')
    expect(body).not.toContain('Quote in message')
    expect(body).not.toContain('Delete session')
    expect(body).not.toContain('Stop agent')
    expect(body).not.toContain('type="file"')
    expect(body).not.toContain('aria-label="Add attachment"')
    expect(body.slice(body.indexOf('data-agent-composer'))).toContain('<svg')
    expect(action).not.toHaveBeenCalled()
  })

  it('only offers a connection retry for a visible error and never during deletion', () => {
    const action = vi.fn()
    const snapshot = pageSnapshot()
    const props = { snapshot, onPrompt: action, onCancel: action, onStart: action, onRespondPermission: action }
    const ready = render(ManagedSessionWorkspace, { props }).body
    expect(ready).not.toContain('Retry')
    expect(ready).not.toContain('codex/compact-ui')
    const offline = { ...snapshot, runtime: { ...snapshot.runtime, connection: 'failed' as const, instance_id: null } }
    const failed = render(ManagedSessionWorkspace, { props: { ...props, snapshot: offline, connectionError: 'Connection timed out' } }).body
    expect(failed).toContain('Connection timed out')
    expect(failed).toContain('Retry')
    const deleting = render(ManagedSessionWorkspace, { props: { ...props, snapshot: { ...offline, deleting: true }, connectionError: 'Connection timed out' } }).body
    expect(deleting).not.toMatch(/<button[^>]*>[\s\S]*?>Retry</)
    expect(action).not.toHaveBeenCalled()
  })

  it('offers read retry for query failures without treating them or turn failures as connection attempts', () => {
    const onStart = vi.fn()
    const onPrompt = vi.fn()
    const onRefresh = vi.fn()
    const base = pageSnapshot()
    const props = { onPrompt, onStart, onRefresh, onCancel: vi.fn(), onRespondPermission: vi.fn() }
    for (const connection of ['connected', 'stopped'] as const) {
      const snapshot = { ...base, runtime: { ...base.runtime, connection } }
      const { body } = render(ManagedSessionWorkspace, { props: { ...props, snapshot, error: 'Could not load agent configurations.' } })
      expect(body).toContain('aria-label="Reload session"')
      expect(body).not.toContain('aria-label="Retry connection"')
    }
    const connected = render(ManagedSessionWorkspace, { props: {
      ...props, snapshot: { ...base, runtime: { ...base.runtime, last_error: 'Prompt failed; draft preserved' } },
    } }).body
    expect(connected).toContain('Prompt failed; draft preserved')
    expect(connected).not.toContain('aria-label="Retry connection"')
    expect(connected).not.toContain('aria-label="Reload session"')
    const failed = render(ManagedSessionWorkspace, { props: {
      ...props, snapshot: { ...base, runtime: { ...base.runtime, connection: 'failed', last_error: 'Connection failed' } },
    } }).body
    expect(failed).toContain('aria-label="Retry connection"')
    expect(failed).not.toContain('aria-label="Reload session"')
    expect(onStart).not.toHaveBeenCalled()
    expect(onPrompt).not.toHaveBeenCalled()
    expect(onRefresh).not.toHaveBeenCalled()
  })

  it('initially mounts twenty prompts and keeps an accessible fallback for loading older history', () => {
    const snapshot: ManagedSessionViewSnapshot = {
      deleting: false,
      session: { session_id: 'history-window', host_id: 'dsh', host_session_id: 'feedback-history', title: 'Long history', created_at: 'today', updated_at: 'today',
        management: { kind: 'managed', protocol: 'acp', agent_config_id: 'config', cwd: '/repo', remote_session_id: 'remote' } },
      runtime: { configuration: { options: [], modes: null, models: null }, connection: 'connected', activity: 'idle', instance_id: 'instance', config_updated_at: null,
        capabilities: { load_session: true, resume_session: false, http_mcp: true, prompt: { image: false, audio: false, embedded_context: false, resource_links: true } }, last_error: null },
    }
    const action = vi.fn()
    const { body } = render(ManagedSessionWorkspace, { props: {
      snapshot, onPrompt: action, onStart: action, onCancel: action, onRespondPermission: action, onLoadOlder: action,
      activities: Array.from({ length: 1000 }, (_, index) => ({ id: `history-${index + 1}`, sequence: index + 1, session_id: 'history-window', kind: 'user_message' as const, text: `Message ${index + 1}`, tool_call_id: null, created_at: 'today' })),
    } })
    expect(body.match(/data-activity-id=/g)).toHaveLength(20)
    expect(body).toContain('Message 981')
    expect(body).toContain('Message 1000')
    expect(body).not.toContain('Message 980')
    expect(body).toContain('Load earlier messages')
    expect(action).not.toHaveBeenCalled()
  })

  it('shows only the active permission details as escaped, redacted text before approval', () => {
    const snapshot: ManagedSessionViewSnapshot = {
      deleting: false,
      session: { session_id: 'local-details', host_id: 'dsh', host_session_id: 'feedback-details', title: 'Permission context',
        created_at: 'today', updated_at: 'today',
        management: { kind: 'managed', protocol: 'acp', agent_config_id: 'config', cwd: '/repo', remote_session_id: 'remote' } },
      runtime: { configuration: { options: [], modes: null, models: null }, connection: 'connected', activity: 'waiting_permission', instance_id: 'instance', config_updated_at: null,
        capabilities: { prompt: { image: false, audio: false, embedded_context: false, resource_links: true }, load_session: true, resume_session: false, http_mcp: true }, last_error: null },
    }
    const action = vi.fn()
    const { body } = render(ManagedSessionWorkspace, { props: {
      snapshot, config: { id: 'config', name: 'Agent', host_id: 'dsh', protocol: 'acp', enabled: true,
        command: 'agent', args: [], env: { TOKEN: 'private-permission-token' }, created_at: 'today', updated_at: 'today' },
      permissions: [
        { request_id: 'active', session_id: 'local-details', title: 'Run command?', details: 'command: cat /repo/input\nTOKEN=private-permission-token\n<script>untrusted()</script>', options: [{ option_id: 'allow', name: 'Allow once', kind: 'allow_once' }] },
        { request_id: 'foreign', session_id: 'another-session', title: 'Foreign permission', details: 'Foreign operation details', options: [] },
        { request_id: 'queued', session_id: 'local-details', title: 'Queued permission', details: 'Queued operation details', options: [] },
      ],
      onPrompt: action, onStart: action, onCancel: action, onRespondPermission: action,
    } })
    expect(body).toContain('Operation details')
    expect(body).toMatch(/<details[^>]*open/)
    expect(body).toContain('command: cat /repo/input\nTOKEN=[redacted]')
    expect(body).toMatch(/&lt;script(?:&gt;|>)untrusted\(\)&lt;\/script(?:&gt;|>)/)
    expect(body).not.toContain('<script>')
    expect(body).not.toContain('private-permission-token')
    expect(body).not.toContain('Foreign operation details')
    expect(body).not.toContain('Queued operation details')
    expect(action).not.toHaveBeenCalled()
  })

  it('keeps history and the deletion warning while disabling work for a deleting session', () => {
    const snapshot: ManagedSessionViewSnapshot = {
      deleting: true,
      session: { session_id: 'deleting-render', host_id: 'dsh', host_session_id: 'feedback-deleting', title: 'Cleanup incomplete',
        created_at: 'today', updated_at: 'today',
        management: { kind: 'managed', protocol: 'acp', agent_config_id: 'config', cwd: '/repo', remote_session_id: 'remote' } },
      runtime: { configuration: { options: [], modes: null, models: null }, connection: 'connected', activity: 'running', instance_id: 'instance', config_updated_at: null,
        capabilities: { prompt: { image: false, audio: false, embedded_context: false, resource_links: true }, load_session: true, resume_session: false, http_mcp: true }, last_error: 'Cleanup failed' },
    }
    const action = vi.fn()
    const { body } = render(ManagedSessionWorkspace, { props: {
      snapshot, onPrompt: action, onStart: action, onCancel: action,
      onRespondPermission: action,
      activities: [{ id: 'past', session_id: 'deleting-render', kind: 'agent_message', text: 'Readable history', tool_call_id: null, created_at: 'today' }],
    } })
    expect(body).toContain('Readable history')
    expect(body).toContain('Retry deletion to finish cleanup')
    expect(body).not.toContain('Delete session')
    expect(body).not.toContain('Stop agent')
    expect(body).not.toContain('Cancel turn')
    expect(body).toMatch(/<button[^>]*disabled[^>]*aria-label="Send message"/)
    expect(body).not.toContain('aria-label="Cancel current turn"')
    expect(action).not.toHaveBeenCalled()
  })

  it('keeps a new session free of manual connection and deletion controls', () => {
    const snapshot: ManagedSessionViewSnapshot = {
      deleting: false,
      session: { session_id: 'local-empty', host_id: 'dsh', host_session_id: 'feedback-empty', title: 'Empty project',
        created_at: '2026-09-04', updated_at: '2026-09-04',
        management: { kind: 'managed', protocol: 'acp', agent_config_id: 'config', cwd: '/repo', remote_session_id: null } },
      runtime: { configuration: { options: [], modes: null, models: null }, connection: 'stopped', activity: 'idle', instance_id: null,
        config_updated_at: null, capabilities: { prompt: { image: false, audio: false, embedded_context: false, resource_links: true }, load_session: false, resume_session: false, http_mcp: false }, last_error: null },
    }
    const runtimeAction = vi.fn()
    const { body } = render(ManagedSessionWorkspace, { props: {
      snapshot, onPrompt: runtimeAction, onCancel: runtimeAction, onStart: runtimeAction,
      onRespondPermission: runtimeAction,
    } })
    expect(body).toContain('Empty project')
    expect(body).toContain('No messages yet')
    expect(body).not.toContain('Start agent')
    expect(body).not.toContain('Delete session')
    expect(runtimeAction).not.toHaveBeenCalled()
  })

  it('shows only current-session activity and the first permission without starting or stopping anything', () => {
    const snapshot: ManagedSessionViewSnapshot = {
      deleting: false,
      session: { session_id: 'local-one', host_id: 'dsh', host_session_id: 'feedback-one', title: 'Project one',
        created_at: '2026-09-04', updated_at: '2026-09-04',
        management: { kind: 'managed', protocol: 'acp', agent_config_id: 'config-one', cwd: '/repo', remote_session_id: 'remote-one' } },
      runtime: { configuration: { options: [], modes: null, models: null }, connection: 'connected', activity: 'waiting_permission', instance_id: 'instance-one',
        config_updated_at: null, capabilities: { prompt: { image: false, audio: false, embedded_context: false, resource_links: true }, load_session: true, resume_session: false, http_mcp: true }, last_error: null },
    }
    const runtimeAction = vi.fn()
    const { body } = render(ManagedSessionWorkspace, { props: {
      snapshot,
      activities: [
        { id: 'one', session_id: 'local-one', kind: 'agent_message', text: 'Current project output', tool_call_id: null, created_at: '2026-09-04' },
        { id: 'two', session_id: 'local-two', kind: 'agent_message', text: 'Foreign project output', tool_call_id: null, created_at: '2026-09-04' },
      ],
      permissions: [
        { request_id: 'first', session_id: 'local-one', title: 'First permission title', details: null, options: [{ option_id: 'once', name: 'Allow precisely once', kind: 'allow_once' }] },
        { request_id: 'foreign', session_id: 'local-two', title: 'Foreign permission title', details: null, options: [] },
        { request_id: 'second', session_id: 'local-one', title: 'Second permission title', details: null, options: [] },
      ],
      onPrompt: runtimeAction, onCancel: runtimeAction, onStart: runtimeAction,
      onRespondPermission: runtimeAction,
    } })
    expect(body).toContain('Current project output')
    expect(body).not.toContain('Foreign project output')
    expect(body).toContain('First permission title')
    expect(body).toContain('Allow precisely once')
    expect(body).not.toContain('Second permission title')
    expect(body).not.toContain('Foreign permission title')
    expect(body).not.toContain('Operation details')
    expect(runtimeAction).not.toHaveBeenCalled()
  })
})
