import type { Component } from 'svelte'
import type { ApplicationTransport } from '../../application/applicationTransport'
import type { AttachmentView, FeedbackWorkspaceView } from '../../feedback'
import type { WorkbenchSpec, WorkbenchState } from '../../generated/feedback'
import type { WorkbenchFieldAdapter } from '../fields/contracts'

export type WorkbenchSubmissionIntent = 'submit' | 'approve' | 'cancel'
export type WorkbenchController = {
  prepareSubmission: (intent?: WorkbenchSubmissionIntent) => Promise<void>
  attach?: (binding: unknown) => () => void
  dispose: () => void
}
export type WorkbenchControllerContext = {
  requestId: string
  getState: () => WorkbenchState | null
  getWorkspace?: () => FeedbackWorkspaceView | null
  updateState: (state: WorkbenchState) => void
  isEditable: () => boolean
  setBusy: (busy: boolean) => void
  runtime: {
    transport: ApplicationTransport
    persistGeneratedAttachment?: (input: { fileName: string; contents: ArrayBuffer }) => Promise<AttachmentView>
  }
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
export type WorkbenchExampleAttachment = { id?: string; name: string; mimeType?: string } &
  ({ content: string; contentsBase64?: never } | { content?: never; contentsBase64: string })
export type WorkbenchExample = {
  /** Unique optional selector for another example of the same type; development previews only. */
  key?: string
  order: number
  title: string
  markdown: string
  spec: WorkbenchSpec
  createSpec?: (options: { origin?: string }) => WorkbenchSpec
  state?: WorkbenchState
  actions?: readonly FeedbackWorkspaceView['actions'][number][]
  attachments?: readonly WorkbenchExampleAttachment[]
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
