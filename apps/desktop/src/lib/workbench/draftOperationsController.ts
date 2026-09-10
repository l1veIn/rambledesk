import { tick } from 'svelte'
import { get, writable } from 'svelte/store'

import { readApplicationSnapshot } from '../application/readApplicationSnapshot'
import type { ApplicationTransport } from '../application/applicationTransport'
import { writeBackgroundDraftOperation } from '../backgroundDraftWriter'
import type { ActiveAction, DraftOperation } from '../draftOperations'
import type { DraftView, FeedbackRequestSummary, FeedbackWorkspaceView } from '../feedback'
import {
  shouldAdoptTaskBackgroundDraft,
  shouldUseForegroundDraftEditor,
} from '../workspace/draftOperationRouting'
import type { WorkspaceViewDescriptor } from '../workspace/viewDescriptors'
import type { FeedbackEditorHandle } from '../editor/feedbackEditorHandle'

export type DraftOperationsContext = {
  transport: ApplicationTransport
  tr: (source: string, values?: Record<string, string | number>) => string
  messageFrom: (cause: unknown) => string
  getActiveView: () => WorkspaceViewDescriptor | null
  getPendingViewKey: () => string | null
  getWorkspace: () => FeedbackWorkspaceView | null
  getCurrentRequest: () => FeedbackRequestSummary | null
  getEditor: () => FeedbackEditorHandle | undefined
  isWorkbenchMounted: () => boolean
  isTransitionLocked: () => boolean
  getDraftMessage: () => string
  saveDraftNow: () => Promise<boolean>
  setWorkspaceDraft: (draft: DraftView) => void
  adoptDraft: (draft: DraftView) => void
  setPageError: (message: string) => void
}

/**
 * Draft document writes: the serial queue that keeps background document
 * operations ordered, the foreground/background routing for a single
 * operation, and the action-group selection attached to the open request.
 */
export function createDraftOperationsController(context: DraftOperationsContext) {
  const activeActionByRequest = writable<ReadonlyMap<string, NonNullable<ActiveAction>>>(new Map())
  let documentQueue: Promise<void> = Promise.resolve()

  function activeActionFor(requestId: string): ActiveAction {
    return get(activeActionByRequest).get(requestId) ?? null
  }

  function activeActionId(requestId: string | null): string | null {
    if (!requestId) return null
    return get(activeActionByRequest).get(requestId)?.actionId ?? null
  }

  function enqueueDocumentTask<T>(task: () => Promise<T>): Promise<T> {
    const run = documentQueue.then(task)
    documentQueue = run.then(
      () => undefined,
      () => undefined,
    )
    return run
  }

  /** Resolves after every document task queued so far has settled. */
  function waitForDocumentQueue(): Promise<void> {
    return documentQueue.catch(() => {})
  }

  async function routeDraftOperation(requestId: string, operation: DraftOperation): Promise<void> {
    if (!requestId) return
    const run = enqueueDocumentTask(async () => {
      const foregroundWorkspace = context.getWorkspace()
      if (
        shouldUseForegroundDraftEditor({
          activeView: context.getActiveView(),
          workbenchMounted: context.isWorkbenchMounted(),
          editorReady: context.getEditor() !== undefined,
          workspaceRequestId: foregroundWorkspace?.request.request_id ?? null,
          requestId,
        }) &&
        foregroundWorkspace
      ) {
        if (
          foregroundWorkspace.request.status === 'completed' ||
          foregroundWorkspace.request.status === 'cancelled'
        ) {
          throw new Error(context.tr('This request is closed. The document is read-only.'))
        }
        let applied = context.getEditor()?.applyDraftOperation(operation) ?? false
        if (!applied) {
          await tick()
          applied = context.getEditor()?.applyDraftOperation(operation) ?? false
        }
        if (!applied) {
          throw new Error(context.tr('The current editor is not ready. Try the action again.'))
        }
        if (!(await context.saveDraftNow())) {
          throw new Error(context.getDraftMessage() || context.tr('The current draft could not be saved.'))
        }
        return
      }

      const savedDraft = await writeBackgroundDraftOperation(requestId, operation, {
        load: async () => {
          const target = await readApplicationSnapshot(
            context.transport,
            'getFeedbackWorkspace',
            { request_id: requestId },
          )
          if (!target) throw new Error(context.tr('This feedback request could not be found.'))
          return target
        },
        save: async (input) => context.transport.call('saveFeedbackDraft', input),
      })
      if (
        shouldAdoptTaskBackgroundDraft(
          context.getActiveView(),
          context.getCurrentRequest()?.request_id ?? null,
          requestId,
        ) &&
        context.getWorkspace()
      ) {
        context.setWorkspaceDraft(savedDraft)
        context.adoptDraft(savedDraft)
      }
    })
    try {
      await run
    } catch (cause) {
      context.setPageError(
        context.tr('Failed to write Ramble content: {error}', { error: context.messageFrom(cause) }),
      )
      throw cause
    }
  }

  function selectAction(actionId: string, actionIndex: number, title: string) {
    const request = context.getCurrentRequest()
    const requestId = request?.request_id
    if (
      !requestId ||
      context.isTransitionLocked() ||
      context.getPendingViewKey() !== null ||
      request?.status === 'completed' ||
      request?.status === 'cancelled'
    ) return
    if (get(activeActionByRequest).get(requestId)?.actionId === actionId) {
      activeActionByRequest.update((current) => {
        const next = new Map(current)
        next.delete(requestId)
        return next
      })
      void routeDraftOperation(requestId, { kind: 'clearActionGroup', actionId }).catch(() => {})
      return
    }
    const action = { actionId, actionIndex, title }
    activeActionByRequest.update((current) => new Map(current).set(requestId, action))
    void routeDraftOperation(requestId, { kind: 'startActionGroup', action }).catch(() => {})
  }

  return {
    subscribe: activeActionByRequest.subscribe,
    activeActionFor,
    activeActionId,
    enqueueDocumentTask,
    routeDraftOperation,
    selectAction,
    waitForDocumentQueue,
  }
}

export type DraftOperationsController = ReturnType<typeof createDraftOperationsController>
