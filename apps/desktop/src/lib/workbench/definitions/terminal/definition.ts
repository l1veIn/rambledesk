import type { TerminalData } from '../../../generated/feedback'
import type { WorkbenchDefinition } from '../contracts'
import { validTerminalInput } from './input'
import { readTerminalState } from './state'
import { hasTerminalInput, validateTerminalState } from '../../terminalModel'
import { createTerminalWorkbenchController } from '../../terminal/terminalWorkbenchController'
import { examples } from './examples'
const message: WorkbenchDefinition['submissionMessage'] = (spec, state) => state && state.type !== 'terminal'
  ? 'Invalid terminal trial state.' : validateTerminalState(spec.data as TerminalData, state?.type === 'terminal' ? state : null)
export const terminalDefinition: WorkbenchDefinition = {
  type: 'terminal', version: 1, accepts: validTerminalInput, decodeState: readTerminalState,
  hasInput: (spec, state) => hasTerminalInput(spec.data as TerminalData, state?.type === 'terminal' ? state : null),
  complete: (spec, state) => message(spec, state) === null, submissionMessage: message,
  layout: { padded: false, interactivePreview: true, expanded: true },
  loadView: () => import('./View.svelte'), createController: createTerminalWorkbenchController, examples,
}
