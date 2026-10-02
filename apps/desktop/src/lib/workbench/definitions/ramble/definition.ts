import type { WorkbenchDefinition } from '../contracts'
import { validRambleInput } from './input'
import { examples } from './examples'
export const rambleDefinition: WorkbenchDefinition = {
  type: 'ramble', version: 1, accepts: validRambleInput, decodeState: () => null,
  hasInput: () => false, complete: () => true, supportsApproval: true,
  layout: { padded: true, interactivePreview: false, expanded: false },
  loadView: () => import('./View.svelte'), examples,
}
