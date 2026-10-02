// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import App from './App.svelte'
import { UNAVAILABLE_CAPABILITY_MANIFEST } from './lib/capabilities/unavailableCapabilities'
import { autoOpenTaskBrief, cookingEnabled, locale, onboardingCompleted } from './lib/preferences'
import { PreviewApplicationTransport } from './lib/preview/previewApplicationTransport'
import { previewFixtures } from './lib/preview/previewFixtures'
import { readWorkbenchState } from './lib/workbenchState'
import { resetPreviewWorkspaceSnapshot } from './lib/workspace/previewWorkspaceSnapshot'
import { sessionViewDescriptor, workbenchReviewViewDescriptor, workspaceViewKey } from './lib/workspace/viewDescriptors'

const request = previewFixtures.requests[0]
const sessionKey = workspaceViewKey(sessionViewDescriptor(request.host_id, request.host_session_id))
const reviewKey = workspaceViewKey(workbenchReviewViewDescriptor(request.request_id))
const opinionSelector = '[contenteditable="true"][aria-label="意见（可选）"]'
const bodySelector = '[contenteditable="true"][aria-label="Markdown rich-text feedback body"]'
let app: ReturnType<typeof mount> | undefined
let previousUrl = ''
const originals = ['getClientRects', 'getBoundingClientRect'].map((name) => [name, Object.getOwnPropertyDescriptor(Range.prototype, name)] as const)
const animations = Object.getOwnPropertyDescriptor(Element.prototype, 'getAnimations')
beforeEach(() => {
  localStorage.clear(); resetPreviewWorkspaceSnapshot()
  previousUrl = location.href; history.replaceState(null, '', '?preview=fixtures&workspace=rating_review')
  locale.set('en'); onboardingCompleted.set(true); autoOpenTaskBrief.set(false); cookingEnabled.set(false)
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  vi.stubGlobal('matchMedia', (query: string) => ({ matches: false, media: query,
    addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {} }))
  Object.defineProperty(Element.prototype, 'getAnimations', { configurable: true, value: () => [] })
  Object.defineProperty(Range.prototype, 'getBoundingClientRect', { configurable: true, value: () => new DOMRect() })
  Object.defineProperty(Range.prototype, 'getClientRects', { configurable: true, value: () => [] })
})
afterEach(async () => {
  if (app) await unmount(app)
  app = undefined; document.body.replaceChildren(); history.replaceState(null, '', previousUrl)
  resetPreviewWorkspaceSnapshot(); vi.restoreAllMocks(); vi.unstubAllGlobals()
  for (const [name, descriptor] of originals) {
    if (descriptor) Object.defineProperty(Range.prototype, name, descriptor)
    else Reflect.deleteProperty(Range.prototype, name)
  }
  if (animations) Object.defineProperty(Element.prototype, 'getAnimations', animations)
  else Reflect.deleteProperty(Element.prototype, 'getAnimations')
})
function button(label: string): HTMLButtonElement {
  const found = [...document.querySelectorAll<HTMLButtonElement>('button')].find((item) => item.textContent?.trim() === label || item.getAttribute('aria-label') === label)
  expect(found, label).toBeDefined(); return found!
}
async function append(selector: string, text: string) {
  const editor = document.querySelector<HTMLElement>(selector)!
  const paragraph = document.createElement('p'); paragraph.textContent = text; editor.append(paragraph)
  editor.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertText', data: text }))
  await tick()
}
const activeKey = () => document.querySelector('[data-workspace-tab-item][data-active="true"]')?.getAttribute('data-workspace-view-key')

it('runs a newly registered ordinary workbench through App saving, expansion, remount recovery and read-only publication', async () => {
  const transport = new PreviewApplicationTransport(UNAVAILABLE_CAPABILITY_MANIFEST, { workspace: 'rating_review' })
  const mountApp = () => { app = mount(App, { target: document.body, props: { applicationTransport: transport,
    environment: 'browser', previewMode: true, publishedFeedbackAction: { label: 'Open feedback package', run: async () => {} } } }) }
  mountApp()
  await vi.waitFor(() => expect(document.querySelector(opinionSelector)).not.toBeNull())
  await append(opinionSelector, 'Useful idea, add an example.')
  await append(bodySelector, 'Shared feedback remains separate.')
  expect(button('Submit feedback').disabled).toBe(true)
  button('4 分').click()
  await vi.waitFor(async () => {
    const saved = await transport.call('getFeedbackWorkspace', { request_id: request.request_id })
    expect(readWorkbenchState(saved.draft.document_json)).toMatchObject({ type: 'rating_review', score: 4, note: expect.stringContaining('Useful idea') })
    expect(saved.draft.body_markdown).toContain('Shared feedback')
  }, { timeout: 3000 })
  const rail = document.querySelector('aside[aria-label="Projects"]')!
  expect(rail).not.toBeNull()
  const opinion = document.querySelector(opinionSelector)
  button('全屏评审').click()
  await vi.waitFor(() => expect(activeKey()).toBe(reviewKey))
  expect(document.querySelector('aside[aria-label="Projects"]')).toBe(rail)
  expect(document.querySelector(opinionSelector)).toBe(opinion)
  button('Return to workbench').click()
  await vi.waitFor(() => expect(activeKey()).toBe(sessionKey))
  await unmount(app!); app = undefined; document.body.replaceChildren(); resetPreviewWorkspaceSnapshot()
  mountApp()
  await vi.waitFor(() => expect(document.querySelector(opinionSelector)?.textContent).toContain('Useful idea'))
  expect(button('4 分').getAttribute('aria-pressed')).toBe('true')
  await vi.waitFor(() => expect(button('Submit feedback').disabled).toBe(false))
  button('Submit feedback').click()
  await vi.waitFor(async () => expect((await transport.call('getFeedbackWorkspace', { request_id: request.request_id })).request.status).toBe('completed'))
  await vi.waitFor(() => expect(document.querySelector('[data-rating-review] button[aria-label="4 分"]')?.closest('fieldset')?.disabled).toBe(true))
  expect(readWorkbenchState((await transport.call('getFeedbackWorkspace', { request_id: request.request_id })).draft.document_json)).toMatchObject({ score: 4, note: expect.stringContaining('Useful idea') })
}, 15000)
