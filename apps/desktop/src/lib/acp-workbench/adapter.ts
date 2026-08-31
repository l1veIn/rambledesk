import { invoke } from '@tauri-apps/api/core'

import type {
  AcpSessionSummary,
  AcpWorkbenchSnapshot,
  AcpClientReadiness,
  AddDraftArtifactInput,
  AddDraftArtifactPathInput,
  AddCompletedClipboardCaptureInput,
  AddCompletedScreenCaptureInput,
  DraftInput,
  DraftSnapshotV3,
  FeedbackDecisionInput,
  FeedbackDetailV3,
  LaunchDraft,
  LaunchPreflightInput,
  LaunchPreflight,
  PermissionAnswerInput,
  QuestionAnswerInput,
  RemoveDraftArtifactInput,
  ReorderDraftArtifactsInput,
} from './types'

export interface AcpWorkbenchAdapter {
  connectClient(agentId: string): Promise<AcpClientReadiness>
  readWorkbench(): Promise<AcpWorkbenchSnapshot>
  renameSession(sessionId: string, title: string): Promise<AcpWorkbenchSnapshot>
  setSessionPinned(sessionId: string, pinned: boolean): Promise<AcpWorkbenchSnapshot>
  archiveSession(sessionId: string): Promise<AcpWorkbenchSnapshot>
  unarchiveSession(sessionId: string): Promise<AcpWorkbenchSnapshot>
  readArchivedSessions(): Promise<AcpSessionSummary[]>
  readFeedback(requestId: string): Promise<FeedbackDetailV3>
  preflightLaunch(input: LaunchPreflightInput): Promise<LaunchPreflight>
  launchRamble(input: LaunchDraft): Promise<AcpWorkbenchSnapshot>
  saveDraft(input: DraftInput): Promise<AcpWorkbenchSnapshot>
  addDraftArtifact(input: AddDraftArtifactInput): Promise<DraftSnapshotV3>
  addDraftArtifactPath(input: AddDraftArtifactPathInput): Promise<DraftSnapshotV3>
  addCompletedScreenCapture(input: AddCompletedScreenCaptureInput): Promise<DraftSnapshotV3>
  addCompletedClipboardCapture(input: AddCompletedClipboardCaptureInput): Promise<DraftSnapshotV3>
  removeDraftArtifact(input: RemoveDraftArtifactInput): Promise<DraftSnapshotV3>
  reorderDraftArtifacts(input: ReorderDraftArtifactsInput): Promise<DraftSnapshotV3>
  readDraftArtifact(requestId: string, artifactId: string): Promise<ArrayBuffer>
  submitFeedback(input: FeedbackDecisionInput): Promise<AcpWorkbenchSnapshot>
  cancelFeedback(requestId: string): Promise<AcpWorkbenchSnapshot>
  answerPermission(input: PermissionAnswerInput): Promise<AcpWorkbenchSnapshot>
  answerQuestion(input: QuestionAnswerInput): Promise<AcpWorkbenchSnapshot>
}

/**
 * The only Desktop seam that knows Tauri command names. The backend may change
 * its wire DTO without leaking that churn into the Workbench implementation.
 */
export function createNativeAcpWorkbenchAdapter(): AcpWorkbenchAdapter {
  return {
    connectClient: (agentId) => invoke('connect_acp_client', { agentId }),
    readWorkbench: () => invoke('read_acp_workbench'),
    renameSession: (sessionId, title) =>
      invoke('rename_acp_session_v3', { input: { sessionId, title } }),
    setSessionPinned: (sessionId, pinned) =>
      invoke('set_acp_session_pinned_v3', { input: { sessionId, pinned } }),
    archiveSession: (sessionId) => invoke('archive_acp_session_v3', { sessionId }),
    unarchiveSession: (sessionId) => invoke('unarchive_acp_session_v3', { sessionId }),
    readArchivedSessions: () => invoke('read_archived_acp_sessions_v3'),
    readFeedback: (requestId) => invoke('read_feedback_v3', { requestId }),
    preflightLaunch: (input) => invoke('preflight_acp_launch', { input }),
    launchRamble: (input) => invoke('launch_ramble_v3', { input }),
    saveDraft: (input) => invoke('save_ramble_draft_v3', { input }),
    addDraftArtifact: (input) => invoke('add_feedback_draft_artifact_v3', { input }),
    addDraftArtifactPath: (input) => invoke('import_feedback_draft_artifact_path_v3', input),
    addCompletedScreenCapture: (input) => invoke('add_completed_screen_capture_v3', input),
    addCompletedClipboardCapture: (input) => invoke('add_completed_clipboard_capture_v3', input),
    removeDraftArtifact: (input) => invoke('remove_feedback_draft_artifact_v3', { input }),
    reorderDraftArtifacts: (input) => invoke('reorder_feedback_draft_artifacts_v3', { input }),
    readDraftArtifact: (requestId, artifactId) =>
      invoke('read_feedback_draft_artifact_v3', { requestId, artifactId }),
    submitFeedback: (input) => invoke('submit_feedback_v3', { input }),
    cancelFeedback: (requestId) => invoke('cancel_feedback_v3', { requestId }),
    answerPermission: (input) => invoke('answer_acp_permission', { input }),
    answerQuestion: (input) => invoke('answer_acp_question', { input }),
  }
}

export function acpAdapterErrorMessage(cause: unknown): string {
  const code = cause && typeof cause === 'object' && 'code' in cause
    ? String((cause as { code: unknown }).code)
    : ''
  const raw =
    cause instanceof Error
      ? cause.message
      : cause && typeof cause === 'object' && 'message' in cause
        ? String((cause as { message: unknown }).message)
        : String(cause)
  if (code === 'ACP_CLIENT_UNAVAILABLE' || /command .* not found|unknown command/i.test(raw)) {
    return 'ACP Client 尚未接入当前 Desktop 构建。界面没有提交或伪造任何结果。'
  }
  if (code === 'ACP_RUNTIME_MISSING') {
    return '缺少这个 Agent 所需的 Node.js。请先安装 Node.js，重启 RambleDesk 后再试。'
  }
  if (code === 'ACP_PLATFORM_UNSUPPORTED') {
    return '这个 Agent 暂不支持当前系统或处理器。'
  }
  if (code === 'ACP_INSTALL_FAILED') {
    return `Agent Client 安装失败：${raw}`
  }
  if (code === 'ACP_AUTHENTICATION_REQUIRED') {
    return 'Agent Client 已连接，但当前账号尚未登录、缺少许可或无权使用。处理账号状态后再点一次连接。'
  }
  if (code === 'ACP_SESSION_TOOLSET_UNSUPPORTED') {
    return '这个 Agent 可以通过 ACP 连接，但当前不能接收 RambleDesk 的结构化反馈工具，因此暂时不能启动 ACP Ramble。'
  }
  if (code === 'ACP_AGENT_LAUNCH_FAILED') {
    return `ACP Agent 没有成功启动：${raw}`
  }
  if (code === 'ACP_OPERATION_TIMED_OUT') {
    return '准备 ACP Agent 超时。首次安装可能较慢，请检查网络后重试。'
  }
  if (code === 'ACP_PROTOCOL_VIOLATION' || code === 'ACP_RPC_ERROR') {
    return `ACP Agent 已启动，但协议握手失败：${raw}`
  }
  return raw
}
