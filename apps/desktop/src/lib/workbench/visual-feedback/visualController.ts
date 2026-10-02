import type { VisualFeedbackData } from '../../generated/feedback'
import type { WorkbenchController, WorkbenchControllerContext } from '../definitions/contracts'
import { validVisualFeedbackInput } from '../definitions/visual_feedback/input'
import { text } from '../definitions/validation'
import { fieldFingerprint } from '../fields/value'
import { annotationsKey, emptyVisualState, validVisualState } from './visualModel'
import { exportVisualPng, loadVisualImage } from './visualExport'

export type VisualWorkbenchController = WorkbenchController & {
  loadBackground: (data: VisualFeedbackData) => Promise<Blob | null>
}
export function createVisualWorkbenchController(context: WorkbenchControllerContext,
  dependencies = { exportPng: exportVisualPng, decodeImage: loadVisualImage }): VisualWorkbenchController {
  let disposed = false, pending: Promise<void> | null = null
  const backgrounds = new Map<string, Promise<Blob | null>>()
  let prepared: { key: string; attachmentId: string } | null = null
  const stateNow = () => { const state = context.getState(); return state?.type === 'visual_feedback' ? state : emptyVisualState() }
  async function workspace() {
    return context.getWorkspace?.() ?? await context.runtime.transport.call('getFeedbackWorkspace', { request_id: context.requestId })
  }
  async function loadBackground(data: VisualFeedbackData): Promise<Blob | null> {
    if (data.image_file_name === null) return null
    const key = fieldFingerprint(data)
    let loading = backgrounds.get(key)
    if (!loading) {
      loading = (async () => {
        const current = await workspace()
        if (disposed || current.request.request_id !== context.requestId
          || current.workbench?.type !== 'visual_feedback' || fieldFingerprint(current.workbench.data) !== key) throw new Error('The canvas is no longer available.')
        const matches = current.request_attachments.filter((item) => item.file_name === data.image_file_name)
        if (matches.length !== 1 || !matches[0].media_type.startsWith('image/')) throw new Error('The original image is unavailable.')
        const bytes = await context.runtime.transport.call('readRequestAttachment', { request_id: context.requestId, attachment_id: matches[0].attachment_id })
        if (disposed) throw new Error('The canvas is no longer available.')
        const blob = new Blob([bytes], { type: matches[0].media_type })
        const image = await dependencies.decodeImage(blob)
        if (image.naturalWidth !== data.width || image.naturalHeight !== data.height) throw new Error('The original image dimensions do not match the canvas.')
        if (disposed) throw new Error('The canvas is no longer available.')
        return blob
      })()
      backgrounds.set(key, loading)
      void loading.catch(() => { if (backgrounds.get(key) === loading) backgrounds.delete(key) })
    }
    return loading
  }
  async function prepare() {
    if (disposed || !context.isEditable()) throw new Error('The canvas is no longer editable.')
    const current = await workspace(), spec = current.workbench
    if (current.request.request_id !== context.requestId || spec?.type !== 'visual_feedback' || spec.version !== 1 || !validVisualFeedbackInput(spec.data)) throw new Error('The canvas is invalid.')
    const data = spec.data as VisualFeedbackData
    const state = stateNow()
    if (!validVisualState(data, state) || state.annotations.some((mark) => mark.kind === 'text' && !text(mark.text, 2000))) throw new Error('The visual annotations are invalid.')
    const key = fieldFingerprint([data, annotationsKey(state.annotations)])
    if (prepared?.key === key && state.composite_attachment_id === prepared.attachmentId
      && current.attachments.some((item) => item.attachment_id === prepared!.attachmentId)) return
    const persist = context.runtime.persistGeneratedAttachment
    if (!persist) throw new Error('Saving a generated image is unavailable in this client.')
    const unchanged = () => {
      const latest = context.getWorkspace?.()
      return !disposed && context.isEditable() && (!latest || (latest.request.request_id === context.requestId
        && latest.workbench?.type === 'visual_feedback' && fieldFingerprint(latest.workbench.data) === fieldFingerprint(data)))
        && annotationsKey(stateNow().annotations) === annotationsKey(state.annotations)
    }
    const png = await dependencies.exportPng(data, state.annotations, await loadBackground(data), current.attachments)
    if (!unchanged()) throw new Error('The annotations changed while preparing feedback. Try again.')
    const attachment = await persist({ fileName: 'visual-feedback.png', contents: png })
    if (!unchanged()) throw new Error('The annotations changed while preparing feedback. Try again.')
    context.updateState({ ...state, composite_attachment_id: attachment.attachment_id })
    prepared = { key, attachmentId: attachment.attachment_id }
  }
  return {
    loadBackground,
    prepareSubmission(intent = 'submit') {
      if (intent === 'cancel') return Promise.resolve()
      if (pending) return pending
      context.setBusy(true)
      pending = prepare().finally(() => { pending = null; context.setBusy(false) })
      return pending
    },
    dispose() { disposed = true; backgrounds.clear(); context.setBusy(false) },
  }
}
