import { get, writable } from 'svelte/store'
import { DEFAULT_APPEARANCE, normalizeAppearance, type AppearanceSettings } from './appearanceSettings'
import { readBackground, validateBackground, writeBackground, type SavedBackground } from './backgroundStorage'

export const APPEARANCE_KEY = 'rambledesk.appearance.v1'
const BACKGROUND_REVISION_KEY = 'rambledesk.appearance.background-revision'

function readPreferences(): AppearanceSettings {
  try { return normalizeAppearance(JSON.parse(localStorage.getItem(APPEARANCE_KEY) ?? 'null')) }
  catch { return { ...DEFAULT_APPEARANCE } }
}

const preferences = writable(readPreferences())
export const appearancePreferences = { subscribe: preferences.subscribe }
export function updateAppearance(patch: Partial<AppearanceSettings>): void {
  const next = normalizeAppearance({ ...get(preferences), ...patch })
  localStorage.setItem(APPEARANCE_KEY, JSON.stringify(next))
  preferences.set(next)
}
export function resetAppearance(): void { updateAppearance({ ...DEFAULT_APPEARANCE }) }

type BackgroundState = { url: string | null; name: string | null; loading: boolean; error: string | null }
const background = writable<BackgroundState>({ url: null, name: null, loading: false, error: null })
export const workspaceBackground = { subscribe: background.subscribe }
let generation = 0
let mutation: Promise<void> = Promise.resolve()
let references = 0

function showBackground(saved: SavedBackground | null) {
  const previous = get(background).url
  background.set({ url: saved ? URL.createObjectURL(saved.blob) : null, name: saved?.name ?? null, loading: false, error: null })
  if (previous) URL.revokeObjectURL(previous)
}

async function reloadBackground() {
  const current = ++generation
  background.update(value => ({ ...value, loading: true }))
  try {
    const saved = await readBackground()
    if (current === generation) showBackground(saved)
  } catch {
    if (current === generation) background.update(value => ({ ...value, loading: false, error: 'background_storage' }))
  }
}

function serializeMutation(action: () => Promise<void>): Promise<void> {
  const next = mutation.then(action)
  mutation = next.catch(() => {})
  return next
}

export function setWorkspaceBackground(file: File): Promise<void> {
  return serializeMutation(async () => {
    const blob = await validateBackground(file)
    const saved = { blob, name: file.name, revision: crypto.randomUUID() }
    await writeBackground(saved)
    ++generation
    if (references > 0) showBackground(saved)
    // The committed image is still usable if preference storage is full; expose
    // that failure to the settings UI instead of pretending selection was saved.
    localStorage.setItem(BACKGROUND_REVISION_KEY, saved.revision)
    updateAppearance({ background: 'image' })
  })
}

export function removeWorkspaceBackground(): Promise<void> {
  return serializeMutation(async () => {
    await writeBackground(null)
    ++generation
    showBackground(null)
    localStorage.setItem(BACKGROUND_REVISION_KEY, crypto.randomUUID())
    if (get(preferences).background === 'image') updateAppearance({ background: 'pattern' })
  })
}

function onStorage(event: StorageEvent) {
  if (event.storageArea && event.storageArea !== localStorage) return
  if (event.key === APPEARANCE_KEY || event.key === null) preferences.set(readPreferences())
  if (event.key === BACKGROUND_REVISION_KEY || event.key === null) void reloadBackground()
}

/** A lifecycle shared by the workbench and its isolated settings preview. */
export function initializeAppearancePreferences(): () => void {
  if (references++ === 0) {
    preferences.set(readPreferences())
    window.addEventListener('storage', onStorage)
    void reloadBackground()
  }
  let released = false
  return () => {
    if (released) return
    released = true
    if (--references !== 0) return
    window.removeEventListener('storage', onStorage)
    ++generation
    showBackground(null)
  }
}
