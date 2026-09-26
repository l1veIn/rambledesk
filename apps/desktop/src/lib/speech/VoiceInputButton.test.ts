// @vitest-environment jsdom
import { mount, unmount } from 'svelte'
import { writable } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { locale } from '../preferences'
import VoiceInputButton from './VoiceInputButton.svelte'
import { VOICE_INPUT_CONTEXT, type VoiceInputState } from './voiceInputContext'
import type { SpeechTarget } from './speechDraftQueue'

const target: SpeechTarget = { requestId: 'request-1', requestTitle: 'Draft review', destination: { kind: 'document', action: null } }
let view: ReturnType<typeof mount> | undefined
beforeEach(() => locale.set('en'))
afterEach(async () => { if (view) await unmount(view); document.body.replaceChildren() })

describe('shared voice input button', () => {
  it('starts or selects the shared session without making a field button pause recording', async () => {
    const state = writable<VoiceInputState>({ requestId: target.requestId, documentTarget: target, nextTarget: null, recording: false, disabled: false })
    const start = vi.fn(), stop = vi.fn(), selectTarget = vi.fn()
    view = mount(VoiceInputButton, { target: document.body, context: new Map([[VOICE_INPUT_CONTEXT, { state, start, stop, selectTarget }]]), props: { target } })
    const button = document.querySelector('button')!
    button.click()
    expect(start).toHaveBeenCalledWith(target)
    state.update((value) => ({ ...value, nextTarget: target, recording: true }))
    await vi.waitFor(() => expect(button.getAttribute('aria-pressed')).toBe('true'))
    expect(button.getAttribute('aria-label')).toBe('Speak here')
    button.click()
    expect(stop).not.toHaveBeenCalled()
    expect(start).toHaveBeenCalledTimes(2)
    expect(selectTarget).not.toHaveBeenCalled()
  })

  it('switches the next target without toggling off a recording in another input and respects live locks', async () => {
    const state = writable<VoiceInputState>({ requestId: target.requestId, documentTarget: target,
      nextTarget: { ...target, destination: { kind: 'review_annotation', annotationId: 'note-1', field: 'body', sourceVersion: 'v1', paragraphLabel: 'Opening' } }, recording: true, disabled: false })
    const start = vi.fn(), stop = vi.fn()
    view = mount(VoiceInputButton, { target: document.body, context: new Map([[VOICE_INPUT_CONTEXT, { state, start, stop, selectTarget: vi.fn() }]]), props: { target } })
    const button = document.querySelector('button')!
    button.click()
    expect(start).toHaveBeenCalledWith(target)
    expect(stop).not.toHaveBeenCalled()
    state.update((value) => ({ ...value, disabled: true }))
    await vi.waitFor(() => expect(button.disabled).toBe(true))
    button.click()
    expect(start).toHaveBeenCalledTimes(1)
  })
})
