import type { MediaReviewData } from '../../generated/feedback'
import type { WorkbenchController, WorkbenchControllerContext } from '../definitions/contracts'
import { validMediaReviewInput } from '../definitions/media_review/input'
import { readMediaReviewState } from '../definitions/media_review/state'
import { fieldFingerprint } from '../fields/value'
import { validateMediaReviewState } from './mediaModel'

export type MediaReviewSource = { url: string; mediaType: string }
export type MediaReviewController = WorkbenchController & {
  loadSource: (data: MediaReviewData) => Promise<MediaReviewSource>
  bindPlayer: (element: HTMLMediaElement) => { active: () => boolean; release: () => void }
  position: () => number
}
export function createMediaReviewController(context: WorkbenchControllerContext,
  urls = { create: (blob: Blob) => URL.createObjectURL(blob), revoke: (url: string) => URL.revokeObjectURL(url) }): MediaReviewController {
  let disposed = false, cached: { key: string; promise: Promise<MediaReviewSource> } | null = null
  let ownedUrl: string | null = null, player: HTMLMediaElement | null = null, positionMs = 0
  const clearPlayer = (element: HTMLMediaElement) => {
    if (player === element && Number.isFinite(element.currentTime)) positionMs = Math.max(0, Math.round(element.currentTime * 1000))
    element.pause(); element.removeAttribute('src'); element.load()
  }
  return {
    position: () => positionMs,
    bindPlayer(element) {
      if (disposed) throw new Error('The media is no longer available.')
      if (player && player !== element) clearPlayer(player)
      player = element
      return { active: () => !disposed && player === element, release: () => { clearPlayer(element); if (player === element) player = null } }
    },
    loadSource(data) {
      if (disposed || !validMediaReviewInput(data as unknown as Record<string, unknown>)) return Promise.reject(new Error('The media is no longer available.'))
      const key = fieldFingerprint(data)
      if (cached?.key === key) return cached.promise
      if (cached) return Promise.reject(new Error('The original media has changed. Select the comment again.'))
      const promise = (async () => {
        const workspace = context.getWorkspace?.() ?? await context.runtime.transport.call('getFeedbackWorkspace', { request_id: context.requestId })
        if (disposed || workspace.request.request_id !== context.requestId || workspace.workbench?.type !== 'media_review' || workspace.workbench.version !== 1
          || fieldFingerprint(workspace.workbench.data) !== key) throw new Error('The media is no longer available.')
        const matches = workspace.request_attachments.filter((attachment) => attachment.file_name === data.media_file_name)
        if (matches.length !== 1 || !matches[0].media_type.startsWith(`${data.media_kind}/`)) throw new Error('The original media attachment is unavailable.')
        const bytes = await context.runtime.transport.call('readRequestAttachment', { request_id: context.requestId, attachment_id: matches[0].attachment_id })
        if (disposed) throw new Error('The media is no longer available.')
        if (!bytes.byteLength || bytes.byteLength > 20 * 1024 * 1024) throw new Error('Media must be between 1 byte and 20 MiB.')
        ownedUrl = urls.create(new Blob([bytes], { type: matches[0].media_type }))
        return { url: ownedUrl, mediaType: matches[0].media_type }
      })()
      cached = { key, promise }
      void promise.catch(() => { if (cached?.promise === promise) cached = null })
      return promise
    },
    async prepareSubmission(intent = 'submit') {
      player?.pause()
      if (intent === 'cancel') return
      if (disposed || !context.isEditable()) throw new Error('The media is no longer editable.')
      const workspace = context.getWorkspace?.() ?? await context.runtime.transport.call('getFeedbackWorkspace', { request_id: context.requestId })
      const spec = workspace.workbench, value = context.getState()
      if (workspace.request.request_id !== context.requestId || spec?.type !== 'media_review' || spec.version !== 1 || !validMediaReviewInput(spec.data)) throw new Error('This media review is unavailable.')
      const state = value === null ? null : readMediaReviewState(value)
      const issue = value !== null && !state ? 'This media review draft is invalid.' : validateMediaReviewState(spec.data as MediaReviewData, state)
      if (issue) throw new Error(issue)
    },
    dispose() {
      if (disposed) return
      disposed = true
      if (player) { clearPlayer(player); player = null }
      if (ownedUrl) { urls.revoke(ownedUrl); ownedUrl = null }
      cached = null
    },
  }
}
