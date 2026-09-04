import { describe, expect, it } from 'vitest'
import type { AgentConfig, FeedbackRequestSummary } from '$lib/generated/feedback'
import {
  activitiesForSession, feedbackForSession, managedSessionActions, permissionsForSession,
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
    runtime: {
      connection: 'connected', activity: 'idle', instance_id: 'instance-one', config_updated_at: 'old',
      capabilities: { load_session: true, resume_session: false, http_mcp: true }, last_error: null,
    },
  }
}

const activity = (id: string, sessionId: string, text: string): SessionActivity => ({
  id, session_id: sessionId, kind: 'tool_call', text, tool_call_id: `tool-${id}`, created_at: '2026-09-04',
})
const permission = (requestId: string, sessionId = 'local-one'): SessionPermission => ({
  request_id: requestId, session_id: sessionId, title: 'Run command?',
  options: [{ option_id: 'approve', name: 'Allow once', kind: 'allow_once' }],
})

describe('managed session views', () => {
  it('disables every work action while deletion is incomplete without changing the history identity', () => {
    const deleting = { ...snapshot(), deleting: true }
    const actions = managedSessionActions(deleting, 1)
    expect(actions).toMatchObject({ canPrompt: false, canStart: false, canCancel: false, canStop: false })
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
      expect(actions.canStop).toBe(true)
    }
  })

  it('offers resume for a bound offline session without guessing capabilities before the handshake', () => {
    const view = snapshot()
    view.session.management = { kind: 'managed', protocol: 'acp', agent_config_id: 'config-one', cwd: '/project', remote_session_id: 'remote-one' }
    view.runtime.connection = 'stopped'
    view.runtime.capabilities.load_session = false
    expect(managedSessionActions(view, 0)).toMatchObject({ canStart: true, canStop: false, startLabel: 'Resume session' })
    view.runtime.connection = 'connecting'
    expect(managedSessionActions(view, 0)).toMatchObject({ canStart: false, canStop: true })
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

  it('matches feedback on both legacy host identifiers without inventing ACP session identity', () => {
    const request = { request_id: 'feedback', host_id: 'dsh', host_session_id: 'feedback-one' } as FeedbackRequestSummary
    const requests = [request, { ...request, request_id: 'wrong-host', host_id: 'pi' }, { ...request, request_id: 'wrong-session', host_session_id: 'different' }]
    expect(feedbackForSession(snapshot().session, requests).map((item) => item.request_id)).toEqual(['feedback'])
  })
})

describe('session prompt drafts', () => {
  it('keeps drafts distinct across session view switches', () => {
    const drafts = new SessionPromptDrafts()
    drafts.write('local-one', 'Work in project one')
    drafts.write('local-two', 'Work in project two')
    expect(drafts.read('local-one')).toBe('Work in project one')
    drafts.accepted('local-one', 'Work in project one')
    expect(drafts.read('local-one')).toBe('')
    expect(drafts.read('local-two')).toBe('Work in project two')
  })

  it('retains a newer draft when a prior prompt finishes sending and removes only deleted sessions', () => {
    const drafts = new SessionPromptDrafts()
    drafts.write('one', 'First prompt')
    drafts.write('one', 'Next prompt')
    drafts.accepted('one', 'First prompt')
    expect(drafts.read('one')).toBe('Next prompt')
    drafts.write('two', 'Another project')
    drafts.remove('one')
    expect(drafts.read('one')).toBe('')
    expect(drafts.read('two')).toBe('Another project')
  })
})
