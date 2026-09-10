import { derived, get, writable } from 'svelte/store'

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
type WorkspaceSessionFacts = Readonly<{
  workspace: FeedbackWorkspaceView | null
  completedResult: FeedbackRequestView | null
  publishedFeedback: PublishedFeedbackView | null
  loadingWorkspace: boolean
  submitStage: SubmitStage
  approving: boolean
  cancelling: boolean
}>

export type WorkspaceSessionState = WorkspaceSessionFacts & Readonly<{
  submitting: boolean
  request: FeedbackRequestSummary | null
  terminal: boolean
  feedbackResult: FeedbackRequestView['feedback']
  interactionLocked: boolean
}>

function project(facts: WorkspaceSessionFacts): WorkspaceSessionState {
  const request = facts.workspace?.request ?? null
  const submitting = facts.submitStage !== 'idle'
  return {
    ...facts,
    submitting,
    request,
    terminal: request?.status === 'completed' || request?.status === 'cancelled',
    feedbackResult: facts.completedResult?.feedback ?? facts.workspace?.feedback ?? null,
    interactionLocked: submitting || facts.cancelling || facts.approving,
  }
}

const initial: WorkspaceSessionFacts = {
  workspace: null,
  completedResult: null,
  publishedFeedback: null,
  loadingWorkspace: false,
  submitStage: 'idle',
  approving: false,
  cancelling: false,
}

export type WorkspaceSession = ReturnType<typeof createWorkspaceSession>

export function createWorkspaceSession() {
  const store = writable<WorkspaceSessionFacts>(initial)
  // UI and imperative callers observe the same projection. A normal getter that
  // calls get(store) cannot establish a Svelte component's reactive dependency.
  const state = derived(store, project)

  function patch(next: Partial<WorkspaceSessionFacts>) {
    store.update((current) => ({ ...current, ...next }))
  }

  function request(): FeedbackRequestSummary | null {
    return get(state).request
  }

  function requestId(): string | null {
    return request()?.request_id ?? null
  }

  function isTerminal(): boolean {
    return get(state).terminal
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
          allow_finish: result.allow_finish,
          final_summary: result.final_summary,
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

  /** Publish the operation and its UI stage together, including preparation. */
  function setSubmissionStage(submitStage: SubmitStage) {
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
    subscribe: state.subscribe,
    request,
    requestId,
    isTerminal,
    open,
    replace,
    setDraft,
    setLoading,
    close,
    applyMutationResult,
    setCompleted,
    setPublished,
    setSubmissionStage,
    beginApprove,
    endApprove,
    beginCancel,
    endCancel,
  }
}
