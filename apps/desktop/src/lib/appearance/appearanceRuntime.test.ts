import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { get, type Writable } from 'svelte/store'
import { DEFAULT_APPEARANCE, type AppearanceSettings } from './appearanceSettings'
import { appearancePreferences, initializeAppearancePreferences, updateAppearance } from './appearancePreferences'
import { appearanceRuntimeError, initializeAppearance } from './appearanceRuntime'

vi.mock('./appearancePreferences', async () => {
  const { writable } = await import('svelte/store')
  const { DEFAULT_APPEARANCE, normalizeAppearance } = await import('./appearanceSettings')
  const preferences = writable({ ...DEFAULT_APPEARANCE })
  return {
    appearancePreferences: preferences,
    workspaceBackground: writable({ url: null, name: null, loading: false, error: null }),
    initializeAppearancePreferences: vi.fn(() => vi.fn()),
    updateAppearance: vi.fn((patch: Partial<AppearanceSettings>) => {
      preferences.update(current => normalizeAppearance({ ...current, ...patch }))
    }),
  }
})

function deferred() {
  let resolve!: () => void
  let reject!: (cause: Error) => void
  const promise = new Promise<void>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}

type ZoomCall = ReturnType<typeof deferred> & { factor: number }
const disposers: (() => void)[] = []
let root: HTMLElement
let addEventListener: ReturnType<typeof vi.fn>
let removeEventListener: ReturnType<typeof vi.fn>

function nativeRuntime() {
  const calls: ZoomCall[] = []
  const setZoom = vi.fn((factor: number) => {
    const call = { factor, ...deferred() }
    calls.push(call)
    return call.promise
  })
  const dispose = initializeAppearance({ setZoom })
  disposers.push(dispose)
  return { calls, setZoom, dispose }
}

async function succeed(call: ZoomCall) {
  call.resolve()
  await call.promise
  await Promise.resolve()
}

async function fail(call: ZoomCall, message = 'native zoom failed') {
  call.reject(new Error(message))
  await call.promise.catch(() => {})
  await Promise.resolve()
}

beforeEach(() => {
  vi.clearAllMocks()
  ;(appearancePreferences as Writable<AppearanceSettings>).set({ ...DEFAULT_APPEARANCE })
  appearanceRuntimeError.set(null)
  root = {
    dataset: {},
    style: { zoom: '', width: '', height: '', setProperty: vi.fn(), removeProperty: vi.fn() },
  } as unknown as HTMLElement
  addEventListener = vi.fn()
  removeEventListener = vi.fn()
  vi.stubGlobal('document', { documentElement: root })
  vi.stubGlobal('window', { addEventListener, removeEventListener })
})

afterEach(() => {
  for (const dispose of disposers.splice(0).reverse()) dispose()
  vi.unstubAllGlobals()
})

describe('appearance zoom runtime', () => {
  it('allows one native request at a time and coalesces rapid changes to the latest zoom', async () => {
    const { calls } = nativeRuntime()
    expect(calls.map(call => call.factor)).toEqual([1])
    updateAppearance({ zoom: 150 })
    updateAppearance({ zoom: 200 })
    expect(calls).toHaveLength(1)
    await succeed(calls[0])
    expect(calls.map(call => call.factor)).toEqual([1, 2])
    updateAppearance({ zoom: 80 })
    updateAppearance({ zoom: 125 })
    expect(calls).toHaveLength(2)
    await succeed(calls[1])
    expect(calls.map(call => call.factor)).toEqual([1, 2, 1.25])
    await succeed(calls[2])
    expect(get(appearancePreferences).zoom).toBe(125)
    expect(get(appearanceRuntimeError)).toBeNull()
  })

  it('retains the requested preference after failure and retries the same selection', async () => {
    const { calls } = nativeRuntime()
    await succeed(calls[0])
    updateAppearance({ zoom: 150 })
    await fail(calls[1])
    expect(get(appearancePreferences).zoom).toBe(150)
    expect(get(appearanceRuntimeError)).toBe('native zoom failed')
    expect(calls).toHaveLength(2)
    updateAppearance({ zoom: 150 })
    expect(calls.map(call => call.factor)).toEqual([1, 1.5, 1.5])
    await succeed(calls[2])
    expect(get(appearanceRuntimeError)).toBeNull()
  })

  it('clears a failed zoom error when the user returns to the already applied zoom', async () => {
    const { calls } = nativeRuntime()
    await succeed(calls[0])
    updateAppearance({ zoom: 150 })
    await fail(calls[1])
    expect(get(appearanceRuntimeError)).toBe('native zoom failed')
    updateAppearance({ zoom: 100 })
    expect(get(appearanceRuntimeError)).toBeNull()
    expect(get(appearancePreferences).zoom).toBe(100)
    expect(calls).toHaveLength(2)
  })

  it('ignores an obsolete rejection after the user has already reverted to the applied zoom', async () => {
    const { calls } = nativeRuntime()
    await succeed(calls[0])
    updateAppearance({ zoom: 150 })
    updateAppearance({ zoom: 100 })
    await fail(calls[1])
    expect(get(appearanceRuntimeError)).toBeNull()
    expect(get(appearancePreferences).zoom).toBe(100)
    expect(calls).toHaveLength(2)
  })

  it('continues to the latest requested zoom after an older request fails', async () => {
    const { calls } = nativeRuntime()
    await succeed(calls[0])
    updateAppearance({ zoom: 150 })
    updateAppearance({ zoom: 200 })
    await fail(calls[1])
    expect(calls.map(call => call.factor)).toEqual([1, 1.5, 2])
    expect(get(appearancePreferences).zoom).toBe(200)
    await succeed(calls[2])
    expect(get(appearanceRuntimeError)).toBeNull()
  })

  it('releases listeners once and ignores late native results after cleanup', async () => {
    const { calls, dispose } = nativeRuntime()
    const release = vi.mocked(initializeAppearancePreferences).mock.results[0].value
    const listener = addEventListener.mock.calls.find(call => call[0] === 'keydown')?.[1]
    dispose()
    dispose()
    updateAppearance({ zoom: 150 })
    await fail(calls[0])
    expect(calls).toHaveLength(1)
    expect(release).toHaveBeenCalledOnce()
    expect(removeEventListener).toHaveBeenCalledExactlyOnceWith('keydown', listener)
    expect(get(appearanceRuntimeError)).toBeNull()
  })

  it('restores browser geometry on cleanup so remounting still resets correctly to 100%', async () => {
    updateAppearance({ zoom: 150 })
    const firstDispose = initializeAppearance()
    disposers.push(firstDispose)
    expect(root.style.zoom).toBe('1.5')
    firstDispose()
    expect([root.style.zoom, root.style.width, root.style.height]).toEqual(['', '', ''])
    const secondDispose = initializeAppearance()
    disposers.push(secondDispose)
    expect(root.style.zoom).toBe('1.5')
    updateAppearance({ zoom: 100 })
    await Promise.resolve()
    await Promise.resolve()
    expect([root.style.zoom, root.style.width, root.style.height]).toEqual(['', '', ''])
    expect(get(appearancePreferences).zoom).toBe(100)
    expect(get(appearanceRuntimeError)).toBeNull()
    secondDispose()
    expect(removeEventListener).toHaveBeenCalledTimes(2)
  })
})
