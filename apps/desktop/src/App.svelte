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
  import SettingsPanel from './lib/SettingsPanel.svelte'
  import UpdateAvailableDialog from './lib/UpdateAvailableDialog.svelte'
  import ArchivedSessionsDialog from './lib/components/navigation/ArchivedSessionsDialog.svelte'
  import RequestListPane from './lib/components/navigation/RequestListPane.svelte'
  import SessionRail from './lib/components/navigation/SessionRail.svelte'
  import {
    workbenchRequestKey,
    type WorkbenchRequestListItem,
  } from './lib/components/navigation/requestListItem'
  import {
    sessionRailKey,
    type SessionOrigin,
    type SessionRailItem,
  } from './lib/components/navigation/sessionRailItem'
  import AskView from './lib/acp-workbench/AskView.svelte'
  import LaunchRambleDialog from './lib/acp-workbench/LaunchRambleDialog.svelte'
  import PermissionView from './lib/acp-workbench/PermissionView.svelte'
  import {
    acpAdapterErrorMessage,
    createNativeAcpWorkbenchAdapter,
  } from './lib/acp-workbench/adapter'
  import { createPreviewAcpWorkbenchAdapter } from './lib/acp-workbench/previewAdapter'
  import {
    isAttentionItemAnswerable,
    itemsForSession,
    orderSessions,
  } from './lib/acp-workbench/state'
  import type {
    AcpWorkbenchSnapshot,
    AttentionItem,
    DraftSnapshotV3,
    FeedbackAttentionItem,
    LaunchDraft,
    LaunchPreflight,
    PermissionOption,
    QuestionAttentionItem,
  } from './lib/acp-workbench/types'
  import {
    matchesUnifiedWorkbenchRequestFilter,
    projectFeedbackWorkspace,
    projectUnifiedWorkbench,
    resolveAgentProfile as resolveAcpAgentProfile,
    type UnifiedWorkbenchRequestListItem,
  } from './lib/acp-workbench/workbenchProjection'
  import { Sonner, toast } from './lib/components/ui/sonner'
  import ResumePromptDialog from './lib/workbench/ResumePromptDialog.svelte'
  import WorkspacePanel from './lib/workbench/WorkspacePanel.svelte'
  import type { JSONContent } from '@tiptap/core'

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
    type AttachmentArtifactPort,
    type AttachmentMessageTone,
  } from './lib/workbench/attachmentController'
  import { createNavigationController } from './lib/workbench/navigationController'
  import { resolvedRamblePhase } from './lib/workbench/rambleSessionState'
  import {
    createWorkspaceLoadGate,
    ownerForOperation,
    type WorkbenchOperationTarget,
    type WorkbenchRequestOwner,
  } from './lib/workbench/workbenchRouting'
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
  let workspacePanel: FeedbackEditorHandle
  let rambleController: RambleSessionControllerHandle
  let resumePrompt: ResumePrompt | null = null
  let resumeCopyState: 'idle' | 'copied' | 'failed' = 'idle'
  let notificationState: NotificationState = 'checking'
  let settingsOpen = false
  let archivedSessionsOpen = false
  let settingsSection: SettingsSection = 'general'
  let onboardingOpen = false
  let launchUpdateCheckDue = false
  let workbenchInitialized = false
  const isTauri = '__TAURI_INTERNALS__' in window
  const isMac = currentDesktopPlatform() === 'macOS'
  const pageParameters = new URLSearchParams(window.location.search)
  const acpPreviewMode = !isTauri && pageParameters.get('preview') === 'acp'
  const acpEnabled = isTauri || acpPreviewMode
  const acpAdapter = acpPreviewMode
    ? createPreviewAcpWorkbenchAdapter()
    : createNativeAcpWorkbenchAdapter()
  const previewMode =
    import.meta.env.DEV &&
    !isTauri &&
    pageParameters.get('preview') === 'fixtures'
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
  let rambleOwner: WorkbenchRequestOwner | null = null
  let activeActionByRequest = new Map<string, NonNullable<ActiveAction>>()
  let inboxTimer: ReturnType<typeof setInterval> | undefined
  let acpSnapshot: AcpWorkbenchSnapshot = { sessions: [], attentionItems: [], agents: [] }
  let activeSessionKey: string | null = null
  let activeRequestKey: string | null = null
  let workspaceOrigin: SessionOrigin | null = null
  let workspaceRequestKey: string | null = null
  let acpLoading = false
  let acpRefreshing = false
  let acpRefreshInFlight = false
  let acpLoadError = ''
  let acpActionBusy = false
  let acpSnapshotEpoch = 0
  let launchRambleOpen = false
  const feedbackSubmissionIds = new Map<string, string>()
  const workspaceLoadGate = createWorkspaceLoadGate()

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function currentWorkspaceOwner(): WorkbenchRequestOwner | null {
    if (!workspace || !workspaceRequestKey || !workspaceOrigin) return null
    return {
      key: workspaceRequestKey,
      origin: workspaceOrigin,
      requestId: workspace.request.request_id,
      sessionId: workspace.request.host_session_id,
    }
  }

  function ownerForTarget(
    requestId: string,
    target: WorkbenchOperationTarget,
  ): WorkbenchRequestOwner | null {
    return ownerForOperation(requestId, target, currentWorkspaceOwner(), rambleOwner)
  }

  function acpArtifactPortFor(sessionId: string): AttachmentArtifactPort {
    return {
      loadWorkspace: (requestId) => loadAcpFeedbackWorkspace(requestId, sessionId),
      addBytes: async (input) => workspaceFromAcpDraft(
        input.requestId,
        await acpAdapter.addDraftArtifact(input),
        sessionId,
      ),
      addPath: async (input) => workspaceFromAcpDraft(
        input.requestId,
        await acpAdapter.addDraftArtifactPath(input),
        sessionId,
      ),
      addScreenCapture: async (input) => workspaceFromAcpDraft(
        input.requestId,
        await acpAdapter.addCompletedScreenCapture(input),
        sessionId,
      ),
      remove: async (input) => workspaceFromAcpDraft(
        input.requestId,
        await acpAdapter.removeDraftArtifact(input),
        sessionId,
      ),
      reorder: async (input) => workspaceFromAcpDraft(
        input.requestId,
        await acpAdapter.reorderDraftArtifacts(input),
        sessionId,
      ),
      read: (requestId, artifactId) => acpAdapter.readDraftArtifact(requestId, artifactId),
    }
  }

  function acpClipboardArtifactPortFor(sessionId: string) {
    return {
      loadWorkspace: (requestId: string) => loadAcpFeedbackWorkspace(requestId, sessionId),
      addClipboardCapture: async (input: {
        requestId: string
        captureId: string
        rambleContextId: string
        fileName: string
        expectedRevision: number
      }) => workspaceFromAcpDraft(
        input.requestId,
        await acpAdapter.addCompletedClipboardCapture(input),
        sessionId,
      ),
    }
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
    persistDraft: persistActiveDraft,
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
    routeDraftOperation: (requestId, operation, target = 'workspace') =>
      routeDraftOperation(
        requestId,
        operation,
        ownerForTarget(requestId, target),
      ),
    activeActionFor: (requestId, target = 'workspace') =>
      activeActionFor(requestId, ownerForTarget(requestId, target)),
    applyWorkspaceMutation,
    getArtifactPort: (requestId, target) => {
      const owner = ownerForTarget(requestId, target)
      return owner?.origin === 'managed_acp'
        ? acpArtifactPortFor(owner.sessionId)
        : undefined
    },
    isOperationTargetVisible: (requestId, target) =>
      ownerForTarget(requestId, target)?.key === workspaceRequestKey,
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

  function resolveRequestAgentProfile(agentId: string) {
    const hostProfile = resolveHostProfile(agentId)
    const acpProfile = resolveAcpAgentProfile(acpSnapshot.agents, agentId)
    return {
      id: hostProfile.id,
      label: hostProfile.label || acpProfile.label,
      iconSvg: hostProfile.icon_svg || acpProfile.iconSvg,
    }
  }

  function resolveWorkspaceProfile(agentId: string) {
    const hostProfile = resolveHostProfile(agentId)
    if (workspaceOrigin !== 'managed_acp') return hostProfile
    const profile = resolveAcpAgentProfile(acpSnapshot.agents, agentId)
    return {
      id: profile.id,
      label: profile.label,
      icon_svg: hostProfile.icon_svg || profile.iconSvg,
      default_adapter: 'generic_mcp' as const,
      continuation_mode: 'not_required' as const,
    }
  }

  function applyAcpSnapshot(next: AcpWorkbenchSnapshot) {
    acpSnapshot = next
  }

  function acpItemById(itemId: string, sessionId?: string): AttentionItem | null {
    return acpSnapshot.attentionItems.find(
      (item) => item.id === itemId && (!sessionId || item.sessionId === sessionId),
    ) ?? null
  }

  function acpSessionFor(item: AttentionItem) {
    return acpSnapshot.sessions.find((session) => session.sessionId === item.sessionId)
  }

  function workspaceFromAcpDraft(
    requestId: string,
    draft: DraftSnapshotV3,
    sessionId = workspace?.request.host_session_id,
  ) {
    const item = acpItemById(requestId, sessionId)
    if (!item || item.kind !== 'feedback') {
      throw new Error(tr('This Feedback Request could not be found.'))
    }
    return projectFeedbackWorkspace(item, acpSessionFor(item), draft)
  }

  async function loadAcpFeedbackWorkspace(requestId: string, sessionId?: string) {
    const detail = await acpAdapter.readFeedback(requestId)
    if (!detail.draft) throw new Error(tr('Save the Feedback Draft before adding an Artifact.'))
    return workspaceFromAcpDraft(requestId, detail.draft, sessionId)
  }

  async function persistAcpDraft(input: SaveDraftInput, sessionId?: string): Promise<DraftView> {
    acpSnapshotEpoch += 1
    const next = await acpAdapter.saveDraft({
      requestId: input.request_id,
      expectedRevision: input.expected_revision,
      documentJson: input.document_json,
      bodyMarkdown: input.body_markdown,
    })
    applyAcpSnapshot(next)
    const saved = next.attentionItems.find(
      (item): item is FeedbackAttentionItem =>
        item.id === input.request_id &&
        item.sessionId === (sessionId ?? workspace?.request.host_session_id) &&
        item.kind === 'feedback',
    )
    if (!saved) throw new Error(tr('The saved Feedback Request was not found.'))
    return {
      document_json: saved.draftDocument === null
        ? input.document_json
        : typeof saved.draftDocument === 'string'
          ? saved.draftDocument
          : JSON.stringify(saved.draftDocument),
      body_markdown: saved.draftMarkdown,
      saved_revision: saved.draftRevision,
      updated_at: new Date().toISOString(),
    }
  }

  function persistActiveDraft(input: SaveDraftInput): Promise<DraftView> {
    if (workspaceOrigin === 'managed_acp') return persistAcpDraft(input)
    return invoke<DraftView>('save_feedback_draft', { input })
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
  $: unifiedWorkbench = projectUnifiedWorkbench({
    adapterSessions: $navigation.hostSessions,
    adapterRequests: $navigation.requests,
    acpSessions: acpSnapshot.sessions,
    attentionItems: acpSnapshot.attentionItems,
    agents: acpSnapshot.agents,
    resolveHostProfile,
  })
  $: selectedSessionItem = activeSessionKey
    ? unifiedWorkbench.sessions.find((session) => session.key === activeSessionKey) ?? null
    : null
  $: requestListItems = unifiedWorkbench.requests.filter((request) =>
    matchesUnifiedWorkbenchRequestFilter(request, {
      sessionKey: activeSessionKey,
      search: $navigation.requestSearch,
    }))
  $: visibleRequests = todayOnly
    ? requestListItems.filter((request) => isWithinLast24Hours(request.updatedAt))
    : requestListItems
  $: selectedRequestItem = activeRequestKey
    ? unifiedWorkbench.requests.find((request) => request.key === activeRequestKey) ?? null
    : null
  $: selectedAcpItem = selectedRequestItem?.origin === 'managed_acp'
    ? acpItemById(selectedRequestItem.rawRequestId, selectedRequestItem.sessionId)
    : null
  $: visibleAcpItems = selectedAcpItem
    ? itemsForSession(acpSnapshot.attentionItems, selectedAcpItem.sessionId)
    : []
  $: selectedAcpItemAnswerable = selectedAcpItem
    ? isAttentionItemAnswerable(visibleAcpItems, selectedAcpItem.id)
    : false
  $: requestScopeLabel = selectedSessionItem?.title ?? tr('All requests')
  $: feedbackResult = completedResult?.feedback ?? workspace?.feedback ?? null
  $: canOpenResumePrompt = shouldShowResumePromptButton(
    feedbackResult,
    completedResult?.resolution ?? workspace?.request.resolution,
  )
  $: currentRequestCooking =
    workspace !== null && cookingRequestIds.has(workspaceRequestKey ?? workspace.request.request_id)
  $: cookingRequestKeys = cookingRequestIds
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
  $: rambleBelongsToWorkspace =
    !rambleEngaged || (!!workspaceRequestKey && workspaceRequestKey === rambleOwner?.key)
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
        adoptDraft(previewFixtures.workspace.draft)
        savePhase = 'saved'
        if (new URLSearchParams(window.location.search).get('dialog') === 'resume') {
          resumePrompt = previewFixtures.resumePrompt
        }
      }
      notificationState = 'unavailable'
      if (new URLSearchParams(window.location.search).get('dialog') === 'update') {
        void checkForUpdates({ prompt: true, forcePrompt: true })
      }
      return () => {
        draftController.cancelPendingSave()
        if (inboxTimer) clearInterval(inboxTimer)
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

  async function openAcpItem(
    request: UnifiedWorkbenchRequestListItem,
    saveCurrent = true,
  ) {
    const item = acpItemById(request.rawRequestId, request.sessionId)
    if (!item || interactionLocked) return
    if (saveCurrent && dirty && !(await saveDraftNow())) return
    if (
      item.kind === 'feedback' &&
      workspaceRequestKey !== request.key &&
      rambleCanExit
    ) {
      await exitRamble()
    }
    activeRequestKey = request.key
    if (item.kind !== 'feedback') {
      workspaceLoadGate.invalidate()
      loadingWorkspace = false
      return
    }
    if (workspaceRequestKey === request.key) return
    pageError = ''
    completedResult = null
    publishedFeedback = null
    clearWorkspace()
    const loadToken = workspaceLoadGate.begin(request.key)
    loadingWorkspace = true
    try {
      const detail = await acpAdapter.readFeedback(item.id)
      const next = projectFeedbackWorkspace(item, acpSessionFor(item), detail.draft)
      if (!workspaceLoadGate.isCurrent(loadToken, activeRequestKey)) return
      workspace = next
      workspaceOrigin = 'managed_acp'
      workspaceRequestKey = request.key
      cookedPreview = null
      adoptDraft(next.draft)
      savePhase = next.draft.updated_at ? 'saved' : 'idle'
      saveMessage = ''
      attachmentMessage = ''
      publishedFeedback = detail.publishedFeedback
      await attachmentController.refreshPreviews(next)
    } catch (cause) {
      if (workspaceLoadGate.isCurrent(loadToken, activeRequestKey)) {
        pageError = acpAdapterErrorMessage(cause)
      }
    } finally {
      if (workspaceLoadGate.isCurrent(loadToken, activeRequestKey)) {
        loadingWorkspace = false
      }
    }
  }

  async function reconcileManagedSelection(sessionId?: string) {
    await tick()
    const current = activeRequestKey
      ? unifiedWorkbench.requests.find((request) => request.key === activeRequestKey)
      : null
    if (current?.origin === 'managed_acp') {
      if (current.kind === 'feedback' && workspaceRequestKey !== current.key) {
        await openAcpItem(current, false)
      }
      return
    }
    if (!sessionId) return
    const session = acpSnapshot.sessions.find((candidate) => candidate.sessionId === sessionId)
    const next = session
      ? unifiedWorkbench.requests.find(
          (request) =>
            request.origin === 'managed_acp' &&
            request.sessionId === sessionId &&
            request.agentId === session.agentId,
        )
      : null
    activeRequestKey = next?.key ?? null
    if (next?.kind === 'feedback') await openAcpItem(next, false)
    else if (!next && workspaceOrigin === 'managed_acp') clearWorkspace()
  }

  async function openUnifiedRequest(
    request: WorkbenchRequestListItem,
    saveCurrent = true,
  ) {
    if (
      interactionLocked ||
      (activeRequestKey === request.key &&
        (request.kind !== 'feedback' || workspaceRequestKey === request.key))
    ) return
    if (saveCurrent && dirty && !(await saveDraftNow())) return
    if (request.origin === 'adapter') {
      await openRequest(request.rawRequestId, false, request.key)
      return
    }
    await openAcpItem(request, false)
  }

  async function chooseUnifiedSession(item: SessionRailItem | null) {
    if (dirty && !(await saveDraftNow())) return
    if (item && selectedRequestItem?.sessionKey !== item.key) activeRequestKey = null
    activeSessionKey = item?.key ?? null
    if (!item) {
      await navigation.selectScope(null, null)
      return
    }
    if (item.origin === 'adapter') {
      await navigation.selectScope(item.hostId, item.sessionId)
      return
    }
    await navigation.selectScope(null, null)
  }

  function adapterHostSessionFor(item: SessionRailItem) {
    if (item.origin !== 'adapter') return null
    return $navigation.hostSessions.find(
      (session) =>
        session.host_id === item.hostId &&
        session.host_session_id === item.sessionId,
    ) ?? null
  }

  async function renameUnifiedSession(item: SessionRailItem, title: string) {
    if (!item.canRename) return
    if (item.origin === 'managed_acp') {
      acpSnapshotEpoch += 1
      try {
        applyAcpSnapshot(await acpAdapter.renameSession(item.sessionId, title))
      } catch (cause) {
        pageError = acpAdapterErrorMessage(cause)
      }
      return
    }
    const session = adapterHostSessionFor(item)
    if (session) await navigation.renameHostSession(session, title)
  }

  async function setUnifiedSessionPinned(item: SessionRailItem, pinned: boolean) {
    if (!item.canPin) return
    if (item.origin === 'managed_acp') {
      acpSnapshotEpoch += 1
      try {
        applyAcpSnapshot(await acpAdapter.setSessionPinned(item.sessionId, pinned))
      } catch (cause) {
        pageError = acpAdapterErrorMessage(cause)
      }
      return
    }
    const session = adapterHostSessionFor(item)
    if (session) await navigation.setHostSessionPinned(session, pinned)
  }

  async function archiveUnifiedSession(item: SessionRailItem) {
    const session = adapterHostSessionFor(item)
    if (!session || !item.canArchive) return
    await navigation.archiveHostSession(session)
    if (activeSessionKey === item.key) {
      activeSessionKey = null
      await navigation.selectScope(null, null)
    }
    if (activeRequestKey?.startsWith(`${item.origin}\u0000${item.key}\u0000`)) {
      activeRequestKey = null
    }
  }

  async function refreshUnifiedWorkbench() {
    await Promise.all([
      navigation.refreshPage(),
      acpEnabled ? refreshAcp() : Promise.resolve(),
    ])
  }

  async function refreshAcp(background = false) {
    if (acpRefreshInFlight) return
    acpRefreshInFlight = true
    if (!background) acpRefreshing = true
    if (!background) acpLoading = true
    const refreshEpoch = acpSnapshotEpoch
    try {
      const next = await acpAdapter.readWorkbench()
      if (refreshEpoch !== acpSnapshotEpoch) return
      const ownerClosed =
        workspaceOrigin === 'managed_acp' &&
        rambleEngaged &&
        rambleRequestId !== '' &&
        !next.attentionItems.some(
          (item) =>
            item.kind === 'feedback' &&
            item.id === rambleRequestId &&
            item.sessionId === workspace?.request.host_session_id &&
            item.status === 'waiting',
        )
      const selectedManagedSessionId = selectedAcpItem?.sessionId
      applyAcpSnapshot(next)
      if (ownerClosed) {
        await exitRamble()
        toast.info(tr('The active Feedback Request was closed, so Ramble recording stopped.'))
      }
      acpLoadError = ''
      await reconcileManagedSelection(selectedManagedSessionId)
    } catch (cause) {
      acpLoadError = acpAdapterErrorMessage(cause)
      if (!background) pageError = acpLoadError
    } finally {
      acpRefreshInFlight = false
      if (!background) {
        acpRefreshing = false
        acpLoading = false
      }
    }
  }

  async function answerAcpPermission(requestId: string, option: PermissionOption) {
    if (acpActionBusy || !selectedAcpItemAnswerable) return
    const sessionId = selectedAcpItem?.sessionId
    acpActionBusy = true
    acpSnapshotEpoch += 1
    try {
      applyAcpSnapshot(await acpAdapter.answerPermission({ requestId, optionId: option.id }))
      toast.success(tr('Permission answered'))
      await reconcileManagedSelection(sessionId)
    } catch (cause) {
      pageError = acpAdapterErrorMessage(cause)
    } finally {
      acpActionBusy = false
    }
  }

  async function answerAcpQuestion(
    item: QuestionAttentionItem,
    choiceIds: string[],
    skipped: boolean,
  ) {
    if (acpActionBusy || !selectedAcpItemAnswerable) return
    const sessionId = item.sessionId
    acpActionBusy = true
    acpSnapshotEpoch += 1
    try {
      applyAcpSnapshot(await acpAdapter.answerQuestion({
        requestId: item.id,
        choiceIds,
        skipped,
      }))
      toast.success(skipped ? tr('Question skipped') : tr('Answer sent'))
      await reconcileManagedSelection(sessionId)
    } catch (cause) {
      pageError = acpAdapterErrorMessage(cause)
    } finally {
      acpActionBusy = false
    }
  }

  async function preflightAcpLaunch(input: LaunchDraft): Promise<LaunchPreflight> {
    try {
      return await acpAdapter.preflightLaunch(input)
    } catch (cause) {
      return {
        agentId: input.agentId,
        models: [],
        reasoningEfforts: [],
        accessModes: [],
        warning: acpAdapterErrorMessage(cause),
      }
    }
  }

  async function launchAcpRamble(input: LaunchDraft) {
    if (acpActionBusy) return
    acpActionBusy = true
    acpSnapshotEpoch += 1
    const existing = new Set(acpSnapshot.sessions.map((session) => session.sessionId))
    try {
      const next = await acpAdapter.launchRamble(input)
      applyAcpSnapshot(next)
      const launched = orderSessions(next.sessions).find((session) => !existing.has(session.sessionId))
      if (launched) {
        activeSessionKey = sessionRailKey('managed_acp', launched.agentId, launched.sessionId)
      }
      launchRambleOpen = false
      toast.success(tr('Ramble launched'))
      await reconcileManagedSelection(launched?.sessionId)
    } catch (cause) {
      pageError = acpAdapterErrorMessage(cause)
    } finally {
      acpActionBusy = false
    }
  }

  function startWorkbench() {
    if (workbenchInitialized) return
    workbenchInitialized = true
    void navigation.initialize()
    if (acpEnabled) void refreshAcp(true)
    if (isTauri) {
      inboxTimer = setInterval(() => {
        void navigation.refreshNavigation(true)
        void refreshAcp(true)
      }, 5_000)
    }
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

  function openResumePrompt() {
    if (!workspace || !canOpenResumePrompt) return
    resumePrompt = buildResumePrompt(workspace, resolveHostProfile(workspace.request.host_id), tr)
    resumeCopyState = 'idle'
  }

  function clearWorkspace() {
    workspaceLoadGate.invalidate()
    workspace = null
    workspaceOrigin = null
    workspaceRequestKey = null
    completedResult = null
    publishedFeedback = null
    attachmentController.releasePreviews()
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

  async function openRequest(
    requestId: string,
    saveCurrent = true,
    requestKey?: string,
  ) {
    if (
      interactionLocked ||
      (workspaceOrigin === 'adapter' && workspaceRequestKey === requestKey)
    ) return
    if (saveCurrent && !(await saveDraftNow())) return
    const loadToken = workspaceLoadGate.begin(requestKey ?? null)
    loadingWorkspace = true
    pageError = ''
    completedResult = null
    publishedFeedback = null
    try {
      await enqueueDocumentTask(async () => {
        const next = previewMode
          ? previewWorkspaceFor(requestId)
          : await invoke<FeedbackWorkspaceView>('get_feedback_workspace', {
              requestId,
            })
        if (!next) throw new Error(tr('This feedback request could not be found.'))
        const nextPublishedFeedback = next.request.status === 'completed' && next.feedback
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
        if (!workspaceLoadGate.isCurrent(loadToken, activeRequestKey)) return
        workspace = next
        workspaceOrigin = 'adapter'
        workspaceRequestKey = requestKey ?? workbenchRequestKey(
          'adapter',
          sessionRailKey('adapter', next.request.host_id, next.request.host_session_id),
          next.request.request_id,
        )
        activeRequestKey = workspaceRequestKey
        cookedPreview = null
        adoptDraft(next.draft)
        savePhase = next.draft.updated_at ? 'saved' : 'idle'
        saveMessage = ''
        attachmentMessage = ''
        publishedFeedback = nextPublishedFeedback
        await attachmentController.refreshPreviews(next)
      })
    } catch (cause) {
      if (workspaceLoadGate.isCurrent(loadToken, activeRequestKey)) {
        pageError = messageFrom(cause)
      }
    } finally {
      if (workspaceLoadGate.isCurrent(loadToken, activeRequestKey)) {
        loadingWorkspace = false
      }
    }
  }

  function activeActionFor(
    requestId: string,
    owner: WorkbenchRequestOwner | null = currentWorkspaceOwner(),
  ): ActiveAction {
    const key = owner?.requestId === requestId ? owner.key : requestId
    return activeActionByRequest.get(key) ?? null
  }

  function enqueueDocumentTask<T>(task: () => Promise<T>): Promise<T> {
    const run = rambleDocumentQueue.then(task)
    rambleDocumentQueue = run.then(
      () => undefined,
      () => undefined,
    )
    return run
  }

  async function routeDraftOperation(
    requestId: string,
    operation: DraftOperation,
    owner: WorkbenchRequestOwner | null = currentWorkspaceOwner(),
  ): Promise<void> {
    if (!requestId) return
    const run = enqueueDocumentTask(async () => {
      if (owner && workspace && workspaceRequestKey === owner.key) {
        if (
          workspace.request.status === 'completed' ||
          workspace.request.status === 'cancelled'
        ) {
          throw new Error(tr('This request is closed. The document is read-only.'))
        }
        let applied = workspacePanel?.applyDraftOperation(operation) ?? false
        if (!applied) {
          await tick()
          applied = workspacePanel?.applyDraftOperation(operation) ?? false
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
          if (owner?.origin === 'managed_acp') {
            return loadAcpFeedbackWorkspace(requestId, owner.sessionId)
          }
          const target = previewMode
            ? previewWorkspaceFor(requestId)
            : await invoke<FeedbackWorkspaceView>('get_feedback_workspace', { requestId })
          if (!target) throw new Error(tr('This feedback request could not be found.'))
          return target
        },
        save: async (input) => {
          if (owner?.origin === 'managed_acp') {
            return persistAcpDraft(input, owner.sessionId)
          }
          return previewMode
            ? {
                document_json: input.document_json,
                body_markdown: input.body_markdown,
                saved_revision: input.expected_revision + 1,
                updated_at: new Date().toISOString(),
              }
            : invoke<DraftView>('save_feedback_draft', { input })
        },
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
    const owner = currentWorkspaceOwner()
    const requestKey = workspaceRequestKey ?? requestId
    if (activeActionByRequest.get(requestKey)?.actionId === actionId) {
      activeActionByRequest.delete(requestKey)
      activeActionByRequest = new Map(activeActionByRequest)
      void routeDraftOperation(
        requestId,
        { kind: 'clearActionGroup', actionId },
        owner,
      ).catch(() => {})
      return
    }
    const action = { actionId, actionIndex, title }
    activeActionByRequest.set(requestKey, action)
    activeActionByRequest = new Map(activeActionByRequest)
    void routeDraftOperation(
      requestId,
      { kind: 'startActionGroup', action },
      owner,
    ).catch(() => {})
  }

  async function reloadWorkspace() {
    if (interactionLocked) return
    const requestId = workspace?.request.request_id
    if (!requestId) return
    if (rambleCanExit) await exitRamble()
    if (dirty && !(await saveDraftNow())) return
    const requestKey = workspaceRequestKey
    clearWorkspace()
    const request = requestKey
      ? unifiedWorkbench.requests.find((candidate) => candidate.key === requestKey)
      : null
    if (request?.origin === 'managed_acp') await openAcpItem(request, false)
    else await openRequest(requestId, false, requestKey ?? undefined)
  }

  async function openSettings(section: SettingsSection) {
    settingsSection = section
    settingsOpen = true
    pageError = ''
    await tick()
    if (!isTauri || section !== 'adapters') return
    try {
      genericMcpConfiguration = await invoke<string>('get_generic_mcp_configuration')
    } catch (cause) {
      pageError = messageFrom(cause)
    }
  }

  function openArchivedSessions() {
    settingsOpen = false
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
    const key = workspace?.request.request_id === requestId
      ? workspaceRequestKey ?? requestId
      : requestId
    if (cooking) next.add(key)
    else next.delete(key)
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
    refreshNavigation: (force) => navigation.refreshNavigation(force),
    showSubmittedToast: (cooked) => {
      toast.success(tr('Feedback submitted'), {
        description: cooked ? tr('Cooked and uncooked feedback published') : tr('Feedback package published'),
      })
    },
  })
  const submitLegacyFeedback = publisherController.submitFeedback

  async function submitFeedback() {
    if (workspaceOrigin !== 'managed_acp') {
      await submitLegacyFeedback()
      return
    }
    const item = workspace
      ? acpItemById(workspace.request.request_id, workspace.request.host_session_id)
      : null
    if (!workspace || item?.kind !== 'feedback' || !canSubmit || submitting) return
    if (rambleCanExit) await exitRamble()
    if (!(await saveDraftNow())) return
    if ($cookingEnabled && !cookedPreview) {
      await cookPreviewOnly()
      if (!cookedPreview) return
    }
    const requestId = item.id
    const submissionKey = workspaceRequestKey ?? requestId
    let submissionId = feedbackSubmissionIds.get(submissionKey)
    if (!submissionId) {
      submissionId = crypto.randomUUID()
      feedbackSubmissionIds.set(submissionKey, submissionId)
    }
    submitting = true
    submitStage = 'publishing'
    pageError = ''
    acpSnapshotEpoch += 1
    const submittedFeedback = {
      markdown: cookedPreview?.markdown ?? draftBody,
      uncooked_markdown: cookedPreview?.original ?? draftBody,
    }
    try {
      const next = await acpAdapter.submitFeedback({
        requestId,
        expectedRevision: savedRevision,
        documentJson: draftDocumentJson,
        bodyMarkdown: draftBody,
        submissionId,
        cookedMarkdown: cookedPreview?.markdown,
        cookingModel: cookedPreview?.model,
        uncookedMarkdown: cookedPreview?.original,
      })
      workspace = {
        ...workspace,
        request: {
          ...workspace.request,
          status: 'completed',
          resolution: 'feedback_submitted',
          updated_at: new Date().toISOString(),
        },
      }
      cookedPreview = null
      publishedFeedback = submittedFeedback
      savePhase = 'saved'
      applyAcpSnapshot(next)
      toast.success(tr('Feedback submitted'), {
        description: tr('The Agent can continue from this structured Feedback Package.'),
      })
      await reconcileManagedSelection(item.sessionId)
    } catch (cause) {
      pageError = acpAdapterErrorMessage(cause)
    } finally {
      submitting = false
      submitStage = 'idle'
    }
  }

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
      if (workspaceOrigin === 'managed_acp') {
        const sessionId = workspace.request.host_session_id
        acpSnapshotEpoch += 1
        const next = await acpAdapter.cancelFeedback(workspace.request.request_id)
        workspace = {
          ...workspace,
          request: {
            ...workspace.request,
            status: 'cancelled',
            resolution: 'cancelled',
            updated_at: new Date().toISOString(),
          },
        }
        savePhase = 'saved'
        applyAcpSnapshot(next)
        toast.success(tr('Request cancelled'))
        await reconcileManagedSelection(sessionId)
        return
      }
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

  function captureRambleOwner() {
    rambleOwner = currentWorkspaceOwner()
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
    requestOrigin={rambleOwner?.origin ?? workspaceOrigin}
    {rambleBelongsToWorkspace}
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
    onRefreshAttachmentPreviews={(next) => attachmentController.refreshPreviews(next, 'ramble')}
    onStartScreenCapture={attachmentController.startScreenCapture}
    onImportAttachmentPaths={attachmentController.importAttachmentPaths}
    onCaptureRambleOwner={captureRambleOwner}
    onRouteDraftOperation={(requestId, operation) =>
      routeDraftOperation(requestId, operation, rambleOwner)}
    getActiveAction={(requestId) => activeActionFor(requestId, rambleOwner)}
    artifactPort={rambleOwner?.origin === 'managed_acp'
      ? acpClipboardArtifactPortFor(rambleOwner.sessionId)
      : null}
  />

  <AppTitlebar
    sourceLabel={selectedSessionItem?.hostLabel ??
      workspace?.request.source_hint ?? workspace?.request.title ?? 'Workbench'}
    pendingCount={$navigation.pendingRequests.length +
      acpSnapshot.attentionItems.filter((item) => item.status === 'waiting').length}
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
    <SessionRail
      bind:collapsed={hostSessionRailCollapsed}
      items={unifiedWorkbench.sessions}
      activeKey={activeSessionKey}
      requestSearch={$navigation.requestSearch}
      loading={$navigation.loadingNavigation || (acpEnabled && acpLoading)}
      refreshing={$navigation.refreshingPage || acpRefreshing}
      onSelect={(item) => void chooseUnifiedSession(item)}
      onRequestSearch={(search) => void navigation.setRequestSearch(search)}
      onLaunch={() => acpEnabled
        ? (launchRambleOpen = true)
        : void openSettings('acp-client')}
      onRenameSession={renameUnifiedSession}
      onSetSessionPinned={setUnifiedSessionPinned}
      onArchiveSession={archiveUnifiedSession}
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
          {activeRequestKey}
          {cookingRequestKeys}
          scopeLabel={requestScopeLabel}
          searchQuery={$navigation.requestSearch}
          loading={$navigation.loadingRequests || (acpEnabled && acpLoading)}
          refreshing={$navigation.refreshingPage || acpRefreshing}
          loadingMore={$navigation.loadingMoreRequests}
          hasMore={selectedSessionItem?.origin === 'managed_acp' || todayOnly
            ? false
            : $navigation.nextRequestCursor !== null}
          {todayOnly}
          resolveAgentProfile={resolveRequestAgentProfile}
          formatTime={formatTimeLocal}
          onRefresh={() => void refreshUnifiedWorkbench()}
          onLoadMore={() => {
            if (selectedSessionItem?.origin !== 'managed_acp') {
              void navigation.loadMoreRequests()
            }
          }}
          onOpenRequest={(request) => void openUnifiedRequest(request)}
          onToggleToday={() => (todayOnly = !todayOnly)}
        />
      </Pane>

      <PaneResizer
        class="workbench-pane-resizer workbench-pane-resizer--vertical"
        aria-label={tr('Resize request list')}
      />

      <Pane id="workspace-pane" minSize={workspaceMinimumSize}>
        {#if selectedAcpItem?.kind === 'permission'}
          <PermissionView
            item={selectedAcpItem}
            busy={acpActionBusy}
            answerable={selectedAcpItemAnswerable}
            onAnswer={(option) => void answerAcpPermission(selectedAcpItem.id, option)}
          />
        {:else if selectedAcpItem?.kind === 'question'}
          <AskView
            item={selectedAcpItem}
            busy={acpActionBusy}
            answerable={selectedAcpItemAnswerable}
            onAnswer={(choiceIds, skipped) =>
              void answerAcpQuestion(selectedAcpItem, choiceIds, skipped)}
          />
        {:else}
          <WorkspacePanel
          bind:this={workspacePanel}
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
            ? activeActionByRequest.get(workspaceRequestKey ?? workspace.request.request_id)?.actionId ?? null
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
          resolveHostProfile={resolveWorkspaceProfile}
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
          readArtifactBytes={workspaceOrigin === 'managed_acp'
            ? (requestId, artifactId) => acpAdapter.readDraftArtifact(requestId, artifactId)
            : null}
          allowLocalArtifactActions={workspaceOrigin !== 'managed_acp'}
          />
        {/if}
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

{#if acpEnabled}
  <LaunchRambleDialog
    bind:open={launchRambleOpen}
    agents={acpSnapshot.agents}
    busy={acpActionBusy}
    error={acpLoadError}
    onPreflight={preflightAcpLaunch}
    onLaunch={(input) => void launchAcpRamble(input)}
  />
{/if}

<OnboardingWizard bind:openWizard={onboardingOpen} onClose={closeOnboarding} />

<ArchivedSessionsDialog
  bind:open={archivedSessionsOpen}
  {isTauri}
  {previewMode}
  {resolveHostProfile}
  formatTime={formatTimeLocal}
  {messageFrom}
  onError={(message) => (pageError = message)}
  onChanged={() => navigation.refreshNavigation(true)}
/>

{#if settingsOpen}
  <SettingsPanel
    mcpConfiguration={genericMcpConfiguration}
    initialSection={settingsSection}
    {updateInstallBlocked}
    onRestartOnboarding={restartOnboarding}
    onOpenArchived={openArchivedSessions}
    onClose={() => {
      settingsOpen = false
      void refreshNotificationPermission()
    }}
  />
{/if}

<UpdateAvailableDialog
  installBlocked={updateInstallBlocked}
  onOpenReleases={() => void openGithubReleases()}
/>
{/key}
