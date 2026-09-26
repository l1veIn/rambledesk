// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { get } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { locale } from '$lib/preferences'
import DraftManagedSessionWorkspace from './DraftManagedSessionWorkspace.svelte'
import { setup } from './draftManagedSessionControllerTestHarness'
import { getSessionStarters } from './sessionStarters'

let view: ReturnType<typeof mount> | undefined
let context: ReturnType<typeof setup> | undefined
const rangeRects = Object.getOwnPropertyDescriptor(Range.prototype, 'getClientRects')
const rangeBounds = Object.getOwnPropertyDescriptor(Range.prototype, 'getBoundingClientRect')
const editor = () => document.querySelector<HTMLElement>('[role="textbox"][contenteditable="true"]')!

beforeEach(() => {
  locale.set('en')
  // Keep Tiptap's real focus path; jsdom supplies no range layout geometry.
  Object.defineProperty(Range.prototype, 'getClientRects', { configurable: true, value: () => [] })
  Object.defineProperty(Range.prototype, 'getBoundingClientRect', { configurable: true, value: () => new DOMRect() })
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }))
})

afterEach(async () => {
  if (view) await unmount(view)
  view = undefined
  await context?.controller.close()
  context = undefined
  document.body.replaceChildren()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  for (const [name, descriptor] of [['getClientRects', rangeRects], ['getBoundingClientRect', rangeBounds]] as const) {
    if (descriptor) Object.defineProperty(Range.prototype, name, descriptor)
    else Reflect.deleteProperty(Range.prototype, name)
  }
})

async function mountDraft(text: string) {
  context = setup()
  const { transport, controller } = context
  transport.resolve('getManagedWorkspaceInfo', { cwd: '/repo', branch: 'main' })
  controller.edit(text)
  view = mount(DraftManagedSessionWorkspace, { target: document.body, props: {
    transport, controller, draftId: 'draft', onConfigure: vi.fn(),
  } })
  await vi.waitFor(() => expect(get(controller).phase).toBe('ready'))
  await tick()
  expect(editor()).not.toBeNull()
  return context
}

describe('new session starter integration', () => {
  it('fills and focuses the real composer without sending or changing the selected agent and project', async () => {
    const { controller, storage, transport, promoted } = await mountDraft('')
    const edit = vi.spyOn(controller, 'edit')
    const prompt = getSessionStarters('en')[0].prompt
    const preparedCalls = transport.callsFor('prepareManagedSession').length
    const card = document.querySelector<HTMLButtonElement>('[data-session-starters] [aria-label="Brainstorm"]')!
    expect(card).not.toBeNull()
    card.click()
    await vi.waitFor(() => expect(document.activeElement).toBe(editor()))

    expect(edit).toHaveBeenCalledExactlyOnceWith(prompt)
    expect(get(controller)).toMatchObject({ phase: 'ready', text: prompt, choice: 'config:config', cwd: '/repo' })
    expect(storage.load('draft')).toEqual({ text: prompt, choice: 'config:config', cwd: '/repo' })
    for (const paragraph of prompt.split('\n\n')) expect(editor().textContent).toContain(paragraph)
    expect(document.querySelector('[data-session-starters]')).toBeNull()
    expect(document.querySelector<HTMLButtonElement>('[aria-label="Send message"]')?.disabled).toBe(false)
    expect(transport.callsFor('sendManagedPrompt')).toHaveLength(0)
    expect(transport.callsFor('prepareManagedSession')).toHaveLength(preparedCalls)
    expect(transport.callsFor('discardPreparedSession')).toHaveLength(0)
    expect(promoted).not.toHaveBeenCalled()
  })

  it('preserves a restored draft and hides the cards until the draft is empty', async () => {
    const draft = 'Keep my own task, including details I already wrote.'
    const { controller, storage, transport } = await mountDraft(draft)
    expect(editor().textContent).toBe(draft)
    expect(document.querySelector('[data-session-starters]')).toBeNull()
    expect(storage.load('draft')).toMatchObject({ text: draft, choice: 'config:config', cwd: '/repo' })

    controller.edit('')
    await tick()
    expect(document.querySelectorAll('[data-session-starters] button')).toHaveLength(3)
    controller.edit('A new task I chose to type instead')
    await tick()
    expect(document.querySelector('[data-session-starters]')).toBeNull()
    expect(editor().textContent).toBe('A new task I chose to type instead')
    expect(get(controller)).toMatchObject({ text: 'A new task I chose to type instead', choice: 'config:config', cwd: '/repo' })
    expect(transport.callsFor('sendManagedPrompt')).toHaveLength(0)
  })
})
