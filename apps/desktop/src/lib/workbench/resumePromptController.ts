import { get, writable } from 'svelte/store'

import {
  defineApplicationStream,
  type ApplicationTransport,
  type Unsubscribe,
} from '../application/applicationTransport'
import { readApplicationSnapshot } from '../application/readApplicationSnapshot'
import type { HostProfile } from '../domain/hostProfile'
import type { ResumePrompt } from '../domain/resumePrompt'
import type { FeedbackRequestSummary, FeedbackWorkspaceView } from '../feedback'
import type { NotificationState } from '../notifications'
import { buildResumePrompt } from './resumePrompt'

export const RESUME_PROMPT_STREAM = defineApplicationStream<ResumePrompt>(
  'rambledesk://resume-prompt',
)

export type ResumePromptControllerState = Readonly<{
  prompt: ResumePrompt | null
  copyState: 'idle' | 'copied' | 'failed'
}>

export type ResumePromptControllerContext = {
  transport: ApplicationTransport
  tr: (source: string, values?: Record<string, string | number>) => string
  messageFrom: (cause: unknown) => string
  getCurrentRequest: () => FeedbackRequestSummary | null
  getKnownRequests: () => readonly FeedbackRequestSummary[]
  getWorkspace: () => FeedbackWorkspaceView | null
  resolveHostProfile: (hostId: string) => HostProfile
  canOpenFromWorkspace: () => boolean
  notifications: {
    available: boolean
    isMac: boolean
    getPopupEnabled: () => boolean
    getState: () => NotificationState
    send: (input: { title: string; body: string }) => Promise<void>
  }
  setPageError: (message: string) => void
  /** Injectable for tests; defaults to the system clipboard. */
  copyText?: (text: string) => Promise<void>
}

const COPY_RESET_MS = 2_000

/**
 * The resume prompt shown after a submission: prompts arriving from the
 * application stream, the manual reopen action, and the copy feedback.
 */
export function createResumePromptController(context: ResumePromptControllerContext) {
  const store = writable<ResumePromptControllerState>({ prompt: null, copyState: 'idle' })
  let mounted = true
  let generation = 0
  let copyResetTimer: ReturnType<typeof setTimeout> | undefined

  function patch(next: Partial<ResumePromptControllerState>) {
    store.update((current) => ({ ...current, ...next }))
  }

  async function present(prompt: ResumePrompt) {
    const currentGeneration = ++generation
    const isCurrent = () => mounted && generation === currentGeneration
    try {
      const current = context.getCurrentRequest()
      const known =
        current?.request_id === prompt.request_id
          ? current
          : context
              .getKnownRequests()
              .find((request) => request.request_id === prompt.request_id)
      const request =
        known ??
        (
          await readApplicationSnapshot(context.transport, 'getFeedbackWorkspace', {
            request_id: prompt.request_id,
          })
        ).request
      if (!isCurrent()) return
      // Managed sessions have their own resume flow; the dialog is for host sessions.
      if (request.managed_session_id) return
      patch({ prompt, copyState: 'idle' })
      if (
        context.notifications.available &&
        context.notifications.isMac &&
        context.notifications.getPopupEnabled() &&
        context.notifications.getState() === 'enabled'
      ) {
        void context.notifications
          .send({
            title: prompt.title,
            body: context.tr(
              'Return to {host} and use the resume prompt to continue the host session.',
              { host: prompt.host_label },
            ),
          })
          .catch(() => {})
      }
      // The alert sound is reserved for a new request arriving, not for the
      // resume prompt shown after a submission completes.
    } catch (cause) {
      if (isCurrent()) context.setPageError(context.messageFrom(cause))
    }
  }

  function subscribeStream(): Unsubscribe {
    return context.transport.subscribe(
      RESUME_PROMPT_STREAM,
      (prompt) => void present(prompt),
      () => {
        // The manual reopen action remains available for external sessions.
      },
    )
  }

  /** Shows a prompt directly, used by the preview fixtures. */
  function show(prompt: ResumePrompt) {
    patch({ prompt, copyState: 'idle' })
  }

  function open() {
    const workspace = context.getWorkspace()
    if (!workspace || !context.canOpenFromWorkspace()) return
    patch({
      prompt: buildResumePrompt(
        workspace,
        context.resolveHostProfile(workspace.request.host_id),
        context.tr,
      ),
      copyState: 'idle',
    })
  }

  async function copy() {
    const prompt = get(store).prompt
    if (!prompt) return
    try {
      const write =
        context.copyText ?? ((text: string) => navigator.clipboard.writeText(text))
      await write(prompt.resume_prompt)
      patch({ copyState: 'copied' })
      if (copyResetTimer !== undefined) clearTimeout(copyResetTimer)
      copyResetTimer = setTimeout(() => {
        if (get(store).copyState === 'copied') patch({ copyState: 'idle' })
      }, COPY_RESET_MS)
    } catch {
      patch({ copyState: 'failed' })
    }
  }

  function dismiss() {
    patch({ prompt: null, copyState: 'idle' })
  }

  function dispose() {
    mounted = false
    generation += 1
    if (copyResetTimer !== undefined) clearTimeout(copyResetTimer)
  }

  return { subscribe: store.subscribe, subscribeStream, show, open, copy, dismiss, dispose }
}

export type ResumePromptController = ReturnType<typeof createResumePromptController>
