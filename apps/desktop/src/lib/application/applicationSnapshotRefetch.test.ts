import { describe, expect, it, vi } from 'vitest'

import { sessionViewDescriptor, workspaceViewKey } from '../workspace/viewDescriptors'
import { createWorkspaceTransition } from '../workspace/workspaceTransition'
import {
  applicationResourcesAffectNavigation,
  applicationResourcesAffectAgentConfigurations,
  applicationResourcesAffectManagedSession,
  applicationResourcesAffectWorkspace,
  applicationResourcesRequireFullNavigationSnapshot,
  createApplicationSnapshotRefetch,
} from './applicationSnapshotRefetch'

async function flush(): Promise<void> {
  await Promise.resolve()
  await Promise.resolve()
}

describe('ApplicationSnapshotRefetch', () => {
  it('refetches only the affected managed session or agent configuration list', () => {
    const resources = [{ kind: 'managed_session', session_id: 'session-one' }] as const
    expect(applicationResourcesAffectManagedSession(resources, 'session-one')).toBe(true)
    expect(applicationResourcesAffectManagedSession(resources, 'session-two')).toBe(false)
    expect(applicationResourcesAffectAgentConfigurations(resources)).toBe(false)
    // Streaming agent output must not refresh the unrelated feedback editor or sidebar.
    expect(applicationResourcesAffectNavigation(resources)).toBe(false)
    expect(applicationResourcesAffectWorkspace(resources, {
      hostId: 'dsh', hostSessionId: 'external-one', requestId: 'feedback-one',
    })).toBe(false)
    expect(applicationResourcesAffectAgentConfigurations([{ kind: 'agent_configurations' }])).toBe(true)
    expect(applicationResourcesAffectManagedSession([{ kind: 'agent_configurations' }], 'session-one')).toBe(false)
    expect(applicationResourcesAffectAgentConfigurations([{ kind: 'all' }])).toBe(true)
    expect(applicationResourcesAffectManagedSession([{ kind: 'all' }], 'session-one')).toBe(true)
  })

  it('coalesces duplicate invalidations and runs one trailing fetch', async () => {
    let release: (() => void) | undefined
    const first = new Promise<void>((resolve) => { release = resolve })
    const batches: unknown[][] = []
    const refetch = createApplicationSnapshotRefetch({
      refetch: async ({ resources }) => {
        batches.push([...resources])
        if (batches.length === 1) await first
      },
    })
    refetch.request([{ kind: 'navigation' }, { kind: 'navigation' }])
    await flush()
    refetch.request([
      { kind: 'feedback_workspace', request_id: 'request-1' },
      { kind: 'feedback_workspace', request_id: 'request-1' },
    ])
    release?.()
    await vi.waitFor(() => expect(batches).toHaveLength(2))
    expect(batches[0]).toEqual([{ kind: 'navigation' }])
    expect(batches[1]).toEqual([
      { kind: 'feedback_workspace', request_id: 'request-1' },
    ])
  })

  it('makes an in-flight intent stale after generation invalidation', async () => {
    let release: (() => void) | undefined
    const waiting = new Promise<void>((resolve) => { release = resolve })
    const applied: boolean[] = []
    const refetch = createApplicationSnapshotRefetch({
      refetch: async (intent) => {
        await waiting
        applied.push(intent.isCurrent())
      },
    })
    refetch.request([{ kind: 'all' }])
    await flush()
    refetch.invalidate()
    release?.()
    await vi.waitFor(() => expect(applied).toEqual([false]))
  })

  it('preserves a dirty editor when the workspace save gate blocks refetch', async () => {
    const view = sessionViewDescriptor('codex', 'session-1')
    let editorProjection = 'unsaved local draft'
    const saveCurrent = vi.fn(async () => false)
    const commitTarget = vi.fn(() => {
      editorProjection = 'server snapshot'
    })
    const transition = createWorkspaceTransition({
      saveCurrent,
      unmountCurrent: vi.fn(),
      loadTarget: vi.fn(async () => ({ body: 'server snapshot' })),
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

    refetch.request([{ kind: 'feedback_workspace', request_id: 'request-1' }])
    await vi.waitFor(() => expect(saveCurrent).toHaveBeenCalledOnce())
    expect(commitTarget).not.toHaveBeenCalled()
    expect(editorProjection).toBe('unsaved local draft')
  })

  it('matches navigation and scoped workspace resources', () => {
    const workspace = {
      requestId: 'request-1',
      hostId: 'codex',
      hostSessionId: 'session-1',
    }
    expect(applicationResourcesAffectNavigation([{ kind: 'navigation' }])).toBe(true)
    expect(applicationResourcesRequireFullNavigationSnapshot([{ kind: 'all' }])).toBe(true)
    expect(
      applicationResourcesRequireFullNavigationSnapshot([{ kind: 'navigation' }]),
    ).toBe(false)
    expect(
      applicationResourcesAffectWorkspace(
        [{ kind: 'host_session_resources', host_id: 'codex', host_session_id: 'session-1' }],
        workspace,
      ),
    ).toBe(true)
    expect(
      applicationResourcesAffectWorkspace(
        [{ kind: 'published_feedback', request_id: 'request-2' }],
        workspace,
      ),
    ).toBe(false)
  })
})
