/** Opt-in production entry; drives the real App against the isolated HTTP fixture. */
import { acquireRun, editorSelector, longTitle, open, ordinaryTitle, releaseRun, requestButton, round, until } from './qualityBenchmarkDom'
import { installEditBenchmark } from './qualityEditBenchmark'
import { installManagedHistoryBenchmark } from './qualityManagedHistoryBenchmark'
import { installResourceBenchmark } from './qualityResourceBenchmark'
import { trackClientResources } from './qualityResourceTracker'

// These hooks must run before the dynamic import mounts the real App.
const resources = trackClientResources()
installResourceBenchmark(resources)
installEditBenchmark(resources)
installManagedHistoryBenchmark(resources)
const storageKey = 'rambledesk.quality-benchmark.v1'
type Result = {
  schema: 1
  startedAt: string
  userAgent: string
  viewport: { width: number; height: number }
  cache: string
  budgetsMs: { reloadMedian: number; ordinarySwitchP95: number; longSwitchP95: number }
  reloadMs: number[]
  ordinarySwitchMs: number[]
  longSwitchMs: number[]
  phase: 'reload' | 'switch' | 'done' | 'failed'
  error?: string
}
const benchmarkStatus = document.querySelector<HTMLOutputElement>('#quality-status')!
const output = document.querySelector<HTMLTextAreaElement>('#quality-results')!
const runButton = document.querySelector<HTMLButtonElement>('#quality-run')!
const read = (): Result | null => JSON.parse(sessionStorage.getItem(storageKey) ?? 'null')
function persist(result: Result) {
  sessionStorage.setItem(storageKey, JSON.stringify(result))
  output.value = JSON.stringify(result, null, 2)
  benchmarkStatus.textContent = `${result.phase}: reload ${result.reloadMs.length}/5, ordinary ${result.ordinarySwitchMs.length}/30, long ${result.longSwitchMs.length}/30${result.error ? ` · ${result.error}` : ''}`
}
async function resume(result: Result) {
  if (!acquireRun('reload-switch')) return
  try {
    await until(() => !!document.querySelector(editorSelector), 'editable workspace after reload')
    if (result.phase === 'reload') {
      // Navigation start to actual editable DOM plus two animation frames.
      // Auth/onboarding are completed before the run. Cache is explicitly uncontrolled.
      result.reloadMs.push(round(performance.now()))
      if (result.reloadMs.length < 5) {
        persist(result)
        location.reload()
        return
      }
      result.phase = 'switch'
    }
    for (let index = 0; index < 30; index += 1) {
      result.longSwitchMs.push(await open(longTitle, true))
      result.ordinarySwitchMs.push(await open(ordinaryTitle, false))
      persist(result)
    }
    result.phase = 'done'
  } catch (error) {
    result.phase = 'failed'
    result.error = error instanceof Error ? error.message : String(error)
  } finally {
    persist(result)
    releaseRun()
  }
}
runButton.onclick = async () => {
  if (!document.querySelector(editorSelector) || !requestButton(longTitle)) {
    benchmarkStatus.textContent = 'Open the ordinary fixture with the request list visible before measuring.'
    return
  }
  if (!acquireRun('reload-switch')) return
  try {
    await open(ordinaryTitle, false)
    persist({
      schema: 1, startedAt: new Date().toISOString(), userAgent: navigator.userAgent,
      viewport: { width: innerWidth, height: innerHeight },
      cache: 'Five fresh-document reloads; browser cache uncontrolled; fixture serves NoStore. Not native or OS-cold startup.',
      budgetsMs: { reloadMedian: 1500, ordinarySwitchP95: 300, longSwitchP95: 500 },
      reloadMs: [], ordinarySwitchMs: [], longSwitchMs: [], phase: 'reload',
    })
    location.reload()
  } catch (error) { benchmarkStatus.textContent = String(error); releaseRun() }
}
document.querySelector<HTMLButtonElement>('#quality-export')!.onclick = () => {
  const result = read()
  if (!result) return
  resources.exportJson(result, 'rambledesk-quality-benchmark.json')
}
const pending = read()
if (pending) persist(pending)
void import('../main').then(() => {
  if (pending?.phase === 'reload' || pending?.phase === 'switch') void resume(pending)
})

export {}
