import { agentText } from '$lib/agents/agentI18n'
import type { HostProfile } from '$lib/domain/hostProfile'
import type { HostSessionSummary } from '$lib/feedback'
import type { Locale } from '$lib/preferences'
import type { SessionViewDescriptor, WorkspaceViewDescriptor } from './viewDescriptors'

export type TabLabelContext = Readonly<{
  hostSessions: readonly HostSessionSummary[]
  resolveHostProfile: (hostId: string) => HostProfile
  taskTabTitles: ReadonlyMap<string, string>
  locale: Locale
  tr: (source: string) => string
}>

/** `Session title · Host` for a session tab. */
export function sessionTabLabel(
  view: SessionViewDescriptor,
  context: Pick<TabLabelContext, 'hostSessions' | 'resolveHostProfile'>,
): string {
  const session = context.hostSessions.find(
    (candidate) =>
      candidate.host_id === view.hostId && candidate.host_session_id === view.hostSessionId,
  )
  return `${session?.title ?? view.hostSessionId} · ${context.resolveHostProfile(view.hostId).label}`
}

export function workspaceTabLabel(
  view: WorkspaceViewDescriptor,
  context: TabLabelContext,
): string {
  switch (view.kind) {
    case 'agent-draft':
      return context.locale === 'zh-CN' ? '新建会话' : 'New session'
    case 'inbox':
      return context.tr('All requests')
    case 'archive':
      return context.tr('Archived sessions')
    case 'settings':
      return context.tr('Settings')
    case 'agent-session': {
      const session = context.hostSessions.find(
        (candidate) => candidate.session_id === view.sessionId,
      )
      return session
        ? `${session.title} · Agent`
        : agentText(context.locale, 'Agent session')
    }
    case 'request-task':
      return context.taskTabTitles.get(view.requestId) ?? context.tr('Task brief')
    case 'rambelle-profile':
      return 'Rambelle'
    case 'session':
      return sessionTabLabel(view, context)
  }
}
