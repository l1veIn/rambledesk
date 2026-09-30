// @vitest-environment jsdom
import { tick, type ComponentProps, type SvelteComponent } from 'svelte'
import { createClassComponent } from 'svelte/legacy'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { WebReviewRect, WebReviewSelection } from './webReviewProtocol'
import WebReviewSurface from './WebReviewSurface.svelte'

type FrameHooks = { onSelection: (selection: WebReviewSelection) => void; onAnchor?: (rect: WebReviewRect | null) => void }
const bridge = vi.hoisted(() => ({ options: null as FrameHooks | null, create: vi.fn(), connect: vi.fn(), dispose: vi.fn() }))
vi.mock('./webReviewFrame', () => ({ createWebReviewFrame: (options: FrameHooks) => {
  bridge.options = options
  bridge.create(options)
  return { setMode: vi.fn(), setAnnotations: vi.fn(), focus: vi.fn(), connect: bridge.connect, dispose: bridge.dispose }
} }))

let view: SvelteComponent<ComponentProps<typeof WebReviewSurface>> | undefined
let resized: (() => void) | undefined
const observed = new Set<Element>()
beforeEach(() => {
  bridge.options = null
  vi.clearAllMocks()
  observed.clear()
  vi.stubGlobal('ResizeObserver', class {
    constructor(callback: ResizeObserverCallback) { resized = () => callback([], this as unknown as ResizeObserver) }
    observe(target: Element) { observed.add(target) }
    disconnect() { observed.clear() }
  })
})
afterEach(async () => {
  view?.$destroy()
  view = undefined
  await tick()
  document.body.replaceChildren()
  vi.unstubAllGlobals()
})

describe('web review surface geometry', () => {
  it('tracks the centered viewport and outer scrolling without reloading or changing captured element coordinates', async () => {
    const onSelect = vi.fn()
    const onSelectionRect = vi.fn()
    view = createClassComponent({ component: WebReviewSurface, target: document.body, props: {
      url: 'https://example.com/', viewport: { width: 1440, height: 900 }, onSelect, onSelectionRect, onStatus: vi.fn(),
    } })
    await vi.waitFor(() => expect(bridge.options).not.toBeNull())
    const root = document.querySelector<HTMLDivElement>('[data-web-review-surface]')!
    const frame = root.querySelector('iframe')!
    let availableWidth = 900
    Object.defineProperty(root, 'clientWidth', { configurable: true, get: () => availableWidth })
    Object.defineProperty(root, 'clientHeight', { configurable: true, value: 500 })
    Object.defineProperty(frame, 'clientWidth', { configurable: true, get: () => Number.parseFloat(frame.style.width) })
    vi.spyOn(root, 'getBoundingClientRect').mockImplementation(() => new DOMRect(120, 80, availableWidth, 500))
    vi.spyOn(frame, 'getBoundingClientRect').mockImplementation(() => {
      const width = Number.parseFloat(frame.style.width)
      return new DOMRect(120 + Math.max(0, (availableWidth - width) / 2) - root.scrollLeft,
        80 - root.scrollTop, width, Number.parseFloat(frame.style.height))
    })
    const captured: WebReviewSelection = { page_url: 'https://example.com/', page_title: 'Page', selector: '#cta', tag_name: 'button',
      text: 'Start', attributes: {}, rect: { x: 40, y: 4000, width: 60, height: 20 },
      viewport: { width: 1440, height: 900, scroll_x: 0, scroll_y: 3900 }, captured_at: '2026-09-30T12:00:00Z' }
    const capturedBeforeResize = structuredClone(captured)
    bridge.options!.onSelection(captured)
    bridge.options!.onAnchor?.({ x: 40, y: 100, width: 60, height: 20 })
    expect(onSelectionRect).toHaveBeenLastCalledWith({ x: 40, y: 100, width: 60, height: 20 })
    expect(observed.has(root)).toBe(true)
    expect(observed.has(frame)).toBe(true)

    for (const width of [390, 726]) {
      view.$set({ viewport: { width, height: 844 } })
      await tick()
      resized!()
      expect(onSelectionRect).toHaveBeenLastCalledWith({ x: 40 + (availableWidth - width) / 2, y: 100, width: 60, height: 20 })
      expect([frame.style.width, frame.style.height]).toEqual([`${width}px`, '844px'])
      expect(root.querySelector('iframe')).toBe(frame)
    }
    availableWidth = 1100
    resized!()
    expect(onSelectionRect).toHaveBeenLastCalledWith({ x: 227, y: 100, width: 60, height: 20 })

    view.$set({ viewport: { width: 1440, height: 900 } })
    await tick()
    resized!()
    root.scrollLeft = 300
    root.scrollTop = 50
    bridge.options!.onAnchor?.({ x: 550, y: 100, width: 60, height: 20 })
    root.dispatchEvent(new Event('scroll'))
    expect(onSelectionRect).toHaveBeenLastCalledWith({ x: 250, y: 50, width: 60, height: 20 })
    expect(bridge.create).toHaveBeenCalledTimes(1)
    expect(bridge.connect).toHaveBeenCalledTimes(1)
    expect(bridge.dispose).not.toHaveBeenCalled()
    expect(onSelect).toHaveBeenCalledExactlyOnceWith(capturedBeforeResize)
    expect(captured).toEqual(capturedBeforeResize)
  })

  it('centers clipped narrow viewports on width changes while preserving manual horizontal scrolling', async () => {
    const onSelectionRect = vi.fn()
    view = createClassComponent({ component: WebReviewSurface, target: document.body, props: {
      url: 'https://example.com/', viewport: { width: 390, height: 844 }, onSelect: vi.fn(), onSelectionRect, onStatus: vi.fn(),
    } })
    await vi.waitFor(() => expect(bridge.options).not.toBeNull())
    const root = document.querySelector<HTMLDivElement>('[data-web-review-surface]')!
    const frame = root.querySelector('iframe')!
    let availableWidth = 366
    Object.defineProperty(root, 'clientWidth', { configurable: true, get: () => availableWidth })
    Object.defineProperty(root, 'clientHeight', { configurable: true, value: 500 })
    Object.defineProperty(frame, 'clientWidth', { configurable: true, get: () => Number.parseFloat(frame.style.width) })
    vi.spyOn(root, 'getBoundingClientRect').mockImplementation(() => new DOMRect(0, 0, availableWidth, 500))
    vi.spyOn(frame, 'getBoundingClientRect').mockImplementation(() => new DOMRect(-root.scrollLeft, 0,
      Number.parseFloat(frame.style.width), Number.parseFloat(frame.style.height)))
    bridge.options!.onAnchor?.({ x: 200, y: 100, width: 40, height: 20 })
    resized!()
    expect(root.scrollLeft).toBe(12)
    expect(onSelectionRect).toHaveBeenLastCalledWith({ x: 188, y: 100, width: 40, height: 20 })

    for (const scrollLeft of [0, 24]) {
      root.scrollLeft = scrollLeft
      root.dispatchEvent(new Event('scroll'))
      resized!()
      expect(root.scrollLeft).toBe(scrollLeft)
      expect(onSelectionRect).toHaveBeenLastCalledWith({ x: 200 - scrollLeft, y: 100, width: 40, height: 20 })
    }
    availableWidth = 350
    resized!()
    expect(root.scrollLeft).toBe(20)
    expect(onSelectionRect).toHaveBeenLastCalledWith({ x: 180, y: 100, width: 40, height: 20 })

    view.$set({ viewport: { width: 726, height: 844 } })
    await tick()
    resized!()
    expect(root.scrollLeft).toBe(188)
    expect(onSelectionRect).toHaveBeenLastCalledWith({ x: 12, y: 100, width: 40, height: 20 })
    view.$set({ viewport: { width: 768, height: 1024 } })
    await tick()
    resized!()
    expect(root.scrollLeft).toBe(0)
    expect(onSelectionRect).toHaveBeenLastCalledWith({ x: 200, y: 100, width: 40, height: 20 })
    expect(root.querySelector('iframe')).toBe(frame)
    expect(bridge.create).toHaveBeenCalledTimes(1)
    expect(bridge.connect).toHaveBeenCalledTimes(1)
    expect(bridge.dispose).not.toHaveBeenCalled()
  })
})
