import type { ApplicationTransport } from '../application/applicationTransport'
import type { FeedbackWorkspaceView } from '../feedback'
import type { WorkbenchState } from '../generated/feedback'
import { resolveWorkbenchDefinition } from './definitions/registry'
import type { WorkbenchController } from './definitions/contracts'

export function createWorkbenchLifecycle(context: {
  transport: ApplicationTransport
  getWorkspace: () => FeedbackWorkspaceView | null
  getState: () => WorkbenchState | null
  updateState: (state: WorkbenchState) => void
  isEditable: () => boolean
  onBusy: (requestId: string, busy: boolean) => void
}) {
  let key = '', controller: WorkbenchController | undefined
  function forWorkspace(workspace: FeedbackWorkspaceView | null): WorkbenchController | undefined {
    const definition = resolveWorkbenchDefinition(workspace?.workbench)
    const nextKey = workspace ? `${workspace.request.request_id}:${definition?.type}:${definition?.version}` : ''
    if (nextKey !== key) {
      controller?.dispose(); controller = undefined; key = nextKey
      if (workspace && definition?.createController) {
        const requestId = workspace.request.request_id
        const current = () => context.getWorkspace()?.request.request_id === requestId
        controller = definition.createController({
          requestId, runtime: { transport: context.transport },
          getState: () => current() ? context.getState() : null,
          updateState: (state) => { if (current() && context.isEditable()) context.updateState(state) },
          isEditable: () => current() && context.isEditable(),
          setBusy: (busy) => context.onBusy(requestId, busy),
        })
      }
    }
    return controller
  }
  return {
    forWorkspace,
    async prepareSubmission(requestId: string) {
      const workspace = context.getWorkspace()
      if (workspace?.request.request_id !== requestId || !context.isEditable()) throw new Error('The request changed before submission finished.')
      await forWorkspace(workspace)?.prepareSubmission()
      if (context.getWorkspace()?.request.request_id !== requestId || !context.isEditable()) throw new Error('The request changed before submission finished.')
    },
    dispose() { controller?.dispose(); controller = undefined; key = '' },
  }
}
