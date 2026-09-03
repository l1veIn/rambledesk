import { get, writable } from 'svelte/store'

import type { ActiveAction, DraftOperation } from '../draftOperations'

export type SpeechTarget = { requestId: string; requestTitle: string; action: ActiveAction }
export type SpeechDraft = SpeechTarget & {
  id: string
  text: string
  status: 'pending' | 'writing' | 'failed'
  error: string
}
export type SpeechReceipt = SpeechTarget & { id: string; text: string }
export type SpeechDraftState = { drafts: SpeechDraft[]; receipt: SpeechReceipt | null }
export const PENDING_SPEECH_KEY = 'rambledesk.speech.pending-drafts'

type Storage = Pick<globalThis.Storage, 'getItem' | 'setItem'>

function restore(storage?: Storage): SpeechDraft[] {
  try {
    const value: unknown = JSON.parse(storage?.getItem(PENDING_SPEECH_KEY) ?? '[]')
    if (!Array.isArray(value)) return []
    return value.filter((item) =>
      item && typeof item.id === 'string' && typeof item.requestId === 'string' &&
      typeof item.requestTitle === 'string' && typeof item.text === 'string' && item.text.trim() &&
      (item.action === null || (typeof item.action?.actionId === 'string' &&
        typeof item.action?.actionIndex === 'number' && typeof item.action?.title === 'string')),
    ).map((item) => ({ ...item, status: 'pending', error: '' }))
  } catch {
    return []
  }
}

/** Only this queue owns uncommitted speech. Views send explicit segment IDs,
 * so delayed clicks cannot accept or discard words that arrived afterward. */
export function createSpeechDraftQueue(options: {
  write: (requestId: string, operation: DraftOperation) => Promise<void>
  storage?: Storage
  onStorageError?: (cause: unknown) => void
}) {
  const state = writable<SpeechDraftState>({ drafts: restore(options.storage), receipt: null })
  const seen = new Set(get(state).drafts.map((draft) => draft.id))
  let writes: Promise<void> = Promise.resolve()

  function update(change: (current: SpeechDraftState) => SpeechDraftState) {
    state.update((current) => {
      const next = change(current)
      try { options.storage?.setItem(PENDING_SPEECH_KEY, JSON.stringify(next.drafts)) }
      catch (cause) { options.onStorageError?.(cause) }
      return next
    })
  }

  function accept(ids: readonly string[]): Promise<void> {
    const selected = new Set(ids)
    const drafts = get(state).drafts.filter((draft) => selected.has(draft.id) && draft.status !== 'writing')
    const claimed = new Set(drafts.map((draft) => draft.id))
    update((current) => ({ ...current, drafts: current.drafts.map((draft) =>
      claimed.has(draft.id) ? { ...draft, status: 'writing', error: '' } : draft,
    ) }))
    const run = writes.then(async () => {
      for (const draft of drafts) {
        try {
          await options.write(draft.requestId, {
            kind: 'appendSpeech', segmentId: draft.id, text: draft.text, action: draft.action,
          })
          update((current) => ({
            drafts: current.drafts.filter((item) => item.id !== draft.id),
            receipt: { ...draft },
          }))
        } catch (cause) {
          const error = cause instanceof Error ? cause.message : String(cause)
          update((current) => ({ ...current, drafts: current.drafts.map((item) =>
            item.id === draft.id ? { ...item, status: 'failed', error } : item,
          ) }))
        }
      }
    })
    writes = run
    return run
  }

  function enqueue(id: string, text: string, target: SpeechTarget, needsConfirmation: boolean) {
    if (!text.trim() || seen.has(id)) return
    seen.add(id)
    const draft: SpeechDraft = {
      ...target, action: target.action ? { ...target.action } : null,
      id, text: text.trim(), status: 'pending', error: '',
    }
    update((current) => ({ ...current, drafts: [...current.drafts, draft] }))
    if (!needsConfirmation) void accept([id])
  }

  return {
    subscribe: state.subscribe,
    enqueue,
    accept,
    discard(ids: readonly string[]) {
      const selected = new Set(ids)
      update((current) => ({ ...current, drafts: current.drafts.filter((draft) =>
        !selected.has(draft.id) || draft.status === 'writing',
      ) }))
    },
    clearReceipt(id: string) {
      state.update((current) => current.receipt?.id === id ? { ...current, receipt: null } : current)
    },
    hasPending: (requestId: string) => get(state).drafts.some((draft) => draft.requestId === requestId),
    settled: () => writes,
  }
}

export type SpeechDraftGroup = SpeechTarget & { ids: string[]; text: string; busy: boolean; error: string }

export function groupSpeechDrafts(drafts: readonly SpeechDraft[]): SpeechDraftGroup[] {
  const groups: SpeechDraftGroup[] = []
  for (const draft of drafts) {
    let group = groups.at(-1)
    if (!group || group.requestId !== draft.requestId || group.action?.actionId !== draft.action?.actionId) {
      group = { ...draft, ids: [], text: '', busy: false, error: '' }
      groups.push(group)
    }
    group.ids.push(draft.id)
    group.text += (group.text ? '\n' : '') + draft.text
    group.busy ||= draft.status === 'writing'
    group.error ||= draft.error
  }
  return groups
}
