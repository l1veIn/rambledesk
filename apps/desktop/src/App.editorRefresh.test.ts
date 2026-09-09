// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from './App.svelte'
import { APPLICATION_EVENTS_STREAM } from './lib/application/applicationEvents'
import type { ApplicationStream, SubscriptionErrorHandler, Unsubscribe } from './lib/application/applicationTransport'
import type { ApplicationCommandInput, ApplicationCommandName, ApplicationCommandResult } from './lib/application/contracts'
import { UNAVAILABLE_CAPABILITY_MANIFEST } from './lib/capabilities/unavailableCapabilities'
import { autoOpenTaskBrief, cookingEnabled, locale, onboardingCompleted } from './lib/preferences'
import { PreviewApplicationTransport } from './lib/preview/previewApplicationTransport'
import { previewFixtures } from './lib/preview/previewFixtures'
import { saveWorkspaceSnapshot } from './lib/uiPreferences'
import { sessionViewDescriptor, workspaceViewKey } from './lib/workspace/viewDescriptors'

const request = previewFixtures.requests[0]
const sessionView = sessionViewDescriptor(request.host_id, request.host_session_id)
const editorSelector = '[contenteditable="true"][aria-label="Markdown rich-text feedback body"]'

/** The ordinary preview server plus the invalidation stream emitted by the real backend. */
class InvalidatingTransport extends PreviewApplicationTransport {
  private listeners = new Set<(event: unknown) => void>()
  workspaceReads = 0
  saves = 0

  constructor() { super(UNAVAILABLE_CAPABILITY_MANIFEST) }

  override async call<Name extends ApplicationCommandName>(name: Name, input: ApplicationCommandInput<Name>): Promise<ApplicationCommandResult<Name>> {
    const result = await super.call(name, input)
    if (name === 'getFeedbackWorkspace') this.workspaceReads += 1
    if (name === 'saveFeedbackDraft') {
      this.saves += 1
      this.invalidate()
    }
    return result
  }

  override subscribe<Event>(stream: ApplicationStream<Event>, handler: (event: Event) => void, _onError: SubscriptionErrorHandler): Unsubscribe {
    if (stream.id !== APPLICATION_EVENTS_STREAM.id) return () => {}
    const receive = handler as (event: unknown) => void
    this.listeners.add(receive)
    return () => { this.listeners.delete(receive) }
  }

  invalidate() {
    for (const listener of this.listeners) listener({
      type: 'invalidate', runtime_generation: 'editor-test', revision: String(this.saves),
      resources: [{ kind: 'feedback_workspace', request_id: request.request_id }],
    })
  }
}

let app: ReturnType<typeof mount> | undefined
const rangeRects = Object.getOwnPropertyDescriptor(Range.prototype, 'getClientRects')
const rangeBounds = Object.getOwnPropertyDescriptor(Range.prototype, 'getBoundingClientRect')
beforeEach(() => {
  localStorage.clear()
  locale.set('en')
  onboardingCompleted.set(true)
  autoOpenTaskBrief.set(false)
  cookingEnabled.set(false)
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  vi.stubGlobal('matchMedia', (query: string) => ({ matches: false, media: query, addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {} }))
  Object.defineProperty(Range.prototype, 'getBoundingClientRect', { configurable: true, value: () => new DOMRect() })
  Object.defineProperty(Range.prototype, 'getClientRects', { configurable: true, value: () => [] })
  saveWorkspaceSnapshot({ version: 2, views: [{ ...sessionView, lastRequestId: request.request_id }], activeViewKey: workspaceViewKey(sessionView) })
})
afterEach(async () => {
  if (app) await unmount(app)
  app = undefined
  document.body.replaceChildren()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  for (const [name, descriptor] of [['getClientRects', rangeRects], ['getBoundingClientRect', rangeBounds]] as const) {
    if (descriptor) Object.defineProperty(Range.prototype, name, descriptor)
    else Reflect.deleteProperty(Range.prototype, name)
  }
})

describe('editor history across real App save invalidations', () => {
  it('keeps the mounted editor and Undo/Redo after autosave refreshes the same server document', async () => {
    const transport = new InvalidatingTransport()
    app = mount(App, { target: document.body, props: {
      applicationTransport: transport, environment: 'browser',
      publishedFeedbackAction: { label: 'Open feedback package', run: async () => {} },
    } })
    await vi.waitFor(() => expect(document.querySelector(editorSelector)).not.toBeNull())
    const editor = document.querySelector<HTMLElement>(editorSelector)!
    const initialReads = transport.workspaceReads
    const paragraph = document.createElement('p')
    paragraph.textContent = 'Keep undo after this has been saved.'
    editor.append(paragraph)
    editor.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertText', data: paragraph.textContent }))
    await tick()
    await vi.waitFor(() => expect(transport.saves).toBe(1), { timeout: 2_000 })
    await vi.waitFor(() => expect(transport.workspaceReads).toBeGreaterThan(initialReads))
    await vi.waitFor(() => expect(document.querySelector(editorSelector)).not.toBeNull())
    expect(editor.isConnected).toBe(true)
    const undo = document.querySelector<HTMLButtonElement>('button[aria-label="Undo"]')!
    expect(undo.disabled).toBe(false)
    undo.click()
    await vi.waitFor(() => expect(editor.textContent).not.toContain('Keep undo after this has been saved.'))
    const redo = document.querySelector<HTMLButtonElement>('button[aria-label="Redo"]')!
    expect(redo.disabled).toBe(false)
    redo.click()
    await vi.waitFor(() => expect(editor.textContent).toContain('Keep undo after this has been saved.'))
  })
})
