<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { getCurrentWebview } from '@tauri-apps/api/webview'
  import {
    isPermissionGranted,
    requestPermission,
    sendNotification,
  } from '@tauri-apps/plugin-notification'
  import { revealItemInDir } from '@tauri-apps/plugin-opener'
  import { onMount, tick } from 'svelte'

  import RichFeedbackEditor from './lib/RichFeedbackEditor.svelte'
  import type {
    AddAttachmentInput,
    AttachmentView,
    DraftView,
    FeedbackRequestSummary,
    FeedbackRequestView,
    FeedbackWorkspaceView,
    ListFeedbackRequestsOutput,
    RemoveAttachmentInput,
    ReorderAttachmentsInput,
    SaveDraftInput,
    SubmitFeedbackInput,
  } from './lib/feedback'
  import { requestStatusLabel } from './lib/feedback'
  import {
    clipboardCaptureLabel,
    eventBelongsToRamble,
    type ClipboardCaptureEvent,
  } from './lib/clipboardCapture'
  import type { HealthSnapshot } from './lib/generated/health'
  import {
    InboxNotificationTracker,
    notificationLabel,
    type NotificationState,
  } from './lib/notifications'
  import { desktopPath } from './lib/nativePath'
  import {
    eventBelongsToVoiceSession,
    stableTranscript,
    type SpeechEvent,
    type VoiceRambleSessionView,
  } from './lib/speech'
  import type { ScreenCaptureReady } from './lib/screenCapture'

  type SavePhase = 'idle' | 'unsaved' | 'saving' | 'saved' | 'error'
  type RamblePhase = 'idle' | 'starting' | 'active' | 'stopping' | 'error'
  type VoicePhase = 'idle' | 'starting' | 'listening' | 'processing' | 'stopping' | 'error'
  type CommandError = { code: string; message: string; retryable: boolean }

  let health: HealthSnapshot | null = null
  let endpoint = '正在连接…'
  let inbox: FeedbackRequestSummary[] = []
  let history: FeedbackRequestSummary[] = []
  let inboxMode: 'open' | 'history' = 'open'
  let workspace: FeedbackWorkspaceView | null = null
  let completedResult: FeedbackRequestView | null = null
  let draftBody = ''
  let savedBody = ''
  let savedRevision = 0
  let savePhase: SavePhase = 'idle'
  let saveMessage = ''
  let pageError = ''
  let loadingInbox = true
  let loadingHistory = false
  let loadingWorkspace = false
  let submitting = false
  let attachmentBusy = false
  let attachmentMessage = ''
  let attachmentPreviews: Record<string, string> = {}
  let dragActive = false
  let attachmentInput: HTMLInputElement
  let richEditor: RichFeedbackEditor
  let notificationState: NotificationState = 'checking'
  let settingsOpen = false
  let mcpConfiguration = ''
  let voicePhase: VoicePhase = 'idle'
  let voiceRequestId = ''
  let voiceSessionId = ''
  let voiceDevice = ''
  let voicePartial = ''
  let voiceMessage = ''
  let voiceLevel = 0
  let voiceChunkIndex = 0
  let ramblePhase: RamblePhase = 'idle'
  let rambleStartedOnce = false
  let rambleMessage = ''
  let clipboardActive = false
  let clipboardCaptureCount = 0
  let clipboardImageQueue: Promise<void> = Promise.resolve()
  let copyState: 'idle' | 'copied' | 'error' = 'idle'
  let saveTimer: ReturnType<typeof setTimeout> | undefined
  let inboxTimer: ReturnType<typeof setInterval> | undefined
  let activeSave: Promise<boolean> | null = null
  const notificationTracker = new InboxNotificationTracker()

  $: dirty = workspace !== null && draftBody !== savedBody
  $: displayedRequests = inboxMode === 'open' ? inbox : history
  $: feedbackResult = completedResult?.feedback ?? workspace?.feedback ?? null
  $: canSubmit =
    workspace !== null &&
    workspace.request.status !== 'completed' &&
    workspace.request.status !== 'cancelled' &&
    draftBody.trim().length > 0 &&
    !submitting
  $: voiceActive =
    voicePhase === 'starting' ||
    voicePhase === 'listening' ||
    voicePhase === 'processing' ||
    voicePhase === 'stopping'
  $: voiceCanStop =
    voiceActive || (voicePhase === 'error' && voiceSessionId.length > 0)
  $: rambleActive = ramblePhase === 'active'
  $: rambleBusy = ramblePhase === 'starting' || ramblePhase === 'stopping'
  $: rambleCanStop =
    rambleActive || ramblePhase === 'error' || voiceCanStop || clipboardActive

  onMount(() => {
    void initialize()
    void refreshNotificationPermission()
    inboxTimer = setInterval(() => void refreshInbox(), 5_000)
    let dragUnlisten: (() => void) | undefined
    let voiceUnlisten: (() => void) | undefined
    let rambleShortcutUnlisten: (() => void) | undefined
    let clipboardUnlisten: (() => void) | undefined
    let captureShortcutUnlisten: (() => void) | undefined
    let captureReadyUnlisten: (() => void) | undefined
    let captureCancelledUnlisten: (() => void) | undefined
    void listen<SpeechEvent>('voice-ramble-event', (event) => {
      handleVoiceEvent(event.payload)
    })
      .then((unlisten) => {
        voiceUnlisten = unlisten
      })
      .catch((cause) => {
        voicePhase = 'error'
        voiceMessage = `无法监听语音识别事件：${messageFrom(cause)}`
      })
    void listen<string>('screen-capture-shortcut', () => {
      if (rambleActive) void startScreenCapture()
    })
      .then((unlisten) => {
        captureShortcutUnlisten = unlisten
      })
      .catch((cause) => {
        attachmentMessage = `无法监听截图快捷键：${messageFrom(cause)}`
      })
    void listen<string>('ramble-toggle-shortcut', () => {
      void toggleRamble()
    })
      .then((unlisten) => {
        rambleShortcutUnlisten = unlisten
      })
      .catch((cause) => {
        ramblePhase = 'error'
        rambleMessage = `无法监听 Ramble 快捷键：${messageFrom(cause)}`
      })
    void listen<ClipboardCaptureEvent>('clipboard-capture-event', (event) => {
      handleClipboardCaptureEvent(event.payload)
    })
      .then((unlisten) => {
        clipboardUnlisten = unlisten
      })
      .catch((cause) => {
        ramblePhase = 'error'
        rambleMessage = `无法监听剪贴板：${messageFrom(cause)}`
      })
    void listen<ScreenCaptureReady>('screen-capture-ready', (event) => {
      void importScreenCapture(event.payload)
    })
      .then((unlisten) => {
        captureReadyUnlisten = unlisten
      })
      .catch((cause) => {
        attachmentMessage = `无法接收截图结果：${messageFrom(cause)}`
      })
    void listen('screen-capture-cancelled', () => {
      attachmentBusy = false
      attachmentMessage = '截图已取消'
    })
      .then((unlisten) => {
        captureCancelledUnlisten = unlisten
      })
      .catch(() => {
        // A failed cancellation listener does not affect capture or attachment storage.
      })
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        dragActive = event.payload.type === 'enter' || event.payload.type === 'over'
        if (event.payload.type === 'drop') {
          dragActive = false
          void importAttachmentPaths(event.payload.paths)
        } else if (event.payload.type === 'leave') {
          dragActive = false
        }
      })
      .then((unlisten) => {
        dragUnlisten = unlisten
      })
      .catch(() => {
        attachmentMessage = '当前窗口无法监听文件拖放，请使用文件选择或粘贴。'
      })
    window.addEventListener('paste', handlePaste)
    return () => {
      if (saveTimer) clearTimeout(saveTimer)
      if (inboxTimer) clearInterval(inboxTimer)
      dragUnlisten?.()
      voiceUnlisten?.()
      rambleShortcutUnlisten?.()
      clipboardUnlisten?.()
      captureShortcutUnlisten?.()
      captureReadyUnlisten?.()
      captureCancelledUnlisten?.()
      if (voiceCanStop) void invoke('stop_voice_ramble')
      if (clipboardActive) void invoke('stop_clipboard_capture')
      window.removeEventListener('paste', handlePaste)
      releaseAttachmentPreviews()
    }
  })

  async function initialize() {
    pageError = ''
    loadingInbox = true
    try {
      const [nextHealth, nextEndpoint, nextInbox] = await Promise.all([
        invoke<HealthSnapshot>('get_health'),
        invoke<string>('get_mcp_endpoint'),
        invoke<FeedbackRequestSummary[]>('list_feedback_inbox'),
      ])
      health = nextHealth
      endpoint = nextEndpoint
      applyInboxSnapshot(nextInbox)
      if (nextInbox.length > 0) {
        await openRequest(nextInbox[0].request_id, false)
      }
    } catch (cause) {
      pageError = messageFrom(cause)
    } finally {
      loadingInbox = false
    }
  }

  async function refreshInbox() {
    try {
      const nextInbox = await invoke<FeedbackRequestSummary[]>('list_feedback_inbox')
      applyInboxSnapshot(nextInbox)
    } catch (cause) {
      pageError = messageFrom(cause)
    }
  }

  function applyInboxSnapshot(nextInbox: FeedbackRequestSummary[]) {
    const arrivals = notificationTracker.observe(nextInbox)
    inbox = nextInbox
    void invoke('set_pending_count', { count: nextInbox.length }).catch(() => {
      // Tray updates are a convenience; the inbox remains authoritative.
    })
    if (arrivals.length > 0 && notificationState === 'enabled') {
      sendNotification({
        title: 'RambleDesk',
        body:
          arrivals.length === 1
            ? '新的体验反馈请求已到达。打开工作台查看。'
            : `${arrivals.length} 个新的体验反馈请求已到达。打开工作台查看。`,
      })
    }
  }

  async function refreshNotificationPermission() {
    try {
      notificationState = (await isPermissionGranted()) ? 'enabled' : 'disabled'
    } catch {
      notificationState = 'unavailable'
    }
  }

  async function enableNotifications() {
    if (notificationState !== 'disabled') return
    notificationState = 'checking'
    try {
      const permission = await requestPermission()
      notificationState = permission === 'granted' ? 'enabled' : 'disabled'
    } catch {
      notificationState = 'unavailable'
    }
  }

  async function openRequest(requestId: string, saveCurrent = true) {
    if (workspace?.request.request_id === requestId) return
    if (rambleCanStop) await stopRamble()
    if (saveCurrent && !(await saveDraftNow())) return

    loadingWorkspace = true
    pageError = ''
    completedResult = null
    try {
      const next = await invoke<FeedbackWorkspaceView>('get_feedback_workspace', {
        requestId,
      })
      workspace = next
      draftBody = next.draft.body_markdown
      savedBody = next.draft.body_markdown
      savedRevision = next.draft.saved_revision
      savePhase = next.draft.updated_at ? 'saved' : 'idle'
      saveMessage = ''
      attachmentMessage = ''
      resetVoiceUi()
      resetRambleUi()
      await refreshAttachmentPreviews(next)
    } catch (cause) {
      pageError = messageFrom(cause)
    } finally {
      loadingWorkspace = false
    }
  }

  function updateDraft(value: string) {
    draftBody = value
    savePhase = draftBody === savedBody ? 'saved' : 'unsaved'
    saveMessage = ''
    if (saveTimer) clearTimeout(saveTimer)
    saveTimer = setTimeout(() => void saveDraftNow(), 700)
  }

  async function saveDraftNow(): Promise<boolean> {
    if (saveTimer) {
      clearTimeout(saveTimer)
      saveTimer = undefined
    }
    if (!workspace || !dirty) return true
    if (activeSave) {
      await activeSave
      return dirty ? saveDraftNow() : savePhase !== 'error'
    }

    const requestId = workspace.request.request_id
    const bodyToSave = draftBody
    const revisionToSave = savedRevision
    savePhase = 'saving'
    saveMessage = ''

    activeSave = (async () => {
      try {
        const input: SaveDraftInput = {
          request_id: requestId,
          body_markdown: bodyToSave,
          expected_revision: revisionToSave,
        }
        const saved = await invoke<DraftView>('save_feedback_draft', { input })
        if (workspace?.request.request_id === requestId) {
          savedBody = bodyToSave
          savedRevision = saved.saved_revision
          workspace = { ...workspace, draft: saved }
          savePhase = draftBody === bodyToSave ? 'saved' : 'unsaved'
        }
        return true
      } catch (cause) {
        savePhase = 'error'
        saveMessage = messageFrom(cause)
        return false
      }
    })()

    const succeeded = await activeSave
    activeSave = null
    if (succeeded && workspace?.request.request_id === requestId && draftBody !== savedBody) {
      return saveDraftNow()
    }
    return succeeded
  }

  async function reloadWorkspace() {
    const requestId = workspace?.request.request_id
    if (!requestId) return
    if (rambleCanStop) await stopRamble()
    if (dirty && !(await saveDraftNow())) return
    workspace = null
    await openRequest(requestId, false)
  }

  async function toggleSettings() {
    if (settingsOpen) {
      settingsOpen = false
      return
    }
    pageError = ''
    copyState = 'idle'
    try {
      mcpConfiguration = await invoke<string>('get_mcp_configuration')
      settingsOpen = true
    } catch (cause) {
      pageError = messageFrom(cause)
    }
  }

  async function copyMcpConfiguration() {
    try {
      await navigator.clipboard.writeText(mcpConfiguration)
      copyState = 'copied'
    } catch {
      copyState = 'error'
    }
  }

  async function refreshHistory() {
    loadingHistory = true
    try {
      const result = await invoke<ListFeedbackRequestsOutput>('list_feedback_history')
      history = result.requests
    } catch (cause) {
      pageError = messageFrom(cause)
    } finally {
      loadingHistory = false
    }
  }

  function showOpenRequests() {
    inboxMode = 'open'
    void refreshInbox()
  }

  function showHistory() {
    inboxMode = 'history'
    void refreshHistory()
  }

  function refreshCurrentList() {
    if (inboxMode === 'history') void refreshHistory()
    else void refreshInbox()
  }

  function handlePaste(event: ClipboardEvent) {
    if (!workspace || attachmentBusy || !event.clipboardData) return
    const images = Array.from(event.clipboardData.files).filter((file) =>
      file.type.startsWith('image/'),
    )
    if (images.length === 0) return
    event.preventDefault()
    void importAttachmentFiles(images)
  }

  function handleFileSelection(event: Event) {
    const input = event.currentTarget as HTMLInputElement
    const files = Array.from(input.files ?? [])
    input.value = ''
    void importAttachmentFiles(files)
  }

  async function importAttachmentFiles(files: File[]) {
    if (!workspace || files.length === 0 || attachmentBusy) return
    if (!(await saveDraftNow())) return
    attachmentBusy = true
    attachmentMessage = ''
    try {
      let next = workspace
      const existingIds = new Set(next.attachments.map((item) => item.attachment_id))
      for (const file of files) {
        if (file.size > 20 * 1024 * 1024) {
          throw new Error(`${file.name} 超过 20 MiB 限制`)
        }
        const input: AddAttachmentInput = {
          request_id: next.request.request_id,
          file_name: file.name || `pasted-image-${Date.now()}.png`,
          contents: Array.from(new Uint8Array(await file.arrayBuffer())),
          expected_revision: next.draft.saved_revision,
        }
        next = await invoke<FeedbackWorkspaceView>('add_feedback_attachment', { input })
        applyWorkspaceMutation(next)
      }
      await refreshAttachmentPreviews(next)
      await tick()
      const inserted = richEditor?.insertAttachments(
        next.attachments.filter((item) => !existingIds.has(item.attachment_id)),
      )
      if (!inserted) {
        throw new Error('附件已保存，但编辑器未能在当前光标位置插入图片')
      }
      await saveDraftNow()
    } catch (cause) {
      attachmentMessage = messageFrom(cause)
      if (workspace) await refreshAttachmentPreviews(workspace)
    } finally {
      attachmentBusy = false
    }
  }

  async function importAttachmentPaths(paths: string[]) {
    if (!workspace || paths.length === 0 || attachmentBusy) return
    if (!(await saveDraftNow())) return
    attachmentBusy = true
    attachmentMessage = ''
    try {
      let next = workspace
      const existingIds = new Set(next.attachments.map((item) => item.attachment_id))
      for (const path of paths) {
        next = await invoke<FeedbackWorkspaceView>('import_feedback_attachment_path', {
          requestId: next.request.request_id,
          path,
          expectedRevision: next.draft.saved_revision,
        })
        applyWorkspaceMutation(next)
      }
      await refreshAttachmentPreviews(next)
      await tick()
      richEditor?.insertAttachments(
        next.attachments.filter((item) => !existingIds.has(item.attachment_id)),
      )
      await saveDraftNow()
    } catch (cause) {
      attachmentMessage = messageFrom(cause)
      if (workspace) await refreshAttachmentPreviews(workspace)
    } finally {
      attachmentBusy = false
    }
  }

  async function startScreenCapture() {
    if (
      !workspace ||
      !rambleActive ||
      attachmentBusy ||
      workspace.request.status === 'completed' ||
      workspace.request.status === 'cancelled'
    ) {
      return
    }
    if (!(await saveDraftNow())) return
    attachmentBusy = true
    attachmentMessage = '截图工具已唤起：拖动框选，Esc 或右键取消'
    try {
      await invoke('begin_screen_capture')
    } catch (cause) {
      attachmentBusy = false
      attachmentMessage = messageFrom(cause)
    }
  }

  async function importScreenCapture(capture: ScreenCaptureReady) {
    if (!workspace) {
      await invoke('discard_screen_capture', { sessionId: capture.session_id }).catch(() => {})
      attachmentBusy = false
      return
    }
    const requestId = workspace.request.request_id
    try {
      if (!(await saveDraftNow())) {
        throw new Error('当前草稿无法保存，截图尚未写入')
      }
      const existingIds = new Set(workspace.attachments.map((item) => item.attachment_id))
      const png = await invoke<ArrayBuffer>('read_completed_screen_capture', {
        sessionId: capture.session_id,
      })
      const input: AddAttachmentInput = {
        request_id: requestId,
        file_name: capture.file_name,
        contents: Array.from(new Uint8Array(png)),
        expected_revision: savedRevision,
      }
      const next = await invoke<FeedbackWorkspaceView>('add_feedback_attachment', { input })
      if (workspace?.request.request_id !== requestId) return
      applyWorkspaceMutation(next)
      await refreshAttachmentPreviews(next)
      await tick()
      const inserted = richEditor?.insertAttachments(
        next.attachments.filter((item) => !existingIds.has(item.attachment_id)),
      )
      if (!inserted) {
        throw new Error('截图附件已保存，但编辑器未能在当前光标位置插入图片')
      }
      await saveDraftNow()
      attachmentMessage = '截图已自动插入当前文档位置'
    } catch (cause) {
      attachmentMessage = `截图写入失败：${messageFrom(cause)}`
      if (workspace?.request.request_id === requestId) {
        await refreshAttachmentPreviews(workspace)
      }
    } finally {
      await invoke('discard_screen_capture', { sessionId: capture.session_id }).catch(() => {})
      attachmentBusy = false
    }
  }

  async function removeAttachment(attachment: AttachmentView) {
    if (!workspace || attachmentBusy) return
    richEditor?.removeAttachmentReference(attachment.attachment_id)
    if (!(await saveDraftNow())) return
    attachmentBusy = true
    attachmentMessage = ''
    try {
      const input: RemoveAttachmentInput = {
        request_id: workspace.request.request_id,
        attachment_id: attachment.attachment_id,
        expected_revision: savedRevision,
      }
      const next = await invoke<FeedbackWorkspaceView>('remove_feedback_attachment', { input })
      applyWorkspaceMutation(next)
      await refreshAttachmentPreviews(next)
    } catch (cause) {
      attachmentMessage = messageFrom(cause)
    } finally {
      attachmentBusy = false
    }
  }

  function insertExistingAttachment(attachment: AttachmentView) {
    richEditor?.insertAttachments([attachment])
  }

  async function moveAttachment(index: number, offset: number) {
    if (!workspace || attachmentBusy) return
    const target = index + offset
    if (target < 0 || target >= workspace.attachments.length) return
    if (!(await saveDraftNow())) return
    attachmentBusy = true
    attachmentMessage = ''
    try {
      const attachmentIds = workspace.attachments.map((item) => item.attachment_id)
      ;[attachmentIds[index], attachmentIds[target]] = [attachmentIds[target], attachmentIds[index]]
      const input: ReorderAttachmentsInput = {
        request_id: workspace.request.request_id,
        attachment_ids: attachmentIds,
        expected_revision: savedRevision,
      }
      const next = await invoke<FeedbackWorkspaceView>('reorder_feedback_attachments', { input })
      applyWorkspaceMutation(next)
      await refreshAttachmentPreviews(next)
    } catch (cause) {
      attachmentMessage = messageFrom(cause)
    } finally {
      attachmentBusy = false
    }
  }

  function applyWorkspaceMutation(next: FeedbackWorkspaceView) {
    const localBody = draftBody
    workspace = next
    savedBody = next.draft.body_markdown
    savedRevision = next.draft.saved_revision
    if (localBody === next.draft.body_markdown) {
      draftBody = next.draft.body_markdown
      savePhase = 'saved'
    } else {
      draftBody = localBody
      savePhase = 'unsaved'
      if (saveTimer) clearTimeout(saveTimer)
      saveTimer = setTimeout(() => void saveDraftNow(), 700)
    }
  }

  async function refreshAttachmentPreviews(next: FeedbackWorkspaceView) {
    releaseAttachmentPreviews()
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
    attachmentPreviews = previews
  }

  function releaseAttachmentPreviews() {
    for (const url of Object.values(attachmentPreviews)) URL.revokeObjectURL(url)
    attachmentPreviews = {}
  }

  async function submitFeedback() {
    if (!workspace || !canSubmit) return
    if (rambleCanStop) await stopRamble()
    if (!(await saveDraftNow())) return

    submitting = true
    pageError = ''
    try {
      const input: SubmitFeedbackInput = {
        request_id: workspace.request.request_id,
        expected_revision: savedRevision,
      }
      const result = await invoke<FeedbackRequestView>('submit_feedback', { input })
      completedResult = result
      workspace = {
        ...workspace,
        feedback: result.feedback,
        request: {
          ...workspace.request,
          status: result.status,
          updated_at: result.updated_at,
        },
      }
      savePhase = 'saved'
      await refreshInbox()
      if (history.length > 0) await refreshHistory()
    } catch (cause) {
      pageError = messageFrom(cause)
    } finally {
      submitting = false
    }
  }

  async function openFeedbackPackage() {
    if (!feedbackResult) return
    try {
      await revealItemInDir(desktopPath(feedbackResult.markdown_path))
    } catch (cause) {
      pageError = `无法打开 Feedback Package：${messageFrom(cause)}`
    }
  }

  async function startRamble() {
    if (
      !workspace ||
      rambleBusy ||
      rambleActive ||
      workspace.request.status === 'completed' ||
      workspace.request.status === 'cancelled'
    ) {
      return
    }
    const requestId = workspace.request.request_id
    ramblePhase = 'starting'
    rambleMessage = '正在启动麦克风、截图快捷键和剪贴板捕获…'
    clipboardCaptureCount = 0

    const voiceStarted = await startVoiceRamble()
    if (!voiceStarted || !voiceSessionId) {
      ramblePhase = 'error'
      rambleMessage = voiceMessage || '麦克风启动失败'
      return
    }

    try {
      await invoke('start_clipboard_capture', {
        input: {
          request_id: requestId,
          ramble_session_id: voiceSessionId,
        },
      })
      if (workspace?.request.request_id !== requestId) {
        await Promise.allSettled([
          invoke('stop_clipboard_capture'),
          invoke('stop_voice_ramble'),
        ])
        resetVoiceUi()
        ramblePhase = 'idle'
        return
      }
      clipboardActive = true
      rambleStartedOnce = true
      ramblePhase = 'active'
      rambleMessage = 'Ramble 进行中 · 可离开窗口继续操作电脑'
    } catch (cause) {
      await stopVoiceRamble()
      clipboardActive = false
      ramblePhase = 'error'
      rambleMessage = `剪贴板捕获启动失败：${messageFrom(cause)}`
    }
  }

  async function stopRamble() {
    if (!rambleCanStop || ramblePhase === 'stopping') return
    ramblePhase = 'stopping'
    rambleMessage = '正在收尾最后一段语音并暂停上下文采集…'
    let stopError = ''
    if (clipboardActive) {
      try {
        await invoke('stop_clipboard_capture')
      } catch (cause) {
        stopError = messageFrom(cause)
      } finally {
        clipboardActive = false
      }
    }
    if (voiceCanStop) {
      const voiceStopped = await stopVoiceRamble()
      if (!voiceStopped && !stopError) stopError = voiceMessage || '麦克风停止失败'
    }
    if (stopError) {
      ramblePhase = 'error'
      rambleMessage = stopError
    } else {
      ramblePhase = 'idle'
      rambleMessage = 'Ramble 已暂停；正文保留，随时可以继续'
    }
  }

  async function toggleRamble() {
    if (rambleBusy) return
    if (rambleActive || voiceCanStop || clipboardActive) await stopRamble()
    else await startRamble()
  }

  async function startVoiceRamble(): Promise<boolean> {
    if (
      !workspace ||
      voiceActive ||
      workspace.request.status === 'completed' ||
      workspace.request.status === 'cancelled'
    ) {
      return false
    }
    voicePhase = 'starting'
    voiceRequestId = workspace.request.request_id
    voiceSessionId = ''
    voiceDevice = ''
    voicePartial = ''
    voiceMessage = '正在加载本地模型并连接麦克风…'
    voiceLevel = 0
    try {
      const session = await invoke<VoiceRambleSessionView>('start_voice_ramble', {
        input: {
          request_id: workspace.request.request_id,
        },
      })
      voiceSessionId = session.session_id
      if (voicePhase === 'starting') {
        voicePhase = 'listening'
        voiceMessage = 'Sherpa 真流式识别 · 自然停顿后写入正文'
      }
    } catch (cause) {
      voicePhase = 'error'
      voiceMessage = messageFrom(cause)
      return false
    }
    return true
  }

  async function stopVoiceRamble(): Promise<boolean> {
    if (!voiceCanStop) return true
    voicePhase = 'stopping'
    voiceMessage = '正在完成最后一段识别…'
    try {
      await invoke('stop_voice_ramble')
      for (let attempt = 0; attempt < 5 && voicePhase === 'stopping'; attempt += 1) {
        await new Promise((resolve) => setTimeout(resolve, 20))
      }
      await tick()
      if (voicePhase === 'stopping') {
        voicePhase = 'idle'
        voiceMessage = '录音已停止'
      }
    } catch (cause) {
      voicePhase = 'error'
      voiceMessage = messageFrom(cause)
      return false
    } finally {
      voiceLevel = 0
    }
    return true
  }

  function handleClipboardCaptureEvent(event: ClipboardCaptureEvent) {
    if (
      !rambleActive ||
      !workspace ||
      !eventBelongsToRamble(
        event,
        workspace.request.request_id,
        voiceSessionId,
      )
    ) {
      if (event.type === 'image') {
        void invoke('discard_clipboard_capture_image', {
          captureId: event.capture_id,
        })
      }
      return
    }

    if (event.type === 'warning') {
      rambleMessage = event.message
      return
    }
    if (event.type === 'text') {
      const inserted = richEditor?.appendClipboardCapture(
        event.text,
        clipboardCaptureLabel(event.captured_at_ms, event.truncated),
      )
      if (inserted) {
        clipboardCaptureCount += 1
        rambleMessage = `Ramble 进行中 · 已捕获 ${clipboardCaptureCount} 项剪贴板上下文`
      }
      return
    }

    clipboardImageQueue = clipboardImageQueue
      .then(() => importClipboardImage(event))
      .catch((cause) => {
        attachmentMessage = `剪贴板图片写入失败：${messageFrom(cause)}`
      })
  }

  async function importClipboardImage(
    event: Extract<ClipboardCaptureEvent, { type: 'image' }>,
  ) {
    const requestId = event.request_id
    try {
      for (let attempt = 0; attachmentBusy && attempt < 200; attempt += 1) {
        await new Promise((resolve) => setTimeout(resolve, 50))
      }
      if (attachmentBusy) throw new Error('附件通道正忙，请稍后重新复制图片')
      if (!workspace || workspace.request.request_id !== requestId) return
      if (!(await saveDraftNow())) throw new Error('当前草稿无法保存')

      attachmentBusy = true
      const png = await invoke<ArrayBuffer>('read_clipboard_capture_image', {
        captureId: event.capture_id,
        requestId,
        rambleSessionId: event.ramble_session_id,
      })
      const input: AddAttachmentInput = {
        request_id: requestId,
        file_name: event.file_name,
        contents: Array.from(new Uint8Array(png)),
        expected_revision: savedRevision,
      }
      const next = await invoke<FeedbackWorkspaceView>('add_feedback_attachment', { input })
      if (workspace?.request.request_id !== requestId) return
      const attachment = next.attachments.find(
        (item) => !workspace?.attachments.some(
          (existing) => existing.attachment_id === item.attachment_id,
        ),
      )
      applyWorkspaceMutation(next)
      await refreshAttachmentPreviews(next)
      await tick()
      if (
        !attachment ||
        !richEditor?.appendCapturedAttachment(
          attachment,
          clipboardCaptureLabel(event.captured_at_ms),
        )
      ) {
        throw new Error('图片附件已保存，但未能写入文档流')
      }
      await saveDraftNow()
      clipboardCaptureCount += 1
      rambleMessage = `Ramble 进行中 · 已捕获 ${clipboardCaptureCount} 项剪贴板上下文`
    } finally {
      attachmentBusy = false
      await invoke('discard_clipboard_capture_image', {
        captureId: event.capture_id,
      }).catch(() => {})
    }
  }

  function handleVoiceEvent(event: SpeechEvent) {
    const currentRequestId = workspace?.request.request_id ?? voiceRequestId
    if (
      !eventBelongsToVoiceSession(
        event,
        currentRequestId,
        voiceSessionId,
      )
    ) {
      return
    }
    voiceRequestId = event.request_id
    voiceSessionId = event.session_id
    switch (event.type) {
      case 'started':
        voicePhase = 'listening'
        voiceDevice = event.input_device
        voiceMessage = `正在录音 · ${event.input_device}`
        break
      case 'partial':
        voicePartial = event.text
        if (voicePhase !== 'stopping') voicePhase = 'listening'
        break
      case 'level':
        voiceLevel = Math.min(1, Math.max(0, event.rms * 8))
        if (voicePhase !== 'stopping') voicePhase = 'listening'
        break
      case 'processing':
        voiceChunkIndex = event.chunk_index + 1
        if (voicePhase !== 'stopping') voicePhase = 'processing'
        voiceMessage = `正在识别第 ${event.chunk_index + 1} 段…`
        break
      case 'stable': {
        const transcript = stableTranscript(event)
        if (transcript) richEditor?.appendTranscript(transcript)
        voicePartial = ''
        voiceChunkIndex = event.chunk_index + 1
        if (voicePhase !== 'stopping') voicePhase = 'listening'
        voiceMessage = `第 ${event.chunk_index + 1} 段已写入正文`
        break
      }
      case 'warning':
        voiceMessage = event.message
        break
      case 'stopped':
        voicePhase = 'idle'
        voiceSessionId = ''
        voiceLevel = 0
        voicePartial = ''
        voiceMessage = '录音已停止'
        if (ramblePhase === 'active') {
          void invoke('stop_clipboard_capture')
            .catch(() => {})
            .finally(() => {
              clipboardActive = false
            })
          ramblePhase = 'error'
          rambleMessage = '麦克风意外停止，Ramble 已暂停'
        }
        break
      case 'error':
        voicePhase = 'error'
        voiceLevel = 0
        voicePartial = ''
        voiceMessage = event.message
        if (ramblePhase === 'active') {
          void invoke('stop_clipboard_capture')
            .catch(() => {})
            .finally(() => {
              clipboardActive = false
            })
          ramblePhase = 'error'
          rambleMessage = `麦克风错误，Ramble 已暂停：${event.message}`
        }
        break
    }
  }

  function resetVoiceUi() {
    voicePhase = 'idle'
    voiceRequestId = ''
    voiceSessionId = ''
    voiceDevice = ''
    voicePartial = ''
    voiceMessage = ''
    voiceLevel = 0
    voiceChunkIndex = 0
  }

  function resetRambleUi() {
    ramblePhase = 'idle'
    rambleStartedOnce = false
    rambleMessage = ''
    clipboardActive = false
    clipboardCaptureCount = 0
  }

  function formatTime(value: string | null | undefined): string {
    if (!value) return '尚未保存'
    const date = new Date(value)
    return Number.isNaN(date.getTime()) ? value : date.toLocaleString('zh-CN')
  }

  function messageFrom(cause: unknown): string {
    if (cause instanceof Error) return cause.message
    if (
      cause &&
      typeof cause === 'object' &&
      'message' in cause &&
      typeof (cause as CommandError).message === 'string'
    ) {
      return (cause as CommandError).message
    }
    return String(cause)
  }
</script>

<svelte:head>
  <title>RambleDesk · Feedback Inbox</title>
</svelte:head>

<main class="shell">
  <header class="topbar">
    <div class="brand">
      <span class="brand-mark">R</span>
      <div>
        <strong>RambleDesk</strong>
        <small>体验反馈工作台</small>
      </div>
    </div>
    <div class="topbar-actions">
      <button class="notification-button" onclick={toggleSettings}>连接设置</button>
      <button
        class:enabled={notificationState === 'enabled'}
        class="notification-button"
        disabled={notificationState !== 'disabled'}
        onclick={enableNotifications}
        title="新请求通知不会包含项目或反馈内容"
      >
        {notificationLabel(notificationState)}
      </button>
      <div class="runtime" title={endpoint}>
        <span class:online={health?.status === 'ready'}></span>
        {health?.status === 'ready' ? 'MCP 在线' : '正在连接'}
      </div>
    </div>
  </header>

  <div class="workbench">
    <aside class="inbox-panel">
      <div class="panel-heading">
        <div>
          <p class="eyebrow">{inboxMode === 'open' ? 'INBOX' : 'HISTORY'}</p>
          <h1>{inboxMode === 'open' ? '待反馈' : '历史记录'}</h1>
        </div>
        <button class="icon-button" aria-label="刷新反馈请求" onclick={refreshCurrentList}>↻</button>
      </div>

      <div class="inbox-tabs" aria-label="反馈列表范围">
        <button class:active={inboxMode === 'open'} onclick={showOpenRequests}>待处理</button>
        <button class:active={inboxMode === 'history'} onclick={showHistory}>全部历史</button>
      </div>

      {#if (inboxMode === 'open' && loadingInbox) || (inboxMode === 'history' && loadingHistory)}
        <p class="empty-state">正在读取持久请求…</p>
      {:else if displayedRequests.length === 0}
        <div class="empty-state">
          <strong>{inboxMode === 'open' ? '当前没有待处理请求' : '还没有反馈历史'}</strong>
          <span>
            {inboxMode === 'open'
              ? '保持工作台开启，Agent 的新请求会出现在这里。'
              : '创建过的请求会按最近更新时间显示在这里。'}
          </span>
        </div>
      {:else}
        <nav aria-label={inboxMode === 'open' ? '待反馈请求' : '反馈历史'}>
          {#each displayedRequests as request (request.request_id)}
            <button
              class:active={workspace?.request.request_id === request.request_id}
              class="request-card"
              onclick={() => openRequest(request.request_id)}
            >
              <span class="request-meta">
                <b>{request.project_name}</b>
                <em>{requestStatusLabel(request.status)}</em>
              </span>
              <strong>{request.what_happened}</strong>
              <small>{request.agent} · {formatTime(request.updated_at)}</small>
            </button>
          {/each}
        </nav>
      {/if}

      <div class="connection-card">
        <span>Local MCP</span>
        <code>{endpoint}</code>
      </div>
    </aside>

    <section class="workspace-panel">
      {#if loadingWorkspace}
        <div class="workspace-placeholder">正在打开反馈工作区…</div>
      {:else if workspace}
        <header class="workspace-heading">
          <div>
            <div class="workspace-meta">
              <span>{workspace.request.project_name}</span>
              <span>{workspace.request.agent}</span>
              <span>{requestStatusLabel(workspace.request.status)}</span>
            </div>
            <h2>{workspace.request.what_happened}</h2>
            <p>Session · {workspace.request.session_id}</p>
          </div>
          <button class="secondary-button" onclick={reloadWorkspace}>重新载入</button>
        </header>

        <div class="task-sheet">
          <section>
            <p class="eyebrow">WHAT TO TRY</p>
            <ol class="actions">
              {#each workspace.actions as action}
                <li>
                  <span>{action.id}</span>
                  <p>{action.instruction}</p>
                </li>
              {/each}
            </ol>
          </section>

          {#if workspace.context_refs.length > 0}
            <section class="context">
              <p class="eyebrow">CONTEXT</p>
              {#each workspace.context_refs as reference}
                <div>
                  <strong>{reference.label}</strong>
                  <code>{reference.uri}</code>
                </div>
              {/each}
            </section>
          {/if}
        </div>

        <section class:drag-active={dragActive} class="editor-section">
          <div class="editor-heading">
            <div>
              <p class="eyebrow">YOUR FEEDBACK</p>
              <h3>边体验，边记下来</h3>
            </div>
            <div class:failed={savePhase === 'error'} class="save-state" aria-live="polite">
              {#if savePhase === 'saving'}
                正在保存…
              {:else if savePhase === 'unsaved'}
                等待自动保存
              {:else if savePhase === 'error'}
                保存失败
              {:else}
                已保存 · revision {savedRevision}
              {/if}
            </div>
          </div>

          <div
            class:active={rambleActive}
            class:error={ramblePhase === 'error'}
            class="voice-toolbar"
          >
            <div class="voice-status">
              <div class="voice-title">
                <span class="voice-dot"></span>
                <strong>Ramble 状态</strong>
                <span>{rambleActive ? '收音 · 截图 · 剪贴板已激活' : '当前未采集'}</span>
                {#if voiceChunkIndex > 0}
                  <span>已写入 {voiceChunkIndex} 段</span>
                {/if}
              </div>
              <span>
                {rambleMessage || '开始后可正常操作电脑，Ramble 会在后台安静记录上下文。'}
              </span>
              {#if voicePartial}
                <em class="voice-partial">正在听：{voicePartial}</em>
              {/if}
              {#if voiceDevice}
                <small>
                  {voiceDevice}
                  · Sherpa X-ASR
                  {clipboardActive ? ' · 剪贴板监听中' : ''}
                </small>
              {/if}
              <div class="voice-meter" aria-label="麦克风音量">
                <span style={`width: ${voiceLevel * 100}%`}></span>
              </div>
            </div>
            <button
              class:recording={rambleActive}
              class="voice-button"
              disabled={rambleBusy || workspace.request.status === 'completed' || workspace.request.status === 'cancelled'}
              onclick={toggleRamble}
              title="全局快捷键 Ctrl + Shift + R"
            >
              {#if ramblePhase === 'starting'}
                正在启动…
              {:else if ramblePhase === 'stopping'}
                正在停止…
              {:else if rambleActive}
                停止 Ramble
              {:else if rambleStartedOnce}
                继续 Ramble
              {:else}
                开始 Ramble
              {/if}
            </button>
          </div>

          <RichFeedbackEditor
            bind:this={richEditor}
            markdown={draftBody}
            previews={attachmentPreviews}
            disabled={workspace.request.status === 'completed' || workspace.request.status === 'cancelled'}
            onChange={updateDraft}
          />

          <div class="attachment-toolbar">
            <div>
              <strong>Ramble 工具</strong>
              <span>
                Ramble 进行中可按 Ctrl + Shift + 1 截图；复制的文字和图片会自动追加到文档流。
              </span>
            </div>
            <button
              class="secondary-button"
              disabled={!rambleActive || attachmentBusy || workspace.request.status === 'completed' || workspace.request.status === 'cancelled'}
              onclick={startScreenCapture}
            >
              {rambleActive ? '区域截图' : '截图随 Ramble 激活'}
            </button>
            <button
              class="secondary-button"
              disabled={attachmentBusy || workspace.request.status === 'completed' || workspace.request.status === 'cancelled'}
              onclick={() => attachmentInput.click()}
            >
              从文件插入
            </button>
            <input
              bind:this={attachmentInput}
              class="visually-hidden"
              type="file"
              accept="image/png,image/jpeg,image/gif,image/webp"
              multiple
              onchange={handleFileSelection}
            />
          </div>

          {#if workspace.attachments.length > 0}
            <div class="attachment-list" aria-label="文档附件">
              {#each workspace.attachments as attachment, index (attachment.attachment_id)}
                <div class="attachment-row">
                  <span class="attachment-dot"></span>
                  <div>
                    <strong>{attachment.file_name}</strong>
                    <span>{(attachment.byte_size / 1024).toFixed(1)} KiB · 正文内图片</span>
                  </div>
                  <div class="attachment-actions">
                    <button
                      aria-label={`插入正文 ${attachment.file_name}`}
                      disabled={attachmentBusy || workspace.request.status === 'completed' || workspace.request.status === 'cancelled'}
                      onclick={() => insertExistingAttachment(attachment)}
                    >插入</button>
                    <button
                      aria-label={`上移 ${attachment.file_name}`}
                      disabled={attachmentBusy || index === 0 || workspace.request.status === 'completed' || workspace.request.status === 'cancelled'}
                      onclick={() => moveAttachment(index, -1)}
                    >↑</button>
                    <button
                      aria-label={`下移 ${attachment.file_name}`}
                      disabled={attachmentBusy || index === workspace!.attachments.length - 1 || workspace.request.status === 'completed' || workspace.request.status === 'cancelled'}
                      onclick={() => moveAttachment(index, 1)}
                    >↓</button>
                    <button
                      class="remove-attachment"
                      aria-label={`删除 ${attachment.file_name}`}
                      disabled={attachmentBusy || workspace.request.status === 'completed' || workspace.request.status === 'cancelled'}
                      onclick={() => removeAttachment(attachment)}
                    >删除</button>
                  </div>
                </div>
              {/each}
            </div>
          {/if}

          {#if attachmentMessage}
            <p class="inline-error">{attachmentMessage}</p>
          {/if}

          {#if saveMessage}
            <p class="inline-error">{saveMessage}。请重新载入后再试，当前文字仍保留在编辑器中。</p>
          {/if}

          {#if feedbackResult}
            <div class="completion-card">
              <div>
                <strong>反馈已提交</strong>
                <span>Agent 已可取得不可变 Feedback Package。</span>
                <code>{desktopPath(feedbackResult.directory_path)}</code>
              </div>
              <button class="secondary-button" onclick={openFeedbackPackage}>
                打开 Feedback Package
              </button>
            </div>
          {/if}

          <footer class="editor-footer">
            <span>
              {draftBody.length.toLocaleString()} 字符 ·
              {formatTime(workspace.draft.updated_at)}
            </span>
            <button class="primary-button" disabled={!canSubmit} onclick={submitFeedback}>
              {submitting ? '正在发布…' : '提交反馈'}
            </button>
          </footer>
        </section>
      {:else}
        <div class="workspace-placeholder">
          <span class="placeholder-mark">↙</span>
          <strong>选择一个请求开始体验</strong>
          <p>任务清单和你的 Markdown 草稿都会持久保存在本机。</p>
        </div>
      {/if}

      {#if pageError}
        <div class="error-banner" role="alert">
          <strong>工作台暂时无法完成操作</strong>
          <span>{pageError}</span>
        </div>
      {/if}
    </section>
  </div>
</main>

{#if settingsOpen}
  <div
    class="settings-backdrop"
    role="presentation"
    onclick={(event) => {
      if (event.target === event.currentTarget) settingsOpen = false
    }}
  >
    <div
      class="settings-panel"
      role="dialog"
      aria-modal="true"
      aria-labelledby="settings-title"
    >
      <header>
        <div>
          <p class="eyebrow">LOCAL MCP</p>
          <h2 id="settings-title">连接 RambleDesk</h2>
        </div>
        <button class="icon-button" aria-label="关闭连接设置" onclick={() => (settingsOpen = false)}
          >×</button
        >
      </header>
      <p>
        将这段配置加入支持 Streamable HTTP 的 MCP 客户端。访问令牌只在本机显示，请勿发送给他人。
      </p>
      <pre>{mcpConfiguration}</pre>
      <footer>
        <span class:error={copyState === 'error'}>
          {copyState === 'copied'
            ? '已复制到剪贴板'
            : copyState === 'error'
              ? '无法访问剪贴板，请手动复制'
              : '配置包含本机访问令牌'}
        </span>
        <button class="primary-button" onclick={copyMcpConfiguration}>
          {copyState === 'copied' ? '已复制' : '复制 MCP 配置'}
        </button>
      </footer>
    </div>
  </div>
{/if}
