import { describe, expect, it } from 'vitest'
import type { HostSessionSummary } from '$lib/generated/feedback'
import { agentSessionForView, agentViewForEmptyRamble, agentViewForRequest } from './agentViewRouting'
import { agentSessionViewDescriptor, sessionViewDescriptor, workspaceViewKey } from './viewDescriptors'
import { EMPTY_WORKSPACE_SHELL_STATE, workspaceShellReducer } from './workspaceShell'

const managed: HostSessionSummary = {
  session_id: 'local-agent', host_id: 'pi', host_session_id: 'visible-session', title: 'Project',
  management: { kind: 'managed', protocol: 'acp', agent_config_id: 'pi', cwd: '/repo', remote_session_id: 'remote-agent' },
  source_hint: null, request_count: 1, pending_count: 1, updated_at: 'now', pinned_at: null, archived_at: null, host_pinned_at: null,
}

describe('Agent view routing', () => {
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
