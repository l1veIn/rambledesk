// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { writable } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { SHADOW_ITEM_MARKER_PROPERTY_NAME, SHADOW_PLACEHOLDER_ITEM_ID, SOURCES, TRIGGERS, type DndEvent } from 'svelte-dnd-action'
import type { FeedbackWorkspaceView } from '../../../feedback'
import type { SortData, WorkbenchState } from '../../../generated/feedback'
import type { WorkbenchViewContext } from '../contracts'
import { sortDefinition } from './definition'
import Harness from './ViewTestHarness.svelte'

const spec = sortDefinition.examples![0].spec
const data = spec.data as SortData
const original = data.items.map((item) => item.id)
let view: ReturnType<typeof mount> | undefined
let context: ReturnType<typeof writable<WorkbenchViewContext>>
let updateState = vi.fn()
let openExpanded = vi.fn()
const originalAnimate = Object.getOwnPropertyDescriptor(Element.prototype, 'animate')
const originalAnimations = Object.getOwnPropertyDescriptor(Element.prototype, 'getAnimations')
const order = () => [...document.querySelectorAll('[data-sort-item-id]')].map((node) => node.getAttribute('data-sort-item-id'))
const button = (label: string) => document.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`)!
type PreviewItem = { id: string; sourceId: string; label: string }
const itemsFor = (ids: string[]) => ids.map((id) => ({ ...data.items.find((item) => item.id === id)!, id: `sort:${id}`, sourceId: id }))
const dragItems = itemsFor(original)

beforeEach(() => {
  updateState = vi.fn((state: WorkbenchState) => context.update((current) => ({ ...current, state })))
  openExpanded = vi.fn()
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => setTimeout(() => callback(0), 0))
  vi.stubGlobal('cancelAnimationFrame', (id: number) => clearTimeout(id))
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }))
  vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(() => new DOMRect(0, 0, 500, 60))
  Object.defineProperty(Element.prototype, 'animate', { configurable: true, value: () => ({ cancel() {}, finished: Promise.resolve() }) })
  Object.defineProperty(Element.prototype, 'getAnimations', { configurable: true, value: () => [] })
})
afterEach(async () => {
  if (view) await unmount(view)
  view = undefined; document.body.replaceChildren(); vi.restoreAllMocks(); vi.unstubAllGlobals()
  if (originalAnimate) Object.defineProperty(Element.prototype, 'animate', originalAnimate)
  else Reflect.deleteProperty(Element.prototype, 'animate')
  if (originalAnimations) Object.defineProperty(Element.prototype, 'getAnimations', originalAnimations)
  else Reflect.deleteProperty(Element.prototype, 'getAnimations')
})
async function open(state: WorkbenchState | null = null, readOnly = false, disabled = false) {
  context = writable({ workspace: { workbench: spec } as FeedbackWorkspaceView, state, disabled, readOnly,
    activeActionId: null, host: { requestId: 'sort-request', updateState, openExpanded, quote() {}, selectAction() {} } })
  view = mount(Harness, { target: document.body, props: { context } })
  await tick()
}
async function dnd(event: 'consider' | 'finalize', items: PreviewItem[], trigger: TRIGGERS, source = SOURCES.POINTER, id = 'sort:config') {
  document.querySelector('[data-sort-list]')!.dispatchEvent(new CustomEvent<DndEvent<PreviewItem>>(event, { detail: { items, info: { trigger, source, id } } }))
  await tick()
}

describe('sort workbench view', () => {
  it('initializes the editable original order and moves stable rows using accessible controls', async () => {
    await open()
    expect(updateState).toHaveBeenCalledExactlyOnceWith({ type: 'sort', order: original })
    expect(order()).toEqual(original)
    expect(button('上移 快速上手').disabled).toBe(true)
    expect(button('下移 配置文件').disabled).toBe(true)
    const row = document.querySelector('[data-sort-item-id="config"]')!
    button('上移 配置文件').focus(); button('上移 配置文件').click(); await tick()
    expect(order()).toEqual(['quickstart', 'errors', 'config', 'completion'])
    expect(document.querySelector('[data-sort-item-id="config"]')).toBe(row)
    expect(document.activeElement).toBe(button('上移 配置文件'))
    expect(original).toEqual(['quickstart', 'errors', 'completion', 'config'])
    ;[...document.querySelectorAll<HTMLButtonElement>('button')].find((node) => node.textContent?.trim() === '全屏工作台')!.click()
    expect(openExpanded).toHaveBeenCalledOnce()
  })
  it('uses the actual drag-handle keyboard interaction to reorder and stop a drag', async () => {
    await open({ type: 'sort', order: original })
    const handle = document.querySelector<HTMLElement>('[data-sort-handle][aria-label="拖动 配置文件"]')!
    handle.focus()
    handle.dispatchEvent(new KeyboardEvent('keydown', { key: ' ', bubbles: true, cancelable: true })); await tick()
    const item = document.querySelector<HTMLElement>('[data-sort-item-id="config"]')!
    item.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowUp', bubbles: true, cancelable: true })); await tick()
    expect(updateState).toHaveBeenLastCalledWith({ type: 'sort', order: ['quickstart', 'errors', 'config', 'completion'] })
    const moved = document.querySelector<HTMLElement>('[data-sort-item-id="config"]')!
    moved.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowUp', bubbles: true, cancelable: true })); await tick()
    expect(order()).toEqual(['quickstart', 'config', 'errors', 'completion'])
    document.querySelector<HTMLElement>('[data-sort-item-id="config"]')!.dispatchEvent(new KeyboardEvent('keydown', { key: ' ', bubbles: true, cancelable: true })); await tick()
    expect(button('上移 配置文件').disabled).toBe(false)
  })
  it('keeps drag preview local and publishes only a complete permutation on drop', async () => {
    await open({ type: 'sort', order: original })
    const shadow = { id: SHADOW_PLACEHOLDER_ITEM_ID, sourceId: 'config', label: '配置文件', [SHADOW_ITEM_MARKER_PROPERTY_NAME]: true }
    await dnd('consider', [...dragItems.slice(0, 3), shadow], TRIGGERS.DRAG_STARTED)
    expect(updateState).not.toHaveBeenCalled()
    expect(document.querySelector('.sort-placeholder')).not.toBeNull()
    await dnd('consider', [shadow, ...dragItems.slice(0, 3)], TRIGGERS.DRAGGED_OVER_INDEX)
    await dnd('finalize', itemsFor(['config', 'quickstart', 'errors', 'completion']), TRIGGERS.DROPPED_INTO_ZONE)
    expect(updateState).toHaveBeenCalledExactlyOnceWith({ type: 'sort', order: ['config', 'quickstart', 'errors', 'completion'] })
    expect(order()).toEqual(['config', 'quickstart', 'errors', 'completion'])
  })
  it('uses the real pointer action through start, movement, observation and drop', async () => {
    // jsdom has no layout engine: give the real action the list and row geometry
    // a browser would supply. Events and drag observation remain unmocked.
    vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (this: Element) {
      const node = this as HTMLElement
      if (node.hasAttribute('data-sort-list')) return new DOMRect(100, 100, 500, 260)
      if (node.parentElement?.hasAttribute('data-sort-list')) {
        const index = [...node.parentElement.children].indexOf(node)
        return new DOMRect(100, 100 + index * 65, 500, 60)
      }
      if (node.id === 'dnd-action-dragged-el') return new DOMRect(100, 295, 500, 60)
      return new DOMRect(0, 0, 800, 800)
    })
    const width = Object.getOwnPropertyDescriptor(document.documentElement, 'scrollWidth')
    const height = Object.getOwnPropertyDescriptor(document.documentElement, 'scrollHeight')
    const scrollingElement = Object.getOwnPropertyDescriptor(document, 'scrollingElement')
    Object.defineProperty(document.documentElement, 'scrollWidth', { configurable: true, value: 800 })
    Object.defineProperty(document.documentElement, 'scrollHeight', { configurable: true, value: 800 })
    Object.defineProperty(document, 'scrollingElement', { configurable: true, value: document.documentElement })
    try {
      await open({ type: 'sort', order: original })
      const handle = document.querySelector<HTMLElement>('[data-sort-handle][aria-label="拖动 配置文件"]')!
      handle.dispatchEvent(new MouseEvent('mousedown', { button: 0, clientX: 120, clientY: 325, bubbles: true, cancelable: true }))
      window.dispatchEvent(new MouseEvent('mousemove', { buttons: 1, clientX: 120, clientY: 315, bubbles: true, cancelable: true }))
      await tick()
      window.dispatchEvent(new MouseEvent('mousemove', { buttons: 1, clientX: 120, clientY: 130, bubbles: true, cancelable: true }))
      await vi.waitFor(() => expect(document.querySelector('[data-sort-list]')?.firstElementChild?.getAttribute('data-sort-item-id')).toBe('config'))
      expect(updateState).not.toHaveBeenCalled()
      window.dispatchEvent(new MouseEvent('mouseup', { button: 0, clientX: 120, clientY: 130, bubbles: true, cancelable: true }))
      await vi.waitFor(() => expect(updateState).toHaveBeenLastCalledWith({ type: 'sort', order: ['config', 'quickstart', 'errors', 'completion'] }))
      expect(order()).toEqual(['config', 'quickstart', 'errors', 'completion'])
    } finally {
      window.dispatchEvent(new MouseEvent('mouseup', { button: 0, clientX: 120, clientY: 130, bubbles: true, cancelable: true }))
      if (width) Object.defineProperty(document.documentElement, 'scrollWidth', width)
      else Reflect.deleteProperty(document.documentElement, 'scrollWidth')
      if (height) Object.defineProperty(document.documentElement, 'scrollHeight', height)
      else Reflect.deleteProperty(document.documentElement, 'scrollHeight')
      if (scrollingElement) Object.defineProperty(document, 'scrollingElement', scrollingElement)
      else Reflect.deleteProperty(document, 'scrollingElement')
    }
  })
  it('rejects invalid drops and stale drops after an external state change', async () => {
    await open({ type: 'sort', order: original })
    await dnd('consider', dragItems, TRIGGERS.DRAG_STARTED)
    await dnd('finalize', [dragItems[0], dragItems[0], ...dragItems.slice(2)], TRIGGERS.DROPPED_INTO_ZONE)
    expect(updateState).not.toHaveBeenCalled()
    await dnd('consider', dragItems, TRIGGERS.DRAG_STARTED)
    context.update((value) => ({ ...value, state: { type: 'sort', order: [...original].reverse() } })); await tick()
    await dnd('finalize', dragItems, TRIGGERS.DROPPED_INTO_ZONE)
    expect(updateState).not.toHaveBeenCalled()
    expect(order()).toEqual([...original].reverse())
  })
  it('shows the saved order without initialization or mutation when read-only or disabled', async () => {
    await open({ type: 'sort', order: [...original].reverse() }, true)
    expect(order()).toEqual([...original].reverse())
    expect(button('上移 配置文件')).toBeNull()
    await dnd('consider', dragItems, TRIGGERS.DRAG_STARTED)
    await dnd('finalize', dragItems, TRIGGERS.DROPPED_INTO_ZONE)
    expect(updateState).not.toHaveBeenCalled()
    await unmount(view!); view = undefined; document.body.replaceChildren()
    await open(null, false, true)
    expect(updateState).not.toHaveBeenCalled()
    expect(document.querySelector('[data-sort-handle]')).toBeNull()
  })
})
