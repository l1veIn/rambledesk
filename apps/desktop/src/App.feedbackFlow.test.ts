// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from './App.svelte'
import { createWorkbenchCapabilities } from './lib/capabilities/workbenchCapabilities'
import type { SpeechRecognitionListener } from './lib/speech/speech'
import type {
  ApplicationCommandInput,
  ApplicationCommandName,
  ApplicationCommandResult,
} from './lib/application/contracts'
import {
  createUnavailableWorkbenchCapabilities,
  UNAVAILABLE_CAPABILITY_MANIFEST,
} from './lib/capabilities/unavailableCapabilities'
import type { SaveDraftInput } from './lib/feedback'
import { autoOpenTaskBrief, cookingEnabled, locale, onboardingCompleted } from './lib/preferences'
import { PreviewApplicationTransport } from './lib/preview/previewApplicationTransport'
import { previewFixtures } from './lib/preview/previewFixtures'
import { saveWorkspaceSnapshot } from './lib/uiPreferences'
import { inboxViewDescriptor, sessionViewDescriptor, workspaceViewKey } from './lib/workspace/viewDescriptors'

const request = previewFixtures.requests[0]
const sessionView = sessionViewDescriptor(request.host_id, request.host_session_id)
const editorSelector = '[contenteditable="true"][aria-label="Markdown rich-text feedback body"]'

/** The real in-memory server, with controllable latency and one-shot failures at its boundary. */
class FeedbackFlowTransport extends PreviewApplicationTransport {
  readonly mutations: Array<{ name: string; input: unknown }> = []
  failNextSave = false
  saveGate: Promise<void> | null = null
  submitGate: Promise<void> | null = null

  constructor() {
    super(UNAVAILABLE_CAPABILITY_MANIFEST)
  }

  override async call<Name extends ApplicationCommandName>(
    name: Name,
    input: ApplicationCommandInput<Name>,
  ): Promise<ApplicationCommandResult<Name>> {
    if (name === 'saveFeedbackDraft' || name === 'submitFeedback') {
      this.mutations.push({ name, input })
    }
    if (name === 'saveFeedbackDraft' && this.failNextSave) {
      this.failNextSave = false
      throw new Error('Draft save unavailable; retry when connected.')
    }
    if (name === 'saveFeedbackDraft' && this.saveGate) await this.saveGate
    if (name === 'submitFeedback' && this.submitGate) await this.submitGate
    return super.call(name, input)
  }

  saves(): SaveDraftInput[] {
    return this.mutations
      .filter(({ name }) => name === 'saveFeedbackDraft')
      .map(({ input }) => input as SaveDraftInput)
  }

  submits() {
    return this.mutations.filter(({ name }) => name === 'submitFeedback')
  }
}

let app: ReturnType<typeof mount> | undefined
let host: HTMLDivElement
const rangeRects = Object.getOwnPropertyDescriptor(Range.prototype, 'getClientRects')
const rangeBounds = Object.getOwnPropertyDescriptor(Range.prototype, 'getBoundingClientRect')

beforeEach(() => {
  localStorage.clear()
  locale.set('en')
  onboardingCompleted.set(true)
  autoOpenTaskBrief.set(false)
  cookingEnabled.set(false)
  // jsdom has no layout geometry; Tiptap's real focus/undo path still asks for it.
  Object.defineProperty(Range.prototype, 'getClientRects', { configurable: true, value: () => [] })
  Object.defineProperty(Range.prototype, 'getBoundingClientRect', { configurable: true, value: () => new DOMRect() })
  saveWorkspaceSnapshot({
    version: 2,
    views: [inboxViewDescriptor(), { ...sessionView, lastRequestId: request.request_id }],
    activeViewKey: workspaceViewKey(sessionView),
  })
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as never
  window.matchMedia = ((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener() {},
    removeEventListener() {},
    addListener() {},
    removeListener() {},
    dispatchEvent: () => false,
  })) as never
  host = document.createElement('div')
  document.body.append(host)
})

afterEach(async () => {
  if (app) await unmount(app)
  app = undefined
  host.remove()
  document.body.replaceChildren()
  for (const [name, descriptor] of [['getClientRects', rangeRects], ['getBoundingClientRect', rangeBounds]] as const) {
    if (descriptor) Object.defineProperty(Range.prototype, name, descriptor)
    else Reflect.deleteProperty(Range.prototype, name)
  }
})

async function openWorkbench(
  transport = new FeedbackFlowTransport(),
  capabilities = createUnavailableWorkbenchCapabilities(),
  environment: 'browser' | 'desktop' = 'browser',
) {
  const openPackage = vi.fn(async () => undefined)
  app = mount(App, {
    target: host,
    props: {
      applicationTransport: transport,
      capabilities,
      publishedFeedbackAction: { label: 'Open feedback package', run: openPackage },
      environment,
    },
  })
  await vi.waitFor(() => expect(host.querySelector(editorSelector)).not.toBeNull())
  return { transport, openPackage }
}

function button(label: string): HTMLButtonElement | undefined {
  return [...host.querySelectorAll('button')].find((candidate) => candidate.textContent?.trim() === label)
}

async function appendFeedback(text: string) {
  const editor = host.querySelector<HTMLElement>(editorSelector)
  expect(editor).not.toBeNull()
  // jsdom has no native contenteditable typing. Apply the browser DOM mutation and
  // input event; the real ProseMirror observer parses it and emits the TipTap update.
  const paragraph = document.createElement('p')
  paragraph.textContent = text
  editor!.append(paragraph)
  editor!.dispatchEvent(new InputEvent('input', {
    bubbles: true,
    inputType: 'insertText',
    data: text,
  }))
  await tick()
  await vi.waitFor(() => {
    expect(host.textContent).toContain('Waiting to autosave')
    expect(button('Submit feedback')?.disabled).toBe(false)
  })
}

async function expectPublished() {
  await vi.waitFor(() => expect(button('Open feedback package')).toBeDefined(), { timeout: 3_000 })
  expect(host.textContent).toContain('Feedback Package')
  expect(host.querySelector('[aria-label="Markdown rich-text feedback body"]')?.getAttribute('contenteditable')).toBe('false')
}

describe('feedback flow through the real App and editor', () => {
  function requestsLeaveConfirmation() {
    const event = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(event)
    return event.defaultPrevented
  }

  it('guards a browser formatting edit and in-flight save, then stops prompting after Saved', async () => {
    const transport = new FeedbackFlowTransport()
    let release!: () => void
    transport.saveGate = new Promise<void>((resolve) => { release = resolve })
    await openWorkbench(transport)
    expect(requestsLeaveConfirmation()).toBe(false)
    try {
      host.querySelector<HTMLButtonElement>('button[aria-label="Heading 2"]')!.click()
      await tick()
      expect(host.querySelector(`${editorSelector} h2`)).not.toBeNull()
      expect(requestsLeaveConfirmation()).toBe(true)
      await vi.waitFor(() => expect(transport.saves()).toHaveLength(1), { timeout: 2_000 })
      expect(requestsLeaveConfirmation()).toBe(true)
    } finally {
      release()
    }
    await vi.waitFor(() => expect(host.textContent).toContain('Saved · r'))
    expect(requestsLeaveConfirmation()).toBe(false)
  })

  it('still guards an in-flight save when Undo restores the old saved document', async () => {
    const transport = new FeedbackFlowTransport()
    let release!: () => void
    transport.saveGate = new Promise<void>((resolve) => { release = resolve })
    await openWorkbench(transport)
    const original = (await transport.call('getFeedbackWorkspace', { request_id: request.request_id }))!.draft
    try {
      host.querySelector<HTMLButtonElement>('button[aria-label="Heading 2"]')!.click()
      await vi.waitFor(() => expect(transport.saves()).toHaveLength(1), { timeout: 2_000 })
      // jsdom reports a non-Mac platform, so Mod-z is Control-z here.
      host.querySelector<HTMLElement>(editorSelector)!.dispatchEvent(new KeyboardEvent('keydown', {
        key: 'z', code: 'KeyZ', ctrlKey: true, bubbles: true, cancelable: true,
      }))
      await tick()
      expect(host.querySelector(`${editorSelector} h2`)).toBeNull()
      expect(host.textContent).toContain(`Saved · r${original.saved_revision}`)
      expect(requestsLeaveConfirmation()).toBe(true)
    } finally {
      release()
    }
    await vi.waitFor(() => expect(transport.saves()).toHaveLength(2))
    await vi.waitFor(() => expect(host.textContent).toContain(`Saved · r${original.saved_revision + 2}`))
    const saved = (await transport.call('getFeedbackWorkspace', { request_id: request.request_id }))!.draft
    expect(saved.body_markdown).toBe(original.body_markdown)
    expect(requestsLeaveConfirmation()).toBe(false)
  })

  it('keeps the browser guard after a failed save and removes its listener on disposal', async () => {
    const add = vi.spyOn(window, 'addEventListener')
    const remove = vi.spyOn(window, 'removeEventListener')
    try {
      const { transport } = await openWorkbench()
      transport.failNextSave = true
      await appendFeedback('Preserve this draft when the network is unavailable.')
      await vi.waitFor(() => expect(document.body.textContent).toContain('Draft save unavailable; retry when connected.'), { timeout: 2_000 })
      expect(requestsLeaveConfirmation()).toBe(true)
      const guards = add.mock.calls.filter(([type]) => type === 'beforeunload')
      expect(guards).toHaveLength(1)
      await unmount(app!)
      app = undefined
      expect(remove.mock.calls.filter(([type]) => type === 'beforeunload')).toEqual(guards)
      expect(requestsLeaveConfirmation()).toBe(false)
    } finally {
      add.mockRestore()
      remove.mockRestore()
    }
  })

  it('does not install browser leave protection for the desktop environment', async () => {
    const add = vi.spyOn(window, 'addEventListener')
    try {
      await openWorkbench(new FeedbackFlowTransport(), createUnavailableWorkbenchCapabilities(), 'desktop')
      await appendFeedback('Desktop closing continues to belong to the native shell.')
      expect(requestsLeaveConfirmation()).toBe(false)
      expect(add.mock.calls.some(([type]) => type === 'beforeunload')).toBe(false)
    } finally {
      add.mockRestore()
    }
  })

  it('keeps the microphone owner alive while the interface language changes', async () => {
    const unavailable = createUnavailableWorkbenchCapabilities()
    let listener!: SpeechRecognitionListener
    const cancel = vi.fn(async () => {})
    const start = vi.fn((_options, next: SpeechRecognitionListener) => {
      listener = next
      return { id: 'language-test', ready: Promise.resolve(), cancel, stop: async () => {
        listener.onEvent({ type: 'stopped', sessionId: 'language-test', reason: 'stopped' })
      } }
    })
    await openWorkbench(new FeedbackFlowTransport(), createWorkbenchCapabilities({
      ...unavailable,
      speech: { status: { availability: 'available', source: 'browser' }, implementation: { ...unavailable.speech.implementation, start } },
    }))
    const record = button('Start recording')
    expect(record).not.toBeNull()
    record!.click()
    await vi.waitFor(() => expect(start).toHaveBeenCalledOnce())
    locale.set('zh-CN')
    await tick()
    expect(cancel).not.toHaveBeenCalled()
    listener.onEvent({ type: 'partial', sessionId: 'language-test', text: 'Still recording after the language change' })
    await tick()
    expect(document.body.textContent).toContain('Still recording after the language change')
    await unmount(app!)
    app = undefined
    await vi.waitFor(() => expect(cancel).toHaveBeenCalledOnce())
  })

  it('shows an action selection immediately while its document save is still in flight', async () => {
    const transport = new FeedbackFlowTransport()
    let release!: () => void
    const saving = new Promise<void>((resolve) => { release = resolve })
    const call = transport.call.bind(transport)
    transport.call = (async (name, input) => {
      if (name === 'saveFeedbackDraft') await saving
      return call(name, input)
    }) as typeof transport.call
    await openWorkbench(transport)
    const action = [...host.querySelectorAll<HTMLButtonElement>('button[aria-pressed]')]
      .find((candidate) => candidate.textContent?.includes(previewFixtures.workspace.actions[0].instruction))
    expect(action).toBeDefined()
    try {
      action!.click()
      await tick()
      expect(action!.getAttribute('aria-pressed')).toBe('true')
      expect(transport.saves()).toHaveLength(0)
    } finally { release() }
    await vi.waitFor(() => expect(transport.saves()).toHaveLength(1))
  })

  it('autosaves the edited document, submits that revision, and exposes the feedback package', async () => {
    const { transport, openPackage } = await openWorkbench()
    const text = 'The mobile request drawer is now comfortable to use.'
    await appendFeedback(text)

    await vi.waitFor(async () => {
      const workspace = await transport.call('getFeedbackWorkspace', { request_id: request.request_id })
      expect(workspace?.draft.body_markdown).toContain(text)
    }, { timeout: 3_000 })
    const saved = transport.saves().at(-1)!
    expect(saved.document_json).toContain(text)
    await vi.waitFor(() => expect(host.textContent).toContain(`Saved · r${saved.expected_revision + 1}`))

    button('Submit feedback')!.click()
    await expectPublished()
    expect(transport.mutations.map(({ name }) => name)).toEqual(['saveFeedbackDraft', 'submitFeedback'])
    expect(transport.submits()[0].input).toEqual({
      request_id: request.request_id,
      expected_revision: saved.expected_revision + 1,
    })
    const published = await transport.call('readPublishedFeedback', { request_id: request.request_id })
    expect(published?.markdown).toContain(text)
    button('Open feedback package')!.click()
    expect(openPackage).toHaveBeenCalledWith(request.request_id)
  })

  it('keeps workspace tabs locked until the submission finishes', async () => {
    const transport = new FeedbackFlowTransport()
    let finishSubmission!: () => void
    transport.submitGate = new Promise<void>((resolve) => { finishSubmission = resolve })
    await openWorkbench(transport)
    const activeKey = workspaceViewKey(sessionView)
    const tabs = [...host.querySelectorAll<HTMLElement>('[role="tab"][data-workspace-tab-trigger]')]
    expect(tabs).toHaveLength(2)

    try {
      button('Submit feedback')!.click()
      await vi.waitFor(() => expect(transport.submits()).toHaveLength(1))
      await tick()
      expect(tabs.every((tab) => tab.getAttribute('aria-disabled') === 'true')).toBe(true)
      tabs.find((tab) => tab.dataset.workspaceViewKey !== activeKey)!.click()
      await tick()
      expect(host.querySelector('[role="tab"][aria-selected="true"]')?.getAttribute('data-workspace-view-key')).toBe(activeKey)
    } finally {
      finishSubmission()
    }
    await expectPublished()
    expect(tabs.every((tab) => tab.getAttribute('aria-disabled') === 'false')).toBe(true)
  })

  it('preserves edits when saving fails, blocks publication, and succeeds when retried', async () => {
    const { transport } = await openWorkbench()
    const text = 'Keep this unsaved feedback through a connection failure.'
    transport.failNextSave = true
    await appendFeedback(text)
    button('Submit feedback')!.click()

    await vi.waitFor(() => expect(document.body.textContent).toContain('Draft save unavailable; retry when connected.'))
    expect(transport.saves()).toHaveLength(1)
    expect(transport.submits()).toHaveLength(0)
    expect(host.querySelector(editorSelector)?.textContent).toContain(text)
    const original = await transport.call('getFeedbackWorkspace', { request_id: request.request_id })
    expect(original?.draft.body_markdown).not.toContain(text)

    button('Submit feedback')!.click()
    await expectPublished()
    expect(transport.mutations.map(({ name }) => name)).toEqual([
      'saveFeedbackDraft', 'saveFeedbackDraft', 'submitFeedback',
    ])
    expect(transport.saves()[1].document_json).toBe(transport.saves()[0].document_json)
    const published = await transport.call('readPublishedFeedback', { request_id: request.request_id })
    expect(published?.markdown).toContain(text)
  })
})
