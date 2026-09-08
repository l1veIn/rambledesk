<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { initializeAppearance } from './lib/appearance/appearanceRuntime'

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
  import StartupRecoveryPanel from './lib/workbench/StartupRecoveryPanel.svelte'
  import WorkbenchShell from './lib/workbench/WorkbenchShell.svelte'
  import InboxWorkspaceView from './lib/workspace/InboxWorkspaceView.svelte'
  import MissingSessionView from './lib/workspace/MissingSessionView.svelte'
  import RambelleProfileWorkspaceView from './lib/workspace/RambelleProfileWorkspaceView.svelte'
  import SettingsWorkspaceView from './lib/workspace/SettingsWorkspaceView.svelte'
  import TaskWorkspaceView from './lib/workspace/TaskWorkspaceView.svelte'
  import ArchivedSessionsWorkspaceView from './lib/workspace/ArchivedSessionsWorkspaceView.svelte'
  import ManagedSessionSection from './lib/agents/ManagedSessionSection.svelte'
  import ManagedFeedbackRequestStatus from './lib/agents/ManagedFeedbackRequestStatus.svelte'
  import DraftManagedSessionWorkspace from './lib/agents/DraftManagedSessionWorkspace.svelte'
  import { agentText } from './lib/agents/agentI18n'
  import { Button } from './lib/components/ui/button'
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
  import { readApplicationSnapshot } from './lib/application/readApplicationSnapshot'
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
  /** Browser clients are full-bleed pages; the desktop shell owns the rounded window frame. */
  export let environment: 'desktop' | 'browser' = 'desktop'

  provideWorkbenchCapabilities(capabilities)
  onMount(() => initializeAppearance({
    setZoom: capabilities.windowControls.status.source === 'native'
      ? factor => capabilities.windowControls.implementation.setZoom(factor)
      : undefined,
  }))

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
    notificationStateForPermission,
    type NotificationState,
  } from './lib/notifications'
  import {
    archiveViewDescriptor,
    rambelleProfileViewDescriptor,
    requestTaskViewDescriptor,
    sessionViewDescriptor,
    settingsViewDescriptor,
    workspaceViewKey,
    type SessionViewDescriptor,
    type WorkspaceViewDescriptor,
  } from './lib/workspace/viewDescriptors'
  import { activeWorkspaceView } from './lib/workspace/workspaceShell'
  import { updateTaskTabTitles } from './lib/workspace/taskTabTitles'
  import { agentSessionForView, agentViewForEmptyRamble, agentViewForRequest } from './lib/workspace/agentViewRouting'
  import { seedPreviewWorkspaceScenario } from './lib/workspace/previewWorkspaceSnapshot'
  import type { SessionViewResolution } from './lib/workspace/sessionViewRecovery'
  import {
    workspaceTabId,
    workspaceTabPanelId,
  } from './lib/workspace/workspaceTabNavigation'
  import WorkspaceTabStrip from './lib/workspace/WorkspaceTabStrip.svelte'
  import { workspaceSurface } from './lib/workspace/workspaceSurface'
  import { createWorkspaceTransition } from './lib/workspace/workspaceTransition'
  import {
    shouldAdoptTaskBackgroundDraft,
    shouldUseForegroundDraftEditor,
  } from './lib/workspace/draftOperationRouting'
  import { previewFixtures, previewWorkspaceFor } from './lib/previewFixtures'
  import type { PublishedFeedbackView } from './lib/publishedFeedback'
  import { formatTime, messageFrom } from './lib/workbench/feedbackText'
  import { createCookingController } from './lib/workbench/cookingController'
  import { createCookingSession } from './lib/workbench/cookingSession'
  import { createDraftController } from './lib/workbench/draftController'
  import { createDraftOperationsController } from './lib/workbench/draftOperationsController'
  import { createDraftSession } from './lib/workbench/draftSession'
  import { createSubmissionController } from './lib/workbench/submissionController'
  import { createAttachmentSession } from './lib/workbench/attachmentSession'
  import { createManagedSessionActions } from './lib/workbench/managedSessionActions'
  import { createWorkspaceNavigationController, type WorkspaceNavigationController } from './lib/workbench/workspaceNavigationController'
  import { createStartupController, type StartupController } from './lib/workbench/startupController'
  import { createRambleSession } from './lib/workbench/rambleSession'
  import { createShellLayoutSession } from './lib/workbench/shellLayoutSession'
  import { createWorkspaceSession } from './lib/workbench/workspaceSession'
  import { createWorkspaceShellSession } from './lib/workbench/workspaceShellSession'
  import { createPublisherController } from './lib/workbench/publisherController'
  import { buildResumePrompt, shouldShowResumePromptButton } from './lib/workbench/resumePrompt'
  import {
    createAttachmentController,
  } from './lib/workbench/attachmentController'
  import { createNavigationController } from './lib/workbench/navigationController'
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
    initialWebAccessAutostart,
    initialWebAccessPort,
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
  const formatTimeLocal = (value: string | null | undefined) =>
    formatTime(value, $locale, tr('Not saved yet'))
  const workspaceSession = createWorkspaceSession()
  const attachmentSession = createAttachmentSession()
  const cookingSession = createCookingSession()
  const shellLayout = createShellLayoutSession()
  const rambleSession = createRambleSession()
  /** The open request, projected from the session so field reads stay short. */
  $: currentRequest = $workspaceSession.workspace?.request ?? null
  let taskTabTitles: ReadonlyMap<string, string> = new Map()
  let renderedWorkspaceView: WorkspaceViewDescriptor | null = null
  let renderedSessionView: SessionViewDescriptor | null = null
  let renderedSessionResolution: SessionViewResolution | null = null
  let pageError = ''
  let deliveredAttachmentMessage = ''
  let deliveredPageError = ''
  let deliveredSaveError = ''
  let sessionWorkbench: FeedbackEditorHandle | undefined
  let managedSessionSection: ManagedSessionSection | undefined
  let rambleController: RambleSessionControllerHandle
  let resumePrompt: ResumePrompt | null = null
  let resumeCopyState: 'idle' | 'copied' | 'failed' = 'idle'
  let notificationState: NotificationState = 'checking'
  let archivedInitialSession: SessionViewDescriptor | null = null
  let archivedSelectionEpoch = 0
  let settingsSection: SettingsSection = 'general'
  let settingsSectionSelectionEpoch = 0
  let settingsAgentConfigId: string | undefined = undefined
  let settingsAgentAdvanced = false
  let lastAutoOpenedTaskRequestId = ''
  let onboardingOpen = false
  let launchUpdateCheckDue = false
  const desktopShellAvailable = capabilities.windowControls.status.source === 'native'
  const isMac = capabilities.windowControls.implementation.platform() === 'macOS'
  const notificationsAvailable = capabilities.notifications.status.availability !== 'unavailable'
  const softwareUpdatesAvailable = capabilities.softwareUpdates.status.availability !== 'unavailable'
  const onboardingAvailable = !previewMode
  const previewWorkspaceScenario = previewMode
    ? seedPreviewWorkspaceScenario(
        new URLSearchParams(window.location.search).get('workspace'),
      )
    : null
  const workspaceShell = createWorkspaceShellSession({ previewMode })
  if (workspaceShell.restoredActiveView()) workspaceSession.setLoading(true)
  let taskBriefOpen = true
  let hostRailDisplayWidth = 0
  let navigationResizing = false
  let projectSearch = ''
  $: tidyConfig = {
    provider: $tidyProvider,
    apiKey: $tidyApiKey,
    baseUrl: $tidyBaseUrl,
    model: $tidyModel,
    reasoningEffort: $tidyReasoningEffort,
    locale: $locale,
    systemPrompt: $tidySystemPrompt,
  }
  let inboxTimer: ReturnType<typeof setInterval> | undefined

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  const draftSession = createDraftSession()
  const draftController = createDraftController({
    transport: applicationTransport,
    messageFrom,
    isPreviewMode: () => previewMode,
    isInteractionLocked: () => interactionLocked,
    isWorkspaceTerminal: () => workspaceSession.isTerminal(),
    getWorkspace: () => $workspaceSession.workspace,
    session: draftSession,
    setWorkspaceDraft: (draft) => workspaceSession.setDraft(draft),
  })
  const updateDraft = draftController.updateDraft
  const saveDraftNow = draftController.saveDraftNow

  const draftOperations = createDraftOperationsController({
    transport: applicationTransport,
    tr,
    messageFrom,
    isPreviewMode: () => previewMode,
    getActiveView: () => activeWorkspaceView($workspaceShell.shell),
    getPendingViewKey: () => $workspaceShell.pendingViewKey,
    getWorkspace: () => $workspaceSession.workspace,
    getCurrentRequest: () => currentRequest,
    getEditor: () => sessionWorkbench,
    isWorkbenchMounted: () => $startup.mounted,
    isTransitionLocked: () => workspaceTransitionLocked,
    getDraftMessage: () => $draftSession.message,
    saveDraftNow,
    setWorkspaceDraft: (draft) => workspaceSession.setDraft(draft),
    adoptDraft: (draft) => draftSession.adopt(draft),
    setPageError: (message) => {
      pageError = message
    },
  })
  const {
    routeDraftOperation,
    activeActionFor,
    enqueueDocumentTask,
    waitForDocumentQueue,
    selectAction,
  } = draftOperations

  const attachmentController = createAttachmentController({
    capabilities,
    transport: applicationTransport,
    tr,
    messageFrom,
    getWorkspace: () => $workspaceSession.workspace,
    getEditor: () => sessionWorkbench,
    getRambleRequestId: () => $rambleSession.requestId,
    getInteractionLocked: () => interactionLocked || currentRequestCooking || cookedDraftReady,
    getSavedRevision: () => $draftSession.savedRevision,
    session: attachmentSession,
    saveDraftNow,
    waitForRambleMarkdown: waitForDocumentQueue,
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

  // These controllers are assigned below; the callbacks only run after composition.
  let startup: StartupController
  let workspaceNavigation: WorkspaceNavigationController
  const workspaceTransition = createWorkspaceTransition<LoadedWorkspaceTarget>({
    saveCurrent: saveDraftNow,
    unmountCurrent: () => {
      startup.patch({ mounted: false })
      sessionWorkbench = undefined
      workspaceSession.setLoading(true)
    },
    loadTarget: (target) => workspaceNavigation.loadWorkspaceTarget(target),
    commitTarget: (target, loaded) => workspaceNavigation.commitWorkspaceTarget(target, loaded),
    restoreCurrent: () => {
      startup.patch({ mounted: true })
      workspaceSession.setLoading(false)
    },
    setPendingTarget: (target) => {
      workspaceShell.setPendingViewKey(target?.pendingViewKey ?? null)
    },
    reportFailure: (cause) => {
      if (startup.phase() === 'loading') startup.patch({ workspaceFailure: cause })
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
    getWorkspaceRequestId: () => workspaceSession.requestId() ?? undefined,
    isDirty: () => dirty,
    saveDraftNow,
    openRequest: (requestId) => workspaceNavigation.openRequest(requestId),
    clearWorkspace,
    onPageError: (message) => (pageError = message),
    canSendOsBanners: () => isMac,
    onRequestsArrived: (requests) => { void workspaceNavigation.autoOpenArrivingRequest(requests) },
  })
  const resolveHostProfile = navigation.resolveHostProfile
  startup = createStartupController({
    navigation,
    workspaceShell,
    workspaceSession,
    draftSession,
    transport: applicationTransport,
    workspaceTransition,
    previewMode,
    desktopShellAvailable,
    tr,
    messageFrom,
    pageError: () => pageError,
    setPageError: (message) => {
      pageError = message
    },
    clearWorkspace,
    selectAgentNavigationScope: (view) => workspaceNavigation.selectAgentNavigationScope(view),
    requestIdForSession: (view, requests) => workspaceNavigation.requestIdForSession(view, requests),
    onReady: () => {
      inboxTimer = ensureDesktopNavigationPolling(
        desktopShellAvailable,
        inboxTimer,
        setInterval,
        () => void navigation.refreshNavigation(true),
      )
    },
  })

  const managedSessions = createManagedSessionActions({
    transport: applicationTransport,
    navigation,
    workspaceShell,
    workspaceSession,
    draftSession,
    workspaceTransition,
    previewMode,
    tr,
    messageFrom,
    setPageError: (message) => {
      pageError = message
    },
    clearWorkspace,
    setCookingPreview: (preview) => cookingSession.setPreview(preview),
    isTransitionLocked: () => workspaceTransitionLocked,
    selectAgentNavigationScope: (view) => workspaceNavigation.selectAgentNavigationScope(view),
    exitRamble,
    rambleCanExit: () => rambleCanExit,
    openArchivedSessions: (initial) => openArchivedSessions(initial),
  })

  workspaceNavigation = createWorkspaceNavigationController({
    navigation,
    workspaceShell,
    workspaceSession,
    draftSession,
    attachmentSession,
    startup,
    managedSessions,
    transport: applicationTransport,
    workspaceTransition,
    previewMode,
    tr,
    messageFrom,
    pageError: () => pageError,
    setPageError: (message) => {
      pageError = message
    },
    clearWorkspace,
    refreshNotificationPermission,
    isTransitionLocked: () => workspaceTransitionLocked,
    enqueueDocumentTask,
    canAutoOpenRamble: (sessionId) => managedSessionSection?.canAutoOpenRamble(sessionId) === true,
    onboardingOpen: () => onboardingOpen,
    resumePromptOpen: () => resumePrompt !== null,
    rambleEngaged: () => rambleEngaged,
    releaseAttachmentPreviews: () => attachmentController.releasePreviews(),
    refreshAttachmentPreviews: (workspace) => attachmentController.refreshPreviews(workspace),
    setCookingPreview: (preview) => cookingSession.setPreview(preview),
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

  $: railAgentSession = agentSessionForView(
    renderedWorkspaceView?.kind === 'agent-session' ? renderedWorkspaceView : null,
    $navigation.hostSessions,
  )
  $: dirty =
    currentRequest !== null &&
    currentRequest.status !== 'completed' &&
    currentRequest.status !== 'cancelled' &&
    draftSession.isDirty()
  $: {
    if (!pageError) deliveredPageError = ''
    else if (pageError !== deliveredPageError) {
      deliveredPageError = pageError
      toast.error(tr('Operation failed'), { description: pageError })
    }
  }
  $: {
    if (!$draftSession.message) deliveredSaveError = ''
    else if ($draftSession.message !== deliveredSaveError) {
      deliveredSaveError = $draftSession.message
      toast.error(tr('Save failed'), { description: $draftSession.message })
    }
  }
  $: {
    if (!$attachmentSession.message) {
      deliveredAttachmentMessage = ''
    } else if ($attachmentSession.message !== deliveredAttachmentMessage) {
      deliveredAttachmentMessage = $attachmentSession.message
      const options = { description: $attachmentSession.message }
      if ($attachmentSession.tone === 'success') toast.success(tr('Attachment action completed'), options)
      else if ($attachmentSession.tone === 'info') toast.info(tr('Attachment status'), options)
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
  $: renderedWorkspaceView = activeWorkspaceView($workspaceShell.shell)
  $: renderedWorkspaceSurface = workspaceSurface(renderedWorkspaceView)
  $: renderedSessionView = renderedWorkspaceView?.kind === 'session'
    ? renderedWorkspaceView
    : null
  $: renderedSessionResolution = startup.resolutionFor($workspaceShell.shell.activeViewKey)
  $: renderedAgentSessionView = renderedWorkspaceView?.kind === 'agent-session' ? renderedWorkspaceView : null
  $: renderedAgentDraftView = renderedWorkspaceView?.kind === 'agent-draft' ? renderedWorkspaceView : null
  $: renderedAgentDraftController = renderedAgentDraftView ? managedSessions.draftController(renderedAgentDraftView.draftId) : null
  $: renderedManagedSession = agentSessionForView(renderedAgentSessionView, $navigation.hostSessions)
  const sessionTabLabel = (view: SessionViewDescriptor) => {
    const session = $navigation.hostSessions.find(
      (candidate) =>
        candidate.host_id === view.hostId &&
        candidate.host_session_id === view.hostSessionId,
    )
    const hostLabel = resolveHostProfile(view.hostId).label
    return `${session?.title ?? view.hostSessionId} · ${hostLabel}`
  }
  $: taskTabTitles = updateTaskTabTitles(taskTabTitles, $workspaceShell.shell.views, [
    ...(currentRequest ? [currentRequest] : []),
    ...$navigation.pendingRequests,
    ...$navigation.requests,
  ])
  $: workspaceTabLabel = (view: WorkspaceViewDescriptor) => {
    switch (view.kind) {
      case 'agent-draft':
        return $locale === 'zh-CN' ? '新建会话' : 'New session'
      case 'inbox':
        return tr('All requests')
      case 'archive':
        return tr('Archived sessions')
      case 'settings':
        return tr('Settings')
      case 'agent-session': {
        const session = $navigation.hostSessions.find((candidate) => candidate.session_id === view.sessionId)
        return session ? `${session.title} · Agent` : agentText($locale, 'Agent session')
      }
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
  $: feedbackResult = workspaceSession.feedbackResult()
  $: canOpenResumePrompt = shouldShowResumePromptButton(
    feedbackResult,
    $workspaceSession.completedResult?.resolution ?? currentRequest?.resolution,
    currentRequest?.managed_session_id,
  )
  $: feedbackManagedSessionId = agentViewForRequest(currentRequest)?.sessionId ?? null
  $: rambleAgentSessionId = feedbackManagedSessionId ?? (currentRequest ? null : agentViewForEmptyRamble(renderedSessionView, $navigation.hostSessions)?.sessionId ?? null)
  $: managedFeedbackReadOnly = !!feedbackManagedSessionId && ($managedSessions.deletingCommands.has(feedbackManagedSessionId) || $managedSessions.deletingSessions.has(feedbackManagedSessionId))
  $: currentRequestCooking =
    currentRequest !== null && $cookingSession.cookingRequestIds.has(currentRequest.request_id)
  $: cookedDraftReady = $cookingSession.preview !== null
  // Turning Cooking off discards any pending cooked preview: submitting then
  // publishes the editor content as-is.
  $: if (!$cookingEnabled) cookingSession.setPreview(null)
  $: canSubmit =
    !managedFeedbackReadOnly &&
    currentRequest !== null &&
    currentRequest.status !== 'completed' &&
    currentRequest.status !== 'cancelled' &&
    $draftSession.body.trim().length > 0 &&
    !currentRequestCooking &&
    !workspaceSession.interactionLocked()
  $: canCancel =
    currentRequest !== null &&
    currentRequest.status !== 'completed' &&
    currentRequest.status !== 'cancelled' &&
    !currentRequestCooking &&
    !workspaceSession.interactionLocked()
  $: interactionLocked = workspaceSession.interactionLocked()
  $: workspaceTransitionLocked =
    interactionLocked ||
    $attachmentSession.busy ||
    $attachmentSession.captureBusy ||
    currentRequestCooking ||
    cookedDraftReady
  $: recoveryFingerprint = `${$navigation.hostSessionFactsStatus}:${$navigation.hostSessionFactsRevision}:${$workspaceShell.shell.views
    .map(workspaceViewKey)
    .join('\u0001')}:${$navigation.hostSessions
    .map((session) =>
      workspaceViewKey(sessionViewDescriptor(session.host_id, session.host_session_id)),
    )
    .sort()
    .join('\u0001')}`
  $: if (recoveryFingerprint) void startup.recoverIfChanged(recoveryFingerprint)
  $: voiceActive = $rambleSession.voicePhase === 'starting' ||
    $rambleSession.voicePhase === 'listening' ||
    $rambleSession.voicePhase === 'processing' ||
    $rambleSession.voicePhase === 'stopping'
  $: voiceCanStop = voiceActive || $rambleSession.voicePhase === 'error'
  $: visibleRamblePhase = resolvedRamblePhase($rambleSession.phase, $rambleSession.voicePhase)
  $: rambleActive = visibleRamblePhase === 'active'
  $: rambleEngaged = visibleRamblePhase !== 'idle'
  $: rambleBelongsToWorkspace =
    !rambleEngaged || currentRequest?.request_id === $rambleSession.requestId
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
    $attachmentSession.busy ||
    interactionLocked ||
    currentRequestCooking ||
    currentRequest?.status === 'in_progress'
  function setHostRailCollapsed(collapsed: boolean) {
    shellLayout.setRailCollapsed('host', collapsed)
  }

  function setRequestRailCollapsed(collapsed: boolean) {
    shellLayout.setRailCollapsed('request', collapsed)
  }

  function closePhoneDrawers() {
    shellLayout.closePhoneDrawers()
  }

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
      if ($onboardingCompleted || !onboardingAvailable) void startup.start()
      else onboardingOpen = true
      if (previewMode) {
        if (!workspaceShell.hasRestoredSnapshot()) {
          workspaceSession.open(previewFixtures.workspace)
          draftSession.adopt(previewFixtures.workspace.draft)
          workspaceNavigation.openLoadedWorkspaceView(previewFixtures.workspace)
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
    if ($onboardingCompleted || !onboardingAvailable) startup.start()
    else onboardingOpen = true
    // Browser server autostart is a desktop preference; the browser client never runs it.
    if (
      initialWebAccessAutostart() &&
      capabilities.webAccessAdministration.status.availability !== 'unavailable'
    ) {
      void capabilities.webAccessAdministration.implementation
        .setEnabled(true, initialWebAccessPort())
        .catch(() => undefined)
    }
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
        const knownRequest = currentRequest?.request_id === prompt.request_id ? currentRequest
          : [...$navigation.requests, ...$navigation.pendingRequests].find((request) => request.request_id === prompt.request_id)
        const request = knownRequest ?? (await readApplicationSnapshot(applicationTransport, 'getFeedbackWorkspace', { request_id: prompt.request_id })).request
        if (!isCurrent()) return
        if (request.managed_session_id) return
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
      if (updateCheckTimer !== undefined) clearTimeout(updateCheckTimer)
      cleanupAttachments()
      managedSessions.closeAllDrafts()
    }
  })

  async function startOnboardingSession(configId?: string) {
    if (!await startup.start()) {
      throw new Error($locale === 'zh-CN' ? '初始化未完成，请检查连接后重试。' : 'Initialization did not finish. Check the connection and retry.')
    }
    if (!await managedSessions.openNewManagedSession(configId)) {
      throw new Error($locale === 'zh-CN' ? '暂时无法打开新会话，请稍后重试。' : 'Could not open a new session. Please retry.')
    }
  }

  function closeOnboarding() {
    onboardingOpen = false
    startup.start()
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
    void workspaceNavigation.closeWorkspaceTab(workspaceViewKey(settingsViewDescriptor()))
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
    const workspace = $workspaceSession.workspace
    if (!workspace || !canOpenResumePrompt) return
    resumePrompt = buildResumePrompt(workspace, resolveHostProfile(workspace.request.host_id), tr)
    resumeCopyState = 'idle'
  }

  function clearWorkspace() {
    workspaceSession.close()
    draftSession.reset()
    attachmentController.releasePreviews()
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
      await startup.refreshSessionViewRecovery()
      if (!intent.isCurrent()) return
    }

    while (workspaceTransitionLocked && intent.isCurrent()) {
      await new Promise((resolve) => window.setTimeout(resolve, 50))
    }
    if (!intent.isCurrent()) return

    const activeView = activeWorkspaceView($workspaceShell.shell)
    const activeWorkspace = $workspaceSession.workspace
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

  async function refreshNotificationPermission() {
    try {
      const granted = await capabilities.notifications.implementation.permission() === 'granted'
      if (isMac && !granted && $notificationPopupEnabled) setNotificationPopupEnabled(false)
      notificationState = notificationStateForPermission(granted, $notificationPopupEnabled)
    } catch {
      notificationState = 'unavailable'
    }
  }

  async function openSettings(section: SettingsSection, agentConfigId?: string, agentAdvanced = false) {
    if ($startup.phase === 'failed') {
      startup.patch({ settingsOpen: true })
      return
    }
    await workspaceNavigation.openView(settingsViewDescriptor(), {
      prepare: () => {
        settingsSection = section
        settingsAgentConfigId = agentConfigId
        settingsAgentAdvanced = agentAdvanced
        settingsSectionSelectionEpoch += 1
      },
    })
  }

  async function openTaskWorkspace(requestId: string) {
    await workspaceNavigation.openView(requestTaskViewDescriptor(requestId), { requestId })
  }

  function autoOpenTaskWorkspace(requestId: string) {
    if (lastAutoOpenedTaskRequestId === requestId) return
    lastAutoOpenedTaskRequestId = requestId
    void openTaskWorkspace(requestId)
  }

  async function openRambelleProfile() {
    await workspaceNavigation.openView(rambelleProfileViewDescriptor())
  }

  async function openArchivedSessions(initialSession: SessionViewDescriptor | null = null) {
    await workspaceNavigation.openView(archiveViewDescriptor(), {
      prepare: () => {
        archivedInitialSession = initialSession
        archivedSelectionEpoch += 1
      },
    })
  }

  function applyWorkspaceMutation(next: FeedbackWorkspaceView) {
    workspaceSession.replace(next)
    if (draftSession.reconcile(next.draft) === 'kept-local') draftController.scheduleSave()
  }

  const cookingController = createCookingController({
    tr,
    messageFrom,
    getWorkspace: () => $workspaceSession.workspace,
    getDraftBody: () => $draftSession.body,
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
    setCooking: cookingSession.setCooking,
    publishCooked: (input, cookedMarkdown, uncookedMarkdown) =>
      publisherController.publishFeedback(input, cookedMarkdown, uncookedMarkdown),
    setPreview: cookingSession.setPreview,
  })
  const cookPreviewOnly = cookingController.cookPreviewOnly
  const restoreOriginalAfterCook = cookingController.restoreOriginal

  const publisherController = createPublisherController({
    transport: applicationTransport,
    tr,
    messageFrom,
    isPreviewMode: () => previewMode,
    getWorkspace: () => $workspaceSession.workspace,
    setWorkspace: (next) => workspaceSession.replace(next),
    setCompletedResult: (result) => workspaceSession.setCompleted(result),
    setPublishedFeedback: (feedback) => workspaceSession.setPublished(feedback),
    setSavePhase: () => {
      draftSession.markSaved()
    },
    setPageError: (message) => {
      pageError = message
    },
    getCanSubmit: () => canSubmit,
    getRambleCanExit: () => rambleCanExit,
    hasPendingSpeech: (requestId) => rambleController?.hasPendingSpeech(requestId) ?? false,
    getSpeechStopError: () => rambleSession.speechStopError(),
    exitRamble,
    saveDraftNow,
    getDraftBody: () => $draftSession.body,
    getSavedRevision: () => $draftSession.savedRevision,
    getCookingEnabled: () => $cookingEnabled,
    getPreview: cookingSession.preview,
    setPreview: cookingSession.setPreview,
    setCooking: cookingSession.setCooking,
    cookAndPublish: cookingController.cookAndPublish,
    setSubmitting: (value) => workspaceSession.setSubmitting(value),
    setSubmitStage: (stage) => workspaceSession.setSubmitStage(stage),
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

  const submissionController = createSubmissionController({
    transport: applicationTransport,
    session: workspaceSession,
    draftSession,
    publishedFeedbackAction,
    tr,
    messageFrom,
    canCancel: () => canCancel,
    rambleCanExit: () => rambleCanExit,
    exitRamble,
    refreshNavigation: async () => {
      await navigation.refreshNavigation(true)
    },
    setPageError: (message) => {
      pageError = message
    },
    notifyApproved: () => toast.success(tr('Approved and finished')),
    notifyCancelled: () => toast.success(tr('Request cancelled')),
  })
  const approveFeedback = submissionController.approveFeedback
  const cancelFeedback = submissionController.cancelFeedback
  const openFeedbackPackage = submissionController.openFeedbackPackage

  async function exitRamble() {
    await rambleController?.exitRamble()
    await rambleController?.settleSpeechDrafts()
    await waitForDocumentQueue()
  }

  async function toggleRamble() {
    await rambleController?.toggleRamble()
  }

  async function importClipboardNow() {
    await rambleController?.importClipboardNow()
  }
</script>

{#snippet requestAgentStatus()}
  {#if feedbackManagedSessionId && currentRequest && !previewMode}
    {#key `${feedbackManagedSessionId}:${currentRequest.request_id}`}
      <ManagedFeedbackRequestStatus transport={applicationTransport} sessionId={feedbackManagedSessionId} requestId={currentRequest.request_id}
        disabled={managedFeedbackReadOnly || workspaceTransitionLocked || $workspaceShell.pendingViewKey !== null}
        navigationDisabled={workspaceTransitionLocked || $workspaceShell.pendingViewKey !== null}
        onOpenAgent={() => feedbackManagedSessionId && void managedSessions.openAgentSession(feedbackManagedSessionId)}
        onDeletingChange={managedSessions.observeDeletion} />
    {/key}
  {:else if rambleAgentSessionId}
    <div class="flex items-center justify-between gap-2 text-xs">
      <span class="text-muted-foreground">ACP</span>
      <Button size="sm" variant="ghost" disabled={workspaceTransitionLocked || $workspaceShell.pendingViewKey !== null} onclick={() => rambleAgentSessionId && void managedSessions.openAgentSession(rambleAgentSessionId)}>{$locale === 'zh-CN' ? '查看 Agent' : 'View Agent'}</Button>
    </div>
  {/if}
{/snippet}

<svelte:head>
  <title>RambleDesk · Feedback Inbox</title>
</svelte:head>

{#key $locale}
<main class={[
  'flex h-full w-full flex-col overflow-hidden rounded-[var(--app-frame-radius)] bg-background text-foreground',
  environment === 'browser' ? '' : 'border shadow-sm',
]}
  class:navigation-resizing={navigationResizing}
  style:--workbench-sidebar-width={`${hostRailDisplayWidth}px`}>
  <Sonner />
  <RambleSessionController
    bind:this={rambleController}
    {capabilities}
    {tidyConfig}
    workspace={$workspaceSession.workspace}
    bind:attachmentBusy={$attachmentSession.busy}
    screenCaptureBusy={$attachmentSession.captureBusy}
    bind:attachmentMessage={$attachmentSession.message}
    bind:voicePhase={$rambleSession.voicePhase}
    bind:voiceDevice={$rambleSession.voiceDevice}
    bind:voicePartial={$rambleSession.voicePartial}
    bind:voiceLevel={$rambleSession.voiceLevel}
    bind:voiceChunkIndex={$rambleSession.voiceChunkIndex}
    bind:voiceModelMissing={$rambleSession.voiceModelMissing}
    bind:ramblePhase={$rambleSession.phase}
    bind:rambleStartedOnce={$rambleSession.startedOnce}
    bind:rambleRequestId={$rambleSession.requestId}
    bind:rambleRequestTitle={$rambleSession.requestTitle}
    bind:rambleMessage={$rambleSession.message}
    interactionLocked={managedFeedbackReadOnly || interactionLocked || currentRequestCooking || cookedDraftReady}
    onPageError={(message) => (pageError = message)}
    onStartScreenCapture={attachmentController.startScreenCapture}
    onImportServerAttachmentPaths={attachmentController.importServerAttachmentPaths}
    onPersistAttachmentCandidates={attachmentController.persistAttachmentCandidates}
    onRouteDraftOperation={routeDraftOperation}
    getActiveAction={activeActionFor}
    onOpenSpeechTarget={async (requestId, segmentId) => {
      if (await workspaceNavigation.openRequest(requestId)) {
        await tick()
        if (segmentId) highlightSpeechSegment(document, segmentId, true)
      }
    }}
  />

  <AppTitlebar
    windowControls={capabilities.windowControls}
    sidebarCollapsed={$shellLayout.hostCollapsed}
    onToggleSidebar={$shellLayout.mode === 'phone' ? () => setHostRailCollapsed(!$shellLayout.hostCollapsed) : undefined}
    pendingCount={$navigation.pendingRequests.length}
    ramblePhase={visibleRamblePhase}
    rambleRequestTitle={$rambleSession.requestTitle}
    onWindowError={(message) => (pageError = tr('Window action failed: {error}', { error: message }))}
  >
    {#snippet workspaceTabs()}
      <WorkspaceTabStrip
        views={$workspaceShell.shell.views}
        activeViewKey={$workspaceShell.shell.activeViewKey}
        pendingViewKey={$workspaceShell.pendingViewKey}
        disabled={workspaceTransitionLocked}
        labelForView={workspaceTabLabel}
        onActivate={(viewKey) => void workspaceNavigation.activateWorkspaceTab(viewKey)}
        onClose={workspaceNavigation.closeWorkspaceTab}
        onReorder={workspaceNavigation.reorderWorkspaceTabs}
      />
    {/snippet}
  </AppTitlebar>

  <WorkbenchShell
    hostCollapsed={$shellLayout.hostCollapsed}
    requestCollapsed={$shellLayout.requestCollapsed}
    onHostCollapsedChange={setHostRailCollapsed}
    onRequestCollapsedChange={setRequestRailCollapsed}
    startupFailed={$startup.phase === 'failed'}
    requestPaneVisible={renderedWorkspaceSurface !== 'standalone'}
    mode={$shellLayout.mode}
    onModeChange={shellLayout.setMode}
    bind:hostDisplayWidth={hostRailDisplayWidth}
    bind:resizing={navigationResizing}
  >
    {#snippet hostRail()}
      <HostSessionRail
        collapsed={$shellLayout.hostCollapsed}
        onCollapsedChange={setHostRailCollapsed}
        sessions={$navigation.hostSessions}
        activeHostId={renderedWorkspaceView?.kind === 'session' ? renderedWorkspaceView.hostId : railAgentSession?.host_id ?? null}
        activeHostSessionId={renderedWorkspaceView?.kind === 'session' ? renderedWorkspaceView.hostSessionId : railAgentSession?.host_session_id ?? null}
        inboxActive={renderedWorkspaceView?.kind === 'inbox'}
        requestSearch={projectSearch}
        loading={$navigation.loadingNavigation}
        refreshing={$navigation.refreshingPage}
        {resolveHostProfile}
        onSelect={(hostId, hostSessionId) => {
          closePhoneDrawers()
          void workspaceNavigation.selectRailScope(hostId, hostSessionId)
        }}
        onRequestSearch={(search) => projectSearch = search}
        onSearchRequests={(search) => void workspaceNavigation.searchWorkspaceRequests(search)}
        onSetSessionPinned={(session, pinned) => navigation.setHostSessionPinned(session, pinned)}
        onArchiveSession={managedSessions.archiveSessionFromUi}
        onSettings={() => {
          closePhoneDrawers()
          void openSettings('general')
        }}
        onNewSession={previewMode ? undefined : (cwd) => {
          closePhoneDrawers()
          void managedSessions.openNewManagedSession(undefined, cwd)
        }}
      />
    {/snippet}

    {#snippet startupRecovery()}
      <StartupRecoveryPanel {capabilities} message={$startup.failureMessage} timedOut={$startup.failureTimedOut} bind:settingsOpen={$startup.settingsOpen} onRetry={() => void startup.start()} />
    {/snippet}

    {#snippet requestPane()}
      <RequestListPane
        collapsed={$shellLayout.requestCollapsed}
        onCollapsedChange={setRequestRailCollapsed}
        requests={$navigation.requests}
        activeRequestId={currentRequest?.request_id ?? null}
        cookingRequestIds={$cookingSession.cookingRequestIds}
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
        onOpenRequest={(requestId) => {
          closePhoneDrawers()
          void workspaceNavigation.openRequest(requestId)
        }}
        onFiltersChange={(filters) => void navigation.setRequestFilters(filters)}
        onClearSearch={() => void navigation.setRequestSearch('')}
      />
    {/snippet}

    {#snippet workspacePane()}
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
          <InboxWorkspaceView onNewSession={previewMode ? undefined : () => void managedSessions.openNewManagedSession()} />
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
            onChanged={startup.retrySessionViewRecovery}
            onDeleteManagedSession={managedSessions.deleteManagedSessionFromUi}
          />
        {:else if renderedWorkspaceView?.kind === 'settings'}
          <SettingsWorkspaceView
            transport={applicationTransport}
            {capabilities}
            section={settingsSection}
            sectionSelectionEpoch={settingsSectionSelectionEpoch}
            agentConfigId={settingsAgentConfigId}
            agentAdvanced={settingsAgentAdvanced}
            {updateInstallBlocked}
            onRestartOnboarding={restartOnboarding}
            onOpenArchived={() => void openArchivedSessions()}
            onOpenRambelleProfile={() => void openRambelleProfile()}
          />
        {:else if renderedWorkspaceView?.kind === 'request-task'}
          <TaskWorkspaceView
            agentStatus={rambleAgentSessionId ? requestAgentStatus : undefined}
            transport={applicationTransport}
            {capabilities}
            workspace={$workspaceSession.workspace}
            editorDocument={$draftSession.editorDocument}
            activeActionId={currentRequest
              ? draftOperations.activeActionId(currentRequest.request_id)
              : null}
            actionsDisabled={managedFeedbackReadOnly || workspaceTransitionLocked || $workspaceShell.pendingViewKey !== null}
            onSelectAction={selectAction}
            previews={$attachmentSession.previews}
            loading={$workspaceSession.loadingWorkspace}
            formatTime={formatTimeLocal}
            {resolveHostProfile}
            onToggleRamble={() => void toggleRamble()}
            ramblePhase={rambleBelongsToWorkspace ? visibleRamblePhase : 'idle'}
            rambleStartedOnce={rambleBelongsToWorkspace ? $rambleSession.startedOnce : false}
            rambleBusy={rambleBelongsToWorkspace ? rambleBusy : true}
            canSubmit={!managedFeedbackReadOnly}
            cookingEnabled={$cookingEnabled}
            {cookedDraftReady}
            cooking={currentRequestCooking}
            submitting={$workspaceSession.submitting}
            onSubmitFeedback={() => void submitFeedback()}
          />
        {:else if renderedAgentDraftView && renderedAgentDraftController}
          {#if $startup.mounted && $startup.phase === 'ready'}
          {#key renderedAgentDraftView.draftId}
            <DraftManagedSessionWorkspace transport={applicationTransport} controller={renderedAgentDraftController} draftId={renderedAgentDraftView.draftId}
              onConfigure={() => void openSettings('agents')}
              onConfigureAgent={(configId, advanced) => void openSettings('agents', configId, advanced)}
              onChooseDirectory={capabilities.serverPaths.status.availability === 'unavailable' ? undefined : () => capabilities.serverPaths.implementation.chooseDirectory()} />
          {/key}
          {/if}
        {:else if renderedAgentSessionView}
          {#if $startup.mounted && $startup.phase === 'ready'}
          {#key renderedAgentSessionView.sessionId}
            <ManagedSessionSection
              bind:this={managedSessionSection}
              transport={applicationTransport}
              sessionId={renderedAgentSessionView.sessionId}
              deletionPending={$managedSessions.deletingCommands.has(renderedAgentSessionView.sessionId)}
              onDeletingChange={managedSessions.observeDeletion}
              onConfigureAgent={(configId, advanced) => void openSettings('agents', configId, advanced)}
              onOpenRamble={renderedManagedSession ? async () => {
                if (renderedManagedSession) await workspaceNavigation.selectRailScope(renderedManagedSession.host_id, renderedManagedSession.host_session_id)
              } : undefined}
            />
          {/key}
          {/if}
        {:else if renderedWorkspaceView?.kind === 'rambelle-profile'}
          <RambelleProfileWorkspaceView />
        {:else if renderedSessionResolution?.kind === 'missing-session'}
          <MissingSessionView
            missing={renderedSessionResolution}
            label={sessionTabLabel(renderedSessionResolution.session)}
            busy={renderedSessionResolution.reason === 'unresolved' || $workspaceShell.pendingViewKey !== null}
            onRetry={startup.retrySessionViewRecovery}
            onClose={() => workspaceNavigation.closeWorkspaceTab(workspaceViewKey(renderedSessionResolution!.session))}
            onOpenArchive={() => void openArchivedSessions(renderedSessionResolution!.session)}
          />
        {:else if $startup.mounted}
          {#key renderedSessionView ? workspaceViewKey(renderedSessionView) : 'workspace:empty'}
          <SessionWorkbench
        agentStatus={rambleAgentSessionId ? requestAgentStatus : undefined}
        readOnly={managedFeedbackReadOnly}
        transport={applicationTransport}
        {capabilities}
        bind:this={sessionWorkbench}
        view={renderedSessionView}
        bind:taskBriefOpen
        loadingWorkspace={$workspaceSession.loadingWorkspace}
        workspace={$workspaceSession.workspace}
        {feedbackResult}
        draftBody={$draftSession.body}
        editorDocument={$draftSession.editorDocument}
        editorEpoch={$draftSession.editorEpoch}
        {tidyConfig}
        tidyAutoThreshold={$tidyAutoThreshold}
        activeActionId={currentRequest
          ? draftOperations.activeActionId(currentRequest.request_id)
          : null}
        savedRevision={$draftSession.savedRevision}
        savePhase={$draftSession.phase}
        attachmentPreviews={$attachmentSession.previews}
        dragActive={$attachmentSession.dragActive}
        rambelleStatusPortrait={rambleBelongsToWorkspace
          ? rambelleStatusPortrait
          : feedbackResult
            ? rambelleArchived
            : rambelleIdle}
        rambleEngaged={rambleBelongsToWorkspace ? rambleEngaged : false}
        rambleActive={rambleBelongsToWorkspace ? rambleActive : false}
        ramblePhase={rambleBelongsToWorkspace ? visibleRamblePhase : 'idle'}
        rambleBusy={rambleBelongsToWorkspace ? rambleBusy : true}
        rambleStartedOnce={rambleBelongsToWorkspace ? $rambleSession.startedOnce : false}
        voiceDevice={rambleBelongsToWorkspace ? $rambleSession.voiceDevice : ''}
        voiceChunkIndex={rambleBelongsToWorkspace ? $rambleSession.voiceChunkIndex : 0}
        voicePartial={rambleBelongsToWorkspace ? $rambleSession.voicePartial : ''}
        voiceLevel={rambleBelongsToWorkspace ? $rambleSession.voiceLevel : 0}
        voiceModelMissing={rambleBelongsToWorkspace ? $rambleSession.voiceModelMissing : false}
        rambleMessage={rambleBelongsToWorkspace ? $rambleSession.message : ''}
        attachmentBusy={rambleBelongsToWorkspace ? $attachmentSession.busy : false}
        {canSubmit}
        cooking={currentRequestCooking}
        cookingEnabled={$cookingEnabled}
        {cookedDraftReady}
        cookedPreviewModel={$cookingSession.preview?.model ?? ''}
        cookedPreviewMarkdown={$cookingSession.preview?.markdown ?? ''}
        onCookPreview={() => void cookPreviewOnly()}
        onRestoreOriginal={restoreOriginalAfterCook}
        submitting={$workspaceSession.submitting}
        submitStage={$workspaceSession.submitStage}
        publishedFeedback={$workspaceSession.publishedFeedback}
        {canCancel}
        cancelling={$workspaceSession.cancelling}
        approving={$workspaceSession.approving}
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
    {/snippet}
  </WorkbenchShell>

  {#if resumePrompt}
    <ResumePromptDialog
      prompt={resumePrompt}
      copyState={resumeCopyState}
      onCopy={() => void copyResumePrompt()}
      onDismiss={dismissResumePrompt}
    />
  {/if}
</main>

{#if onboardingAvailable && onboardingOpen}
  <OnboardingWizard {capabilities} transport={applicationTransport} bind:openWizard={onboardingOpen} onClose={closeOnboarding} onStartSession={startOnboardingSession} />
{/if}

{#if softwareUpdatesAvailable}
  <UpdateAvailableDialog
    softwareUpdates={capabilities.softwareUpdates}
    installBlocked={updateInstallBlocked}
    onOpenReleases={() => void openGithubReleases()}
  />
{/if}
{/key}
