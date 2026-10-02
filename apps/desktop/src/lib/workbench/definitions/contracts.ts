import type { Component } from 'svelte'
import type { ApplicationTransport } from '../../application/applicationTransport'
import type { FeedbackWorkspaceView } from '../../feedback'
import type { WorkbenchSpec, WorkbenchState } from '../../generated/feedback'
import type { WorkbenchFieldAdapter } from '../fields/contracts'

export type WorkbenchController = {
  prepareSubmission: () => Promise<void>
  attach?: (binding: unknown) => () => void
  dispose: () => void
}
export type WorkbenchControllerContext = {
  requestId: string
  getState: () => WorkbenchState | null
  updateState: (state: WorkbenchState) => void
  isEditable: () => boolean
  setBusy: (busy: boolean) => void
  runtime: { transport: ApplicationTransport }
}
export type WorkbenchViewHost = {
  requestId: string
  updateState: (state: WorkbenchState) => void
  quote: (text: string) => void
  openExpanded?: () => void
  selectAction: (id: string, index: number, title: string) => void
  controller?: WorkbenchController
}
export type WorkbenchViewContext = {
  workspace: FeedbackWorkspaceView
  state: WorkbenchState | null
  disabled: boolean
  readOnly: boolean
  activeActionId: string | null
  host: WorkbenchViewHost
}
export type WorkbenchExample = {
  order: number
  title: string
  markdown: string
  spec: WorkbenchSpec
  createSpec?: (options: { origin?: string }) => WorkbenchSpec
  state?: WorkbenchState
  actions?: readonly FeedbackWorkspaceView['actions'][number][]
  attachments?: readonly { id?: string; name: string; content: string; mimeType?: string }[]
}
export type WorkbenchDefinition = {
  type: string
  version: number
  legacy?: boolean
  accepts: (data: Record<string, unknown>) => boolean
  decodeState: (value: unknown) => WorkbenchState | null
  hasInput: (spec: WorkbenchSpec, state: WorkbenchState | null) => boolean
  complete: (spec: WorkbenchSpec, state: WorkbenchState | null) => boolean
  submissionMessage?: (spec: WorkbenchSpec, state: WorkbenchState | null) => string | null
  supportsApproval?: boolean
  layout: { padded: boolean; interactivePreview: boolean; expanded: boolean }
  fields?: readonly WorkbenchFieldAdapter[]
  loadView: () => Promise<{ default: Component<{ context: WorkbenchViewContext }> }>
  createController?: (context: WorkbenchControllerContext) => WorkbenchController
  examples?: readonly WorkbenchExample[]
}
