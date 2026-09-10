<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { initializeAppearance } from './lib/appearance/appearanceRuntime'

  import rambelleArchived from './assets/rambelle-states/archived.webp'
  import rambelleIdle from './assets/rambelle-states/idle.webp'
  import rambelleOrganizing from './assets/rambelle-states/organizing.webp'
  import rambelleRecording from './assets/rambelle-states/recording.webp'
  import AppTitlebar from './lib/shell/AppTitlebar.svelte'
  import OnboardingWizard from './lib/onboarding/OnboardingWizard.svelte'
  import UpdateAvailableDialog from './lib/updates/UpdateAvailableDialog.svelte'
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
  import { Button } from './lib/components/ui/button'
  import type { ApplicationTransport } from './lib/application/applicationTransport'
  import type { WorkbenchCapabilities } from './lib/capabilities/workbenchCapabilities'
  import { provideWorkbenchCapabilities } from './lib/capabilities/capabilityContext'
  import { createUnavailableWorkbenchCapabilities } from './lib/capabilities/unavailableCapabilities'
  import type { PublishedFeedbackAction } from './lib/publishedFeedbackAction'
  import { APPLICATION_EVENTS_STREAM } from './lib/application/applicationEvents'
  import { readApplicationSnapshot } from './lib/application/readApplicationSnapshot'


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

  import type { FeedbackWorkspaceView } from './lib/feedback'
  import { createNotificationPermissionController } from './lib/workbench/notificationPermissionController'
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
  import {
    savePreviewWorkspaceSnapshot,
    savedPreviewWorkspaceSnapshot,
    seedPreviewWorkspaceScenario,
  } from './lib/workspace/previewWorkspaceSnapshot'
  import { sessionViewResolution, type SessionViewResolution } from './lib/workspace/sessionViewRecovery'
  import {
    workspaceTabId,
    workspaceTabPanelId,
  } from './lib/workspace/workspaceTabNavigation'
  import WorkspaceTabStrip from './lib/workspace/WorkspaceTabStrip.svelte'
  import { workspaceSurface } from './lib/workspace/workspaceSurface'
  import { previewFixtures } from './lib/preview/previewFixtures'
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
  import { shouldShowResumePromptButton } from './lib/workbench/resumePrompt'
  import { createResumePromptController } from './lib/workbench/resumePromptController'
  import { createMessageToaster } from './lib/workbench/messageToasts'
  import { createOnboardingController } from './lib/onboarding/onboardingController'
  import {
    sessionTabLabel as sessionTabLabelFor,
    workspaceTabLabel as workspaceTabLabelFor,
  } from './lib/workspace/tabLabels'
  import {
    createAttachmentController,
  } from './lib/workbench/attachmentController'
  import { createNavigationController } from './lib/workbench/navigationController'
  import { ensureDesktopNavigationPolling } from './lib/workbench/navigationPolling'
  import type { FeedbackEditorHandle } from './lib/editor/feedbackEditorHandle'
  import type { RambleSessionControllerHandle } from './lib/speech/rambleSessionControllerHandle'
import type { SettingsSection } from './lib/domain/settingsSection'
  import RambleSessionController from './lib/workbench/RambleSessionController.svelte'
  import { highlightSpeechSegment } from './lib/speech/highlightSpeechSegment'
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

  const formatTimeLocal = (value: string | null | undefined) =>
    formatTime(value, $locale, tr('Not saved yet'))
  const workspaceSession = createWorkspaceSession()
  const attachmentSession = createAttachmentSession()
  const cookingSession = createCookingSession()
  const shellLayout = createShellLayoutSession()
  const rambleSession = createRambleSession()
  /** The open request, projected from the session so field reads stay short. */
  $: currentRequest = $workspaceSession.request
  let taskTabTitles: ReadonlyMap<string, string> = new Map()
  let renderedWorkspaceView: WorkspaceViewDescriptor | null = null
  let renderedSessionView: SessionViewDescriptor | null = null
  let renderedSessionResolution: SessionViewResolution | null = null
  let pageError = ''
  let sessionWorkbench: FeedbackEditorHandle | undefined
  let managedSessionSection: ManagedSessionSection | undefined
  let rambleController: RambleSessionControllerHandle
  let archivedInitialSession: SessionViewDescriptor | null = null
  let archivedSelectionEpoch = 0
  let settingsSection: SettingsSection = 'general'
  let settingsSectionSelectionEpoch = 0
  let settingsAgentConfigId: string | undefined = undefined
  let settingsAgentAdvanced = false
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
  const workspaceShell = createWorkspaceShellSession({
    snapshots: previewMode
      ? {
          load: savedPreviewWorkspaceSnapshot,
          save: savePreviewWorkspaceSnapshot,
        }
      : undefined,
  })
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
    isInteractionLocked: () => $workspaceSession.interactionLocked,
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

  // Mutual callbacks resolve after all three owners have been composed.
  let startup: StartupController
  let workspaceNavigation: WorkspaceNavigationController

  const navigation = createNavigationController({
    capabilities,
    transport: applicationTransport,
    tr,
    messageFrom,
    getNotificationState: () => $notificationPermission,
    getWorkspaceRequestId: () => workspaceSession.requestId() ?? undefined,
    isDirty: () => dirty,
    saveDraftNow,
    openRequest: (requestId) => workspaceNavigation.openRequest(requestId),
    clearWorkspace: () => workspaceNavigation.clearWorkspace(),
    onPageError: (message) => (pageError = message),
    canSendOsBanners: () => isMac,
    onRequestsArrived: (requests) => { void workspaceNavigation.autoOpenArrivingRequest(requests) },
  })
  const resolveHostProfile = navigation.resolveHostProfile
  startup = createStartupController({
    navigation,
    workspaceShell,
    workspaceSession,
    transport: applicationTransport,
    workspaceNavigation: () => workspaceNavigation,
    tr,
    messageFrom,
    pageError: () => pageError,
    setPageError: (message) => {
      pageError = message
    },
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
    canOpenManagedSession: () => !previewMode,
    transport: applicationTransport,
    navigation,
    workspaceShell,
    workspaceSession,
    workspaceNavigation: () => workspaceNavigation,
    tr,
    messageFrom,
    setPageError: (message) => {
      pageError = message
    },
    isTransitionLocked: () => workspaceTransitionLocked,
    exitRamble,
    rambleCanExit: () => rambleCanExit,
    openArchivedSessions: (initial) => openArchivedSessions(initial),
  })

  workspaceNavigation = createWorkspaceNavigationController({
    navigation,
    workspaceShell,
    workspaceSession,
    draftSession,
    draftController,
    attachmentSession,
    attachmentController,
    cookingSession,
    startup: () => startup,
    managedSessions: () => managedSessions,
    transport: applicationTransport,
    tr,
    messageFrom,
    setPageError: (message) => { pageError = message },
    releaseEditor: () => { sessionWorkbench = undefined },
    refreshNotificationPermission: () => notificationPermission.refresh(),
    isTransitionLocked: () => workspaceTransitionLocked,
    enqueueDocumentTask,
    canAutoOpenRamble: (sessionId) => managedSessionSection?.canAutoOpenRamble(sessionId) === true,
    onboardingOpen: () => $onboarding.open,
    resumePromptOpen: () => $resumePrompts.prompt !== null,
    rambleEngaged: () => rambleEngaged,
  })

  export function refetchAfterTransportReady() {
    workspaceNavigation.refetchAfterTransportReady()
  }

  const toaster = createMessageToaster({ tr })

  const notificationPermission = createNotificationPermissionController({
    notifications: capabilities.notifications,
    isMac: () => isMac,
    getPopupEnabled: () => $notificationPopupEnabled,
    setPopupEnabled: (enabled) => setNotificationPopupEnabled(enabled),
  })

  const onboarding = createOnboardingController({
    locale: () => $locale,
    isCompleted: () => $onboardingCompleted,
    isAvailable: () => onboardingAvailable,
    startup: { start: () => startup.start() },
    managedSessions: {
      openNewManagedSession: (configId) => managedSessions.openNewManagedSession(configId),
    },
    closeSettingsTab: async () => { await workspaceNavigation.closeWorkspaceTab(workspaceViewKey(settingsViewDescriptor())) },
    updates: {
      available: softwareUpdatesAvailable,
      check: (input) => capabilities.softwareUpdates.implementation.check(input),
    },
    reset: () => resetOnboarding(),
  })

  const resumePrompts = createResumePromptController({
    transport: applicationTransport,
    tr,
    messageFrom,
    getCurrentRequest: () => currentRequest,
    getKnownRequests: () => [...$navigation.requests, ...$navigation.pendingRequests],
    getWorkspace: () => $workspaceSession.workspace,
    resolveHostProfile,
    canOpenFromWorkspace: () => canOpenResumePrompt,
    notifications: {
      available: notificationsAvailable,
      isMac,
      getPopupEnabled: () => $notificationPopupEnabled,
      getState: () => $notificationPermission,
      send: (input) => capabilities.notifications.implementation.send(input),
    },
    setPageError: (message) => {
      pageError = message
    },
  })

  $: railAgentSession = agentSessionForView(
    renderedWorkspaceView?.kind === 'agent-session' ? renderedWorkspaceView : null,
    $navigation.hostSessions,
  )
  $: dirty =
    currentRequest !== null &&
    currentRequest.status !== 'completed' &&
    currentRequest.status !== 'cancelled' &&
    $draftSession.dirty
  $: toaster.pageError(pageError)
  $: toaster.saveError($draftSession.message)
  $: toaster.attachmentMessage($attachmentSession.message, $attachmentSession.tone)
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
  $: renderedSessionResolution = sessionViewResolution($startup.resolutions, $workspaceShell.shell.activeViewKey)
  $: renderedAgentSessionView = renderedWorkspaceView?.kind === 'agent-session' ? renderedWorkspaceView : null
  $: renderedAgentDraftView = renderedWorkspaceView?.kind === 'agent-draft' ? renderedWorkspaceView : null
  $: renderedAgentDraftController = renderedAgentDraftView ? managedSessions.draftController(renderedAgentDraftView.draftId) : null
  $: renderedManagedSession = agentSessionForView(renderedAgentSessionView, $navigation.hostSessions)
  $: taskTabTitles = updateTaskTabTitles(taskTabTitles, $workspaceShell.shell.views, [
    ...(currentRequest ? [currentRequest] : []),
    ...$navigation.pendingRequests,
    ...$navigation.requests,
  ])
  $: workspaceTabLabel = (view: WorkspaceViewDescriptor) =>
    workspaceTabLabelFor(view, {
      hostSessions: $navigation.hostSessions,
      resolveHostProfile,
      taskTabTitles,
      locale: $locale,
      tr,
    })
  $: requestScopeLabel = $navigation.selectedHostId
    ? $navigation.selectedHostSessionId
      ? selectedHostSession?.source_hint ??
        selectedHostSession?.title ??
        resolveHostProfile($navigation.selectedHostId).label
      : resolveHostProfile($navigation.selectedHostId).label
    : tr('All hosts')
  $: feedbackResult = $workspaceSession.feedbackResult
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
    !$workspaceSession.interactionLocked
  $: canCancel =
    currentRequest !== null &&
    currentRequest.status !== 'completed' &&
    currentRequest.status !== 'cancelled' &&
    !currentRequestCooking &&
    !$workspaceSession.interactionLocked
  $: interactionLocked = $workspaceSession.interactionLocked
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
  $: voiceActive = $rambleSession.voiceActive
  $: voiceCanStop = $rambleSession.voiceCanStop
  $: visibleRamblePhase = $rambleSession.visiblePhase
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
    function guardBrowserLeave(event: BeforeUnloadEvent) {
      // Undo can make the draft locally clean while an earlier save is still
      // in flight. Read both owners at the event boundary; never save on unload.
      if (!draftSession.isDirty() && !draftController.hasPendingSave()) return
      event.preventDefault()
      event.returnValue = ''
    }
    if (environment === 'browser') window.addEventListener('beforeunload', guardBrowserLeave)
    const cleanupAttachments = attachmentController.mount()
    const unsubscribeApplicationEvents = !desktopShellAvailable && !previewMode
      ? applicationTransport.subscribe(
          APPLICATION_EVENTS_STREAM,
          (event) => {
            if (event.type === 'invalidate') {
              workspaceNavigation.requestRefetch(event.resources)
            }
          },
          (cause) => {
            pageError = messageFrom(cause)
          },
        )
      : () => {}

    let resumePromptUnlisten = () => {}
    function disposeClient() {
      if (environment === 'browser') window.removeEventListener('beforeunload', guardBrowserLeave)
      unsubscribeApplicationEvents()
      startup.dispose()
      workspaceNavigation.dispose()
      draftController.cancelPendingSave()
      if (inboxTimer) clearInterval(inboxTimer)
      resumePromptUnlisten()
      resumePrompts.dispose()
      onboarding.dispose()
      cleanupAttachments()
      managedSessions.closeAllDrafts()
    }

    if (!desktopShellAvailable) {
      onboarding.begin()
      if (
        previewMode &&
        new URLSearchParams(window.location.search).get('dialog') === 'resume'
      ) {
        resumePrompts.show(previewFixtures.resumePrompt)
      }
      notificationPermission.markUnavailable()
      if (
        capabilities.softwareUpdates.status.availability !== 'unavailable' &&
        new URLSearchParams(window.location.search).get('dialog') === 'update'
      ) {
        void capabilities.softwareUpdates.implementation.check({ prompt: true, forcePrompt: true })
      }
      return disposeClient
    }
    onboarding.begin()
    // Browser server autostart is a desktop preference; the browser client never runs it.
    if (
      initialWebAccessAutostart() &&
      capabilities.webAccessAdministration.status.availability !== 'unavailable'
    ) {
      void capabilities.webAccessAdministration.implementation
        .setEnabled(true, initialWebAccessPort())
        .catch(() => undefined)
    }
    onboarding.scheduleLaunchCheck()
    if (notificationsAvailable) void notificationPermission.refresh()
    else notificationPermission.markUnavailable()
    resumePromptUnlisten = resumePrompts.subscribeStream()
    return disposeClient
  })

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
    prepareFeedback: (requestId) => rambleController.prepareFeedback(requestId),
    saveDraftNow,
    setPageError: (message) => {
      pageError = message
    },
    setCooking: cookingSession.setCooking,
    setPreview: cookingSession.setPreview,
  })
  const cookPreviewOnly = cookingController.cookPreviewOnly
  const restoreOriginalAfterCook = cookingController.restoreOriginal

  const publisherController = createPublisherController({
    transport: applicationTransport,
    tr,
    messageFrom,
    session: workspaceSession,
    draft: draftSession,
    cooking: cookingSession,
    setPageError: (message) => {
      pageError = message
    },
    isReadOnly: () => managedFeedbackReadOnly,
    prepareFeedback: (requestId) => rambleController.prepareFeedback(requestId),
    saveDraftNow,
    getCookingEnabled: () => $cookingEnabled,
    cookSubmission: cookingController.cookSubmission,
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
    prepareFeedback: (requestId) => rambleController.prepareFeedback(requestId),
    saveDraftNow,
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

  <RambleSessionController
    bind:this={rambleController}
    session={rambleSession}
    {capabilities}
    {tidyConfig}
    workspace={$workspaceSession.workspace}
    attachmentBusy={$attachmentSession.busy}
    screenCaptureBusy={$attachmentSession.captureBusy}
    onAttachmentMessage={attachmentSession.setMessage}
    interactionLocked={managedFeedbackReadOnly || interactionLocked || currentRequestCooking || cookedDraftReady}
    onPageError={(message) => (pageError = message)}
    onStartScreenCapture={attachmentController.startScreenCapture}
    onImportServerAttachmentPaths={attachmentController.importServerAttachmentPaths}
    onPersistAttachmentCandidates={attachmentController.persistAttachmentCandidates}
    onRouteDraftOperation={routeDraftOperation}
    waitForDocumentWrites={waitForDocumentQueue}
    getActiveAction={activeActionFor}
    onOpenSpeechTarget={async (requestId, segmentId) => {
      if (await workspaceNavigation.openRequest(requestId)) {
        await tick()
        if (segmentId) highlightSpeechSegment(document, segmentId, true)
      }
    }}
  />

{#key $locale}
<main class={[
  'flex h-full w-full flex-col overflow-hidden rounded-[var(--app-frame-radius)] bg-background text-foreground',
  environment === 'browser' ? '' : 'border shadow-sm',
]}
  class:navigation-resizing={navigationResizing}
  style:--workbench-sidebar-width={`${hostRailDisplayWidth}px`}>
  <Sonner />


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
        onClose={async (viewKey) => { await workspaceNavigation.closeWorkspaceTab(viewKey) }}
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
      <StartupRecoveryPanel {capabilities} message={$startup.failureMessage} timedOut={$startup.failureTimedOut} settingsOpen={$startup.settingsOpen} onSettingsOpenChange={(settingsOpen) => startup.patch({ settingsOpen })} onRetry={() => void startup.start()} />
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
            onRestartOnboarding={onboarding.restart}
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
              ? $draftOperations.get(currentRequest.request_id)?.actionId ?? null
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
            label={sessionTabLabelFor(renderedSessionResolution.session, {
              hostSessions: $navigation.hostSessions,
              resolveHostProfile,
            })}
            busy={renderedSessionResolution.reason === 'unresolved' || $workspaceShell.pendingViewKey !== null}
            onRetry={startup.retrySessionViewRecovery}
            onClose={async () => { await workspaceNavigation.closeWorkspaceTab(workspaceViewKey(renderedSessionResolution!.session)) }}
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
          ? $draftOperations.get(currentRequest.request_id)?.actionId ?? null
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
        onAutoOpenTask={workspaceNavigation.autoOpenTaskView}
        onStartScreenCapture={() => void attachmentController.startScreenCapture()}
        onImportClipboard={() => void importClipboardNow()}
        onFileSelection={attachmentController.handleFileSelection}
        onPasteCandidates={attachmentController.acceptAttachmentCandidates}
        onPasteError={attachmentController.reportClientFileError}
        onRemoveAttachment={(attachment) => void attachmentController.removeAttachment(attachment)}
        onOpenPackage={() => void openFeedbackPackage()}
        packageActionLabel={tr(publishedFeedbackAction.label)}
        onOpenResumePrompt={resumePrompts.open}
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

  {#if $resumePrompts.prompt}
    <ResumePromptDialog
      prompt={$resumePrompts.prompt}
      copyState={$resumePrompts.copyState}
      onCopy={() => void resumePrompts.copy()}
      onDismiss={resumePrompts.dismiss}
    />
  {/if}
</main>

{#if onboardingAvailable && $onboarding.open}
  <OnboardingWizard
    {capabilities}
    transport={applicationTransport}
    openWizard={$onboarding.open}
    onClose={onboarding.close}
    onStartSession={onboarding.startSession}
  />
{/if}

{#if softwareUpdatesAvailable}
  <UpdateAvailableDialog
    softwareUpdates={capabilities.softwareUpdates}
    installBlocked={updateInstallBlocked}
    externalLinks={capabilities.externalLinks}
    onError={(message) => (pageError = message)}
  />
{/if}
{/key}
