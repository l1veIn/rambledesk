import { describe, expect, it, vi } from 'vitest'

import { sessionViewDescriptor, workspaceViewKey } from './viewDescriptors'
import {
  resolveSessionViews,
  createSessionViewRecoveryResolver,
  preserveLoadedSessionDuringUnconfirmedRecovery,
  sessionViewResolution,
  type SessionViewCatalog,
} from './sessionViewRecovery'

const codexShared = sessionViewDescriptor('codex', 'shared')
const piShared = sessionViewDescriptor('pi', 'shared')
const missing = sessionViewDescriptor('codex', 'missing')

const ready = (views: readonly ReturnType<typeof sessionViewDescriptor>[]): SessionViewCatalog => ({
  status: 'ready',
  views,
})

describe('session view recovery', () => {
  it('classifies active, archived, and unavailable views using composite identity', () => {
    const resolutions = resolveSessionViews([codexShared, piShared, missing], {
      active: ready([codexShared]),
      archived: ready([piShared]),
    })

    expect(resolutions).toEqual([
      { kind: 'active', session: codexShared },
      { kind: 'missing-session', session: piShared, reason: 'archived' },
      { kind: 'missing-session', session: missing, reason: 'unavailable' },
    ])
  })

  it('does not call a view unavailable until both authoritative catalogs are ready', () => {
    expect(
      resolveSessionViews([missing], {
        active: ready([]),
        archived: { status: 'pending' },
      }),
    ).toEqual([{ kind: 'missing-session', session: missing, reason: 'unresolved' }])
    expect(
      resolveSessionViews([missing], {
        active: { status: 'pending' },
        archived: ready([]),
      }),
    ).toEqual([{ kind: 'missing-session', session: missing, reason: 'unresolved' }])
  })

  it('uses unknown when an authoritative lookup fails and never invents a deletion reason', () => {
    expect(
      resolveSessionViews([missing], {
        active: { status: 'failed' },
        archived: ready([]),
      }),
    ).toEqual([{ kind: 'missing-session', session: missing, reason: 'unknown' }])
    expect(
      resolveSessionViews([missing], {
        active: ready([]),
        archived: { status: 'failed' },
      }),
    ).toEqual([{ kind: 'missing-session', session: missing, reason: 'unknown' }])
  })

  it('keeps an active match authoritative while archived lookup is pending', () => {
    expect(
      resolveSessionViews([codexShared], {
        active: ready([codexShared]),
        archived: { status: 'pending' },
      }),
    ).toEqual([{ kind: 'active', session: codexShared }])
  })

  it('preserves a loaded editor until a missing-session result is confirmed', () => {
    const unresolved = [{ kind: 'missing-session', session: missing, reason: 'unresolved' }] as const
    const unknown = [{ kind: 'missing-session', session: missing, reason: 'unknown' }] as const
    const archived = [{ kind: 'missing-session', session: missing, reason: 'archived' }] as const

    expect(preserveLoadedSessionDuringUnconfirmedRecovery(unresolved, missing)).toEqual([
      { kind: 'active', session: missing },
    ])
    expect(preserveLoadedSessionDuringUnconfirmedRecovery(unknown, missing)).toEqual([
      { kind: 'active', session: missing },
    ])
    expect(preserveLoadedSessionDuringUnconfirmedRecovery(archived, missing)).toEqual(archived)
  })

  it('finds resolutions by the stable session key', () => {
    const resolutions = resolveSessionViews([codexShared, piShared], {
      active: ready([codexShared]),
      archived: ready([piShared]),
    })

    expect(sessionViewResolution(resolutions, workspaceViewKey(piShared))).toEqual(
      resolutions[1],
    )
    expect(sessionViewResolution(resolutions, 'session:missing')).toBeNull()
  })

  it('lets the latest archived lookup win without applying a stale result', async () => {
    let resolveFirst: ((views: readonly ReturnType<typeof sessionViewDescriptor>[]) => void) | undefined
    const first = new Promise<readonly ReturnType<typeof sessionViewDescriptor>[]>((resolve) => {
      resolveFirst = resolve
    })
    const loadArchived = vi
      .fn<() => Promise<readonly ReturnType<typeof sessionViewDescriptor>[]>>()
      .mockReturnValueOnce(first)
      .mockResolvedValueOnce([missing])
    const onUpdate = vi.fn()
    const resolver = createSessionViewRecoveryResolver({ loadArchived, onUpdate })

    const older = resolver.refresh([missing], ready([]))
    await Promise.resolve()
    await Promise.resolve()
    const newer = resolver.refresh([missing], ready([]))
    await expect(newer).resolves.toBe('applied')
    resolveFirst?.([])
    await expect(older).resolves.toBe('stale')

    expect(onUpdate.mock.calls.at(-1)?.[0]).toEqual([
      { kind: 'missing-session', session: missing, reason: 'archived' },
    ])
  })

  it('invalidates an asynchronous projection before a newer refresh can overtake it', async () => {
    let releaseOlder: (() => void) | undefined
    const olderProjection = new Promise<void>((resolve) => (releaseOlder = resolve))
    const onUpdate = vi
      .fn<() => Promise<void>>()
      .mockReturnValueOnce(olderProjection)
      .mockResolvedValue(undefined)
    const onInvalidate = vi.fn()
    const resolver = createSessionViewRecoveryResolver({
      loadArchived: vi.fn(async () => []),
      onInvalidate,
      onUpdate,
    })

    const older = resolver.refresh([missing], ready([]))
    await Promise.resolve()
    const newer = resolver.refresh([missing], ready([missing]))
    await expect(newer).resolves.toBe('applied')
    releaseOlder?.()
    await expect(older).resolves.toBe('stale')

    expect(onInvalidate).toHaveBeenCalledTimes(2)
  })

  it('stops recovery lookup when the caller blocks the missing-view projection', async () => {
    const loadArchived = vi.fn(async () => [missing])
    const resolver = createSessionViewRecoveryResolver({
      loadArchived,
      onUpdate: () => false,
    })

    await expect(resolver.refresh([missing], ready([]))).resolves.toBe('blocked')
    expect(loadArchived).not.toHaveBeenCalled()
  })
})
