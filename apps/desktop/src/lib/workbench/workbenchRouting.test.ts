import { describe, expect, it } from 'vitest'

import {
  createWorkspaceLoadGate,
  ownerForOperation,
  type WorkbenchRequestOwner,
} from './workbenchRouting'

const adapterOwner: WorkbenchRequestOwner = {
  key: 'adapter-key',
  origin: 'adapter',
  requestId: 'same-id',
  sessionId: 'adapter-session',
}

const acpOwner: WorkbenchRequestOwner = {
  key: 'acp-key',
  origin: 'managed_acp',
  requestId: 'same-id',
  sessionId: 'acp-session',
}

describe('Workbench request routing', () => {
  it('keeps the Ramble owner distinct from the visible Workspace when ids collide', () => {
    expect(ownerForOperation('same-id', 'workspace', adapterOwner, acpOwner)).toBe(adapterOwner)
    expect(ownerForOperation('same-id', 'ramble', adapterOwner, acpOwner)).toBe(acpOwner)
  })

  it('rejects a response from an older request load', () => {
    const gate = createWorkspaceLoadGate()
    const first = gate.begin('request-a')
    const second = gate.begin('request-b')

    expect(gate.isCurrent(first, 'request-b')).toBe(false)
    expect(gate.isCurrent(second, 'request-b')).toBe(true)
  })

  it('invalidates an in-flight load when the Workspace is cleared', () => {
    const gate = createWorkspaceLoadGate()
    const pending = gate.begin('request-a')
    gate.invalidate()

    expect(gate.isCurrent(pending, null)).toBe(false)
  })
})
