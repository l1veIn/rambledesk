import { acquireRun, environment, releaseRun, restoreResult, round, until } from './qualityBenchmarkDom'
import { earlierButton, exactWindow, expandLatestProcess, expandLatestTool, historyTab, historyTitle, historyWindow, neighborTitle, observeHistoryHttp, openHistoryTab } from './qualityManagedHistoryDom'
import { createBenchmarkOutput } from './qualityBenchmarkOutput'
import type { ClientResourceTracker } from './qualityResourceTracker'

const storageKey = 'rambledesk.quality-managed-history.v1'
export function installManagedHistoryBenchmark(resources: ClientResourceTracker) {
  const output = document.querySelector<HTMLTextAreaElement>('#quality-history-results')!
  const status = document.querySelector<HTMLOutputElement>('#quality-history-status')!
  restoreResult(storageKey, output)
  document.querySelector<HTMLButtonElement>('#quality-history-export')!.onclick = () => {
    if (output.value) resources.exportJson(JSON.parse(output.value), 'rambledesk-quality-managed-history.json')
  }
  document.querySelector<HTMLButtonElement>('#quality-history-run')!.onclick = async () => {
    if (!acquireRun('managed-history')) return
    const http = observeHistoryHttp()
    const report = createBenchmarkOutput(storageKey, output, status)
    report.begin()
    const result = {
      schema: 1, harnessRevision: 'managed-history-v3-output-control', outputMode: report.mode, startedAt: new Date().toISOString(), ...environment(),
      fixture: 'rambledesk-managed-history-v1', phase: 'running',
      budgetsMs: { enterP95: 500, processExpandP95: 200, toolExpandP95: 200, historyHttpPageVisibleP95: 750 },
      method: 'Real keyed Agent tab switch, exact initial DOM user/turn window 041–060, actual process and tool expansion. HTTP page samples require a new Resource Timing fetch with transferred bytes for listManagedSessionActivity and exact rendered window 021–060. Each iteration switches through the neighboring Agent tab to recreate session history state. No store, editor or transport shortcuts; no model calls.',
      initialWindow: null as ReturnType<typeof historyWindow>,
      entryMs: [] as number[], processExpandMs: [] as number[], toolExpandMs: [] as number[],
      pages: [] as Array<Record<string, unknown>>, cycles: [] as Array<Record<string, unknown>>,
      pagination: { phase: http.supported ? 'pending' : 'unmeasured', reason: http.supported ? '' : 'Resource Timing observation is unavailable; HTTP pagination cannot be proved.' },
      hiddenTransitions: 0, error: undefined as string | undefined,
    }
    const visibilityChanged = () => { if (document.hidden) result.hiddenTransitions += 1 }
    document.addEventListener('visibilitychange', visibilityChanged)
    const label = () => `${result.phase}: enter ${result.entryMs.length}/30, process ${result.processExpandMs.length}/30, tool ${result.toolExpandMs.length}/30, HTTP page ${result.pages.length}/30${result.error ? ` \u00b7 ${result.error}` : ''}`
    const persist = () => report.progress(result, {
      schema: result.schema, harnessRevision: result.harnessRevision, startedAt: result.startedAt, phase: result.phase,
      counts: { entry: result.entryMs.length, process: result.processExpandMs.length, tool: result.toolExpandMs.length, pages: result.pages.length },
    }, label())
    try {
      persist()
      if (document.hidden) throw new Error('Keep this benchmark document foreground.')
      historyTab(historyTitle)
      historyTab(neighborTitle)
      await openHistoryTab(neighborTitle)
      for (let index = 1; index <= 30; index += 1) {
        const navigationStarted = performance.now()
        await openHistoryTab(historyTitle)
        await until(() => exactWindow(41, 60), 'fresh recent100 window: exactly user/Agent turns 041–060')
        const enterMs = round(performance.now() - navigationStarted)
        const before = historyWindow()!
        if (result.initialWindow && before.sessionId !== result.initialWindow.sessionId) {
          throw new Error('The observed history session changed during this benchmark.')
        }
        result.initialWindow ??= before
        result.entryMs.push(enterMs)
        const processMs = await expandLatestProcess()
        result.processExpandMs.push(processMs)
        const toolMs = await expandLatestTool()
        result.toolExpandMs.push(toolMs)
        const cycle: Record<string, unknown> = { index, enterMs, processMs, toolMs, before: { sessionId: before.sessionId, firstUserTurn: before.firstUserTurn, lastUserTurn: before.lastUserTurn, userCount: before.users.length }, enterHttp: http.since(navigationStarted) }
        result.cycles.push(cycle)
        persist()
        if (result.pagination.phase === 'pending' || result.pagination.phase === 'measured') {
          if (!exactWindow(41, 60)) throw new Error('History changed before pagination; cached/local history must not be measured as a fresh HTTP page.')
          const button = earlierButton()
          const started = performance.now()
          button.click()
          await until(() => exactWindow(21, 60), 'new page actually rendered: exactly user/Agent turns 021–060')
          const visibleMs = round(performance.now() - started)
          const requests = http.since(started).filter((entry) => entry.path.endsWith('/listManagedSessionActivity'))
          if (requests.length === 1 && requests[0].transferSize > 0
            && (requests[0].responseStatus === null || requests[0].responseStatus === 200)) {
            const page = { index, visibleMs, before, after: historyWindow(), http: requests[0] }
            result.pages.push(page)
            result.pagination.phase = 'measured'
            cycle.pageIndex = result.pages.length - 1
          } else {
            result.pagination.phase = result.pages.length ? 'partial' : 'unmeasured'
            result.pagination.reason = 'Rendered older content, but Resource Timing did not prove exactly one transferred successful HTTP response. Further pagination samples disabled.'
            cycle.unverifiedPage = { visibleMs, after: historyWindow(), http: requests }
          }
          persist()
        }
        await openHistoryTab(neighborTitle)
      }
      result.phase = result.hiddenTransitions ? 'done-with-visibility-interruption'
        : result.pages.length === 30 ? 'done' : 'done-with-unmeasured-pagination'
    } catch (error) {
      result.phase = result.entryMs.length === 0 ? 'setup-required' : 'failed'
      result.error = error instanceof Error ? error.message : String(error)
    } finally {
      report.finish(result, label())
      http.disconnect()
      document.removeEventListener('visibilitychange', visibilityChanged)
      releaseRun()
    }
  }
}
