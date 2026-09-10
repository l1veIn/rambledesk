export type SavedManagedSessionDraft = Readonly<{ choice: string; cwd: string; text: string }>
type DraftStorage = Pick<Storage, 'getItem' | 'setItem'>
const STORAGE_KEY = 'rambledesk.agent-drafts'
const EMPTY: SavedManagedSessionDraft = { choice: '', cwd: '', text: '' }

/** Persist only user input. Prepared session ids and credentials stay in memory. */
export function createManagedSessionDraftStorage(storage?: DraftStorage) {
  function read(): { recent?: SavedManagedSessionDraft; drafts?: Record<string, SavedManagedSessionDraft> } {
    try { return JSON.parse(storage?.getItem(STORAGE_KEY) ?? '{}') ?? {} } catch { return {} }
  }
  function valid(value: unknown): SavedManagedSessionDraft | null {
    if (!value || typeof value !== 'object') return null
    const candidate = value as Record<string, unknown>
    return typeof candidate.choice === 'string' && typeof candidate.cwd === 'string' && typeof candidate.text === 'string'
      ? { choice: candidate.choice.slice(0, 512), cwd: candidate.cwd.slice(0, 8192), text: candidate.text.slice(0, 262144) } : null
  }
  function write(value: unknown) { try { storage?.setItem(STORAGE_KEY, JSON.stringify(value)) } catch { /* Optional local drafts must not block sending. */ } }
  return {
    load(id: string): SavedManagedSessionDraft {
      const saved = read()
      return valid(saved.drafts?.[id]) ?? { ...(valid(saved.recent) ?? EMPTY), text: '' }
    },
    save(id: string, draft: SavedManagedSessionDraft) {
      const saved = read()
      const drafts = Object.fromEntries(Object.entries(saved.drafts ?? {}).filter(([key]) => key !== id).slice(-49))
      drafts[id] = { choice: draft.choice, cwd: draft.cwd, text: draft.text }
      write({ recent: { choice: draft.choice, cwd: draft.cwd, text: '' }, drafts })
    },
    remove(id: string) {
      const saved = read()
      if (saved.drafts) delete saved.drafts[id]
      write(saved)
    },
  }
}

export type ManagedSessionDraftStorage = ReturnType<typeof createManagedSessionDraftStorage>
