<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import {
    isPermissionGranted,
    sendNotification,
  } from '@tauri-apps/plugin-notification'
  import { onMount, tick } from 'svelte'
  import { Pane, PaneGroup, PaneResizer } from 'paneforge'

  import rambelleArchived from './assets/rambelle-states/archived.webp'
  import rambelleIdle from './assets/rambelle-states/idle.webp'
  import rambelleOrganizing from './assets/rambelle-states/organizing.webp'
  import rambelleRecording from './assets/rambelle-states/recording.webp'
  import AppTitlebar from './lib/AppTitlebar.svelte'
  import OnboardingWizard from './lib/OnboardingWizard.svelte'
  import UpdateAvailableDialog from './lib/UpdateAvailableDialog.svelte'
  import ArchivedSessionsDialog from './lib/components/navigation/ArchivedSessionsDialog.svelte'
  import HostSessionRail from './lib/components/navigation/HostSessionRail.svelte'
  import RequestListPane from './lib/components/navigation/RequestListPane.svelte'
  import { Sonner, toast } from './lib/components/ui/sonner'
  import ResumePromptDialog from './lib/workbench/ResumePromptDialog.svelte'
  import SessionWorkbench from './lib/workbench/SessionWorkbench.svelte'
  import MissingSessionView from './lib/workspace/MissingSessionView.svelte'
  import SettingsWorkspaceView from './lib/workspace/SettingsWorkspaceView.svelte'
  import type { JSONContent } from '@tiptap/core'

  import type {
    ApproveFeedbackInput,
    CancelFeedbackInput,
    DraftView,
    FeedbackRequestView,
    FeedbackWorkspaceView,
    HostSessionSummary,
    SubmitFeedbackInput,
  } from './lib/feedback'
  import {
    type ActiveAction,
    type DraftOperation,
  } from './lib/draftOperations'
  import { writeBackgroundDraftOperation } from './lib/backgroundDraftWriter'
  import {
    decodeFeedbackDraftDocument,
    restoreFeedbackDraftDocument,
    snapshotFeedbackDraftDocument,
    type FeedbackDraftSnapshot,
  } from './lib/feedbackDraftDocument'
  import {
    notificationLabel,
    notificationStateForPermission,
    type NotificationState,
  } from './lib/notifications'
  import { desktopPath } from './lib/nativePath'
  import { openExternalUrl } from './lib/openExternalUrl'
  import { currentDesktopPlatform } from './lib/platform'
  import { isWithinLast24Hours } from './lib/requestRecency'
  import { checkForUpdates } from './lib/updater'
  import {
    sessionViewDescriptor,
    settingsViewDescriptor,
    workspaceViewKey,
    type SessionViewDescriptor,
    type WorkspaceViewDescriptor,
  } from './lib/workspace/viewDescriptors'
  import {
    activeWorkspaceView,
    EMPTY_WORKSPACE_SHELL_STATE,
    workspaceShellReducer,
    type WorkspaceShellState,
  } from './lib/workspace/workspaceShell'
  import {
    createWorkspaceSnapshot,
  } from './lib/workspace/workspaceSnapshot'
  import {
    savedPreviewWorkspaceSnapshot,
    savePreviewWorkspaceSnapshot,
    seedPreviewWorkspaceScenario,
  } from './lib/workspace/previewWorkspaceSnapshot'
  import {
    createSessionViewRecoveryResolver,
    preserveLoadedSessionDuringUnconfirmedRecovery,
    sessionViewResolution,
    type SessionViewCatalog,
    type SessionViewResolution,
  } from './lib/workspace/sessionViewRecovery'
  import WorkspaceTabStrip from './lib/workspace/WorkspaceTabStrip.svelte'
  import {
    workspaceTabId,
    workspaceTabPanelId,
  } from './lib/workspace/workspaceTabNavigation'
  import {
    createSessionWorkspaceTransition,
    type SessionWorkspaceTransitionOutcome,
    type SessionWorkspaceTransitionTarget,
  } from './lib/workspace/sessionWorkspaceTransition'
  import { previewFixtures, previewWorkspaceFor } from './lib/previewFixtures'
  import {
    restorePublishedAttachmentUrls,
    normalizePublishedFeedback,
    type PublishedAttachmentPath,
    type PublishedFeedbackPackage,
    type PublishedFeedbackView,
  } from './lib/publishedFeedback'
  import {
    formatTime,
    messageFrom,
    operatorFeedbackBody,
  } from './lib/workbench/feedbackText'
  import { createCookingController } from './lib/workbench/cookingController'
  import { createDraftController } from './lib/workbench/draftController'
  import { createPublisherController } from './lib/workbench/publisherController'
  import { buildResumePrompt, shouldShowResumePromptButton } from './lib/workbench/resumePrompt'
  import {
    createAttachmentController,
    type AttachmentMessageTone,
  } from './lib/workbench/attachmentController'
  import { createNavigationController } from './lib/workbench/navigationController'
  import { resolvedRamblePhase } from './lib/workbench/rambleSessionState'
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
    saveWorkspaceSnapshot,
    savedPaneLayout,
    savedWorkspaceSnapshot,
  } from './lib/uiPreferences'
  import {
    cookingApiKey,
    cookingBaseUrl,
    cookingEnabled,
    cookingModel,
    cookingProvider,
    cookingReasoningEffort,
    cookingSystemPrompt,
    locale,
    notificationPopupEnabled,
    onboardingCompleted,
    resetOnboarding,
    notificationSoundEnabled,
    setNotificationPopupEnabled,
    tidyApiKey,
    tidyAutoThreshold,
    tidyBaseUrl,
    tidyModel,
    tidyProvider,
    tidyReasoningEffort,
    tidySystemPrompt,
  } from './lib/preferences'

  type PaneGroupHandle = {
    setLayout: (layout: number[]) => void
  }

  const RESUME_PROMPT_EVENT = 'rambledesk://resume-prompt'
  const OPEN_ADAPTERS_EVENT = 'rambledesk://open-adapters'
  const formatTimeLocal = (value: string | null | undefined) =>
    formatTime(value, $locale, tr('Not saved yet'))
  let workspace: FeedbackWorkspaceView | null = null
  let workspaceShellState: WorkspaceShellState = EMPTY_WORKSPACE_SHELL_STATE
  let renderedWorkspaceView: WorkspaceViewDescriptor | null = null
  let renderedSessionView: SessionViewDescriptor | null = null
  let renderedSessionResolution: SessionViewResolution | null = null
  let sessionViewResolutions: readonly SessionViewResolution[] = []
  let sessionRequestIds = new Map<string, string>()
  let pendingWorkspaceViewKey: string | null = null
  let workbenchMounted = true
  let completedResult: FeedbackRequestView | null = null
  let publishedFeedback: PublishedFeedbackView | null = null
  let draftBody = ''
  let savedBody = ''
  let draftDocumentJson = ''
  let savedDocumentJson = ''
  let editorDocument: JSONContent | null = null
  let editorEpoch = 0
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
  let cancelling = false
  let approving = false
  let attachmentBusy = false
  let screenCaptureBusy = false
  let attachmentMessage = ''
  let attachmentMessageTone: AttachmentMessageTone = 'info'
  let deliveredAttachmentMessage = ''
  let deliveredPageError = ''
  let deliveredSaveError = ''
  let attachmentPreviews: Record<string, string> = {}
  let dragActive = false
  let sessionWorkbench: FeedbackEditorHandle | undefined
  let rambleController: RambleSessionControllerHandle
  let resumePrompt: ResumePrompt | null = null
  let resumeCopyState: 'idle' | 'copied' | 'failed' = 'idle'
  let notificationState: NotificationState = 'checking'
  let archivedSessionsOpen = false
  let archivedInitialSession: SessionViewDescriptor | null = null
  let lastSessionRecoveryFingerprint = ''
  let activeRecoveryTransition: object | null = null
  let settingsSection: SettingsSection = 'general'
  let onboardingOpen = false
  let launchUpdateCheckDue = false
  let workbenchInitialized = false
  const isTauri = '__TAURI_INTERNALS__' in window
  const isMac = currentDesktopPlatform() === 'macOS'
  const previewMode =
    import.meta.env.DEV &&
    !isTauri &&
    new URLSearchParams(window.location.search).get('preview') === 'fixtures'
  const previewWorkspaceScenario = previewMode
    ? seedPreviewWorkspaceScenario(
        new URLSearchParams(window.location.search).get('workspace'),
      )
    : null
  const initialWorkspaceSnapshot = previewMode
    ? savedPreviewWorkspaceSnapshot()
    : savedWorkspaceSnapshot()
  if (initialWorkspaceSnapshot) {
    workspaceShellState = initialWorkspaceSnapshot.shellState
    sessionRequestIds = new Map(initialWorkspaceSnapshot.requestIds)
    if (initialWorkspaceSnapshot.shellState.activeViewKey) {
      workbenchMounted = false
      loadingWorkspace = true
    }
  }
  const REQUEST_LIST_DEFAULT_WIDTH = 296
  const REQUEST_LIST_MIN_WIDTH = 240
  const WIDE_WORKSPACE_MIN_WIDTH = 648
  const NARROW_WORKSPACE_MIN_WIDTH = 360
  const PANE_RESIZER_SIZE = 11
  const REQUEST_WORKSPACE_LAYOUT_KEY = 'request-workspace-layout'
  const savedRequestWorkspaceLayout = savedPaneLayout(REQUEST_WORKSPACE_LAYOUT_KEY)

  let taskBriefOpen = true
  let todayOnly = false
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
  let rambleDocumentQueue: Promise<void> = Promise.resolve()
  let activeActionByRequest = new Map<string, NonNullable<ActiveAction>>()
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
    getSnapshot: () => currentDraftSnapshot(),
    setSnapshot: (snapshot) => applyDraftSnapshot(snapshot),
    getSavedSnapshot: () => savedDraftSnapshot(),
    setSavedSnapshot: (snapshot) => {
      savedDocumentJson = snapshot.documentJson
      savedBody = snapshot.bodyMarkdown
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
    getEditor: () => sessionWorkbench,
    getRambleRequestId: () => rambleRequestId,
    getInteractionLocked: () => interactionLocked || currentRequestCooking || cookedDraftReady,
    getSavedRevision: () => savedRevision,
    getBusy: () => attachmentBusy,
    getCaptureBusy: () => screenCaptureBusy,
    getPreviews: () => attachmentPreviews,
    setBusy: (busy) => (attachmentBusy = busy),
    setCaptureBusy: (busy) => (screenCaptureBusy = busy),
    setMessage: (message, tone) => {
      if (tone) attachmentMessageTone = tone
      attachmentMessage = message
    },
    setPreviews: (previews) => (attachmentPreviews = previews),
    setDragActive: (active) => (dragActive = active),
    saveDraftNow,
    waitForRambleMarkdown: () => rambleDocumentQueue.catch(() => {}),
    routeDraftOperation,
    activeActionFor,
    applyWorkspaceMutation,
  })

  type LoadedSessionWorkspace = Readonly<{
    workspace: FeedbackWorkspaceView
    publishedFeedback: PublishedFeedbackView | null
  }>

  const sessionWorkspaceTransition = createSessionWorkspaceTransition<LoadedSessionWorkspace>({
    saveCurrent: saveDraftNow,
    unmountCurrent: () => {
      workbenchMounted = false
      sessionWorkbench = undefined
      loadingWorkspace = true
    },
    loadTarget: loadSessionWorkspaceTarget,
    commitTarget: commitSessionWorkspaceTarget,
    restoreCurrent: () => {
      workbenchMounted = true
      loadingWorkspace = false
    },
    setPendingTarget: (target) => {
      pendingWorkspaceViewKey = target?.pendingViewKey ?? null
    },
    reportFailure: (cause) => {
      pageError = messageFrom(cause)
    },
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
    canSendOsBanners: () => isMac,
  })
  const resolveHostProfile = navigation.resolveHostProfile
  const sessionViewRecoveryResolver = createSessionViewRecoveryResolver({
    loadArchived: async () => {
      const sessions =
        previewMode || !isTauri
          ? previewWorkspaceScenario === 'unknown'
            ? await Promise.reject(new Error('Preview archived catalog unavailable'))
            : previewFixtures.archivedHostSessions
          : await invoke<HostSessionSummary[]>('list_archived_host_sessions', {
              input: { search: null },
            })
      return sessions.map((session) =>
        sessionViewDescriptor(session.host_id, session.host_session_id),
      )
    },
    onInvalidate: () => {
      if (!activeRecoveryTransition) return
      activeRecoveryTransition = null
      sessionWorkspaceTransition.invalidate()
    },
    onUpdate: applySessionViewResolutions,
  })

  function currentDraftSnapshot(): FeedbackDraftSnapshot {
    return { documentJson: draftDocumentJson, bodyMarkdown: draftBody }
  }

  function savedDraftSnapshot(): FeedbackDraftSnapshot {
    return { documentJson: savedDocumentJson, bodyMarkdown: savedBody }
  }

  function applyDraftSnapshot(snapshot: FeedbackDraftSnapshot) {
    draftDocumentJson = snapshot.documentJson
    draftBody = snapshot.bodyMarkdown
    editorDocument = decodeFeedbackDraftDocument(snapshot.documentJson)
  }

  function adoptDraft(draft: DraftView, options: { loadEditor?: boolean } = {}) {
    const restored = restoreFeedbackDraftDocument(draft.document_json, draft.body_markdown)
    const snapshot = snapshotFeedbackDraftDocument(restored)
    applyDraftSnapshot(snapshot)
    savedDocumentJson = snapshot.documentJson
    savedBody = snapshot.bodyMarkdown
    savedRevision = draft.saved_revision
    if (options.loadEditor !== false) {
      editorDocument = restored
      editorEpoch += 1
    }
    return snapshot
  }

  $: dirty =
    workspace !== null &&
    workspace.request.status !== 'completed' &&
    workspace.request.status !== 'cancelled' &&
    draftDocumentJson !== savedDocumentJson
  $: {
    if (!pageError) deliveredPageError = ''
    else if (pageError !== deliveredPageError) {
      deliveredPageError = pageError
      toast.error(tr('Operation failed'), { description: pageError })
    }
  }
  $: {
    if (!saveMessage) deliveredSaveError = ''
    else if (saveMessage !== deliveredSaveError) {
      deliveredSaveError = saveMessage
      toast.error(tr('Save failed'), { description: saveMessage })
    }
  }
  $: {
    if (!attachmentMessage) {
      deliveredAttachmentMessage = ''
    } else if (attachmentMessage !== deliveredAttachmentMessage) {
      deliveredAttachmentMessage = attachmentMessage
      const options = { description: attachmentMessage }
      if (attachmentMessageTone === 'success') toast.success(tr('Attachment action completed'), options)
      else if (attachmentMessageTone === 'info') toast.info(tr('Attachment status'), options)
      else toast.error(tr('Attachment action failed'), options)
    }
  }
  $: visibleRequests = todayOnly
    ? $navigation.requests.filter((request) => isWithinLast24Hours(request.updated_at))
    : $navigation.requests
  $: selectedHostSession = $navigation.selectedHostSessionId
    ? $navigation.hostSessions.find(
        (session) =>
          session.host_id === $navigation.selectedHostId &&
          session.host_session_id === $navigation.selectedHostSessionId,
      )
    : undefined
  $: renderedWorkspaceView = activeWorkspaceView(workspaceShellState)
  $: renderedSessionView = renderedWorkspaceView?.kind === 'session'
    ? renderedWorkspaceView
    : null
  $: renderedSessionResolution = sessionViewResolution(
    sessionViewResolutions,
    workspaceShellState.activeViewKey,
  )
  const sessionTabLabel = (view: SessionViewDescriptor) => {
    const session = $navigation.hostSessions.find(
      (candidate) =>
        candidate.host_id === view.hostId &&
        candidate.host_session_id === view.hostSessionId,
    )
    const hostLabel = resolveHostProfile(view.hostId).label
    return `${session?.title ?? view.hostSessionId} · ${hostLabel}`
  }
  const workspaceTabLabel = (view: WorkspaceViewDescriptor) =>
    view.kind === 'settings' ? tr('Settings') : sessionTabLabel(view)
  $: requestScopeLabel = $navigation.selectedHostId
    ? $navigation.selectedHostSessionId
      ? selectedHostSession?.source_hint ??
        selectedHostSession?.title ??
        resolveHostProfile($navigation.selectedHostId).label
      : resolveHostProfile($navigation.selectedHostId).label
    : tr('All hosts')
  $: feedbackResult = completedResult?.feedback ?? workspace?.feedback ?? null
  $: canOpenResumePrompt = shouldShowResumePromptButton(
    feedbackResult,
    completedResult?.resolution ?? workspace?.request.resolution,
  )
  $: currentRequestCooking =
    workspace !== null && cookingRequestIds.has(workspace.request.request_id)
  $: cookedDraftReady = cookedPreview !== null
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
  $: workspaceTransitionLocked =
    interactionLocked ||
    attachmentBusy ||
    screenCaptureBusy ||
    currentRequestCooking ||
    cookedDraftReady
  $: {
    const recoveryFingerprint = `${$navigation.hostSessionFactsStatus}:${$navigation.hostSessionFactsRevision}:${workspaceShellState.views
      .map(workspaceViewKey)
      .join('\u0001')}:${$navigation.hostSessions
      .map((session) =>
        workspaceViewKey(sessionViewDescriptor(session.host_id, session.host_session_id)),
      )
      .sort()
      .join('\u0001')}`
    if (
      $navigation.hostSessionFactsStatus !== 'pending' &&
      recoveryFingerprint !== lastSessionRecoveryFingerprint
    ) {
      lastSessionRecoveryFingerprint = recoveryFingerprint
      void refreshSessionViewRecovery()
    }
  }
  $: voiceActive =
    voicePhase === 'starting' ||
    voicePhase === 'listening' ||
    voicePhase === 'processing' ||
    voicePhase === 'stopping'
  $: voiceCanStop =
    voiceActive || voicePhase === 'error'
  $: visibleRamblePhase = resolvedRamblePhase(ramblePhase, voicePhase)
  $: rambleActive = visibleRamblePhase === 'active'
  $: rambleEngaged = visibleRamblePhase !== 'idle'
  $: rambleBelongsToWorkspace =
    !rambleEngaged || workspace?.request.request_id === rambleRequestId
  $: rambelleStatusPortrait = feedbackResult
    ? rambelleArchived
    : currentRequestCooking
      ? rambelleOrganizing
      : rambleActive
        ? rambelleRecording
        : rambleEngaged
          ? rambelleOrganizing
          : rambelleIdle
  $: rambleBusy = visibleRamblePhase === 'starting' || visibleRamblePhase === 'stopping'
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
        if (!initialWorkspaceSnapshot) {
          workspace = previewFixtures.workspace
          adoptDraft(previewFixtures.workspace.draft)
          openLoadedWorkspaceView(previewFixtures.workspace)
          savePhase = 'saved'
        }
        if (new URLSearchParams(window.location.search).get('dialog') === 'resume') {
          resumePrompt = previewFixtures.resumePrompt
        }
      }
      notificationState = 'unavailable'
      if (new URLSearchParams(window.location.search).get('dialog') === 'update') {
        void checkForUpdates({ prompt: true, forcePrompt: true })
      }
      return () => {
        cleanupLayoutObserver()
        cleanupAttachments()
      }
    }
    if ($onboardingCompleted) startWorkbench()
    else onboardingOpen = true
    const updateCheckTimer = window.setTimeout(() => {
      launchUpdateCheckDue = true
      if (!onboardingOpen) void checkForUpdates({ prompt: true })
    }, 4_000)
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
      if (isMac && $notificationPopupEnabled && notificationState === 'enabled') {
        sendNotification({
          title: event.payload.title,
          body: tr('Return to {host} and use the resume prompt to continue the host session.', {
            host: event.payload.host_label,
          }),
        })
      }
      // The alert sound is reserved for a new request arriving, not for the
      // resume prompt shown after a submission completes, so it is not played
      // here.
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
      if (updateCheckTimer !== undefined) clearTimeout(updateCheckTimer)
      cleanupLayoutObserver()
      cleanupAttachments()
    }
  })

  function startWorkbench() {
    if (workbenchInitialized) return
    workbenchInitialized = true
    void (async () => {
      await navigation.initialize(initialWorkspaceSnapshot === null)
      await refreshSessionViewRecovery()
      if (initialWorkspaceSnapshot) await restoreInitialWorkspaceSnapshot()
    })()
    if (isTauri) inboxTimer = setInterval(() => void navigation.refreshNavigation(true), 5_000)
  }

  function closeOnboarding() {
    onboardingOpen = false
    startWorkbench()
    if (launchUpdateCheckDue) void checkForUpdates({ prompt: true })
  }

  async function openGithubReleases() {
    const releasesUrl = 'https://github.com/l1veIn/rambledesk/releases'
    try {
      await openExternalUrl(releasesUrl)
    } catch (cause) {
      pageError = messageFrom(cause)
    }
  }

  function restartOnboarding() {
    resetOnboarding()
    void closeWorkspaceTab(workspaceViewKey(settingsViewDescriptor()))
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

  function openResumePrompt() {
    if (!workspace || !canOpenResumePrompt) return
    resumePrompt = buildResumePrompt(workspace, resolveHostProfile(workspace.request.host_id), tr)
    resumeCopyState = 'idle'
  }

  function clearWorkspace() {
    workspace = null
    completedResult = null
    publishedFeedback = null
    attachmentController.releasePreviews()
  }

  function persistCurrentWorkspaceSnapshot() {
    const snapshot = createWorkspaceSnapshot(workspaceShellState, sessionRequestIds)
    if (previewMode) {
      savePreviewWorkspaceSnapshot(snapshot)
      return
    }
    saveWorkspaceSnapshot(snapshot)
  }

  function openSessionView(view: SessionViewDescriptor, requestId?: string) {
    if (requestId) {
      const nextRequestIds = new Map(sessionRequestIds)
      nextRequestIds.set(workspaceViewKey(view), requestId)
      sessionRequestIds = nextRequestIds
    }
    workspaceShellState = workspaceShellReducer(workspaceShellState, { type: 'open', view })
    persistCurrentWorkspaceSnapshot()
  }

  function openLoadedWorkspaceView(next: FeedbackWorkspaceView) {
    openSessionView(
      sessionViewDescriptor(next.request.host_id, next.request.host_session_id),
      next.request.request_id,
    )
  }

  async function applySessionViewResolutions(
    resolutions: readonly SessionViewResolution[],
  ) {
    const activeView = activeWorkspaceView(workspaceShellState)
    const workspaceView = workspace
      ? sessionViewDescriptor(workspace.request.host_id, workspace.request.host_session_id)
      : null
    const safeResolutions = preserveLoadedSessionDuringUnconfirmedRecovery(
      resolutions,
      workspaceView,
    )
    const nextActive = sessionViewResolution(
      safeResolutions,
      workspaceShellState.activeViewKey,
    )
    if (
      activeView?.kind === 'session' &&
      nextActive?.kind === 'missing-session' &&
      workspaceView &&
      workspaceViewKey(workspaceView) === workspaceViewKey(activeView)
    ) {
      const recoveryTransition = {}
      activeRecoveryTransition = recoveryTransition
      const outcome = await sessionWorkspaceTransition.activate({
        view: activeView,
        requestId: null,
        shellAction: { type: 'open' },
        pendingViewKey: workspaceViewKey(activeView),
      })
      if (activeRecoveryTransition === recoveryTransition) {
        activeRecoveryTransition = null
      }
      if (outcome !== 'activated') return false
      await navigation.selectScope(null, null)
    }
    sessionViewResolutions = safeResolutions
    return true
  }

  function activeSessionCatalog(): SessionViewCatalog {
    if ($navigation.hostSessionFactsStatus === 'pending') return { status: 'pending' }
    if ($navigation.hostSessionFactsStatus === 'failed') return { status: 'failed' }
    return {
      status: 'ready',
      views: $navigation.hostSessions.map((session) =>
        sessionViewDescriptor(session.host_id, session.host_session_id),
      ),
    }
  }

  async function refreshSessionViewRecovery() {
    return sessionViewRecoveryResolver.refresh(
      workspaceShellState.views.filter(
        (view): view is SessionViewDescriptor => view.kind === 'session',
      ),
      activeSessionCatalog(),
    )
  }

  async function retrySessionViewRecovery() {
    const retryingMissingView = renderedSessionResolution?.kind === 'missing-session'
    if (retryingMissingView) {
      workbenchMounted = false
      loadingWorkspace = true
    }
    await navigation.refreshNavigation(true)
    lastSessionRecoveryFingerprint = ''
    await refreshSessionViewRecovery()
    const activeResolution = sessionViewResolution(
      sessionViewResolutions,
      workspaceShellState.activeViewKey,
    )
    if (activeResolution?.kind === 'active' && workspace === null) {
      await restoreInitialWorkspaceSnapshot()
    } else if (retryingMissingView) {
      workbenchMounted = true
      loadingWorkspace = false
    }
  }

  async function restoreInitialWorkspaceSnapshot() {
    const view = activeWorkspaceView(workspaceShellState)
    if (!view) {
      clearWorkspace()
      workbenchMounted = true
      loadingWorkspace = false
      return
    }
    if (view.kind === 'settings') {
      clearWorkspace()
      workbenchMounted = true
      loadingWorkspace = false
      void refreshGenericMcpConfiguration()
      return
    }

    const resolution = sessionViewResolution(sessionViewResolutions, workspaceViewKey(view))
    if (!resolution || resolution.kind !== 'active') {
      clearWorkspace()
      workbenchMounted = true
      loadingWorkspace = false
      return
    }

    const selection = await navigation.selectScope(view.hostId, view.hostSessionId)
    if (!selection.selected) {
      workbenchMounted = true
      loadingWorkspace = false
      return
    }
    const rememberedRequestId = sessionRequestIds.get(workspaceViewKey(view))
    const request =
      selection.requests.find((candidate) => candidate.request_id === rememberedRequestId) ??
      selection.requests[0]
    await sessionWorkspaceTransition.activate({
      view,
      requestId: request?.request_id ?? null,
      shellAction: { type: 'open' },
      pendingViewKey: workspaceViewKey(view),
    })
  }

  type NavigationScope = Readonly<{
    hostId: string | null
    hostSessionId: string | null
  }>

  function currentNavigationScope(): NavigationScope {
    return {
      hostId: $navigation.selectedHostId,
      hostSessionId: $navigation.selectedHostSessionId,
    }
  }

  async function restoreNavigationScope(
    scope: NavigationScope,
    outcome: SessionWorkspaceTransitionOutcome,
  ) {
    if (outcome !== 'blocked' && outcome !== 'failed') return
    await navigation.selectScope(scope.hostId, scope.hostSessionId)
  }

  async function selectRailScope(hostId: string | null, hostSessionId: string | null) {
    if (workspaceTransitionLocked) return
    const priorScope = currentNavigationScope()
    sessionWorkspaceTransition.invalidate()
    const selection = await navigation.selectScope(hostId, hostSessionId)
    if (!selection.selected || !hostId || !hostSessionId) return
    if (workspaceTransitionLocked) {
      await navigation.selectScope(priorScope.hostId, priorScope.hostSessionId)
      return
    }

    const view = sessionViewDescriptor(hostId, hostSessionId)
    const viewKey = workspaceViewKey(view)
    const rememberedRequestId = sessionRequestIds.get(viewKey)
    const request =
      selection.requests.find((candidate) => candidate.request_id === rememberedRequestId) ??
      selection.requests[0]
    if (request) {
      const outcome = await activateRequest(request.request_id)
      await restoreNavigationScope(priorScope, outcome)
      return
    }
    const outcome = await sessionWorkspaceTransition.activate({
      view,
      requestId: null,
      shellAction: { type: 'open' },
      pendingViewKey: viewKey,
    })
    await restoreNavigationScope(priorScope, outcome)
  }

  async function activateWorkspaceTab(viewKey: string) {
    if (workspaceTransitionLocked || workspaceShellState.activeViewKey === viewKey) return
    const priorScope = currentNavigationScope()
    sessionWorkspaceTransition.invalidate()
    const view = workspaceShellState.views.find(
      (candidate) => workspaceViewKey(candidate) === viewKey,
    )
    if (!view) return
    if (view.kind === 'settings') {
      const outcome = await sessionWorkspaceTransition.activate({
        view,
        requestId: null,
        shellAction: { type: 'open' },
        pendingViewKey: viewKey,
      })
      if (outcome === 'activated') void refreshGenericMcpConfiguration()
      return
    }
    const resolution = sessionViewResolution(sessionViewResolutions, viewKey)
    if (resolution?.kind === 'missing-session') {
      const outcome = await sessionWorkspaceTransition.activate({
        view,
        requestId: null,
        shellAction: { type: 'open' },
        pendingViewKey: viewKey,
      })
      if (outcome === 'activated') await navigation.selectScope(null, null)
      return
    }

    const selection = await navigation.selectScope(view.hostId, view.hostSessionId)
    if (!selection.selected) return
    if (workspaceTransitionLocked) {
      await navigation.selectScope(priorScope.hostId, priorScope.hostSessionId)
      return
    }
    const rememberedRequestId = sessionRequestIds.get(viewKey)
    const request =
      selection.requests.find((candidate) => candidate.request_id === rememberedRequestId) ??
      selection.requests[0]
    if (request) {
      const outcome = await activateRequest(request.request_id)
      await restoreNavigationScope(priorScope, outcome)
      return
    }
    const outcome = await sessionWorkspaceTransition.activate({
      view,
      requestId: null,
      shellAction: { type: 'open' },
      pendingViewKey: viewKey,
    })
    await restoreNavigationScope(priorScope, outcome)
  }

  async function closeWorkspaceTab(viewKey: string) {
    if (workspaceTransitionLocked || pendingWorkspaceViewKey) return
    const closingActive = workspaceShellState.activeViewKey === viewKey
    if (!closingActive) {
      workspaceShellState = workspaceShellReducer(workspaceShellState, {
        type: 'close',
        viewKey,
      })
      const nextRequestIds = new Map(sessionRequestIds)
      nextRequestIds.delete(viewKey)
      sessionRequestIds = nextRequestIds
      persistCurrentWorkspaceSnapshot()
      return
    }
    sessionWorkspaceTransition.invalidate()
    const priorScope = currentNavigationScope()

    const nextShellState = workspaceShellReducer(workspaceShellState, {
      type: 'close',
      viewKey,
    })
    const fallbackView = activeWorkspaceView(nextShellState)
    const fallbackResolution = fallbackView?.kind === 'session'
      ? sessionViewResolution(sessionViewResolutions, workspaceViewKey(fallbackView))
      : null
    let fallbackRequestId: string | null = null
    if (fallbackView?.kind === 'session' && fallbackResolution?.kind !== 'missing-session') {
      const selection = await navigation.selectScope(
        fallbackView.hostId,
        fallbackView.hostSessionId,
      )
      if (!selection.selected) return
      if (workspaceTransitionLocked) {
        await navigation.selectScope(priorScope.hostId, priorScope.hostSessionId)
        return
      }
      const rememberedRequestId = sessionRequestIds.get(workspaceViewKey(fallbackView))
      fallbackRequestId =
        selection.requests.find((candidate) => candidate.request_id === rememberedRequestId)
          ?.request_id ??
        selection.requests[0]?.request_id ??
        null
    }

    const outcome = await sessionWorkspaceTransition.activate({
      view: fallbackView,
      requestId: fallbackRequestId,
      shellAction: { type: 'close', viewKey },
      pendingViewKey: viewKey,
    })
    if (
      outcome === 'activated' &&
      (!fallbackView ||
        (fallbackView.kind === 'session' && fallbackResolution?.kind === 'missing-session'))
    ) {
      await navigation.selectScope(null, null)
    }
    await restoreNavigationScope(priorScope, outcome)
  }

  async function refreshNotificationPermission() {
    try {
      const granted = await isPermissionGranted()
      if (isMac && !granted && $notificationPopupEnabled) setNotificationPopupEnabled(false)
      notificationState = notificationStateForPermission(granted, $notificationPopupEnabled)
    } catch {
      notificationState = 'unavailable'
    }
  }

  function viewForRequest(requestId: string): SessionViewDescriptor | null {
    const request = [...$navigation.requests, ...$navigation.pendingRequests].find(
      (candidate) => candidate.request_id === requestId,
    )
    return request
      ? sessionViewDescriptor(request.host_id, request.host_session_id)
      : workspace?.request.request_id === requestId
        ? sessionViewDescriptor(workspace.request.host_id, workspace.request.host_session_id)
        : null
  }

  async function loadSessionWorkspaceTarget(
    target: SessionWorkspaceTransitionTarget,
  ): Promise<LoadedSessionWorkspace | null> {
    if (!target.requestId) return null
    return enqueueDocumentTask(async () => {
      const next = previewMode
        ? previewWorkspaceFor(target.requestId as string)
        : await invoke<FeedbackWorkspaceView>('get_feedback_workspace', {
            requestId: target.requestId,
          })
      if (!next) throw new Error(tr('This feedback request could not be found.'))

      const loadedView = sessionViewDescriptor(next.request.host_id, next.request.host_session_id)
      if (target.view && workspaceViewKey(target.view) !== workspaceViewKey(loadedView)) {
        throw new Error(tr('The feedback request no longer belongs to the selected session.'))
      }

      const nextPublishedFeedback =
        next.request.status === 'completed' && next.feedback
          ? previewMode
            ? {
                markdown: next.draft.body_markdown,
                uncooked_markdown: next.draft.body_markdown,
              }
            : normalizePublishedFeedback(
                await invoke<PublishedFeedbackPackage | null>('read_published_feedback', {
                  requestId: next.request.request_id,
                }),
              )
          : null
      return { workspace: next, publishedFeedback: nextPublishedFeedback }
    })
  }

  function commitSessionWorkspaceTarget(
    target: SessionWorkspaceTransitionTarget,
    loaded: LoadedSessionWorkspace | null,
  ) {
    const loadedView = loaded
      ? sessionViewDescriptor(
          loaded.workspace.request.host_id,
          loaded.workspace.request.host_session_id,
        )
      : target.view
    if (target.shellAction.type === 'open' && !loadedView) {
      throw new Error(tr('This feedback request could not be found.'))
    }

    const nextShellState =
      target.shellAction.type === 'close'
        ? workspaceShellReducer(workspaceShellState, target.shellAction)
        : workspaceShellReducer(workspaceShellState, { type: 'open', view: loadedView! })

    if (loaded) {
      attachmentController.releasePreviews()
      workspace = loaded.workspace
      completedResult = null
      publishedFeedback = loaded.publishedFeedback
      cookedPreview = null
      adoptDraft(loaded.workspace.draft)
      savePhase = loaded.workspace.draft.updated_at ? 'saved' : 'idle'
      saveMessage = ''
      attachmentMessage = ''
      const nextRequestIds = new Map(sessionRequestIds)
      nextRequestIds.set(workspaceViewKey(loadedView!), loaded.workspace.request.request_id)
      if (target.shellAction.type === 'close') nextRequestIds.delete(target.shellAction.viewKey)
      sessionRequestIds = nextRequestIds
    } else {
      clearWorkspace()
      if (target.shellAction.type === 'close') {
        const nextRequestIds = new Map(sessionRequestIds)
        nextRequestIds.delete(target.shellAction.viewKey)
        sessionRequestIds = nextRequestIds
      }
    }

    workspaceShellState = nextShellState
    persistCurrentWorkspaceSnapshot()
    workbenchMounted = true
    loadingWorkspace = false
    if (loaded) void attachmentController.refreshPreviews(loaded.workspace)
  }

  async function activateRequest(
    requestId: string,
  ): Promise<SessionWorkspaceTransitionOutcome> {
    if (workspaceTransitionLocked) return 'blocked'
    sessionWorkspaceTransition.invalidate()
    if (workspace?.request.request_id === requestId) {
      openLoadedWorkspaceView(workspace)
      return 'activated'
    }
    pageError = ''
    const view = viewForRequest(requestId)
    return sessionWorkspaceTransition.activate({
      view,
      requestId,
      shellAction: { type: 'open' },
      pendingViewKey: view ? workspaceViewKey(view) : `request:${JSON.stringify(requestId)}`,
    })
  }

  async function openRequest(requestId: string, _saveCurrent = true): Promise<boolean> {
    return (await activateRequest(requestId)) === 'activated'
  }

  function activeActionFor(requestId: string): ActiveAction {
    return activeActionByRequest.get(requestId) ?? null
  }

  function enqueueDocumentTask<T>(task: () => Promise<T>): Promise<T> {
    const run = rambleDocumentQueue.then(task)
    rambleDocumentQueue = run.then(
      () => undefined,
      () => undefined,
    )
    return run
  }

  async function routeDraftOperation(requestId: string, operation: DraftOperation): Promise<void> {
    if (!requestId) return
    const run = enqueueDocumentTask(async () => {
      if (workbenchMounted && workspace?.request.request_id === requestId) {
        if (
          workspace.request.status === 'completed' ||
          workspace.request.status === 'cancelled'
        ) {
          throw new Error(tr('This request is closed. The document is read-only.'))
        }
        let applied = sessionWorkbench?.applyDraftOperation(operation) ?? false
        if (!applied) {
          await tick()
          applied = sessionWorkbench?.applyDraftOperation(operation) ?? false
        }
        if (!applied) {
          throw new Error(tr('The current editor is not ready. Try the action again.'))
        }
        if (!(await saveDraftNow())) {
          throw new Error(saveMessage || tr('The current draft could not be saved.'))
        }
        return
      }

      await writeBackgroundDraftOperation(requestId, operation, {
        load: async () => {
          const target = previewMode
            ? previewWorkspaceFor(requestId)
            : await invoke<FeedbackWorkspaceView>('get_feedback_workspace', { requestId })
          if (!target) throw new Error(tr('This feedback request could not be found.'))
          return target
        },
        save: async (input) =>
          previewMode
            ? {
                document_json: input.document_json,
                body_markdown: input.body_markdown,
                saved_revision: input.expected_revision + 1,
                updated_at: new Date().toISOString(),
              }
            : invoke<DraftView>('save_feedback_draft', { input }),
      })
    })
    try {
      await run
    } catch (cause) {
      pageError = tr('Failed to write Ramble content: {error}', { error: messageFrom(cause) })
      throw cause
    }
  }

  function selectAction(actionId: string, actionIndex: number, title: string) {
    const requestId = workspace?.request.request_id
    if (!requestId || cookedDraftReady) return
    if (activeActionByRequest.get(requestId)?.actionId === actionId) {
      activeActionByRequest.delete(requestId)
      activeActionByRequest = new Map(activeActionByRequest)
      void routeDraftOperation(requestId, { kind: 'clearActionGroup', actionId }).catch(() => {})
      return
    }
    const action = { actionId, actionIndex, title }
    activeActionByRequest.set(requestId, action)
    activeActionByRequest = new Map(activeActionByRequest)
    void routeDraftOperation(requestId, { kind: 'startActionGroup', action }).catch(() => {})
  }

  async function reloadWorkspace() {
    if (workspaceTransitionLocked) return
    const requestId = workspace?.request.request_id
    if (!requestId) return
    if (rambleCanExit) await exitRamble()
    const view = sessionViewDescriptor(
      workspace!.request.host_id,
      workspace!.request.host_session_id,
    )
    await sessionWorkspaceTransition.activate({
      view,
      requestId,
      shellAction: { type: 'open' },
      pendingViewKey: workspaceViewKey(view),
    })
  }

  async function refreshGenericMcpConfiguration() {
    pageError = ''
    if (!isTauri) return
    try {
      genericMcpConfiguration = await invoke<string>('get_generic_mcp_configuration')
    } catch (cause) {
      pageError = messageFrom(cause)
    }
  }

  async function openSettings(section: SettingsSection) {
    settingsSection = section
    const view = settingsViewDescriptor()
    const viewKey = workspaceViewKey(view)
    if (workspaceTransitionLocked || pendingWorkspaceViewKey) return
    if (workspaceShellState.activeViewKey !== viewKey) {
      sessionWorkspaceTransition.invalidate()
      const outcome = await sessionWorkspaceTransition.activate({
        view,
        requestId: null,
        shellAction: { type: 'open' },
        pendingViewKey: viewKey,
      })
      if (outcome !== 'activated') return
    }
    await refreshGenericMcpConfiguration()
  }

  function openArchivedSessions(initialSession: SessionViewDescriptor | null = null) {
    archivedInitialSession = initialSession
    archivedSessionsOpen = true
  }

  function applyWorkspaceMutation(next: FeedbackWorkspaceView) {
    const localSnapshot = currentDraftSnapshot()
    workspace = next
    savedRevision = next.draft.saved_revision
    const remote = snapshotFeedbackDraftDocument(
      restoreFeedbackDraftDocument(next.draft.document_json, next.draft.body_markdown),
    )
    savedDocumentJson = remote.documentJson
    savedBody = remote.bodyMarkdown
    if (localSnapshot.documentJson === remote.documentJson) {
      applyDraftSnapshot(remote)
      savePhase = 'saved'
    } else {
      applyDraftSnapshot(localSnapshot)
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
    getCookingConfig: () => ({
      provider: $cookingProvider,
      apiKey: $cookingApiKey,
      baseUrl: $cookingBaseUrl,
      model: $cookingModel,
      reasoningEffort: $cookingReasoningEffort,
      locale: $locale,
      systemPrompt: $cookingSystemPrompt,
    }),
    isCookingEnabled: () => $cookingEnabled,
    isCooking: () => currentRequestCooking,
    exitRamble: async () => {
      if (rambleCanExit) await exitRamble()
    },
    saveDraftNow,
    setPageError: (message) => {
      pageError = message
    },
    setCooking: setCookingRequest,
    publishCooked: (input, cookedMarkdown, uncookedMarkdown) =>
      publisherController.publishFeedback(input, cookedMarkdown, uncookedMarkdown),
    setPreview: (preview) => {
      cookedPreview = preview
    },
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
    refreshNavigation: async (force) => {
      await navigation.refreshNavigation(force)
    },
    showSubmittedToast: (cooked) => {
      toast.success(tr('Feedback submitted'), {
        description: cooked ? tr('Cooked and uncooked feedback published') : tr('Feedback package published'),
      })
    },
  })
  const submitFeedback = publisherController.submitFeedback

  async function approveFeedback() {
    if (!workspace || !workspace.request.allow_finish || approving) return
    if (!window.confirm(tr('Approve this final summary and end Pi’s Ramble flow?'))) return
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
      toast.success(tr('Approved and finished'))
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
      toast.success(tr('Request cancelled'))
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
      await invoke('reveal_path_in_folder', {
        path: desktopPath(feedbackResult.markdown_path),
      })
    } catch (cause) {
      pageError = tr('Could not open Feedback Package: {error}', { error: messageFrom(cause) })
    }
  }

  async function exitRamble() {
    await rambleController?.exitRamble()
    await rambleDocumentQueue.catch(() => {})
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
    bind:attachmentBusy
    {screenCaptureBusy}
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
    interactionLocked={interactionLocked || currentRequestCooking || cookedDraftReady}
    onPageError={(message) => (pageError = message)}
    onSaveDraftNow={saveDraftNow}
    onApplyWorkspaceMutation={applyWorkspaceMutation}
    onRefreshAttachmentPreviews={attachmentController.refreshPreviews}
    onStartScreenCapture={attachmentController.startScreenCapture}
    onImportAttachmentPaths={attachmentController.importAttachmentPaths}
    onRouteDraftOperation={routeDraftOperation}
    getActiveAction={activeActionFor}
  />

  <AppTitlebar
    sourceLabel={workspace?.request.source_hint ?? workspace?.request.title ?? 'Workbench'}
    pendingCount={$navigation.pendingRequests.length}
    {rambleEngaged}
    {rambleActive}
    {rambleRequestTitle}
    notificationText={$notificationSoundEnabled
      ? tr('Notification settings · sound on')
      : notificationLabel(notificationState, $locale)}
    notificationEnabled={notificationState === 'enabled' || $notificationSoundEnabled}
    notificationDisabled={false}
    onNotifications={() => void openSettings('notifications')}
    onWindowError={(message) => (pageError = tr('Window action failed: {error}', { error: message }))}
  />

  <div bind:this={workbenchLayout} class="flex h-[calc(100%-46px)] min-h-0 min-w-0">
    <HostSessionRail
      bind:collapsed={hostSessionRailCollapsed}
      sessions={$navigation.hostSessions}
      activeHostId={$navigation.selectedHostId}
      activeHostSessionId={$navigation.selectedHostSessionId}
      requestSearch={$navigation.requestSearch}
      loading={$navigation.loadingNavigation}
      refreshing={$navigation.refreshingPage}
      {resolveHostProfile}
      onSelect={(hostId, hostSessionId) =>
        void selectRailScope(hostId, hostSessionId)}
      onRequestSearch={(search) => void navigation.setRequestSearch(search)}
      onRenameSession={(session, title) => navigation.renameHostSession(session, title)}
      onSetSessionPinned={(session, pinned) => navigation.setHostSessionPinned(session, pinned)}
      onArchiveSession={(session) => navigation.archiveHostSession(session)}
      onSetHostPinned={(hostId, pinned) => navigation.setHostPinned(hostId, pinned)}
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
          requests={visibleRequests}
          activeRequestId={workspace?.request.request_id ?? null}
          cookingRequestIds={cookingRequestIds}
          scopeLabel={requestScopeLabel}
          searchQuery={$navigation.requestSearch}
          loading={$navigation.loadingRequests}
          refreshing={$navigation.refreshingPage}
          loadingMore={$navigation.loadingMoreRequests}
          hasMore={todayOnly ? false : $navigation.nextRequestCursor !== null}
          {todayOnly}
          {resolveHostProfile}
          formatTime={formatTimeLocal}
          onRefresh={() => void navigation.refreshPage()}
          onLoadMore={() => void navigation.loadMoreRequests()}
          onOpenRequest={(requestId) => void openRequest(requestId)}
          onToggleToday={() => (todayOnly = !todayOnly)}
        />
      </Pane>

      <PaneResizer
        class="workbench-pane-resizer workbench-pane-resizer--vertical"
        aria-label={tr('Resize request list')}
      />

      <Pane id="workspace-pane" minSize={workspaceMinimumSize}>
        <div class="flex h-full min-h-0 min-w-0 flex-col">
          <WorkspaceTabStrip
            views={workspaceShellState.views}
            activeViewKey={workspaceShellState.activeViewKey}
            pendingViewKey={pendingWorkspaceViewKey}
            disabled={workspaceTransitionLocked}
            labelForView={workspaceTabLabel}
            onActivate={(viewKey) => void activateWorkspaceTab(viewKey)}
            onClose={closeWorkspaceTab}
          />
          <div
            class="min-h-0 flex-1"
            role={renderedWorkspaceView ? 'tabpanel' : undefined}
            id={renderedWorkspaceView
              ? workspaceTabPanelId(workspaceViewKey(renderedWorkspaceView))
              : undefined}
            aria-labelledby={renderedWorkspaceView
              ? workspaceTabId(workspaceViewKey(renderedWorkspaceView))
              : undefined}
          >
            {#if renderedWorkspaceView?.kind === 'settings'}
              <SettingsWorkspaceView
                mcpConfiguration={genericMcpConfiguration}
                section={settingsSection}
                {updateInstallBlocked}
                onRestartOnboarding={restartOnboarding}
                onOpenArchived={openArchivedSessions}
                onClose={() => {
                  void closeWorkspaceTab(workspaceViewKey(settingsViewDescriptor()))
                  void refreshNotificationPermission()
                }}
              />
            {:else if renderedSessionResolution?.kind === 'missing-session'}
              <MissingSessionView
                missing={renderedSessionResolution}
                label={sessionTabLabel(renderedSessionResolution.session)}
                busy={renderedSessionResolution.reason === 'unresolved' || pendingWorkspaceViewKey !== null}
                onRetry={retrySessionViewRecovery}
                onClose={() => closeWorkspaceTab(workspaceViewKey(renderedSessionResolution!.session))}
                onOpenArchive={() => openArchivedSessions(renderedSessionResolution!.session)}
              />
            {:else if workbenchMounted}
              {#key renderedSessionView ? workspaceViewKey(renderedSessionView) : 'workspace:empty'}
              <SessionWorkbench
            bind:this={sessionWorkbench}
            view={renderedSessionView}
            bind:taskBriefOpen
            {loadingWorkspace}
            {workspace}
            {feedbackResult}
            {draftBody}
            {editorDocument}
            {editorEpoch}
            tidyConfig={{
              provider: $tidyProvider,
              apiKey: $tidyApiKey,
              baseUrl: $tidyBaseUrl,
              model: $tidyModel,
              reasoningEffort: $tidyReasoningEffort,
              locale: $locale,
              systemPrompt: $tidySystemPrompt,
            }}
            tidyAutoThreshold={$tidyAutoThreshold}
            activeActionId={workspace
              ? activeActionByRequest.get(workspace.request.request_id)?.actionId ?? null
              : null}
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
            ramblePhase={rambleBelongsToWorkspace ? visibleRamblePhase : 'idle'}
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
            {cookedDraftReady}
            cookedPreviewModel={cookedPreview?.model ?? ''}
            cookedPreviewMarkdown={cookedPreview?.markdown ?? ''}
            onCookPreview={() => void cookPreviewOnly()}
            onRestoreOriginal={restoreOriginalAfterCook}
            {submitting}
            {submitStage}
            {publishedFeedback}
            {canCancel}
            {cancelling}
            {approving}
            {canOpenResumePrompt}
            {resolveHostProfile}
            formatTime={formatTimeLocal}
            onReload={() => void reloadWorkspace()}
            onDraftChange={updateDraft}
            onTidyError={(message) => (pageError = message)}
            onOpenTidySettings={() => void openSettings('post-processing')}
            onSelectAction={selectAction}
            onToggleRamble={() => void toggleRamble()}
            onExitRamble={() => void exitRamble()}
            onOpenVoiceSettings={() => void openSettings('voice')}
            onStartScreenCapture={() => void attachmentController.startScreenCapture()}
            onImportClipboard={() => void importClipboardNow()}
            onFileSelection={attachmentController.handleFileSelection}
            onRemoveAttachment={(attachment) => void attachmentController.removeAttachment(attachment)}
            onOpenPackage={() => void openFeedbackPackage()}
            onOpenResumePrompt={openResumePrompt}
            onSubmit={() => void submitFeedback()}
            onCancel={() => void cancelFeedback()}
            onApprove={() => void approveFeedback()}
              />
              {/key}
            {:else}
              <div
                class="grid h-full min-h-0 place-items-center text-sm text-muted-foreground"
                aria-busy="true"
                aria-live="polite"
              >
                {tr('Loading workspace…')}
              </div>
            {/if}
          </div>
        </div>
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

<ArchivedSessionsDialog
  bind:open={archivedSessionsOpen}
  {isTauri}
  {previewMode}
  {resolveHostProfile}
  formatTime={formatTimeLocal}
  {messageFrom}
  initialSession={archivedInitialSession}
  onError={(message) => (pageError = message)}
  onChanged={retrySessionViewRecovery}
/>

<UpdateAvailableDialog
  installBlocked={updateInstallBlocked}
  onOpenReleases={() => void openGithubReleases()}
/>
{/key}
