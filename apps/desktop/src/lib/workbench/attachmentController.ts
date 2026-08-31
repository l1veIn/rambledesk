import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { tick } from 'svelte'

import { isImageMediaType } from '../attachmentMarkdown'
import type { ApplicationTransport } from '../application/applicationTransport'
import type {
  AddAttachmentInput,
  AttachmentView,
  FeedbackWorkspaceView,
  RemoveAttachmentInput,
  ReorderAttachmentsInput,
} from '../feedback'
import type { ActiveAction } from '../draftOperations'
import type { ScreenCaptureReady } from '../screenCapture'
import type { FeedbackEditorHandle } from './types'

export type AttachmentMessageTone = 'info' | 'success' | 'error'

type ScreenCaptureFinished = {
  capture_session_id: string | null
  outcome: 'cancelled' | 'pinned'
}

type AttachmentControllerContext = {
  isTauri: boolean
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

export function createAttachmentController(context: AttachmentControllerContext) {
  let screenCaptureRequestId = ''
  let screenCaptureAction: ActiveAction = null

  function mount() {
    let disposed = false
    let dragUnlisten: (() => void) | undefined
    let captureReadyUnlisten: (() => void) | undefined
    let captureFinishedUnlisten: (() => void) | undefined

    if (context.isTauri) {
      void listen<ScreenCaptureReady>('screen-capture-ready', (event) => {
        void importScreenCapture(event.payload)
      })
        .then((unlisten) => {
          if (disposed) unlisten()
          else captureReadyUnlisten = unlisten
        })
        .catch((cause) => {
          context.setMessage(
            context.tr('Cannot receive the capture result: {error}', { error: context.messageFrom(cause) }),
            'error',
          )
        })
      void listen<ScreenCaptureFinished>('screen-capture-finished', () => {
        screenCaptureRequestId = ''
        screenCaptureAction = null
        context.setCaptureBusy(false)
        context.setMessage('')
      })
        .then((unlisten) => {
          if (disposed) unlisten()
          else captureFinishedUnlisten = unlisten
        })
        .catch(() => {
          // A failed cancellation listener does not affect capture or attachment storage.
        })
      void getCurrentWebview()
        .onDragDropEvent((event) => {
          context.setDragActive(event.payload.type === 'enter' || event.payload.type === 'over')
          if (event.payload.type === 'drop') {
            context.setDragActive(false)
            void importAttachmentPaths(event.payload.paths)
          } else if (event.payload.type === 'leave') {
            context.setDragActive(false)
          }
        })
        .then((unlisten) => {
          if (disposed) unlisten()
          else dragUnlisten = unlisten
        })
        .catch(() => {
          context.setMessage(
            context.tr('File drop is unavailable in this window. Use the file picker or paste instead.'),
            'error',
          )
        })
    }

    window.addEventListener('paste', handlePaste)
    return () => {
      disposed = true
      dragUnlisten?.()
      captureReadyUnlisten?.()
      captureFinishedUnlisten?.()
      window.removeEventListener('paste', handlePaste)
      releasePreviews()
    }
  }

  function handlePaste(event: ClipboardEvent) {
    if (context.getInteractionLocked() || !context.getWorkspace() || context.getBusy() || !event.clipboardData) return
    const images = Array.from(event.clipboardData.files).filter((file) =>
      file.type.startsWith('image/'),
    )
    if (images.length === 0) return
    event.preventDefault()
    void importFiles(images)
  }

  function handleFileSelection(event: Event) {
    const input = event.currentTarget as HTMLInputElement
    const files = Array.from(input.files ?? [])
    input.value = ''
    void importFiles(files)
  }

  async function importFiles(files: File[]) {
    const workspace = context.getWorkspace()
    if (context.getInteractionLocked() || !workspace || files.length === 0 || context.getBusy()) return
    const requestId = workspace.request.request_id
    const action = context.activeActionFor(requestId)
    if (!(await context.saveDraftNow())) return
    await context.waitForRambleMarkdown()
    context.setBusy(true)
    context.setMessage('')
    try {
      let next = await context.transport.call('getFeedbackWorkspace', { request_id: requestId })
      if (!next) throw new Error(context.tr('This feedback request could not be found.'))
      const existingIds = new Set(next.attachments.map((item) => item.attachment_id))
      for (const file of files) {
        if (file.size > 20 * 1024 * 1024) {
          throw new Error(context.tr('{name} exceeds the 20 MiB limit', { name: file.name }))
        }
        const input: AddAttachmentInput = {
          request_id: requestId,
          file_name: file.name || `attachment-${Date.now()}`,
          contents: Array.from(new Uint8Array(await file.arrayBuffer())),
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
      context.setBusy(false)
    }
  }

  async function importAttachmentPaths(paths: string[]) {
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
        next = await invoke<FeedbackWorkspaceView>('import_feedback_attachment_path', {
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
    if (!context.isTauri) return
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      const appWindow = getCurrentWindow()
      if (await appWindow.isFullscreen()) {
        await appWindow.setFullscreen(false)
      }
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
      await invoke('begin_screen_capture')
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
          await invoke('restart_application')
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
      const next = await invoke<FeedbackWorkspaceView>('add_completed_screen_capture', {
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
    await invoke('discard_screen_capture', { captureSessionId }).catch(() => {})
  }

  return {
    mount,
    handleFileSelection,
    importFiles,
    importAttachmentPaths,
    startScreenCapture,
    removeAttachment,
    insertExistingAttachment,
    moveAttachment,
    refreshPreviews,
    releasePreviews,
  }
}
