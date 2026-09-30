import type { DocumentReviewData } from '../../../generated/feedback'
import type { WorkbenchDefinition } from '../contracts'
import { validDocumentReviewInput } from './input'
import { readDocumentReviewState } from './state'
import { hasReviewInput, validateReviewState } from '../../document-review/reviewModel'
import { documentReviewInputFields } from '../../document-review/inputFields'
import { examples } from './examples'
const message: WorkbenchDefinition['submissionMessage'] = (spec, state) => validateReviewState(spec.data as DocumentReviewData, state?.type === 'document_review' ? state : null)
export const documentReviewDefinition: WorkbenchDefinition = {
  type: 'document_review', version: 1, accepts: validDocumentReviewInput, decodeState: readDocumentReviewState,
  hasInput: (spec, state) => hasReviewInput(spec.data as DocumentReviewData, state?.type === 'document_review' ? state : null),
  complete: (spec, state) => message(spec, state) === null, submissionMessage: message,
  layout: { padded: true, interactivePreview: false, expanded: false }, fields: documentReviewInputFields,
  loadView: () => import('./View.svelte'), examples,
}
