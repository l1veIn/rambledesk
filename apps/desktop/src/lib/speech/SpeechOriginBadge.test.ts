// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { writable } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { locale } from '../preferences'
import type { FeedbackDraftSnapshot } from '../feedbackDraftDocument'
import SpeechOriginBadge from './SpeechOriginBadge.svelte'
import type { SpeechTarget } from './speechTargets'
import { VOICE_INPUT_CONTEXT, type VoiceInputState } from './voiceInputContext'

const target: SpeechTarget = { requestId: 'request-1', requestTitle: 'Questions',
  destination: { kind: 'question_answer', questionId: 'audience', questionLabel: 'Audience' } }
const snapshot = (status: 'pending' | 'cleaned', value = 'Voice answer'): FeedbackDraftSnapshot => ({ bodyMarkdown: '', documentJson: JSON.stringify({
  schemaVersion: 2, doc: { type: 'doc', content: [] },
  workbenchState: { type: 'questions', answers: [{ id: 'audience', value, label: value, wasCustom: true }] },
  fieldSpeechSegments: { version: 1, segments: [{ segmentId: 'segment-1', target, start: 0, end: 12,
    text: 'Voice answer', state: status, contract: 'question-v1', identity: 'answer-1' }] },
}) })
let view: ReturnType<typeof mount> | undefined
beforeEach(() => locale.set('en'))
afterEach(async () => { if (view) await unmount(view); document.body.replaceChildren() })

describe('field speech origin badge', () => {
  it('reflects the shared pending/cleaned metadata and hides spans replaced by typing', async () => {
    const state = writable<VoiceInputState>({ requestId: target.requestId, documentTarget: null, nextTarget: target,
      recording: false, disabled: false, draftSnapshot: snapshot('pending') })
    view = mount(SpeechOriginBadge, { target: document.body, props: { target }, context: new Map([[VOICE_INPUT_CONTEXT,
      { state, start: vi.fn(), stop: vi.fn(), selectTarget: vi.fn() }]]) })
    expect(document.body.textContent).toContain('1 speech segments to tidy')
    state.update(value => ({ ...value, draftSnapshot: snapshot('cleaned') }))
    await tick()
    expect(document.body.textContent).toContain('1 speech segments tidied')
    expect(document.body.textContent).not.toContain('to tidy')
    state.update(value => ({ ...value, draftSnapshot: snapshot('cleaned', 'Handwritten answer') }))
    await tick()
    expect(document.querySelector('[data-speech-origin-badge]')).toBeNull()
  })

  it('does not show provenance from another request or without a field target', () => {
    const state = writable<VoiceInputState>({ requestId: 'different-request', documentTarget: null, nextTarget: null,
      recording: false, disabled: false, draftSnapshot: snapshot('pending') })
    view = mount(SpeechOriginBadge, { target: document.body, props: { target }, context: new Map([[VOICE_INPUT_CONTEXT,
      { state, start: vi.fn(), stop: vi.fn(), selectTarget: vi.fn() }]]) })
    expect(document.querySelector('[data-speech-origin-badge]')).toBeNull()
  })
})
