// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { fromStore, writable, type Writable } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { locale } from '../../preferences'
import { VOICE_INPUT_CONTEXT, type VoiceInputState } from '../../speech/voiceInputContext'
import { emptyRequestSpeechTidy, type RequestSpeechTidyState } from '../../speech/requestSpeechTidy'
import WorkbenchGuide from './WorkbenchGuide.svelte'
import { createWorkbenchTourSeenStore } from './workbenchTourSeen'
import { getWorkbenchTour } from './workbenchTours'

// Keep the real storage implementation, but give each test an independent
// session-memory fallback just as a new browser session would have.
const seen = vi.hoisted(() => ({
  store: null as ReturnType<typeof createWorkbenchTourSeenStore> | null,
  tidy: null as Writable<RequestSpeechTidyState> | null,
}))
vi.mock('../../speech/requestSpeechToolsContext', () => ({ useRequestSpeechTools: () => seen.tidy }))
vi.mock('./workbenchTourSeen', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./workbenchTourSeen')>()
  return {
    ...actual,
    hasSeenWorkbenchTour: (...args: Parameters<typeof actual.hasSeenWorkbenchTour>) => seen.store!.hasSeen(...args),
    markWorkbenchTourSeen: (...args: Parameters<typeof actual.markWorkbenchTourSeen>) => seen.store!.markSeen(...args),
  }
})

let view: ReturnType<typeof mount> | undefined
let scope: HTMLElement
let voiceState: Writable<VoiceInputState>
const dialog = () => document.querySelector<HTMLElement>('dialog[open], [role="dialog"]')
const trigger = () => document.querySelector<HTMLButtonElement>('[data-workbench-guide-trigger]')!
const button = (label: string) => Array.from(dialog()!.querySelectorAll('button')).find((item) => item.textContent?.trim() === label || item.getAttribute('aria-label') === label)!

beforeEach(() => {
  localStorage.clear()
  locale.set('en')
  seen.store = createWorkbenchTourSeenStore()
  seen.tidy = writable({ ...emptyRequestSpeechTidy, requestId: 'request-1', disabled: false })
  voiceState = writable({ requestId: 'request-1', documentTarget: null, nextTarget: null, recording: false, disabled: false })
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue(new DOMRect(40, 70, 280, 90))
  vi.spyOn(HTMLElement.prototype, 'getClientRects').mockReturnValue([new DOMRect(40, 70, 280, 90)] as unknown as DOMRectList)
  HTMLElement.prototype.scrollIntoView = vi.fn()
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  scope = document.createElement('section')
  scope.innerHTML = `
    <div data-tour="request-context">Request context</div>
    <button data-tour="ramble-actions">Choose a shortcut</button>
    <div data-tour="feedback-input"><input value="Existing feedback"></div>
    <div data-tour="input-console">Rambelle</div>
    <p data-review-text="paragraph-1">Original paragraph</p>
    <button data-tour="review-comment">Add comment</button>
    <button data-tour="review-delete">Delete paragraph</button>
    <div data-tour="review-verdict">Review decision</div>
    <button data-feedback-actions>Submit feedback</button>
  `
  document.body.append(scope)
})

afterEach(async () => {
  if (view) await unmount(view)
  view = undefined
  document.body.replaceChildren()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

function open(initial: { kind?: string; requestId?: string; disabled?: boolean; scope?: HTMLElement } = {}) {
  const state = writable({ kind: 'ramble', requestId: 'request-1', disabled: false, scope, ...initial })
  const props = fromStore(state)
  const host = document.createElement('div')
  scope.append(host)
  view = mount(WorkbenchGuide, { target: host, context: new Map([[VOICE_INPUT_CONTEXT, { state: voiceState, start: vi.fn(), stop: vi.fn(), selectTarget: vi.fn() }]]), props: {
    get kind() { return props.current.kind },
    get requestId() { return props.current.requestId },
    get disabled() { return props.current.disabled },
    get scope() { return props.current.scope },
  } })
  return state
}

describe('workbench first-use guide', () => {
  it('opens on the first editable visit and remembers an explicit skip across requests', async () => {
    const state = open()
    await vi.waitFor(() => expect(dialog()?.textContent).toContain('Start with the context'))
    expect(dialog()?.textContent).toContain('Step 1 of 5')
    expect(seen.store!.hasSeen('ramble', 1)).toBe(false)
    button('Skip guide').click()
    await vi.waitFor(() => expect(dialog()).toBeNull())
    expect(JSON.parse(localStorage.getItem('rambledesk.workbench-tour.ramble')!)).toEqual({ version: 1 })
    state.update((current) => ({ ...current, requestId: 'request-2' }))
    await tick()
    expect(dialog()).toBeNull()
  })

  it('navigates all steps and completes without clicking or changing the underlying workbench', async () => {
    const businessAction = vi.fn()
    scope.querySelectorAll('button').forEach((element) => element.addEventListener('click', businessAction))
    open({ kind: 'document_review' })
    const tour = getWorkbenchTour('document_review', 'en')!
    await vi.waitFor(() => expect(dialog()?.textContent).toContain(tour.steps[0].title))
    button('Next').click()
    await vi.waitFor(() => expect(dialog()?.textContent).toContain(tour.steps[1].title))
    button('Back').click()
    await vi.waitFor(() => expect(dialog()?.textContent).toContain(tour.steps[0].title))
    for (const step of tour.steps.slice(1)) {
      button('Next').click()
      await vi.waitFor(() => expect(dialog()?.textContent).toContain(step.title))
    }
    button('Get started').click()
    await vi.waitFor(() => expect(dialog()).toBeNull())
    expect(seen.store!.hasSeen('document_review', 1)).toBe(true)
    expect(businessAction).not.toHaveBeenCalled()
    expect(scope.querySelector('input')!.value).toBe('Existing feedback')
    expect(scope.querySelector('[data-review-text]')!.textContent).toBe('Original paragraph')
  })

  it('keeps first-use completion separate for each workbench and supports manual replay', async () => {
    seen.store!.markSeen('ramble', 1)
    const state = open()
    await tick()
    expect(dialog()).toBeNull()
    trigger().click()
    await vi.waitFor(() => expect(dialog()?.textContent).toContain('Start with the context'))
    button('Skip guide').click()
    await vi.waitFor(() => expect(dialog()).toBeNull())
    state.update((current) => ({ ...current, kind: 'document_review' }))
    await vi.waitFor(() => expect(dialog()?.textContent).toContain('Keep the original, add your feedback'))
    expect(seen.store!.hasSeen('document_review', 1)).toBe(false)
  })

  it('waits for editing to become available and does not persist or restart when an operation interrupts it', async () => {
    const state = open({ disabled: true })
    await tick()
    expect(dialog()).toBeNull()
    trigger()?.click()
    await tick()
    expect(dialog()).toBeNull()
    state.update((current) => ({ ...current, disabled: false }))
    await vi.waitFor(() => expect(dialog()).not.toBeNull())
    state.update((current) => ({ ...current, disabled: true }))
    await vi.waitFor(() => expect(dialog()).toBeNull())
    expect(seen.store!.hasSeen('ramble', 1)).toBe(false)
    state.update((current) => ({ ...current, disabled: false }))
    await tick()
    expect(dialog()).toBeNull()
    trigger().click()
    await vi.waitFor(() => expect(dialog()).not.toBeNull())
  })

  it('restarts at the first step for a new unseen request without completing the interrupted visit', async () => {
    const state = open()
    await vi.waitFor(() => expect(dialog()).not.toBeNull())
    button('Next').click()
    await vi.waitFor(() => expect(dialog()?.textContent).toContain('Start with an action to try'))
    state.update((current) => ({ ...current, requestId: 'request-2' }))
    await vi.waitFor(() => expect(dialog()?.textContent).toContain('Start with the context'))
    expect(seen.store!.hasSeen('ramble', 1)).toBe(false)
    expect(document.querySelectorAll('dialog[open], [role="dialog"]')).toHaveLength(1)
  })

  it('closes a stale type without marking it complete or showing guidance for unsupported workbenches', async () => {
    const state = open()
    await vi.waitFor(() => expect(dialog()).not.toBeNull())
    state.update((current) => ({ ...current, kind: 'questions' }))
    await vi.waitFor(() => expect(dialog()).toBeNull())
    expect(trigger()).toBeNull()
    expect(seen.store!.hasSeen('ramble', 1)).toBe(false)
    state.update((current) => ({ ...current, kind: 'future-workbench' }))
    await tick()
    expect(dialog()).toBeNull()
    expect(trigger()).toBeNull()
  })

  it('defers the first guide until another modal closes without consuming the first-use opportunity', async () => {
    const other = document.createElement('dialog')
    other.setAttribute('open', '')
    document.body.append(other)
    open()
    await tick()
    expect(document.querySelector('[data-workbench-spotlight-tour]')).toBeNull()
    expect(seen.store!.hasSeen('ramble', 1)).toBe(false)
    other.removeAttribute('open')
    await vi.waitFor(() => expect(dialog()?.textContent).toContain('Start with the context'))
    expect(seen.store!.hasSeen('ramble', 1)).toBe(false)
  })

  it('cancels a deferred guide on request change and locking, then allows the next visit to start', async () => {
    const other = document.createElement('div')
    other.setAttribute('role', 'dialog')
    other.setAttribute('aria-modal', 'true')
    document.body.append(other)
    const state = open()
    await tick()
    state.update((current) => ({ ...current, requestId: 'request-2', disabled: true }))
    await tick()
    other.remove()
    await tick()
    expect(dialog()).toBeNull()
    expect(seen.store!.hasSeen('ramble', 1)).toBe(false)
    state.update((current) => ({ ...current, disabled: false }))
    await vi.waitFor(() => expect(dialog()?.textContent).toContain('Start with the context'))
    expect(document.querySelectorAll('[data-workbench-spotlight-tour]')).toHaveLength(1)
    button('Skip guide').click()
    await vi.waitFor(() => expect(dialog()).toBeNull())
  })

  it('resumes an unseen guide when a lock interrupted waiting for another modal in the same request', async () => {
    const other = document.createElement('dialog')
    other.setAttribute('open', '')
    document.body.append(other)
    const state = open()
    await tick()
    state.update((current) => ({ ...current, disabled: true }))
    await tick()
    other.remove()
    await tick()
    expect(dialog()).toBeNull()
    expect(seen.store!.hasSeen('ramble', 1)).toBe(false)
    state.update((current) => ({ ...current, disabled: false }))
    await vi.waitFor(() => expect(dialog()?.textContent).toContain('Start with the context'))
  })

  it.each(['recording', 'tidying'] as const)('waits while %s and prevents replay from covering active input', async (activity) => {
    function setBusy(busy: boolean) {
      if (activity === 'recording') voiceState.update((current) => ({ ...current, recording: busy }))
      else seen.tidy!.update((current) => ({ ...current, busy }))
    }
    setBusy(true)
    open()
    await tick()
    expect(dialog()).toBeNull()
    expect(trigger().disabled).toBe(true)
    trigger().click()
    await tick()
    expect(dialog()).toBeNull()
    setBusy(false)
    await vi.waitFor(() => expect(dialog()).not.toBeNull())
    setBusy(true)
    await vi.waitFor(() => expect(dialog()).toBeNull())
    expect(seen.store!.hasSeen('ramble', 1)).toBe(false)
    expect(trigger().disabled).toBe(true)
    setBusy(false)
    await tick()
    expect(dialog()).toBeNull()
    trigger().click()
    await vi.waitFor(() => expect(dialog()).not.toBeNull())
  })

})
