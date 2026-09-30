// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from './App.svelte'
import type { ApplicationCommandInput, ApplicationCommandName, ApplicationCommandResult } from './lib/application/contracts'
import { UNAVAILABLE_CAPABILITY_MANIFEST } from './lib/capabilities/unavailableCapabilities'
import { snapshotFeedbackDraftDocument } from './lib/feedbackDraftDocument'
import type { WebReviewAnnotation } from './lib/generated/feedback'
import { autoOpenTaskBrief, cookingEnabled, locale, onboardingCompleted } from './lib/preferences'
import { PreviewApplicationTransport } from './lib/preview/previewApplicationTransport'
import { previewFixtures } from './lib/preview/previewFixtures'
import { readWorkbenchState, withWorkbenchState } from './lib/workbenchState'
import { resetPreviewWorkspaceSnapshot } from './lib/workspace/previewWorkspaceSnapshot'
import { sessionViewDescriptor, workbenchReviewViewDescriptor, workspaceViewKey } from './lib/workspace/viewDescriptors'

const request = previewFixtures.requests[0]
const sessionKey = workspaceViewKey(sessionViewDescriptor(request.host_id, request.host_session_id))
const reviewKey = workspaceViewKey(workbenchReviewViewDescriptor(request.request_id))
const bodyEditorSelector = '[contenteditable="true"][aria-label="Markdown rich-text feedback body"]'
const commentEditorSelector = '[contenteditable="true"][aria-label="Your comment"]'
const frameSelector = 'iframe[title="Web page under review"]'

class WebReviewTransport extends PreviewApplicationTransport {
  workspaceReads = 0
  override async call<Name extends ApplicationCommandName>(name: Name, input: ApplicationCommandInput<Name>): Promise<ApplicationCommandResult<Name>> {
    const result = await super.call(name, input)
    if (name === 'getFeedbackWorkspace') this.workspaceReads += 1
    return result
  }
}

let app: ReturnType<typeof mount> | undefined
let previousUrl: string
const rangeRects = Object.getOwnPropertyDescriptor(Range.prototype, 'getClientRects')
const rangeBounds = Object.getOwnPropertyDescriptor(Range.prototype, 'getBoundingClientRect')
const getAnimations = Object.getOwnPropertyDescriptor(Element.prototype, 'getAnimations')

beforeEach(() => {
  localStorage.clear()
  resetPreviewWorkspaceSnapshot()
  previousUrl = window.location.href
  window.history.replaceState(null, '', '?preview=fixtures&workspace=web_review')
  locale.set('en')
  onboardingCompleted.set(true)
  autoOpenTaskBrief.set(false)
  cookingEnabled.set(false)
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  vi.stubGlobal('matchMedia', (query: string) => ({ matches: false, media: query,
    addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {} }))
  Object.defineProperty(Element.prototype, 'getAnimations', { configurable: true, value: () => [] })
  Object.defineProperty(Range.prototype, 'getBoundingClientRect', { configurable: true, value: () => new DOMRect() })
  Object.defineProperty(Range.prototype, 'getClientRects', { configurable: true, value: () => [] })
})

afterEach(async () => {
  if (app) await unmount(app)
  app = undefined
  document.body.replaceChildren()
  window.history.replaceState(null, '', previousUrl)
  resetPreviewWorkspaceSnapshot()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  for (const [prototype, name, descriptor] of [
    [Range.prototype, 'getClientRects', rangeRects],
    [Range.prototype, 'getBoundingClientRect', rangeBounds],
    [Element.prototype, 'getAnimations', getAnimations],
  ] as const) {
    if (descriptor) Object.defineProperty(prototype, name, descriptor)
    else Reflect.deleteProperty(prototype, name)
  }
})

function button(label: string): HTMLButtonElement {
  const found = [...document.querySelectorAll<HTMLButtonElement>('button')]
    .find((item) => item.textContent?.trim() === label || item.getAttribute('aria-label') === label)
  expect(found, `button ${label}`).toBeDefined()
  return found!
}

function activeTabKey(): string | null {
  return document.querySelector('[data-workspace-tab-item][data-active="true"]')?.getAttribute('data-workspace-view-key') ?? null
}

async function appendParagraph(editor: HTMLElement, text: string) {
  const paragraph = document.createElement('p')
  paragraph.textContent = text
  editor.append(paragraph)
  editor.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertText', data: text }))
  await tick()
}

describe('web review through the real App', () => {
  it('keeps one live page and draft while opening, returning from, and closing the review tab', async () => {
    // The URL chooses both the first workbench and the restored ordinary session.
    const transport = new WebReviewTransport(UNAVAILABLE_CAPABILITY_MANIFEST)
    const initial = await transport.call('getFeedbackWorkspace', { request_id: request.request_id })
    expect(initial.workbench?.type).toBe('web_review')
    const pageUrl = `${window.location.origin}/web-review-fixture.html`
    expect(initial.workbench?.data).toMatchObject({ url: pageUrl })
    const annotation: WebReviewAnnotation = {
      id: 'saved-heading', page_url: pageUrl, viewport: { width: 1280, height: 800 },
      element: { selector: '#hero-title', tag_name: 'h1', text: 'Tools for thoughtful work',
        rect: { x: 76, y: 108, width: 680, height: 164 } },
      body: 'The heading needs a clearer promise.',
    }
    const draft = withWorkbenchState(snapshotFeedbackDraftDocument({ type: 'doc', content: [
      { type: 'paragraph', content: [{ type: 'text', text: 'Overall: keep the calm layout.' }] },
    ] }), { type: 'web_review', annotations: [annotation] })
    await transport.call('saveFeedbackDraft', { request_id: request.request_id,
      document_json: draft.documentJson, body_markdown: draft.bodyMarkdown, expected_revision: 0 })
    app = mount(App, { target: document.body, props: {
      applicationTransport: transport, environment: 'browser', previewMode: true,
      publishedFeedbackAction: { label: 'Open feedback package', run: async () => {} },
    } })
    await vi.waitFor(() => expect(document.querySelector(bodyEditorSelector)).not.toBeNull())
    expect(activeTabKey()).toBe(sessionKey)
    expect(document.querySelector('[data-workbench-review-mode="false"]')).not.toBeNull()
    await vi.waitFor(() => expect(document.querySelector('[data-web-review-toolbar]')?.contains(button('Open review tab'))).toBe(true))
    expect(document.querySelectorAll('[data-workspace-tab-item]')).toHaveLength(1)
    const rail = document.querySelector<HTMLElement>('aside[aria-label="Projects"]')!
    expect(rail).not.toBeNull()
    expect(rail.querySelectorAll('button.session-title, button[data-session-id]').length).toBeGreaterThan(0)
    const frame = document.querySelector<HTMLIFrameElement>(frameSelector)!
    const bodyEditor = document.querySelector<HTMLElement>(bodyEditorSelector)!
    expect(frame.src).toBe(pageUrl)
    await appendParagraph(bodyEditor, 'The navigation should stay available.')
    await vi.waitFor(() => expect(document.querySelector<HTMLButtonElement>('button[aria-label="Undo"]')?.disabled).toBe(false))
    const initialReads = transport.workspaceReads

    button('Open review tab').click()
    await vi.waitFor(() => expect(activeTabKey()).toBe(reviewKey))
    expect(document.querySelectorAll('[data-workspace-tab-item]')).toHaveLength(2)
    expect(document.querySelector('[data-workbench-review-mode="true"]')).not.toBeNull()
    expect(document.querySelector('aside[aria-label="Projects"]')).toBe(rail)
    expect(document.querySelector(frameSelector)).toBe(frame)
    expect(document.querySelector(bodyEditorSelector)).toBe(bodyEditor)
    expect(document.querySelectorAll(frameSelector)).toHaveLength(1)
    expect(document.querySelectorAll(bodyEditorSelector)).toHaveLength(1)
    expect(transport.workspaceReads).toBe(initialReads)

    button('Review comments · 1').click()
    await vi.waitFor(() => expect(document.querySelector('[data-web-review-note-preview="saved-heading"]')).not.toBeNull())
    document.querySelector<HTMLButtonElement>('[data-web-review-note-preview="saved-heading"]')!.click()
    await vi.waitFor(() => expect(document.querySelector(commentEditorSelector)).not.toBeNull())
    const commentEditor = document.querySelector<HTMLElement>(commentEditorSelector)!
    expect(commentEditor.textContent).toContain(annotation.body)
    await appendParagraph(commentEditor, 'Keep the original element context.')
    await vi.waitFor(async () => {
      const workspace = await transport.call('getFeedbackWorkspace', { request_id: request.request_id })
      const state = readWorkbenchState(workspace.draft.document_json)
      expect(state?.type).toBe('web_review')
      if (state?.type === 'web_review') expect(state.annotations[0]?.body).toContain('Keep the original element context.')
    }, { timeout: 2_000 })

    button('Return to workbench').click()
    await vi.waitFor(() => expect(activeTabKey()).toBe(sessionKey))
    expect(document.querySelector('[data-workbench-review-mode="false"]')).not.toBeNull()
    expect(document.querySelector(frameSelector)).toBe(frame)
    expect(document.querySelector(bodyEditorSelector)).toBe(bodyEditor)
    expect(document.querySelector(commentEditorSelector)).toBe(commentEditor)
    button('Open review tab').click()
    await vi.waitFor(() => expect(activeTabKey()).toBe(reviewKey))
    expect(document.querySelectorAll('[data-workspace-tab-item]')).toHaveLength(2)
    const reviewTab = [...document.querySelectorAll<HTMLElement>('[data-workspace-tab-item]')]
      .find((item) => item.getAttribute('data-workspace-view-key') === reviewKey)!
    reviewTab.querySelector<HTMLButtonElement>('button[aria-label^="Close workspace tab"]')!.click()
    await vi.waitFor(() => expect(activeTabKey()).toBe(sessionKey))
    expect(document.querySelectorAll('[data-workspace-tab-item]')).toHaveLength(1)
    expect(frame.isConnected).toBe(true)
    expect(bodyEditor.isConnected).toBe(true)
    expect(commentEditor.isConnected).toBe(true)
    expect(document.querySelector('aside[aria-label="Projects"]')).toBe(rail)
    const saved = await transport.call('getFeedbackWorkspace', { request_id: request.request_id })
    expect(saved.draft.body_markdown).toContain('Overall: keep the calm layout.')
    expect(saved.draft.body_markdown).toContain('The navigation should stay available.')
    const state = readWorkbenchState(saved.draft.document_json)
    expect(state?.type).toBe('web_review')
    if (state?.type === 'web_review') {
      expect(state.annotations).toHaveLength(1)
      expect(state.annotations[0]).toMatchObject({ ...annotation,
        body: expect.stringContaining('Keep the original element context.') })
    }
  })
})
