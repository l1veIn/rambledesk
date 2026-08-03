<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
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
    ApproveFeedbackInput,
    CancelFeedbackInput,
    DraftView,
    FeedbackRequestView,
    FeedbackWorkspaceView,
    SaveDraftInput,
    SubmitFeedbackInput,
  } from './lib/feedback'
  import {
    notificationLabel,
    notificationStateForPermission,
    playNotificationSound,
    type NotificationState,
  } from './lib/notifications'
  import { desktopPath } from './lib/nativePath'
  import { previewFixtures, previewWorkspaceFor } from './lib/previewFixtures'
  import {
    createAttachmentController,
    type AttachmentMessageTone,
  } from './lib/workbench/attachmentController'
  import { createNavigationController } from './lib/workbench/navigationController'
  import type {
    FeedbackEditorHandle,
    RamblePhase,
    RambleSessionControllerHandle,
    ResumePrompt,
    SavePhase,
    SettingsSection,
    VoicePhase,
  } from './lib/workbench/types'
  import RambleSessionController from './lib/workbench/RambleSessionController.svelte'
  import { t } from './lib/i18n'
  import {
    locale,
    notificationPopupEnabled,
    notificationSound,
    notificationSoundEnabled,
    notificationVolume,
    setNotificationPopupEnabled,
  } from './lib/preferences'

  type CommandError = { code: string; message: string; retryable: boolean }

  const RESUME_PROMPT_EVENT = 'rambledesk://resume-prompt'
  const OPEN_ADAPTERS_EVENT = 'rambledesk://open-adapters'
  let workspace: FeedbackWorkspaceView | null = null
  let completedResult: FeedbackRequestView | null = null
  let draftBody = ''
  let savedBody = ''
  let savedRevision = 0
  let savePhase: SavePhase = 'idle'
  let saveMessage = ''
  let pageError = ''
  let loadingWorkspace = false
  let submitting = false
  let cancelling = false
  let approving = false
  let attachmentBusy = false
  let attachmentMessage = ''
  let attachmentMessageTone: AttachmentMessageTone = 'info'
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
  let voiceModelMissing = false
  let ramblePhase: RamblePhase = 'idle'
  let rambleStartedOnce = false
  let rambleRequestId = ''
  let rambleRequestTitle = ''
  let rambleMessage = ''
  let rambleMarkdownQueue: Promise<void> = Promise.resolve()
  let saveTimer: ReturnType<typeof setTimeout> | undefined
  let inboxTimer: ReturnType<typeof setInterval> | undefined
  let activeSave: Promise<boolean> | null = null

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  const attachmentController = createAttachmentController({
    isTauri,
    tr,
    messageFrom,
    getWorkspace: () => workspace,
    getEditor: () => workspacePanel,
    getRambleRequestId: () => rambleRequestId,
    getRambleEngaged: () => rambleEngaged,
    getSavedRevision: () => savedRevision,
    getBusy: () => attachmentBusy,
    getPreviews: () => attachmentPreviews,
    setBusy: (busy) => (attachmentBusy = busy),
    setMessage: (message, tone) => {
      if (tone) attachmentMessageTone = tone
      attachmentMessage = message
    },
    setPreviews: (previews) => (attachmentPreviews = previews),
    setDragActive: (active) => (dragActive = active),
    saveDraftNow,
    waitForRambleMarkdown: () => rambleMarkdownQueue.catch(() => {}),
    appendRambleMarkdown,
    applyWorkspaceMutation,
  })

  const navigation = createNavigationController({
    isTauri,
    previewMode,
    tr,
    messageFrom,
    getNotificationState: () => notificationState,
    getWorkspaceRequestId: () => workspace?.request.request_id,
    isDirty: () => dirty,
    saveDraftNow,
    openRequest,
    clearWorkspace,
    onPageError: (message) => (pageError = message),
  })
  const resolveHostProfile = navigation.resolveHostProfile

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
  $: selectedHostSession = $navigation.selectedHostSessionId
    ? $navigation.hostSessions.find(
        (session) =>
          session.host_id === $navigation.selectedHostId &&
          session.host_session_id === $navigation.selectedHostSessionId,
      )
    : undefined
  $: requestScopeLabel = $navigation.selectedHostId
    ? $navigation.selectedHostSessionId
      ? selectedHostSession?.source_hint ??
        selectedHostSession?.title ??
        resolveHostProfile($navigation.selectedHostId).label
      : resolveHostProfile($navigation.selectedHostId).label
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
    const cleanupAttachments = attachmentController.mount()
    void navigation.initialize()

    if (!isTauri) {
      if (previewMode) {
        workspace = previewFixtures.workspace
        draftBody = previewFixtures.workspace.draft.body_markdown
        savedBody = draftBody
        savedRevision = previewFixtures.workspace.draft.saved_revision
        savePhase = 'saved'
        if (new URLSearchParams(window.location.search).get('dialog') === 'resume') {
          resumePrompt = previewFixtures.resumePrompt
        }
      }
      notificationState = 'unavailable'
      return cleanupAttachments
    }
    void refreshNotificationPermission()
    inboxTimer = setInterval(() => void navigation.refreshNavigation(true), 5_000)
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
    return () => {
      if (saveTimer) clearTimeout(saveTimer)
      if (inboxTimer) clearInterval(inboxTimer)
      resumePromptUnlisten?.()
      openAdaptersUnlisten?.()
      cleanupAttachments()
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

  function clearWorkspace() {
    workspace = null
    completedResult = null
    attachmentController.releasePreviews()
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
      await attachmentController.refreshPreviews(next)
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
      await navigation.refreshNavigation(true)
    } catch (cause) {
      pageError = messageFrom(cause)
    } finally {
      submitting = false
    }
  }

  async function approveFeedback() {
    if (!workspace || !workspace.request.allow_finish || approving) return
    if (!window.confirm(tr('同意这个最终总结并结束 Pi 的 Ramble 流程？'))) return
    if (rambleCanExit) await exitRamble()
    approving = true
    pageError = ''
    try {
      const input: ApproveFeedbackInput = { request_id: workspace.request.request_id }
      const result = await invoke<FeedbackRequestView>('approve_feedback_request', { input })
      completedResult = result
      workspace = {
        ...workspace,
        request: {
          ...workspace.request,
          status: result.status,
          resolution: result.resolution,
          updated_at: result.updated_at,
        },
      }
      await navigation.refreshNavigation(true)
    } catch (cause) {
      pageError = messageFrom(cause)
    } finally {
      approving = false
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
      await navigation.refreshNavigation(true)
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
    bind:voiceModelMissing
    bind:ramblePhase
    bind:rambleStartedOnce
    bind:rambleRequestId
    bind:rambleRequestTitle
    bind:rambleMessage
    onPageError={(message) => (pageError = message)}
    onSaveDraftNow={saveDraftNow}
    onApplyWorkspaceMutation={applyWorkspaceMutation}
    onRefreshAttachmentPreviews={attachmentController.refreshPreviews}
    onStartScreenCapture={attachmentController.startScreenCapture}
    onImportAttachmentPaths={attachmentController.importAttachmentPaths}
    onAppendRambleMarkdown={appendRambleMarkdown}
  />

  <AppTitlebar
    sourceLabel={workspace?.request.source_hint ?? workspace?.request.title ?? 'Workbench'}
    pendingCount={$navigation.pendingRequests.length}
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
      sessions={$navigation.hostSessions}
      activeHostId={$navigation.selectedHostId}
      activeHostSessionId={$navigation.selectedHostSessionId}
      loading={$navigation.loadingNavigation}
      {resolveHostProfile}
      onSelect={(hostId, hostSessionId) =>
        void navigation.selectScope(hostId, hostSessionId)}
      onRefresh={() => void navigation.refreshNavigation(true)}
      onSettings={() => void openSettings('adapters')}
    />

    <RequestListPane
      requests={$navigation.requests}
      activeRequestId={workspace?.request.request_id ?? null}
      scopeLabel={requestScopeLabel}
      loading={$navigation.loadingRequests}
      loadingMore={$navigation.loadingMoreRequests}
      hasMore={$navigation.nextRequestCursor !== null}
      {resolveHostProfile}
      {formatTime}
      onRefresh={() => void navigation.refreshRequests(false)}
      onLoadMore={() => void navigation.loadMoreRequests()}
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
      voiceModelMissing={rambleBelongsToWorkspace ? voiceModelMissing : false}
      rambleMessage={rambleBelongsToWorkspace ? rambleMessage : ''}
      attachmentBusy={rambleBelongsToWorkspace ? attachmentBusy : false}
      {canSubmit}
      {submitting}
      {canCancel}
      {cancelling}
      {approving}
      {resolveHostProfile}
      {formatTime}
      onReload={() => void reloadWorkspace()}
      onDraftChange={updateDraft}
      onToggleRamble={() => void toggleRamble()}
      onExitRamble={() => void exitRamble()}
      onOpenVoiceSettings={() => void openSettings('voice')}
      onStartScreenCapture={() => void attachmentController.startScreenCapture()}
      onImportClipboard={() => void importClipboardNow()}
      onFileSelection={attachmentController.handleFileSelection}
      onInsertAttachment={attachmentController.insertExistingAttachment}
      onRemoveAttachment={(attachment) => void attachmentController.removeAttachment(attachment)}
      onOpenPackage={() => void openFeedbackPackage()}
      onSubmit={() => void submitFeedback()}
      onCancel={() => void cancelFeedback()}
      onApprove={() => void approveFeedback()}
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
