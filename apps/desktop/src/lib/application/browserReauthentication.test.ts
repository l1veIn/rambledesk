import { describe, expect, it, vi } from 'vitest'

import { sessionViewDescriptor, workspaceViewKey } from '../workspace/viewDescriptors'
import { createWorkspaceTransition } from '../workspace/workspaceTransition'
import { createApplicationSnapshotRefetch } from './applicationSnapshotRefetch'
import { replaceReadyApplicationTransport } from './browserReauthentication'
import { ReplaceableApplicationTransport } from './replaceableApplicationTransport'
import { TestApplicationTransport } from './testApplicationTransport'

describe('browser reauthentication', () => {
  it('waits for new ready, then refetches all through the dirty-editor save gate', async () => {
    const first = new TestApplicationTransport(undefined, { initiallyReady: true })
    const next = new TestApplicationTransport(undefined)
    const current = new ReplaceableApplicationTransport(first)
    let editorProjection = 'unsaved local draft'
    const saveCurrent = vi.fn(async () => false)
    const commitTarget = vi.fn(() => {
      editorProjection = 'server projection'
    })
    const view = sessionViewDescriptor('codex', 'session-1')
    const transition = createWorkspaceTransition({
      saveCurrent,
      unmountCurrent: vi.fn(),
      loadTarget: vi.fn(async () => ({ body: 'server projection' })),
      commitTarget,
      restoreCurrent: vi.fn(),
      setPendingTarget: vi.fn(),
      reportFailure: vi.fn(),
    })
    const refetch = createApplicationSnapshotRefetch({
      refetch: async () => {
        await transition.activate({
          view,
          requestId: 'request-1',
          shellAction: { type: 'open' },
          pendingViewKey: workspaceViewKey(view),
        })
      },
    })
    const replacement = replaceReadyApplicationTransport(current, next, () => {
      refetch.request([{ kind: 'all' }])
    })

    await Promise.resolve()
    expect(saveCurrent).not.toHaveBeenCalled()
    next.markReady()
    await replacement
    await vi.waitFor(() => expect(saveCurrent).toHaveBeenCalledOnce())
    expect(commitTarget).not.toHaveBeenCalled()
    expect(editorProjection).toBe('unsaved local draft')
  })
})
