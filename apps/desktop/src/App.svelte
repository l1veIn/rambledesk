<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { emitTo, listen } from '@tauri-apps/api/event'
  import { getCurrentWebview } from '@tauri-apps/api/webview'
  import {
    isPermissionGranted,
    requestPermission,
    sendNotification,
  } from '@tauri-apps/plugin-notification'
  import { revealItemInDir } from '@tauri-apps/plugin-opener'
  import { ArrowUpRight } from '@lucide/svelte'
  import { onMount, tick } from 'svelte'

  import rambelleArchived from './assets/rambelle-states/archived.png'
  import rambelleIdle from './assets/rambelle-states/idle.png'
  import rambelleOrganizing from './assets/rambelle-states/organizing.png'
  import rambelleRecording from './assets/rambelle-states/recording.png'
  import AppTitlebar from './lib/AppTitlebar.svelte'
  import RichFeedbackEditor from './lib/RichFeedbackEditor.svelte'
  import SettingsPanel from './lib/SettingsPanel.svelte'
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
    notificationStateForPermission,
    type NotificationState,
  } from './lib/notifications'
  import { desktopPath } from './lib/nativePath'
  import {
    RAMBLE_CONSOLE_COMMAND_EVENT,
    RAMBLE_CONSOLE_HIDE_EVENT,
    RAMBLE_CONSOLE_READY_EVENT,
    RAMBLE_CONSOLE_SHOW_EVENT,
    RAMBLE_CONSOLE_STATE_EVENT,
    type RambleConsoleCommand,
    type RambleConsoleState,
  } from './lib/rambleConsole'
  import {
    eventBelongsToVoiceSession,
    stableTranscript,
    type SpeechEvent,
    type VoiceRambleSessionView,
  } from './lib/speech'
  import type { ScreenCaptureReady } from './lib/screenCapture'
  import { t } from './lib/i18n'
  import { locale } from './lib/preferences'

  type SavePhase = 'idle' | 'unsaved' | 'saving' | 'saved' | 'error'
  type RamblePhase = 'idle' | 'starting' | 'active' | 'paused' | 'stopping' | 'error'
  type VoicePhase = 'idle' | 'starting' | 'listening' | 'processing' | 'stopping' | 'error'
  type CommandError = { code: string; message: string; retryable: boolean }
  type SettingsSection = 'general' | 'mcp'

  let health: HealthSnapshot | null = null
  let endpoint = tr('正在连接…')
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
  let settingsSection: SettingsSection = 'general'
  const isTauri = '__TAURI_INTERNALS__' in window
  let taskBriefOpen = true
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
  let rambleContextId = ''
  let rambleMessage = ''
  let clipboardCaptureCount = 0
  let clipboardImageQueue: Promise<void> = Promise.resolve()
  let saveTimer: ReturnType<typeof setTimeout> | undefined
  let inboxTimer: ReturnType<typeof setInterval> | undefined
  let activeSave: Promise<boolean> | null = null
  const notificationTracker = new InboxNotificationTracker()
  const NOTIFICATION_PREFERENCE_KEY = 'rambledesk.notifications.enabled'

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

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
  $: rambleEngaged = ramblePhase !== 'idle'
  $: rambelleStatusPortrait = feedbackResult
    ? rambelleArchived
    : rambleActive
      ? rambelleRecording
      : rambleEngaged
        ? rambelleOrganizing
        : rambelleIdle
  $: rambleBusy = ramblePhase === 'starting' || ramblePhase === 'stopping'
  $: rambleCanStop = rambleActive || voiceCanStop
  $: rambleCanExit = rambleEngaged || voiceCanStop
  $: if (rambleEngaged && workspace) {
    const consoleState: RambleConsoleState = {
      phase:
        ramblePhase === 'active'
          ? 'recording'
          : ramblePhase === 'idle'
            ? 'paused'
            : ramblePhase,
      projectName: workspace.request.project_name,
      requestTitle: workspace.request.what_happened,
      recording: rambleActive,
      busy: rambleBusy,
      captureBusy: attachmentBusy,
      voiceLevel,
      partialTranscript: voicePartial,
      message: rambleMessage,
    }
    void emitTo('ramble-console', RAMBLE_CONSOLE_STATE_EVENT, consoleState).catch(() => {})
  }

  onMount(() => {
    if (!isTauri) {
      loadingInbox = false
      notificationState = 'unavailable'
      window.addEventListener('paste', handlePaste)
      return () => window.removeEventListener('paste', handlePaste)
    }
    void initialize()
    void refreshNotificationPermission()
    inboxTimer = setInterval(() => void refreshInbox(), 5_000)
    let dragUnlisten: (() => void) | undefined
    let voiceUnlisten: (() => void) | undefined
    let rambleShortcutUnlisten: (() => void) | undefined
    let captureShortcutUnlisten: (() => void) | undefined
    let captureReadyUnlisten: (() => void) | undefined
    let captureCancelledUnlisten: (() => void) | undefined
    let consoleCommandUnlisten: (() => void) | undefined
    let consoleReadyUnlisten: (() => void) | undefined
    void listen<SpeechEvent>('voice-ramble-event', (event) => {
      handleVoiceEvent(event.payload)
    })
      .then((unlisten) => {
        voiceUnlisten = unlisten
      })
      .catch((cause) => {
        voicePhase = 'error'
        voiceMessage = tr('无法监听语音识别事件：{error}', { error: messageFrom(cause) })
      })
    void listen<string>('screen-capture-shortcut', () => {
      if (rambleEngaged) void startScreenCapture()
    })
      .then((unlisten) => {
        captureShortcutUnlisten = unlisten
      })
      .catch((cause) => {
        attachmentMessage = tr('无法监听截图快捷键：{error}', { error: messageFrom(cause) })
      })
    void listen<string>('ramble-toggle-shortcut', () => {
      void toggleRamble()
    })
      .then((unlisten) => {
        rambleShortcutUnlisten = unlisten
      })
      .catch((cause) => {
        ramblePhase = 'error'
        rambleMessage = tr('无法监听 Ramble 快捷键：{error}', { error: messageFrom(cause) })
      })
    void listen<ScreenCaptureReady>('screen-capture-ready', (event) => {
      void importScreenCapture(event.payload)
    })
      .then((unlisten) => {
        captureReadyUnlisten = unlisten
      })
      .catch((cause) => {
        attachmentMessage = tr('无法接收截图结果：{error}', { error: messageFrom(cause) })
      })
    void listen('screen-capture-cancelled', () => {
      attachmentBusy = false
      attachmentMessage = tr('截图已取消')
    })
      .then((unlisten) => {
        captureCancelledUnlisten = unlisten
      })
      .catch(() => {
        // A failed cancellation listener does not affect capture or attachment storage.
      })
    void listen<RambleConsoleCommand>(RAMBLE_CONSOLE_COMMAND_EVENT, (event) => {
      void handleRambleConsoleCommand(event.payload)
    }).then((unlisten) => {
      consoleCommandUnlisten = unlisten
    })
    void listen(RAMBLE_CONSOLE_READY_EVENT, () => {
      broadcastRambleConsoleState()
    }).then((unlisten) => {
      consoleReadyUnlisten = unlisten
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
        attachmentMessage = tr('当前窗口无法监听文件拖放，请使用文件选择或粘贴。')
      })
    window.addEventListener('paste', handlePaste)
    return () => {
      if (saveTimer) clearTimeout(saveTimer)
      if (inboxTimer) clearInterval(inboxTimer)
      dragUnlisten?.()
      voiceUnlisten?.()
      rambleShortcutUnlisten?.()
      captureShortcutUnlisten?.()
      captureReadyUnlisten?.()
      captureCancelledUnlisten?.()
      consoleCommandUnlisten?.()
      consoleReadyUnlisten?.()
      if (voiceCanStop) void invoke('stop_voice_ramble')
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
            ? tr('新的体验反馈请求已到达。打开工作台查看。')
            : tr('{count} 个新的体验反馈请求已到达。打开工作台查看。', {
                count: arrivals.length,
              }),
      })
    }
  }

  async function refreshNotificationPermission() {
    try {
      const granted = await isPermissionGranted()
      const preferred = localStorage.getItem(NOTIFICATION_PREFERENCE_KEY) !== 'false'
      notificationState = notificationStateForPermission(granted, preferred)
    } catch {
      notificationState = 'unavailable'
    }
  }

  async function toggleNotifications() {
    if (notificationState === 'checking' || notificationState === 'unavailable') return
    if (notificationState === 'enabled') {
      localStorage.setItem(NOTIFICATION_PREFERENCE_KEY, 'false')
      notificationState = 'muted'
      return
    }
    notificationState = 'checking'
    try {
      const permission = (await isPermissionGranted()) ? 'granted' : await requestPermission()
      if (permission === 'granted') {
        localStorage.setItem(NOTIFICATION_PREFERENCE_KEY, 'true')
        notificationState = 'enabled'
      } else {
        notificationState = 'disabled'
      }
    } catch {
      notificationState = 'unavailable'
    }
  }

  async function openRequest(requestId: string, saveCurrent = true) {
    if (workspace?.request.request_id === requestId) return
    if (rambleCanExit) await exitRamble()
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
    if (rambleCanExit) await exitRamble()
    if (dirty && !(await saveDraftNow())) return
    workspace = null
    await openRequest(requestId, false)
  }

  async function openSettings(section: SettingsSection) {
    settingsSection = section
    settingsOpen = true
    pageError = ''
    await tick()
    if (!isTauri) return
    try {
      mcpConfiguration = await invoke<string>('get_mcp_configuration')
    } catch (cause) {
      pageError = messageFrom(cause)
    }
  }

  async function toggleSettings() {
    if (settingsOpen) {
      settingsOpen = false
      return
    }
    await openSettings('general')
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
          throw new Error(tr('{name} 超过 20 MiB 限制', { name: file.name }))
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
        throw new Error(tr('附件已保存，但编辑器未能在当前光标位置插入图片'))
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
      !rambleEngaged ||
      attachmentBusy ||
      workspace.request.status === 'completed' ||
      workspace.request.status === 'cancelled'
    ) {
      return
    }
    if (!(await saveDraftNow())) return
    attachmentBusy = true
    attachmentMessage = tr('截图工具已唤起：拖动框选，Esc 或右键取消')
    try {
      await invoke('begin_screen_capture')
    } catch (cause) {
      attachmentBusy = false
      const message = messageFrom(cause)
      attachmentMessage =
        message === '内置区域截图目前只在 Windows 开发环境启用' ? tr(message) : message
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
        throw new Error(tr('当前草稿无法保存，截图尚未写入'))
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
        throw new Error(tr('截图附件已保存，但编辑器未能在当前光标位置插入图片'))
      }
      await saveDraftNow()
      attachmentMessage = tr('截图已自动插入当前文档位置')
    } catch (cause) {
      attachmentMessage = tr('截图写入失败：{error}', { error: messageFrom(cause) })
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
    if (rambleCanExit) await exitRamble()
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
      pageError = tr('无法打开 Feedback Package：{error}', { error: messageFrom(cause) })
    }
  }

  async function startRamble() {
    if (
      !workspace ||
      rambleBusy ||
      rambleEngaged ||
      workspace.request.status === 'completed' ||
      workspace.request.status === 'cancelled'
    ) {
      return
    }
    rambleStartedOnce = true
    rambleContextId = crypto.randomUUID()
    clipboardCaptureCount = 0
    ramblePhase = 'starting'
    rambleMessage = tr('正在打开 Ramble 操作台…')
    void emitTo('ramble-console', RAMBLE_CONSOLE_SHOW_EVENT).catch((cause) => {
      pageError = tr('无法打开 Ramble 操作台：{error}', { error: messageFrom(cause) })
    })
    await resumeRamble()
  }

  async function resumeRamble() {
    if (!workspace || rambleBusy || rambleActive || !rambleContextId) return
    const requestId = workspace.request.request_id
    ramblePhase = 'starting'
    rambleMessage = tr('正在启动麦克风与实时转写…')
    const voiceStarted = await startVoiceRamble()
    if (!voiceStarted || !voiceSessionId) {
      ramblePhase = 'error'
      rambleMessage = voiceMessage || tr('麦克风启动失败')
      return
    }

    if (workspace?.request.request_id !== requestId) {
      await invoke('stop_voice_ramble').catch(() => {})
      resetVoiceUi()
      await exitRamble()
      return
    }
    ramblePhase = 'active'
    rambleMessage = tr('Ramble 进行中 · 剪贴板仅在点击导入时读取')
  }

  async function stopRamble() {
    if (!rambleCanStop || ramblePhase === 'stopping') return
    ramblePhase = 'stopping'
    rambleMessage = tr('正在收尾最后一段语音并暂停记录…')
    let stopError = ''
    if (voiceCanStop) {
      const voiceStopped = await stopVoiceRamble()
      if (!voiceStopped && !stopError) stopError = voiceMessage || tr('麦克风停止失败')
    }
    if (stopError) {
      ramblePhase = 'error'
      rambleMessage = stopError
    } else {
      ramblePhase = 'paused'
      rambleMessage = tr('Ramble 已暂停；正文保留，截图和导入仍可使用')
    }
  }

  async function exitRamble() {
    if (!rambleCanExit && !rambleStartedOnce) return
    if (voiceCanStop) {
      ramblePhase = 'stopping'
      rambleMessage = tr('正在结束 Ramble…')
      await stopVoiceRamble()
    }
    void emitTo('ramble-console', RAMBLE_CONSOLE_HIDE_EVENT).catch(() => {})
    resetVoiceUi()
    resetRambleUi()
  }

  async function toggleRamble() {
    if (rambleBusy) return
    if (rambleActive || voiceCanStop) await stopRamble()
    else if (rambleEngaged) await resumeRamble()
    else await startRamble()
  }

  async function importClipboardNow() {
    if (!workspace || !rambleEngaged || !rambleContextId || attachmentBusy) return
    attachmentMessage = ''
    try {
      const event = await invoke<ClipboardCaptureEvent>('capture_clipboard_once', {
        input: {
          request_id: workspace.request.request_id,
          ramble_session_id: rambleContextId,
        },
      })
      handleClipboardCaptureEvent(event)
    } catch (cause) {
      attachmentMessage = tr('无法导入剪贴板：{error}', { error: messageFrom(cause) })
    }
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
    voiceMessage = tr('正在加载本地模型并连接麦克风…')
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
        voiceMessage = tr('Sherpa 真流式识别 · 自然停顿后写入正文')
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
    voiceMessage = tr('正在完成最后一段识别…')
    try {
      await invoke('stop_voice_ramble')
      for (let attempt = 0; attempt < 5 && voicePhase === 'stopping'; attempt += 1) {
        await new Promise((resolve) => setTimeout(resolve, 20))
      }
      await tick()
      if (voicePhase === 'stopping') {
        voicePhase = 'idle'
        voiceMessage = tr('录音已停止')
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
      !rambleEngaged ||
      !workspace ||
      !eventBelongsToRamble(
        event,
        workspace.request.request_id,
        rambleContextId,
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
        clipboardCaptureLabel(event.captured_at_ms, event.truncated, $locale),
      )
      if (inserted) {
        clipboardCaptureCount += 1
        rambleMessage = tr('Ramble 进行中 · 已捕获 {count} 项剪贴板上下文', { count: clipboardCaptureCount })
      }
      return
    }

    clipboardImageQueue = clipboardImageQueue
      .then(() => importClipboardImage(event))
      .catch((cause) => {
        attachmentMessage = tr('剪贴板图片写入失败：{error}', { error: messageFrom(cause) })
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
      if (attachmentBusy) throw new Error(tr('附件通道正忙，请稍后重新复制图片'))
      if (!workspace || workspace.request.request_id !== requestId) return
      if (!(await saveDraftNow())) throw new Error(tr('当前草稿无法保存'))

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
          clipboardCaptureLabel(event.captured_at_ms, false, $locale),
        )
      ) {
        throw new Error(tr('图片附件已保存，但未能写入文档流'))
      }
      await saveDraftNow()
      clipboardCaptureCount += 1
      rambleMessage = tr('Ramble 进行中 · 已捕获 {count} 项剪贴板上下文', { count: clipboardCaptureCount })
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
        voiceMessage = tr('正在录音 · {device}', { device: event.input_device })
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
        voiceMessage = tr('正在识别第 {count} 段…', { count: event.chunk_index + 1 })
        break
      case 'stable': {
        const transcript = stableTranscript(event)
        if (transcript) richEditor?.appendTranscript(transcript)
        voicePartial = ''
        voiceChunkIndex = event.chunk_index + 1
        if (voicePhase !== 'stopping') voicePhase = 'listening'
        voiceMessage = tr('第 {count} 段已写入正文', { count: event.chunk_index + 1 })
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
        voiceMessage = tr('录音已停止')
        if (ramblePhase === 'active') {
          ramblePhase = 'error'
          rambleMessage = tr('麦克风意外停止，Ramble 已暂停')
        }
        break
      case 'error':
        voicePhase = 'error'
        voiceLevel = 0
        voicePartial = ''
        voiceMessage = event.message
        if (ramblePhase === 'active') {
          ramblePhase = 'error'
          rambleMessage = tr('麦克风错误，Ramble 已暂停：{error}', { error: event.message })
        }
        break
    }
  }

  async function handleRambleConsoleCommand(command: RambleConsoleCommand) {
    switch (command.type) {
      case 'toggle-recording':
        await toggleRamble()
        break
      case 'capture-screen':
        await startScreenCapture()
        break
      case 'import-clipboard':
        await importClipboardNow()
        break
      case 'import-files':
        await importAttachmentPaths(command.paths)
        break
      case 'exit':
        await exitRamble()
        break
    }
  }

  function broadcastRambleConsoleState() {
    if (!rambleEngaged || !workspace) return
    const state: RambleConsoleState = {
      phase:
        ramblePhase === 'active'
          ? 'recording'
          : ramblePhase === 'idle'
            ? 'paused'
            : ramblePhase,
      projectName: workspace.request.project_name,
      requestTitle: workspace.request.what_happened,
      recording: rambleActive,
      busy: rambleBusy,
      captureBusy: attachmentBusy,
      voiceLevel,
      partialTranscript: voicePartial,
      message: rambleMessage,
    }
    void emitTo('ramble-console', RAMBLE_CONSOLE_STATE_EVENT, state).catch(() => {})
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
    rambleContextId = ''
    rambleMessage = ''
    clipboardCaptureCount = 0
  }

  function formatTime(value: string | null | undefined): string {
    if (!value) return tr('尚未保存')
    const date = new Date(value)
    return Number.isNaN(date.getTime()) ? value : date.toLocaleString($locale)
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

{#key $locale}
<main class="shell">
  <AppTitlebar
    projectName={workspace?.request.project_name ?? 'Vault Zero Archive'}
    connected={health?.status === 'ready'}
    pendingCount={inbox.length}
    notificationText={notificationLabel(notificationState, $locale)}
    notificationEnabled={notificationState === 'enabled'}
    notificationDisabled={notificationState === 'checking' || notificationState === 'unavailable'}
    onSettings={toggleSettings}
    onNotifications={toggleNotifications}
    onWindowError={(message) => (pageError = tr('窗口操作失败：{error}', { error: message }))}
  />

  <div class="workbench">
    <aside class="inbox-panel">
      <div class="panel-heading">
        <div>
          <p class="eyebrow">{inboxMode === 'open' ? 'INBOX' : 'HISTORY'}</p>
          <h1>{inboxMode === 'open' ? tr('待反馈') : tr('历史记录')}</h1>
        </div>
        <button class="icon-button" aria-label={tr('刷新反馈请求')} onclick={refreshCurrentList}>↻</button>
      </div>

      <div class="inbox-tabs" aria-label={tr('反馈列表范围')}>
        <button class:active={inboxMode === 'open'} onclick={showOpenRequests}>{tr('待处理')}</button>
        <button class:active={inboxMode === 'history'} onclick={showHistory}>{tr('全部历史')}</button>
      </div>

      {#if (inboxMode === 'open' && loadingInbox) || (inboxMode === 'history' && loadingHistory)}
        <p class="empty-state">{tr('正在读取持久请求…')}</p>
      {:else if displayedRequests.length === 0}
        <div class="empty-state">
          <strong>{inboxMode === 'open' ? tr('当前没有待处理请求') : tr('还没有反馈历史')}</strong>
          <span>
            {inboxMode === 'open'
              ? tr('保持工作台开启，Agent 的新请求会出现在这里。')
              : tr('创建过的请求会按最近更新时间显示在这里。')}
          </span>
        </div>
      {:else}
        <nav aria-label={inboxMode === 'open' ? tr('待反馈请求') : tr('反馈历史')}>
          {#each displayedRequests as request (request.request_id)}
            <button
              class:active={workspace?.request.request_id === request.request_id}
              class="request-card"
              onclick={() => openRequest(request.request_id)}
            >
              <span class="request-meta">
                <b>{request.project_name}</b>
                <em>{requestStatusLabel(request.status, $locale)}</em>
              </span>
              <strong>{request.what_happened}</strong>
              <small>{request.agent} · {formatTime(request.updated_at)}</small>
            </button>
          {/each}
        </nav>
      {/if}

      <button
        type="button"
        class="connection-card"
        aria-label={tr('打开 MCP 设置')}
        onclick={() => void openSettings('mcp')}
      >
        <span>Local MCP</span>
        <code>{endpoint}</code>
        <ArrowUpRight size={15} strokeWidth={1.7} />
      </button>
    </aside>

    <section class="workspace-panel">
      {#if loadingWorkspace}
        <div class="workspace-placeholder">{tr('正在打开反馈工作区…')}</div>
      {:else if workspace}
        <div class="workspace-stage">
          <header class="workspace-heading">
            <div class="workspace-heading-copy">
              <div class="workspace-meta">
                <span>{workspace.request.project_name}</span>
                <span>{workspace.request.agent}</span>
                <span class="status-chip">{requestStatusLabel(workspace.request.status, $locale)}</span>
              </div>
              <h2>{workspace.request.what_happened}</h2>
              <p>Session · {workspace.request.session_id}</p>
            </div>
            <button class="secondary-button compact-button" onclick={reloadWorkspace}>{tr('重新载入')}</button>
          </header>

          <div class="workspace-columns">
            <div class="document-column">
              <section class:open={taskBriefOpen} class="task-sheet">
                <button
                  class="task-sheet-toggle"
                  aria-expanded={taskBriefOpen}
                  onclick={() => (taskBriefOpen = !taskBriefOpen)}
                >
                  <span>
                    <i>01</i>
                    <strong>{tr('任务简报')}</strong>
                    <em>{tr('{count} 个体验步骤', { count: workspace.actions.length })}</em>
                  </span>
                  <b>{taskBriefOpen ? tr('收起') : tr('展开')}⌄</b>
                </button>

                {#if taskBriefOpen}
                  <div class="task-sheet-body">
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
                {/if}
              </section>

              <section class:drag-active={dragActive} class="editor-section">
                <div class="editor-heading">
                  <div>
                    <p class="eyebrow">YOUR FEEDBACK</p>
                    <h3>{tr('边体验，边记下来')}</h3>
                  </div>
                  <div class:failed={savePhase === 'error'} class="save-state" aria-live="polite">
                    <span class="save-dot"></span>
                    {#if savePhase === 'saving'}
                      {tr('正在保存…')}
                    {:else if savePhase === 'unsaved'}
                      {tr('等待自动保存')}
                    {:else if savePhase === 'error'}
                      {tr('保存失败')}
                    {:else}
                      {tr('已保存')} · revision {savedRevision}
                    {/if}
                  </div>
                </div>

                <RichFeedbackEditor
                  bind:this={richEditor}
                  markdown={draftBody}
                  previews={attachmentPreviews}
                  disabled={workspace.request.status === 'completed' || workspace.request.status === 'cancelled'}
                  onChange={updateDraft}
                />

                {#if attachmentMessage}
                  <p class="inline-error">{attachmentMessage}</p>
                {/if}

                {#if saveMessage}
                  <p class="inline-error">{saveMessage}。{tr('请重新载入后再试，当前文字仍保留在编辑器中。')}</p>
                {/if}

                <footer class="editor-footer">
                  <span>{tr('{count} 字符', { count: draftBody.length.toLocaleString($locale) })}</span>
                  <span>{tr('Markdown 文档流')}</span>
                  <span>{formatTime(workspace.draft.updated_at)}</span>
                </footer>
              </section>
            </div>

            <aside class="command-rail" aria-label={tr('Ramble 操作台')}>
              <section
                class:active={rambleEngaged}
                class:error={ramblePhase === 'error'}
                class="ramble-console"
              >
                <div class="rail-heading">
                  <div>
                    <p class="eyebrow">RAMBLE</p>
                    <strong>{rambleActive ? tr('正在记录') : rambleEngaged ? tr('Ramble 已暂停') : tr('记录待命')}</strong>
                  </div>
                  <span class="ramble-led"></span>
                </div>

                <button
                  class:recording={rambleActive}
                  class="ramble-primary"
                  disabled={rambleBusy || workspace.request.status === 'completed' || workspace.request.status === 'cancelled'}
                  onclick={toggleRamble}
                  title={tr('全局快捷键 Ctrl + Shift + R')}
                >
                  <span>{rambleActive ? 'Ⅱ' : '●'}</span>
                  {#if ramblePhase === 'starting'}
                    {tr('正在启动…')}
                  {:else if ramblePhase === 'stopping'}
                    {tr('正在暂停…')}
                  {:else if rambleActive}
                    {tr('暂停 Ramble')}
                  {:else if rambleStartedOnce}
                    {tr('继续 Ramble')}
                  {:else}
                    {tr('开始 Ramble')}
                  {/if}
                </button>

                {#if rambleEngaged}
                  <button class="ramble-exit" disabled={rambleBusy} onclick={exitRamble}>
                    {tr('退出 Ramble 操作台')}
                  </button>
                {/if}

                <div class="voice-status">
                  <div class="voice-title">
                    <span class="voice-dot"></span>
                    <strong>{voiceDevice || tr('默认麦克风')}</strong>
                    {#if voiceChunkIndex > 0}<span>{tr('{count} 段', { count: voiceChunkIndex })}</span>{/if}
                  </div>
                  <span>{rambleMessage || tr('开始后可离开窗口继续操作，录音会实时写入正文。')}</span>
                  {#if voicePartial}
                    <em class="voice-partial">{tr('正在听：{text}', { text: voicePartial })}</em>
                  {/if}
                  <small>{tr('Sherpa X-ASR · 本地流式转写')}</small>
                  <div class="voice-meter" aria-label={tr('麦克风音量')}>
                    <span style={`width: ${voiceLevel * 100}%`}></span>
                  </div>
                </div>
              </section>

              <section class="tool-card">
                <div class="rail-heading">
                  <div>
                    <p class="eyebrow">CAPTURE</p>
                    <strong>{tr('添加上下文')}</strong>
                  </div>
                  <span>{workspace.attachments.length}</span>
                </div>
                <div class="tool-grid">
                  <button
                    disabled={!rambleEngaged || attachmentBusy || workspace.request.status === 'completed' || workspace.request.status === 'cancelled'}
                    onclick={startScreenCapture}
                    title="Ctrl + Shift + 1"
                  >
                    <span class="tool-icon">⌗</span>
                    <strong>{tr('截图')}</strong>
                    <small>{tr('区域捕获')}</small>
                  </button>
                  <button
                    disabled={!rambleEngaged || attachmentBusy || workspace.request.status === 'completed' || workspace.request.status === 'cancelled'}
                    onclick={importClipboardNow}
                  >
                    <span class="tool-icon">▣</span>
                    <strong>{tr('剪贴板')}</strong>
                    <small>{tr('显式导入')}</small>
                  </button>
                  <button
                    disabled={attachmentBusy || workspace.request.status === 'completed' || workspace.request.status === 'cancelled'}
                    onclick={() => attachmentInput.click()}
                  >
                    <span class="tool-icon">＋</span>
                    <strong>{tr('文件')}</strong>
                    <small>{tr('选择或拖入')}</small>
                  </button>
                </div>
                <input
                  bind:this={attachmentInput}
                  class="visually-hidden"
                  type="file"
                  multiple
                  onchange={handleFileSelection}
                />
                <p class="tool-hint">{tr('不会监听剪贴板；只有点击导入时才读取一次当前内容。')}</p>
              </section>

              <section class="attachments-card">
                <div class="rail-heading">
                  <div>
                    <p class="eyebrow">ATTACHMENTS</p>
                    <strong>{tr('文档素材')}</strong>
                  </div>
                  <span>{workspace.attachments.length}</span>
                </div>
                {#if workspace.attachments.length > 0}
                  <div class="attachment-list" aria-label={tr('文档附件')}>
                    {#each workspace.attachments as attachment, index (attachment.attachment_id)}
                      <div class="attachment-row">
                        <span class="attachment-dot"></span>
                        <div>
                          <strong>{attachment.file_name}</strong>
                          <span>{(attachment.byte_size / 1024).toFixed(1)} KiB</span>
                        </div>
                        <div class="attachment-actions">
                          <button
                            aria-label={tr('插入正文 {name}', { name: attachment.file_name })}
                            disabled={attachmentBusy || workspace.request.status === 'completed' || workspace.request.status === 'cancelled'}
                            onclick={() => insertExistingAttachment(attachment)}
                          >{tr('插入')}</button>
                          <button
                            class="remove-attachment"
                            aria-label={tr('删除 {name}', { name: attachment.file_name })}
                            disabled={attachmentBusy || workspace.request.status === 'completed' || workspace.request.status === 'cancelled'}
                            onclick={() => removeAttachment(attachment)}
                          >×</button>
                        </div>
                      </div>
                    {/each}
                  </div>
                {:else}
                  <p class="rail-empty">{tr('截图和导入的文件会直接进入正文，也会在这里留档。')}</p>
                {/if}
              </section>

              <section class="rambelle-note" aria-label={tr('Rambelle 状态')}>
                <div>
                  <span>RAMBELLE · ONLINE</span>
                  <p>
                    {feedbackResult
                      ? tr('记录在案了，长官。')
                      : rambleEngaged
                        ? tr('我正在整理这次 Ramble。')
                        : tr('档案已就绪，随时可以继续。')}
                  </p>
                </div>
                <img src={rambelleStatusPortrait} alt="Rambelle" />
              </section>

              <section class="delivery-card">
                <div class="delivery-status">
                  <span class:ready={canSubmit}></span>
                  <div>
                    <strong>{feedbackResult ? tr('反馈包已归档') : 'Feedback Package'}</strong>
                    <small>{feedbackResult ? tr('Agent 已可读取不可变结果') : tr('正文保存后即可提交')}</small>
                  </div>
                </div>
                {#if feedbackResult}
                  <code>{desktopPath(feedbackResult.directory_path)}</code>
                  <button class="package-button" onclick={openFeedbackPackage}>{tr('打开 Feedback Package')}</button>
                {:else}
                  <button class="primary-button wide-button" disabled={!canSubmit} onclick={submitFeedback}>
                    {submitting ? tr('正在发布…') : tr('提交反馈')}
                  </button>
                {/if}
              </section>
            </aside>
          </div>
        </div>
      {:else}
        <div class="workspace-placeholder">
          <span class="placeholder-mark">↙</span>
          <strong>{tr('选择一个请求开始体验')}</strong>
          <p>{tr('任务清单和你的 Markdown 草稿都会持久保存在本机。')}</p>
        </div>
      {/if}

      {#if pageError}
        <div class="error-banner" role="alert">
          <strong>{tr('工作台暂时无法完成操作')}</strong>
          <span>{pageError}</span>
        </div>
      {/if}
    </section>
  </div>
</main>

{#if settingsOpen}
  <SettingsPanel {mcpConfiguration} initialSection={settingsSection} onClose={() => (settingsOpen = false)} />
{/if}
{/key}
