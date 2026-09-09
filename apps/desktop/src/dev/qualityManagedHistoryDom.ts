/** DOM and Resource Timing only; no private session/controller or transport calls. */
import { round, until } from './qualityBenchmarkDom'

export const historyTitle = 'Website project'
export const neighborTitle = 'CLI project'
export function historyTab(title: string): HTMLElement {
  const matches = [...document.querySelectorAll<HTMLElement>('#app [role="tab"]')]
    .filter((node) => node.title === `${title} \u00b7 Agent`
      && node.dataset.workspaceViewKey?.startsWith('agent-session:'))
  if (matches.length !== 1) {
    throw new Error(`Benchmark setup: open ${title}, then use View Agent to create its actual Agent workspace tab; found ${matches.length} Agent tabs.`)
  }
  const tab = matches[0]
  if (tab.getAttribute('aria-disabled') === 'true') throw new Error(`Benchmark setup: workspace navigation is disabled: ${title}`)
  return tab
}
export function historyView(title: string): HTMLElement | null {
  const view = document.querySelector<HTMLElement>('#app [data-managed-session-id]')
  return view?.querySelector('header h2')?.textContent === title ? view : null
}
export function historyWindow() {
  const view = historyView(historyTitle)
  if (!view) return null
  const turns = [...view.querySelectorAll<HTMLElement>('[data-turn-id]')].map((node) => node.dataset.turnId!)
  const users = [...view.querySelectorAll<HTMLElement>('[data-activity-kind="user_message"]')].map((node) => ({
    id: node.dataset.activityId!,
    turn: Number(node.textContent?.match(/Quality history turn (\d{3}):/)?.[1] ?? -1),
  }))
  return { sessionId: view.dataset.managedSessionId!, turns, users, firstUserTurn: users[0]?.turn, lastUserTurn: users.at(-1)?.turn }
}
export function exactWindow(first: number, last: number) {
  const window = historyWindow()
  return !!window && window.users.length === last - first + 1 && window.turns.length === last - first + 1
    && window.users.every((row, index) => row.turn === first + index)
    && window.turns.every((id, index) => id === `quality-turn-${String(first + index).padStart(3, '0')}`)
}
export async function openHistoryTab(title: string) {
  const tab = historyTab(title)
  const started = performance.now()
  if (tab.getAttribute('aria-selected') !== 'true') tab.click()
  await until(() => !!historyView(title)?.dataset.managedSessionId
    && tab.isConnected && tab.getAttribute('aria-selected') === 'true'
    && tab.getAttribute('aria-disabled') !== 'true', `${title} managed history DOM and selected tab; a Ramble-only scope is not an Agent history view`)
  return round(performance.now() - started)
}
export async function expandLatestProcess() {
  const turn = historyView(historyTitle)?.querySelector<HTMLElement>('[data-turn-id="quality-turn-060"]')
  const toggle = turn?.querySelector<HTMLButtonElement>('header button[aria-expanded]')
  if (!turn || !toggle) throw new Error('Latest synthetic turn must expose a process toggle.')
  if (toggle.getAttribute('aria-expanded') === 'true') {
    toggle.click()
    await until(() => toggle.getAttribute('aria-expanded') === 'false' && !turn.querySelector('[data-turn-process]'), 'process collapsed before measurement')
  }
  const started = performance.now()
  toggle.click()
  await until(() => toggle.getAttribute('aria-expanded') === 'true'
    && !!turn.querySelector('[data-turn-process] [data-tool-id="quality-tool-060"]'), 'latest process and typed tool mounted')
  return round(performance.now() - started)
}
export async function expandLatestTool() {
  const tool = historyView(historyTitle)?.querySelector<HTMLDetailsElement>('[data-tool-id="quality-tool-060"]')
  const toggle = tool?.querySelector<HTMLElement>('summary')
  if (!tool || !toggle) throw new Error('Latest typed tool must be mounted before expansion.')
  if (tool.open) {
    toggle.click()
    await until(() => !tool.open && !tool.querySelector('section[aria-label="Input"]'), 'tool collapsed before measurement')
  }
  const started = performance.now()
  toggle.click()
  await until(() => tool.open && !!tool.querySelector('section[aria-label="Input"]')
    && tool.textContent?.includes('Synthetic fixture output 060.') === true, 'tool input and output content mounted')
  return round(performance.now() - started)
}
export function earlierButton() {
  const button = [...(historyView(historyTitle)?.querySelectorAll<HTMLButtonElement>('button') ?? [])]
    .find((node) => node.textContent?.trim() === 'Load earlier messages')
  if (!button || button.disabled) throw new Error('An enabled Load earlier messages button is required.')
  return button
}
export type HistoryHttp = { path: string; startTime: number; duration: number; transferSize: number; responseStatus: number | null }
export function observeHistoryHttp() {
  const entries: HistoryHttp[] = []
  const supported = typeof PerformanceObserver !== 'undefined' && PerformanceObserver.supportedEntryTypes.includes('resource')
  const observer = supported ? new PerformanceObserver((list) => {
    for (const entry of list.getEntries() as PerformanceResourceTiming[]) {
      const url = new URL(entry.name, location.href)
      if (url.origin !== location.origin || entry.initiatorType !== 'fetch'
        || !['/api/application/getManagedSession', '/api/application/listManagedSessionActivity'].includes(url.pathname)) continue
      entries.push({ path: url.pathname, startTime: entry.startTime, duration: round(entry.duration), transferSize: entry.transferSize,
        responseStatus: 'responseStatus' in entry ? Number(entry.responseStatus) : null })
    }
  }) : null
  observer?.observe({ entryTypes: ['resource'] })
  return { supported, since: (time: number) => entries.filter((entry) => entry.startTime >= time), disconnect: () => observer?.disconnect() }
}
