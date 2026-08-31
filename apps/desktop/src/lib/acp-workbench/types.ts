export type AgentSummary = {
  id: string
  label: string
  iconSvg: string
  supportsStructuredRamble: boolean
}

export type AccessMode = 'read_only' | 'workspace_write' | 'yolo'
export type SessionStatus = 'running' | 'waiting' | 'offline' | 'completed'

export type AcpSessionSummary = {
  sessionId: string
  title: string
  agentId: string
  agentLabel: string
  workspace: string
  model: string
  reasoningEffort: string
  accessMode: AccessMode
  status: SessionStatus
  pendingCount: number
  pinnedAt: string | null
  archivedAt: string | null
  updatedAt: string
}

type AttentionBase = {
  id: string
  sessionId: string
  title: string
  createdAt: string
  status: 'waiting' | 'answered' | 'submitted' | 'cancelled'
}

export type FeedbackAttentionItem = AttentionBase & {
  kind: 'feedback'
  updatedAt?: string
  summary: string
  instructions: string
  actions: string[]
  draftDocument: unknown | null
  draftMarkdown: string
  draftRevision: number
}

export type PermissionOption = {
  id: string
  label: string
  tone: 'allow' | 'deny' | 'neutral'
}

export type PermissionAttentionItem = AttentionBase & {
  kind: 'permission'
  description: string
  toolTitle: string
  command: string | null
  path: string | null
  toolCall: unknown
  options: PermissionOption[]
}

export type AcpClientReadiness = {
  agentId: string
  status: 'ready' | 'unavailable'
  reasonCode: string | null
  reason: string | null
  retryable: boolean
}

export type QuestionChoice = {
  id: string
  label: string
  description: string | null
}

export type QuestionAttentionItem = AttentionBase & {
  kind: 'question'
  prompt: string
  choices: QuestionChoice[]
  multiple: boolean
  allowSkip: boolean
  unsupportedReason?: string
}

export type AttentionItem =
  | FeedbackAttentionItem
  | PermissionAttentionItem
  | QuestionAttentionItem

export type AcpWorkbenchSnapshot = {
  sessions: AcpSessionSummary[]
  attentionItems: AttentionItem[]
  agents: AgentSummary[]
  /**
   * Ephemeral projection of the currently attached ACP runs. RambleDesk does
   * not promise that this survives a Desktop or Agent restart.
   */
  timelines?: SessionTimeline[]
}

export type TimelineEntryStatus = 'running' | 'completed' | 'failed' | 'waiting'
export type TimelineEntryKind = 'thought' | 'tool' | 'message' | 'status' | 'error'

export type TimelineEntry = {
  id: string
  kind: TimelineEntryKind
  title: string
  content: string
  status: TimelineEntryStatus
  createdAt: string
}

export type TimelineTurn = {
  turnId: string
  status: 'running' | 'completed' | 'failed'
  startedAt: string
  completedAt: string | null
  entries: TimelineEntry[]
}

export type SessionTimeline = {
  sessionId: string
  liveOnly: true
  turns: TimelineTurn[]
}

export type LaunchDraft = {
  submissionId: string
  workspace: string
  agentId: string
  schemaDigest: string
  configValues: LaunchConfigSelection[]
  documentJson: string
  bodyMarkdown: string
}

export type LaunchPreflightInput = {
  workspace: string
  agentId: string
}

export type LaunchConfigValue = string | boolean

export type LaunchConfigSelection = {
  id: string
  value: LaunchConfigValue
}

export type LaunchConfigChoice = {
  value: string
  name: string
  description: string | null
  /** Legacy preview convenience; native ACP uses `groups` on the option. */
  group?: string | null
}

export type LaunchConfigGroup = {
  id: string
  name: string
  options: LaunchConfigChoice[]
}

type LaunchConfigOptionBase = {
  id: string
  name: string
  description: string | null
  category: string | null
  source: 'agent' | 'profile'
}

export type LaunchSelectConfigOption = LaunchConfigOptionBase & {
  kind: 'select'
  currentValue: string
  options: LaunchConfigChoice[]
  groups?: LaunchConfigGroup[]
}

export type LaunchBooleanConfigOption = LaunchConfigOptionBase & {
  kind: 'boolean'
  currentValue: boolean
}

export type LaunchUnsupportedConfigOption = LaunchConfigOptionBase & {
  kind: 'unsupported'
  currentValue: unknown
  rawType: string
  raw?: unknown
}

export type LaunchConfigOption =
  | LaunchSelectConfigOption
  | LaunchBooleanConfigOption
  | LaunchUnsupportedConfigOption

export type LaunchPreflight = {
  agentId: string
  schemaDigest: string
  configOptions: LaunchConfigOption[]
  warning: string | null
}

export type DraftInput = {
  requestId: string
  expectedRevision: number
  documentJson: string
  bodyMarkdown: string
}

export type DraftArtifactV3 = {
  artifactId: string
  fileName: string
  mediaType: string
  byteSize: number
  sha256: string
  position: number
}

export type DraftSnapshotV3 = {
  draftId: string
  intent: 'launch' | 'feedback'
  sessionId: string | null
  requestId: string | null
  documentJson: string
  bodyMarkdown: string
  revision: number
  artifacts: DraftArtifactV3[]
  createdAt: string
  updatedAt: string
}

export type FeedbackDetailV3 = {
  request: unknown
  session: unknown
  delivery: unknown | null
  draft: DraftSnapshotV3 | null
  publishedFeedback: {
    markdown: string
    uncooked_markdown?: string
  } | null
}

export type AddDraftArtifactInput = {
  requestId: string
  expectedRevision: number
  fileName: string
  mediaType: string
  contents: number[]
}

export type RemoveDraftArtifactInput = {
  requestId: string
  artifactId: string
  expectedRevision: number
}

export type ReorderDraftArtifactsInput = {
  requestId: string
  artifactIds: string[]
  expectedRevision: number
}

export type AddDraftArtifactPathInput = {
  requestId: string
  path: string
  expectedRevision: number
}

export type AddCompletedScreenCaptureInput = {
  requestId: string
  captureSessionId: string
  expectedRevision: number
}

export type AddCompletedClipboardCaptureInput = {
  requestId: string
  captureId: string
  rambleContextId: string
  fileName: string
  expectedRevision: number
}

export type FeedbackDecisionInput = DraftInput & {
  submissionId: string
  cookedMarkdown?: string
  cookingModel?: string
  uncookedMarkdown?: string
}

export type PermissionAnswerInput = {
  requestId: string
  optionId: string
}

export type QuestionAnswerInput = {
  requestId: string
  choiceIds: string[]
  skipped: boolean
}
