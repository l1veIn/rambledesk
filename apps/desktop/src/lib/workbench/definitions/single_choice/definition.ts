import type { SingleChoiceData } from '../../../generated/feedback'
import type { WorkbenchDefinition } from '../contracts'
import { validSingleChoiceInput } from './input'
import { readSingleChoiceState } from './state'
const hasInput: WorkbenchDefinition['hasInput'] = (spec, state) => state?.type === 'single_choice' &&
  (spec.data as SingleChoiceData).options.some((option) => option.id === state.selected_option_id)
export const singleChoiceDefinition: WorkbenchDefinition = {
  type: 'single_choice', version: 1, legacy: true, accepts: validSingleChoiceInput, decodeState: readSingleChoiceState,
  hasInput, complete: hasInput,
  layout: { padded: true, interactivePreview: false, expanded: false },
  loadView: () => import('./View.svelte'),
}
