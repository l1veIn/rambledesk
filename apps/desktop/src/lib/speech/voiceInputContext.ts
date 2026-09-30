import { getContext, setContext } from 'svelte'
import { readable, type Readable } from 'svelte/store'
import type { SpeechTarget } from './speechDraftQueue'
import type { FeedbackDraftSnapshot } from '../feedbackDraftDocument'
import { workbenchFieldTarget, type WorkbenchFieldDestination } from '../domain/inputTarget'

export type VoiceInputState = {
  requestId: string
  documentTarget: SpeechTarget | null
  nextTarget: SpeechTarget | null
  recording: boolean
  disabled: boolean
  revealTarget?: SpeechTarget | null
  revealSequence?: number
  draftSnapshot?: FeedbackDraftSnapshot | null
}

/** The request owns recording; input views only select a durable destination. */
export type VoiceInputContext = {
  state: Readable<VoiceInputState>
  selectTarget: (target: SpeechTarget) => void
  start: (target: SpeechTarget) => void | Promise<void>
  stop: () => void | Promise<void>
}

export const VOICE_INPUT_CONTEXT = Symbol('rambledesk.voice-input')
export const unavailableVoiceInputState = readable<VoiceInputState>({
  requestId: '', documentTarget: null, nextTarget: null, recording: false, disabled: true,
})

export function provideVoiceInput(context: VoiceInputContext): void { setContext(VOICE_INPUT_CONTEXT, context) }
export function useVoiceInput(): VoiceInputContext | undefined { return getContext<VoiceInputContext | undefined>(VOICE_INPUT_CONTEXT) }

export function workbenchFieldVoiceTarget(state: VoiceInputState,
  field: Omit<WorkbenchFieldDestination, 'kind'>): SpeechTarget | null {
  const document = state.documentTarget
  return document && document.requestId === state.requestId ? workbenchFieldTarget(document, field) : null
}

export function reviewAnnotationVoiceTarget(state: VoiceInputState, annotationId: string, field: 'body' | 'replacement', sourceVersion: string, paragraphLabel: string): SpeechTarget | null {
  const document = state.documentTarget
  return document && document.requestId === state.requestId ? {
    requestId: document.requestId, requestTitle: document.requestTitle,
    destination: { kind: 'review_annotation', annotationId, field, sourceVersion, paragraphLabel },
  } : null
}

export function questionAnswerVoiceTarget(state: VoiceInputState, questionId: string, questionLabel: string): SpeechTarget | null {
  const document = state.documentTarget
  return document && document.requestId === state.requestId ? {
    requestId: document.requestId, requestTitle: document.requestTitle,
    destination: { kind: 'question_answer', questionId, questionLabel },
  } : null
}

export function webReviewAnnotationVoiceTarget(state: VoiceInputState, annotationId: string, elementLabel: string): SpeechTarget | null {
  const document = state.documentTarget
  return document && document.requestId === state.requestId ? {
    requestId: document.requestId, requestTitle: document.requestTitle,
    destination: { kind: 'web_review_annotation', annotationId, elementLabel },
  } : null
}
