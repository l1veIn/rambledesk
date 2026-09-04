<script lang="ts">
  import { onMount, tick } from 'svelte'

  import rambelleArchived from './assets/rambelle-states/archived.webp'
  import rambelleIdle from './assets/rambelle-states/idle.webp'
  import rambelleOrganizing from './assets/rambelle-states/organizing.webp'
  import rambelleRecording from './assets/rambelle-states/recording.webp'
  import AppTitlebar from './lib/AppTitlebar.svelte'
  import OnboardingWizard from './lib/OnboardingWizard.svelte'
  import UpdateAvailableDialog from './lib/UpdateAvailableDialog.svelte'
  import HostSessionRail from './lib/components/navigation/HostSessionRail.svelte'
  import RequestListPane from './lib/components/navigation/RequestListPane.svelte'
  import { Sonner, toast } from './lib/components/ui/sonner'
  import ResumePromptDialog from './lib/workbench/ResumePromptDialog.svelte'
  import SessionWorkbench from './lib/workbench/SessionWorkbench.svelte'
  import InboxWorkspaceView from './lib/workspace/InboxWorkspaceView.svelte'
  import MissingSessionView from './lib/workspace/MissingSessionView.svelte'
  import RambelleProfileWorkspaceView from './lib/workspace/RambelleProfileWorkspaceView.svelte'
  import SettingsWorkspaceView from './lib/workspace/SettingsWorkspaceView.svelte'
  import TaskWorkspaceView from './lib/workspace/TaskWorkspaceView.svelte'
  import ArchivedSessionsWorkspaceView from './lib/workspace/ArchivedSessionsWorkspaceView.svelte'
  import ManagedSessionSection from './lib/agents/ManagedSessionSection.svelte'
  import NewManagedSessionSection from './lib/agents/NewManagedSessionSection.svelte'
  import { agentText } from './lib/agents/agentI18n'
  import { deleteSessionRecord, removeManagedSessionViews } from './lib/agents/managedSessionDeletion'
  import { Button } from './lib/components/ui/button'
  import * as Dialog from './lib/components/ui/dialog'
  import type { JSONContent } from '@tiptap/core'
  import {
    defineApplicationStream,
    type ApplicationTransport,
  } from './lib/application/applicationTransport'
  import type { WorkbenchCapabilities } from './lib/capabilities/workbenchCapabilities'
  import { provideWorkbenchCapabilities } from './lib/capabilities/capabilityContext'
  import { createUnavailableWorkbenchCapabilities } from './lib/capabilities/unavailableCapabilities'
  import type { PublishedFeedbackAction } from './lib/publishedFeedbackAction'
  import { APPLICATION_EVENTS_STREAM } from './lib/application/applicationEvents'
  import {
    applicationResourcesAffectNavigation,
    applicationResourcesAffectWorkspace,
    applicationResourcesRequireFullNavigationSnapshot,
    createApplicationSnapshotRefetch,
    type ApplicationSnapshotRefetchIntent,
  } from './lib/application/applicationSnapshotRefetch'

  export let applicationTransport: ApplicationTransport
  export let capabilities: WorkbenchCapabilities = createUnavailableWorkbenchCapabilities()
  export let publishedFeedbackAction: PublishedFeedbackAction
  export let previewMode = false

  provideWorkbenchCapabilities(capabilities)

  import type {
    ApproveFeedbackInput,
    CancelFeedbackInput,
    DraftView,
    FeedbackRequestView,
    FeedbackRequestSummary,
    FeedbackWorkspaceView,
    SubmitFeedbackInput,
  } from './lib/feedback'
  import type { HostSessionSummary, ManagedSessionSnapshot } from './lib/generated/feedback'
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
    notificationStateForPermission,
    type NotificationState,
  } from './lib/notifications'
  import {
    archiveViewDescriptor,
    inboxViewDescriptor,
    rambelleProfileViewDescriptor,
    requestTaskViewDescriptor,
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
  import { updateTaskTabTitles } from './lib/workspace/taskTabTitles'
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
  import {
    workspaceTabId,
    workspaceTabPanelId,
  } from './lib/workspace/workspaceTabNavigation'
  import WorkspaceTabStrip from './lib/workspace/WorkspaceTabStrip.svelte'
  import { workspaceSurface } from './lib/workspace/workspaceSurface'
  import {
    createWorkspaceTransition,
    type WorkspaceTransitionOutcome,
    type WorkspaceTransitionTarget,
  } from './lib/workspace/workspaceTransition'
  import { leavesSettingsView } from './lib/workspace/workspaceViewLifecycle'
  import {
    shouldAdoptTaskBackgroundDraft,
    shouldUseForegroundDraftEditor,
  } from './lib/workspace/draftOperationRouting'
  import { previewFixtures, previewWorkspaceFor } from './lib/previewFixtures'
  import {
    restorePublishedAttachmentUrls,
    normalizePublishedFeedback,
    type PublishedAttachmentPath,
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
  import { buildResumePrompt, requestSessionManagement, shouldShowResumePromptButton } from './lib/workbench/resumePrompt'
  import {
    createAttachmentController,
    type AttachmentMessageTone,
  } from './lib/workbench/attachmentController'
  import { createNavigationController } from './lib/workbench/navigationController'
  import { requestFilterCount } from './lib/workbench/requestFilters'
  import { ensureDesktopNavigationPolling } from './lib/workbench/navigationPolling'
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
  import { highlightSpeechSegment } from './lib/highlightSpeechSegment'
  import { t } from './lib/i18n'
  import {
    initialHostRailCollapsed,
    initialRequestRailCollapsed,
    saveHostRailCollapsed,
    saveRequestRailCollapsed,
    saveWorkspaceSnapshot,
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
    setNotificationPopupEnabled,
    tidyApiKey,
    tidyAutoThreshold,
    tidyBaseUrl,
    tidyModel,
    tidyProvider,
    tidyReasoningEffort,
    tidySystemPrompt,
  } from './lib/preferences'

  const RESUME_PROMPT_STREAM = defineApplicationStream<ResumePrompt>('rambledesk://resume-prompt')
  const OPEN_ADAPTERS_STREAM = defineApplicationStream<void>('rambledesk://open-adapters')
  const formatTimeLocal = (value: string | null | undefined) =>
    formatTime(value, $locale, tr('Not saved yet'))
  let workspace: FeedbackWorkspaceView | null = null
  let workspaceShellState: WorkspaceShellState = EMPTY_WORKSPACE_SHELL_STATE
  let taskTabTitles: ReadonlyMap<string, string> = new Map()
  let renderedWorkspaceView: WorkspaceViewDescriptor | null = null
  let renderedSessionView: SessionViewDescriptor | null = null
  let renderedSessionResolution: SessionViewResolution | null = null
  let sessionViewResolutions: readonly SessionViewResolution[] = []
  let sessionRequestIds = new Map<string, string>()
  let managedSessionPanels: ReadonlyMap<string, 'agent' | 'feedback'> = new Map()
  let newManagedSessionOpen = false
  let creatingManagedSession = false
  let deletingSessionCommands = new Set<string>()
  let deletingManagedSessionIds = new Set<string>()
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
  let archivedInitialSession: SessionViewDescriptor | null = null
  let archivedSelectionEpoch = 0
  let lastSessionRecoveryFingerprint = ''
  let activeRecoveryTransition: object | null = null
  let settingsSection: SettingsSection = 'general'
  let settingsSectionSelectionEpoch = 0
  let lastAutoOpenedTaskRequestId = ''
  let onboardingOpen = false
  let launchUpdateCheckDue = false
  let workbenchInitialized = false
  const desktopShellAvailable = capabilities.windowControls.status.source === 'native'
  const isMac = capabilities.windowControls.implementation.platform() === 'macOS'
  const notificationsAvailable = capabilities.notifications.status.availability !== 'unavailable'
  const softwareUpdatesAvailable = capabilities.softwareUpdates.status.availability !== 'unavailable'
  const onboardingAvailable =
    capabilities.dataStorageAdministration.status.availability !== 'unavailable' ||
    capabilities.speech.status.availability !== 'unavailable' ||
    capabilities.hostIntegrationAdministration.status.availability !== 'unavailable' ||
    notificationsAvailable ||
    capabilities.webAccessAdministration.status.availability !== 'unavailable'
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
  let taskBriefOpen = true
  let requestRailCollapsed = initialRequestRailCollapsed()
  let hostSessionRailCollapsed = initialHostRailCollapsed()
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
    transport: applicationTransport,
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
    capabilities,
    transport: applicationTransport,
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
    recordAttachmentDiagnostic: async (activity, requestId) => {
      if (capabilities.rambleConsole.status.availability === 'unavailable') return
      await capabilities.rambleConsole.implementation
        .recordDiagnostic(activity, requestId)
        .catch(() => {})
    },
  })

  type LoadedWorkspaceTarget =
    | Readonly<{
        kind: 'session'
        workspace: FeedbackWorkspaceView
        publishedFeedback: PublishedFeedbackView | null
      }>
    | Readonly<{
        kind: 'request-task'
        workspace: FeedbackWorkspaceView
      }>

  const workspaceTransition = createWorkspaceTransition<LoadedWorkspaceTarget>({
    saveCurrent: saveDraftNow,
    unmountCurrent: () => {
      workbenchMounted = false
      sessionWorkbench = undefined
      loadingWorkspace = true
    },
    loadTarget: loadWorkspaceTarget,
    commitTarget: commitWorkspaceTarget,
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
    capabilities,
    previewMode,
    transport: applicationTransport,
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
        previewMode
          ? previewWorkspaceScenario === 'unknown'
            ? await Promise.reject(new Error('Preview archived catalog unavailable'))
            : previewFixtures.archivedHostSessions
          : await applicationTransport.call('listArchivedHostSessions', { search: null })
      return sessions.map((session) =>
        sessionViewDescriptor(session.host_id, session.host_session_id),
      )
    },
    onInvalidate: () => {
      if (!activeRecoveryTransition) return
      activeRecoveryTransition = null
      workspaceTransition.invalidate()
    },
    onUpdate: applySessionViewResolutions,
  })
  const applicationSnapshotRefetch = createApplicationSnapshotRefetch({
    refetch: refetchApplicationSnapshots,
    reportError: (cause) => {
      pageError = messageFrom(cause)
    },
  })

  export function refetchAfterTransportReady() {
    applicationSnapshotRefetch.request([{ kind: 'all' }])
  }

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
  $: selectedHostSession = $navigation.selectedHostSessionId
    ? $navigation.hostSessions.find(
        (session) =>
          session.host_id === $navigation.selectedHostId &&
          session.host_session_id === $navigation.selectedHostSessionId,
      )
    : undefined
  $: renderedWorkspaceView = activeWorkspaceView(workspaceShellState)
  $: renderedWorkspaceSurface = workspaceSurface(renderedWorkspaceView)
  $: renderedSessionView = renderedWorkspaceView?.kind === 'session'
    ? renderedWorkspaceView
    : null
  $: renderedSessionResolution = sessionViewResolution(
    sessionViewResolutions,
    workspaceShellState.activeViewKey,
  )
  $: renderedManagedSession = renderedSessionView
    ? $navigation.hostSessions.find((session) => session.management.kind === 'managed'
      && session.host_id === renderedSessionView!.hostId
      && session.host_session_id === renderedSessionView!.hostSessionId)
    : undefined
  $: managedFeedbackRequestId = renderedManagedSession && workspace
    && workspace.request.host_id === renderedManagedSession.host_id
    && workspace.request.host_session_id === renderedManagedSession.host_session_id
      ? workspace.request.request_id : null
  $: renderedManagedFeedbackRequests = renderedManagedSession
    ? $navigation.requests.filter((request) => request.host_id === renderedManagedSession!.host_id
      && request.host_session_id === renderedManagedSession!.host_session_id)
    : []
  $: showManagedAgent = renderedManagedSession !== undefined
    && (managedSessionPanels.get(renderedManagedSession.session_id) !== 'feedback' || !managedFeedbackRequestId)
  const sessionTabLabel = (view: SessionViewDescriptor) => {
    const session = $navigation.hostSessions.find(
      (candidate) =>
        candidate.host_id === view.hostId &&
        candidate.host_session_id === view.hostSessionId,
    )
    const hostLabel = resolveHostProfile(view.hostId).label
    return `${session?.title ?? view.hostSessionId} · ${hostLabel}`
  }
  $: taskTabTitles = updateTaskTabTitles(taskTabTitles, workspaceShellState.views, [
    ...(workspace ? [workspace.request] : []),
    ...$navigation.pendingRequests,
    ...$navigation.requests,
  ])
  $: workspaceTabLabel = (view: WorkspaceViewDescriptor) => {
    switch (view.kind) {
      case 'inbox':
        return tr('All requests')
      case 'archive':
        return tr('Archived sessions')
      case 'settings':
        return tr('Settings')
      case 'request-task':
        return taskTabTitles.get(view.requestId) ?? tr('Task brief')
      case 'rambelle-profile':
        return 'Rambelle'
      case 'session':
        return sessionTabLabel(view)
    }
  }
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
    requestSessionManagement(workspace?.request, $navigation.hostSessions),
  )
  $: feedbackManagedSession = workspace ? $navigation.hostSessions.find((session) => session.management.kind === 'managed'
    && session.host_id === workspace!.request.host_id && session.host_session_id === workspace!.request.host_session_id) : undefined
  $: managedFeedbackReadOnly = !!feedbackManagedSession && (deletingSessionCommands.has(feedbackManagedSession.session_id) || deletingManagedSessionIds.has(feedbackManagedSession.session_id))
  $: currentRequestCooking =
    workspace !== null && cookingRequestIds.has(workspace.request.request_id)
  $: cookedDraftReady = cookedPreview !== null
  // Turning Cooking off discards any pending cooked preview: submitting then
  // publishes the editor content as-is.
  $: if (!$cookingEnabled) cookedPreview = null
  $: canSubmit =
    !managedFeedbackReadOnly &&
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
  $: saveHostRailCollapsed(hostSessionRailCollapsed)
  $: saveRequestRailCollapsed(requestRailCollapsed)

  onMount(() => {
    const cleanupAttachments = attachmentController.mount()
    const unsubscribeApplicationEvents = !desktopShellAvailable && !previewMode
      ? applicationTransport.subscribe(
          APPLICATION_EVENTS_STREAM,
          (event) => {
            if (event.type === 'invalidate') {
              applicationSnapshotRefetch.request(event.resources)
            }
          },
          (cause) => {
            pageError = messageFrom(cause)
          },
        )
      : () => {}

    if (!desktopShellAvailable) {
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
      if (
        capabilities.softwareUpdates.status.availability !== 'unavailable' &&
        new URLSearchParams(window.location.search).get('dialog') === 'update'
      ) {
        void capabilities.softwareUpdates.implementation.check({ prompt: true, forcePrompt: true })
      }
      return () => {
        unsubscribeApplicationEvents()
        applicationSnapshotRefetch.dispose()
        cleanupAttachments()
      }
    }
    if ($onboardingCompleted || !onboardingAvailable) startWorkbench()
    else onboardingOpen = true
    const updateCheckTimer = softwareUpdatesAvailable
      ? window.setTimeout(() => {
          launchUpdateCheckDue = true
          if (!onboardingOpen) {
            void capabilities.softwareUpdates.implementation.check({ prompt: true, forcePrompt: false })
          }
        }, 4_000)
      : undefined
    if (notificationsAvailable) void refreshNotificationPermission()
    else notificationState = 'unavailable'
    const openAdaptersUnlisten = applicationTransport.subscribe(
      OPEN_ADAPTERS_STREAM,
      () => void openSettings('adapters'),
      () => {
        // The tray entry is an optional Desktop Shell affordance.
      },
    )
    let resumePromptMounted = true
    let resumePromptGeneration = 0
    const resumePromptUnlisten = applicationTransport.subscribe(
      RESUME_PROMPT_STREAM,
      (prompt) => {
        const generation = ++resumePromptGeneration
        void presentExternalResumePrompt(prompt, () => resumePromptMounted && generation === resumePromptGeneration)
      },
      () => {
        // The manual reopen action remains available for external sessions.
      },
    )
    async function presentExternalResumePrompt(prompt: ResumePrompt, isCurrent: () => boolean) {
      try {
        const knownRequest = workspace?.request.request_id === prompt.request_id ? workspace.request
          : [...$navigation.requests, ...$navigation.pendingRequests].find((request) => request.request_id === prompt.request_id)
        const request = knownRequest ?? (await applicationTransport.call('getFeedbackWorkspace', { request_id: prompt.request_id })).request
        if (!isCurrent()) return
        let management = requestSessionManagement(request, $navigation.hostSessions)
        if (!management) management = requestSessionManagement(request, await applicationTransport.call('listHostSessions', undefined))
        if (!isCurrent()) return
        if (!management) management = requestSessionManagement(request, await applicationTransport.call('listArchivedHostSessions', { search: null }))
        if (!isCurrent() || management?.kind === 'managed') return
        resumePrompt = prompt
        resumeCopyState = 'idle'
        if (
          notificationsAvailable &&
          isMac &&
          $notificationPopupEnabled &&
          notificationState === 'enabled'
        ) {
          void capabilities.notifications.implementation
            .send({
              title: prompt.title,
              body: tr(
                'Return to {host} and use the resume prompt to continue the host session.',
                { host: prompt.host_label },
              ),
            })
            .catch(() => {})
        }
        // The alert sound is reserved for a new request arriving, not for the
        // resume prompt shown after a submission completes, so it is not played
        // here.
      } catch (cause) {
        if (isCurrent()) pageError = messageFrom(cause)
      }
    }
    return () => {
      resumePromptMounted = false
      resumePromptGeneration += 1
      unsubscribeApplicationEvents()
      applicationSnapshotRefetch.dispose()
      draftController.cancelPendingSave()
      if (inboxTimer) clearInterval(inboxTimer)
      resumePromptUnlisten()
      openAdaptersUnlisten()
      if (updateCheckTimer !== undefined) clearTimeout(updateCheckTimer)
      cleanupAttachments()
    }
  })

  function startWorkbench() {
    if (workbenchInitialized) return
    workbenchInitialized = true
    inboxTimer = ensureDesktopNavigationPolling(
      desktopShellAvailable,
      inboxTimer,
      setInterval,
      () => void navigation.refreshNavigation(true),
    )
    void (async () => {
      const initialized = await navigation.initialize(initialWorkspaceSnapshot === null)
      if (!initialized) return
      await refreshSessionViewRecovery()
      if (initialWorkspaceSnapshot) await restoreInitialWorkspaceSnapshot()
      else if (previewMode && workspace) {
        await navigation.selectScope(workspace.request.host_id, workspace.request.host_session_id)
      }
    })()
  }

  function closeOnboarding() {
    onboardingOpen = false
    startWorkbench()
    if (softwareUpdatesAvailable && launchUpdateCheckDue) {
      void capabilities.softwareUpdates.implementation.check({ prompt: true, forcePrompt: false })
    }
  }

  async function openGithubReleases() {
    const releasesUrl = 'https://github.com/l1veIn/rambledesk/releases'
    try {
      await capabilities.externalLinks.implementation.open(releasesUrl)
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

  function reorderWorkspaceTabs(viewKeys: readonly string[]) {
    const nextState = workspaceShellReducer(workspaceShellState, {
      type: 'reorder',
      viewKeys,
    })
    if (nextState === workspaceShellState) return
    workspaceShellState = nextState
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
      const outcome = await workspaceTransition.activate({
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

  async function refetchApplicationSnapshots(
    intent: ApplicationSnapshotRefetchIntent,
  ): Promise<void> {
    if (applicationResourcesAffectNavigation(intent.resources)) {
      if (applicationResourcesRequireFullNavigationSnapshot(intent.resources)) {
        await navigation.initialize(false)
      } else {
        await navigation.refreshNavigation(true)
      }
      if (!intent.isCurrent()) return
      await refreshSessionViewRecovery()
      if (!intent.isCurrent()) return
    }

    while (workspaceTransitionLocked && intent.isCurrent()) {
      await new Promise((resolve) => window.setTimeout(resolve, 50))
    }
    if (!intent.isCurrent()) return

    const activeView = activeWorkspaceView(workspaceShellState)
    const activeWorkspace = workspace
    if (
      !activeView ||
      !activeWorkspace ||
      (activeView.kind !== 'session' && activeView.kind !== 'request-task') ||
      !applicationResourcesAffectWorkspace(intent.resources, {
        requestId: activeWorkspace.request.request_id,
        hostId: activeWorkspace.request.host_id,
        hostSessionId: activeWorkspace.request.host_session_id,
      })
    ) {
      return
    }

    const outcome = await workspaceTransition.activate({
      view: activeView,
      requestId: activeWorkspace.request.request_id,
      shellAction: { type: 'open' },
      pendingViewKey: workspaceViewKey(activeView),
    })
    if (!intent.isCurrent() || outcome === 'stale') return
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
    if (
      view.kind === 'inbox' ||
      view.kind === 'archive' ||
      view.kind === 'settings' ||
      view.kind === 'rambelle-profile'
    ) {
      clearWorkspace()
      workbenchMounted = true
      loadingWorkspace = false
      if (view.kind === 'inbox') await navigation.selectScope(null, null)
      else if (view.kind === 'settings') void refreshGenericMcpConfiguration()
      return
    }
    if (view.kind === 'request-task') {
      await workspaceTransition.activate({
        view,
        requestId: view.requestId,
        shellAction: { type: 'open' },
        pendingViewKey: workspaceViewKey(view),
      })
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
    await workspaceTransition.activate({
      view,
      requestId: requestIdForSession(view, selection.requests),
      shellAction: { type: 'open' },
      pendingViewKey: workspaceViewKey(view),
    })
  }

  type NavigationScope = Readonly<{
    hostId: string | null
    hostSessionId: string | null
  }>

  function requestIdForSession(
    view: SessionViewDescriptor,
    requests: readonly FeedbackRequestSummary[],
  ): string | null {
    const rememberedRequestId = sessionRequestIds.get(workspaceViewKey(view))
    // List filters may hide an open request; they must not reset its workspace tab.
    if (rememberedRequestId && (
      requestFilterCount($navigation.requestFilters) > 0 ||
      requests.some((request) => request.request_id === rememberedRequestId)
    )) return rememberedRequestId
    return requests[0]?.request_id ?? null
  }

  function currentNavigationScope(): NavigationScope {
    return {
      hostId: $navigation.selectedHostId,
      hostSessionId: $navigation.selectedHostSessionId,
    }
  }

  async function restoreNavigationScope(
    scope: NavigationScope,
    outcome: WorkspaceTransitionOutcome,
  ) {
    if (outcome !== 'blocked' && outcome !== 'failed') return
    await navigation.selectScope(scope.hostId, scope.hostSessionId)
  }

  async function selectRailScope(hostId: string | null, hostSessionId: string | null) {
    if (workspaceTransitionLocked) return
    const priorScope = currentNavigationScope()
    workspaceTransition.invalidate()
    const selection = await navigation.selectScope(hostId, hostSessionId)
    if (!selection.selected) return
    if (!hostId || !hostSessionId) {
      const outcome = await workspaceTransition.activate({
        view: inboxViewDescriptor(),
        requestId: null,
        shellAction: { type: 'open' },
        pendingViewKey: workspaceViewKey(inboxViewDescriptor()),
      })
      await restoreNavigationScope(priorScope, outcome)
      return
    }
    if (workspaceTransitionLocked) {
      await navigation.selectScope(priorScope.hostId, priorScope.hostSessionId)
      return
    }

    const view = sessionViewDescriptor(hostId, hostSessionId)
    const viewKey = workspaceViewKey(view)
    const requestId = requestIdForSession(view, selection.requests)
    if (requestId) {
      const outcome = await activateRequest(requestId)
      await restoreNavigationScope(priorScope, outcome)
      return
    }
    const outcome = await workspaceTransition.activate({
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
    workspaceTransition.invalidate()
    const view = workspaceShellState.views.find(
      (candidate) => workspaceViewKey(candidate) === viewKey,
    )
    if (!view) return
    if (view.kind !== 'session') {
      if (view.kind === 'inbox') {
        const selection = await navigation.selectScope(null, null)
        if (!selection.selected) return
      }
      const outcome = await workspaceTransition.activate({
        view,
        requestId: view.kind === 'request-task' ? view.requestId : null,
        shellAction: { type: 'open' },
        pendingViewKey: viewKey,
      })
      if (outcome === 'activated' && view.kind === 'settings') {
        void refreshGenericMcpConfiguration()
      }
      if (view.kind === 'inbox') await restoreNavigationScope(priorScope, outcome)
      return
    }
    const resolution = sessionViewResolution(sessionViewResolutions, viewKey)
    if (resolution?.kind === 'missing-session') {
      const outcome = await workspaceTransition.activate({
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
    const requestId = requestIdForSession(view, selection.requests)
    if (requestId) {
      const outcome = await activateRequest(requestId)
      await restoreNavigationScope(priorScope, outcome)
      return
    }
    const outcome = await workspaceTransition.activate({
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
    workspaceTransition.invalidate()
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
      fallbackRequestId = requestIdForSession(fallbackView, selection.requests)
    } else if (fallbackView?.kind === 'inbox') {
      const selection = await navigation.selectScope(null, null)
      if (!selection.selected) return
    } else if (fallbackView?.kind === 'request-task') {
      fallbackRequestId = fallbackView.requestId
    }

    const outcome = await workspaceTransition.activate({
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
      const granted = await capabilities.notifications.implementation.permission() === 'granted'
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

  async function loadWorkspaceTarget(
    target: WorkspaceTransitionTarget,
  ): Promise<LoadedWorkspaceTarget | null> {
    if (!target.requestId) return null
    const requestId = target.requestId
    return enqueueDocumentTask(async () => {
      const next = previewMode
        ? previewWorkspaceFor(requestId)
        : await applicationTransport.call('getFeedbackWorkspace', {
            request_id: requestId,
          })
      if (!next) throw new Error(tr('This feedback request could not be found.'))

      if (target.view?.kind === 'request-task') {
        if (target.view.requestId !== next.request.request_id) {
          throw new Error(tr('This feedback request could not be found.'))
        }
        return { kind: 'request-task', workspace: next }
      }

      const loadedView = sessionViewDescriptor(next.request.host_id, next.request.host_session_id)
      if (
        target.view?.kind === 'session' &&
        workspaceViewKey(target.view) !== workspaceViewKey(loadedView)
      ) {
        throw new Error(tr('The feedback request no longer belongs to the selected session.'))
      }
      if (target.view && target.view.kind !== 'session') {
        throw new Error(tr('This feedback request could not be found.'))
      }

      const nextPublishedFeedback =
        next.request.status === 'completed' && next.feedback
          ? previewMode
            ? {
                markdown: next.draft.body_markdown,
                uncooked_markdown: next.draft.body_markdown,
              }
            : normalizePublishedFeedback(
                await applicationTransport.call('readPublishedFeedback', {
                  request_id: next.request.request_id,
                }),
              )
          : null
      return { kind: 'session', workspace: next, publishedFeedback: nextPublishedFeedback }
    })
  }

  function commitWorkspaceTarget(
    target: WorkspaceTransitionTarget,
    loaded: LoadedWorkspaceTarget | null,
  ) {
    const previousActiveView = activeWorkspaceView(workspaceShellState)
    const loadedView = loaded?.kind === 'session'
      ? sessionViewDescriptor(
          loaded.workspace.request.host_id,
          loaded.workspace.request.host_session_id,
        )
      : loaded?.kind === 'request-task'
        ? requestTaskViewDescriptor(loaded.workspace.request.request_id)
        : target.view
    if (target.shellAction.type === 'open' && !loadedView) {
      throw new Error(tr('This feedback request could not be found.'))
    }

    const nextShellState =
      target.shellAction.type === 'close'
        ? workspaceShellReducer(workspaceShellState, target.shellAction)
        : workspaceShellReducer(workspaceShellState, { type: 'open', view: loadedView! })

    if (loaded?.kind === 'session') {
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
    } else if (loaded?.kind === 'request-task') {
      attachmentController.releasePreviews()
      workspace = loaded.workspace
      completedResult = null
      publishedFeedback = null
      cookedPreview = null
      adoptDraft(loaded.workspace.draft)
      savePhase = loaded.workspace.draft.updated_at ? 'saved' : 'idle'
      saveMessage = ''
      attachmentMessage = ''
      if (target.shellAction.type === 'close') {
        const nextRequestIds = new Map(sessionRequestIds)
        nextRequestIds.delete(target.shellAction.viewKey)
        sessionRequestIds = nextRequestIds
      }
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
    if (leavesSettingsView(previousActiveView, activeWorkspaceView(nextShellState))) {
      void refreshNotificationPermission()
    }
    if (loaded) void attachmentController.refreshPreviews(loaded.workspace)
  }

  async function activateRequest(
    requestId: string,
  ): Promise<WorkspaceTransitionOutcome> {
    if (workspaceTransitionLocked) return 'blocked'
    const priorScope = currentNavigationScope()
    workspaceTransition.invalidate()
    if (
      workspace?.request.request_id === requestId &&
      renderedWorkspaceView?.kind === 'session'
    ) {
      openLoadedWorkspaceView(workspace)
      return 'activated'
    }
    pageError = ''
    const view = viewForRequest(requestId)
    if (view) {
      const selection = await navigation.selectScope(view.hostId, view.hostSessionId)
      if (!selection.selected) return 'failed'
    }
    const outcome = await workspaceTransition.activate({
      view,
      requestId,
      shellAction: { type: 'open' },
      pendingViewKey: view ? workspaceViewKey(view) : `request:${JSON.stringify(requestId)}`,
    })
    await restoreNavigationScope(priorScope, outcome)
    return outcome
  }

  async function openRequest(requestId: string, _saveCurrent = true): Promise<boolean> {
    const opened = (await activateRequest(requestId)) === 'activated'
    if (opened && workspace?.request.request_id === requestId) {
      const session = $navigation.hostSessions.find((candidate) => candidate.management.kind === 'managed'
        && candidate.host_id === workspace!.request.host_id
        && candidate.host_session_id === workspace!.request.host_session_id)
      if (session) managedSessionPanels = new Map(managedSessionPanels).set(session.session_id, 'feedback')
    }
    return opened
  }

  async function showManagedAgentPanel() {
    if (!renderedManagedSession || workspaceTransitionLocked) return
    const sessionId = renderedManagedSession.session_id
    if (!(await saveDraftNow()) || renderedManagedSession?.session_id !== sessionId) return
    managedSessionPanels = new Map(managedSessionPanels).set(sessionId, 'agent')
  }

  async function openNewManagedSession() {
    if (workspaceTransitionLocked || previewMode || !(await saveDraftNow())) return
    newManagedSessionOpen = true
  }

  async function managedSessionCreated(snapshot: ManagedSessionSnapshot) {
    newManagedSessionOpen = false
    managedSessionPanels = new Map(managedSessionPanels).set(snapshot.session.session_id, 'agent')
    await navigation.refreshNavigation(true)
    await selectRailScope(snapshot.session.host_id, snapshot.session.host_session_id)
  }

  function observeManagedDeletion(sessionId: string, deleting: boolean) {
    const next = new Set(deletingManagedSessionIds)
    if (deleting) next.add(sessionId)
    else next.delete(sessionId)
    deletingManagedSessionIds = next
  }

  async function deleteManagedSessionFromUi(session: HostSessionSummary) {
    if (session.management.kind !== 'managed' || deletingSessionCommands.has(session.session_id)) return
    deletingSessionCommands = new Set([...deletingSessionCommands, session.session_id])
    try {
      const ownsFeedback = () => workspace?.request.host_id === session.host_id && workspace?.request.host_session_id === session.host_session_id
      if (ownsFeedback() && rambleBelongsToWorkspace && rambleCanExit) await exitRamble()
      await deleteSessionRecord(applicationTransport, session)
      const viewKey = workspaceViewKey(sessionViewDescriptor(session.host_id, session.host_session_id))
      const requestId = ownsFeedback() ? workspace!.request.request_id : null
      const rememberedRequestId = sessionRequestIds.get(viewKey)
      const cleanup = removeManagedSessionViews(workspaceShellState, session, [requestId, rememberedRequestId].filter((id): id is string => !!id))
      const closedActive = cleanup.closedActive || ownsFeedback()
      if (pendingWorkspaceViewKey && cleanup.closedViewKeys.includes(pendingWorkspaceViewKey)) workspaceTransition.invalidate()
      if (closedActive) {
        workspaceTransition.invalidate()
        draftController.cancelPendingSave()
        clearWorkspace()
        cookedPreview = null
      }
      workspaceShellState = cleanup.shell
      if (closedActive) workspaceShellState = workspaceShellReducer(workspaceShellState, { type: 'open', view: inboxViewDescriptor() })
      const requestIds = new Map(sessionRequestIds)
      requestIds.delete(viewKey)
      sessionRequestIds = requestIds
      const panels = new Map(managedSessionPanels)
      panels.delete(session.session_id)
      managedSessionPanels = panels
      observeManagedDeletion(session.session_id, false)
      persistCurrentWorkspaceSnapshot()
      if (closedActive || ($navigation.selectedHostId === session.host_id && $navigation.selectedHostSessionId === session.host_session_id)) await navigation.selectScope(null, null)
      await navigation.refreshNavigation(true)
    } catch (cause) {
      pageError = messageFrom(cause)
      throw cause
    } finally {
      const pending = new Set(deletingSessionCommands)
      pending.delete(session.session_id)
      deletingSessionCommands = pending
    }
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
      const foregroundWorkspace = workspace
      if (shouldUseForegroundDraftEditor({
        activeView: activeWorkspaceView(workspaceShellState),
        workbenchMounted,
        editorReady: sessionWorkbench !== undefined,
        workspaceRequestId: foregroundWorkspace?.request.request_id ?? null,
        requestId,
      }) && foregroundWorkspace) {
        if (
          foregroundWorkspace.request.status === 'completed' ||
          foregroundWorkspace.request.status === 'cancelled'
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

      const savedDraft = await writeBackgroundDraftOperation(requestId, operation, {
        load: async () => {
          const target = previewMode
            ? previewWorkspaceFor(requestId)
            : await applicationTransport.call('getFeedbackWorkspace', {
                request_id: requestId,
              })
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
            : applicationTransport.call('saveFeedbackDraft', input),
      })
      if (
        shouldAdoptTaskBackgroundDraft(
          activeWorkspaceView(workspaceShellState),
          workspace?.request.request_id ?? null,
          requestId,
        ) &&
        workspace
      ) {
        workspace = { ...workspace, draft: savedDraft }
        adoptDraft(savedDraft)
        savePhase = savedDraft.updated_at ? 'saved' : 'idle'
        saveMessage = ''
      }
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
    if (
      !requestId ||
      workspaceTransitionLocked ||
      pendingWorkspaceViewKey !== null ||
      workspace?.request.status === 'completed' ||
      workspace?.request.status === 'cancelled'
    ) return
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

  async function refreshGenericMcpConfiguration() {
    pageError = ''
    if (capabilities.hostIntegrationAdministration.status.availability === 'unavailable') return
    try {
      genericMcpConfiguration = await capabilities.hostIntegrationAdministration.implementation
        .genericMcpConfiguration()
    } catch (cause) {
      pageError = messageFrom(cause)
    }
  }

  async function openSettings(section: SettingsSection) {
    settingsSection = section
    settingsSectionSelectionEpoch += 1
    const view = settingsViewDescriptor()
    const viewKey = workspaceViewKey(view)
    if (workspaceTransitionLocked || pendingWorkspaceViewKey) return
    if (workspaceShellState.activeViewKey !== viewKey) {
      workspaceTransition.invalidate()
      const outcome = await workspaceTransition.activate({
        view,
        requestId: null,
        shellAction: { type: 'open' },
        pendingViewKey: viewKey,
      })
      if (outcome !== 'activated') return
    }
    await refreshGenericMcpConfiguration()
  }

  async function openTaskWorkspace(requestId: string) {
    if (workspaceTransitionLocked || pendingWorkspaceViewKey) return
    const view = requestTaskViewDescriptor(requestId)
    workspaceTransition.invalidate()
    await workspaceTransition.activate({
      view,
      requestId,
      shellAction: { type: 'open' },
      pendingViewKey: workspaceViewKey(view),
    })
  }

  function autoOpenTaskWorkspace(requestId: string) {
    if (lastAutoOpenedTaskRequestId === requestId) return
    lastAutoOpenedTaskRequestId = requestId
    void openTaskWorkspace(requestId)
  }

  async function openRambelleProfile() {
    if (workspaceTransitionLocked || pendingWorkspaceViewKey) return
    const view = rambelleProfileViewDescriptor()
    if (workspaceShellState.activeViewKey === workspaceViewKey(view)) return
    workspaceTransition.invalidate()
    await workspaceTransition.activate({
      view,
      requestId: null,
      shellAction: { type: 'open' },
      pendingViewKey: workspaceViewKey(view),
    })
  }

  async function openArchivedSessions(initialSession: SessionViewDescriptor | null = null) {
    if (workspaceTransitionLocked || pendingWorkspaceViewKey) return
    archivedInitialSession = initialSession
    archivedSelectionEpoch += 1
    const view = archiveViewDescriptor()
    const viewKey = workspaceViewKey(view)
    if (workspaceShellState.activeViewKey === viewKey) return
    workspaceTransition.invalidate()
    await workspaceTransition.activate({
      view,
      requestId: null,
      shellAction: { type: 'open' },
      pendingViewKey: viewKey,
    })
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
    transport: applicationTransport,
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
    hasPendingSpeech: (requestId) => rambleController?.hasPendingSpeech(requestId) ?? false,
    getSpeechStopError: () => voicePhase === 'error' ? rambleMessage : '',
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
      const result = await applicationTransport.call('approveFeedbackRequest', input)
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
        reason: 'Human cancelled from RambleDesk',
      }
      const result = await applicationTransport.call('cancelFeedbackRequest', input)
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
    if (!feedbackResult || !workspace) return
    try {
      await publishedFeedbackAction.run(workspace.request.request_id)
    } catch (cause) {
      pageError = tr(
        publishedFeedbackAction.label === 'Open feedback package'
          ? 'Could not open Feedback Package: {error}'
          : 'Could not download published feedback: {error}',
        { error: messageFrom(cause) },
      )
    }
  }

  async function exitRamble() {
    await rambleController?.exitRamble()
    await rambleController?.settleSpeechDrafts()
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
    {capabilities}
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
    interactionLocked={managedFeedbackReadOnly || interactionLocked || currentRequestCooking || cookedDraftReady}
    onPageError={(message) => (pageError = message)}
    onStartScreenCapture={attachmentController.startScreenCapture}
    onImportServerAttachmentPaths={attachmentController.importServerAttachmentPaths}
    onPersistAttachmentCandidates={attachmentController.persistAttachmentCandidates}
    onRouteDraftOperation={routeDraftOperation}
    getActiveAction={activeActionFor}
    onOpenSpeechTarget={async (requestId, segmentId) => {
      if (await openRequest(requestId)) {
        await tick()
        if (segmentId) highlightSpeechSegment(document, segmentId, true)
      }
    }}
  />

  <AppTitlebar
    windowControls={capabilities.windowControls}
    sidebarCollapsed={hostSessionRailCollapsed}
    pendingCount={$navigation.pendingRequests.length}
    ramblePhase={visibleRamblePhase}
    {rambleRequestTitle}
    onWindowError={(message) => (pageError = tr('Window action failed: {error}', { error: message }))}
  >
    {#snippet workspaceTabs()}
      <WorkspaceTabStrip
        views={workspaceShellState.views}
        activeViewKey={workspaceShellState.activeViewKey}
        pendingViewKey={pendingWorkspaceViewKey}
        disabled={workspaceTransitionLocked}
        labelForView={workspaceTabLabel}
        onActivate={(viewKey) => void activateWorkspaceTab(viewKey)}
        onClose={closeWorkspaceTab}
        onReorder={reorderWorkspaceTabs}
      />
    {/snippet}
  </AppTitlebar>

  <div class="flex h-[calc(100%-40px)] min-h-0 min-w-0">
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
      onNewSession={previewMode ? undefined : () => void openNewManagedSession()}
      onDeleteManagedSession={(session) => deleteManagedSessionFromUi(session).catch(() => {})}
    />

    <div class="flex min-h-0 min-w-0 flex-1" id="request-workspace-layout">
      {#if renderedWorkspaceSurface !== 'standalone'}
        <div
          class={['shrink-0 border-r transition-[width] duration-200 motion-reduce:transition-none', requestRailCollapsed ? 'w-14' : 'w-[296px]']}
          id="request-list-pane"
        >
          <RequestListPane
            bind:collapsed={requestRailCollapsed}
            requests={$navigation.requests}
            activeRequestId={workspace?.request.request_id ?? null}
            cookingRequestIds={cookingRequestIds}
            scopeLabel={requestScopeLabel}
            searchQuery={$navigation.requestSearch}
            loading={$navigation.loadingRequests}
            refreshing={$navigation.refreshingPage}
            loadingMore={$navigation.loadingMoreRequests}
            hasMore={$navigation.nextRequestCursor !== null}
            filters={$navigation.requestFilters}
            {resolveHostProfile}
            formatTime={formatTimeLocal}
            onLoadMore={() => void navigation.loadMoreRequests()}
            onOpenRequest={(requestId) => void openRequest(requestId)}
            onFiltersChange={(filters) => void navigation.setRequestFilters(filters)}
          />
        </div>
      {/if}

      <div class="min-h-0 min-w-0 flex-1" id="workspace-pane">
        <div class="flex h-full min-h-0 min-w-0 flex-col">
          {#if renderedManagedSession && renderedSessionResolution?.kind !== 'missing-session'}
            <div class="flex shrink-0 items-center gap-1 border-b px-4 py-2" role="group" aria-label={agentText($locale, 'Managed session')}>
              <Button size="sm" variant={showManagedAgent ? 'secondary' : 'ghost'} aria-pressed={showManagedAgent} disabled={workspaceTransitionLocked} onclick={() => void showManagedAgentPanel()}>{agentText($locale, 'Agent session')}</Button>
              <Button size="sm" variant={!showManagedAgent ? 'secondary' : 'ghost'} aria-pressed={!showManagedAgent} disabled={workspaceTransitionLocked || (!managedFeedbackRequestId && renderedManagedFeedbackRequests.length === 0)} onclick={() => { const requestId = managedFeedbackRequestId ?? renderedManagedFeedbackRequests[0]?.request_id; if (requestId) void openRequest(requestId) }}>{agentText($locale, 'Feedback requests')}<span class="ml-1 text-[10px] tabular-nums text-muted-foreground">{renderedManagedSession.request_count}</span></Button>
            </div>
          {/if}
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
            {#if renderedWorkspaceView?.kind === 'inbox'}
              <InboxWorkspaceView />
            {:else if renderedWorkspaceView?.kind === 'archive'}
              <ArchivedSessionsWorkspaceView
                transport={applicationTransport}
                {previewMode}
                {resolveHostProfile}
                formatTime={formatTimeLocal}
                {messageFrom}
                initialSession={archivedInitialSession}
                selectionEpoch={archivedSelectionEpoch}
                onError={(message) => (pageError = message)}
                onChanged={retrySessionViewRecovery}
                onDeleteManagedSession={deleteManagedSessionFromUi}
              />
            {:else if renderedWorkspaceView?.kind === 'settings'}
              <SettingsWorkspaceView
                transport={applicationTransport}
                {capabilities}
                mcpConfiguration={genericMcpConfiguration}
                section={settingsSection}
                sectionSelectionEpoch={settingsSectionSelectionEpoch}
                {updateInstallBlocked}
                onRestartOnboarding={restartOnboarding}
                onOpenArchived={() => void openArchivedSessions()}
                onOpenRambelleProfile={() => void openRambelleProfile()}
              />
            {:else if renderedWorkspaceView?.kind === 'request-task'}
              <TaskWorkspaceView
                transport={applicationTransport}
                {capabilities}
                {workspace}
                {editorDocument}
                activeActionId={workspace
                  ? activeActionByRequest.get(workspace.request.request_id)?.actionId ?? null
                  : null}
                actionsDisabled={managedFeedbackReadOnly || workspaceTransitionLocked || pendingWorkspaceViewKey !== null}
                onSelectAction={selectAction}
                previews={attachmentPreviews}
                loading={loadingWorkspace}
                formatTime={formatTimeLocal}
                {resolveHostProfile}
                onToggleRamble={() => void toggleRamble()}
                ramblePhase={rambleBelongsToWorkspace ? visibleRamblePhase : 'idle'}
                rambleStartedOnce={rambleBelongsToWorkspace ? rambleStartedOnce : false}
                rambleBusy={rambleBelongsToWorkspace ? rambleBusy : true}
                canSubmit={!managedFeedbackReadOnly}
                cookingEnabled={$cookingEnabled}
                {cookedDraftReady}
                cooking={currentRequestCooking}
                {submitting}
                onSubmitFeedback={() => void submitFeedback()}
              />
            {:else if renderedWorkspaceView?.kind === 'rambelle-profile'}
              <RambelleProfileWorkspaceView />
            {:else if renderedSessionResolution?.kind === 'missing-session'}
              <MissingSessionView
                missing={renderedSessionResolution}
                label={sessionTabLabel(renderedSessionResolution.session)}
                busy={renderedSessionResolution.reason === 'unresolved' || pendingWorkspaceViewKey !== null}
                onRetry={retrySessionViewRecovery}
                onClose={() => closeWorkspaceTab(workspaceViewKey(renderedSessionResolution!.session))}
                onOpenArchive={() => void openArchivedSessions(renderedSessionResolution!.session)}
              />
            {:else if workbenchMounted && renderedManagedSession && showManagedAgent}
              {#key renderedManagedSession.session_id}
                <ManagedSessionSection
                  transport={applicationTransport}
                  sessionId={renderedManagedSession.session_id}
                  deletionPending={deletingSessionCommands.has(renderedManagedSession.session_id)}
                  onDeletingChange={observeManagedDeletion}
                  onDelete={() => deleteManagedSessionFromUi(renderedManagedSession!)}
                  feedbackRequests={renderedManagedFeedbackRequests}
                  onOpenFeedback={async (requestId) => { await openRequest(requestId) }}
                />
              {/key}
            {:else if workbenchMounted}
              <div class="flex h-full min-h-0 flex-col">
              {#if renderedManagedSession}
                {#key renderedManagedSession.session_id}
                  <ManagedSessionSection
                    transport={applicationTransport}
                    sessionId={renderedManagedSession.session_id}
                    deletionPending={deletingSessionCommands.has(renderedManagedSession.session_id)}
                    onDeletingChange={observeManagedDeletion}
                    onDelete={() => deleteManagedSessionFromUi(renderedManagedSession!)}
                    showWorkspace={false}
                    feedbackRequests={renderedManagedFeedbackRequests}
                    onOpenFeedback={async (requestId) => { await openRequest(requestId) }}
                  />
                {/key}
              {/if}
              <div class="min-h-0 flex-1">
              {#key renderedSessionView ? workspaceViewKey(renderedSessionView) : 'workspace:empty'}
              <SessionWorkbench
            readOnly={managedFeedbackReadOnly}
            transport={applicationTransport}
            {capabilities}
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
            onDraftChange={updateDraft}
            onTidyError={(message) => (pageError = message)}
            onOpenTidySettings={() => void openSettings('post-processing')}
            onSelectAction={selectAction}
            onToggleRamble={() => void toggleRamble()}
            onExitRamble={() => void exitRamble()}
            onOpenVoiceSettings={() => void openSettings('voice')}
            onOpenTask={(requestId) => void openTaskWorkspace(requestId)}
            onAutoOpenTask={autoOpenTaskWorkspace}
            onStartScreenCapture={() => void attachmentController.startScreenCapture()}
            onImportClipboard={() => void importClipboardNow()}
            onFileSelection={attachmentController.handleFileSelection}
            onPasteCandidates={attachmentController.acceptAttachmentCandidates}
            onPasteError={attachmentController.reportClientFileError}
            onRemoveAttachment={(attachment) => void attachmentController.removeAttachment(attachment)}
            onOpenPackage={() => void openFeedbackPackage()}
            packageActionLabel={tr(publishedFeedbackAction.label)}
            onOpenResumePrompt={openResumePrompt}
            onSubmit={() => void submitFeedback()}
            onCancel={() => void cancelFeedback()}
            onApprove={() => void approveFeedback()}
              />
              {/key}
              </div>
              </div>
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
      </div>
    </div>

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

<Dialog.Root open={newManagedSessionOpen} onOpenChange={(open) => { if (!creatingManagedSession) newManagedSessionOpen = open }}>
  <Dialog.Content class="max-h-[90vh] overflow-y-auto p-0 sm:max-w-xl" showCloseButton={!creatingManagedSession}>
    <Dialog.Title class="sr-only">{agentText($locale, 'New agent session')}</Dialog.Title>
    <Dialog.Description class="sr-only">{agentText($locale, 'Use an absolute directory on the computer running RambleDesk.')}</Dialog.Description>
    <NewManagedSessionSection
      transport={applicationTransport}
      onCreating={(creating) => { creatingManagedSession = creating }}
      onCreated={managedSessionCreated}
      onConfigure={() => { newManagedSessionOpen = false; void openSettings('agents') }}
    />
  </Dialog.Content>
</Dialog.Root>

{#if onboardingAvailable}
  <OnboardingWizard {capabilities} bind:openWizard={onboardingOpen} onClose={closeOnboarding} />
{/if}

{#if softwareUpdatesAvailable}
  <UpdateAvailableDialog
    softwareUpdates={capabilities.softwareUpdates}
    installBlocked={updateInstallBlocked}
    onOpenReleases={() => void openGithubReleases()}
  />
{/if}
{/key}
