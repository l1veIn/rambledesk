import type { FeedbackDraftSnapshot } from '../feedbackDraftDocument'
import type { InputWriteWorkspace } from '../domain/inputTarget'
import { readWorkbenchField, sameWorkbenchFieldIdentity, validFieldText } from '../workbenchFields'
import { snapshotSpeechTarget, type SpeechTarget } from './speechTargets'

export type FieldTidyWorkspace = InputWriteWorkspace
export type FieldTidyCapture = {
  target: SpeechTarget
  original: string
  contract: string
  identity: string
}

/** Capture the whole selected field once; later focus changes never redirect it. */
export function captureFieldTidy(workspace: FieldTidyWorkspace, target: SpeechTarget): FieldTidyCapture {
  const field = readWorkbenchField(workspace, target)
  if (!field.value.trim()) throw new Error('Write or speak something in this input before tidying it.')
  return { target: snapshotSpeechTarget(target), original: field.value, contract: field.contract, identity: field.identity }
}

/** Compare and replace against the latest complete draft; opaque data and speech receipts survive. */
export function replaceFieldTidy(workspace: FieldTidyWorkspace, capture: FieldTidyCapture,
  expectedText: string, nextText: string): FeedbackDraftSnapshot {
  const field = readWorkbenchField(workspace, capture.target)
  if (field.contract !== capture.contract || !sameWorkbenchFieldIdentity(capture.identity, field.identity, capture.target)) throw new Error('This input changed after tidying. Keep your original and tidy again.')
  if (field.value !== expectedText) throw new Error('This input was edited after tidying. Your current text has been kept.')
  if (!validFieldText(nextText, field.limit)) throw new Error('The tidied text exceeds this input limit or contains invalid text. Your original has been kept.')
  return field.replace(nextText)
}
