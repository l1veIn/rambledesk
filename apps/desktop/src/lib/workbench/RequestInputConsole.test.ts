// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { writable } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { locale } from '../preferences'
import { VOICE_INPUT_CONTEXT, type VoiceInputState } from '../speech/voiceInputContext'
import type { SpeechTarget } from '../speech/speechTargets'
import RequestInputConsole from './RequestInputConsole.svelte'
import FeedbackEditorPanel from './FeedbackEditorPanel.svelte'
import { previewFixtures } from '../preview/previewFixtures'

const documentTarget: SpeechTarget = {
  requestId: 'request-1', requestTitle: 'Script review', destination: { kind: 'document', action: null },
}
const comment: SpeechTarget = { ...documentTarget, destination: {
  kind: 'review_annotation', annotationId: 'note-1', field: 'body', sourceVersion: 'v1', paragraphLabel: 'Opening',
} }
let view: ReturnType<typeof mount> | undefined
beforeEach(() => locale.set('en'))
afterEach(async () => { if (view) await unmount(view); document.body.replaceChildren() })

function setup(nextTarget: SpeechTarget | null = comment) {
  const state = writable<VoiceInputState>({ requestId: documentTarget.requestId, documentTarget, nextTarget,
    recording: true, disabled: false })
  const start = vi.fn(), stop = vi.fn(), open = vi.fn()
  const selectTarget = vi.fn((target: SpeechTarget) => state.update(value => ({ ...value, nextTarget: target })))
  view = mount(RequestInputConsole, { target: document.body, props: { portrait: '/rambelle.webp' },
    context: new Map([[VOICE_INPUT_CONTEXT, { state, start, stop, open, selectTarget }]]) })
  return { state, start, stop, open, selectTarget }
}

describe('request input console', () => {
  it('reflects the shared recording in one bubble without footer navigation or recording controls', async () => {
    const app = setup()
    const bubble = document.querySelector('[data-rambelle-bubble]')!
    expect(bubble.textContent).toContain('Commander, I am recording this now.')
    expect(document.querySelector('[data-request-input-console] button')).toBeNull()
    expect(document.querySelector('[data-input-target]')).toBeNull()
    expect(document.querySelector('[data-request-input-caption]')).toBeNull()
    app.state.update(value => ({ ...value, recording: false }))
    await tick()
    expect(bubble.textContent).toContain('Commander, standing by.')
    expect(app.start).not.toHaveBeenCalled()
    expect(app.stop).not.toHaveBeenCalled()
  })

  it('focuses the existing document editor when asked to reveal the default target', async () => {
    const workspace = previewFixtures.workspace
    const target = { ...documentTarget, requestId: workspace.request.request_id }
    const state = writable<VoiceInputState>({ requestId: target.requestId, documentTarget: target, nextTarget: target,
      recording: false, disabled: false, revealTarget: null, revealSequence: 0 })
    const start = vi.fn(), stop = vi.fn()
    view = mount(FeedbackEditorPanel, { target: document.body, props: { workspace, formatTime: () => '10:00' },
      context: new Map([[VOICE_INPUT_CONTEXT, { state, start, stop, selectTarget: vi.fn() }]]) })
    await vi.waitFor(() => expect(document.querySelector('[contenteditable="true"]')).not.toBeNull())
    const editor = document.querySelector<HTMLElement>('[contenteditable="true"]')!
    const elsewhere = document.createElement('button')
    document.body.append(elsewhere)
    elsewhere.focus()
    state.update(value => ({ ...value, revealTarget: target, revealSequence: 1 }))
    await vi.waitFor(() => expect(document.activeElement).toBe(editor))
    expect(start).not.toHaveBeenCalled()
    expect(stop).not.toHaveBeenCalled()
  })
})
