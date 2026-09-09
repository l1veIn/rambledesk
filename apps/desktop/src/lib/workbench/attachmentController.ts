import { tick } from 'svelte'

import type { ApplicationTransport } from '../application/applicationTransport'
import { readApplicationSnapshot } from '../application/readApplicationSnapshot'
import type { ApplicationAddAttachmentInput } from '../application/contracts'
import type { WorkbenchCapabilities } from '../capabilities/workbenchCapabilities'
import type { AttachmentCandidate } from '../capabilities/capturePlugin'
import { fileAttachmentCandidate } from '../capabilities/fileAttachmentCandidate'
import type {
  AttachmentView,
  FeedbackWorkspaceView,
  RemoveAttachmentInput,
  ReorderAttachmentsInput,
} from '../feedback'
import type { ActiveAction } from '../draftOperations'
import type { FeedbackEditorHandle } from '../editor/feedbackEditorHandle'
import type { AttachmentSession } from './attachmentSession'
import { createAttachmentPreviews } from './attachmentPreviews'

export type { AttachmentMessageTone } from './attachmentSession'

type AttachmentControllerContext = {
  capabilities: Pick<WorkbenchCapabilities, 'screenCapture' | 'serverPaths' | 'windowControls'>
  transport: ApplicationTransport
  tr: (source: string, values?: Record<string, string | number>) => string
  messageFrom: (cause: unknown) => string
  getWorkspace: () => FeedbackWorkspaceView | null
  getEditor: () => FeedbackEditorHandle | undefined
  getRambleRequestId: () => string
  getInteractionLocked: () => boolean
  getSavedRevision: () => number
  /** Runtime capture/import state shared with the workbench view. */
  session: AttachmentSession
  saveDraftNow: () => Promise<boolean>
  waitForRambleMarkdown: () => Promise<void>
  routeDraftOperation: (requestId: string, operation: import('../draftOperations').DraftOperation) => Promise<void>
  activeActionFor: (requestId: string) => import('../draftOperations').ActiveAction
  applyWorkspaceMutation: (next: FeedbackWorkspaceView) => void
  recordAttachmentDiagnostic?: (
    activity: AttachmentImportDiagnostic,
    requestId: string,
  ) => Promise<void>
}

export type AttachmentController = ReturnType<typeof createAttachmentController>

export type AttachmentCandidateTarget = Readonly<{
  requestId: string
  action: ActiveAction
  label?: string
}>

type AttachmentImportDiagnostic = 'screen_capture_imported' | 'clipboard_image_imported'

function attachmentDiagnosticActivity(
  candidate: AttachmentCandidate,
): AttachmentImportDiagnostic | null {
  if (candidate.source === 'screen-capture') return 'screen_capture_imported'
  if (candidate.source === 'clipboard-image') return 'clipboard_image_imported'
  return null
}

export function createAttachmentController(context: AttachmentControllerContext) {
  let screenCaptureTarget: AttachmentCandidateTarget | null = null
  let candidateImportPending = false
  let attachmentOperationQueue: Promise<void> = Promise.resolve()
  let attachmentOperationsPending = 0
  let lifecycleGeneration = 0
  let disposed = false
  const previewResources = createAttachmentPreviews({
    transport: context.transport,
    getRequestId: () => context.getWorkspace()?.request.request_id,
    publish: context.session.setPreviews,
  })

  function mount() {
    disposed = false
    let dragUnlisten: (() => void) | undefined
    let captureReadyUnlisten: (() => void) | undefined
    let captureFinishedUnlisten: (() => void) | undefined

    const screenCapture = context.capabilities.screenCapture
    if (screenCapture.status.availability !== 'unavailable') {
      captureReadyUnlisten = screenCapture.implementation.onCandidate(
        (candidate) => void importScreenCapture(candidate),
        (cause) => {
          if (disposed) return
          context.session.setMessage(
            context.tr('Cannot receive the capture result: {error}', { error: context.messageFrom(cause) }),
            'error',
          )
        },
      )
      captureFinishedUnlisten = screenCapture.implementation.onFinished(
        () => {
          if (disposed) return
          screenCaptureTarget = null
          context.session.setCaptureBusy(false)
          context.session.setMessage('')
        },
        () => {
          // A failed cancellation listener does not affect capture or attachment storage.
        },
      )
    }

    const serverPaths = context.capabilities.serverPaths
    if (serverPaths.status.availability !== 'unavailable') {
      dragUnlisten = serverPaths.implementation.onFileDrop(
        (event) => {
          if (disposed) return
          context.session.setDragActive(event.type === 'enter' || event.type === 'over')
          if (event.type === 'drop') {
            context.session.setDragActive(false)
            void importServerAttachmentPaths(event.paths)
          } else if (event.type === 'leave') {
            context.session.setDragActive(false)
          }
        },
        () => {
          if (disposed) return
          context.session.setMessage(
            context.tr('File drop is unavailable in this window. Use the file picker or paste instead.'),
            'error',
          )
        },
      )
    }

    return () => {
      disposed = true
      lifecycleGeneration += 1
      dragUnlisten?.()
      captureReadyUnlisten?.()
      captureFinishedUnlisten?.()
      screenCaptureTarget = null
      context.session.setBusy(false)
      context.session.setCaptureBusy(false)
      context.session.setDragActive(false)
      releasePreviews()
    }
  }

  function canImportCandidates(candidates: readonly AttachmentCandidate[]): boolean {
    const workspace = context.getWorkspace()
    return candidates.length > 0
      && !disposed
      && !candidateImportPending
      && !context.getInteractionLocked()
      && !context.session.busy()
      && workspace !== null
      && workspace.request.status !== 'completed'
      && workspace.request.status !== 'cancelled'
  }

  function acceptAttachmentCandidates(candidates: readonly AttachmentCandidate[]): boolean {
    if (!canImportCandidates(candidates)) {
      void disposeCandidates(candidates)
      return false
    }
    candidateImportPending = true
    void importAttachmentCandidates(candidates).finally(() => {
      candidateImportPending = false
    })
    return true
  }

  function handleFileSelection(event: Event) {
    const input = event.currentTarget as HTMLInputElement
    const candidates = Array.from(input.files ?? [], fileInputCandidate)
    input.value = ''
    acceptAttachmentCandidates(candidates)
  }

  async function importAttachmentCandidates(candidates: readonly AttachmentCandidate[]) {
    const workspace = context.getWorkspace()
    if (
      disposed || context.getInteractionLocked()
      || !workspace
      || workspace.request.status === 'completed'
      || workspace.request.status === 'cancelled'
      || candidates.length === 0
      || context.session.busy()
    ) {
      await disposeCandidates(candidates)
      return
    }
    const target = {
      requestId: workspace.request.request_id,
      action: context.activeActionFor(workspace.request.request_id),
    }
    await persistAttachmentCandidates(target, candidates)
  }

  function fileInputCandidate(file: File): AttachmentCandidate {
    return fileAttachmentCandidate(file, 'file-input')
  }

  async function disposeCandidates(candidates: readonly AttachmentCandidate[]) {
    await Promise.allSettled(candidates.map((candidate) => candidate.dispose()))
  }

  /**
   * The only shared Attachment Candidate persistence path. Target identity is
   * supplied by the acquisition caller, so later workspace or Action changes
   * cannot retarget the bytes. All callers share one CAS queue.
   */
  function persistAttachmentCandidates(
    target: AttachmentCandidateTarget,
    candidates: readonly AttachmentCandidate[],
  ): Promise<boolean> {
    return queueAttachmentImports(target, candidates.map((candidate) => async (next, active) => {
      if (candidate.byteLength > 20 * 1024 * 1024) {
        throw new Error(context.tr('{name} exceeds the 20 MiB limit', { name: candidate.fileName }))
      }
      const contents = await candidate.readBytes()
      if (!active()) throw new Error('Attachment operation was disposed')
      const input: ApplicationAddAttachmentInput = {
        request_id: target.requestId,
        file_name: candidate.fileName || `attachment-${Date.now()}`,
        contents,
        expected_revision: next.draft.saved_revision,
      }
      const result = await context.transport.call('addFeedbackAttachment', input)
      const activity = attachmentDiagnosticActivity(candidate)
      if (active() && activity && context.recordAttachmentDiagnostic) {
        await context.recordAttachmentDiagnostic(activity, target.requestId).catch(() => {})
      }
      return result
    }), () => disposeCandidates(candidates))
  }

  function isTerminal(workspace: FeedbackWorkspaceView) {
    return workspace.request.status === 'completed' || workspace.request.status === 'cancelled'
  }

  async function readTarget(requestId: string) {
    const workspace = await readApplicationSnapshot(context.transport, 'getFeedbackWorkspace', { request_id: requestId })
    if (!workspace) throw new Error(context.tr('This feedback request could not be found.'))
    return workspace
  }

  async function showWorkspaceMutation(next: FeedbackWorkspaceView) {
    const current = context.getWorkspace()
    if (disposed || current?.request.request_id !== next.request.request_id) return
    // A completed projection or a newer saved revision may have won while this response decoded.
    if ((isTerminal(current) && !isTerminal(next)) || current.draft.saved_revision > next.draft.saved_revision) return
    context.applyWorkspaceMutation(next)
    // Preview reads have their own lifetime and must not delay insertion or its save receipt.
    void refreshPreviews(next)
    await tick()
  }

  async function reconcileMutationFailure(requestId: string, active: () => boolean) {
    try {
      const current = await readTarget(requestId)
      if (active()) await showWorkspaceMutation(current)
    } catch {
      // Retain the original operation error; an unavailable refresh must not trigger a mutation retry.
    }
  }

  function queueAttachmentImports(
    target: AttachmentCandidateTarget,
    imports: ReadonlyArray<(next: FeedbackWorkspaceView, active: () => boolean) => Promise<FeedbackWorkspaceView>>,
    cleanup: () => Promise<void> = async () => {},
  ): Promise<boolean> {
    if (imports.length === 0) return cleanup().then(() => !disposed)
    return queueAttachmentWork((active) => persistAttachmentBatch(target, imports, active), cleanup)
  }

  function queueAttachmentWork(
    work: (active: () => boolean) => Promise<boolean>,
    cleanup: () => Promise<void> = async () => {},
  ): Promise<boolean> {
    const generation = lifecycleGeneration
    const active = () => !disposed && generation === lifecycleGeneration
    if (!active()) return cleanup().then(() => false)
    attachmentOperationsPending += 1
    context.session.setBusy(true)
    const run = attachmentOperationQueue.then(async () => {
      try {
        return active() ? await work(active) : false
      } finally {
        await cleanup()
      }
    })
    attachmentOperationQueue = run.then(() => undefined, () => undefined)
    return run.finally(() => {
      attachmentOperationsPending -= 1
      if (active() && attachmentOperationsPending === 0) context.session.setBusy(false)
    })
  }

  async function persistAttachmentBatch(
    target: AttachmentCandidateTarget,
    imports: ReadonlyArray<(next: FeedbackWorkspaceView, active: () => boolean) => Promise<FeedbackWorkspaceView>>,
    active: () => boolean,
  ): Promise<boolean> {
    if (!active()) return false
    const { requestId, action } = target
    context.session.setMessage('')
    let importPending = false
    try {
      const visibleTarget = context.getWorkspace()?.request.request_id === requestId
      if (visibleTarget && !(await context.saveDraftNow())) return false
      await context.waitForRambleMarkdown()
      if (!active()) return false
      for (const persist of imports) {
        // Each document insertion also saves; refetch before the next upload's CAS.
        const previous = await readTarget(requestId)
        if (!active()) return false
        if (isTerminal(previous)) {
          throw new Error(context.tr('This request is closed. The document is read-only.'))
        }
        const existingIds = new Set(previous.attachments.map((item) => item.attachment_id))
        importPending = true
        const next = await persist(previous, active)
        importPending = false
        if (!active()) return false
        await showWorkspaceMutation(next)
        for (const attachment of next.attachments.filter((item) => !existingIds.has(item.attachment_id))) {
          if (!active()) return false
          await context.routeDraftOperation(requestId, {
            kind: 'appendAttachment', attachment,
            label: target.label ?? attachment.file_name, action,
          })
        }
      }
      return true
    } catch (cause) {
      if (!active()) return false
      if (importPending) await reconcileMutationFailure(requestId, active)
      if (!active()) return false
      context.session.setMessage(context.messageFrom(cause), 'error')
      return false
    }
  }

  function reportClientFileError(cause: unknown) {
    if (!disposed) context.session.setMessage(context.messageFrom(cause), 'error')
  }

  async function importServerAttachmentPaths(paths: readonly string[]) {
    const workspace = context.getWorkspace()
    const requestId = context.getRambleRequestId() || workspace?.request.request_id || ''
    if (disposed || context.getInteractionLocked() || !requestId || paths.length === 0 || context.session.busy()) return
    const action = context.activeActionFor(requestId)
    await queueAttachmentImports({ requestId, action }, paths.map((path) => (next) =>
      context.capabilities.serverPaths.implementation.importAttachmentPath({
        requestId, path, expectedRevision: next.draft.saved_revision,
      }),
    ))
  }

  async function leaveFullscreenIfNeeded() {
    if (context.capabilities.windowControls.status.availability === 'unavailable') return
    try {
      await context.capabilities.windowControls.implementation.leaveFullscreen()
    } catch {
      // The native capture command also leaves fullscreen if this fails.
    }
  }

  async function startScreenCapture() {
    const workspace = context.getWorkspace()
    const requestId = workspace?.request.request_id || context.getRambleRequestId() || ''
    if (
      disposed || context.getInteractionLocked() ||
      !requestId ||
      (workspace && isTerminal(workspace)) ||
      context.session.busy() ||
      context.session.captureBusy()
    ) return
    const generation = lifecycleGeneration
    const active = () => !disposed && generation === lifecycleGeneration
    screenCaptureTarget = {
      requestId,
      action: context.activeActionFor(requestId),
    }
    context.session.setCaptureBusy(true)
    context.session.setMessage('')
    try {
      if (workspace?.request.request_id === requestId && !(await context.saveDraftNow())) {
        screenCaptureTarget = null
        context.session.setCaptureBusy(false)
        return
      }
      await context.waitForRambleMarkdown()
      if (!active()) return
      await leaveFullscreenIfNeeded()
      if (!active()) return
      await context.capabilities.screenCapture.implementation.begin()
    } catch (cause) {
      if (!active()) return
      screenCaptureTarget = null
      context.session.setCaptureBusy(false)
      const message = context.messageFrom(cause)
      if (message.includes('SCREEN_CAPTURE_PERMISSION_RESTART_REQUIRED')) {
        context.session.setMessage(
          context.tr('Permission granted. Restart RambleDesk to enable screen capture.'),
          'info',
        )
        try {
          await context.capabilities.windowControls.implementation.restart()
        } catch (restartCause) {
          context.session.setMessage(context.messageFrom(restartCause), 'error')
        }
        return
      }
      context.session.setMessage(
        message === 'Built-in region capture is currently available only in Windows development builds.'
          ? context.tr(message)
          : message,
        'error',
      )
    }
  }

  async function importScreenCapture(candidate: AttachmentCandidate) {
    if (disposed) {
      await candidate.dispose().catch(() => {})
      return
    }
    if (context.getInteractionLocked()) {
      await candidate.dispose().catch(() => {})
      context.session.setCaptureBusy(false)
      return
    }
    const target = screenCaptureTarget
    if (!target) {
      await candidate.dispose().catch(() => {})
      context.session.setCaptureBusy(false)
      return
    }
    context.session.setCaptureBusy(true)
    const generation = lifecycleGeneration
    const active = () => !disposed && generation === lifecycleGeneration
    try {
      const persisted = await persistAttachmentCandidates(target, [candidate])
      if (persisted && active()) {
        context.session.setMessage(
          context.tr('Capture inserted at the current document position'),
          'success',
        )
      }
    } finally {
      if (active()) {
        screenCaptureTarget = null
        context.session.setCaptureBusy(false)
      }
    }
  }

  async function removeAttachment(attachment: AttachmentView) {
    const workspace = context.getWorkspace()
    if (disposed || context.getInteractionLocked() || !workspace || isTerminal(workspace) || context.session.busy()) return
    const requestId = workspace.request.request_id
    await queueAttachmentWork(async (active) => {
      context.session.setMessage('')
      try {
        if (context.getWorkspace()?.request.request_id !== requestId) return false
        context.getEditor()?.removeAttachmentReference(attachment.attachment_id)
        if (!(await context.saveDraftNow())) return false
        const current = context.getWorkspace()
        if (!active() || current?.request.request_id !== requestId || isTerminal(current)) return false
        const input: RemoveAttachmentInput = {
          request_id: requestId,
          attachment_id: attachment.attachment_id,
          expected_revision: context.getSavedRevision(),
        }
        const next = await context.transport.call('removeFeedbackAttachment', input)
        if (active()) await showWorkspaceMutation(next)
        return active()
      } catch (cause) {
        if (active()) {
          await reconcileMutationFailure(requestId, active)
          if (active()) context.session.setMessage(context.messageFrom(cause), 'error')
        }
        return false
      }
    })
  }

  function insertExistingAttachment(attachment: AttachmentView) {
    const workspace = context.getWorkspace()
    const requestId = workspace?.request.request_id
    if (disposed || context.getInteractionLocked() || !requestId || !workspace || isTerminal(workspace)) return
    const action = context.activeActionFor(requestId)
    void context.routeDraftOperation(requestId, {
      kind: 'appendAttachment',
      attachment,
      label: attachment.file_name,
      action,
    }).catch((cause) => {
      context.session.setMessage(context.messageFrom(cause), 'error')
    })
  }

  async function moveAttachment(index: number, offset: number) {
    const workspace = context.getWorkspace()
    if (disposed || context.getInteractionLocked() || !workspace || isTerminal(workspace) || context.session.busy()) return
    const requestId = workspace.request.request_id
    const target = index + offset
    if (target < 0 || target >= workspace.attachments.length) return
    await queueAttachmentWork(async (active) => {
      context.session.setMessage('')
      try {
        if (context.getWorkspace()?.request.request_id !== requestId) return false
        if (!(await context.saveDraftNow())) return false
        const current = context.getWorkspace()
        if (!active() || current?.request.request_id !== requestId || isTerminal(current)) return false
        const attachmentIds = workspace.attachments.map((item) => item.attachment_id)
        ;[attachmentIds[index], attachmentIds[target]] = [attachmentIds[target], attachmentIds[index]]
        const input: ReorderAttachmentsInput = {
          request_id: requestId,
          attachment_ids: attachmentIds,
          expected_revision: context.getSavedRevision(),
        }
        const next = await context.transport.call('reorderFeedbackAttachments', input)
        if (active()) await showWorkspaceMutation(next)
        return active()
      } catch (cause) {
        if (active()) {
          await reconcileMutationFailure(requestId, active)
          if (active()) context.session.setMessage(context.messageFrom(cause), 'error')
        }
        return false
      }
    })
  }

  const refreshPreviews = previewResources.refresh
  const releasePreviews = previewResources.release

  return {
    mount,
    handleFileSelection,
    acceptAttachmentCandidates,
    importAttachmentCandidates,
    persistAttachmentCandidates,
    reportClientFileError,
    importServerAttachmentPaths,
    startScreenCapture,
    removeAttachment,
    insertExistingAttachment,
    moveAttachment,
    refreshPreviews,
    releasePreviews,
  }
}
