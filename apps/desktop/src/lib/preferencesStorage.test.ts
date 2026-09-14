import { get } from 'svelte/store'
import { afterEach, describe, expect, it, vi } from 'vitest'

type Preferences = typeof import('./preferences')
type TestWindow = { storage: Storage; target: EventTarget; preferences: Preferences }

/** Separate module instances share storage; storage events arrive asynchronously. */
function windows() {
  const values = new Map<string, string>()
  const views: TestWindow[] = []
  const events: Array<{ view: TestWindow; key: string; oldValue: string | null; newValue: string | null }> = []
  const writes: Array<{ view: TestWindow; key: string }> = []
  function activate(view: TestWindow) {
    vi.stubGlobal('localStorage', view.storage)
    vi.stubGlobal('navigator', { language: 'en-US' })
    vi.stubGlobal('document', { documentElement: { dataset: {}, style: {} } })
    vi.stubGlobal('window', Object.assign(view.target, {
      matchMedia: () => ({ matches: false, addEventListener() {} }),
    }))
  }
  function emit(view: TestWindow, key: string, oldValue: string | null, newValue: string | null, area = view.storage) {
    activate(view)
    const event = new Event('storage')
    Object.defineProperties(event, {
      key: { value: key }, oldValue: { value: oldValue }, newValue: { value: newValue }, storageArea: { value: area },
    })
    view.target.dispatchEvent(event)
  }
  return {
    values, events, writes, activate, emit,
    async open() {
      const view = { target: new EventTarget() } as TestWindow
      function write(key: string, value: string | null) {
        writes.push({ view, key })
        const oldValue = values.get(key) ?? null
        if (oldValue === value) return
        if (value === null) values.delete(key)
        else values.set(key, value)
        for (const other of views) if (other !== view) events.push({ view: other, key, oldValue, newValue: value })
      }
      view.storage = {
        get length() { return values.size },
        getItem: key => values.get(key) ?? null,
        key: index => [...values.keys()][index] ?? null,
        setItem: (key, value) => write(key, value),
        removeItem: key => write(key, null),
        clear: () => { for (const key of values.keys()) write(key, null) },
      }
      activate(view)
      vi.resetModules()
      view.preferences = await import('./preferences')
      view.preferences.initializePreferences()
      views.push(view)
      return view
    },
    drain(limit = 100) {
      let delivered = 0
      while (events.length && delivered++ < limit) {
        const event = events.shift()!
        emit(event.view, event.key, event.oldValue, event.newValue)
      }
      return events.length
    },
  }
}

afterEach(() => { vi.unstubAllGlobals() })

describe('preference synchronization across the main window and floating windows', () => {
  it.each([
    ['light-cleanup.base-url', 'tidyBaseUrl', 'setTidyBaseUrl', 'https://example.test/v', 'https://example.test/v1'],
    ['light-cleanup.model', 'tidyModel', 'setTidyModel', 'model-v', 'model-v1'],
    ['light-cleanup.api-key', 'tidyApiKey', 'setTidyApiKey', 'fixture-ke', 'fixture-key'],
    ['cooking.base-url', 'cookingBaseUrl', 'setCookingBaseUrl', 'https://cook.test/v', 'https://cook.test/v1'],
    ['cooking.model', 'cookingModel', 'setCookingModel', 'cook-v', 'cook-v1'],
    ['cooking.api-key', 'cookingApiKey', 'setCookingApiKey', 'fixture-coo', 'fixture-cook'],
  ] as const)('keeps the newest %s edit without circulating earlier keystrokes', async (suffix, store, setter, first, last) => {
    const bus = windows()
    const main = await bus.open()
    const overlays = [await bus.open(), await bus.open(), await bus.open()]
    expect(bus.drain()).toBe(0)
    bus.writes.length = 0
    bus.activate(main)
    const observed: string[] = []
    const unsubscribe = main.preferences[store].subscribe(value => observed.push(value))
    main.preferences[setter](first)
    main.preferences[setter](last)
    expect(bus.drain()).toBe(0)
    expect(bus.values.get(`rambledesk.${suffix}`)).toBe(last)
    expect(observed.slice(1)).toEqual([first, last])
    for (const view of overlays) expect(get(view.preferences[store])).toBe(last)
    expect(bus.writes).toHaveLength(2)
    expect(bus.writes.every(write => write.view === main)).toBe(true)
    unsubscribe()
  })

  it('does not overwrite a newer local edit with an older event from another window', async () => {
    const bus = windows()
    const main = await bus.open()
    const other = await bus.open()
    bus.drain()
    bus.activate(other)
    other.preferences.setTidyModel('older-model')
    bus.activate(main)
    main.preferences.setTidyModel('newer-model')
    const seen: string[] = []
    const unsubscribe = main.preferences.tidyModel.subscribe(value => seen.push(value))
    expect(bus.drain()).toBe(0)
    expect(seen).toEqual(['newer-model'])
    expect(get(other.preferences.tidyModel)).toBe('newer-model')
    unsubscribe()
  })

  it('applies credential removal without restoring it or writing an empty credential back', async () => {
    const bus = windows()
    const main = await bus.open()
    const overlay = await bus.open()
    bus.activate(main)
    main.preferences.setTidyApiKey('fixture-key')
    bus.drain()
    bus.activate(main)
    bus.writes.length = 0
    main.storage.removeItem('rambledesk.light-cleanup.api-key')
    expect(bus.drain()).toBe(0)
    expect(get(overlay.preferences.tidyApiKey)).toBe('')
    expect(bus.values.has('rambledesk.light-cleanup.api-key')).toBe(false)
    expect(bus.writes).toHaveLength(1)
  })

  it('does not persist object and normalized numeric preferences received from another window', async () => {
    const bus = windows()
    const main = await bus.open()
    const overlay = await bus.open()
    bus.drain()
    bus.activate(main)
    bus.writes.length = 0
    main.storage.setItem('rambledesk.speech.hotwords', '[" example "]')
    main.storage.setItem('rambledesk.speech.overlay-opacity', '1')
    expect(bus.drain()).toBe(0)
    expect(get(overlay.preferences.speechHotwords)).toEqual(['example'])
    expect(get(overlay.preferences.speechOverlayOpacity)).toBe(30)
    expect(bus.writes).toHaveLength(2)
    expect(bus.values.get('rambledesk.speech.overlay-opacity')).toBe('1')
  })

  it('ignores events for sessionStorage and other storage areas', async () => {
    const bus = windows()
    const main = await bus.open()
    const key = 'rambledesk.light-cleanup.model'
    const original = get(main.preferences.tidyModel)
    bus.emit(main, key, original, 'foreign-model', {} as Storage)
    expect(get(main.preferences.tidyModel)).toBe(original)
  })
})
