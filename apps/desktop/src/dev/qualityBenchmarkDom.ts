/** DOM-only driver: no App, store, editor-instance, or transport shortcuts. */
export const editorSelector = '[contenteditable="true"][aria-label="Markdown rich-text feedback body"]'
export const ordinaryTitle = '01 · Ordinary feedback'
export const longTitle = '02 · Long structured draft (240 paragraphs)'
export const frame = () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
export const round = (value: number) => Math.round(value * 10) / 10
export const delay = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms))
let activeRun: string | null = null
export function acquireRun(name: string): boolean {
  if (activeRun) return false
  activeRun = name
  document.querySelectorAll<HTMLButtonElement>('[data-quality-run]').forEach((button) => { button.disabled = true })
  return true
}
export function releaseRun() {
  activeRun = null
  document.querySelectorAll<HTMLButtonElement>('[data-quality-run]').forEach((button) => { button.disabled = false })
}
export function requestButton(title: string) {
  return [...document.querySelectorAll<HTMLButtonElement>('#app button')]
    .find((button) => button.getAttribute('aria-label') === `${title} · Acceptance fixture`)
}
export async function until(predicate: () => boolean, description: string) {
  const deadline = performance.now() + 15_000
  // Recheck after each frame: an old matching DOM can disappear during navigation.
  let consecutive = predicate() ? 1 : 0
  while (consecutive < 3) {
    if (performance.now() > deadline) throw new Error(`Timed out: ${description}`)
    await frame()
    consecutive = predicate() ? consecutive + 1 : 0
  }
}
export function fixtureVisible(title: string, long: boolean) {
  const editors = document.querySelectorAll(`#app ${editorSelector}`)
  const editor = editors[0]
  return editors.length === 1 && editor.querySelector('h2')?.textContent === title
    && (editor.querySelectorAll('p').length >= 240) === long
}
export async function open(title: string, long: boolean) {
  const started = performance.now()
  // Never launch another navigation when the requested view is already present.
  // Its asynchronous replacement could otherwise outlive our completion check.
  if (!fixtureVisible(title, long)) {
    const button = requestButton(title)
    if (!button || button.disabled) throw new Error(`Request list must show an enabled ${title}`)
    button.click()
  }
  await until(() => fixtureVisible(title, long), title)
  return round(performance.now() - started)
}
export async function prepareFixtures() {
  if (!requestButton(ordinaryTitle) || !requestButton(longTitle)) {
    throw new Error('Use English, finish onboarding, and keep both acceptance requests visible in the request list.')
  }
  await open(longTitle, true)
  await open(ordinaryTitle, false)
}
export function environment() {
  return { userAgent: navigator.userAgent, viewport: { width: innerWidth, height: innerHeight }, visibility: document.visibilityState }
}
export function restoreResult(key: string, output: HTMLTextAreaElement) {
  const previous = sessionStorage.getItem(key)
  if (!previous) return
  const result = JSON.parse(previous)
  if (result.phase === 'running') {
    result.phase = 'interrupted'
    result.error = 'Document reloaded before this manual run finished; start a new run.'
  }
  output.value = JSON.stringify(result, null, 2)
}
