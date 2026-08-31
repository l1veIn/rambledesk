import type {
  AcpSessionSummary,
  AcpWorkbenchSnapshot,
  AttentionItem,
  LaunchConfigSelection,
  LaunchConfigValue,
  LaunchPreflight,
} from './types'

export type AcpWorkbenchSelection = {
  sessionId: string | null
  itemId: string | null
}

export function orderSessions(sessions: AcpSessionSummary[]): AcpSessionSummary[] {
  return [...sessions].sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
}

export function itemsForSession(items: AttentionItem[], sessionId: string | null): AttentionItem[] {
  if (!sessionId) return []
  const sessionItems = items.filter((item) => item.sessionId === sessionId)
  const waiting = sessionItems.filter((item) => item.status === 'waiting')
  const history = sessionItems
    .filter((item) => item.status !== 'waiting')
    .sort((left, right) => right.createdAt.localeCompare(left.createdAt))
  // The Desktop projection already orders live Permission and Ask requests by
  // their backend FIFO queue. Never reorder waiting items by wall-clock time.
  return [...waiting, ...history]
}

export function isAttentionItemAnswerable(items: AttentionItem[], itemId: string): boolean {
  const item = items.find((candidate) => candidate.id === itemId)
  if (!item || item.status !== 'waiting') return false
  if (item.kind === 'feedback') return true
  return items.find((candidate) => candidate.status === 'waiting' && candidate.kind === item.kind)?.id === itemId
}

export function reconcileSelection(
  snapshot: AcpWorkbenchSnapshot,
  selection: AcpWorkbenchSelection,
): AcpWorkbenchSelection {
  const sessions = orderSessions(snapshot.sessions)
  const sessionId = sessions.some((session) => session.sessionId === selection.sessionId)
    ? selection.sessionId
    : sessions[0]?.sessionId ?? null
  const items = itemsForSession(snapshot.attentionItems, sessionId)
  const itemId = items.some((item) => item.id === selection.itemId)
    ? selection.itemId
    : items[0]?.id ?? null
  return { sessionId, itemId }
}

export function selectSession(
  snapshot: AcpWorkbenchSnapshot,
  sessionId: string,
): AcpWorkbenchSelection {
  return {
    sessionId,
    itemId: itemsForSession(snapshot.attentionItems, sessionId)[0]?.id ?? null,
  }
}

export function resolvePreflightSelection(
  preflight: LaunchPreflight,
  preferred: Record<string, LaunchConfigValue> = {},
): LaunchConfigSelection[] {
  const selections: LaunchConfigSelection[] = []
  for (const option of preflight.configOptions) {
    if (option.kind === 'unsupported') continue
    const candidate = preferred[option.id]
    if (option.kind === 'boolean') {
      selections.push({
        id: option.id,
        value: typeof candidate === 'boolean' ? candidate : option.currentValue,
      })
      continue
    }
    const selected = typeof candidate === 'string'
      && option.options.some((choice) => choice.value === candidate)
      ? candidate
      : option.options.some((choice) => choice.value === option.currentValue)
        ? option.currentValue
        : option.options[0]?.value
    if (selected !== undefined) selections.push({ id: option.id, value: selected })
  }
  return selections
}

export function launchConfigIsComplete(
  preflight: LaunchPreflight | null,
  selections: readonly LaunchConfigSelection[],
): boolean {
  if (!preflight) return false
  const selected = new Map(selections.map((selection) => [selection.id, selection.value]))
  return preflight.configOptions.every((option) => {
    if (option.kind === 'unsupported') return true
    const value = selected.get(option.id)
    if (option.kind === 'boolean') return typeof value === 'boolean'
    return typeof value === 'string'
      && option.options.some((choice) => choice.value === value)
  })
}

export function isUsablePreflight(preflight: LaunchPreflight | null): boolean {
  return preflight !== null
    && preflight.schemaDigest.length > 0
}

export type LaunchPreflightContext = {
  generation: number
  workspace: string
  agentId: string
}

export function isCurrentPreflightContext(
  expected: LaunchPreflightContext,
  current: LaunchPreflightContext,
): boolean {
  return expected.generation === current.generation
    && expected.workspace === current.workspace
    && expected.agentId === current.agentId
}
