import { describe, expect, it } from 'vitest'
import { TestApplicationTransport } from '$lib/application/testApplicationTransport'
import type { HostSessionSummary } from '$lib/generated/feedback'
import { deleteSessionRecord, removeManagedSessionViews } from './managedSessionDeletion'
import { agentSessionViewDescriptor, requestTaskViewDescriptor, sessionViewDescriptor, workspaceViewKey } from '$lib/workspace/viewDescriptors'
import { sessionPromptDrafts } from './managedSessionUi'

function session(id: string, managed = true): HostSessionSummary {
  return { session_id: id, host_id: 'dsh', host_session_id: `host-${id}`, title: id,
    management: managed ? { kind: 'managed', protocol: 'acp', agent_config_id: 'config', cwd: '/repo', remote_session_id: 'remote' } : { kind: 'external' },
    source_hint: null, request_count: 0, pending_count: 0, updated_at: 'today', pinned_at: null, archived_at: null, host_pinned_at: null }
}

describe('managed session deletion', () => {
  it('closes the Agent conversation and related Ramble views together', () => {
    const agent = agentSessionViewDescriptor('one')
    const ramble = sessionViewDescriptor('dsh', 'host-one')
    const brief = requestTaskViewDescriptor('request-one')
    const other = agentSessionViewDescriptor('two')
    const result = removeManagedSessionViews({ views: [ramble, agent, brief, other], activeViewKey: workspaceViewKey(agent) }, session('one'), ['request-one'])
    expect(result.closedActive).toBe(true)
    expect(result.shell.views).toEqual([other])
    expect(result.shell.activeViewKey).toBe(workspaceViewKey(other))
  })
  it('closes only the matching host pair and its known task view while preserving another active session', () => {
    const target = sessionViewDescriptor('dsh', 'host-one')
    const other = sessionViewDescriptor('dsh', 'host-two')
    const brief = requestTaskViewDescriptor('request-one')
    const result = removeManagedSessionViews({ views: [target, other, brief], activeViewKey: workspaceViewKey(other) }, session('one'), ['request-one'])
    expect(result.shell).toEqual({ views: [other], activeViewKey: workspaceViewKey(other) })
    expect(result.closedActive).toBe(false)
    expect(removeManagedSessionViews({ views: [target, other], activeViewKey: workspaceViewKey(target) }, session('one'), []).closedActive).toBe(true)
  })
  it('keeps the owning draft until successful cleanup, then rejects late unmount writes', async () => {
    let complete!: () => void
    const transport = new TestApplicationTransport().handle('deleteManagedSession', () => new Promise<void>((resolve) => { complete = resolve }))
    sessionPromptDrafts.write('deleting-one', 'keep until confirmed')
    sessionPromptDrafts.write('unrelated', 'another project')
    const deletion = deleteSessionRecord(transport, session('deleting-one'))
    expect(sessionPromptDrafts.read('deleting-one')).toBe('keep until confirmed')
    complete()
    await deletion
    sessionPromptDrafts.write('deleting-one', 'late destroyed editor draft')
    expect(sessionPromptDrafts.read('deleting-one')).toBe('')
    expect(sessionPromptDrafts.read('unrelated')).toBe('another project')
    expect(transport.calls).toEqual([{ name: 'deleteManagedSession', input: { session_id: 'deleting-one' } }])
  })

  it('preserves drafts after cleanup failure and permits a later explicit retry', async () => {
    const transport = new TestApplicationTransport().reject('deleteManagedSession', new Error('Process did not stop'))
    sessionPromptDrafts.write('failed-delete', 'draft after failure')
    await expect(deleteSessionRecord(transport, session('failed-delete'))).rejects.toThrow('Process did not stop')
    expect(sessionPromptDrafts.read('failed-delete')).toBe('draft after failure')
    expect(transport.callsFor('deleteManagedSession')).toHaveLength(1)
    transport.resolve('deleteManagedSession', undefined)
    await deleteSessionRecord(transport, session('failed-delete'))
    expect(transport.callsFor('deleteManagedSession')).toHaveLength(2)
    expect(sessionPromptDrafts.read('failed-delete')).toBe('')
  })

  it('routes managed sessions directly regardless of request count or archive state and preserves the external command', async () => {
    const transport = new TestApplicationTransport().resolve('deleteManagedSession', undefined).resolve('deleteHostSession', undefined)
    await deleteSessionRecord(transport, { ...session('busy-delete'), pending_count: 4, request_count: 5 })
    await deleteSessionRecord(transport, { ...session('archived-delete'), archived_at: 'today' })
    await deleteSessionRecord(transport, session('external-delete', false))
    expect(transport.callsFor('deleteManagedSession').map((call) => call.input.session_id)).toEqual(['busy-delete', 'archived-delete'])
    expect(transport.callsFor('deleteHostSession')[0].input).toEqual({ host_id: 'dsh', host_session_id: 'host-external-delete' })
    expect(transport.callsFor('archiveHostSession')).toHaveLength(0)
  })
})
