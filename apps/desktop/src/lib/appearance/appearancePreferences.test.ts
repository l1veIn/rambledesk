import { get } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { SavedBackground } from './backgroundStorage'
import { DEFAULT_APPEARANCE } from './appearanceSettings'

const repository = vi.hoisted(() => ({
  readBackground: vi.fn<() => Promise<SavedBackground | null>>(),
  writeBackground: vi.fn<(value: SavedBackground | null) => Promise<void>>(),
  validateBackground: vi.fn<(file: File) => Promise<Blob>>(),
}))
vi.mock('./backgroundStorage', () => repository)

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (cause: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}
const settle = async () => { for (let index = 0; index < 8; index++) await Promise.resolve() }
const saved = (name: string): SavedBackground => ({ blob: new Blob([name], { type: 'image/png' }), name, revision: name })
const file = (name = 'new.png') => new File(['image'], name, { type: 'image/png' })
const revisionKey = 'rambledesk.appearance.background-revision'
let data: Map<string, string>
let target: EventTarget
let storage: { getItem: ReturnType<typeof vi.fn>; setItem: ReturnType<typeof vi.fn> }
let release: (() => void) | undefined
let urlIndex = 0

function storageEvent(key: string | null, area: unknown = storage) {
  const event = new Event('storage')
  Object.defineProperties(event, { key: { value: key }, storageArea: { value: area } })
  target.dispatchEvent(event)
}

beforeEach(() => {
  vi.resetModules()
  vi.resetAllMocks()
  data = new Map()
  target = new EventTarget()
  storage = {
    getItem: vi.fn((key: string) => data.get(key) ?? null),
    setItem: vi.fn((key: string, value: string) => { data.set(key, value) }),
  }
  vi.stubGlobal('localStorage', storage)
  vi.stubGlobal('window', target)
  urlIndex = 0
  vi.spyOn(URL, 'createObjectURL').mockImplementation(() => `blob:background-${++urlIndex}`)
  vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {})
  repository.readBackground.mockResolvedValue(null)
  repository.writeBackground.mockResolvedValue(undefined)
  repository.validateBackground.mockImplementation(async value => value)
})

afterEach(() => {
  release?.()
  release = undefined
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('appearance persistence and cross-window updates', () => {
  it('normalizes saved data and persists patches without replacing independent preferences', async () => {
    data.set('rambledesk.appearance.v1', JSON.stringify({ palette: 'rose', zoom: 123, uiFont: 'geist' }))
    const api = await import('./appearancePreferences')
    expect(get(api.appearancePreferences)).toMatchObject({ palette: 'rose', zoom: 125, uiFont: 'geist' })
    api.updateAppearance({ backgroundBlur: 8 })
    expect(JSON.parse(data.get(api.APPEARANCE_KEY)!)).toMatchObject({ palette: 'rose', zoom: 125, backgroundBlur: 8, uiFont: 'geist' })
    api.resetAppearance()
    expect(get(api.appearancePreferences)).toEqual(DEFAULT_APPEARANCE)
    expect(repository.writeBackground).not.toHaveBeenCalled()
  })

  it('loads defaults when preference storage is corrupted or unreadable', async () => {
    data.set('rambledesk.appearance.v1', '{bad')
    let api = await import('./appearancePreferences')
    expect(get(api.appearancePreferences)).toEqual(DEFAULT_APPEARANCE)
    vi.resetModules()
    storage.getItem.mockImplementation(() => { throw new Error('storage blocked') })
    api = await import('./appearancePreferences')
    expect(get(api.appearancePreferences)).toEqual(DEFAULT_APPEARANCE)
  })

  it('retains the acknowledged live setting when persistence fails', async () => {
    const api = await import('./appearancePreferences')
    api.updateAppearance({ palette: 'green' })
    storage.setItem.mockImplementation(() => { throw new Error('quota') })
    expect(() => api.updateAppearance({ palette: 'orange' })).toThrow('quota')
    expect(get(api.appearancePreferences).palette).toBe('green')
  })

  it('adopts other-window settings without writing them back and ignores other storage areas', async () => {
    const api = await import('./appearancePreferences')
    release = api.initializeAppearancePreferences()
    data.set(api.APPEARANCE_KEY, JSON.stringify({ palette: 'blue', codeFontSize: 20 }))
    storageEvent(api.APPEARANCE_KEY, {})
    expect(get(api.appearancePreferences).palette).toBe('classic')
    storageEvent(api.APPEARANCE_KEY)
    expect(get(api.appearancePreferences)).toMatchObject({ palette: 'blue', codeFontSize: 20 })
    expect(storage.setItem).not.toHaveBeenCalled()
    data.clear()
    storageEvent(null)
    expect(get(api.appearancePreferences)).toEqual(DEFAULT_APPEARANCE)
  })

  it('refreshes preferences changed while every consumer was unmounted', async () => {
    const api = await import('./appearancePreferences')
    api.initializeAppearancePreferences()()
    data.set(api.APPEARANCE_KEY, JSON.stringify({ palette: 'violet', zoom: 150 }))
    storageEvent(api.APPEARANCE_KEY)
    release = api.initializeAppearancePreferences()
    expect(get(api.appearancePreferences)).toMatchObject({ palette: 'violet', zoom: 150 })
  })
})

describe('background mutation and URL lifecycle', () => {
  it('ignores a saved-image read that completes after a replacement commits', async () => {
    const oldRead = deferred<SavedBackground | null>()
    repository.readBackground.mockReturnValueOnce(oldRead.promise)
    const api = await import('./appearancePreferences')
    release = api.initializeAppearancePreferences()
    await api.setWorkspaceBackground(file())
    const committed = get(api.workspaceBackground)
    oldRead.resolve(saved('old.png'))
    await settle()
    expect(get(api.workspaceBackground)).toEqual(committed)
    expect(committed.name).toBe('new.png')
    expect(URL.createObjectURL).toHaveBeenCalledTimes(1)
    expect(get(api.appearancePreferences).background).toBe('image')
  })

  it('does not resurrect a removed image from a late read', async () => {
    const oldRead = deferred<SavedBackground | null>()
    repository.readBackground.mockReturnValueOnce(oldRead.promise)
    const api = await import('./appearancePreferences')
    release = api.initializeAppearancePreferences()
    api.updateAppearance({ background: 'image' })
    await api.removeWorkspaceBackground()
    oldRead.resolve(saved('removed.png'))
    await settle()
    expect(get(api.workspaceBackground)).toMatchObject({ url: null, name: null, loading: false })
    expect(get(api.appearancePreferences).background).toBe('pattern')
    expect(URL.createObjectURL).not.toHaveBeenCalled()
  })

  it('keeps the latest other-window revision when reads finish out of order', async () => {
    const first = deferred<SavedBackground | null>()
    const second = deferred<SavedBackground | null>()
    repository.readBackground.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)
    const api = await import('./appearancePreferences')
    release = api.initializeAppearancePreferences()
    storageEvent(revisionKey)
    second.resolve(saved('latest.png'))
    await settle()
    first.resolve(saved('stale.png'))
    await settle()
    expect(get(api.workspaceBackground).name).toBe('latest.png')
    expect(URL.createObjectURL).toHaveBeenCalledTimes(1)
  })

  it('retains the previous image after a rejected storage write and permits a later retry', async () => {
    repository.readBackground.mockResolvedValueOnce(saved('kept.png'))
    const api = await import('./appearancePreferences')
    release = api.initializeAppearancePreferences()
    await settle()
    const previous = get(api.workspaceBackground)
    repository.writeBackground.mockRejectedValueOnce(new Error('disk quota'))
    await expect(api.setWorkspaceBackground(file())).rejects.toThrow('disk quota')
    expect(get(api.workspaceBackground)).toEqual(previous)
    expect(URL.revokeObjectURL).not.toHaveBeenCalled()
    await api.setWorkspaceBackground(file('retry.png'))
    expect(get(api.workspaceBackground).name).toBe('retry.png')
    expect(URL.revokeObjectURL).toHaveBeenCalledWith(previous.url)
  })

  it('serializes replace and remove across a pending validation', async () => {
    const validation = deferred<Blob>()
    repository.validateBackground.mockReturnValueOnce(validation.promise)
    const api = await import('./appearancePreferences')
    release = api.initializeAppearancePreferences()
    const replacement = api.setWorkspaceBackground(file())
    const removal = api.removeWorkspaceBackground()
    await settle()
    expect(repository.writeBackground).not.toHaveBeenCalled()
    validation.resolve(file())
    await Promise.all([replacement, removal])
    expect(repository.writeBackground.mock.calls.map(([value]) => value?.name ?? null)).toEqual(['new.png', null])
    expect(get(api.workspaceBackground).url).toBeNull()
    expect(get(api.appearancePreferences).background).toBe('pattern')
  })

  it('revokes replaced URLs and keeps a shared URL alive until the final consumer releases', async () => {
    repository.readBackground.mockResolvedValueOnce(saved('initial.png'))
    const api = await import('./appearancePreferences')
    const firstRelease = api.initializeAppearancePreferences()
    release = api.initializeAppearancePreferences()
    await settle()
    const initialUrl = get(api.workspaceBackground).url
    firstRelease()
    expect(URL.revokeObjectURL).not.toHaveBeenCalled()
    await api.setWorkspaceBackground(file())
    expect(URL.revokeObjectURL).toHaveBeenCalledWith(initialUrl)
    const replacementUrl = get(api.workspaceBackground).url
    release()
    release = undefined
    expect(URL.revokeObjectURL).toHaveBeenCalledWith(replacementUrl)
    expect(get(api.workspaceBackground).url).toBeNull()
  })

  it('ignores reads after final disposal and can initialize again', async () => {
    const read = deferred<SavedBackground | null>()
    repository.readBackground.mockReturnValueOnce(read.promise).mockResolvedValueOnce(saved('remounted.png'))
    const api = await import('./appearancePreferences')
    const stop = api.initializeAppearancePreferences()
    stop()
    read.resolve(saved('disposed.png'))
    await settle()
    expect(URL.createObjectURL).not.toHaveBeenCalled()
    release = api.initializeAppearancePreferences()
    await settle()
    expect(get(api.workspaceBackground).name).toBe('remounted.png')
  })

  it('does not allocate an orphan URL when an authorized replacement finishes after disposal', async () => {
    const write = deferred<void>()
    repository.writeBackground.mockReturnValueOnce(write.promise)
    const api = await import('./appearancePreferences')
    const stop = api.initializeAppearancePreferences()
    const replacement = api.setWorkspaceBackground(file())
    await settle()
    stop()
    write.resolve()
    await replacement
    expect(get(api.workspaceBackground).url).toBeNull()
    expect(URL.createObjectURL).not.toHaveBeenCalled()
  })

  it('tolerates repeated release without breaking a later mount', async () => {
    const api = await import('./appearancePreferences')
    const stop = api.initializeAppearancePreferences()
    stop()
    stop()
    repository.readBackground.mockResolvedValueOnce(saved('remounted.png'))
    release = api.initializeAppearancePreferences()
    await settle()
    expect(get(api.workspaceBackground).name).toBe('remounted.png')
  })
})
