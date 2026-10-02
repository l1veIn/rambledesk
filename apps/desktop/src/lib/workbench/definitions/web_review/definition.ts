import type { WebReviewData } from '../../../generated/feedback'
import type { WorkbenchDefinition } from '../contracts'
import { validWebReviewInput } from './input'
import { readWebReviewState } from './state'
import { hasWebReviewInput, validateWebReviewState } from '../../web-review/reviewModel'
import { webReviewInputFields } from '../../web-review/inputFields'
import { examples } from './examples'
const message: WorkbenchDefinition['submissionMessage'] = (spec, state) => validateWebReviewState(spec.data as WebReviewData, state?.type === 'web_review' ? state : null)
export const webReviewDefinition: WorkbenchDefinition = {
  type: 'web_review', version: 1, accepts: validWebReviewInput, decodeState: readWebReviewState,
  hasInput: (spec, state) => hasWebReviewInput(spec.data as WebReviewData, state?.type === 'web_review' ? state : null),
  complete: (spec, state) => message(spec, state) === null, submissionMessage: message,
  layout: { padded: false, interactivePreview: true, expanded: true }, fields: webReviewInputFields,
  loadView: () => import('./View.svelte'), examples,
}
