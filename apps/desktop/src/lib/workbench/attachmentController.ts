import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { tick } from 'svelte'

import type {
  AddAttachmentInput,
  AttachmentView,
  FeedbackWorkspaceView,
  RemoveAttachmentInput,
  ReorderAttachmentsInput,
} from '../feedback'
import type { ScreenCaptureReady } from '../screenCapture'
import type { FeedbackEditorHandle } from './types'

export type AttachmentMessageTone = 'info' | 'success' | 'error'

type ScreenCaptureFinished = {
  capture_session_id: string | null
  outcome: 'cancelled' | 'pinned'
}

type AttachmentControllerContext = {
  isTauri: boolean
  tr: (source: string, values?: Record<string, string | number>) => string
  messageFrom: (cause: unknown) => string
  getWorkspace: () => FeedbackWorkspaceView | null
  getEditor: () => FeedbackEditorHandle | undefined
  getRambleRequestId: () => string
  getRambleEngaged: () => boolean
  getSavedRevision: () => number
  getBusy: () => boolean
  getPreviews: () => Record<string, string>
  setBusy: (busy: boolean) => void
  setMessage: (message: string, tone?: AttachmentMessageTone) => void
  setPreviews: (previews: Record<string, string>) => void
  setDragActive: (active: boolean) => void
  saveDraftNow: () => Promise<boolean>
  waitForRambleMarkdown: () => Promise<void>
  appendRambleMarkdown: (requestId: string, markdown: string) => Promise<void>
  applyWorkspaceMutation: (next: FeedbackWorkspaceView) => void
}

export type AttachmentController = ReturnType<typeof createAttachmentController>

export function createAttachmentController(context: AttachmentControllerContext) {
  let screenCaptureRequestId = ''

  function mount() {
    let dragUnlisten: (() => void) | undefined
    let captureReadyUnlisten: (() => void) | undefined
    let captureFinishedUnlisten: (() => void) | undefined

    if (context.isTauri) {
      void listen<ScreenCaptureReady>('screen-capture-ready', (event) => {
        void importScreenCapture(event.payload)
      })
        .then((unlisten) => {
          captureReadyUnlisten = unlisten
        })
        .catch((cause) => {
          context.setMessage(
            context.tr('无法接收截图结果：{error}', { error: context.messageFrom(cause) }),
            'error',
          )
        })
      void listen<ScreenCaptureFinished>('screen-capture-finished', () => {
        context.setBusy(false)
        context.setMessage('')
      })
        .then((unlisten) => {
          captureFinishedUnlisten = unlisten
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
          dragUnlisten = unlisten
        })
        .catch(() => {
          context.setMessage(
            context.tr('当前窗口无法监听文件拖放，请使用文件选择或粘贴。'),
            'error',
          )
        })
    }

    window.addEventListener('paste', handlePaste)
    return () => {
      dragUnlisten?.()
      captureReadyUnlisten?.()
      captureFinishedUnlisten?.()
      window.removeEventListener('paste', handlePaste)
      releasePreviews()
    }
  }

  function handlePaste(event: ClipboardEvent) {
    if (!context.getWorkspace() || context.getBusy() || !event.clipboardData) return
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
    if (!workspace || files.length === 0 || context.getBusy()) return
    if (!(await context.saveDraftNow())) return
    context.setBusy(true)
    context.setMessage('')
    try {
      let next = workspace
      const existingIds = new Set(next.attachments.map((item) => item.attachment_id))
      for (const file of files) {
        if (file.size > 20 * 1024 * 1024) {
          throw new Error(context.tr('{name} 超过 20 MiB 限制', { name: file.name }))
        }
        const input: AddAttachmentInput = {
          request_id: next.request.request_id,
          file_name: file.name || `pasted-image-${Date.now()}.png`,
          contents: Array.from(new Uint8Array(await file.arrayBuffer())),
          expected_revision: next.draft.saved_revision,
        }
        next = await invoke<FeedbackWorkspaceView>('add_feedback_attachment', { input })
        context.applyWorkspaceMutation(next)
      }
      await refreshPreviews(next)
      await tick()
      const inserted = context
        .getEditor()
        ?.insertAttachments(next.attachments.filter((item) => !existingIds.has(item.attachment_id)))
      if (!inserted) {
        throw new Error(context.tr('附件已保存，但编辑器未能在当前光标位置插入图片'))
      }
      await context.saveDraftNow()
    } catch (cause) {
      context.setMessage(context.messageFrom(cause), 'error')
      const current = context.getWorkspace()
      if (current) await refreshPreviews(current)
    } finally {
      context.setBusy(false)
    }
  }

  async function importAttachmentPaths(paths: string[]) {
    const workspace = context.getWorkspace()
    const requestId = context.getRambleRequestId() || workspace?.request.request_id || ''
    if (!requestId || paths.length === 0 || context.getBusy()) return
    const visibleTarget = workspace?.request.request_id === requestId
    if (visibleTarget && !(await context.saveDraftNow())) return
    await context.waitForRambleMarkdown()
    context.setBusy(true)
    context.setMessage('')
    try {
      let next = visibleTarget
        ? workspace
        : await invoke<FeedbackWorkspaceView>('get_feedback_workspace', { requestId })
      if (!next) throw new Error(context.tr('找不到这个反馈请求。'))
      const existingIds = new Set(next.attachments.map((item) => item.attachment_id))
      for (const path of paths) {
        next = await invoke<FeedbackWorkspaceView>('import_feedback_attachment_path', {
          requestId,
          path,
          expectedRevision: next.draft.saved_revision,
        })
        if (visibleTarget) context.applyWorkspaceMutation(next)
      }
      const added = next.attachments.filter((item) => !existingIds.has(item.attachment_id))
      if (visibleTarget && context.getWorkspace()?.request.request_id === requestId) {
        await refreshPreviews(next)
        await tick()
        context.getEditor()?.insertAttachments(added)
        await context.saveDraftNow()
      } else {
        await context.appendRambleMarkdown(
          requestId,
          added
            .map(
              (attachment) =>
                `![${attachment.file_name}](attachment://${attachment.attachment_id})`,
            )
            .join('\n\n'),
        )
      }
    } catch (cause) {
      context.setMessage(context.messageFrom(cause), 'error')
      const current = context.getWorkspace()
      if (current?.request.request_id === requestId) await refreshPreviews(current)
    } finally {
      context.setBusy(false)
    }
  }

  async function startScreenCapture() {
    const workspace = context.getWorkspace()
    const requestId = context.getRambleRequestId() || workspace?.request.request_id || ''
    if (!requestId || !context.getRambleEngaged() || context.getBusy()) return
    if (workspace?.request.request_id === requestId && !(await context.saveDraftNow())) return
    await context.waitForRambleMarkdown()
    screenCaptureRequestId = requestId
    context.setBusy(true)
    context.setMessage('')
    try {
      await invoke('begin_screen_capture')
    } catch (cause) {
      screenCaptureRequestId = ''
      context.setBusy(false)
      const message = context.messageFrom(cause)
      context.setMessage(
        message === '内置区域截图目前只在 Windows 开发环境启用'
          ? context.tr(message)
          : message,
        'error',
      )
    }
  }

  async function importScreenCapture(capture: ScreenCaptureReady) {
    const workspace = context.getWorkspace()
    const requestId =
      screenCaptureRequestId || context.getRambleRequestId() || workspace?.request.request_id || ''
    if (!requestId) {
      await discardScreenCapture(capture.capture_session_id)
      context.setBusy(false)
      return
    }
    try {
      const visibleTarget = workspace?.request.request_id === requestId
      if (visibleTarget && !(await context.saveDraftNow())) {
        throw new Error(context.tr('当前草稿无法保存，截图尚未写入'))
      }
      await context.waitForRambleMarkdown()
      const target = visibleTarget
        ? workspace
        : await invoke<FeedbackWorkspaceView>('get_feedback_workspace', { requestId })
      if (!target) throw new Error(context.tr('找不到这个反馈请求。'))
      const existingIds = new Set(target.attachments.map((item) => item.attachment_id))
      const png = await invoke<ArrayBuffer>('read_completed_screen_capture', {
        captureSessionId: capture.capture_session_id,
      })
      const input: AddAttachmentInput = {
        request_id: requestId,
        file_name: capture.file_name,
        contents: Array.from(new Uint8Array(png)),
        expected_revision: target.draft.saved_revision,
      }
      const next = await invoke<FeedbackWorkspaceView>('add_feedback_attachment', { input })
      const added = next.attachments.filter((item) => !existingIds.has(item.attachment_id))
      if (visibleTarget && context.getWorkspace()?.request.request_id === requestId) {
        context.applyWorkspaceMutation(next)
        await refreshPreviews(next)
        await tick()
        if (!context.getEditor()?.insertAttachments(added)) {
          throw new Error(context.tr('截图附件已保存，但编辑器未能在当前光标位置插入图片'))
        }
        await context.saveDraftNow()
      } else {
        const attachment = added[0]
        if (!attachment) {
          throw new Error(context.tr('截图附件已保存，但编辑器未能在当前光标位置插入图片'))
        }
        await context.appendRambleMarkdown(
          requestId,
          `![${attachment.file_name}](attachment://${attachment.attachment_id})`,
        )
      }
      context.setMessage(context.tr('截图已自动插入当前文档位置'), 'success')
    } catch (cause) {
      context.setMessage(
        context.tr('截图写入失败：{error}', { error: context.messageFrom(cause) }),
        'error',
      )
      const current = context.getWorkspace()
      if (current?.request.request_id === requestId) await refreshPreviews(current)
    } finally {
      screenCaptureRequestId = ''
      await discardScreenCapture(capture.capture_session_id)
      context.setBusy(false)
    }
  }

  async function removeAttachment(attachment: AttachmentView) {
    const workspace = context.getWorkspace()
    if (!workspace || context.getBusy()) return
    context.getEditor()?.removeAttachmentReference(attachment.attachment_id)
    if (!(await context.saveDraftNow())) return
    context.setBusy(true)
    context.setMessage('')
    try {
      const input: RemoveAttachmentInput = {
        request_id: workspace.request.request_id,
        attachment_id: attachment.attachment_id,
        expected_revision: context.getSavedRevision(),
      }
      const next = await invoke<FeedbackWorkspaceView>('remove_feedback_attachment', { input })
      context.applyWorkspaceMutation(next)
      await refreshPreviews(next)
    } catch (cause) {
      context.setMessage(context.messageFrom(cause), 'error')
    } finally {
      context.setBusy(false)
    }
  }

  function insertExistingAttachment(attachment: AttachmentView) {
    context.getEditor()?.insertAttachments([attachment])
  }

  async function moveAttachment(index: number, offset: number) {
    const workspace = context.getWorkspace()
    if (!workspace || context.getBusy()) return
    const target = index + offset
    if (target < 0 || target >= workspace.attachments.length) return
    if (!(await context.saveDraftNow())) return
    context.setBusy(true)
    context.setMessage('')
    try {
      const attachmentIds = workspace.attachments.map((item) => item.attachment_id)
      ;[attachmentIds[index], attachmentIds[target]] = [attachmentIds[target], attachmentIds[index]]
      const input: ReorderAttachmentsInput = {
        request_id: workspace.request.request_id,
        attachment_ids: attachmentIds,
        expected_revision: context.getSavedRevision(),
      }
      const next = await invoke<FeedbackWorkspaceView>('reorder_feedback_attachments', { input })
      context.applyWorkspaceMutation(next)
      await refreshPreviews(next)
    } catch (cause) {
      context.setMessage(context.messageFrom(cause), 'error')
    } finally {
      context.setBusy(false)
    }
  }

  async function refreshPreviews(next: FeedbackWorkspaceView) {
    releasePreviews()
    const previews: Record<string, string> = {}
    for (const attachment of next.attachments) {
      try {
        const bytes = await invoke<number[]>('read_feedback_attachment', {
          requestId: next.request.request_id,
          attachmentId: attachment.attachment_id,
        })
        const buffer = Uint8Array.from(bytes).buffer
        previews[attachment.attachment_id] = URL.createObjectURL(
          new Blob([buffer], { type: attachment.media_type }),
        )
      } catch {
        // A missing preview must not block editing or submission.
      }
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
