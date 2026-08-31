import { describe, expect, it, vi } from 'vitest'

import {
  sessionViewDescriptor,
  settingsViewDescriptor,
  workspaceViewKey,
  type WorkspaceViewDescriptor,
} from './viewDescriptors'
import {
  createWorkspaceTransition,
  type WorkspaceTransitionAdapter,
  type WorkspaceTransitionTarget,
} from './workspaceTransition'

type Loaded = Readonly<{ requestId: string }>

const alpha = sessionViewDescriptor('codex', 'alpha')
const beta = sessionViewDescriptor('pi', 'beta')

function target(
  view: WorkspaceViewDescriptor = beta,
  requestId: string | null = 'request-beta',
): WorkspaceTransitionTarget {
  return {
    view,
    requestId,
    shellAction: { type: 'open' },
    pendingViewKey: workspaceViewKey(view),
  }
}

function harness(overrides: Partial<WorkspaceTransitionAdapter<Loaded>> = {}) {
  const events: string[] = []
  let mountedEditors = 1
  let maximumMountedEditors = 1
  const adapter: WorkspaceTransitionAdapter<Loaded> = {
    saveCurrent: vi.fn(async () => {
      events.push('save')
      return true
    }),
    unmountCurrent: vi.fn(() => {
      events.push('unmount')
      mountedEditors = 0
    }),
    loadTarget: vi.fn(async (next) => {
      events.push(`load:${next.requestId}`)
      return { requestId: next.requestId ?? '' }
    }),
    commitTarget: vi.fn((next) => {
      events.push(`commit:${next.requestId}`)
      mountedEditors += 1
      maximumMountedEditors = Math.max(maximumMountedEditors, mountedEditors)
    }),
    restoreCurrent: vi.fn(() => {
      events.push('restore')
      mountedEditors = 1
      maximumMountedEditors = Math.max(maximumMountedEditors, mountedEditors)
    }),
    setPendingTarget: vi.fn(),
    reportFailure: vi.fn(),
    ...overrides,
  }
  return {
    adapter,
    events,
    maximumMountedEditors: () => maximumMountedEditors,
    transition: createWorkspaceTransition(adapter),
  }
}

describe('workspaceTransition', () => {
  it('saves, unmounts, loads, and commits in order with at most one editor mounted', async () => {
    const run = harness()

    await expect(run.transition.activate(target())).resolves.toBe('activated')

    expect(run.events).toEqual(['save', 'unmount', 'load:request-beta', 'commit:request-beta'])
    expect(run.maximumMountedEditors()).toBe(1)
  })

  it('blocks before unmounting when the current draft cannot be saved', async () => {
    const run = harness({
      saveCurrent: vi.fn(async () => {
        run.events.push('save')
        return false
      }),
    })

    await expect(run.transition.activate(target())).resolves.toBe('blocked')

    expect(run.events).toEqual(['save', 'restore'])
    expect(run.adapter.loadTarget).not.toHaveBeenCalled()
    expect(run.adapter.commitTarget).not.toHaveBeenCalled()
  })

  it('restores the prior view when loading fails', async () => {
    const failure = new Error('load failed')
    const run = harness({
      loadTarget: vi.fn(async () => {
        run.events.push('load')
        throw failure
      }),
    })

    await expect(run.transition.activate(target())).resolves.toBe('failed')

    expect(run.events).toEqual(['save', 'unmount', 'load', 'restore'])
    expect(run.adapter.commitTarget).not.toHaveBeenCalled()
    expect(run.adapter.reportFailure).toHaveBeenCalledWith(failure)
  })

  it('commits only the latest target when an older load finishes late', async () => {
    let resolveAlpha: ((loaded: Loaded) => void) | undefined
    const alphaLoaded = new Promise<Loaded>((resolve) => (resolveAlpha = resolve))
    const run = harness({
      loadTarget: vi.fn(async (next) => {
        run.events.push(`load:${next.requestId}`)
        return next.requestId === 'request-alpha'
          ? alphaLoaded
          : { requestId: next.requestId ?? '' }
      }),
    })

    const firstTarget = target(alpha, 'request-alpha')
    const first = run.transition.activate(firstTarget)
    await Promise.resolve()
    await Promise.resolve()
    const latest = run.transition.activate(target(beta, 'request-beta'))
    resolveAlpha?.({ requestId: 'request-alpha' })

    await expect(first).resolves.toBe('stale')
    await expect(latest).resolves.toBe('activated')
    expect(run.adapter.commitTarget).toHaveBeenCalledTimes(1)
    expect(run.adapter.commitTarget).toHaveBeenCalledWith(
      expect.objectContaining({ requestId: 'request-beta' }),
      { requestId: 'request-beta' },
    )
    expect(run.maximumMountedEditors()).toBe(1)
  })

  it('treats an older failed save as stale when a newer intent wins', async () => {
    let resolveOlderSave: ((saved: boolean) => void) | undefined
    const olderSave = new Promise<boolean>((resolve) => (resolveOlderSave = resolve))
    let saveCalls = 0
    const run = harness({
      saveCurrent: vi.fn(async () => {
        run.events.push('save')
        saveCalls += 1
        return saveCalls === 1 ? olderSave : true
      }),
    })

    const older = run.transition.activate(target(alpha, 'request-alpha'))
    await Promise.resolve()
    await Promise.resolve()
    const newest = run.transition.activate(target(beta, 'request-beta'))
    resolveOlderSave?.(false)

    await expect(older).resolves.toBe('stale')
    await expect(newest).resolves.toBe('activated')
    expect(run.adapter.restoreCurrent).not.toHaveBeenCalled()
    expect(run.adapter.commitTarget).toHaveBeenCalledTimes(1)
    expect(run.adapter.commitTarget).toHaveBeenCalledWith(
      expect.objectContaining({ requestId: 'request-beta' }),
      { requestId: 'request-beta' },
    )
  })

  it('invalidates an in-flight load and restores the current editor', async () => {
    let resolveLoad: ((loaded: Loaded) => void) | undefined
    const loading = new Promise<Loaded>((resolve) => (resolveLoad = resolve))
    const run = harness({ loadTarget: vi.fn(async () => loading) })
    const activation = run.transition.activate(target())
    await Promise.resolve()
    await Promise.resolve()

    run.transition.invalidate()
    resolveLoad?.({ requestId: 'request-beta' })

    await expect(activation).resolves.toBe('stale')
    expect(run.adapter.commitTarget).not.toHaveBeenCalled()
    expect(run.adapter.restoreCurrent).toHaveBeenCalledTimes(1)
  })

  it('closes the last view after saving and unmounting without loading another request', async () => {
    const run = harness()
    const closeLast: WorkspaceTransitionTarget = {
      view: null,
      requestId: null,
      shellAction: { type: 'close', viewKey: workspaceViewKey(alpha) },
      pendingViewKey: workspaceViewKey(alpha),
    }

    await expect(run.transition.activate(closeLast)).resolves.toBe('activated')

    expect(run.events).toEqual(['save', 'unmount', 'commit:null'])
    expect(run.adapter.loadTarget).not.toHaveBeenCalled()
  })

  it('opens settings through the save gate without loading a session workspace', async () => {
    const run = harness()
    const settings = target(settingsViewDescriptor(), null)

    await expect(run.transition.activate(settings)).resolves.toBe('activated')

    expect(run.events).toEqual(['save', 'unmount', 'commit:null'])
    expect(run.adapter.loadTarget).not.toHaveBeenCalled()
    expect(run.adapter.commitTarget).toHaveBeenCalledWith(settings, null)
  })
})
