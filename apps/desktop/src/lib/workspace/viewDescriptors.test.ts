import { describe, expect, it } from 'vitest'

import { sessionViewDescriptor, workspaceViewKey } from './viewDescriptors'

describe('workspace view descriptors', () => {
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
})
