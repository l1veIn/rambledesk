import { describe, expect, it } from 'vitest'
import type { FeedbackRequestSummary, HostSessionSummary } from '$lib/generated/feedback'
import { agentSessionForView, agentViewForEmptyRamble, agentViewForRequest, cancelledFeedbackRestoreTarget } from './agentViewRouting'
import { agentSessionViewDescriptor, sessionViewDescriptor, workspaceViewKey } from './viewDescriptors'
import { EMPTY_WORKSPACE_SHELL_STATE, workspaceShellReducer } from './workspaceShell'

const managed: HostSessionSummary = {
  session_id: 'local-agent', host_id: 'pi', host_session_id: 'visible-session', title: 'Project',
  management: { kind: 'managed', protocol: 'acp', agent_config_id: 'pi', cwd: '/repo', remote_session_id: 'remote-agent' },
  source_hint: null, request_count: 1, pending_count: 1, updated_at: 'now', pinned_at: null, archived_at: null, host_pinned_at: null,
}

describe('Agent view routing', () => {
  const cancelled: FeedbackRequestSummary = {
    request_id: 'cancelled', managed_session_id: managed.session_id, host_id: managed.host_id, host_session_id: managed.host_session_id,
    source_hint: null, title: 'Cancelled feedback', what_happened: '', status: 'cancelled', resolution: 'cancelled',
    allow_finish: false, final_summary: null, revision: 1, created_at: '2026-09-06T00:00:00Z', updated_at: '2026-09-06T00:00:00Z',
  }

  it('restores the latest cancelled feedback as details while preserving the Agent tab for explicit navigation', () => {
    const session = { ...managed, pending_count: 0 }
    const target = cancelledFeedbackRestoreTarget(session, [cancelled])!
    expect(target).toEqual({ view: sessionViewDescriptor(managed.host_id, managed.host_session_id), requestId: 'cancelled' })
    const agent = agentSessionViewDescriptor(managed.session_id)
    const restored = workspaceShellReducer(EMPTY_WORKSPACE_SHELL_STATE, { type: 'open', view: agent })
    const paused = workspaceShellReducer(restored, { type: 'open', view: target.view })
    expect(paused.views).toEqual([agent, target.view])
    expect(paused.activeViewKey).toBe(workspaceViewKey(target.view))
    const resumed = workspaceShellReducer(paused, { type: 'open', view: agent })
    expect(resumed.activeViewKey).toBe(workspaceViewKey(agent))
  })

  it.each(['waiting', 'in_progress', 'completed'] as const)('ignores an older cancellation when the latest feedback is %s', status => {
    expect(cancelledFeedbackRestoreTarget({ ...managed, pending_count: 0 }, [
      { ...cancelled, request_id: 'newer', status, resolution: status === 'completed' ? 'feedback_submitted' : null }, cancelled,
    ])).toBeNull()
  })

  it('does not redirect empty, external, mismatched, or still-pending sessions', () => {
    const session = { ...managed, pending_count: 0 }
    expect(cancelledFeedbackRestoreTarget(session, [])).toBeNull()
    expect(cancelledFeedbackRestoreTarget(managed, [cancelled])).toBeNull()
    expect(cancelledFeedbackRestoreTarget({ ...session, management: { kind: 'external' } }, [cancelled])).toBeNull()
    expect(cancelledFeedbackRestoreTarget(session, [{ ...cancelled, managed_session_id: undefined }])).toBeNull()
    expect(cancelledFeedbackRestoreTarget(session, [{ ...cancelled, managed_session_id: 'another-agent' }])).toBeNull()
    expect(cancelledFeedbackRestoreTarget(session, [{ ...cancelled, host_session_id: 'another-session' }])).toBeNull()
  })

  it('opens an owning Agent by its durable request binding before navigation has loaded', () => {
    const view = agentViewForRequest({ managed_session_id: 'local-agent' })
    expect(view).toEqual(agentSessionViewDescriptor('local-agent'))
    expect(agentSessionForView(view, [])).toBeUndefined()
    expect(agentSessionForView(view, [managed])).toBe(managed)
  })

  it('keeps external requests external even when a matching Agent exists in navigation', () => {
    expect(agentViewForRequest({})).toBeNull()
    expect(agentViewForRequest(null)).toBeNull()
    expect(agentSessionForView(agentSessionViewDescriptor(managed.host_session_id), [managed])).toBeUndefined()
    expect(agentSessionForView(agentSessionViewDescriptor(managed.session_id), [{ ...managed, management: { kind: 'external' } }])).toBeUndefined()
  })

  it('keeps View Agent available on a managed Ramble session with no requests', () => {
    const ramble = sessionViewDescriptor(managed.host_id, managed.host_session_id)
    expect(agentViewForEmptyRamble(ramble, [managed])).toEqual(agentSessionViewDescriptor(managed.session_id))
    expect(agentViewForEmptyRamble(ramble, [{ ...managed, management: { kind: 'external' } }])).toBeNull()
    expect(agentViewForEmptyRamble(sessionViewDescriptor('another', managed.host_session_id), [managed])).toBeNull()
  })

  it('keeps the Ramble view open when viewing its Agent, then returns to the same Ramble tab', () => {
    const ramble = sessionViewDescriptor(managed.host_id, managed.host_session_id)
    const agent = agentViewForRequest({ managed_session_id: managed.session_id })!
    const rambleOpen = workspaceShellReducer(EMPTY_WORKSPACE_SHELL_STATE, { type: 'open', view: ramble })
    const agentOpen = workspaceShellReducer(rambleOpen, { type: 'open', view: agent })
    expect(agentOpen.views).toEqual([ramble, agent])
    expect(agentOpen.activeViewKey).toBe(workspaceViewKey(agent))
    const backToRamble = workspaceShellReducer(agentOpen, { type: 'open', view: ramble })
    expect(backToRamble.views).toEqual([ramble, agent])
    expect(backToRamble.activeViewKey).toBe(workspaceViewKey(ramble))
    expect(workspaceShellReducer(agentOpen, { type: 'close', viewKey: workspaceViewKey(agent) })).toEqual(rambleOpen)
  })
})
