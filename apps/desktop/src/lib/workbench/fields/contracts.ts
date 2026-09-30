import type { InputTarget } from '../../domain/inputTarget'
import type { WorkbenchSpec, WorkbenchState } from '../../generated/feedback'

export type WorkbenchFieldContext = {
  spec: WorkbenchSpec
  state: WorkbenchState
  target: InputTarget
}

/** A typed business field. Replacement returns state, never a path into arbitrary JSON. */
export type WorkbenchFieldValue = {
  value: string
  limit: number
  contract: string
  identity: string
  replace: (value: string) => WorkbenchState
}

export type WorkbenchFieldAdapter = {
  accepts: (target: InputTarget) => boolean
  read: (context: WorkbenchFieldContext) => WorkbenchFieldValue
  /** Used only for local speech provenance; authoritative writes always call read. */
  text: (state: WorkbenchState, target: InputTarget) => string | null
  /** Unlink only this adapter's owned fields and attachment associations. */
  removeAttachment: (state: WorkbenchState, attachmentId: string, removeText: (value: string) => string) => WorkbenchState
  sameIdentity?: (captured: string, current: string, target: InputTarget) => boolean
}
