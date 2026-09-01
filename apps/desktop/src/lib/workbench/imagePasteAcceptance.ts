import type { FeedbackStatus } from '../feedback'

export type ImagePasteAcceptanceState = Readonly<{
  loadingWorkspace: boolean
  requestStatus: FeedbackStatus | null
  interactionLocked: boolean
  attachmentBusy: boolean
}>

export function canAcceptImagePaste(state: ImagePasteAcceptanceState): boolean {
  return !state.loadingWorkspace
    && state.requestStatus !== null
    && state.requestStatus !== 'completed'
    && state.requestStatus !== 'cancelled'
    && !state.interactionLocked
    && !state.attachmentBusy
}
