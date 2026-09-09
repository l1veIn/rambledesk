import { get, writable } from 'svelte/store'

import type {
  FeedbackRequestSummary,
  FeedbackRequestView,
  FeedbackWorkspaceView,
} from '../feedback'
import type { PublishedFeedbackView } from '../publishedFeedback'
import type { SubmitStage } from '../domain/sessionPhases'

/**
 * The open feedback request and the server mutations running against it.
 *
 * Every controller that reads or writes the current request goes through this store,
 * so `App.svelte` no longer has to thread getters and setters between them.
 */
export type WorkspaceSessionState = Readonly<{
  workspace: FeedbackWorkspaceView | null
  completedResult: FeedbackRequestView | null
  publishedFeedback: PublishedFeedbackView | null
  loadingWorkspace: boolean
  submitting: boolean
  submitStage: SubmitStage
  approving: boolean
  cancelling: boolean
}>

const initial: WorkspaceSessionState = {
  workspace: null,
  completedResult: null,
  publishedFeedback: null,
  loadingWorkspace: false,
  submitting: false,
  submitStage: 'idle',
  approving: false,
  cancelling: false,
}

export type WorkspaceSession = ReturnType<typeof createWorkspaceSession>

export function createWorkspaceSession() {
  const store = writable<WorkspaceSessionState>(initial)

  function patch(next: Partial<WorkspaceSessionState>) {
    store.update((current) => ({ ...current, ...next }))
  }

  function request(): FeedbackRequestSummary | null {
    return get(store).workspace?.request ?? null
  }

  function requestId(): string | null {
    return request()?.request_id ?? null
  }

  function isTerminal(): boolean {
    const status = request()?.status
    return status === 'completed' || status === 'cancelled'
  }

  /** The submitted or in-progress feedback result shown in the workbench. */
  function feedbackResult() {
    const state = get(store)
    return state.completedResult?.feedback ?? state.workspace?.feedback ?? null
  }

  /** True while any server mutation against the open request is in flight. */
  function interactionLocked(): boolean {
    const state = get(store)
    return state.submitting || state.cancelling || state.approving
  }

  function open(
    workspace: FeedbackWorkspaceView,
    publishedFeedback: PublishedFeedbackView | null = null,
  ) {
    patch({ workspace, completedResult: null, publishedFeedback })
  }

  /** Applies a refreshed or mutated projection without clearing submission state. */
  function replace(workspace: FeedbackWorkspaceView) {
    patch({ workspace })
  }

  function setDraft(draft: FeedbackWorkspaceView['draft']) {
    const state = get(store)
    if (state.workspace) patch({ workspace: { ...state.workspace, draft } })
  }

  function setLoading(loadingWorkspace: boolean) {
    patch({ loadingWorkspace })
  }

  function close() {
    store.set(initial)
  }

  /**
   * Folds a terminal server result into the open request. Returns false when the
   * result belongs to a request the workbench no longer shows.
   */
  function applyMutationResult(result: FeedbackRequestView): boolean {
    const state = get(store)
    if (state.workspace?.request.request_id !== result.request_id) return false
    patch({
      completedResult: result,
      workspace: {
        ...state.workspace,
        feedback: result.feedback,
        request: {
          ...state.workspace.request,
          status: result.status,
          resolution: result.resolution,
          updated_at: result.updated_at,
        },
      },
    })
    return true
  }

  function setCompleted(result: FeedbackRequestView | null) {
    patch({ completedResult: result })
  }

  function setPublished(feedback: PublishedFeedbackView | null) {
    patch({ publishedFeedback: feedback })
  }

  function setSubmitting(submitting: boolean) {
    patch({ submitting })
  }

  function setSubmitStage(submitStage: SubmitStage) {
    patch({ submitStage })
  }

  function beginApprove() {
    patch({ approving: true })
  }

  function endApprove() {
    patch({ approving: false })
  }

  function beginCancel() {
    patch({ cancelling: true })
  }

  function endCancel() {
    patch({ cancelling: false })
  }

  return {
    subscribe: store.subscribe,
    request,
    requestId,
    isTerminal,
    feedbackResult,
    interactionLocked,
    open,
    replace,
    setDraft,
    setLoading,
    close,
    applyMutationResult,
    setCompleted,
    setPublished,
    setSubmitting,
    setSubmitStage,
    beginApprove,
    endApprove,
    beginCancel,
    endCancel,
  }
}
