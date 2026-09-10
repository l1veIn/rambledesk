import { acquireRun, editorSelector, environment, longTitle, open, ordinaryTitle, prepareFixtures, releaseRun, restoreResult, round, until } from './qualityBenchmarkDom'
import type { ClientResourceTracker } from './qualityResourceTracker'

const storageKey = 'rambledesk.quality-edit.v1'
function savedRevision(editor: Element): number | null {
  const badge = editor.closest('section')?.querySelector('footer [aria-live="polite"]')
  const match = badge?.textContent?.trim().match(/^Saved\s*·\s*r(\d+)$/)
  return match ? Number(match[1]) : null
}
function editable(): HTMLElement {
  const editor = document.querySelector<HTMLElement>(editorSelector)
  if (!editor) throw new Error('The real editable feedback DOM is unavailable.')
  return editor
}
async function editSample(title: string, long: boolean, marker: string) {
  await open(title, long)
  const editor = editable()
  await until(() => savedRevision(editor) !== null, 'initial saved revision in the editor footer')
  const beforeRevision = savedRevision(editor)!
  const paragraph = editor.querySelector('p:last-child') ?? [...editor.querySelectorAll('p')].at(-1)
  if (!paragraph) throw new Error('Fixture has no final paragraph to edit.')
  editor.focus({ preventScroll: true })
  const range = document.createRange()
  range.selectNodeContents(paragraph)
  range.collapse(false)
  const selection = window.getSelection()
  if (!selection) throw new Error('DOM selection is unavailable.')
  selection.removeAllRanges()
  selection.addRange(range)
  const inputEvents: Array<{ type: string; trusted: boolean }> = []
  const onInput = (event: Event) => inputEvents.push({ type: (event as InputEvent).inputType, trusted: event.isTrusted })
  editor.addEventListener('input', onInput)
  const started = performance.now()
  try {
    // Browser editing operation, not textContent mutation or a fabricated input event.
    // Unsupported engines fail explicitly; no private TipTap/store fallback.
    if (!document.execCommand('insertText', false, ` ${marker}`)) {
      throw new Error('This browser did not execute insertText on the selected contenteditable.')
    }
    await until(() => editor.textContent?.includes(marker) === true, 'marker actually inserted into the editor DOM')
    const inputToVisibleMs = round(performance.now() - started)
    if (inputEvents.length === 0) throw new Error('Browser edit produced no observable input event.')
    await until(() => (savedRevision(editor) ?? -1) > beforeRevision, 'new Saved · rN result after the real edit')
    const inputToSavedVisibleMs = round(performance.now() - started)
    const afterRevision = savedRevision(editor)!
    await open(long ? ordinaryTitle : longTitle, !long)
    await open(title, long)
    if (!editable().textContent?.includes(marker)) throw new Error(`Saved marker missing after reopening ${title}`)
    return { marker, beforeRevision, afterRevision, inputToVisibleMs, inputToSavedVisibleMs, inputEvents, retainedAfterReopen: true }
  } finally {
    editor.removeEventListener('input', onInput)
  }
}
export function installEditBenchmark(resources: ClientResourceTracker) {
  const status = document.querySelector<HTMLOutputElement>('#quality-edit-status')!
  const output = document.querySelector<HTMLTextAreaElement>('#quality-edit-results')!
  restoreResult(storageKey, output)
  document.querySelector<HTMLButtonElement>('#quality-edit-export')!.onclick = () => {
    if (output.value) resources.exportJson(JSON.parse(output.value), 'rambledesk-quality-edit.json')
  }
  document.querySelector<HTMLButtonElement>('#quality-edit-run')!.onclick = async () => {
    if (!acquireRun('edit')) return
    const result = {
      schema: 1, kind: 'edit-save', startedAt: new Date().toISOString(), ...environment(), phase: 'running',
      budgetsMs: { ordinaryInputVisibleP95: 100, longInputVisibleP95: 200, ordinarySavedVisibleP95: 2000, longSavedVisibleP95: 3000 },
      method: 'Selection + native browser execCommand(insertText), observed input event and marker, new visible Saved revision, switch away/back and retained marker. Save latency includes the normal autosave debounce, HTTP and UI frames. Requires English Chrome-compatible editing behavior. Appends 30 markers to each draft; use an isolated fixture copy and do not reuse it for unchanged-content baselines.',
      ordinary: [] as Array<Awaited<ReturnType<typeof editSample>>>,
      long: [] as Array<Awaited<ReturnType<typeof editSample>>>,
      hiddenTransitions: 0, error: undefined as string | undefined,
    }
    const visibilityChanged = () => { if (document.hidden) result.hiddenTransitions += 1 }
    document.addEventListener('visibilitychange', visibilityChanged)
    const persist = () => {
      const json = JSON.stringify(result, null, 2)
      output.value = json
      sessionStorage.setItem(storageKey, json)
      status.textContent = `${result.phase}: ordinary ${result.ordinary.length}/30, long ${result.long.length}/30${result.error ? ` · ${result.error}` : ''}`
    }
    try {
      persist()
      if (document.hidden) throw new Error('Keep the measurement document foreground.')
      await prepareFixtures()
      const runId = Date.now().toString(36)
      for (let index = 1; index <= 30; index += 1) {
        result.ordinary.push(await editSample(ordinaryTitle, false, `quality-${runId}-ordinary-${index}`))
        persist()
        result.long.push(await editSample(longTitle, true, `quality-${runId}-long-${index}`))
        persist()
      }
      result.phase = result.hiddenTransitions ? 'done-with-visibility-interruption' : 'done'
    } catch (error) {
      result.phase = 'failed'
      result.error = error instanceof Error ? error.message : String(error)
    } finally {
      persist()
      document.removeEventListener('visibilitychange', visibilityChanged)
      releaseRun()
    }
  }
}
