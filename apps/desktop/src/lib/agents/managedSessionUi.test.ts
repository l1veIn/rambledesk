import { describe, expect, it } from 'vitest'
import type { AgentConfig } from '$lib/generated/feedback'
import {
  activitiesForSession, managedSessionActions, managedSessionComposerState, permissionsForSession,
  sessionConfigurationChanged, SessionPromptDrafts,
  type ManagedSessionViewSnapshot, type SessionActivity, type SessionPermission,
} from './managedSessionUi'

function snapshot(): ManagedSessionViewSnapshot {
  return {
    deleting: false,
    session: {
      session_id: 'local-one', host_id: 'dsh', host_session_id: 'feedback-one', title: 'First project',
      created_at: '2026-09-04', updated_at: '2026-09-04',
      management: { kind: 'managed', protocol: 'acp', agent_config_id: 'config-one', cwd: '/project', remote_session_id: null },
    },
    runtime: { configuration: { options: [], modes: null, models: null },
      connection: 'connected', activity: 'idle', instance_id: 'instance-one', config_updated_at: 'old',
      capabilities: { prompt: { image: false, audio: false, embedded_context: false, resource_links: true }, load_session: true, resume_session: false, http_mcp: true }, last_error: null,
    },
  }
}

const activity = (id: string, sessionId: string, text: string): SessionActivity => ({
  id, session_id: sessionId, kind: 'tool_call', text, tool_call_id: `tool-${id}`, created_at: '2026-09-04',
})
const permission = (requestId: string, sessionId = 'local-one'): SessionPermission => ({
  request_id: requestId, session_id: sessionId, title: 'Run command?', details: null,
  options: [{ option_id: 'approve', name: 'Allow once', kind: 'allow_once' }],
})

describe('managed session views', () => {
  it('keeps draft editing available offline and during a turn while independently gating send and cancel', () => {
    const view = snapshot()
    const pending = { busy: false, lifecycle: false, prompt: false }
    expect(managedSessionComposerState(view, 0, pending)).toEqual({ disabled: false, busy: false, sendDisabled: false, canCancel: false })
    view.runtime.connection = 'stopped'
    expect(managedSessionComposerState(view, 0, pending)).toEqual({ disabled: false, busy: false, sendDisabled: true, canCancel: false })
    view.runtime.connection = 'connected'
    view.runtime.activity = 'running'
    expect(managedSessionComposerState(view, 0, pending)).toEqual({ disabled: false, busy: true, sendDisabled: true, canCancel: true })
    expect(managedSessionComposerState({ ...view, deleting: true }, 0, pending)).toMatchObject({ disabled: true, sendDisabled: true, canCancel: false })
  })
  it('disables every work action while deletion is incomplete without changing the history identity', () => {
    const deleting = { ...snapshot(), deleting: true }
    const actions = managedSessionActions(deleting, 1)
    expect(actions).toMatchObject({ canPrompt: false, canStart: false, canCancel: false })
    expect(deleting.session.session_id).toBe('local-one')
  })
  it('isolates activities and replaces repeated tool ids without reordering the timeline', () => {
    const visible = activitiesForSession('local-one', [
      activity('first', 'local-one', 'running'), activity('other', 'local-two', 'private project'),
      activity('second', 'local-one', 'message'), activity('first', 'local-one', 'completed'),
    ])
    expect(visible.map((item) => item.text)).toEqual(['completed', 'message'])
  })

  it('keeps permissions in backend FIFO order and excludes other sessions and duplicate requests', () => {
    expect(permissionsForSession('local-one', [
      permission('one'), permission('foreign', 'local-two'), permission('two'), permission('one'),
    ]).map((item) => item.request_id)).toEqual(['one', 'two'])
  })

  it('disables prompt submission for disconnected, running, and waiting-permission sessions', () => {
    const view = snapshot()
    expect(managedSessionActions(view, 0).canPrompt).toBe(true)
    expect(managedSessionActions(view, 1).canPrompt).toBe(false)
    for (const connection of ['stopped', 'connecting', 'disconnected', 'failed'] as const) {
      expect(managedSessionActions({ ...view, runtime: { ...view.runtime, connection } }, 0).canPrompt).toBe(false)
    }
    for (const activity of ['running', 'waiting_permission'] as const) {
      const actions = managedSessionActions({ ...view, runtime: { ...view.runtime, activity } }, 0)
      expect(actions.canPrompt).toBe(false)
      expect(actions.canCancel).toBe(true)
    }
  })

  it('offers resume for a bound offline session without guessing capabilities before the handshake', () => {
    const view = snapshot()
    view.session.management = { kind: 'managed', protocol: 'acp', agent_config_id: 'config-one', cwd: '/project', remote_session_id: 'remote-one' }
    view.runtime.connection = 'stopped'
    view.runtime.capabilities.load_session = false
    expect(managedSessionActions(view, 0)).toMatchObject({ canStart: true })
    view.runtime.connection = 'connecting'
    expect(managedSessionActions(view, 0)).toMatchObject({ canStart: false })
  })

  it('only reports configuration changes against the configuration used by this instance', () => {
    const config: AgentConfig = { id: 'config-one', name: 'One', host_id: 'dsh', protocol: 'acp', command: 'deepseek-acp', args: [], env: {}, enabled: true, created_at: 'old', updated_at: 'new' }
    expect(sessionConfigurationChanged(snapshot(), config)).toBe(true)
    expect(sessionConfigurationChanged(snapshot(), { ...config, id: 'different' })).toBe(false)
    expect(sessionConfigurationChanged(snapshot(), { ...config, updated_at: 'old' })).toBe(false)
    const view = snapshot()
    view.runtime.config_updated_at = null
    expect(sessionConfigurationChanged(view, config)).toBe(false)
  })
})

describe('session prompt drafts', () => {
  it('clears an accepted send attempt immediately and restores a failed attempt only when untouched', () => {
    const drafts = new SessionPromptDrafts()
    drafts.write('one', 'First task')
    const submitted = drafts.beginSubmission('one', 'First task')
    expect(drafts.read('one')).toBe('')
    drafts.write('two', 'Another session')
    expect(drafts.restoreSubmission(submitted)).toBe(true)
    expect(drafts.read('one')).toBe('First task')
    expect(drafts.read('two')).toBe('Another session')
  })

  it('never overwrites next-turn edits or revives a deleted session after an older send fails', () => {
    const drafts = new SessionPromptDrafts()
    const first = drafts.beginSubmission('one', 'First task')
    drafts.write('one', 'Next task')
    expect(drafts.restoreSubmission(first)).toBe(false)
    expect(drafts.read('one')).toBe('Next task')
    drafts.write('one', '')
    expect(drafts.restoreSubmission(first)).toBe(false)
    const deleted = drafts.beginSubmission('two', 'Deleted task')
    drafts.forgetSession('two')
    expect(drafts.restoreSubmission(deleted)).toBe(false)
  })

  it('keeps drafts distinct across session view switches', () => {
    const drafts = new SessionPromptDrafts()
    drafts.write('local-one', 'Work in project one')
    drafts.write('local-two', 'Work in project two')
    expect(drafts.read('local-one')).toBe('Work in project one')
    drafts.beginSubmission('local-one', 'Work in project one')
    expect(drafts.read('local-one')).toBe('')
    expect(drafts.read('local-two')).toBe('Work in project two')
  })
})
