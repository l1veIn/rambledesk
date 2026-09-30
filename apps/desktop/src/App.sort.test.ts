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
let app: ReturnType<typeof mount> | undefined
let previousUrl = ''
const originals = ['getClientRects', 'getBoundingClientRect'].map((name) => [name, Object.getOwnPropertyDescriptor(Range.prototype, name)] as const)
const animations = Object.getOwnPropertyDescriptor(Element.prototype, 'getAnimations')

beforeEach(() => {
  localStorage.clear(); resetPreviewWorkspaceSnapshot()
  previousUrl = location.href; history.replaceState(null, '', '?preview=fixtures&workspace=sort')
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
const activeKey = () => document.querySelector('[data-workspace-tab-item][data-active="true"]')?.getAttribute('data-workspace-view-key')
const renderedOrder = () => [...document.querySelectorAll('[data-sort-item-id]')].map((row) => row.getAttribute('data-sort-item-id'))

it('uses shared App autosave, expansion, recovery and publication for a new sorting workbench without feedback body text', async () => {
  const transport = new PreviewApplicationTransport(UNAVAILABLE_CAPABILITY_MANIFEST, { workspace: 'sort' })
  const mountApp = () => { app = mount(App, { target: document.body, props: { applicationTransport: transport,
    environment: 'browser', previewMode: true, publishedFeedbackAction: { label: 'Open feedback package', run: async () => {} } } }) }
  const saved = () => transport.call('getFeedbackWorkspace', { request_id: request.request_id })
  mountApp()
  await vi.waitFor(() => expect(renderedOrder()).toEqual(['quickstart', 'errors', 'completion', 'config']))
  for (let move = 0; move < 3; move++) {
    button('上移 配置文件').click()
    await tick()
  }
  const order = ['config', 'quickstart', 'errors', 'completion']
  await vi.waitFor(async () => expect(readWorkbenchState((await saved()).draft.document_json)).toEqual({ type: 'sort', order }), { timeout: 3000 })
  expect((await saved()).draft.body_markdown).toBe('')
  const rail = document.querySelector('aside[aria-label="Projects"]')!
  expect(rail).not.toBeNull()
  button('全屏工作台').click()
  await vi.waitFor(() => expect(activeKey()).toBe(reviewKey))
  expect(document.querySelector('aside[aria-label="Projects"]')).toBe(rail)
  expect(renderedOrder()).toEqual(order)
  button('Return to workbench').click()
  await vi.waitFor(() => expect(activeKey()).toBe(sessionKey))
  await unmount(app!); app = undefined; document.body.replaceChildren(); resetPreviewWorkspaceSnapshot()
  mountApp()
  await vi.waitFor(() => expect(renderedOrder()).toEqual(order))
  await vi.waitFor(() => expect(button('Submit feedback').disabled).toBe(false))
  button('Submit feedback').click()
  await vi.waitFor(async () => expect((await saved()).request.status).toBe('completed'))
  await vi.waitFor(() => expect(document.querySelector('[data-sort-workbench] [aria-label^="拖动 "]')).toBeNull())
  expect(document.querySelector('[data-sort-workbench] button[aria-label^="上移 "]')).toBeNull()
  expect(readWorkbenchState((await saved()).draft.document_json)).toEqual({ type: 'sort', order })
  expect(renderedOrder()).toEqual(order)
}, 15000)
