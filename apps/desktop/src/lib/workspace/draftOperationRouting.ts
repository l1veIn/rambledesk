import type { WorkspaceViewDescriptor } from './viewDescriptors'

export type DraftOperationRouteContext = Readonly<{
  activeView: WorkspaceViewDescriptor | null
  workbenchMounted: boolean
  editorReady: boolean
  workspaceRequestId: string | null
  requestId: string
}>

export function shouldUseForegroundDraftEditor(
  context: DraftOperationRouteContext,
): boolean {
  return (
    context.activeView?.kind === 'session' &&
    context.workbenchMounted &&
    context.editorReady &&
    context.workspaceRequestId === context.requestId
  )
}

export function shouldAdoptTaskBackgroundDraft(
  activeView: WorkspaceViewDescriptor | null,
  workspaceRequestId: string | null,
  completedRequestId: string,
): boolean {
  return (
    activeView?.kind === 'request-task' &&
    activeView.requestId === completedRequestId &&
    workspaceRequestId === completedRequestId
  )
}
