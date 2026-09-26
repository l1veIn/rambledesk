import { get, writable } from 'svelte/store'
import { snapshotInputTarget, type InputTarget } from '../domain/inputTarget'

export function resolveRequestInputTarget(documentTarget: InputTarget, selected?: InputTarget): InputTarget {
  // Document action selection belongs to the document; other fields retain
  // their explicit identity even when their input component is hidden.
  return snapshotInputTarget(selected?.requestId === documentTarget.requestId && selected.destination.kind !== 'document'
    ? selected : documentTarget)
}

/** Selected destinations belong to requests; the speech tracker owns segment pins. */
export function createRequestInputTargets() {
  const selections = writable<ReadonlyMap<string, InputTarget>>(new Map())

  function select(target: InputTarget) {
    if (!target.requestId || target.destination.kind === 'unknown') return
    selections.update((current) => new Map(current).set(target.requestId, snapshotInputTarget(target)))
  }

  function forRequest(documentTarget: InputTarget): InputTarget {
    const selected = get(selections).get(documentTarget.requestId)
    return resolveRequestInputTarget(documentTarget, selected)
  }

  return { subscribe: selections.subscribe, select, forRequest }
}
