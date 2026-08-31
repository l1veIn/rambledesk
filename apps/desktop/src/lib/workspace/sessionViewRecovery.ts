import { workspaceViewKey, type SessionViewDescriptor } from './viewDescriptors'

export type SessionViewCatalog =
  | Readonly<{ status: 'pending' }>
  | Readonly<{ status: 'failed' }>
  | Readonly<{ status: 'ready'; views: readonly SessionViewDescriptor[] }>

export type MissingSessionViewDescriptor = Readonly<{
  kind: 'missing-session'
  session: SessionViewDescriptor
  reason: 'archived' | 'unavailable' | 'unknown' | 'unresolved'
}>

export type SessionViewResolution =
  | Readonly<{ kind: 'active'; session: SessionViewDescriptor }>
  | MissingSessionViewDescriptor

export type SessionViewRecoveryCatalogs = Readonly<{
  active: SessionViewCatalog
  archived: SessionViewCatalog
}>

function catalogContains(catalog: Extract<SessionViewCatalog, { status: 'ready' }>, viewKey: string) {
  return catalog.views.some((view) => workspaceViewKey(view) === viewKey)
}

export function resolveSessionViews(
  views: readonly SessionViewDescriptor[],
  catalogs: SessionViewRecoveryCatalogs,
): readonly SessionViewResolution[] {
  return views.map((session): SessionViewResolution => {
    const viewKey = workspaceViewKey(session)
    if (catalogs.active.status === 'pending') {
      return { kind: 'missing-session', session, reason: 'unresolved' }
    }
    if (catalogs.active.status === 'failed') {
      return { kind: 'missing-session', session, reason: 'unknown' }
    }
    if (catalogContains(catalogs.active, viewKey)) return { kind: 'active', session }
    if (catalogs.archived.status === 'pending') {
      return { kind: 'missing-session', session, reason: 'unresolved' }
    }
    if (catalogs.archived.status === 'failed') {
      return { kind: 'missing-session', session, reason: 'unknown' }
    }
    return {
      kind: 'missing-session',
      session,
      reason: catalogContains(catalogs.archived, viewKey) ? 'archived' : 'unavailable',
    }
  })
}

export function sessionViewResolution(
  resolutions: readonly SessionViewResolution[],
  viewKey: string | null,
): SessionViewResolution | null {
  if (!viewKey) return null
  return (
    resolutions.find((resolution) => workspaceViewKey(resolution.session) === viewKey) ?? null
  )
}

export type SessionViewRecoveryResolverContext = Readonly<{
  loadArchived: () => Promise<readonly SessionViewDescriptor[]>
  onInvalidate?: () => void
  onUpdate: (
    resolutions: readonly SessionViewResolution[],
  ) => Promise<boolean | void> | boolean | void
}>

export function preserveLoadedSessionDuringUnconfirmedRecovery(
  resolutions: readonly SessionViewResolution[],
  loadedSession: SessionViewDescriptor | null,
): readonly SessionViewResolution[] {
  if (!loadedSession) return resolutions
  const loadedKey = workspaceViewKey(loadedSession)
  return resolutions.map((resolution) =>
    resolution.kind === 'missing-session' &&
    workspaceViewKey(resolution.session) === loadedKey &&
    (resolution.reason === 'unknown' || resolution.reason === 'unresolved')
      ? { kind: 'active', session: resolution.session }
      : resolution,
  )
}

export function createSessionViewRecoveryResolver(
  context: SessionViewRecoveryResolverContext,
) {
  let generation = 0

  async function refresh(
    views: readonly SessionViewDescriptor[],
    active: SessionViewCatalog,
  ): Promise<'applied' | 'blocked' | 'stale'> {
    const intent = ++generation
    context.onInvalidate?.()
    const pending = resolveSessionViews(views, {
      active,
      archived: { status: 'pending' },
    })
    const pendingApplied = await context.onUpdate(pending)
    if (intent !== generation) return 'stale'
    if (pendingApplied === false) return 'blocked'
    if (
      active.status !== 'ready' ||
      pending.every((resolution) => resolution.kind === 'active')
    ) {
      return 'applied'
    }

    let archivedViews: readonly SessionViewDescriptor[]
    try {
      archivedViews = await context.loadArchived()
    } catch {
      if (intent !== generation) return 'stale'
      const applied = await context.onUpdate(
        resolveSessionViews(views, {
          active,
          archived: { status: 'failed' },
        }),
      )
      if (intent !== generation) return 'stale'
      return applied === false ? 'blocked' : 'applied'
    }
    if (intent !== generation) return 'stale'
    const applied = await context.onUpdate(
      resolveSessionViews(views, {
        active,
        archived: { status: 'ready', views: archivedViews },
      }),
    )
    if (intent !== generation) return 'stale'
    return applied === false ? 'blocked' : 'applied'
  }

  function invalidate() {
    generation += 1
  }

  return { refresh, invalidate }
}
