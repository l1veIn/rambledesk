// @vitest-environment jsdom
import { mount, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { locale } from '../preferences'
import RecordingOverlay from './RecordingOverlay.svelte'
import type { SpeechOverlayState } from './speechOverlay'
import type { SpeechTarget } from './speechDraftQueue'

const target: SpeechTarget = { requestId: 'request-1', requestTitle: 'Review draft', destination: { kind: 'review_annotation', annotationId: 'note-1', field: 'body', sourceVersion: 'v1', paragraphLabel: 'Opening' } }
function overlay(): SpeechOverlayState {
  return { enabled: true, opacity: 95, selectedGroupId: 'speech-1', shortcuts: { speechAccept: 'Enter', speechDiscard: 'Escape' },
    phase: 'listening', level: 0.2, partial: 'Still speaking', error: '', target,
    nextTarget: { ...target, destination: { kind: 'document', action: null } },
    groups: [{ ...target, ids: ['speech-1'], text: 'Saved pending comment', busy: false, error: '' }], receipt: null }
}
let view: ReturnType<typeof mount> | undefined
beforeEach(() => { locale.set('en'); vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} }) })
afterEach(async () => { if (view) await unmount(view); document.body.replaceChildren(); vi.unstubAllGlobals() })

describe('speech overlay destinations', () => {
  it('labels and opens a question answer independently of the next document destination', () => {
    const state = overlay()
    state.groups[0].destination = { kind: 'question_answer', questionId: 'layout', questionLabel: 'Layout' }
    const onCommand = vi.fn()
    view = mount(RecordingOverlay, { target: document.body, props: { state, onCommand, draggable: false } })
    expect(document.body.textContent).toContain('Pending speech target: Review draft · Layout · Your answer')
    expect(document.body.textContent).toContain('Next segment: Review draft · Feedback document')
    document.querySelector<HTMLButtonElement>('button.destination')!.click()
    expect(onCommand).toHaveBeenCalledWith(expect.objectContaining({ target: expect.objectContaining({ destination: state.groups[0].destination }) }))
  })

  it('keeps pending/current speech targets distinct from the next input and opens the exact annotation', () => {
    const onCommand = vi.fn()
    view = mount(RecordingOverlay, { target: document.body, props: { state: overlay(), onCommand, draggable: false } })
    expect(document.body.textContent).toContain('Pending speech target: Review draft · Opening · Comment')
    expect(document.body.textContent).toContain('Current segment: Review draft · Opening · Comment')
    expect(document.body.textContent).toContain('Next segment: Review draft · Feedback document')
    document.querySelector<HTMLButtonElement>('button.destination')!.click()
    expect(onCommand).toHaveBeenLastCalledWith(expect.objectContaining({ type: 'open-speech-target', requestId: 'request-1', target: expect.objectContaining(target) }))
    const accept = Array.from(document.querySelectorAll('button')).find((button) => button.textContent?.includes('Write to selected input'))!
    accept.click()
    expect(onCommand).toHaveBeenLastCalledWith({ type: 'accept-speech', ids: ['speech-1'] })
  })

  it('preserves an unavailable target visibly without offering a write or navigating elsewhere', () => {
    const state = overlay()
    state.groups[0].destination = { kind: 'unknown', raw: { kind: 'future' } }
    view = mount(RecordingOverlay, { target: document.body, props: { state, draggable: false } })
    expect(document.body.textContent).toContain('Unavailable input target')
    expect(document.querySelector<HTMLButtonElement>('button.destination')?.disabled).toBe(true)
    expect(Array.from(document.querySelectorAll('button')).find((button) => button.textContent?.includes('Write to selected input'))?.disabled).toBe(true)
    expect(Array.from(document.querySelectorAll('button')).find((button) => button.textContent?.trim() === 'Discard')?.disabled).toBe(false)
  })
})
