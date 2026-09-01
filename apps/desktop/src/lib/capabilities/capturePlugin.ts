/**
 * The client-local origin of bytes offered to the shared Draft flow.
 *
 * A source describes acquisition only. It never identifies a Feedback Request,
 * a Draft revision, or a persistence destination.
 */
export type AttachmentCandidateSource =
  | 'screen-capture'
  | 'clipboard-image'
  | 'file-input'
  | 'image-paste'

/**
 * Client-local bytes produced by a Capture Plugin.
 *
 * The receiver owns a candidate once it is returned or emitted and must dispose
 * it after persistence succeeds or fails. `dispose` is idempotent. A candidate
 * is not a persisted attachment and cannot carry a server or filesystem path.
 */
export type AttachmentCandidate = Readonly<{
  id: string
  source: AttachmentCandidateSource
  fileName: string
  mediaType: string
  byteLength: number
  readBytes: () => Promise<ArrayBuffer>
  dispose: () => Promise<void>
}>

export type AttachmentCandidateDefinition = Omit<AttachmentCandidate, 'dispose'> &
  Readonly<{ dispose: () => Promise<void> }>

/** Defines immutable candidate metadata and guarantees one disposal operation. */
export function defineAttachmentCandidate(
  definition: AttachmentCandidateDefinition,
): AttachmentCandidate {
  let disposal: Promise<void> | null = null
  return Object.freeze({
    id: definition.id,
    source: definition.source,
    fileName: definition.fileName,
    mediaType: definition.mediaType,
    byteLength: definition.byteLength,
    readBytes: definition.readBytes,
    dispose: () => {
      disposal ??= Promise.resolve().then(definition.dispose)
      return disposal
    },
  })
}

export type CapturePluginUnsubscribe = () => void
export type CapturePluginErrorHandler = (cause: unknown) => void

export type ScreenCaptureFinished = Readonly<{
  candidateId: string | null
  outcome: 'cancelled' | 'pinned'
}>

/** Native screen acquisition UX that emits client-local candidates only. */
export interface ScreenCapturePlugin {
  onCandidate(
    handler: (candidate: AttachmentCandidate) => void,
    onError: CapturePluginErrorHandler,
  ): CapturePluginUnsubscribe
  onFinished(
    handler: (result: ScreenCaptureFinished) => void,
    onError: CapturePluginErrorHandler,
  ): CapturePluginUnsubscribe
  onShortcut(
    handler: () => void,
    onError: CapturePluginErrorHandler,
  ): CapturePluginUnsubscribe
  begin(): Promise<void>
}

export type ClipboardCaptureResult =
  | Readonly<{
      kind: 'text'
      text: string
      capturedAtMs: number
      truncated: boolean
    }>
  | Readonly<{
      kind: 'attachment'
      candidate: AttachmentCandidate
      capturedAtMs: number
    }>

/** One user-initiated read of the current device clipboard. */
export interface ClipboardCapturePlugin {
  captureOnce(): Promise<ClipboardCaptureResult>
}

/** DOM-scoped image paste acquisition; ordinary text paste remains untouched. */
export interface ImagePastePlugin {
  subscribe(
    target: EventTarget,
    handler: (candidates: readonly AttachmentCandidate[]) => boolean,
    onError: CapturePluginErrorHandler,
  ): CapturePluginUnsubscribe
}

export type CapturePlugins = Readonly<{
  screenCapture: ScreenCapturePlugin
  clipboardCapture: ClipboardCapturePlugin
  imagePaste: ImagePastePlugin
}>
