import { sortDefinition } from './sort/definition'
import type { WorkbenchSpec, WorkbenchState } from '../../generated/feedback'
import type { WorkbenchDefinition } from './contracts'
import { rambleDefinition } from './ramble/definition'
import { questionsDefinition } from './questions/definition'
import { singleChoiceDefinition } from './single_choice/definition'
import { documentReviewDefinition } from './document_review/definition'
import { webReviewDefinition } from './web_review/definition'
import { terminalDefinition } from './terminal/definition'
import { visualFeedbackDefinition } from './visual_feedback/definition'
import { diffReviewDefinition } from './diff_review/definition'
import { record, text } from './stateShape'

const developmentDefinitions: readonly WorkbenchDefinition[] = import.meta.env.DEV &&
  (import.meta.env.VITE_WORKBENCH_FIXTURES === '1' || import.meta.env.MODE === 'test')
  ? [(await import('./rating_review/definition')).ratingReviewDefinition] : []

/** The only frontend registration point. Views are lazy; headless callers load no Svelte modules. */
export const workbenchDefinitions: readonly WorkbenchDefinition[] = [
  rambleDefinition, questionsDefinition, singleChoiceDefinition, documentReviewDefinition, webReviewDefinition, terminalDefinition,
  ...developmentDefinitions,
  sortDefinition,
  visualFeedbackDefinition, diffReviewDefinition,
]
export function getWorkbenchDefinition(type: string, version = 1): WorkbenchDefinition | undefined {
  return workbenchDefinitions.find((definition) => definition.type === type && definition.version === version)
}
export function resolveWorkbenchDefinition(spec: WorkbenchSpec | null | undefined): WorkbenchDefinition | null {
  if (!spec) return rambleDefinition
  const definition = getWorkbenchDefinition(spec.type, spec.version)
  return definition && record(spec.data) && definition.accepts(spec.data) ? definition : null
}
export function decodeRegisteredWorkbenchState(value: unknown): WorkbenchState | null {
  return record(value) && text(value.type) ? getWorkbenchDefinition(value.type)?.decodeState(value) ?? null : null
}
export function workbenchIsReadOnly(spec: WorkbenchSpec | null | undefined): boolean {
  return resolveWorkbenchDefinition(spec) === null
}
export function workbenchSupportsApproval(spec: WorkbenchSpec | null | undefined): boolean {
  return resolveWorkbenchDefinition(spec)?.supportsApproval === true
}
