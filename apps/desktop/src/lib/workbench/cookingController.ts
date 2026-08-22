// Cooking workflow orchestration for the workbench. The controller owns the
// cooking flow (preview into the editor, cook-and-publish on submit) while
// reactive state stays in the component: every mutation goes through the
// context callbacks, so Svelte re-renders exactly as before.

import type { SubmitFeedbackInput, FeedbackWorkspaceView } from '../feedback'
import type { CookingConfig } from '../cooking'
import type { SavePhase } from './types'

export type CookingSubmission = {
  request: FeedbackWorkspaceView['request']
  actions: FeedbackWorkspaceView['actions']
  body: string
  savedRevision: number
}

export type CookedPreview = { markdown: string; original: string; model: string }

type CookingControllerContext = {
  tr: (source: string, values?: Record<string, string | number>) => string
  messageFrom: (cause: unknown) => string
  getWorkspace: () => FeedbackWorkspaceView | null
  getDraftBody: () => string
  getSavedBody: () => string
  getCookingConfig: () => CookingConfig
  isCookingEnabled: () => boolean
  isCooking: () => boolean
  exitRamble: () => Promise<void>
  setDraftBody: (markdown: string) => void
  setSavePhase: (phase: SavePhase) => void
  setSaveMessage: (message: string) => void
  saveDraftNow: () => Promise<boolean>
  applyEditorMarkdown: (markdown: string) => void
  setPageError: (message: string) => void
  setCooking: (requestId: string, cooking: boolean) => void
  publishCooked: (
    input: SubmitFeedbackInput,
    cookedMarkdown: string | undefined,
    uncookedMarkdown: string,
  ) => Promise<void>
  setPreview: (preview: CookedPreview | null) => void
  setPreviewOriginal: (original: string) => void
  getPreviewOriginal: () => string
}

export type CookingController = ReturnType<typeof createCookingController>

export function createCookingController(context: CookingControllerContext) {
  async function cookBody(input: {
    title: string
    whatHappened: string
    actions: FeedbackWorkspaceView['actions']
    uncookedMarkdown: string
  }): Promise<{ markdown: string; model: string }> {
    const { cookFeedback } = await import('../cooking')
    return cookFeedback(input, context.getCookingConfig())
  }

  /** Cook the current draft into the editor without publishing. */
  async function cookPreviewOnly() {
    const workspace = context.getWorkspace()
    if (
      !workspace ||
      !context.isCookingEnabled() ||
      context.isCooking() ||
      workspace.request.status === 'completed' ||
      workspace.request.status === 'cancelled'
    ) return
    await context.exitRamble()

    if (!(await context.saveDraftNow())) return
    if (context.getWorkspace() === null || context.isCooking()) return

    const requestId = workspace.request.request_id
    const original = context.getDraftBody()
    if (!original.trim()) return

    context.setCooking(requestId, true)
    context.setPageError('')
    try {
      const cooked = await cookBody({
        title: workspace.request.title,
        whatHappened: workspace.request.what_happened,
        actions: workspace.actions,
        uncookedMarkdown: original,
      })
      if (context.getWorkspace()?.request.request_id !== requestId) return
      context.setPreview({ markdown: cooked.markdown, original, model: cooked.model })
      context.setPreviewOriginal(original)
      context.setDraftBody(cooked.markdown)
      // Drive the editor instance directly in addition to the reactive prop
      // chain, so the cooked text is visible even if a prop update is lost.
      context.applyEditorMarkdown(cooked.markdown)
      context.setSavePhase(
        context.getDraftBody() === context.getSavedBody() ? 'saved' : 'unsaved',
      )
      context.setSaveMessage('')
      void context.saveDraftNow()
    } catch (cause) {
      context.setPageError(context.messageFrom(cause))
    } finally {
      context.setCooking(requestId, false)
    }
  }

  /** Discard the cooked preview and restore the pre-cook draft. */
  function restoreOriginal() {
    const original = context.getPreviewOriginal()
    if (!original || context.getWorkspace() === null) return
    context.setPreview(null)
    context.setDraftBody(original)
    context.applyEditorMarkdown(original)
    context.setSavePhase(original === context.getSavedBody() ? 'saved' : 'unsaved')
    context.setSaveMessage('')
    void context.saveDraftNow()
  }

  /** Cook the submission body and publish it (the one-click path). */
  async function cookAndPublish(submission: CookingSubmission) {
    try {
      const cooked = await cookBody({
        title: submission.request.title,
        whatHappened: submission.request.what_happened,
        actions: submission.actions,
        uncookedMarkdown: submission.body,
      })
      await context.publishCooked(
        {
          request_id: submission.request.request_id,
          expected_revision: submission.savedRevision,
          cooked_markdown: cooked.markdown,
          cooking_model: cooked.model,
          uncooked_markdown: submission.body,
        },
        cooked.markdown,
        submission.body,
      )
    } catch (cause) {
      context.setPageError(context.messageFrom(cause))
    } finally {
      context.setCooking(submission.request.request_id, false)
    }
  }

  return { cookPreviewOnly, restoreOriginal, cookAndPublish }
}
