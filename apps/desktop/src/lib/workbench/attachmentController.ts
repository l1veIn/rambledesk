import { tick } from 'svelte'

import { isImageMediaType } from '../attachmentMarkdown'
import type { ApplicationTransport } from '../application/applicationTransport'
import type { ApplicationAddAttachmentInput } from '../application/contracts'
import type { WorkbenchCapabilities } from '../capabilities/workbenchCapabilities'
import {
  clientAttachmentCandidate,
  clientAttachmentFile,
  type ClientAttachmentFile,
} from '../capabilities/clientAttachmentFile'
import type { AttachmentCandidate } from '../capabilities/capturePlugin'
import type {
  AttachmentView,
  FeedbackWorkspaceView,
  RemoveAttachmentInput,
  ReorderAttachmentsInput,
} from '../feedback'
import type { ActiveAction } from '../draftOperations'
import type { ScreenCaptureReady } from '../screenCapture'
import type { FeedbackEditorHandle } from './types'

export type AttachmentMessageTone = 'info' | 'success' | 'error'

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
  getBusy: () => boolean
  getCaptureBusy: () => boolean
  getPreviews: () => Record<string, string>
  setBusy: (busy: boolean) => void
  setCaptureBusy: (busy: boolean) => void
  setMessage: (message: string, tone?: AttachmentMessageTone) => void
  setPreviews: (previews: Record<string, string>) => void
  setDragActive: (active: boolean) => void
  saveDraftNow: () => Promise<boolean>
  waitForRambleMarkdown: () => Promise<void>
  routeDraftOperation: (requestId: string, operation: import('../draftOperations').DraftOperation) => Promise<void>
  activeActionFor: (requestId: string) => import('../draftOperations').ActiveAction
  applyWorkspaceMutation: (next: FeedbackWorkspaceView) => void
}

export type AttachmentController = ReturnType<typeof createAttachmentController>

export type AttachmentCandidateTarget = Readonly<{
  requestId: string
  action: ActiveAction
}>

export function createAttachmentController(context: AttachmentControllerContext) {
  let screenCaptureRequestId = ''
  let screenCaptureAction: ActiveAction = null
  let clientFileImportPending = false
  let candidatePersistenceQueue: Promise<void> = Promise.resolve()
  let candidatePersistencePending = 0

  function mount() {
    let dragUnlisten: (() => void) | undefined
    let captureReadyUnlisten: (() => void) | undefined
    let captureFinishedUnlisten: (() => void) | undefined

    const screenCapture = context.capabilities.screenCapture
    if (screenCapture.status.availability !== 'unavailable') {
      captureReadyUnlisten = screenCapture.implementation.onReady(
        (capture) => void importScreenCapture(capture),
        (cause) => {
          context.setMessage(
            context.tr('Cannot receive the capture result: {error}', { error: context.messageFrom(cause) }),
            'error',
          )
        },
      )
      captureFinishedUnlisten = screenCapture.implementation.onFinished(
        () => {
          screenCaptureRequestId = ''
          screenCaptureAction = null
          context.setCaptureBusy(false)
          context.setMessage('')
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
          context.setDragActive(event.type === 'enter' || event.type === 'over')
          if (event.type === 'drop') {
            context.setDragActive(false)
            void importServerAttachmentPaths(event.paths)
          } else if (event.type === 'leave') {
            context.setDragActive(false)
          }
        },
        () => {
          context.setMessage(
            context.tr('File drop is unavailable in this window. Use the file picker or paste instead.'),
            'error',
          )
        },
      )
    }

    return () => {
      dragUnlisten?.()
      captureReadyUnlisten?.()
      captureFinishedUnlisten?.()
      releasePreviews()
    }
  }

  function canImportClientFiles(files: readonly ClientAttachmentFile[]): boolean {
    const workspace = context.getWorkspace()
    return files.length > 0
      && !clientFileImportPending
      && !context.getInteractionLocked()
      && !context.getBusy()
      && workspace !== null
      && workspace.request.status !== 'completed'
      && workspace.request.status !== 'cancelled'
  }

  function acceptClientFiles(files: readonly ClientAttachmentFile[]): boolean {
    if (!canImportClientFiles(files)) return false
    clientFileImportPending = true
    void importClientFiles(files).finally(() => {
      clientFileImportPending = false
    })
    return true
  }

  function handleFileSelection(event: Event) {
    const input = event.currentTarget as HTMLInputElement
    const files = Array.from(input.files ?? [], clientAttachmentFile)
    input.value = ''
    acceptClientFiles(files)
  }

  async function importClientFiles(files: readonly ClientAttachmentFile[]) {
    const workspace = context.getWorkspace()
    if (
      context.getInteractionLocked()
      || !workspace
      || workspace.request.status === 'completed'
      || workspace.request.status === 'cancelled'
      || files.length === 0
      || context.getBusy()
    ) return
    const target = {
      requestId: workspace.request.request_id,
      action: context.activeActionFor(workspace.request.request_id),
    }
    const candidates = files.map((file) => clientAttachmentCandidate(file))
    await persistAttachmentCandidates(target, candidates)
  }

  /**
   * The only shared Attachment Candidate persistence path. Target identity is
   * supplied by the acquisition caller, so later workspace or Action changes
   * cannot retarget the bytes. All callers share one CAS queue.
   */
  function persistAttachmentCandidates(
    target: AttachmentCandidateTarget,
    candidates: readonly AttachmentCandidate[],
  ): Promise<void> {
    if (candidates.length === 0) return Promise.resolve()

    candidatePersistencePending += 1
    context.setBusy(true)
    const run = candidatePersistenceQueue.then(() => persistCandidateBatch(target, candidates))
    candidatePersistenceQueue = run.then(
      () => undefined,
      () => undefined,
    )
    return run.finally(() => {
      candidatePersistencePending -= 1
      if (candidatePersistencePending === 0) context.setBusy(false)
    })
  }

  async function persistCandidateBatch(
    target: AttachmentCandidateTarget,
    candidates: readonly AttachmentCandidate[],
  ) {
    const { requestId, action } = target
    context.setMessage('')
    try {
      const visibleTarget = context.getWorkspace()?.request.request_id === requestId
      if (visibleTarget && !(await context.saveDraftNow())) return
      await context.waitForRambleMarkdown()

      let next = await context.transport.call('getFeedbackWorkspace', { request_id: requestId })
      if (!next) throw new Error(context.tr('This feedback request could not be found.'))
      const existingIds = new Set(next.attachments.map((item) => item.attachment_id))
      for (const candidate of candidates) {
        if (candidate.byteLength > 20 * 1024 * 1024) {
          throw new Error(context.tr('{name} exceeds the 20 MiB limit', {
            name: candidate.fileName,
          }))
        }
        const input: ApplicationAddAttachmentInput = {
          request_id: requestId,
          file_name: candidate.fileName || `attachment-${Date.now()}`,
          contents: await candidate.readBytes(),
          expected_revision: next.draft.saved_revision,
        }
        next = await context.transport.call('addFeedbackAttachment', input)
      }

      const added = next.attachments.filter((item) => !existingIds.has(item.attachment_id))
      if (context.getWorkspace()?.request.request_id === requestId) {
        context.applyWorkspaceMutation(next)
        await refreshPreviews(next)
        await tick()
      }
      for (const attachment of added) {
        await context.routeDraftOperation(requestId, {
          kind: 'appendAttachment',
          attachment,
          label: attachment.file_name,
          action,
        })
      }
    } catch (cause) {
      context.setMessage(context.messageFrom(cause), 'error')
      const current = context.getWorkspace()
      if (current?.request.request_id === requestId) await refreshPreviews(current)
    } finally {
      await Promise.allSettled(candidates.map((candidate) => candidate.dispose()))
    }
  }

  function reportClientFileError(cause: unknown) {
    context.setMessage(context.messageFrom(cause), 'error')
  }

  async function importServerAttachmentPaths(paths: readonly string[]) {
    const workspace = context.getWorkspace()
    const requestId = context.getRambleRequestId() || workspace?.request.request_id || ''
    if (context.getInteractionLocked() || !requestId || paths.length === 0 || context.getBusy()) return
    const visibleTarget = workspace?.request.request_id === requestId
    const action = context.activeActionFor(requestId)
    if (visibleTarget && !(await context.saveDraftNow())) return
    await context.waitForRambleMarkdown()
    context.setBusy(true)
    context.setMessage('')
    try {
      let next = await context.transport.call('getFeedbackWorkspace', { request_id: requestId })
      if (!next) throw new Error(context.tr('This feedback request could not be found.'))
      const existingIds = new Set(next.attachments.map((item) => item.attachment_id))
      for (const path of paths) {
        next = await context.capabilities.serverPaths.implementation.importAttachmentPath({
          requestId,
          path,
          expectedRevision: next.draft.saved_revision,
        })
        if (context.getWorkspace()?.request.request_id === requestId) {
          context.applyWorkspaceMutation(next)
        }
      }
      const added = next.attachments.filter((item) => !existingIds.has(item.attachment_id))
      if (context.getWorkspace()?.request.request_id === requestId) {
        await refreshPreviews(next)
        await tick()
      }
      for (const attachment of added) {
        await context.routeDraftOperation(requestId, {
          kind: 'appendAttachment',
          attachment,
          label: attachment.file_name,
          action,
        })
      }
    } catch (cause) {
      context.setMessage(context.messageFrom(cause), 'error')
      const current = context.getWorkspace()
      if (current?.request.request_id === requestId) await refreshPreviews(current)
    } finally {
      context.setBusy(false)
    }
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
    const requestId = context.getRambleRequestId() || workspace?.request.request_id || ''
    if (
      context.getInteractionLocked() ||
      !requestId ||
      context.getBusy() ||
      context.getCaptureBusy()
    ) return
    if (workspace?.request.request_id === requestId && !(await context.saveDraftNow())) return
    await context.waitForRambleMarkdown()
    screenCaptureRequestId = requestId
    screenCaptureAction = context.activeActionFor(requestId)
    context.setCaptureBusy(true)
    context.setMessage('')
    try {
      await leaveFullscreenIfNeeded()
      await context.capabilities.screenCapture.implementation.begin()
    } catch (cause) {
      screenCaptureRequestId = ''
      screenCaptureAction = null
      context.setCaptureBusy(false)
      const message = context.messageFrom(cause)
      if (message.includes('SCREEN_CAPTURE_PERMISSION_RESTART_REQUIRED')) {
        context.setMessage(
          context.tr('Permission granted. Restart RambleDesk to enable screen capture.'),
          'info',
        )
        try {
          await context.capabilities.windowControls.implementation.restart()
        } catch (restartCause) {
          context.setMessage(context.messageFrom(restartCause), 'error')
        }
        return
      }
      context.setMessage(
        message === 'Built-in region capture is currently available only in Windows development builds.'
          ? context.tr(message)
          : message,
        'error',
      )
    }
  }

  async function importScreenCapture(capture: ScreenCaptureReady) {
    if (context.getInteractionLocked()) {
      await discardScreenCapture(capture.capture_session_id)
      context.setCaptureBusy(false)
      return
    }
    const workspace = context.getWorkspace()
    const requestId =
      screenCaptureRequestId || context.getRambleRequestId() || workspace?.request.request_id || ''
    const action = screenCaptureAction
    if (!requestId) {
      await discardScreenCapture(capture.capture_session_id)
      context.setCaptureBusy(false)
      return
    }
    context.setCaptureBusy(true)
    try {
      const visibleTarget = workspace?.request.request_id === requestId
      if (visibleTarget && !(await context.saveDraftNow())) {
        throw new Error(context.tr('The current draft could not be saved, so the capture was not inserted.'))
      }
      await context.waitForRambleMarkdown()
      const target = visibleTarget
        ? workspace
        : await context.transport.call('getFeedbackWorkspace', { request_id: requestId })
      if (!target) throw new Error(context.tr('This feedback request could not be found.'))
      const existingIds = new Set(target.attachments.map((item) => item.attachment_id))
      const next = await context.capabilities.screenCapture.implementation.complete({
        requestId,
        captureSessionId: capture.capture_session_id,
        expectedRevision: target.draft.saved_revision,
      })
      const added = next.attachments.filter((item) => !existingIds.has(item.attachment_id))
      if (visibleTarget && context.getWorkspace()?.request.request_id === requestId) {
        context.applyWorkspaceMutation(next)
        await refreshPreviews(next)
        await tick()
      }
      const attachment = added[0]
      if (!attachment) {
        throw new Error(context.tr('The captured attachment was saved, but the editor could not insert it at the current cursor.'))
      }
      await context.routeDraftOperation(requestId, {
        kind: 'appendAttachment',
        attachment,
        label: attachment.file_name,
        action,
      })
      context.setMessage(context.tr('Capture inserted at the current document position'), 'success')
    } catch (cause) {
      context.setMessage(
        context.tr('Could not insert capture: {error}', { error: context.messageFrom(cause) }),
        'error',
      )
      const current = context.getWorkspace()
      if (current?.request.request_id === requestId) await refreshPreviews(current)
    } finally {
      screenCaptureRequestId = ''
      screenCaptureAction = null
      await discardScreenCapture(capture.capture_session_id)
      context.setCaptureBusy(false)
    }
  }

  async function removeAttachment(attachment: AttachmentView) {
    const workspace = context.getWorkspace()
    if (context.getInteractionLocked() || !workspace || context.getBusy()) return
    const requestId = workspace.request.request_id
    context.setBusy(true)
    context.setMessage('')
    try {
      context.getEditor()?.removeAttachmentReference(attachment.attachment_id)
      if (!(await context.saveDraftNow())) return
      const input: RemoveAttachmentInput = {
        request_id: requestId,
        attachment_id: attachment.attachment_id,
        expected_revision: context.getSavedRevision(),
      }
      const next = await context.transport.call('removeFeedbackAttachment', input)
      if (context.getWorkspace()?.request.request_id !== requestId) return
      context.applyWorkspaceMutation(next)
      await refreshPreviews(next)
    } catch (cause) {
      context.setMessage(context.messageFrom(cause), 'error')
    } finally {
      context.setBusy(false)
    }
  }

  function insertExistingAttachment(attachment: AttachmentView) {
    const requestId = context.getWorkspace()?.request.request_id
    if (context.getInteractionLocked() || !requestId) return
    const action = context.activeActionFor(requestId)
    void context.routeDraftOperation(requestId, {
      kind: 'appendAttachment',
      attachment,
      label: attachment.file_name,
      action,
    }).catch((cause) => {
      context.setMessage(context.messageFrom(cause), 'error')
    })
  }

  async function moveAttachment(index: number, offset: number) {
    const workspace = context.getWorkspace()
    if (context.getInteractionLocked() || !workspace || context.getBusy()) return
    const requestId = workspace.request.request_id
    const target = index + offset
    if (target < 0 || target >= workspace.attachments.length) return
    context.setBusy(true)
    context.setMessage('')
    try {
      if (!(await context.saveDraftNow())) return
      const attachmentIds = workspace.attachments.map((item) => item.attachment_id)
      ;[attachmentIds[index], attachmentIds[target]] = [attachmentIds[target], attachmentIds[index]]
      const input: ReorderAttachmentsInput = {
        request_id: requestId,
        attachment_ids: attachmentIds,
        expected_revision: context.getSavedRevision(),
      }
      const next = await context.transport.call('reorderFeedbackAttachments', input)
      if (context.getWorkspace()?.request.request_id !== requestId) return
      context.applyWorkspaceMutation(next)
      await refreshPreviews(next)
    } catch (cause) {
      context.setMessage(context.messageFrom(cause), 'error')
    } finally {
      context.setBusy(false)
    }
  }

  async function refreshPreviews(next: FeedbackWorkspaceView) {
    const current = context.getPreviews()
    const keep = new Set(
      next.attachments
        .filter((attachment) => isImageMediaType(attachment.media_type))
        .map((attachment) => attachment.attachment_id),
    )
    const previews: Record<string, string> = {}
    for (const [attachmentId, url] of Object.entries(current)) {
      if (keep.has(attachmentId)) previews[attachmentId] = url
      else URL.revokeObjectURL(url)
    }
    for (const attachment of next.attachments) {
      if (!isImageMediaType(attachment.media_type) || previews[attachment.attachment_id]) continue
      try {
        const bytes = await context.transport.call('readFeedbackAttachment', {
          request_id: next.request.request_id,
          attachment_id: attachment.attachment_id,
        })
        previews[attachment.attachment_id] = URL.createObjectURL(
          new Blob([bytes], { type: attachment.media_type }),
        )
      } catch {
        // A missing preview must not block editing or submission.
      }
    }
    if (context.getWorkspace()?.request.request_id !== next.request.request_id) {
      for (const url of Object.values(previews)) URL.revokeObjectURL(url)
      return
    }
    context.setPreviews(previews)
  }

  function releasePreviews() {
    for (const url of Object.values(context.getPreviews())) URL.revokeObjectURL(url)
    context.setPreviews({})
  }

  async function discardScreenCapture(captureSessionId: string) {
    await context.capabilities.screenCapture.implementation.discard(captureSessionId).catch(() => {})
  }

  return {
    mount,
    handleFileSelection,
    acceptClientFiles,
    importClientFiles,
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
