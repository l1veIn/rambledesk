import type { WorkbenchSpec } from './generated/feedback'
import { resolveWorkbenchDefinition, decodeRegisteredWorkbenchState } from './workbench/definitions/registry'

export type WorkbenchType = string
export const resolveWorkbenchPolicy = resolveWorkbenchDefinition
export const decodeWorkbenchState = decodeRegisteredWorkbenchState
export function workbenchIsReadOnly(spec: WorkbenchSpec | null | undefined): boolean { return resolveWorkbenchDefinition(spec) === null }
export function workbenchSupportsApproval(spec: WorkbenchSpec | null | undefined): boolean { return resolveWorkbenchDefinition(spec)?.supportsApproval === true }
