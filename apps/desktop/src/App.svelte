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

  import rambelleArchived from './assets/rambelle-states/archived.png'
  import rambelleIdle from './assets/rambelle-states/idle.png'
  import rambelleOrganizing from './assets/rambelle-states/organizing.png'
  import rambelleRecording from './assets/rambelle-states/recording.png'
  import AppTitlebar from './lib/AppTitlebar.svelte'
  import SettingsPanel from './lib/SettingsPanel.svelte'
  import InboxPanel from './lib/workbench/InboxPanel.svelte'
  import ResumePromptDialog from './lib/workbench/ResumePromptDialog.svelte'
  import WorkspacePanel from './lib/workbench/WorkspacePanel.svelte'
  import type {
    AddAttachmentInput,
    AttachmentView,
    CancelFeedbackInput,
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
  import {
    InboxNotificationTracker,
    notificationLabel,
    notificationStateForPermission,
    type NotificationState,
  } from './lib/notifications'
  import { desktopPath } from './lib/nativePath'
  import type {
    AdapterPresentation,
    FeedbackEditorHandle,
    RamblePhase,
    RambleSessionControllerHandle,
    ResumePrompt,
    SavePhase,
    SettingsSection,
    VoicePhase,
  } from './lib/workbench/types'
  import RambleSessionController from './lib/workbench/RambleSessionController.svelte'
  import type { ScreenCaptureReady } from './lib/screenCapture'
  import { t } from './lib/i18n'
  import { locale } from './lib/preferences'

  type ScreenCaptureFinished = {
    session_id: string | null
    outcome: 'cancelled' | 'pinned'
  }

  type CommandError = { code: string; message: string; retryable: boolean }

  const RESUME_PROMPT_EVENT = 'rambledesk://resume-prompt'

  let inbox: FeedbackRequestSummary[] = []
  let history: FeedbackRequestSummary[] = []
  let adapterPresentations: Record<string, AdapterPresentation> = {}
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
  let cancelling = false
  let attachmentBusy = false
  let attachmentMessage = ''
  let attachmentPreviews: Record<string, string> = {}
  let dragActive = false
  let workspacePanel: FeedbackEditorHandle
  let rambleController: RambleSessionControllerHandle
  let resumePrompt: ResumePrompt | null = null
  let resumeCopyState: 'idle' | 'copied' | 'failed' = 'idle'
  let notificationState: NotificationState = 'checking'
  let settingsOpen = false
  let settingsSection: SettingsSection = 'general'
  const isTauri = '__TAURI_INTERNALS__' in window
  let taskBriefOpen = true
  let mcpConfiguration = ''
  let voicePhase: VoicePhase = 'idle'
  let voiceDevice = ''
  let voicePartial = ''
  let voiceLevel = 0
  let voiceChunkIndex = 0
  let ramblePhase: RamblePhase = 'idle'
  let rambleStartedOnce = false
  let rambleMessage = ''
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
    !submitting &&
    !cancelling
  $: canCancel =
    workspace !== null &&
    workspace.request.status !== 'completed' &&
    workspace.request.status !== 'cancelled' &&
    !submitting &&
    !cancelling
  $: voiceActive =
    voicePhase === 'starting' ||
    voicePhase === 'listening' ||
    voicePhase === 'processing' ||
    voicePhase === 'stopping'
  $: voiceCanStop =
    voiceActive || voicePhase === 'error'
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
    let captureReadyUnlisten: (() => void) | undefined
    let captureFinishedUnlisten: (() => void) | undefined
    let resumePromptUnlisten: (() => void) | undefined
    void listen<ResumePrompt>(RESUME_PROMPT_EVENT, (event) => {
      resumePrompt = event.payload
      resumeCopyState = 'idle'
      if (notificationState === 'enabled') {
        sendNotification({
          title: event.payload.title,
          body: tr('请回到 {host}，用恢复提示继续 Agent。', {
            host: event.payload.host_label,
          }),
        })
      }
    })
      .then((unlisten) => {
        resumePromptUnlisten = unlisten
      })
      .catch(() => {
        // Resume prompt still appears if submit path keeps the main window focused.
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
    void listen<ScreenCaptureFinished>('screen-capture-finished', (event) => {
      attachmentBusy = false
      attachmentMessage =
        event.payload.outcome === 'pinned' ? tr('截图已固定到屏幕') : tr('截图已取消')
    })
      .then((unlisten) => {
        captureFinishedUnlisten = unlisten
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
        attachmentMessage = tr('当前窗口无法监听文件拖放，请使用文件选择或粘贴。')
      })
    window.addEventListener('paste', handlePaste)
    return () => {
      if (saveTimer) clearTimeout(saveTimer)
      if (inboxTimer) clearInterval(inboxTimer)
      dragUnlisten?.()
      captureReadyUnlisten?.()
      captureFinishedUnlisten?.()
      resumePromptUnlisten?.()
      window.removeEventListener('paste', handlePaste)
      releaseAttachmentPreviews()
    }
  })

  async function copyResumePrompt() {
    if (!resumePrompt) return
    try {
      await navigator.clipboard.writeText(resumePrompt.resume_prompt)
      resumeCopyState = 'copied'
      window.setTimeout(() => {
        if (resumeCopyState === 'copied') resumeCopyState = 'idle'
      }, 2_000)
    } catch {
      resumeCopyState = 'failed'
    }
  }

  function dismissResumePrompt() {
    resumePrompt = null
    resumeCopyState = 'idle'
  }

  async function initialize() {
    pageError = ''
    loadingInbox = true
    try {
      const [nextInbox, presentations] = await Promise.all([
        invoke<FeedbackRequestSummary[]>('list_feedback_inbox'),
        invoke<AdapterPresentation[]>('list_adapter_presentations'),
      ])
      adapterPresentations = Object.fromEntries(
        presentations.map((presentation) => [presentation.id, presentation]),
      )
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

  function adapterPresentation(hostId: string): AdapterPresentation {
    const normalized = hostId.trim().toLowerCase()
    const presentation = adapterPresentations[normalized]
    if (presentation) return presentation
    return {
      id: normalized || 'generic',
      label: hostId.trim() || adapterPresentations.generic?.label || 'Coding Agent',
      icon_svg: adapterPresentations.generic?.icon_svg || '',
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
      const inserted = workspacePanel?.insertAttachments(
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
      workspacePanel?.insertAttachments(
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
    attachmentMessage = tr('高级截图已唤起：可智能选窗、标注、滚动截图或固定到屏幕')
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
      const inserted = workspacePanel?.insertAttachments(
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
    workspacePanel?.removeAttachmentReference(attachment.attachment_id)
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
    workspacePanel?.insertAttachments([attachment])
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

  async function cancelFeedback() {
    if (!workspace || !canCancel) return
    if (!window.confirm(tr('确认取消这个反馈请求？'))) return
    if (rambleCanExit) await exitRamble()

    cancelling = true
    pageError = ''
    try {
      const input: CancelFeedbackInput = {
        request_id: workspace.request.request_id,
        reason: 'Human cancelled from RambleDesk desktop',
      }
      const result = await invoke<FeedbackRequestView>('cancel_feedback_request', { input })
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
      cancelling = false
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

  async function exitRamble() {
    await rambleController?.exitRamble()
  }

  async function toggleRamble() {
    await rambleController?.toggleRamble()
  }

  async function importClipboardNow() {
    await rambleController?.importClipboardNow()
  }

  function resetVoiceUi() {
    rambleController?.resetVoiceUi()
  }

  function resetRambleUi() {
    rambleController?.resetRambleUi()
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
  <RambleSessionController
    bind:this={rambleController}
    {isTauri}
    {workspace}
    editor={workspacePanel}
    bind:attachmentBusy
    bind:attachmentMessage
    {savedRevision}
    bind:voicePhase
    bind:voiceDevice
    bind:voicePartial
    bind:voiceLevel
    bind:voiceChunkIndex
    bind:ramblePhase
    bind:rambleStartedOnce
    bind:rambleMessage
    onPageError={(message) => (pageError = message)}
    onSaveDraftNow={saveDraftNow}
    onApplyWorkspaceMutation={applyWorkspaceMutation}
    onRefreshAttachmentPreviews={refreshAttachmentPreviews}
    onStartScreenCapture={startScreenCapture}
    onImportAttachmentPaths={importAttachmentPaths}
  />

  <AppTitlebar
    projectName={workspace?.request.project_name ?? 'Vault Zero Archive'}
    pendingCount={inbox.length}
    notificationText={notificationLabel(notificationState, $locale)}
    notificationEnabled={notificationState === 'enabled'}
    notificationDisabled={notificationState === 'checking' || notificationState === 'unavailable'}
    onSettings={toggleSettings}
    onNotifications={toggleNotifications}
    onWindowError={(message) => (pageError = tr('窗口操作失败：{error}', { error: message }))}
  />

  <div class="workbench">
    <InboxPanel
      {inboxMode}
      {loadingInbox}
      {loadingHistory}
      requests={displayedRequests}
      activeRequestId={workspace?.request.request_id ?? null}
      {adapterPresentation}
      {formatTime}
      onRefresh={refreshCurrentList}
      onShowOpen={showOpenRequests}
      onShowHistory={showHistory}
      onOpenRequest={(requestId) => void openRequest(requestId)}
    />

    <WorkspacePanel
      bind:this={workspacePanel}
      bind:taskBriefOpen
      {loadingWorkspace}
      {workspace}
      {feedbackResult}
      {pageError}
      {draftBody}
      {savedRevision}
      {savePhase}
      {attachmentPreviews}
      {dragActive}
      {attachmentMessage}
      {saveMessage}
      {rambelleStatusPortrait}
      {rambleEngaged}
      {rambleActive}
      {ramblePhase}
      {rambleBusy}
      {rambleStartedOnce}
      {voiceDevice}
      {voiceChunkIndex}
      {voicePartial}
      {voiceLevel}
      {rambleMessage}
      {attachmentBusy}
      {canSubmit}
      {submitting}
      {canCancel}
      {cancelling}
      {adapterPresentation}
      {formatTime}
      onReload={() => void reloadWorkspace()}
      onDraftChange={updateDraft}
      onToggleRamble={() => void toggleRamble()}
      onExitRamble={() => void exitRamble()}
      onStartScreenCapture={() => void startScreenCapture()}
      onImportClipboard={() => void importClipboardNow()}
      onFileSelection={handleFileSelection}
      onInsertAttachment={insertExistingAttachment}
      onRemoveAttachment={(attachment) => void removeAttachment(attachment)}
      onOpenPackage={() => void openFeedbackPackage()}
      onSubmit={() => void submitFeedback()}
      onCancel={() => void cancelFeedback()}
    />

    {#if resumePrompt}
      <ResumePromptDialog
        prompt={resumePrompt}
        copyState={resumeCopyState}
        onCopy={() => void copyResumePrompt()}
        onDismiss={dismissResumePrompt}
      />
    {/if}
  </div>
</main>

{#if settingsOpen}
  <SettingsPanel
    {mcpConfiguration}
    initialSection={settingsSection}
    projectRootPath={workspace?.request.project_root_path ?? null}
    onClose={() => (settingsOpen = false)}
  />
{/if}
{/key}
