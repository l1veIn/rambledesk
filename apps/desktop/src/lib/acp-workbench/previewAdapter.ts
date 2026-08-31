import type { AcpWorkbenchAdapter } from './adapter'
import type {
  AcpSessionSummary,
  AcpWorkbenchSnapshot,
  AttentionItem,
  DraftArtifactV3,
  DraftSnapshotV3,
  LaunchDraft,
  LaunchPreflight,
} from './types'

const now = '2026-08-30T09:42:00Z'

export const acpPreviewSnapshot: AcpWorkbenchSnapshot = {
  agents: [
    {
      id: 'codex',
      label: 'Codex',
      iconSvg: '',
      supportsStructuredRamble: true,
      models: ['gpt-5.6-sol', 'gpt-5.6-terra'],
      reasoningEfforts: ['medium', 'high', 'xhigh'],
    },
    {
      id: 'claude_code',
      label: 'Claude Code',
      iconSvg: '',
      supportsStructuredRamble: true,
      models: ['claude-sonnet-4.5'],
      reasoningEfforts: ['medium', 'high'],
    },
  ],
  sessions: [
    {
      sessionId: 'session-acp-first',
      title: 'Managed ACP Workbench',
      agentId: 'codex',
      agentLabel: 'Codex',
      workspace: '/Users/demo/Projects/rambledesk',
      model: 'gpt-5.6-sol',
      reasoningEffort: 'high',
      accessMode: 'workspace_write',
      status: 'waiting',
      pendingCount: 3,
      pinnedAt: null,
      archivedAt: null,
      updatedAt: now,
    },
    {
      sessionId: 'session-release',
      title: 'Release verification',
      agentId: 'claude',
      agentLabel: 'Claude Code',
      workspace: '/Users/demo/Projects/rambledesk',
      model: 'claude-sonnet-4.5',
      reasoningEffort: 'high',
      accessMode: 'read_only',
      status: 'offline',
      pendingCount: 0,
      pinnedAt: null,
      archivedAt: null,
      updatedAt: '2026-08-29T16:05:00Z',
    },
  ],
  attentionItems: [
    {
      id: 'feedback-navigation',
      sessionId: 'session-acp-first',
      kind: 'feedback',
      title: '体验新的三栏工作台',
      summary: 'Agent 已完成第一轮 Desktop 界面实现，需要你从真实使用角度评估。',
      instructions: '依次查看 Session 导航、统一 Inbox，以及三种请求 View。重点记录哪里仍像“开发工具”，而不是自然的人类工作台。',
      actions: ['启动一个新 Ramble', '处理一条授权请求', '回答一条多选问题'],
      draftDocument: null,
      draftMarkdown: '三栏的职责已经很清楚。\n\n我希望 Inbox 的请求类型再容易区分一点。',
      draftRevision: 2,
      status: 'waiting',
      createdAt: now,
    },
    {
      id: 'permission-run-checks',
      sessionId: 'session-acp-first',
      kind: 'permission',
      title: '允许执行项目检查？',
      description: 'Codex 请求执行工作区内的只读检查。这是 Agent 原始 Permission Request 的直接投影。',
      toolTitle: '运行命令',
      command: 'pnpm check && cargo test --workspace',
      path: '/Users/demo/Projects/rambledesk',
      toolCall: {
        toolCallId: 'tool-call-checks',
        title: '运行命令',
        kind: 'execute',
        rawInput: { command: 'pnpm check && cargo test --workspace' },
      },
      options: [
        { id: 'allow_once', label: '允许一次', tone: 'allow' },
        { id: 'allow_session', label: '本 Session 允许', tone: 'neutral' },
        { id: 'deny', label: '拒绝', tone: 'deny' },
      ],
      status: 'waiting',
      createdAt: '2026-08-30T09:38:00Z',
    },
    {
      id: 'question-density',
      sessionId: 'session-acp-first',
      kind: 'question',
      title: 'Inbox 信息密度',
      prompt: '第二栏的请求卡片，你希望默认展示哪些信息？',
      choices: [
        { id: 'type_time', label: '类型与时间', description: '保持最轻量，正文进入右栏查看。' },
        { id: 'summary', label: '一行摘要', description: '增加一点上下文，仍然适合快速扫描。' },
        { id: 'agent', label: 'Agent 状态', description: '同时展示 Agent 是否仍在等待。' },
      ],
      multiple: true,
      allowSkip: true,
      status: 'waiting',
      createdAt: '2026-08-30T09:35:00Z',
    },
    {
      id: 'feedback-release',
      sessionId: 'session-release',
      kind: 'feedback',
      title: 'Release notes review',
      summary: 'Completed feedback remains visible with the Session.',
      instructions: 'Review the release notes.',
      actions: [],
      draftDocument: null,
      draftMarkdown: 'Looks good.',
      draftRevision: 1,
      status: 'submitted',
      createdAt: '2026-08-29T15:40:00Z',
    },
  ],
}

function cloneSnapshot(snapshot: AcpWorkbenchSnapshot): AcpWorkbenchSnapshot {
  return structuredClone(snapshot)
}

function replaceItem(
  snapshot: AcpWorkbenchSnapshot,
  requestId: string,
  update: (item: AttentionItem) => AttentionItem,
) {
  snapshot.attentionItems = snapshot.attentionItems.map((item) =>
    item.id === requestId ? update(item) : item,
  )
  snapshot.sessions = snapshot.sessions.map((session) => ({
    ...session,
    pendingCount: snapshot.attentionItems.filter(
      (item) => item.sessionId === session.sessionId && item.status === 'waiting',
    ).length,
  }))
}

export function createPreviewAcpWorkbenchAdapter(): AcpWorkbenchAdapter {
  let snapshot = cloneSnapshot(acpPreviewSnapshot)
  let archivedSessions: AcpSessionSummary[] = []
  const artifacts = new Map<string, DraftArtifactV3[]>()
  const read = () => Promise.resolve(cloneSnapshot(snapshot))
  const draftFor = (requestId: string): DraftSnapshotV3 => {
    const item = snapshot.attentionItems.find(
      (candidate) => candidate.id === requestId && candidate.kind === 'feedback',
    )
    if (!item || item.kind !== 'feedback') throw new Error('Feedback Request not found')
    return {
      draftId: requestId,
      intent: 'feedback',
      sessionId: item.sessionId,
      requestId,
      documentJson: typeof item.draftDocument === 'string'
        ? item.draftDocument
        : JSON.stringify(item.draftDocument ?? { type: 'doc', content: [] }),
      bodyMarkdown: item.draftMarkdown,
      revision: item.draftRevision,
      artifacts: structuredClone(artifacts.get(requestId) ?? []),
      createdAt: item.createdAt,
      updatedAt: item.createdAt,
    }
  }
  const addPreviewArtifact = (
    requestId: string,
    fileName: string,
    mediaType: string,
    byteSize: number,
  ) => {
    const current = artifacts.get(requestId) ?? []
    current.push({
      artifactId: `preview-artifact-${current.length + 1}`,
      fileName,
      mediaType,
      byteSize,
      sha256: 'preview',
      position: current.length,
    })
    artifacts.set(requestId, current)
    replaceItem(snapshot, requestId, (item) => item.kind === 'feedback'
      ? { ...item, draftRevision: item.draftRevision + 1 }
      : item)
    return draftFor(requestId)
  }
  return {
    connectClient: async (agentId) => ({
      agentId,
      status: 'ready',
      reasonCode: null,
      reason: null,
      retryable: false,
    }),
    readWorkbench: read,
    renameSession: async (sessionId, title) => {
      snapshot.sessions = snapshot.sessions.map((session) =>
        session.sessionId === sessionId ? { ...session, title } : session)
      return cloneSnapshot(snapshot)
    },
    setSessionPinned: async (sessionId, pinned) => {
      snapshot.sessions = snapshot.sessions.map((session) =>
        session.sessionId === sessionId
          ? { ...session, pinnedAt: pinned ? new Date().toISOString() : null }
          : session)
      return cloneSnapshot(snapshot)
    },
    archiveSession: async (sessionId) => {
      const session = snapshot.sessions.find((candidate) => candidate.sessionId === sessionId)
      if (session) {
        archivedSessions = [{ ...session, archivedAt: new Date().toISOString() }, ...archivedSessions]
        snapshot.sessions = snapshot.sessions.filter((candidate) => candidate.sessionId !== sessionId)
      }
      return cloneSnapshot(snapshot)
    },
    unarchiveSession: async (sessionId) => {
      const session = archivedSessions.find((candidate) => candidate.sessionId === sessionId)
      if (session) {
        snapshot.sessions = [{ ...session, archivedAt: null }, ...snapshot.sessions]
        archivedSessions = archivedSessions.filter((candidate) => candidate.sessionId !== sessionId)
      }
      return cloneSnapshot(snapshot)
    },
    readArchivedSessions: async () => structuredClone(archivedSessions),
    readFeedback: async (requestId) => ({
      request: {},
      session: {},
      delivery: null,
      draft: draftFor(requestId),
      publishedFeedback: null,
    }),
    preflightLaunch: async (input: LaunchDraft): Promise<LaunchPreflight> => ({
      agentId: input.agentId,
      models: snapshot.agents.find((agent) => agent.id === input.agentId)?.models ?? [],
      reasoningEfforts:
        snapshot.agents.find((agent) => agent.id === input.agentId)?.reasoningEfforts ?? [],
      accessModes: ['read_only', 'workspace_write', 'yolo'],
      warning: null,
    }),
    launchRamble: async (input) => {
      const agent = snapshot.agents.find((candidate) => candidate.id === input.agentId)
      const sessionId = `preview-session-${snapshot.sessions.length + 1}`
      snapshot.sessions.unshift({
        sessionId,
        title: input.bodyMarkdown.trim().split('\n')[0]?.slice(0, 48) || 'New Ramble',
        agentId: input.agentId,
        agentLabel: agent?.label ?? input.agentId,
        workspace: input.workspace,
        model: input.model,
        reasoningEffort: input.reasoningEffort,
        accessMode: input.accessMode,
        status: 'running',
        pendingCount: 0,
        pinnedAt: null,
        archivedAt: null,
        updatedAt: new Date().toISOString(),
      })
      return cloneSnapshot(snapshot)
    },
    saveDraft: async (input) => {
      replaceItem(snapshot, input.requestId, (item) =>
        item.kind === 'feedback'
          ? {
              ...item,
              draftDocument: JSON.parse(input.documentJson) as unknown,
              draftMarkdown: input.bodyMarkdown,
              draftRevision: item.draftRevision + 1,
            }
          : item,
      )
      return cloneSnapshot(snapshot)
    },
    addDraftArtifact: async (input) => {
      return addPreviewArtifact(
        input.requestId,
        input.fileName,
        input.mediaType,
        input.contents.length,
      )
    },
    addDraftArtifactPath: async (input) => addPreviewArtifact(
      input.requestId,
      input.path.split('/').pop() || 'attachment',
      'application/octet-stream',
      0,
    ),
    addCompletedScreenCapture: async (input) => addPreviewArtifact(
      input.requestId,
      `ramble-screenshot-${input.captureSessionId}.png`,
      'image/png',
      0,
    ),
    addCompletedClipboardCapture: async (input) => addPreviewArtifact(
      input.requestId,
      input.fileName,
      'image/png',
      0,
    ),
    removeDraftArtifact: async (input) => {
      artifacts.set(
        input.requestId,
        (artifacts.get(input.requestId) ?? [])
          .filter((artifact) => artifact.artifactId !== input.artifactId)
          .map((artifact, position) => ({ ...artifact, position })),
      )
      return draftFor(input.requestId)
    },
    reorderDraftArtifacts: async (input) => {
      const byId = new Map(
        (artifacts.get(input.requestId) ?? []).map((artifact) => [artifact.artifactId, artifact]),
      )
      artifacts.set(
        input.requestId,
        input.artifactIds.flatMap((id, position) => {
          const artifact = byId.get(id)
          return artifact ? [{ ...artifact, position }] : []
        }),
      )
      return draftFor(input.requestId)
    },
    readDraftArtifact: async () => new ArrayBuffer(0),
    submitFeedback: async (input) => {
      replaceItem(snapshot, input.requestId, (item) =>
        item.kind === 'feedback'
          ? { ...item, draftMarkdown: input.bodyMarkdown, status: 'submitted' }
          : item,
      )
      return cloneSnapshot(snapshot)
    },
    cancelFeedback: async (requestId) => {
      replaceItem(snapshot, requestId, (item) => ({ ...item, status: 'cancelled' }))
      return cloneSnapshot(snapshot)
    },
    answerPermission: async (input) => {
      replaceItem(snapshot, input.requestId, (item) => ({ ...item, status: 'answered' }))
      return cloneSnapshot(snapshot)
    },
    answerQuestion: async (input) => {
      replaceItem(snapshot, input.requestId, (item) => ({ ...item, status: 'answered' }))
      return cloneSnapshot(snapshot)
    },
  }
}
