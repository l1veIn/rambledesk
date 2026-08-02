<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { getCurrentWebview } from '@tauri-apps/api/webview'
  import {
    isPermissionGranted,
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
  import HostSessionRail from './lib/components/navigation/HostSessionRail.svelte'
  import RequestListPane from './lib/components/navigation/RequestListPane.svelte'
  import { Sonner, toast } from './lib/components/ui/sonner'
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
    HostSessionSummary,
    ListFeedbackRequestsInput,
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
    playNotificationSound,
    type NotificationState,
  } from './lib/notifications'
  import { desktopPath } from './lib/nativePath'
  import { previewFixtures, previewWorkspaceFor } from './lib/previewFixtures'
  import type {
    FeedbackEditorHandle,
    HostProfile,
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
  import {
    locale,
    notificationPopupEnabled,
    notificationSound,
    notificationSoundEnabled,
    notificationVolume,
    setNotificationPopupEnabled,
  } from './lib/preferences'

  type ScreenCaptureFinished = {
    capture_session_id: string | null
    outcome: 'cancelled' | 'pinned'
  }

  type CommandError = { code: string; message: string; retryable: boolean }

  const RESUME_PROMPT_EVENT = 'rambledesk://resume-prompt'
  const OPEN_ADAPTERS_EVENT = 'rambledesk://open-adapters'

  const ALL_REQUEST_STATUSES = ['waiting', 'in_progress', 'completed', 'cancelled'] as const

  let pendingRequests: FeedbackRequestSummary[] = []
  let requests: FeedbackRequestSummary[] = []
  let hostSessions: HostSessionSummary[] = []
  let hostProfiles: Record<string, HostProfile> = {}
  let selectedHostId: string | null = null
  let selectedHostSessionId: string | null = null
  let nextRequestCursor: string | null = null
  let workspace: FeedbackWorkspaceView | null = null
  let completedResult: FeedbackRequestView | null = null
  let draftBody = ''
  let savedBody = ''
  let savedRevision = 0
  let savePhase: SavePhase = 'idle'
  let saveMessage = ''
  let pageError = ''
  let loadingNavigation = true
  let loadingRequests = true
  let loadingMoreRequests = false
  let loadingWorkspace = false
  let submitting = false
  let cancelling = false
  let attachmentBusy = false
  let attachmentMessage = ''
  let attachmentMessageTone: 'info' | 'success' | 'error' = 'info'
  let deliveredAttachmentMessage = ''
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
  const previewMode =
    import.meta.env.DEV &&
    !isTauri &&
    new URLSearchParams(window.location.search).get('preview') === 'fixtures'
  let taskBriefOpen = true
  let genericMcpConfiguration = ''
  let voicePhase: VoicePhase = 'idle'
  let voiceDevice = ''
  let voicePartial = ''
  let voiceLevel = 0
  let voiceChunkIndex = 0
  let ramblePhase: RamblePhase = 'idle'
  let rambleStartedOnce = false
  let rambleRequestId = ''
  let rambleRequestTitle = ''
  let rambleMessage = ''
  let rambleMarkdownQueue: Promise<void> = Promise.resolve()
  let screenCaptureRequestId = ''
  let saveTimer: ReturnType<typeof setTimeout> | undefined
  let inboxTimer: ReturnType<typeof setInterval> | undefined
  let activeSave: Promise<boolean> | null = null
  const notificationTracker = new InboxNotificationTracker()

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  $: dirty = workspace !== null && draftBody !== savedBody
  $: {
    if (!attachmentMessage) {
      deliveredAttachmentMessage = ''
    } else if (attachmentMessage !== deliveredAttachmentMessage) {
      deliveredAttachmentMessage = attachmentMessage
      const options = { description: attachmentMessage }
      if (attachmentMessageTone === 'success') toast.success(tr('附件操作完成'), options)
      else if (attachmentMessageTone === 'info') toast.info(tr('附件操作状态'), options)
      else toast.error(tr('附件操作失败'), options)
    }
  }
  $: selectedHostSession = selectedHostSessionId
    ? hostSessions.find(
        (session) =>
          session.host_id === selectedHostId && session.host_session_id === selectedHostSessionId,
      )
    : undefined
  $: requestScopeLabel = selectedHostId
    ? selectedHostSessionId
      ? selectedHostSession?.source_hint ?? selectedHostSession?.title ?? resolveHostProfile(selectedHostId).label
      : resolveHostProfile(selectedHostId).label
    : tr('全部宿主')
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
  $: rambleBelongsToWorkspace =
    !rambleEngaged || workspace?.request.request_id === rambleRequestId
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
      if (previewMode) {
        requests = previewFixtures.requests
        pendingRequests = previewFixtures.requests.filter(
          (request) => request.status === 'waiting' || request.status === 'in_progress',
        )
        hostSessions = previewFixtures.hostSessions
        hostProfiles = Object.fromEntries(
          previewFixtures.hostProfiles.map((profile) => [profile.id, profile]),
        )
        workspace = previewFixtures.workspace
        draftBody = previewFixtures.workspace.draft.body_markdown
        savedBody = draftBody
        savedRevision = previewFixtures.workspace.draft.saved_revision
        savePhase = 'saved'
        if (new URLSearchParams(window.location.search).get('dialog') === 'resume') {
          resumePrompt = previewFixtures.resumePrompt
        }
      }
      loadingNavigation = false
      loadingRequests = false
      notificationState = 'unavailable'
      window.addEventListener('paste', handlePaste)
      return () => window.removeEventListener('paste', handlePaste)
    }
    void initialize()
    void refreshNotificationPermission()
    inboxTimer = setInterval(() => void refreshNavigation(true), 5_000)
    let dragUnlisten: (() => void) | undefined
    let captureReadyUnlisten: (() => void) | undefined
    let captureFinishedUnlisten: (() => void) | undefined
    let resumePromptUnlisten: (() => void) | undefined
    let openAdaptersUnlisten: (() => void) | undefined
    void listen(OPEN_ADAPTERS_EVENT, () => openSettings('adapters'))
      .then((unlisten) => {
        openAdaptersUnlisten = unlisten
      })
      .catch(() => {
        // The tray entry is unavailable in browser preview.
      })
    void listen<ResumePrompt>(RESUME_PROMPT_EVENT, (event) => {
      resumePrompt = event.payload
      resumeCopyState = 'idle'
      if ($notificationPopupEnabled && notificationState === 'enabled') {
        sendNotification({
          title: event.payload.title,
          body: tr('请回到 {host}，用恢复提示继续宿主会话。', {
            host: event.payload.host_label,
          }),
        })
      }
      if ($notificationSoundEnabled) {
        void playNotificationSound($notificationSound, $notificationVolume)
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
        attachmentMessageTone = 'error'
        attachmentMessage = tr('无法接收截图结果：{error}', { error: messageFrom(cause) })
      })
    void listen<ScreenCaptureFinished>('screen-capture-finished', (event) => {
      attachmentBusy = false
      attachmentMessageTone = 'info'
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
        attachmentMessageTone = 'error'
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
      openAdaptersUnlisten?.()
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
    loadingNavigation = true
    loadingRequests = true
    try {
      const [nextInbox, nextHostSessions, profiles] = await Promise.all([
        invoke<FeedbackRequestSummary[]>('list_feedback_inbox'),
        invoke<HostSessionSummary[]>('list_host_sessions'),
        invoke<HostProfile[]>('list_host_profiles'),
      ])
      hostProfiles = Object.fromEntries(profiles.map((profile) => [profile.id, profile]))
      applyInboxSnapshot(nextInbox)
      hostSessions = nextHostSessions
      await refreshRequests(true)
    } catch (cause) {
      pageError = messageFrom(cause)
    } finally {
      loadingNavigation = false
      loadingRequests = false
    }
  }

  function resolveHostProfile(hostId: string): HostProfile {
    const normalized = hostId.trim().toLowerCase()
    const profile = hostProfiles[normalized]
    if (profile) return profile
    return {
      id: normalized || 'generic',
      label: hostId.trim() || hostProfiles.generic?.label || 'Generic Host',
      icon_svg: hostProfiles.generic?.icon_svg || '',
      default_adapter: 'generic_mcp',
      continuation_mode: 'manual',
    }
  }

  async function refreshNavigation(refreshRequestList = false) {
    loadingNavigation = true
    try {
      const [nextInbox, nextHostSessions] = await Promise.all([
        invoke<FeedbackRequestSummary[]>('list_feedback_inbox'),
        invoke<HostSessionSummary[]>('list_host_sessions'),
      ])
      applyInboxSnapshot(nextInbox)
      hostSessions = nextHostSessions
      if (refreshRequestList) await refreshRequests(false)
    } catch (cause) {
      pageError = messageFrom(cause)
    } finally {
      loadingNavigation = false
    }
  }

  function applyInboxSnapshot(nextInbox: FeedbackRequestSummary[]) {
    const arrivals = notificationTracker.observe(nextInbox)
    pendingRequests = nextInbox
    void invoke('set_pending_count', { count: nextInbox.length }).catch(() => {
      // Tray updates are a convenience; the inbox remains authoritative.
    })
    if (arrivals.length > 0) {
      if ($notificationPopupEnabled && notificationState === 'enabled') {
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
      if ($notificationSoundEnabled) {
        void playNotificationSound($notificationSound, $notificationVolume)
      }
    }
  }

  function requestListInput(cursor: string | null = null): ListFeedbackRequestsInput {
    return {
      host_id: selectedHostId,
      host_session_id: selectedHostSessionId,
      status: [...ALL_REQUEST_STATUSES],
      limit: 100,
      cursor,
    }
  }

  async function refreshRequests(openFirst = false) {
    loadingRequests = true
    try {
      const result: ListFeedbackRequestsOutput = previewMode
        ? {
            requests: previewFixtures.requests.filter(
              (request) =>
                (!selectedHostId || request.host_id === selectedHostId) &&
                (!selectedHostSessionId ||
                  request.host_session_id === selectedHostSessionId),
            ),
            next_cursor: null,
          }
        : await invoke<ListFeedbackRequestsOutput>('list_feedback_requests', {
            input: requestListInput(),
          })
      requests = result.requests
      nextRequestCursor = result.next_cursor
      const currentRequestId = workspace?.request.request_id
      if (openFirst && result.requests[0]) {
        await openRequest(result.requests[0].request_id, currentRequestId !== undefined)
      } else if (openFirst && result.requests.length === 0) {
        if (!dirty || (await saveDraftNow())) {
          workspace = null
          completedResult = null
          releaseAttachmentPreviews()
        }
      }
    } catch (cause) {
      pageError = messageFrom(cause)
    } finally {
      loadingRequests = false
    }
  }

  async function loadMoreRequests() {
    if (!nextRequestCursor || loadingMoreRequests) return
    loadingMoreRequests = true
    try {
      const result = await invoke<ListFeedbackRequestsOutput>('list_feedback_requests', {
        input: requestListInput(nextRequestCursor),
      })
      const known = new Set(requests.map((request) => request.request_id))
      requests = [...requests, ...result.requests.filter((request) => !known.has(request.request_id))]
      nextRequestCursor = result.next_cursor
    } catch (cause) {
      pageError = messageFrom(cause)
    } finally {
      loadingMoreRequests = false
    }
  }

  async function selectNavigationScope(hostId: string | null, hostSessionId: string | null) {
    if (selectedHostId === hostId && selectedHostSessionId === hostSessionId) return
    if (dirty && !(await saveDraftNow())) return
    selectedHostId = hostId
    selectedHostSessionId = hostSessionId
    await refreshRequests(false)
  }

  async function refreshNotificationPermission() {
    try {
      const granted = await isPermissionGranted()
      if (!granted && $notificationPopupEnabled) setNotificationPopupEnabled(false)
      notificationState = notificationStateForPermission(granted, $notificationPopupEnabled)
    } catch {
      notificationState = 'unavailable'
    }
  }

  async function openRequest(requestId: string, saveCurrent = true) {
    if (workspace?.request.request_id === requestId) return
    if (saveCurrent && !(await saveDraftNow())) return
    if (requestId === rambleRequestId) await rambleMarkdownQueue.catch(() => {})

    loadingWorkspace = true
    pageError = ''
    completedResult = null
    try {
      const next = previewMode
        ? previewWorkspaceFor(requestId)
        : await invoke<FeedbackWorkspaceView>('get_feedback_workspace', {
            requestId,
          })
      if (!next) throw new Error(tr('找不到这个反馈请求。'))
      workspace = next
      draftBody = next.draft.body_markdown
      savedBody = next.draft.body_markdown
      savedRevision = next.draft.saved_revision
      savePhase = next.draft.updated_at ? 'saved' : 'idle'
      saveMessage = ''
      attachmentMessage = ''
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
        const saved: DraftView = previewMode
          ? {
              body_markdown: bodyToSave,
              saved_revision: revisionToSave + 1,
              updated_at: new Date().toISOString(),
            }
          : await invoke<DraftView>('save_feedback_draft', { input })
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

  async function appendRambleMarkdown(requestId: string, markdown: string): Promise<void> {
    const block = markdown.trim()
    if (!requestId || !block) return

    const operation = rambleMarkdownQueue.then(async () => {
      if (workspace?.request.request_id === requestId) {
        const nextBody = appendMarkdownBlock(draftBody, block)
        updateDraft(nextBody)
        if (!(await saveDraftNow())) throw new Error(saveMessage || tr('当前草稿无法保存'))
        return
      }

      const target = previewMode
        ? previewWorkspaceFor(requestId)
        : await invoke<FeedbackWorkspaceView>('get_feedback_workspace', { requestId })
      if (!target) throw new Error(tr('找不到这个反馈请求。'))
      const input: SaveDraftInput = {
        request_id: requestId,
        body_markdown: appendMarkdownBlock(target.draft.body_markdown, block),
        expected_revision: target.draft.saved_revision,
      }
      if (!previewMode) await invoke<DraftView>('save_feedback_draft', { input })
    })
    rambleMarkdownQueue = operation.catch((cause) => {
      pageError = tr('Ramble 内容写入失败：{error}', { error: messageFrom(cause) })
    })
    await operation
  }

  function appendMarkdownBlock(body: string, block: string) {
    const current = body.trimEnd()
    return current ? `${current}\n\n${block}` : block
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
      genericMcpConfiguration = await invoke<string>('get_generic_mcp_configuration')
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
      attachmentMessageTone = 'error'
      attachmentMessage = messageFrom(cause)
      if (workspace) await refreshAttachmentPreviews(workspace)
    } finally {
      attachmentBusy = false
    }
  }

  async function importAttachmentPaths(paths: string[]) {
    const requestId = rambleRequestId || workspace?.request.request_id || ''
    if (!requestId || paths.length === 0 || attachmentBusy) return
    const visibleTarget = workspace?.request.request_id === requestId
    if (visibleTarget && !(await saveDraftNow())) return
    await rambleMarkdownQueue.catch(() => {})
    attachmentBusy = true
    attachmentMessage = ''
    try {
      let next = visibleTarget
        ? workspace
        : await invoke<FeedbackWorkspaceView>('get_feedback_workspace', { requestId })
      if (!next) throw new Error(tr('找不到这个反馈请求。'))
      const existingIds = new Set(next.attachments.map((item) => item.attachment_id))
      for (const path of paths) {
        next = await invoke<FeedbackWorkspaceView>('import_feedback_attachment_path', {
          requestId,
          path,
          expectedRevision: next.draft.saved_revision,
        })
        if (visibleTarget) applyWorkspaceMutation(next)
      }
      const added = next.attachments.filter((item) => !existingIds.has(item.attachment_id))
      if (visibleTarget && workspace?.request.request_id === requestId) {
        await refreshAttachmentPreviews(next)
        await tick()
        workspacePanel?.insertAttachments(added)
        await saveDraftNow()
      } else {
        await appendRambleMarkdown(
          requestId,
          added
            .map((attachment) => `![${attachment.file_name}](attachment://${attachment.attachment_id})`)
            .join('\n\n'),
        )
      }
    } catch (cause) {
      attachmentMessageTone = 'error'
      attachmentMessage = messageFrom(cause)
      if (workspace?.request.request_id === requestId) await refreshAttachmentPreviews(workspace)
    } finally {
      attachmentBusy = false
    }
  }

  async function startScreenCapture() {
    const requestId = rambleRequestId || workspace?.request.request_id || ''
    if (!requestId || !rambleEngaged || attachmentBusy) return
    if (workspace?.request.request_id === requestId && !(await saveDraftNow())) return
    await rambleMarkdownQueue.catch(() => {})
    screenCaptureRequestId = requestId
    attachmentBusy = true
    attachmentMessageTone = 'info'
    attachmentMessage = tr('高级截图已唤起：可智能选窗、标注、滚动截图或固定到屏幕')
    try {
      await invoke('begin_screen_capture')
    } catch (cause) {
      screenCaptureRequestId = ''
      attachmentBusy = false
      attachmentMessageTone = 'error'
      const message = messageFrom(cause)
      attachmentMessage =
        message === '内置区域截图目前只在 Windows 开发环境启用' ? tr(message) : message
    }
  }

  async function importScreenCapture(capture: ScreenCaptureReady) {
    const requestId = screenCaptureRequestId || rambleRequestId || workspace?.request.request_id || ''
    if (!requestId) {
      await invoke('discard_screen_capture', { captureSessionId: capture.capture_session_id }).catch(() => {})
      attachmentBusy = false
      return
    }
    try {
      const visibleTarget = workspace?.request.request_id === requestId
      if (visibleTarget && !(await saveDraftNow())) {
        throw new Error(tr('当前草稿无法保存，截图尚未写入'))
      }
      await rambleMarkdownQueue.catch(() => {})
      const target = visibleTarget
        ? workspace
        : await invoke<FeedbackWorkspaceView>('get_feedback_workspace', { requestId })
      if (!target) throw new Error(tr('找不到这个反馈请求。'))
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
      if (visibleTarget && workspace?.request.request_id === requestId) {
        applyWorkspaceMutation(next)
        await refreshAttachmentPreviews(next)
        await tick()
        if (!workspacePanel?.insertAttachments(added)) {
          throw new Error(tr('截图附件已保存，但编辑器未能在当前光标位置插入图片'))
        }
        await saveDraftNow()
      } else {
        const attachment = added[0]
        if (!attachment) throw new Error(tr('截图附件已保存，但编辑器未能在当前光标位置插入图片'))
        await appendRambleMarkdown(
          requestId,
          `![${attachment.file_name}](attachment://${attachment.attachment_id})`,
        )
      }
      attachmentMessageTone = 'success'
      attachmentMessage = tr('截图已自动插入当前文档位置')
    } catch (cause) {
      attachmentMessageTone = 'error'
      attachmentMessage = tr('截图写入失败：{error}', { error: messageFrom(cause) })
      if (workspace?.request.request_id === requestId) {
        await refreshAttachmentPreviews(workspace)
      }
    } finally {
      screenCaptureRequestId = ''
      await invoke('discard_screen_capture', { captureSessionId: capture.capture_session_id }).catch(() => {})
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
      attachmentMessageTone = 'error'
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
      attachmentMessageTone = 'error'
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
      await refreshNavigation(true)
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
      await refreshNavigation(true)
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
<main class="h-full w-full overflow-hidden rounded-[16px] border bg-background text-foreground shadow-sm">
  <Sonner />
  <RambleSessionController
    bind:this={rambleController}
    {isTauri}
    {workspace}
    editor={workspacePanel}
    bind:attachmentBusy
    bind:attachmentMessage
    bind:voicePhase
    bind:voiceDevice
    bind:voicePartial
    bind:voiceLevel
    bind:voiceChunkIndex
    bind:ramblePhase
    bind:rambleStartedOnce
    bind:rambleRequestId
    bind:rambleRequestTitle
    bind:rambleMessage
    onPageError={(message) => (pageError = message)}
    onSaveDraftNow={saveDraftNow}
    onApplyWorkspaceMutation={applyWorkspaceMutation}
    onRefreshAttachmentPreviews={refreshAttachmentPreviews}
    onStartScreenCapture={startScreenCapture}
    onImportAttachmentPaths={importAttachmentPaths}
    onAppendRambleMarkdown={appendRambleMarkdown}
  />

  <AppTitlebar
    sourceLabel={workspace?.request.source_hint ?? workspace?.request.title ?? 'Workbench'}
    pendingCount={pendingRequests.length}
    {rambleEngaged}
    {rambleActive}
    {rambleRequestTitle}
    notificationText={$notificationSoundEnabled
      ? tr('通知设置 · 声音已开启')
      : notificationLabel(notificationState, $locale)}
    notificationEnabled={notificationState === 'enabled' || $notificationSoundEnabled}
    notificationDisabled={false}
    onSettings={toggleSettings}
    onNotifications={() => void openSettings('notifications')}
    onWindowError={(message) => (pageError = tr('窗口操作失败：{error}', { error: message }))}
  />

  <div class="flex h-[calc(100%-46px)] min-h-0">
    <HostSessionRail
      sessions={hostSessions}
      activeHostId={selectedHostId}
      activeHostSessionId={selectedHostSessionId}
      loading={loadingNavigation}
      {resolveHostProfile}
      onSelect={(hostId, hostSessionId) => void selectNavigationScope(hostId, hostSessionId)}
      onRefresh={() => void refreshNavigation(true)}
      onSettings={() => void openSettings('adapters')}
    />

    <RequestListPane
      {requests}
      activeRequestId={workspace?.request.request_id ?? null}
      scopeLabel={requestScopeLabel}
      loading={loadingRequests}
      loadingMore={loadingMoreRequests}
      hasMore={nextRequestCursor !== null}
      {resolveHostProfile}
      {formatTime}
      onRefresh={() => void refreshRequests(false)}
      onLoadMore={() => void loadMoreRequests()}
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
      {saveMessage}
      rambelleStatusPortrait={rambleBelongsToWorkspace
        ? rambelleStatusPortrait
        : feedbackResult
          ? rambelleArchived
          : rambelleIdle}
      rambleEngaged={rambleBelongsToWorkspace ? rambleEngaged : false}
      rambleActive={rambleBelongsToWorkspace ? rambleActive : false}
      ramblePhase={rambleBelongsToWorkspace ? ramblePhase : 'idle'}
      rambleBusy={rambleBelongsToWorkspace ? rambleBusy : true}
      rambleStartedOnce={rambleBelongsToWorkspace ? rambleStartedOnce : false}
      voiceDevice={rambleBelongsToWorkspace ? voiceDevice : ''}
      voiceChunkIndex={rambleBelongsToWorkspace ? voiceChunkIndex : 0}
      voicePartial={rambleBelongsToWorkspace ? voicePartial : ''}
      voiceLevel={rambleBelongsToWorkspace ? voiceLevel : 0}
      rambleMessage={rambleBelongsToWorkspace ? rambleMessage : ''}
      attachmentBusy={rambleBelongsToWorkspace ? attachmentBusy : false}
      {canSubmit}
      {submitting}
      {canCancel}
      {cancelling}
      {resolveHostProfile}
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
    mcpConfiguration={genericMcpConfiguration}
    initialSection={settingsSection}
    onClose={() => {
      settingsOpen = false
      void refreshNotificationPermission()
    }}
  />
{/if}
{/key}
