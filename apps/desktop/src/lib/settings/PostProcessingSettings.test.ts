// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { expect, it, vi } from 'vitest'
import { initializePreferences, setCookingEnabled } from '../preferences'
import PostProcessingSettings from './PostProcessingSettings.svelte'

it('keeps typed URL suffixes, model IDs and keys stable when delayed storage events reach the form', async () => {
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }))
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  const listen = vi.spyOn(window, 'addEventListener')
  initializePreferences()
  setCookingEnabled(true)
  const view = mount(PostProcessingSettings, { target: document.body })
  try {
    await tick()
    const fields = [
      ['tidy-base-url', 'light-cleanup.base-url', 'https://example.test/v', 'https://example.test/v1'],
      ['tidy-model', 'light-cleanup.model', 'model-v', 'model-v1'],
      ['tidy-api-key', 'light-cleanup.api-key', 'fixture-ke', 'fixture-key'],
      ['cooking-base-url', 'cooking.base-url', 'https://cook.test/v', 'https://cook.test/v1'],
      ['cooking-model', 'cooking.model', 'cook-v', 'cook-v1'],
      ['cooking-api-key', 'cooking.api-key', 'fixture-coo', 'fixture-cook'],
    ]
    for (const [id, suffix, oldValue, newValue] of fields) {
      const input = document.getElementById(id) as HTMLInputElement
      input.focus()
      for (const value of [oldValue, newValue]) {
        input.value = value
        input.dispatchEvent(new Event('input', { bubbles: true }))
        await tick()
      }
      const caret = input.selectionStart
      // This notification was queued by the previous keystroke, before the latest write.
      const event = new Event('storage')
      Object.defineProperties(event, {
        key: { value: `rambledesk.${suffix}` }, newValue: { value: oldValue }, storageArea: { value: localStorage },
      })
      window.dispatchEvent(event)
      await tick()
      expect(input.value).toBe(newValue)
      expect(localStorage.getItem(`rambledesk.${suffix}`)).toBe(newValue)
      expect(document.activeElement).toBe(input)
      expect(input.selectionStart).toBe(caret)
    }
    // Updating one field must not reset another field through the provider selector.
    for (const [id, , , value] of fields) expect((document.getElementById(id) as HTMLInputElement).value).toBe(value)
  } finally {
    await unmount(view)
    for (const [type, listener] of listen.mock.calls) if (type === 'storage') window.removeEventListener(type, listener)
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  }
})
