import {
  createFeedbackDraftSession,
  type DraftSavePort,
  type FeedbackDraftSession,
  type SpeechCleanupPort,
} from './feedbackDraftSession'

export type DraftSessionHost = {
  currentGeneration(): number
  bumpGeneration(): number
  visible(): FeedbackDraftSession | null
  owner(): FeedbackDraftSession | null
  get(requestId: string): FeedbackDraftSession | undefined
  mounted(): FeedbackDraftSession[]
  openVisible(input: {
    requestId: string
    markdown: string
    revision: number
  }): FeedbackDraftSession
  setOwner(requestId: string | null): void
  disposeAll(): void
}

const MAX_MOUNTED = 2

export function createDraftSessionHost(input: {
  save: DraftSavePort
  cleanup?: SpeechCleanupPort
  onChange?: () => void
}): DraftSessionHost {
  let generation = 1
  let visibleId = ''
  let ownerId = ''
  const sessions = new Map<string, FeedbackDraftSession>()

  function mounted(): FeedbackDraftSession[] {
    const ids = [...new Set([ownerId, visibleId].filter(Boolean))]
    return ids
      .map((id) => sessions.get(id))
      .filter((session): session is FeedbackDraftSession => Boolean(session && !session.isDisposed()))
  }

  function disposeSession(requestId: string) {
    const session = sessions.get(requestId)
    if (!session) return
    session.dispose()
    sessions.delete(requestId)
  }

  function evictIfNeeded(keepId: string) {
    const keep = new Set([keepId, ownerId].filter(Boolean))
    for (const requestId of [...sessions.keys()]) {
      if (sessions.size <= MAX_MOUNTED) return
      if (!keep.has(requestId)) disposeSession(requestId)
    }
  }

  function openVisible(hydrate: {
    requestId: string
    markdown: string
    revision: number
  }): FeedbackDraftSession {
    const existing = sessions.get(hydrate.requestId)
    visibleId = hydrate.requestId
    if (existing && !existing.isDisposed()) {
      evictIfNeeded(hydrate.requestId)
      input.onChange?.()
      return existing
    }
    const session = createFeedbackDraftSession({
      requestId: hydrate.requestId,
      generation,
      initialMarkdown: hydrate.markdown,
      initialRevision: hydrate.revision,
      save: input.save,
      cleanup: input.cleanup,
      onChange: input.onChange,
    })
    sessions.set(hydrate.requestId, session)
    evictIfNeeded(hydrate.requestId)
    input.onChange?.()
    return session
  }

  function setOwner(requestId: string | null) {
    ownerId = requestId ?? ''
    if (ownerId && !sessions.has(ownerId) && visibleId === ownerId) {
      return
    }
    evictIfNeeded(visibleId)
    input.onChange?.()
  }

  function disposeAll() {
    for (const requestId of [...sessions.keys()]) disposeSession(requestId)
    visibleId = ''
    ownerId = ''
    input.onChange?.()
  }

  function bumpGeneration() {
    disposeAll()
    generation += 1
    return generation
  }

  return {
    currentGeneration: () => generation,
    bumpGeneration,
    visible: () => (visibleId ? sessions.get(visibleId) ?? null : null),
    owner: () => (ownerId ? sessions.get(ownerId) ?? null : null),
    get: (requestId) => sessions.get(requestId),
    mounted,
    openVisible,
    setOwner,
    disposeAll,
  }
}
