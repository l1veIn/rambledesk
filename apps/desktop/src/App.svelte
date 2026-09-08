<script lang="ts">
  import { diagnosticErrorCategory, startClientDiagnostic } from '$lib/diagnostics/clientDiagnostics'
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
  import type { ShellMode } from './lib/workbench/shellMode'
  import InboxWorkspaceView from './lib/workspace/InboxWorkspaceView.svelte'
  import MissingSessionView from './lib/workspace/MissingSessionView.svelte'
  import RambelleProfileWorkspaceView from './lib/workspace/RambelleProfileWorkspaceView.svelte'
  import SettingsWorkspaceView from './lib/workspace/SettingsWorkspaceView.svelte'
  import TaskWorkspaceView from './lib/workspace/TaskWorkspaceView.svelte'
  import ArchivedSessionsWorkspaceView from './lib/workspace/ArchivedSessionsWorkspaceView.svelte'
  import ManagedSessionSection from './lib/agents/ManagedSessionSection.svelte'
  import ManagedFeedbackRequestStatus from './lib/agents/ManagedFeedbackRequestStatus.svelte'
  import DraftManagedSessionWorkspace from './lib/agents/DraftManagedSessionWorkspace.svelte'
  import { createDraftManagedSessionController, type DraftManagedSessionController } from './lib/agents/draftManagedSessionController'
  import { createManagedSessionDraftStorage } from './lib/agents/managedSessionDrafts'
  import { removeArchivedManagedSessionView } from './lib/agents/managedSessionArchive'
  import { agentText } from './lib/agents/agentI18n'
  import { deleteSessionRecord, removeManagedSessionViews } from './lib/agents/managedSessionDeletion'
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
    type ActiveAction,
    type DraftOperation,
  } from './lib/draftOperations'
  import { writeBackgroundDraftOperation } from './lib/backgroundDraftWriter'
  import {
    notificationStateForPermission,
    type NotificationState,
  } from './lib/notifications'
  import {
    agentDraftViewDescriptor,
    agentSessionViewDescriptor,
    archiveViewDescriptor,
    inboxViewDescriptor,
    rambelleProfileViewDescriptor,
    requestTaskViewDescriptor,
    sessionViewDescriptor,
    settingsViewDescriptor,
    workspaceViewKey,
    type AgentSessionViewDescriptor,
    type SessionViewDescriptor,
    type WorkspaceViewDescriptor,
  } from './lib/workspace/viewDescriptors'
  import { activeWorkspaceView, workspaceShellReducer } from './lib/workspace/workspaceShell'
  import { updateTaskTabTitles } from './lib/workspace/taskTabTitles'
  import { agentSessionForView, agentViewForEmptyRamble, agentViewForRequest, arrivingRequestForAgentView } from './lib/workspace/agentViewRouting'
  import { seedPreviewWorkspaceScenario } from './lib/workspace/previewWorkspaceSnapshot'
  import type { SessionViewResolution } from './lib/workspace/sessionViewRecovery'
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
    normalizePublishedFeedback,
    type PublishedFeedbackView,
  } from './lib/publishedFeedback'
  import { formatTime, messageFrom } from './lib/workbench/feedbackText'
  import { createCookingController } from './lib/workbench/cookingController'
  import { createDraftController } from './lib/workbench/draftController'
  import { createDraftSession } from './lib/workbench/draftSession'
  import { createSubmissionController } from './lib/workbench/submissionController'
  import { createAttachmentSession } from './lib/workbench/attachmentSession'
  import { createStartupController, type StartupController } from './lib/workbench/startupController'
  import { createRambleSession } from './lib/workbench/rambleSession'
  import { createWorkspaceSession } from './lib/workbench/workspaceSession'
  import { createWorkspaceShellSession } from './lib/workbench/workspaceShellSession'
  import { createPublisherController } from './lib/workbench/publisherController'
  import { buildResumePrompt, shouldShowResumePromptButton } from './lib/workbench/resumePrompt'
  import {
    createAttachmentController,
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
    initialWebAccessAutostart,
    initialWebAccessPort,
    saveHostRailCollapsed,
    saveRequestRailCollapsed,
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
  const rambleSession = createRambleSession()
  /** The open request, projected from the session so field reads stay short. */
  $: currentRequest = $workspaceSession.workspace?.request ?? null
  let taskTabTitles: ReadonlyMap<string, string> = new Map()
  let renderedWorkspaceView: WorkspaceViewDescriptor | null = null
  let renderedSessionView: SessionViewDescriptor | null = null
  let renderedSessionResolution: SessionViewResolution | null = null
  const managedDraftStorage = createManagedSessionDraftStorage(typeof localStorage === 'undefined' ? undefined : localStorage)
  const managedDraftControllers = new Map<string, DraftManagedSessionController>()
  const promotedManagedDrafts = new Map<string, string>()
  const closingManagedDrafts = new Set<string>()
  let deletingSessionCommands = new Set<string>()
  let deletingManagedSessionIds = new Set<string>()
  let pageError = ''
  let cookingRequestIds = new Set<string>()
  /** Preview cooking result for the current workspace, if generated and current. */
  let cookedPreview: { markdown: string; original: string; model: string } | null = null
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
  let hostRailPreference = initialHostRailCollapsed()
  let requestRailPreference = initialRequestRailCollapsed()
  let phoneHostRailOpen = false
  let phoneRequestRailOpen = false
  let shellMode: ShellMode = 'desktop'
  let hostRailDisplayWidth = 0
  let navigationResizing = false
  let projectSearch = ''
  let rambleDocumentQueue: Promise<void> = Promise.resolve()
  $: tidyConfig = {
    provider: $tidyProvider,
    apiKey: $tidyApiKey,
    baseUrl: $tidyBaseUrl,
    model: $tidyModel,
    reasoningEffort: $tidyReasoningEffort,
    locale: $locale,
    systemPrompt: $tidySystemPrompt,
  }
  let activeActionByRequest = new Map<string, NonNullable<ActiveAction>>()
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

  // `startup` is assigned below; these callbacks only run after composition finishes.
  let startup: StartupController
  const workspaceTransition = createWorkspaceTransition<LoadedWorkspaceTarget>({
    saveCurrent: saveDraftNow,
    unmountCurrent: () => {
      startup.patch({ mounted: false })
      sessionWorkbench = undefined
      workspaceSession.setLoading(true)
    },
    loadTarget: loadWorkspaceTarget,
    commitTarget: commitWorkspaceTarget,
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
    openRequest,
    clearWorkspace,
    onPageError: (message) => (pageError = message),
    canSendOsBanners: () => isMac,
    onRequestsArrived: (requests) => { void autoOpenArrivingRequest(requests) },
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
    selectAgentNavigationScope,
    requestIdForSession,
    onReady: () => {
      inboxTimer = ensureDesktopNavigationPolling(
        desktopShellAvailable,
        inboxTimer,
        setInterval,
        () => void navigation.refreshNavigation(true),
      )
    },
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
  $: renderedAgentDraftController = renderedAgentDraftView ? managedDraftController(renderedAgentDraftView.draftId) : null
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
  $: managedFeedbackReadOnly = !!feedbackManagedSessionId && (deletingSessionCommands.has(feedbackManagedSessionId) || deletingManagedSessionIds.has(feedbackManagedSessionId))
  $: currentRequestCooking =
    currentRequest !== null && cookingRequestIds.has(currentRequest.request_id)
  $: cookedDraftReady = cookedPreview !== null
  // Turning Cooking off discards any pending cooked preview: submitting then
  // publishes the editor content as-is.
  $: if (!$cookingEnabled) cookedPreview = null
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
  // Phones turn the rails into drawers: `collapsed` then means "drawer closed" and the
  // persisted preference is left untouched for when the viewport widens again.
  $: hostSessionRailCollapsed = shellMode === 'phone' ? !phoneHostRailOpen : hostRailPreference
  $: requestRailCollapsed = shellMode === 'phone' ? !phoneRequestRailOpen : requestRailPreference
  $: saveHostRailCollapsed(hostRailPreference)
  $: saveRequestRailCollapsed(requestRailPreference)

  function setHostRailCollapsed(collapsed: boolean) {
    if (shellMode === 'phone') {
      phoneHostRailOpen = !collapsed
      if (!collapsed) phoneRequestRailOpen = false
    } else {
      hostRailPreference = collapsed
    }
  }

  function setRequestRailCollapsed(collapsed: boolean) {
    if (shellMode === 'phone') {
      phoneRequestRailOpen = !collapsed
      if (!collapsed) phoneHostRailOpen = false
    } else {
      requestRailPreference = collapsed
    }
  }

  function closePhoneDrawers() {
    if (shellMode !== 'phone') return
    phoneHostRailOpen = false
    phoneRequestRailOpen = false
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
          openLoadedWorkspaceView(previewFixtures.workspace)
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
      for (const controller of managedDraftControllers.values()) void controller.close().catch(() => {})
      managedDraftControllers.clear()
    }
  })

  async function startOnboardingSession(configId?: string) {
    if (!await startup.start()) {
      throw new Error($locale === 'zh-CN' ? '初始化未完成，请检查连接后重试。' : 'Initialization did not finish. Check the connection and retry.')
    }
    if (!await openNewManagedSession(configId)) {
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

  function openSessionView(view: SessionViewDescriptor, requestId?: string) {
    if (requestId) workspaceShell.bindRequest(workspaceViewKey(view), requestId)
    workspaceShell.dispatch({ type: 'open', view })
  }

  function reorderWorkspaceTabs(viewKeys: readonly string[]) {
    workspaceShell.dispatch({ type: 'reorder', viewKeys })
  }

  function openLoadedWorkspaceView(next: FeedbackWorkspaceView) {
    openSessionView(
      sessionViewDescriptor(next.request.host_id, next.request.host_session_id),
      next.request.request_id,
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

  function requestIdForSession(
    view: SessionViewDescriptor,
    requests: readonly FeedbackRequestSummary[],
  ): string | null {
    const rememberedRequestId = workspaceShell.requestIdFor(view)
    // List filters may hide an open request; they must not reset its workspace tab.
    if (rememberedRequestId && (
      requestFilterCount($navigation.requestFilters) > 0 ||
      requests.some((request) => request.request_id === rememberedRequestId)
    )) return rememberedRequestId
    return requests[0]?.request_id ?? null
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
    outcome: WorkspaceTransitionOutcome,
  ) {
    if (outcome !== 'blocked' && outcome !== 'failed') return
    await navigation.selectScope(scope.hostId, scope.hostSessionId)
  }

  async function selectRailScope(hostId: string | null, hostSessionId: string | null) {
    if (workspaceTransitionLocked) return
    const priorScope = currentNavigationScope()
    const intent = workspaceTransition.invalidate()
    const selection = await navigation.selectScope(hostId, hostSessionId)
    if (!workspaceTransition.isCurrent(intent) || !selection.selected) return
    if (workspaceTransitionLocked) { await restoreNavigationScope(priorScope, 'blocked'); return }
    if (!hostId || !hostSessionId) {
      const outcome = await workspaceTransition.activate({
        view: inboxViewDescriptor(),
        requestId: null,
        shellAction: { type: 'open' },
        pendingViewKey: workspaceViewKey(inboxViewDescriptor()),
      }, intent)
      await restoreNavigationScope(priorScope, outcome)
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
    }, intent)
    await restoreNavigationScope(priorScope, outcome)
  }

  async function activateWorkspaceTab(viewKey: string) {
    if (workspaceTransitionLocked || $workspaceShell.shell.activeViewKey === viewKey) return
    const priorScope = currentNavigationScope()
    const intent = workspaceTransition.invalidate()
    const view = $workspaceShell.shell.views.find(
      (candidate) => workspaceViewKey(candidate) === viewKey,
    )
    if (!view) return
    if (view.kind !== 'session') {
      if (view.kind === 'inbox') {
        const selection = await navigation.selectScope(null, null)
        if (!selection.selected) return
      }
      if (view.kind === 'agent-session') {
        const selection = await selectAgentNavigationScope(view)
        if (!selection.selected) return
      }
      if (!workspaceTransition.isCurrent(intent)) return
      if (workspaceTransitionLocked) { await restoreNavigationScope(priorScope, 'blocked'); return }
      const outcome = await workspaceTransition.activate({
        view,
        requestId: view.kind === 'request-task' ? view.requestId : null,
        shellAction: { type: 'open' },
        pendingViewKey: viewKey,
      }, intent)
      if (view.kind === 'inbox' || view.kind === 'agent-session') await restoreNavigationScope(priorScope, outcome)
      return
    }
    const resolution = startup.resolutionFor(viewKey)
    if (resolution?.kind === 'missing-session') {
      const outcome = await workspaceTransition.activate({
        view,
        requestId: null,
        shellAction: { type: 'open' },
        pendingViewKey: viewKey,
      }, intent)
      if (outcome === 'activated') await navigation.selectScope(null, null)
      return
    }

    const selection = await navigation.selectScope(view.hostId, view.hostSessionId)
    if (!workspaceTransition.isCurrent(intent) || !selection.selected) return
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
    }, intent)
    await restoreNavigationScope(priorScope, outcome)
  }

  async function closeWorkspaceTab(viewKey: string) {
    if (workspaceTransitionLocked || $workspaceShell.pendingViewKey) return
    const closingView = $workspaceShell.shell.views.find((view) => workspaceViewKey(view) === viewKey)
    if (closingView?.kind === 'agent-draft') {
      if (closingManagedDrafts.has(closingView.draftId)) return
      closingManagedDrafts.add(closingView.draftId)
      try {
        const promotedSessionId = await managedDraftController(closingView.draftId).close()
        managedDraftControllers.delete(closingView.draftId)
        if (promotedSessionId) viewKey = workspaceViewKey(agentSessionViewDescriptor(promotedSessionId))
      } catch (cause) {
        toast.error(messageFrom(cause))
        return
      } finally { closingManagedDrafts.delete(closingView.draftId) }
      // Another tab activation may have started while cleanup awaited the agent.
      // Its pending target owns the next mount; only remove the closed descriptor.
      if ($workspaceShell.pendingViewKey) {
        workspaceShell.dispatch({ type: 'close', viewKey })
        return
      }
    }
    const closingActive = $workspaceShell.shell.activeViewKey === viewKey
    if (!closingActive) {
      workspaceShell.dispatch({ type: 'close', viewKey })
      workspaceShell.forgetRequest(viewKey)
      return
    }
    const intent = workspaceTransition.invalidate()
    const priorScope = currentNavigationScope()

    const nextShellState = workspaceShellReducer($workspaceShell.shell, {
      type: 'close',
      viewKey,
    })
    const fallbackView = activeWorkspaceView(nextShellState)
    const fallbackResolution = fallbackView?.kind === 'session'
      ? startup.resolutionFor(workspaceViewKey(fallbackView))
      : null
    let fallbackRequestId: string | null = null
    if (fallbackView?.kind === 'session' && fallbackResolution?.kind !== 'missing-session') {
      const selection = await navigation.selectScope(
        fallbackView.hostId,
        fallbackView.hostSessionId,
      )
      if (!workspaceTransition.isCurrent(intent) || !selection.selected) return
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
    } else if (fallbackView?.kind === 'agent-session') {
      const selection = await selectAgentNavigationScope(fallbackView)
      if (!selection.selected) return
    }

    if (!workspaceTransition.isCurrent(intent)) return
    if (workspaceTransitionLocked) { await restoreNavigationScope(priorScope, 'blocked'); return }
    const outcome = await workspaceTransition.activate({
      view: fallbackView,
      requestId: fallbackRequestId,
      shellAction: { type: 'close', viewKey },
      pendingViewKey: viewKey,
    }, intent)
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
      : currentRequest?.request_id === requestId
        ? sessionViewDescriptor(currentRequest.host_id, currentRequest.host_session_id)
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
        : await readApplicationSnapshot(applicationTransport, 'getFeedbackWorkspace', {
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
                await readApplicationSnapshot(applicationTransport, 'readPublishedFeedback', {
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
    const previousActiveView = activeWorkspaceView($workspaceShell.shell)
    const requestedView = loaded?.kind === 'session'
      ? sessionViewDescriptor(
          loaded.workspace.request.host_id,
          loaded.workspace.request.host_session_id,
        )
      : loaded?.kind === 'request-task'
        ? requestTaskViewDescriptor(loaded.workspace.request.request_id)
        : target.view
    const promotedSessionId = requestedView?.kind === 'agent-draft' ? promotedManagedDrafts.get(requestedView.draftId) : undefined
    const loadedView = promotedSessionId ? agentSessionViewDescriptor(promotedSessionId) : requestedView
    if (target.shellAction.type === 'open' && !loadedView) {
      throw new Error(tr('This feedback request could not be found.'))
    }

    const nextShellState =
      target.shellAction.type === 'close'
        ? workspaceShellReducer($workspaceShell.shell, target.shellAction)
        : workspaceShellReducer($workspaceShell.shell, { type: 'open', view: loadedView! })

    if (loaded?.kind === 'session') {
      attachmentController.releasePreviews()
      workspaceSession.open(loaded.workspace, loaded.publishedFeedback)
      cookedPreview = null
      draftSession.adopt(loaded.workspace.draft)
      attachmentSession.setMessage('')
      workspaceShell.bindRequest(workspaceViewKey(loadedView!), loaded.workspace.request.request_id)
      if (target.shellAction.type === 'close') workspaceShell.forgetRequest(target.shellAction.viewKey)
    } else if (loaded?.kind === 'request-task') {
      attachmentController.releasePreviews()
      workspaceSession.open(loaded.workspace)
      cookedPreview = null
      draftSession.adopt(loaded.workspace.draft)
      attachmentSession.setMessage('')
      if (target.shellAction.type === 'close') workspaceShell.forgetRequest(target.shellAction.viewKey)
    } else {
      clearWorkspace()
      if (target.shellAction.type === 'close') workspaceShell.forgetRequest(target.shellAction.viewKey)
    }

    workspaceShell.replaceShell(nextShellState)
    startup.patch({ mounted: true })
    workspaceSession.setLoading(false)
    if (leavesSettingsView(previousActiveView, activeWorkspaceView(nextShellState))) {
      void refreshNotificationPermission()
    }
    if (loaded) void attachmentController.refreshPreviews(loaded.workspace)
  }

  async function autoOpenArrivingRequest(arrivals: readonly FeedbackRequestSummary[]) {
    const origin = activeWorkspaceView($workspaceShell.shell)
    if (origin?.kind !== 'agent-session' || $workspaceShell.pendingViewKey) return
    const intent = workspaceTransition.currentIntent()
    // Draft promotion can replace the active view in the same update as the first request.
    await tick()
    if (!workspaceTransition.isCurrent(intent) || $workspaceShell.pendingViewKey) return
    const canLeave = () => {
      const current = activeWorkspaceView($workspaceShell.shell)
      return current?.kind === 'agent-session' && current.sessionId === origin.sessionId
        && $startup.phase === 'ready' && !onboardingOpen && !resumePrompt
        && !workspaceTransitionLocked && !rambleEngaged
        && managedSessionSection?.canAutoOpenRamble(origin.sessionId) === true
    }
    const request = arrivingRequestForAgentView(origin, arrivals, canLeave())
    if (request) await activateRequest(request.request_id, canLeave)
  }

  async function activateRequest(
    requestId: string,
    canLeaveCurrent: () => boolean = () => true,
  ): Promise<WorkspaceTransitionOutcome> {
    if (workspaceTransitionLocked || !canLeaveCurrent()) return 'blocked'
    const priorScope = currentNavigationScope()
    const intent = workspaceTransition.invalidate()
    if (
      currentRequest?.request_id === requestId &&
      renderedWorkspaceView?.kind === 'session'
    ) {
      openLoadedWorkspaceView($workspaceSession.workspace!)
      return 'activated'
    }
    pageError = ''
    const view = viewForRequest(requestId)
    // An exact request can load while its already-selected request list is refreshing.
    if (view && (view.hostId !== priorScope.hostId || view.hostSessionId !== priorScope.hostSessionId)) {
      const selection = await navigation.selectScope(view.hostId, view.hostSessionId)
      if (!workspaceTransition.isCurrent(intent)) return 'stale'
      if (!selection.selected) return 'failed'
    }
    if (workspaceTransitionLocked || !canLeaveCurrent()) { await restoreNavigationScope(priorScope, 'blocked'); return 'blocked' }
    const outcome = await workspaceTransition.activate({
      view,
      requestId,
      shellAction: { type: 'open' },
      pendingViewKey: view ? workspaceViewKey(view) : `request:${JSON.stringify(requestId)}`,
    }, intent, canLeaveCurrent)
    await restoreNavigationScope(priorScope, outcome)
    return outcome
  }

  async function openRequest(requestId: string, _saveCurrent = true): Promise<boolean> {
    return (await activateRequest(requestId)) === 'activated'
  }

  async function searchWorkspaceRequests(search: string) {
    if (workspaceTransitionLocked) return
    const intent = workspaceTransition.invalidate()
    await navigation.setRequestSearch(search)
    if (!workspaceTransition.isCurrent(intent) || workspaceTransitionLocked) return
    await selectRailScope(null, null)
  }

  async function selectAgentNavigationScope(view: AgentSessionViewDescriptor) {
    const session = agentSessionForView(view, $navigation.hostSessions)
    return navigation.selectScope(session?.host_id ?? null, session?.host_session_id ?? null)
  }

  async function openAgentSession(sessionId: string) {
    if (workspaceTransitionLocked) return
    const view = agentSessionViewDescriptor(sessionId)
    const priorScope = currentNavigationScope()
    const intent = workspaceTransition.invalidate()
    const selection = await selectAgentNavigationScope(view)
    if (!workspaceTransition.isCurrent(intent) || !selection.selected) return
    if (workspaceTransitionLocked) { await restoreNavigationScope(priorScope, 'blocked'); return }
    const outcome = await workspaceTransition.activate({
      view,
      requestId: null,
      shellAction: { type: 'open' },
      pendingViewKey: workspaceViewKey(view),
    }, intent)
    await restoreNavigationScope(priorScope, outcome)
  }

  async function openNewManagedSession(configId?: string, cwd = ''): Promise<boolean> {
    const finish = startClientDiagnostic('session_navigation', { action: 'open', source: onboardingOpen ? 'onboarding' : 'workbench', selected: !!configId })
    if (workspaceTransitionLocked || previewMode) { finish('blocked', { reason: previewMode ? 'unsupported' : 'in_flight' }); return false }
    try {
    const view = agentDraftViewDescriptor(crypto.randomUUID())
    const recent = managedDraftStorage.load(view.draftId)
    managedDraftStorage.save(view.draftId, {
      choice: configId ? `config:${configId}` : recent.choice,
      cwd,
      text: '',
    })
    workspaceTransition.invalidate()
    const outcome = await workspaceTransition.activate({ view, requestId: null, shellAction: { type: 'open' }, pendingViewKey: workspaceViewKey(view) })
    if (outcome !== 'activated') managedDraftStorage.remove(view.draftId)
    finish(outcome === 'activated' ? 'ok' : outcome === 'failed' ? 'failed' : outcome === 'stale' ? 'cancelled' : 'blocked',
      outcome === 'activated' ? {} : { reason: outcome === 'stale' ? 'stale' : outcome === 'blocked' ? 'in_flight' : 'activation_failed' })
    return outcome === 'activated'
    } catch (cause) { finish('failed', { error_category: diagnosticErrorCategory(cause) }); throw cause }
  }

  function managedDraftController(draftId: string): DraftManagedSessionController {
    let controller = managedDraftControllers.get(draftId)
    if (!controller) {
      controller = createDraftManagedSessionController(applicationTransport, draftId, managedDraftStorage,
        (snapshot) => managedDraftPromoted(draftId, snapshot))
      managedDraftControllers.set(draftId, controller)
    }
    return controller
  }

  async function managedDraftPromoted(draftId: string, snapshot: ManagedSessionSnapshot) {
    promotedManagedDrafts.set(draftId, snapshot.session.session_id)
    const view = agentSessionViewDescriptor(snapshot.session.session_id)
    workspaceShell.dispatch({
      type: 'replace', viewKey: workspaceViewKey(agentDraftViewDescriptor(draftId)), view,
    })
    managedDraftControllers.delete(draftId)
    const intent = workspaceTransition.currentIntent()
    await navigation.refreshNavigation(true)
    if (workspaceTransition.isCurrent(intent) && $workspaceShell.shell.activeViewKey === workspaceViewKey(view)) await selectAgentNavigationScope(view)
  }

  function observeManagedDeletion(sessionId: string, deleting: boolean) {
    const next = new Set(deletingManagedSessionIds)
    if (deleting) next.add(sessionId)
    else next.delete(sessionId)
    deletingManagedSessionIds = next
  }

  async function archiveSessionFromUi(session: HostSessionSummary): Promise<void> {
    const finish = startClientDiagnostic('session_archive', { source: 'workbench', action: 'archive', management: session.management.kind })
    try {
    if (!await navigation.archiveHostSession(session)) { finish('blocked', { reason: 'not_ready' }); return }
    if (session.management.kind !== 'managed') { finish('ok'); return }
    const key = workspaceViewKey(agentSessionViewDescriptor(session.session_id))
    const archived = removeArchivedManagedSessionView($workspaceShell.shell, session.session_id, $workspaceShell.pendingViewKey)
    if (archived.shouldInvalidatePending) workspaceTransition.invalidate()
    workspaceShell.replaceShell(archived.shell)
    workspaceShell.forgetRequest(key)
    if (archived.shouldNavigateToArchive) await openArchivedSessions(sessionViewDescriptor(session.host_id, session.host_session_id))
    finish('ok')
    } catch (cause) { finish('failed', { error_category: diagnosticErrorCategory(cause) }); throw cause }
  }

  async function deleteManagedSessionFromUi(session: HostSessionSummary) {
    if (session.management.kind !== 'managed' || deletingSessionCommands.has(session.session_id)) return
    const finish = startClientDiagnostic('session_delete', { source: 'workbench', action: 'delete', management: 'managed' })
    deletingSessionCommands = new Set([...deletingSessionCommands, session.session_id])
    try {
      const ownsFeedback = () => currentRequest?.managed_session_id === session.session_id
      if (ownsFeedback() && rambleBelongsToWorkspace && rambleCanExit) await exitRamble()
      await deleteSessionRecord(applicationTransport, session)
      const viewKey = workspaceViewKey(sessionViewDescriptor(session.host_id, session.host_session_id))
      const requestId = ownsFeedback() ? currentRequest!.request_id : null
      const rememberedRequestId = workspaceShell.requestIdFor(viewKey)
      const knownRequestIds = [...new Set([
        requestId, rememberedRequestId,
        ...[...$navigation.requests, ...$navigation.pendingRequests]
          .filter((request) => request.managed_session_id === session.session_id)
          .map((request) => request.request_id),
      ].filter((id): id is string => !!id))]
      const cleanup = removeManagedSessionViews($workspaceShell.shell, session, knownRequestIds)
      const closedActive = cleanup.closedActive || ownsFeedback()
      if ($workspaceShell.pendingViewKey && cleanup.closedViewKeys.includes($workspaceShell.pendingViewKey)) workspaceTransition.invalidate()
      if (closedActive) {
        workspaceTransition.invalidate()
        draftController.cancelPendingSave()
        clearWorkspace()
        cookedPreview = null
      }
      workspaceShell.replaceShell(cleanup.shell)
      if (closedActive) workspaceShell.dispatch({ type: 'open', view: inboxViewDescriptor() })
      workspaceShell.forgetRequest(viewKey)
      observeManagedDeletion(session.session_id, false)
      if (closedActive || ($navigation.selectedHostId === session.host_id && $navigation.selectedHostSessionId === session.host_session_id)) await navigation.selectScope(null, null)
      await navigation.refreshNavigation(true)
      finish('ok')
    } catch (cause) {
      finish('failed', { error_category: diagnosticErrorCategory(cause) })
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
      const foregroundWorkspace = $workspaceSession.workspace
      if (shouldUseForegroundDraftEditor({
        activeView: activeWorkspaceView($workspaceShell.shell),
        workbenchMounted: $startup.mounted,
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
          throw new Error($draftSession.message || tr('The current draft could not be saved.'))
        }
        return
      }

      const savedDraft = await writeBackgroundDraftOperation(requestId, operation, {
        load: async () => {
          const target = previewMode
            ? previewWorkspaceFor(requestId)
            : await readApplicationSnapshot(applicationTransport, 'getFeedbackWorkspace', {
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
          activeWorkspaceView($workspaceShell.shell),
          currentRequest?.request_id ?? null,
          requestId,
        ) &&
        $workspaceSession.workspace
      ) {
        workspaceSession.setDraft(savedDraft)
        draftSession.adopt(savedDraft)
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
    const requestId = currentRequest?.request_id
    if (
      !requestId ||
      workspaceTransitionLocked ||
      $workspaceShell.pendingViewKey !== null ||
      currentRequest?.status === 'completed' ||
      currentRequest?.status === 'cancelled'
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

  async function openSettings(section: SettingsSection, agentConfigId?: string, agentAdvanced = false) {
    if ($startup.phase === 'failed') {
      startup.patch({ settingsOpen: true })
      return
    }
    settingsSection = section
    settingsAgentConfigId = agentConfigId
    settingsAgentAdvanced = agentAdvanced
    settingsSectionSelectionEpoch += 1
    const view = settingsViewDescriptor()
    const viewKey = workspaceViewKey(view)
    if (workspaceTransitionLocked || $workspaceShell.pendingViewKey) return
    if ($workspaceShell.shell.activeViewKey !== viewKey) {
      workspaceTransition.invalidate()
      const outcome = await workspaceTransition.activate({
        view,
        requestId: null,
        shellAction: { type: 'open' },
        pendingViewKey: viewKey,
      })
      if (outcome !== 'activated') return
    }
  }

  async function openTaskWorkspace(requestId: string) {
    if (workspaceTransitionLocked || $workspaceShell.pendingViewKey) return
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
    if (workspaceTransitionLocked || $workspaceShell.pendingViewKey) return
    const view = rambelleProfileViewDescriptor()
    if ($workspaceShell.shell.activeViewKey === workspaceViewKey(view)) return
    workspaceTransition.invalidate()
    await workspaceTransition.activate({
      view,
      requestId: null,
      shellAction: { type: 'open' },
      pendingViewKey: workspaceViewKey(view),
    })
  }

  async function openArchivedSessions(initialSession: SessionViewDescriptor | null = null) {
    if (workspaceTransitionLocked || $workspaceShell.pendingViewKey) return
    archivedInitialSession = initialSession
    archivedSelectionEpoch += 1
    const view = archiveViewDescriptor()
    const viewKey = workspaceViewKey(view)
    if ($workspaceShell.shell.activeViewKey === viewKey) return
    workspaceTransition.invalidate()
    await workspaceTransition.activate({
      view,
      requestId: null,
      shellAction: { type: 'open' },
      pendingViewKey: viewKey,
    })
  }

  function applyWorkspaceMutation(next: FeedbackWorkspaceView) {
    workspaceSession.replace(next)
    if (draftSession.reconcile(next.draft) === 'kept-local') draftController.scheduleSave()
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
    getPreview: () => cookedPreview,
    setPreview: (preview) => {
      cookedPreview = preview
    },
    setCooking: setCookingRequest,
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
    await rambleDocumentQueue.catch(() => {})
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
        onOpenAgent={() => feedbackManagedSessionId && void openAgentSession(feedbackManagedSessionId)}
        onDeletingChange={observeManagedDeletion} />
    {/key}
  {:else if rambleAgentSessionId}
    <div class="flex items-center justify-between gap-2 text-xs">
      <span class="text-muted-foreground">ACP</span>
      <Button size="sm" variant="ghost" disabled={workspaceTransitionLocked || $workspaceShell.pendingViewKey !== null} onclick={() => rambleAgentSessionId && void openAgentSession(rambleAgentSessionId)}>{$locale === 'zh-CN' ? '查看 Agent' : 'View Agent'}</Button>
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
      if (await openRequest(requestId)) {
        await tick()
        if (segmentId) highlightSpeechSegment(document, segmentId, true)
      }
    }}
  />

  <AppTitlebar
    windowControls={capabilities.windowControls}
    sidebarCollapsed={hostSessionRailCollapsed}
    onToggleSidebar={shellMode === 'phone' ? () => setHostRailCollapsed(!hostSessionRailCollapsed) : undefined}
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
        onActivate={(viewKey) => void activateWorkspaceTab(viewKey)}
        onClose={closeWorkspaceTab}
        onReorder={reorderWorkspaceTabs}
      />
    {/snippet}
  </AppTitlebar>

  <WorkbenchShell
    hostCollapsed={hostSessionRailCollapsed}
    requestCollapsed={requestRailCollapsed}
    onHostCollapsedChange={setHostRailCollapsed}
    onRequestCollapsedChange={setRequestRailCollapsed}
    startupFailed={$startup.phase === 'failed'}
    requestPaneVisible={renderedWorkspaceSurface !== 'standalone'}
    bind:mode={shellMode}
    bind:hostDisplayWidth={hostRailDisplayWidth}
    bind:resizing={navigationResizing}
  >
    {#snippet hostRail()}
      <HostSessionRail
        collapsed={hostSessionRailCollapsed}
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
          void selectRailScope(hostId, hostSessionId)
        }}
        onRequestSearch={(search) => projectSearch = search}
        onSearchRequests={(search) => void searchWorkspaceRequests(search)}
        onSetSessionPinned={(session, pinned) => navigation.setHostSessionPinned(session, pinned)}
        onArchiveSession={archiveSessionFromUi}
        onSettings={() => {
          closePhoneDrawers()
          void openSettings('general')
        }}
        onNewSession={previewMode ? undefined : (cwd) => {
          closePhoneDrawers()
          void openNewManagedSession(undefined, cwd)
        }}
      />
    {/snippet}

    {#snippet startupRecovery()}
      <StartupRecoveryPanel {capabilities} message={$startup.failureMessage} timedOut={$startup.failureTimedOut} bind:settingsOpen={$startup.settingsOpen} onRetry={() => void startup.start()} />
    {/snippet}

    {#snippet requestPane()}
      <RequestListPane
        collapsed={requestRailCollapsed}
        onCollapsedChange={setRequestRailCollapsed}
        requests={$navigation.requests}
        activeRequestId={currentRequest?.request_id ?? null}
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
        onOpenRequest={(requestId) => {
          closePhoneDrawers()
          void openRequest(requestId)
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
          <InboxWorkspaceView onNewSession={previewMode ? undefined : () => void openNewManagedSession()} />
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
            onDeleteManagedSession={deleteManagedSessionFromUi}
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
              ? activeActionByRequest.get(currentRequest.request_id)?.actionId ?? null
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
              deletionPending={deletingSessionCommands.has(renderedAgentSessionView.sessionId)}
              onDeletingChange={observeManagedDeletion}
              onConfigureAgent={(configId, advanced) => void openSettings('agents', configId, advanced)}
              onOpenRamble={renderedManagedSession ? async () => {
                if (renderedManagedSession) await selectRailScope(renderedManagedSession.host_id, renderedManagedSession.host_session_id)
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
            onClose={() => closeWorkspaceTab(workspaceViewKey(renderedSessionResolution!.session))}
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
          ? activeActionByRequest.get(currentRequest.request_id)?.actionId ?? null
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
        cookedPreviewModel={cookedPreview?.model ?? ''}
        cookedPreviewMarkdown={cookedPreview?.markdown ?? ''}
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
