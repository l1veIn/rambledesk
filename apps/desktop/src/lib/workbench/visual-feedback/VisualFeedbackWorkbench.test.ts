// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { get, writable, type Writable } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { locale } from '../../preferences'
import type { VisualFeedbackData } from '../../generated/feedback'
import { emptyVisualState, type VisualState } from './visualModel'
import Harness from './VisualTestHarness.svelte'
import VisualMarks from './VisualMarks.svelte'

const data: VisualFeedbackData = { title: 'Sketch', source_version: 'snapshot-1', width: 960, height: 600, image_file_name: null, background_color: '#fff8ed' }
const mark = { id: 'saved_mark', kind: 'arrow' as const, points: [{ x: 20, y: 30 }, { x: 200, y: 180 }], color: '#e5484d', stroke_width: 4, text: '', body: 'Move this' }
let view: ReturnType<typeof mount> | undefined, state: Writable<VisualState | null>, onChange: ReturnType<typeof vi.fn<(state: VisualState) => void>>
const button = (label: string) => document.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`)!
const viewport = () => document.querySelector<HTMLElement>('[role="application"]')!
async function pointer(type: string, x: number, y: number, id = 1) {
  const event = new MouseEvent(type, { clientX: x, clientY: y, bubbles: true, cancelable: true, button: 0 })
  Object.defineProperty(event, 'pointerId', { value: id }); viewport().dispatchEvent(event); await tick()
}
async function open(initial: VisualState | null = null, readOnly = false, disabled = false) {
  state = writable(initial); onChange = vi.fn((next: VisualState) => state.set(next))
  view = mount(Harness, { target: document.body, props: { state, data, readOnly, disabled, onChange } }); await tick()
}
beforeEach(() => {
  locale.set('en')
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => setTimeout(() => callback(0), 0))
  vi.stubGlobal('cancelAnimationFrame', (id: number) => clearTimeout(id))
  vi.spyOn(SVGElement.prototype, 'getBoundingClientRect').mockReturnValue(new DOMRect(10, 20, 480, 300))
})
afterEach(async () => { if (view) await unmount(view); view = undefined; document.body.replaceChildren(); vi.restoreAllMocks(); vi.unstubAllGlobals(); locale.set('zh-CN') })
describe('visual drawing and recovered feedback', () => {
  it('renders chip-free canvas text without mutating the annotation source', async () => {
    const text = 'Move this\n\n[spec.pdf](attachment://known_file)\n\n[Other](attachment://unknown_file)'
    const annotation = { ...mark, kind: 'text' as const, points: [{ x: 20, y: 30 }], text }
    view = mount(VisualMarks, { target: document.body, props: { annotations: [annotation], attachments: [{ attachment_id: 'known_file' }] } })
    await tick()
    const rendered = document.querySelector('text')!.textContent!
    expect(rendered).toContain('Move this'); expect(rendered).not.toContain('[spec.pdf]')
    expect(rendered).toContain('[Other](attachment://unknown_file)'); expect(annotation.text).toBe(text)
  })
  it('commits one arrow after a drag, clears composite output, and supports undo/redo', async () => {
    await open({ ...emptyVisualState(), composite_attachment_id: 'previous_png' })
    button('Arrow').click(); await tick()
    await pointer('pointerdown', 20, 30); await pointer('pointermove', 110, 120)
    expect(onChange).not.toHaveBeenCalled(); expect(document.querySelectorAll('[data-visual-mark-id]')).toHaveLength(1)
    await pointer('pointerup', 110, 120)
    expect(get(state)?.annotations).toMatchObject([{ kind: 'arrow', points: [{ x: 20, y: 20 }, { x: 200, y: 200 }] }])
    expect(get(state)?.composite_attachment_id).toBeNull(); expect(onChange).toHaveBeenCalledTimes(1)
    button('Undo').click(); await tick(); expect(get(state)?.annotations).toEqual([])
    button('Redo').click(); await tick(); expect(get(state)?.annotations).toHaveLength(1)
    button('Zoom in').click(); await tick(); expect(get(state)?.annotations[0].points).toEqual([{ x: 20, y: 20 }, { x: 200, y: 200 }])
  })
  it('creates a freehand path and rectangle, discards cancelled strokes, and rejects zero-area boxes', async () => {
    await open(); onChange.mockClear()
    button('Pen').click(); await tick(); await pointer('pointerdown', 20, 30); await pointer('pointermove', 25, 40); await pointer('pointerup', 30, 50)
    expect(get(state)?.annotations[0].kind).toBe('freehand')
    expect(get(state)?.annotations[0].points.at(-1)).toEqual({ x: 40, y: 60 })
    button('Rectangle').click(); await tick(); await pointer('pointerdown', 30, 40); await pointer('pointerup', 60, 80)
    expect(get(state)?.annotations[1]).toMatchObject({ kind: 'rectangle', points: [{ x: 40, y: 40 }, { x: 100, y: 120 }] })
    await pointer('pointerdown', 30, 40); await pointer('pointermove', 60, 80); await pointer('pointercancel', 60, 80)
    await pointer('pointerdown', 30, 40); await pointer('pointerup', 30, 80)
    expect(get(state)?.annotations).toHaveLength(2)
  })
  it('places text, routes the selected editor, and removes/restores an annotation with keyboard', async () => {
    await open(); button('Text').click(); await tick(); await pointer('pointerdown', 50, 60)
    expect(get(state)?.annotations[0]).toMatchObject({ kind: 'text', text: 'Untitled text', points: [{ x: 80, y: 80 }], stroke_width: 24 })
    expect(document.querySelector('[data-visual-field="text"]')).not.toBeNull()
    viewport().focus(); viewport().dispatchEvent(new KeyboardEvent('keydown', { key: 'Delete', bubbles: true, cancelable: true })); await tick()
    expect(get(state)?.annotations).toEqual([])
    viewport().dispatchEvent(new KeyboardEvent('keydown', { key: 'z', ctrlKey: true, bubbles: true, cancelable: true })); await tick()
    expect(get(state)?.annotations).toHaveLength(1)
  })
  it('recovers immutable annotations in readonly history and permits zoom without mutations', async () => {
    await open({ ...emptyVisualState(), annotations: [mark], composite_attachment_id: 'saved_png' }, true)
    expect(document.querySelector('[data-visual-mark-id="saved_mark"]')).not.toBeNull()
    expect(document.querySelector('svg rect')?.getAttribute('fill')).toBe('#fff8ed')
    expect(button('Arrow')).toBeNull()
    document.querySelector<HTMLButtonElement>('[data-visual-list-id="saved_mark"]')!.click(); await tick()
    expect(document.querySelector('[data-visual-annotation-editor]')?.textContent).toContain('Your comment')
    expect(button('Delete annotation')).toBeNull(); button('Zoom in').click(); await tick()
    await pointer('pointerdown', 30, 40); await pointer('pointerup', 50, 80)
    expect(onChange).not.toHaveBeenCalled(); expect(get(state)?.annotations).toEqual([mark])
  })
  it('accepts restored draft changes as authoritative and cannot undo them into an earlier draft', async () => {
    await open({ ...emptyVisualState(), annotations: [mark] })
    document.querySelector<HTMLButtonElement>('[data-visual-list-id="saved_mark"]')!.click(); await tick()
    button('Delete annotation').click(); await tick(); expect(button('Undo').disabled).toBe(false)
    state.set({ ...emptyVisualState(), annotations: [{ ...mark, body: 'Recovered opinion' }] }); await tick()
    expect(button('Undo').disabled).toBe(true); expect(get(state)?.annotations[0].body).toBe('Recovered opinion')
  })
})
