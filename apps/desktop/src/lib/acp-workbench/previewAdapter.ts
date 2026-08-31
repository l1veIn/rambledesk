import type { AcpWorkbenchAdapter } from './adapter'
import type {
  AcpSessionSummary,
  AcpWorkbenchSnapshot,
  AttentionItem,
  DraftArtifactV3,
  DraftSnapshotV3,
  LaunchPreflightInput,
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
    },
    {
      id: 'claude_code',
      label: 'Claude Code',
      iconSvg: '',
      supportsStructuredRamble: true,
    },
    {
      id: 'deepseek',
      label: 'DeepSeek Harness',
      iconSvg: '',
      supportsStructuredRamble: true,
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
  timelines: [
    {
      sessionId: 'session-acp-first',
      liveOnly: true,
      turns: [
        {
          turnId: 'turn-1',
          status: 'completed',
          startedAt: '2026-08-30T09:20:00Z',
          completedAt: '2026-08-30T09:27:00Z',
          entries: [
            {
              id: 'thought-1', kind: 'thought', title: '分析现有三栏布局',
              content: '检查 Session、请求列表和反馈工作区的职责边界。',
              status: 'completed', createdAt: '2026-08-30T09:20:00Z',
            },
            {
              id: 'tool-1', kind: 'tool', title: '读取工作台组件',
              content: '查看现有导航和正文交互，避免重写成熟区域。',
              status: 'completed', createdAt: '2026-08-30T09:22:00Z',
            },
          ],
        },
        {
          turnId: 'turn-2',
          status: 'completed',
          startedAt: '2026-08-30T09:28:00Z',
          completedAt: '2026-08-30T09:42:00Z',
          entries: [
            {
              id: 'thought-2', kind: 'thought', title: '收敛 ACP 请求投影',
              content: '把 Agent 工作过程保持在 Timeline，把结构化请求交给 RambleDesk 工作区。',
              status: 'completed', createdAt: '2026-08-30T09:30:00Z',
            },
            {
              id: 'tool-2', kind: 'tool', title: '更新 Desktop 工作台',
              content: 'Session 导航与 Permission、Ask Question、Feedback Request 已接入。',
              status: 'completed', createdAt: '2026-08-30T09:36:00Z',
            },
          ],
        },
      ],
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
    preflightLaunch: async (input: LaunchPreflightInput): Promise<LaunchPreflight> => ({
      agentId: input.agentId,
      schemaDigest: `${input.agentId}:${input.workspace}`,
      configOptions: input.agentId === 'codex'
        ? [
            {
              id: 'model', name: 'Model', description: null, category: 'model', source: 'agent',
              kind: 'select', currentValue: 'gpt-5.6-sol', options: [
                { value: 'gpt-5.6-sol', name: 'gpt-5.6-sol', description: null, group: 'OpenAI' },
                { value: 'gpt-5.6-terra', name: 'gpt-5.6-terra', description: null, group: 'OpenAI' },
              ],
            },
            {
              id: 'reasoning_effort', name: 'Reasoning effort', description: null,
              category: 'thought_level', source: 'agent', kind: 'select', currentValue: 'high',
              options: ['medium', 'high', 'xhigh'].map((value) => ({
                value, name: value, description: null, group: null,
              })),
            },
            {
              id: 'rambledesk.profile.access_mode', name: 'Access permission',
              description: 'Controls file and command access for this Agent process.',
              category: 'access_mode', source: 'profile', kind: 'select',
              currentValue: 'workspace_write',
              options: ['read_only', 'workspace_write', 'yolo'].map((value) => ({
                value, name: value, description: null, group: null,
              })),
            },
            {
              id: 'fast_mode', name: 'Fast mode', description: 'Use lower-latency responses when available.',
              category: null, source: 'agent', kind: 'boolean', currentValue: false,
            },
          ] as LaunchPreflight['configOptions']
        : input.agentId === 'deepseek'
          ? [
              {
                id: 'model', name: 'Model', description: null, category: 'model', source: 'agent',
                kind: 'select', currentValue: 'DeepSeek-V4-Flash-Vision-Exp', options: [
                  {
                    value: 'DeepSeek-V4-Flash-Vision-Exp',
                    name: 'DeepSeek-V4-Flash-Vision-Exp',
                    description: null,
                    group: null,
                  },
                ],
              },
              {
                id: 'reasoning_effort', name: 'Reasoning profile',
                description: 'Higher settings use more tokens for harder work.',
                category: 'thought_level', source: 'agent', kind: 'select', currentValue: 'highest',
                options: ['balanced', 'high', 'highest'].map((value) => ({
                  value, name: value, description: null, group: null,
                })),
              },
              {
                id: 'rambledesk.profile.access_mode', name: 'File permission',
                description: 'Commands and file tools share this boundary.',
                category: 'access_mode', source: 'profile', kind: 'select', currentValue: 'read_only',
                options: ['read_only', 'workspace_write', 'yolo'].map((value) => ({
                  value, name: value, description: null, group: null,
                })),
              },
            ] as LaunchPreflight['configOptions']
          : [
            {
              id: 'model', name: 'Model', description: null, category: 'model', source: 'agent',
              kind: 'select', currentValue: 'claude-sonnet-4.5', options: [
                { value: 'claude-sonnet-4.5', name: 'Claude Sonnet 4.5', description: null, group: null },
              ],
            },
            {
              id: 'rambledesk.profile.access_mode', name: 'Access permission', description: null,
              category: 'access_mode', source: 'profile', kind: 'select', currentValue: 'workspace_write',
              options: ['read_only', 'workspace_write'].map((value) => ({
                value, name: value, description: null, group: null,
              })),
            },
            ] as LaunchPreflight['configOptions'],
      warning: null,
    }),
    launchRamble: async (input) => {
      const agent = snapshot.agents.find((candidate) => candidate.id === input.agentId)
      const sessionId = `preview-session-${snapshot.sessions.length + 1}`
      const selections = new Map(input.configValues.map((selection) => [selection.id, selection.value]))
      const access = selections.get('rambledesk.profile.access_mode')
      snapshot.sessions.unshift({
        sessionId,
        title: input.bodyMarkdown.trim().split('\n')[0]?.slice(0, 48) || 'New Ramble',
        agentId: input.agentId,
        agentLabel: agent?.label ?? input.agentId,
        workspace: input.workspace,
        model: typeof selections.get('model') === 'string' ? String(selections.get('model')) : '',
        reasoningEffort: typeof selections.get('reasoning_effort') === 'string'
          ? String(selections.get('reasoning_effort')) : '',
        accessMode: access === 'read_only' || access === 'yolo' ? access : 'workspace_write',
        status: 'running',
        pendingCount: 0,
        pinnedAt: null,
        archivedAt: null,
        updatedAt: new Date().toISOString(),
      })
      snapshot.timelines = [
        {
          sessionId,
          liveOnly: true,
          turns: [
            {
              turnId: `${sessionId}:turn-1`,
              status: 'running',
              startedAt: new Date().toISOString(),
              completedAt: null,
              entries: [
                {
                  id: `${sessionId}:status-1`,
                  kind: 'status',
                  title: 'Agent Session 已启动',
                  content: 'Agent 正在准备第一条结构化 Feedback Request。',
                  status: 'running',
                  createdAt: new Date().toISOString(),
                },
              ],
            },
          ],
        },
        ...(snapshot.timelines ?? []),
      ]
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
