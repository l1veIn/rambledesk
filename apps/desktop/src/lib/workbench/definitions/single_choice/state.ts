import type { WorkbenchState } from '../../../generated/feedback'
import { record, nullableText } from '../stateShape'

type SingleChoiceState = Extract<WorkbenchState, { type: 'single_choice' }>

/** Historical single_choice requests retain their original wire state. */
export function readSingleChoiceState(value: unknown): SingleChoiceState | null {
  return record(value) && value.type === 'single_choice' && nullableText(value.selected_option_id)
    ? value as SingleChoiceState : null
}
