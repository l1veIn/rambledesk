<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import {
    isPermissionGranted,
    sendNotification,
  } from '@tauri-apps/plugin-notification'
  import { revealItemInDir } from '@tauri-apps/plugin-opener'
  import { onMount, tick } from 'svelte'
  import { Pane, PaneGroup, PaneResizer } from 'paneforge'

  import rambelleArchived from './assets/rambelle-states/archived.png'
  import rambelleIdle from './assets/rambelle-states/idle.png'
  import rambelleOrganizing from './assets/rambelle-states/organizing.png'
  import rambelleRecording from './assets/rambelle-states/recording.png'
  import AppTitlebar from './lib/AppTitlebar.svelte'
  import OnboardingWizard from './lib/OnboardingWizard.svelte'
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
  import { checkForUpdates } from './lib/updater'
  import { previewFixtures, previewWorkspaceFor } from './lib/previewFixtures'
  import {
    restorePublishedAttachmentUrls,
    normalizePublishedFeedback,
    type PublishedAttachmentPath,
    type PublishedFeedbackPackage,
    type PublishedFeedbackView,
  } from './lib/publishedFeedback'
  import {
    appendMarkdownBlock,
    formatTime,
    messageFrom,
    operatorFeedbackBody,
  } from './lib/workbench/feedbackText'
  import { createCookingController } from './lib/workbench/cookingController'
  import { createDraftController } from './lib/workbench/draftController'
  import { createPublisherController } from './lib/workbench/publisherController'
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
    SubmitStage,
    VoicePhase,
  } from './lib/workbench/types'
  import RambleSessionController from './lib/workbench/RambleSessionController.svelte'
  import { t } from './lib/i18n'
  import {
    initialHostRailCollapsed,
    saveHostRailCollapsed,
    savePaneLayout,
    savedPaneLayout,
  } from './lib/uiPreferences'
  import {
    cookingApiKey,
    cookingBaseUrl,
    cookingEnabled,
    cookingModel,
    cookingProvider,
    cookingReasoningEffort,
    locale,
    notificationPopupEnabled,
    notificationSound,
    onboardingCompleted,
    resetOnboarding,
    notificationSoundEnabled,
    notificationVolume,
    setNotificationPopupEnabled,
  } from './lib/preferences'

  type PaneGroupHandle = {
    setLayout: (layout: number[]) => void
  }

  const RESUME_PROMPT_EVENT = 'rambledesk://resume-prompt'
  const OPEN_ADAPTERS_EVENT = 'rambledesk://open-adapters'
  const formatTimeLocal = (value: string | null | undefined) =>
    formatTime(value, $locale, tr('尚未保存'))
  let workspace: FeedbackWorkspaceView | null = null
  let completedResult: FeedbackRequestView | null = null
  let publishedFeedback: PublishedFeedbackView | null = null
  let draftBody = ''
  let savedBody = ''
  let savedRevision = 0
  let savePhase: SavePhase = 'idle'
  let saveMessage = ''
  let pageError = ''
  let loadingWorkspace = false
  let submitting = false
  let submitStage: SubmitStage = 'idle'
  let cookingRequestIds = new Set<string>()
  /** Preview cooking result for the current workspace, if generated and current. */
  let cookedPreview: { markdown: string; original: string; model: string } | null = null
  /** Pre-cook draft snapshot for the restore action. */
  let cookedPreviewOriginal = ''
  let cancelling = false
  let approving = false
  let attachmentBusy = false
  let attachmentMessage = ''
  let attachmentMessageTone: AttachmentMessageTone = 'info'
  let deliveredAttachmentMessage = ''
  let deliveredPageError = ''
  let deliveredSaveError = ''
  let attachmentPreviews: Record<string, string> = {}
  let dragActive = false
  let workspacePanel: FeedbackEditorHandle
  let rambleController: RambleSessionControllerHandle
  let resumePrompt: ResumePrompt | null = null
  let resumeCopyState: 'idle' | 'copied' | 'failed' = 'idle'
  let notificationState: NotificationState = 'checking'
  let settingsOpen = false
  let settingsSection: SettingsSection = 'general'
  let onboardingOpen = false
  let workbenchInitialized = false
  const isTauri = '__TAURI_INTERNALS__' in window
  const previewMode =
    import.meta.env.DEV &&
    !isTauri &&
    new URLSearchParams(window.location.search).get('preview') === 'fixtures'
  const REQUEST_LIST_DEFAULT_WIDTH = 296
  const REQUEST_LIST_MIN_WIDTH = 240
  const WIDE_WORKSPACE_MIN_WIDTH = 648
  const NARROW_WORKSPACE_MIN_WIDTH = 360
  const PANE_RESIZER_SIZE = 11
  const REQUEST_WORKSPACE_LAYOUT_KEY = 'request-workspace-layout'
  const savedRequestWorkspaceLayout = savedPaneLayout(REQUEST_WORKSPACE_LAYOUT_KEY)

  let taskBriefOpen = true
  let hostSessionRailCollapsed = initialHostRailCollapsed()
  let workbenchLayout: HTMLDivElement
  let requestWorkspaceGroup: HTMLDivElement | null = null
  let requestWorkspacePaneGroup: PaneGroupHandle | undefined
  let requestWorkspaceLayoutReady = false
  let workbenchLayoutWidth = 0
  let requestWorkspaceWidth = 0
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
  let inboxTimer: ReturnType<typeof setInterval> | undefined

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  const draftController = createDraftController({
    messageFrom,
    isPreviewMode: () => previewMode,
    isInteractionLocked: () => interactionLocked,
    isWorkspaceTerminal: () =>
      workspace?.request.status === 'completed' || workspace?.request.status === 'cancelled',
    getWorkspace: () => workspace,
    getBody: () => draftBody,
    setBody: (body) => {
      draftBody = body
    },
    getSavedBody: () => savedBody,
    setSavedBody: (body) => {
      savedBody = body
    },
    getSavedRevision: () => savedRevision,
    setSavedRevision: (revision) => {
      savedRevision = revision
    },
    getPhase: () => savePhase,
    setPhase: (phase) => {
      savePhase = phase
    },
    setMessage: (message) => {
      saveMessage = message
    },
    setWorkspaceDraft: (draft) => {
      if (workspace) workspace = { ...workspace, draft }
    },
  })
  const updateDraft = draftController.updateDraft
  const saveDraftNow = draftController.saveDraftNow

  const attachmentController = createAttachmentController({
    isTauri,
    tr,
    messageFrom,
    getWorkspace: () => workspace,
    getEditor: () => workspacePanel,
    getRambleRequestId: () => rambleRequestId,
    getRambleEngaged: () => rambleEngaged,
    getInteractionLocked: () => interactionLocked || currentRequestCooking,
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

  $: dirty =
    workspace !== null &&
    workspace.request.status !== 'completed' &&
    workspace.request.status !== 'cancelled' &&
    draftBody !== savedBody
  $: {
    if (!pageError) deliveredPageError = ''
    else if (pageError !== deliveredPageError) {
      deliveredPageError = pageError
      toast.error(tr('操作失败'), { description: pageError })
    }
  }
  $: {
    if (!saveMessage) deliveredSaveError = ''
    else if (saveMessage !== deliveredSaveError) {
      deliveredSaveError = saveMessage
      toast.error(tr('保存失败'), { description: saveMessage })
    }
  }
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
  $: currentRequestCooking =
    workspace !== null && cookingRequestIds.has(workspace.request.request_id)
  $: cookedPreviewActive =
    cookedPreview !== null && draftBody === cookedPreview.markdown
  // Turning Cooking off discards any pending cooked preview: submitting then
  // publishes the editor content as-is.
  $: if (!$cookingEnabled) cookedPreview = null
  $: canSubmit =
    workspace !== null &&
    workspace.request.status !== 'completed' &&
    workspace.request.status !== 'cancelled' &&
    draftBody.trim().length > 0 &&
    !currentRequestCooking &&
    !submitting &&
    !cancelling
  $: canCancel =
    workspace !== null &&
    workspace.request.status !== 'completed' &&
    workspace.request.status !== 'cancelled' &&
    !currentRequestCooking &&
    !submitting &&
    !cancelling
  $: interactionLocked = submitting || cancelling || approving
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
  $: updateInstallBlocked =
    dirty ||
    rambleEngaged ||
    attachmentBusy ||
    submitting ||
    cancelling ||
    approving ||
    currentRequestCooking ||
    workspace?.request.status === 'in_progress'
  $: workspaceMinimumWidth =
    workbenchLayoutWidth > 1180 ? WIDE_WORKSPACE_MIN_WIDTH : NARROW_WORKSPACE_MIN_WIDTH
  $: requestWorkspacePaneWidth = Math.max(0, requestWorkspaceWidth - PANE_RESIZER_SIZE)
  $: requestListMinimumSize = requestWorkspacePaneWidth
    ? Math.min(100, (REQUEST_LIST_MIN_WIDTH / requestWorkspacePaneWidth) * 100)
    : 0
  $: desiredWorkspaceMinimumSize = requestWorkspacePaneWidth
    ? Math.min(100, (workspaceMinimumWidth / requestWorkspacePaneWidth) * 100)
    : 0
  $: workspaceMinimumSize = Math.min(
    desiredWorkspaceMinimumSize,
    Math.max(0, 100 - requestListMinimumSize),
  )
  $: requestListMaximumSize = Math.max(requestListMinimumSize, 100 - workspaceMinimumSize)
  $: saveHostRailCollapsed(hostSessionRailCollapsed)

  function saveRequestWorkspaceLayout(layout: number[]) {
    if (requestWorkspaceLayoutReady) savePaneLayout(REQUEST_WORKSPACE_LAYOUT_KEY, layout)
  }

  onMount(() => {
    const cleanupAttachments = attachmentController.mount()
    const syncLayoutDimensions = () => {
      workbenchLayoutWidth = workbenchLayout?.clientWidth ?? 0
      requestWorkspaceWidth = requestWorkspaceGroup?.clientWidth ?? 0
    }
    const layoutObserver = new ResizeObserver(syncLayoutDimensions)
    if (workbenchLayout) layoutObserver.observe(workbenchLayout)
    if (requestWorkspaceGroup) layoutObserver.observe(requestWorkspaceGroup)
    syncLayoutDimensions()
    void tick().then(() => {
      if (!requestWorkspacePaneGroup || requestWorkspacePaneWidth <= 0) return
      const defaultRequestListSize = Math.min(
        requestListMaximumSize,
        Math.max(
          requestListMinimumSize,
          (REQUEST_LIST_DEFAULT_WIDTH / requestWorkspacePaneWidth) * 100,
        ),
      )
      requestWorkspaceLayoutReady = true
      requestWorkspacePaneGroup.setLayout(
        savedRequestWorkspaceLayout ?? [defaultRequestListSize, 100 - defaultRequestListSize],
      )
    })
    const cleanupLayoutObserver = () => layoutObserver.disconnect()

    if (!isTauri) {
      startWorkbench()
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
      return () => {
        cleanupLayoutObserver()
        cleanupAttachments()
      }
    }
    if ($onboardingCompleted) startWorkbench()
    else onboardingOpen = true
    const updateCheckTimer = window.setTimeout(() => void checkForUpdates(), 4_000)
    void refreshNotificationPermission()
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
      draftController.cancelPendingSave()
      if (inboxTimer) clearInterval(inboxTimer)
      resumePromptUnlisten?.()
      openAdaptersUnlisten?.()
      clearTimeout(updateCheckTimer)
      cleanupLayoutObserver()
      cleanupAttachments()
    }
  })

  function startWorkbench() {
    if (workbenchInitialized) return
    workbenchInitialized = true
    void navigation.initialize()
    if (isTauri) inboxTimer = setInterval(() => void navigation.refreshNavigation(true), 5_000)
  }

  function closeOnboarding() {
    onboardingOpen = false
    startWorkbench()
  }

  function restartOnboarding() {
    resetOnboarding()
    settingsOpen = false
    onboardingOpen = true
  }

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
    publishedFeedback = null
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
    if (interactionLocked || workspace?.request.request_id === requestId) return
    if (saveCurrent && !(await saveDraftNow())) return
    if (requestId === rambleRequestId) await rambleMarkdownQueue.catch(() => {})

    loadingWorkspace = true
    pageError = ''
    completedResult = null
    publishedFeedback = null
    try {
      const next = previewMode
        ? previewWorkspaceFor(requestId)
        : await invoke<FeedbackWorkspaceView>('get_feedback_workspace', {
            requestId,
          })
      if (!next) throw new Error(tr('找不到这个反馈请求。'))
      workspace = next
      cookedPreview = null
      cookedPreviewOriginal = ''
      draftBody = next.draft.body_markdown
      savedBody = next.draft.body_markdown
      savedRevision = next.draft.saved_revision
      savePhase = next.draft.updated_at ? 'saved' : 'idle'
      saveMessage = ''
      attachmentMessage = ''
      await attachmentController.refreshPreviews(next)
      if (next.request.status === 'completed' && next.feedback) {
        publishedFeedback = previewMode
          ? {
              markdown: next.draft.body_markdown,
              uncooked_markdown: next.draft.body_markdown,
            }
          : normalizePublishedFeedback(
              await invoke<PublishedFeedbackPackage | null>('read_published_feedback', {
                requestId: next.request.request_id,
              }),
            )
      }
    } catch (cause) {
      pageError = messageFrom(cause)
    } finally {
      loadingWorkspace = false
    }
  }

  async function appendRambleMarkdown(requestId: string, markdown: string): Promise<void> {
    if (interactionLocked) return
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

  async function reloadWorkspace() {
    if (interactionLocked) return
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
      draftController.scheduleSave()
    }
  }

  function setCookingRequest(requestId: string, cooking: boolean) {
    const next = new Set(cookingRequestIds)
    if (cooking) next.add(requestId)
    else next.delete(requestId)
    cookingRequestIds = next
  }

  const cookingController = createCookingController({
    tr,
    messageFrom,
    getWorkspace: () => workspace,
    getDraftBody: () => draftBody,
    getSavedBody: () => savedBody,
    getCookingConfig: () => ({
      provider: $cookingProvider,
      apiKey: $cookingApiKey,
      baseUrl: $cookingBaseUrl,
      model: $cookingModel,
      reasoningEffort: $cookingReasoningEffort,
      locale: $locale,
    }),
    isCookingEnabled: () => $cookingEnabled,
    isCooking: () => currentRequestCooking,
    exitRamble: async () => {
      if (rambleCanExit) await exitRamble()
    },
    setDraftBody: (markdown) => {
      draftBody = markdown
    },
    setSavePhase: (phase) => {
      savePhase = phase
    },
    setSaveMessage: (message) => {
      saveMessage = message
    },
    saveDraftNow,
    applyEditorMarkdown: (markdown) => {
      workspacePanel?.applyExternalMarkdown(markdown)
    },
    setPageError: (message) => {
      pageError = message
    },
    setCooking: setCookingRequest,
    publishCooked: (input, cookedMarkdown, uncookedMarkdown) =>
      publisherController.publishFeedback(input, cookedMarkdown, uncookedMarkdown),
    setPreview: (preview) => {
      cookedPreview = preview
    },
    setPreviewOriginal: (original) => {
      cookedPreviewOriginal = original
    },
    getPreviewOriginal: () => cookedPreviewOriginal,
  })
  const cookPreviewOnly = cookingController.cookPreviewOnly
  const restoreOriginalAfterCook = cookingController.restoreOriginal

  const publisherController = createPublisherController({
    tr,
    messageFrom,
    isPreviewMode: () => previewMode,
    getWorkspace: () => workspace,
    setWorkspace: (next) => {
      workspace = next
    },
    setCompletedResult: (result) => {
      completedResult = result
    },
    setPublishedFeedback: (feedback) => {
      publishedFeedback = feedback
    },
    setSavePhase: (phase) => {
      savePhase = phase
    },
    setPageError: (message) => {
      pageError = message
    },
    getCanSubmit: () => canSubmit,
    getRambleCanExit: () => rambleCanExit,
    exitRamble,
    saveDraftNow,
    getDraftBody: () => draftBody,
    getSavedRevision: () => savedRevision,
    getCookingEnabled: () => $cookingEnabled,
    getPreviewActive: () => cookedPreviewActive,
    getPreview: () => cookedPreview,
    setPreview: (preview) => {
      cookedPreview = preview
    },
    setCooking: setCookingRequest,
    cookAndPublish: cookingController.cookAndPublish,
    setSubmitting: (value) => {
      submitting = value
    },
    setSubmitStage: (stage) => {
      submitStage = stage
    },
    refreshNavigation: (force) => navigation.refreshNavigation(force),
    showSubmittedToast: (cooked) => {
      toast.success(tr('反馈已提交'), {
        description: cooked ? tr('Cooked 与 Uncooked 反馈已发布') : tr('反馈包已发布'),
      })
    },
  })
  const submitFeedback = publisherController.submitFeedback

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
      toast.success(tr('已同意并结束'))
      await navigation.refreshNavigation(true)
    } catch (cause) {
      pageError = messageFrom(cause)
    } finally {
      approving = false
    }
  }

  async function cancelFeedback() {
    if (!workspace || !canCancel) return
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
      toast.success(tr('请求已取消'))
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
    interactionLocked={interactionLocked || currentRequestCooking}
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
    onNotifications={() => void openSettings('notifications')}
    onWindowError={(message) => (pageError = tr('窗口操作失败：{error}', { error: message }))}
  />

  <div bind:this={workbenchLayout} class="flex h-[calc(100%-46px)] min-h-0 min-w-0">
    <HostSessionRail
      bind:collapsed={hostSessionRailCollapsed}
      sessions={$navigation.hostSessions}
      activeHostId={$navigation.selectedHostId}
      activeHostSessionId={$navigation.selectedHostSessionId}
      loading={$navigation.loadingNavigation}
      {resolveHostProfile}
      onSelect={(hostId, hostSessionId) =>
        void navigation.selectScope(hostId, hostSessionId)}
      onSettings={() => void openSettings('general')}
    />

    <PaneGroup
      bind:this={requestWorkspacePaneGroup}
      bind:ref={requestWorkspaceGroup}
      direction="horizontal"
      class="min-h-0 min-w-0 flex-1"
      id="request-workspace-split"
      onLayoutChange={saveRequestWorkspaceLayout}
    >
      <Pane
        id="request-list-pane"
        defaultSize={28}
        minSize={requestListMinimumSize}
        maxSize={requestListMaximumSize}
      >
        <RequestListPane
          requests={$navigation.requests}
          activeRequestId={workspace?.request.request_id ?? null}
          cookingRequestIds={cookingRequestIds}
          scopeLabel={requestScopeLabel}
          loading={$navigation.loadingRequests}
          loadingMore={$navigation.loadingMoreRequests}
          hasMore={$navigation.nextRequestCursor !== null}
          {resolveHostProfile}
          formatTime={formatTimeLocal}
          onRefresh={() => void navigation.refreshRequests(false)}
          onLoadMore={() => void navigation.loadMoreRequests()}
          onOpenRequest={(requestId) => void openRequest(requestId)}
        />
      </Pane>

      <PaneResizer
        class="workbench-pane-resizer workbench-pane-resizer--vertical"
        aria-label={tr('调整请求列表宽度')}
      />

      <Pane id="workspace-pane" minSize={workspaceMinimumSize}>
        <WorkspacePanel
          bind:this={workspacePanel}
          bind:taskBriefOpen
          {loadingWorkspace}
          {workspace}
          {feedbackResult}
          {draftBody}
          {savedRevision}
          {savePhase}
          {attachmentPreviews}
          {dragActive}
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
          cooking={currentRequestCooking}
          cookingEnabled={$cookingEnabled}
          cookedPreviewActive={cookedPreviewActive}
          cookedPreviewModel={cookedPreview?.model ?? ''}
          onCookPreview={() => void cookPreviewOnly()}
          onRestoreOriginal={restoreOriginalAfterCook}
          {submitting}
          {submitStage}
          {publishedFeedback}
          {canCancel}
          {cancelling}
          {approving}
          {resolveHostProfile}
          formatTime={formatTimeLocal}
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
      </Pane>
    </PaneGroup>

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

<OnboardingWizard bind:openWizard={onboardingOpen} onClose={closeOnboarding} />

{#if settingsOpen}
  <SettingsPanel
    mcpConfiguration={genericMcpConfiguration}
    initialSection={settingsSection}
    {updateInstallBlocked}
    onRestartOnboarding={restartOnboarding}
    onClose={() => {
      settingsOpen = false
      void refreshNotificationPermission()
    }}
  />
{/if}
{/key}
