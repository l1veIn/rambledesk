// @vitest-environment jsdom
import { mount, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { SessionConfigChange, SessionConfiguration } from '$lib/generated/feedback'
import SessionConfigurationControls from './SessionConfigurationControls.svelte'

function configuration(): SessionConfiguration {
  return {
    options: [
      {
        id: 'model-picker',
        category: 'model',
        name: 'Model',
        description: 'Select a model',
        kind: {
          type: 'select',
          current_value: 'provider/model-a',
          options: [
            { value: 'provider/model-a', name: 'Flash', description: null, group: 'Provider' },
            { value: 'provider/model-b', name: 'Pro', description: null, group: 'Provider' },
          ],
        },
      },
      {
        id: 'reasoning',
        category: 'reasoning_effort',
        name: 'Reasoning effort',
        description: null,
        kind: {
          type: 'select',
          current_value: 'high',
          options: [
            { value: 'low', name: 'Low', description: null, group: null },
            { value: 'high', name: 'High', description: null, group: null },
          ],
        },
      },
      { id: 'auto', category: null, name: 'Auto approve', description: null, kind: { type: 'boolean', current_value: false } },
    ],
  }
}

function phoneMatchMedia(matches: boolean) {
  window.matchMedia = ((query: string) => ({
    matches,
    media: query,
    onchange: null,
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false,
  })) as never
}

async function settle() {
  await new Promise((resolve) => setTimeout(resolve, 20))
}

describe('compact session configuration', () => {
  let host: HTMLElement

  beforeEach(() => {
    phoneMatchMedia(true)
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    } as never
    host = document.createElement('div')
    document.body.append(host)
  })

  afterEach(() => {
    host.remove()
    vi.unstubAllGlobals()
  })

  it('collapses every option behind the model entry', async () => {
    const app = mount(SessionConfigurationControls, {
      target: host,
      props: { configuration: configuration(), onChange: vi.fn() },
    })
    await settle()

    const trigger = host.querySelector('[data-session-config-compact]')
    expect(trigger).not.toBeNull()
    expect(trigger?.textContent).toContain('Flash')
    // The inline selectors are gone on a phone.
    expect(host.querySelector('[data-session-config-controls]')).toBeNull()
    expect(host.querySelectorAll('select')).toHaveLength(0)
    await unmount(app)
  })

  it('opens a popover listing every option and applies a selection', async () => {
    const onChange = vi.fn(async (_change: SessionConfigChange) => undefined)
    const app = mount(SessionConfigurationControls, {
      target: host,
      props: { configuration: configuration(), onChange },
    })
    await settle()

    ;(host.querySelector('[data-session-config-compact]') as HTMLElement).click()
    await settle()

    const content = document.querySelector('[data-session-config-compact-content]')!
    expect(content.textContent).toContain('Reasoning effort')
    expect(content.textContent).toContain('Auto approve')

    const pro = [...document.querySelectorAll('button')].find((button) =>
      button.textContent?.includes('Pro'),
    )
    expect(pro).toBeDefined()
    ;(pro as HTMLElement).click()
    await settle()

    expect(onChange).toHaveBeenCalledWith({
      config_id: 'model-picker',
      value: { type: 'select', value: 'provider/model-b' },
    })
    // Close the popover before teardown so bits-ui's scroll lock cleans up while
    // the DOM still exists.
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    await settle()
    await unmount(app)
    await settle()
  })

  it('keeps the inline controls on a wide viewport', async () => {
    phoneMatchMedia(false)
    const app = mount(SessionConfigurationControls, {
      target: host,
      props: { configuration: configuration(), onChange: vi.fn() },
    })
    await settle()

    expect(host.querySelector('[data-session-config-controls]')).not.toBeNull()
    expect(host.querySelector('[data-session-config-compact]')).toBeNull()
    expect(host.querySelectorAll('select')).toHaveLength(2)
    await unmount(app)
    await settle()
  })
})
