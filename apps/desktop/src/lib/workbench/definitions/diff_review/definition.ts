import type { DiffReviewData } from '../../../generated/feedback'
import type { WorkbenchDefinition } from '../contracts'
import { validDiffReviewInput } from './input'
import { readDiffReviewState } from './state'
import { validateDiffReviewState } from '../../diff-review/reviewModel'
import { diffReviewInputFields } from '../../diff-review/inputFields'
import { examples } from './examples'

const message: WorkbenchDefinition['submissionMessage'] = (spec, value) => {
  if (spec.type !== 'diff_review' || spec.version !== 1 || !validDiffReviewInput(spec.data)) return 'This diff review is unavailable.'
  const state = value === null ? null : readDiffReviewState(value)
  return value !== null && !state ? 'This diff review draft is invalid.' : validateDiffReviewState(spec.data as DiffReviewData, state)
}
export const diffReviewDefinition: WorkbenchDefinition = {
  type: 'diff_review', version: 1, accepts: validDiffReviewInput, decodeState: readDiffReviewState,
  hasInput: (_, state) => (readDiffReviewState(state)?.comments.length ?? 0) > 0,
  complete: (spec, state) => message(spec, state) === null, submissionMessage: message,
  layout: { padded: false, interactivePreview: false, expanded: true }, fields: diffReviewInputFields,
  loadView: () => import('./View.svelte'), examples,
}
