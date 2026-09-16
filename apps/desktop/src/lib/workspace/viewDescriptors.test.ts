import { describe, expect, it } from 'vitest'

import {
  agentSessionViewDescriptor,
  archiveViewDescriptor,
  fileDiffViewDescriptor,
  inboxViewDescriptor,
  rambelleProfileViewDescriptor,
  requestTaskViewDescriptor,
  sessionViewDescriptor,
  settingsViewDescriptor,
  workspaceViewKey,
} from './viewDescriptors'

describe('workspace view descriptors', () => {
  it('gives Agent conversations a durable identity separate from a Ramble session', () => {
    const agent = agentSessionViewDescriptor('session:one')
    expect(agent).toEqual({ kind: 'agent-session', sessionId: 'session:one' })
    expect(workspaceViewKey(agent)).toBe('agent-session:"session:one"')
    expect(workspaceViewKey(agent)).not.toBe(workspaceViewKey(sessionViewDescriptor('pi', 'session:one')))
  })
  it('uses one stable key for the aggregate Inbox view', () => {
    expect(workspaceViewKey(inboxViewDescriptor())).toBe('inbox:singleton')
  })

  it('creates a readonly session descriptor with a type-prefixed stable key', () => {
    const first = sessionViewDescriptor('codex', 'session-1')
    const second = sessionViewDescriptor('codex', 'session-1')

    expect(first).toEqual({
      kind: 'session',
      hostId: 'codex',
      hostSessionId: 'session-1',
    })
    expect(workspaceViewKey(first)).toBe('session:["codex","session-1"]')
    expect(workspaceViewKey(second)).toBe(workspaceViewKey(first))
  })

  it('distinguishes the same session id across hosts', () => {
    const codex = sessionViewDescriptor('codex', 'shared')
    const pi = sessionViewDescriptor('pi', 'shared')

    expect(workspaceViewKey(codex)).not.toBe(workspaceViewKey(pi))
  })

  it('does not collide when identity fields contain separators', () => {
    const left = sessionViewDescriptor('host:one', 'session')
    const right = sessionViewDescriptor('host', 'one:session')

    expect(workspaceViewKey(left)).not.toBe(workspaceViewKey(right))
  })

  it('gives every settings entry the same singleton key', () => {
    const first = settingsViewDescriptor()
    const second = settingsViewDescriptor()

    expect(first).toEqual({ kind: 'settings' })
    expect(workspaceViewKey(first)).toBe('settings:singleton')
    expect(workspaceViewKey(second)).toBe(workspaceViewKey(first))
  })

  it('gives the archive workspace one singleton key', () => {
    expect(archiveViewDescriptor()).toEqual({ kind: 'archive' })
    expect(workspaceViewKey(archiveViewDescriptor())).toBe('archive:singleton')
  })

  it('keys request tasks by request id and keeps profile singleton', () => {
    const task = requestTaskViewDescriptor('request:one')

    expect(workspaceViewKey(task)).toBe('request-task:"request:one"')
    expect(workspaceViewKey(task)).toBe(
      workspaceViewKey(requestTaskViewDescriptor('request:one')),
    )
    expect(workspaceViewKey(task)).not.toBe(
      workspaceViewKey(requestTaskViewDescriptor('request')),
    )
    expect(workspaceViewKey(rambelleProfileViewDescriptor())).toBe(
      'rambelle-profile:singleton',
    )
  })

  it('keys a file diff by its session, turn and path so turns never share a tab', () => {
    const first = fileDiffViewDescriptor({ id: 'session:turn-a:/repo/main.ts', path: '/repo/main.ts', diff: '--- a\n+++ b' })
    const second = fileDiffViewDescriptor({ id: 'session:turn-b:/repo/main.ts', path: '/repo/main.ts', diff: '--- a\n+++ b' })

    expect(first).toMatchObject({ kind: 'file-diff', path: '/repo/main.ts' })
    expect(workspaceViewKey(first)).toBe('file-diff:"session:turn-a:/repo/main.ts"')
    expect(workspaceViewKey(second)).not.toBe(workspaceViewKey(first))
    expect(workspaceViewKey(first)).toBe(
      workspaceViewKey(fileDiffViewDescriptor({ id: first.id, path: first.path, diff: first.diff })),
    )
  })
})
