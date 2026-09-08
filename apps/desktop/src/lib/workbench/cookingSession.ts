import { get, writable } from 'svelte/store'

export type CookingPreview = Readonly<{ markdown: string; original: string; model: string }>

/**
 * Cooking progress and the generated preview. Both are keyed to the workspace the
 * request belongs to, so the request list and the workbench read the same facts.
 */
export type CookingSessionState = Readonly<{
  cookingRequestIds: ReadonlySet<string>
  preview: CookingPreview | null
}>

const initial: CookingSessionState = {
  cookingRequestIds: new Set(),
  preview: null,
}

export type CookingSession = ReturnType<typeof createCookingSession>

export function createCookingSession() {
  const store = writable<CookingSessionState>(initial)

  function setCooking(requestId: string, cooking: boolean) {
    store.update((current) => {
      const next = new Set(current.cookingRequestIds)
      if (cooking) next.add(requestId)
      else next.delete(requestId)
      return { ...current, cookingRequestIds: next }
    })
  }

  function setPreview(preview: CookingPreview | null) {
    store.update((current) => (current.preview === preview ? current : { ...current, preview }))
  }

  return {
    subscribe: store.subscribe,
    setCooking,
    setPreview,
    isCooking: (requestId: string | null | undefined) =>
      Boolean(requestId) && get(store).cookingRequestIds.has(requestId as string),
    preview: () => get(store).preview,
  }
}
