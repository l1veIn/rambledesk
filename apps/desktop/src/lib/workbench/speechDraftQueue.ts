import { get, writable } from 'svelte/store'

import type { ActiveAction, DraftOperation } from '../draftOperations'

export type SpeechTarget = { requestId: string; requestTitle: string; action: ActiveAction }
export type SpeechDraft = SpeechTarget & {
  id: string
  text: string
  status: 'pending' | 'writing' | 'failed' | 'editing' | 'tidying'
  error: string
  cleanupState?: 'pending' | 'cleaned'
  writeAttempted?: boolean
  mergedIds?: string[]
}
export type SpeechReceipt = SpeechTarget & { id: string; text: string }
export type SpeechDraftEdit = { ids: string[]; text: string }
export type SpeechDraftState = { drafts: SpeechDraft[]; receipt: SpeechReceipt | null; edit: SpeechDraftEdit | null }
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
    ).map((item) => ({
      ...item, status: 'pending', error: '',
      cleanupState: item.cleanupState === 'cleaned' ? 'cleaned' : 'pending',
      // A failed/lost acknowledgement can still mean the server committed the
      // segment. Its idempotent retry must retain the original payload.
      writeAttempted: item.writeAttempted === true || item.status === 'writing' || item.status === 'failed',
      mergedIds: Array.isArray(item.mergedIds) ? item.mergedIds.filter((id: unknown) => typeof id === 'string') : [],
    }))
  } catch {
    return []
  }
}

function sameTarget(first: SpeechTarget, second: SpeechTarget): boolean {
  return first.requestId === second.requestId && first.action?.actionId === second.action?.actionId
}

function editable(draft: SpeechDraft): boolean {
  return draft.status === 'pending' && !draft.writeAttempted
}

function sameIds(first: readonly string[], second: readonly string[]): boolean {
  return first.length === second.length && first.every((id, index) => id === second[index])
}

/** A stale selection may cover a prefix/subset of a group, but cannot skip,
 * reorder, or cross the destination of any captured segment. */
function editableSelection(drafts: readonly SpeechDraft[], ids: readonly string[]): SpeechDraft[] {
  if (!ids.length || new Set(ids).size !== ids.length) return []
  const start = drafts.findIndex((draft) => draft.id === ids[0])
  if (start < 0) return []
  const selected = drafts.slice(start, start + ids.length)
  return selected.length === ids.length && selected.every((draft, index) =>
    draft.id === ids[index] && editable(draft) && sameTarget(draft, selected[0]),
  ) ? selected : []
}

function mergeSelection(drafts: readonly SpeechDraft[], selected: readonly SpeechDraft[], text: string, cleanupState: 'pending' | 'cleaned'): SpeechDraft[] {
  const first = selected[0]
  const ids = new Set(selected.map((draft) => draft.id))
  const merged: SpeechDraft = {
    ...first, text, status: 'pending', error: '', cleanupState,
    mergedIds: [...new Set(selected.flatMap((draft) => [draft.id, ...(draft.mergedIds ?? [])]))],
  }
  return drafts.flatMap((draft) => draft.id === first.id ? [merged] : ids.has(draft.id) ? [] : [draft])
}

/** Only this queue owns uncommitted speech. Views send explicit segment IDs,
 * so delayed clicks cannot accept or discard words that arrived afterward. */
export function createSpeechDraftQueue(options: {
  write: (requestId: string, operation: DraftOperation) => Promise<void>
  storage?: Storage
  onStorageError?: (cause: unknown) => void
  tidy?: (text: string) => Promise<string>
}) {
  const state = writable<SpeechDraftState>({ drafts: restore(options.storage), receipt: null, edit: null })
  const seen = new Set(get(state).drafts.flatMap((draft) => [draft.id, ...(draft.mergedIds ?? [])]))
  let writes: Promise<void> = Promise.resolve()
  let editOriginals: SpeechDraft[] | null = null
  let disposed = false
  const tidyOriginals = new Map<SpeechDraft, SpeechDraft>()

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
    const drafts = get(state).drafts.filter((draft) => selected.has(draft.id) && (draft.status === 'pending' || draft.status === 'failed'))
    const claimed = new Set(drafts.map((draft) => draft.id))
    update((current) => ({ ...current, drafts: current.drafts.map((draft) =>
      claimed.has(draft.id) ? { ...draft, status: 'writing', error: '', writeAttempted: true } : draft,
    ) }))
    const run = writes.then(async () => {
      for (const draft of drafts) {
        try {
          await options.write(draft.requestId, {
            kind: 'appendSpeech', segmentId: draft.id, text: draft.text, action: draft.action,
            ...(draft.cleanupState === 'cleaned' ? { cleanupState: 'cleaned' } : {}),
          })
          update((current) => ({
            ...current,
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

  function beginEdit(ids: readonly string[]) {
    if (disposed || get(state).edit) return
    const selected = editableSelection(get(state).drafts, ids)
    if (!selected.length) return
    editOriginals = selected
    const frozen = new Set(selected)
    update((current) => ({ ...current,
      edit: { ids: [...ids], text: selected.map((draft) => draft.text).join('\n') },
      drafts: current.drafts.map((draft) => frozen.has(draft) ? { ...draft, status: 'editing' } : draft),
    }))
  }

  function saveEdit(ids: readonly string[], text: string) {
    const current = get(state)
    if (!current.edit || !editOriginals || !sameIds(current.edit.ids, ids) || typeof text !== 'string' || !text.trim()) return
    const selected = editOriginals
    editOriginals = null
    update((value) => ({ ...value, edit: null, drafts: mergeSelection(value.drafts, selected, text.trim(), 'pending') }))
  }

  function cancelEdit(ids: readonly string[]) {
    const current = get(state)
    if (!current.edit || !editOriginals || !sameIds(current.edit.ids, ids)) return
    const originals = new Map(editOriginals.map((draft) => [draft.id, draft]))
    editOriginals = null
    update((value) => ({ ...value, edit: null, drafts: value.drafts.map((draft) => originals.get(draft.id) ?? draft) }))
  }

  async function tidy(ids: readonly string[]): Promise<void> {
    if (disposed || !options.tidy) return
    const selected = editableSelection(get(state).drafts, ids)
    if (!selected.length) return
    const locked = new Map(selected.map((draft) => {
      const next: SpeechDraft = { ...draft, status: 'tidying', error: '' }
      tidyOriginals.set(next, draft)
      return [draft.id, next]
    }))
    update((current) => ({ ...current, drafts: current.drafts.map((draft) => locked.get(draft.id) ?? draft) }))
    let text = ''
    let error = ''
    try {
      const result = await options.tidy(selected.map((draft) => draft.text).join('\n'))
      if (typeof result !== 'string' || !result.trim()) throw new Error('Speech cleanup returned no usable text.')
      text = result.trim()
    } catch (cause) {
      error = cause instanceof Error && cause.message ? cause.message : 'Could not tidy speech. Try again.'
    }
    if (disposed) return
    update((current) => {
      const intact = [...locked.values()].every((draft) => current.drafts.includes(draft))
      // Discarding any part invalidates the result for the entire snapshot.
      // Restore only surviving originals; never recreate a removed segment.
      const drafts = intact && !error ? mergeSelection(current.drafts, selected, text, 'cleaned')
        : current.drafts.map((draft) => {
          const original = tidyOriginals.get(draft)
          return locked.get(draft.id) === draft && original ? { ...original, ...(intact ? { error } : {}) } : draft
        })
      return { ...current, drafts }
    })
    for (const draft of locked.values()) tidyOriginals.delete(draft)
  }

  function enqueue(id: string, text: string, target: SpeechTarget, needsConfirmation: boolean, autoTidy = false): Promise<void> | undefined {
    if (!text.trim() || seen.has(id)) return
    seen.add(id)
    const draft: SpeechDraft = {
      ...target, action: target.action ? { ...target.action } : null,
      id, text: text.trim(), status: 'pending', error: '', cleanupState: 'pending',
    }
    update((current) => ({ ...current, drafts: [...current.drafts, draft] }))
    if (!needsConfirmation) return accept([id])
    if (autoTidy) return tidy([id])
  }

  return {
    subscribe: state.subscribe,
    enqueue,
    accept,
    beginEdit,
    saveEdit,
    cancelEdit,
    tidy,
    discard(ids: readonly string[]) {
      const selected = new Set(ids)
      update((current) => ({ ...current, drafts: current.drafts.filter((draft) =>
        !selected.has(draft.id) || draft.status === 'writing' || draft.status === 'editing',
      ) }))
    },
    clearReceipt(id: string) {
      state.update((current) => current.receipt?.id === id ? { ...current, receipt: null } : current)
    },
    hasPending: (requestId: string) => get(state).drafts.some((draft) => draft.requestId === requestId),
    settled: () => writes,
    dispose() {
      if (disposed) return
      disposed = true
      const edit = get(state).edit
      if (edit) cancelEdit(edit.ids)
      if (tidyOriginals.size) update((current) => ({ ...current,
        drafts: current.drafts.map((draft) => tidyOriginals.get(draft) ?? draft),
      }))
      tidyOriginals.clear()
    },
  }
}

export type SpeechDraftGroup = SpeechTarget & {
  ids: string[]; text: string; busy: boolean; error: string
  editing?: boolean; tidying?: boolean; editable?: boolean; cleanupState?: 'pending' | 'cleaned'
}

export function groupSpeechDrafts(drafts: readonly SpeechDraft[]): SpeechDraftGroup[] {
  const groups: SpeechDraftGroup[] = []
  for (const [index, draft] of drafts.entries()) {
    let group = groups.at(-1)
    const previous = drafts[index - 1]
    if (!group || !sameTarget(group, draft) || previous?.status !== draft.status || !!previous?.writeAttempted !== !!draft.writeAttempted) {
      group = { ...draft, ids: [], text: '', busy: false, error: '', editing: draft.status === 'editing', tidying: draft.status === 'tidying', editable: editable(draft), cleanupState: draft.cleanupState ?? 'pending' }
      groups.push(group)
    }
    group.ids.push(draft.id)
    group.text += (group.text ? '\n' : '') + draft.text
    group.busy ||= draft.status === 'writing' || draft.status === 'editing' || draft.status === 'tidying'
    group.editable &&= editable(draft)
    if (draft.cleanupState !== 'cleaned') group.cleanupState = 'pending'
    group.error ||= draft.error
  }
  return groups
}
