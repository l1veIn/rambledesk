// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { fromStore, writable } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { BrainstormData } from '../../generated/feedback'
import type { InputTarget } from '../../domain/inputTarget'
import { locale } from '../../preferences'
import { VOICE_INPUT_CONTEXT, type VoiceInputState } from '../../speech/voiceInputContext'
import { INPUT_TOOLS_CONTEXT } from '../../input/inputToolsContext'
import { inputText, replaceInputText } from '../../../test/tiptap'
import { brainstormExamples } from '../definitions/brainstorm/examples'
import { brainstormDefinition } from '../definitions/brainstorm/definition'
import { initialBrainstormState } from './brainstormModel'
import type { BrainstormState } from '../definitions/brainstorm/state'
import BrainstormWorkbench from './BrainstormWorkbench.svelte'

const data = brainstormExamples[1].spec.data as BrainstormData
let view: ReturnType<typeof mount> | undefined
beforeEach(() => {
  locale.set('en')
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  Object.defineProperty(Element.prototype, 'scrollIntoView', { configurable: true, value: vi.fn() })
  Object.defineProperty(Range.prototype, 'getClientRects', { configurable: true, value: () => [] })
  Object.defineProperty(Range.prototype, 'getBoundingClientRect', { configurable: true, value: () => new DOMRect() })
})
afterEach(async () => { if (view) await unmount(view); view = undefined; document.body.replaceChildren(); vi.unstubAllGlobals(); vi.restoreAllMocks() })
async function open(state: BrainstormState | null = null, readOnly = false) {
  const source = writable({ state, readOnly, disabled: false }), props = fromStore(source)
  const onChange = vi.fn((next: BrainstormState) => source.update((current) => ({ ...current, state: next })))
  const voiceState = writable<VoiceInputState>({ requestId: data.source_version, documentTarget: null, nextTarget: null, recording: false, disabled: false })
  const start = vi.fn(), selectTarget = vi.fn(), onOpenFeedback = vi.fn(), capture = vi.fn(), files = vi.fn()
  view = mount(BrainstormWorkbench, { target: document.body, props: {
    data, requestId: data.source_version, get state() { return props.current.state }, get readOnly() { return props.current.readOnly },
    get disabled() { return props.current.disabled }, onChange, onOpenFeedback,
  }, context: new Map<symbol, unknown>([
    [VOICE_INPUT_CONTEXT, { state: voiceState, start, stop: vi.fn(), selectTarget }],
    [INPUT_TOOLS_CONTEXT, { state: writable({ requestId: data.source_version, disabled: false, busy: false, canCapture: true, canPaste: true, attachments: [] }),
      capture, files, paste: vi.fn(), preview: vi.fn(), reportError: vi.fn() }],
  ]) })
  await tick()
  return { source, props, onChange, start, voiceState, selectTarget, onOpenFeedback, capture, files }
}
const button = (text: string, scope: ParentNode = document) => [...scope.querySelectorAll<HTMLButtonElement>('button')].find((node) => node.textContent?.trim() === text || node.getAttribute('aria-label') === text)!
const card = (id = 'small-task') => document.querySelector<HTMLElement>(`[data-brainstorm-card="${id}"]`)!
const body = () => document.querySelector<HTMLElement>('[data-brainstorm-field="card_body"]')!
async function fill(input: HTMLInputElement, value: string) { input.value = value; input.dispatchEvent(new Event('input', { bubbles: true })); await tick() }
async function edit(id = 'small-task') { button('Edit idea', card(id)).click(); await tick() }

describe('one brainstorm board across views', () => {
  it('shows the inherited board without mutating it and freely changes views without losing selections', async () => {
    const app = await open(), original = JSON.stringify(data)
    expect(document.querySelectorAll('[data-brainstorm-card]')).toHaveLength(5)
    expect(app.onChange).not.toHaveBeenCalled()
    for (const label of ['Diverge', 'Focus', 'Organize']) { button(label).click(); await tick(); expect(document.querySelectorAll('[data-brainstorm-card]')).toHaveLength(5) }
    expect(app.props.current.state?.selected_card_ids).toEqual(data.initial_board.selected_card_ids)
    expect(app.props.current.state?.cards).toEqual(data.initial_board.cards)
    expect(JSON.stringify(data)).toBe(original)
    expect(app.start).not.toHaveBeenCalled()
  })
  it('keeps typed title/body and a local Ctrl+Enter completion without submitting the round', async () => {
    const app = await open(); await edit()
    await fill(card().querySelector<HTMLInputElement>('input[aria-label="Idea title"]')!, 'My revised idea')
    replaceInputText(body(), 'Human explanation stays.'); await tick()
    const event = new KeyboardEvent('keydown', { key: 'Enter', keyCode: 13, ctrlKey: true, bubbles: true, cancelable: true })
    body().dispatchEvent(event); await tick()
    expect(event.defaultPrevented).toBe(true)
    expect(body()).toBeNull()
    expect(app.props.current.state?.cards[0]).toEqual({ ...data.initial_board.cards[0], title: 'My revised idea', body: 'Human explanation stays.' })
    expect(app.props.current.state?.intent).toBeNull()
    expect(app.onOpenFeedback).not.toHaveBeenCalled()
    await edit(); expect(inputText(body())).toBe('Human explanation stays.')
    body().dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', keyCode: 27, isComposing: true, bubbles: true, cancelable: true })); await tick()
    expect(body()).not.toBeNull()
    body().dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', keyCode: 27, bubbles: true, cancelable: true })); await tick()
    expect(body()).toBeNull()
    expect(app.props.current.state?.cards[0].body).toBe('Human explanation stays.')
  })
  it('adds human cards and groups, moves selected ideas and removes groups without deleting cards', async () => {
    const app = await open(); button('Add idea').click(); await tick()
    const created = app.props.current.state!.cards.at(-1)!
    expect(created).toMatchObject({ origin: 'human', origin_request_id: data.source_version, source_card_ids: [] })
    await fill(document.querySelector<HTMLInputElement>('input[aria-label="Idea title"]')!, 'A human idea')
    button('Done', card(created.id)).click(); await tick()
    button('Add group').click(); await tick()
    const group = app.props.current.state!.groups.at(-1)!
    await fill(document.querySelector<HTMLInputElement>(`[data-brainstorm-group-title="${group.id}"]`)!, 'Later experiments')
    const move = document.querySelector<HTMLSelectElement>('select[aria-label="Move selected…"]')!
    move.value = group.id; move.dispatchEvent(new Event('change', { bubbles: true })); await tick()
    expect(app.props.current.state?.cards.slice(0, 2).map((item) => item.group_id)).toEqual([group.id, group.id])
    button('Remove group', document.querySelector(`[data-brainstorm-group="${group.id}"]`)!).click(); await tick()
    expect(app.props.current.state?.cards).toHaveLength(7)
    expect(app.props.current.state?.cards.slice(0, 2).map((item) => item.group_id)).toEqual([null, null])
    expect(app.props.current.state?.cards.at(-1)?.title).toBe('A human idea')
  })
  it('parks and restores cards with edits intact and never restores a stale selection', async () => {
    const app = await open(); button('Park selected').click(); await tick()
    expect(app.props.current.state?.selected_card_ids).toEqual([])
    expect(app.props.current.state?.cards.filter((item) => item.parked)).toHaveLength(3)
    button('Parked (3)').click(); await tick()
    button('Restore idea', card()).click(); await tick()
    expect(app.props.current.state?.cards[0]).toEqual(data.initial_board.cards[0])
    expect(app.props.current.state?.selected_card_ids).toEqual([])
    expect(app.props.current.state?.cards).toHaveLength(6)
  })
  it('removes accidental empty new cards and confirms removal of entered text, while inherited cards have no delete action', async () => {
    const app = await open()
    expect(button('Remove new idea', card())).toBeUndefined()
    button('Add idea').click(); await tick()
    let id = app.props.current.state!.cards.at(-1)!.id
    button('Remove new idea', card(id)).click(); await tick()
    expect(app.props.current.state?.cards).toHaveLength(6)
    button('Add idea').click(); await tick()
    id = app.props.current.state!.cards.at(-1)!.id
    await fill(document.querySelector<HTMLInputElement>('input[aria-label="Idea title"]')!, 'Keep unless confirmed')
    button('Remove new idea', card(id)).click(); await tick()
    const dialog = document.querySelector('[role="dialog"]')!
    expect(dialog.textContent).toContain('Remove this new idea?')
    button('Cancel', dialog).click(); await tick()
    expect(app.props.current.state?.cards.at(-1)?.title).toBe('Keep unless confirmed')
    button('Remove new idea', card(id)).click(); await tick()
    button('Remove new idea', document.querySelector('[role="dialog"]')!).click(); await tick()
    expect(app.props.current.state?.cards).toHaveLength(6)
  })
  it('validates selection-dependent intent and only opens the existing feedback area from the send entry', async () => {
    const app = await open()
    const action = (intent: string) => document.querySelector<HTMLButtonElement>(`[data-brainstorm-intent="${intent}"]`)!
    action('combine').click(); await tick()
    expect(app.props.current.state?.intent).toBe('combine')
    expect(brainstormDefinition.complete(brainstormExamples[1].spec, app.props.current.state)).toBe(true)
    button('Clear selection').click(); await tick()
    expect(action('combine').disabled).toBe(true)
    expect(action('proceed').disabled).toBe(true)
    expect(brainstormDefinition.complete(brainstormExamples[1].spec, app.props.current.state)).toBe(false)
    action('explore').click(); await tick()
    button('Review and send this round').click()
    expect(app.onOpenFeedback).toHaveBeenCalledOnce()
    expect(app.props.current.state?.intent).toBe('explore')
    expect(app.props.current.state?.cards).toEqual(data.initial_board.cards)
  })
  it('routes speech and attachment tools to the edited card and reveals late writes after changing focus', async () => {
    const app = await open(); await edit()
    button('Speak idea', card()).click(); await tick()
    const target = app.start.mock.lastCall?.[0] as InputTarget
    expect(target.destination).toMatchObject({ kind: 'workbench_field', workbenchType: 'brainstorm', field: 'card_body', entityId: 'small-task', sourceVersion: data.source_version })
    button('Capture', card()).click(); await tick()
    expect(app.capture).toHaveBeenCalledWith(target)
    button('Done', card()).click(); await tick()
    app.source.update((value) => ({ ...value, state: { ...value.state!, cards: value.state!.cards.map((item) => item.id === 'small-task' ? { ...item, body: 'Late speech with [evidence](attachment://proof)' } : item) } }))
    app.voiceState.update((value) => ({ ...value, revealSequence: 1, revealTarget: target })); await tick(); await tick()
    expect(inputText(body())).toContain('Late speech')
    expect(app.props.current.state?.cards[0].body).toContain('attachment://proof')
    expect(app.props.current.state?.selected_card_ids).toEqual(data.initial_board.selected_card_ids)
  })
  it('does not mutate completed boards while allowing users to inspect cards and parked ideas', async () => {
    const app = await open(initialBrainstormState(data), true)
    expect(button('Add idea').disabled).toBe(true)
    expect(button('Park selected').disabled).toBe(true)
    await edit()
    expect(body().getAttribute('contenteditable')).toBe('false')
    expect(card().querySelector<HTMLInputElement>('input[aria-label="Idea title"]')?.disabled).toBe(true)
    expect(button('Park idea', card()).disabled).toBe(true)
    button('Done', card()).click(); await tick()
    button('Parked (1)').click(); await tick()
    expect(card('full-tour')).not.toBeNull()
    expect(app.onChange).not.toHaveBeenCalled()
    expect(app.start).not.toHaveBeenCalled()
  })
})
