// @vitest-environment jsdom
import { mount, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import SpotlightTour from './SpotlightTour.svelte'
import type { SpotlightStep } from './spotlightGeometry'

let view: ReturnType<typeof mount> | undefined
const showModalDescriptor = Object.getOwnPropertyDescriptor(HTMLDialogElement.prototype, 'showModal')
const closeDescriptor = Object.getOwnPropertyDescriptor(HTMLDialogElement.prototype, 'close')
const scrollDescriptor = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'scrollIntoView')
const scrollIntoView = vi.fn()
const button = (selector: string) => document.querySelector<HTMLButtonElement>(selector)!
const dialog = () => document.querySelector<HTMLDialogElement>('dialog')!

beforeEach(() => {
  Object.defineProperty(HTMLDialogElement.prototype, 'showModal', { configurable: true, value() { this.setAttribute('open', '') } })
  Object.defineProperty(HTMLDialogElement.prototype, 'close', { configurable: true, value() { this.removeAttribute('open') } })
  Object.defineProperty(HTMLElement.prototype, 'scrollIntoView', { configurable: true, value: scrollIntoView })
  scrollIntoView.mockReset()
})
afterEach(async () => {
  if (view) await unmount(view)
  view = undefined
  document.body.replaceChildren()
  for (const [prototype, key, descriptor] of [
    [HTMLDialogElement.prototype, 'showModal', showModalDescriptor],
    [HTMLDialogElement.prototype, 'close', closeDescriptor],
    [HTMLElement.prototype, 'scrollIntoView', scrollDescriptor],
  ] as const) {
    if (descriptor) Object.defineProperty(prototype, key, descriptor)
    else Reflect.deleteProperty(prototype, key)
  }
})

function open(steps: readonly SpotlightStep[] = [
  { id: 'first', target: '[data-first]', title: 'First step', body: 'Look at the first target.' },
  { id: 'second', target: '[data-second]', title: 'Second step', body: 'Look at the second target.' },
]) {
  const scope = document.createElement('section')
  scope.innerHTML = '<button data-first>First target</button><div data-second>Second target</div>'
  scope.scrollTop = 35
  scope.scrollLeft = 12
  document.body.append(scope)
  const target = scope.querySelector<HTMLButtonElement>('[data-first]')!
  target.getBoundingClientRect = () => new DOMRect(40, 100, 250, 80)
  scope.querySelector<HTMLElement>('[data-second]')!.getBoundingClientRect = () => new DOMRect(500, 300, 200, 140)
  target.focus()
  const onDismiss = vi.fn()
  view = mount(SpotlightTour, { target: document.body, props: {
    steps, scope, portrait: '/portrait.png', onDismiss,
    labels: { dialog: 'Workbench guide', skip: 'Skip', back: 'Back', next: 'Next', done: 'Done', step: (index, total) => `Step ${index + 1} of ${total}` },
  } })
  return { target, scope, onDismiss }
}

describe('spotlight tour', () => {
  it('navigates steps without activating the target and dismisses completion once', async () => {
    const { target, onDismiss } = open()
    const targetClick = vi.fn()
    target.addEventListener('click', targetClick)
    await vi.waitFor(() => expect(dialog().open).toBe(true))
    expect(dialog().getAttribute('aria-modal')).toBe('true')
    expect(button('[data-tour-back]').disabled).toBe(true)
    expect(document.activeElement).toBe(button('[data-tour-next]'))
    button('[data-tour-next]').click()
    await vi.waitFor(() => expect(document.querySelector('[data-tour-step]')?.getAttribute('data-tour-step')).toBe('second'))
    expect(button('[data-tour-next]').textContent).toContain('Done')
    button('[data-tour-back]').click()
    await vi.waitFor(() => expect(document.querySelector('[data-tour-step]')?.getAttribute('data-tour-step')).toBe('first'))
    button('[data-tour-next]').click()
    await vi.waitFor(() => expect(button('[data-tour-next]').textContent).toContain('Done'))
    button('[data-tour-next]').click()
    dialog().dispatchEvent(new Event('close'))
    expect(onDismiss).toHaveBeenCalledExactlyOnceWith('complete')
    expect(targetClick).not.toHaveBeenCalled()
    await unmount(view!)
    view = undefined
    expect(document.activeElement).toBe(target)
  })

  it('traps keyboard focus and treats native Escape cancellation as skip', async () => {
    const { onDismiss } = open()
    await vi.waitFor(() => expect(dialog().open).toBe(true))
    button('[data-tour-next]').focus()
    dialog().dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true }))
    expect(document.activeElement).toBe(dialog().querySelector('button'))
    dialog().dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', shiftKey: true, bubbles: true, cancelable: true }))
    expect(document.activeElement).toBe(button('[data-tour-next]'))
    const cancel = new Event('cancel', { cancelable: true })
    dialog().dispatchEvent(cancel)
    expect(cancel.defaultPrevented).toBe(true)
    expect(onDismiss).toHaveBeenCalledExactlyOnceWith('skip')
  })

  it('tracks nested scrolling and safely centers an absent target', async () => {
    const { target, scope } = open()
    await vi.waitFor(() => expect(document.querySelector<HTMLElement>('.spotlight-ring')?.style.top).toBe('100px'))
    target.getBoundingClientRect = () => new DOMRect(40, 160, 250, 80)
    scope.dispatchEvent(new Event('scroll'))
    await vi.waitFor(() => expect(document.querySelector<HTMLElement>('.spotlight-ring')?.style.top).toBe('160px'))
    target.remove()
    await vi.waitFor(() => expect(document.querySelector('.spotlight-ring')).toBeNull())
    expect(document.querySelector('h2')?.textContent).toBe('First step')
    expect(scrollIntoView).toHaveBeenCalledWith({ block: 'nearest', inline: 'nearest', behavior: 'instant' })
  })

  it('returns nested panels to their original scroll position when dismissed', async () => {
    scrollIntoView.mockImplementation(function(this: HTMLElement) {
      this.parentElement!.scrollTop = 250
      this.parentElement!.scrollLeft = 90
    })
    const { scope } = open()
    await vi.waitFor(() => expect(scope.scrollTop).toBe(250))
    button('[data-tour-next]').click()
    await vi.waitFor(() => expect(document.querySelector('[data-tour-step]')?.getAttribute('data-tour-step')).toBe('second'))
    button('[data-tour-skip]').click()
    expect(scope.scrollTop).toBe(35)
    expect(scope.scrollLeft).toBe(12)
  })
})
