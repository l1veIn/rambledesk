import { acquireRun, delay, editorSelector, environment, fixtureVisible, longTitle, open, ordinaryTitle, prepareFixtures, releaseRun, restoreResult, round, until } from './qualityBenchmarkDom'
import { createBenchmarkOutput } from './qualityBenchmarkOutput'
import type { ClientResourceTracker } from './qualityResourceTracker'

const storageKey = 'rambledesk.quality-resources.v1'
const durationMs = 30 * 60_000
const sampleIntervalMs = 60_000
const cycleIntervalMs = 5_000
const settleMs = 3_000
function memory() {
  const value = (performance as Performance & { memory?: { usedJSHeapSize: number; totalJSHeapSize: number; jsHeapSizeLimit: number } }).memory
  return value ? { usedJSHeapSize: value.usedJSHeapSize, totalJSHeapSize: value.totalJSHeapSize, jsHeapSizeLimit: value.jsHeapSizeLimit } : null
}
function dom() {
  const app = document.querySelector('#app')!
  const walker = document.createTreeWalker(app, NodeFilter.SHOW_ALL)
  let appDomNodes = 1
  while (walker.nextNode()) appDomNodes += 1
  return {
    appDomNodes,
    appElements: app.querySelectorAll('*').length,
    attachedEditorViews: app.querySelectorAll('.ProseMirror').length,
    editableFeedbackViews: app.querySelectorAll(editorSelector).length,
  }
}
export function installResourceBenchmark(tracker: ClientResourceTracker) {
  const status = document.querySelector<HTMLOutputElement>('#quality-resource-status')!
  const output = document.querySelector<HTMLTextAreaElement>('#quality-resource-results')!
  const stop = document.querySelector<HTMLButtonElement>('#quality-resource-stop')!
  restoreResult(storageKey, output)
  document.querySelector<HTMLButtonElement>('#quality-resource-export')!.onclick = () => {
    if (output.value) tracker.exportJson(JSON.parse(output.value), 'rambledesk-quality-resources.json')
  }
  document.querySelector<HTMLButtonElement>('#quality-resource-run')!.onclick = async () => {
    if (!acquireRun('resources')) return
    const report = createBenchmarkOutput(storageKey, output, status)
    report.begin()
    let stopped = false
    let hiddenTransitions = 0
    const visibilityChanged = () => { if (document.hidden) hiddenTransitions += 1 }
    document.addEventListener('visibilitychange', visibilityChanged)
    stop.disabled = false
    stop.onclick = () => { stopped = true; status.textContent = 'Stopping after the current view settles…' }
    const result = {
      schema: 1, harnessRevision: 'stable-view-v3-output-control', outputMode: report.mode, kind: 'resources', startedAt: new Date().toISOString(), ...environment(),
      phase: 'running', durationMs, sampleIntervalMs, cycleIntervalMs, settleMs,
      budgets: { additionalOutstandingObjectUrls: 0, additionalWorkersWithoutTerminate: 0, attachedEditorViewGrowth: 0, editableFeedbackViews: 1, appElementGrowth: 0, appDomNodeGrowth: 0, heapGrowthInvestigationBytes: 10 * 1024 * 1024, heapGrowthInvestigationFraction: 0.1 },
      scope: 'This window only, instrumented before App import. Counts explicit revoke/terminate; Worker self-close, child workers, subscriptions, native processes, ASR, detached editor instances and memory outside this heap are unobserved. DOM switching only; no attachment modal or recording cycle.',
      samples: [] as Array<Record<string, unknown>>,
      cycles: [] as Array<{ elapsedMs: number; title: string; ms: number }>,
      hiddenTransitions: 0, error: undefined as string | undefined,
    }
    let started = performance.now()
    const label = () => `${result.phase}: ${round((performance.now() - started) / 60_000)} min, ${result.samples.length} snapshots, ${result.cycles.length} switches${result.error ? ` \u00b7 ${result.error}` : ''}`
    const persist = () => {
      result.hiddenTransitions = hiddenTransitions
      report.progress(result, {
        schema: result.schema, harnessRevision: result.harnessRevision, startedAt: result.startedAt, phase: result.phase,
        counts: { samples: result.samples.length, cycles: result.cycles.length }, hiddenTransitions,
      }, label())
    }
    const sample = async (stage: string) => {
      await until(() => fixtureVisible(ordinaryTitle, false),
        `stable ordinary view with one editable feedback editor before ${stage}`)
      const currentDom = dom()
      if (!fixtureVisible(ordinaryTitle, false)
        || currentDom.editableFeedbackViews !== 1) {
        throw new Error(`Invalid resource sample at ${stage}: ${JSON.stringify(currentDom)}`)
      }
      result.samples.push({ stage, elapsedMs: round(performance.now() - started), at: new Date().toISOString(), visibility: document.visibilityState, memory: memory(), ...currentDom, ...tracker.snapshot() })
      persist()
    }
    try {
      if (document.hidden) throw new Error('Keep the measurement document foreground; background throttling invalidates timing.')
      status.textContent = 'Warming both fixtures before the resource baseline…'
      await prepareFixtures()
      await delay(settleMs)
      started = performance.now()
      await sample('baseline-ordinary')
      let nextCycle = cycleIntervalMs
      let nextSample = sampleIntervalMs
      let showLong = true
      while (!stopped && performance.now() - started < durationMs) {
        const elapsed = performance.now() - started
        if (elapsed >= nextSample) {
          // Identical ordinary view for comparable DOM/resource snapshots.
          await open(ordinaryTitle, false)
          showLong = true
          await sample('minute-ordinary')
          nextSample = (Math.floor((performance.now() - started) / sampleIntervalMs) + 1) * sampleIntervalMs
        }
        if (elapsed >= nextCycle && !stopped) {
          const title = showLong ? longTitle : ordinaryTitle
          const ms = await open(title, showLong)
          result.cycles.push({ elapsedMs: round(performance.now() - started), title, ms })
          showLong = !showLong
          nextCycle = performance.now() - started + cycleIntervalMs
          persist()
        }
        await delay(250)
      }
      await open(ordinaryTitle, false)
      await delay(settleMs)
      await sample(stopped ? 'stopped-settled-ordinary' : 'final-settled-ordinary')
      result.phase = stopped ? 'stopped' : hiddenTransitions ? 'done-with-visibility-interruption' : 'done'
    } catch (error) {
      result.phase = 'failed'
      result.error = error instanceof Error ? error.message : String(error)
    } finally {
      result.hiddenTransitions = hiddenTransitions
      report.finish(result, label())
      document.removeEventListener('visibilitychange', visibilityChanged)
      stop.disabled = true
      stop.onclick = null
      releaseRun()
    }
  }
}
