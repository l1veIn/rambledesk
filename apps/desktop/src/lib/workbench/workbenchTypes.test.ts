// @vitest-environment jsdom
import { mount, unmount, type ComponentProps } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { workbenchPreviewWorkspace } from '../../dev/workbenchPreviewFixtures'
import { previewHostProfile } from '../../dev/agentPreviewFixtures'
import { createUnavailableWorkbenchCapabilities } from '../capabilities/unavailableCapabilities'
import { TestApplicationTransport } from '../application/testApplicationTransport'
import { decodeFeedbackDraftDocument, type FeedbackDraftSnapshot } from '../feedbackDraftDocument'
import { readWorkbenchState, canSubmitWorkbench } from '../workbenchState'
import { locale } from '../preferences'
import SessionWorkbench from './SessionWorkbench.svelte'
import type { WorkbenchSpec } from '../generated/feedback'
import { snapshotFeedbackDraftMarkdown, updateFeedbackDraftState } from '../feedbackDraftDocument'
import { writable } from 'svelte/store'
import { VOICE_INPUT_CONTEXT, type VoiceInputContext, type VoiceInputState } from '../speech/voiceInputContext'
import type { SpeechTarget } from '../speech/speechDraftQueue'
import { replaceInputText } from '../../test/tiptap'

let view: ReturnType<typeof mount> | undefined
const snapshots: FeedbackDraftSnapshot[] = []
const legacyChoice: WorkbenchSpec = { type: 'single_choice', version: 1, data: {
  prompt: 'Choose a layout', options: [{ id: 'compact', label: 'Compact layout' }, { id: 'roomy', label: 'Roomy layout' }],
} }
beforeEach(() => {
  locale.set('en'); snapshots.length = 0
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  Object.defineProperty(Range.prototype, 'getClientRects', { configurable: true, value: () => [] })
  Object.defineProperty(Range.prototype, 'getBoundingClientRect', { configurable: true, value: () => new DOMRect() })
})
afterEach(async () => { if (view) await unmount(view); view = undefined; document.body.replaceChildren(); vi.unstubAllGlobals() })

function open(index: number, readOnly = false, documentJson?: string, spec?: WorkbenchSpec, extra: Partial<ComponentProps<typeof SessionWorkbench>> = {}, voice?: VoiceInputContext) {
  const workspace = workbenchPreviewWorkspace(index)
  if (spec) workspace.workbench = spec
  workspace.draft.document_json = documentJson ?? null
  view = mount(SessionWorkbench, { target: document.body, context: voice ? new Map([[VOICE_INPUT_CONTEXT, voice]]) : undefined, props: {
    workspace, readOnly, draftDocumentJson: documentJson,
    transport: new TestApplicationTransport(undefined, { initiallyReady: true }),
    capabilities: createUnavailableWorkbenchCapabilities(), resolveHostProfile: previewHostProfile,
    formatTime: () => '', editorDocument: decodeFeedbackDraftDocument(documentJson) ?? { type: 'doc', content: [{ type: 'paragraph' }] },
    onDraftChange: (snapshot) => snapshots.push(snapshot),
    ...extra,
  } })
}
const button = (text: string) => Array.from(document.querySelectorAll('button')).find((button) => button.textContent?.includes(text))!
const latest = () => readWorkbenchState(snapshots.at(-1)?.documentJson)

describe('workbench interaction state is independent of feedback notes', () => {
  it('selects the shared document voice destination on editor focus without starting the microphone', async () => {
    const workspace = workbenchPreviewWorkspace(0)
    const target: SpeechTarget = { requestId: workspace.request.request_id, requestTitle: workspace.request.title,
      destination: { kind: 'document', action: { actionId: 'opening', actionIndex: 0, title: 'Opening' } } }
    const state = writable<VoiceInputState>({ requestId: target.requestId, documentTarget: target, nextTarget: null, recording: false, disabled: false })
    const voice = { state, start: vi.fn(), stop: vi.fn(), selectTarget: vi.fn() }
    open(0, false, undefined, undefined, { workspace }, voice)
    await vi.waitFor(() => expect(document.querySelector('.feedback-prose[contenteditable="true"]')).not.toBeNull())
    document.querySelector('.feedback-prose[contenteditable="true"]')!.dispatchEvent(new FocusEvent('focusin', { bubbles: true }))
    expect(voice.selectTarget).toHaveBeenLastCalledWith(target)
    expect(voice.start).not.toHaveBeenCalled()
    document.querySelector<HTMLButtonElement>('button[aria-label="Speak feedback"]')!.click()
    expect(voice.start).toHaveBeenCalledWith(target)
  })

  it('selects, saves and restores without inserting anything into the editor', async () => {
    open(2, false, undefined, legacyChoice)
    await vi.waitFor(() => expect(document.querySelectorAll('.feedback-prose[contenteditable="true"]')).toHaveLength(1))
    button('Compact layout').click()
    await vi.waitFor(() => expect(latest()).toEqual({ type: 'single_choice', selected_option_id: 'compact' }))
    expect(snapshots.at(-1)!.bodyMarkdown).toBe('')
    expect(JSON.stringify(decodeFeedbackDraftDocument(snapshots.at(-1)!.documentJson))).not.toContain('compact')
    expect(canSubmitWorkbench(legacyChoice, latest(), '')).toBe(true)
    const saved = snapshots.at(-1)!.documentJson
    await unmount(view!); view = undefined; document.body.replaceChildren()
    open(2, false, saved, legacyChoice)
    await vi.waitFor(() => expect(button('Compact layout').getAttribute('aria-pressed')).toBe('true'))
    button('Clear answer').click()
    await vi.waitFor(() => expect(latest()).toEqual({ type: 'single_choice', selected_option_id: null }))
  })

  it('keeps answers when notes change and when editor undo removes the notes', async () => {
    open(2, false, undefined, legacyChoice)
    await vi.waitFor(() => expect(document.querySelector('.feedback-prose[contenteditable="true"]')).not.toBeNull())
    button('Compact layout').click()
    await vi.waitFor(() => expect(latest()?.type).toBe('single_choice'))
    view!.applyDraftOperation({ kind: 'appendClipboardText', text: 'Optional explanation', label: 'Clipboard', action: null })
    await vi.waitFor(() => expect(snapshots.at(-1)?.bodyMarkdown).toContain('Optional explanation'))
    expect(latest()).toEqual({ type: 'single_choice', selected_option_id: 'compact' })
    document.querySelector<HTMLButtonElement>('button[aria-label="Undo"]')!.click()
    await vi.waitFor(() => expect(snapshots.at(-1)?.bodyMarkdown).not.toContain('Optional explanation'))
    expect(latest()).toEqual({ type: 'single_choice', selected_option_id: 'compact' })
  })

  it('asks questions one at a time, supports custom answers, review and editing', async () => {
    open(1)
    await vi.waitFor(() => expect(button('独立开发者')).toBeDefined())
    button('独立开发者').click()
    await vi.waitFor(() => expect(button('快速反馈')).toBeDefined())
    expect(button('独立开发者')).toBeUndefined()
    button('快速反馈').click()
    await vi.waitFor(() => expect(button('Other — write your answer')).toBeDefined())
    button('Other — write your answer').click()
    await vi.waitFor(() => expect(document.querySelector('[data-question-answer]')).not.toBeNull())
    replaceInputText(document.querySelector('[data-question-answer]'), '希望手机上也能使用')
    await vi.waitFor(() => expect(latest()).toMatchObject({ answers: expect.arrayContaining([expect.objectContaining({ id: 'concern', wasCustom: true, value: '希望手机上也能使用' })]) }))
    button('Review answers').click()
    await vi.waitFor(() => expect(document.body.textContent).toContain('All answers are ready'))
    expect(canSubmitWorkbench(workbenchPreviewWorkspace(1).workbench, latest(), '')).toBe(true)
    expect(snapshots.at(-1)!.bodyMarkdown).toBe('')
    button('这个产品最先服务').click()
    await vi.waitFor(() => expect(button('小团队')).toBeDefined())
    button('小团队').click()
    await vi.waitFor(() => expect(latest()).toMatchObject({ answers: expect.arrayContaining([expect.objectContaining({ id: 'audience', value: 'teams', index: 2 })]) }))
    expect(document.querySelector('blockquote[data-action-id]')).toBeNull()
  })

  it('locks answer controls for closed requests', async () => {
    open(2, true, undefined, legacyChoice)
    await vi.waitFor(() => expect(button('Compact layout').disabled).toBe(true))
    button('Compact layout').click()
    expect(snapshots).toHaveLength(0)
  })

  it('gives the dedicated interaction the primary area without a collapsible task brief', async () => {
    open(1)
    await vi.waitFor(() => expect(button('独立开发者')).toBeDefined())
    expect(document.querySelector('[data-workbench-content] [aria-label="Questionnaire"]')).not.toBeNull()
    expect(document.querySelector('[data-request-materials] [aria-label="Questionnaire"]')).toBeNull()
    expect(document.querySelector('button[aria-label="Collapse"]')).toBeNull()
    expect(document.querySelector('button[aria-label="Expand"]')).toBeNull()
  })

  it.each([
    { ...legacyChoice, type: 'future_workbench' },
    { ...legacyChoice, version: 99 },
    { ...legacyChoice, data: { ...legacyChoice.data, future: true } },
  ])('preserves unknown workbenches as read-only instead of offering an unusable feedback fallback', async (spec) => {
    const saved = updateFeedbackDraftState(snapshotFeedbackDraftMarkdown('Saved notes must remain visible'), { type: 'future_workbench', value: 'opaque' })
    open(2, false, saved.documentJson, spec)
    await vi.waitFor(() => expect(document.body.textContent).toContain('Saved notes must remain visible'))
    expect(document.body.textContent).toContain('preserved in read-only mode')
    expect(document.querySelector('.feedback-prose[contenteditable="true"]')).toBeNull()
    expect(document.querySelector('[data-workbench="unsupported"]')).not.toBeNull()
    expect(canSubmitWorkbench(spec, null, 'Saved notes must remain visible')).toBe(false)
    expect(snapshots).toHaveLength(0)
  })

  it('allows cancelling an unsupported request while keeping draft editing and approval unavailable', async () => {
    const workspace = workbenchPreviewWorkspace(2)
    workspace.workbench = { type: 'future_workbench', version: 99, data: {} }
    workspace.request.allow_finish = true
    open(2, false, undefined, undefined, { workspace, canCancel: true })
    await vi.waitFor(() => expect(document.querySelector('[data-workbench="unsupported"]')).not.toBeNull())
    expect(document.querySelector<HTMLButtonElement>('button[aria-label="Cancel feedback"]')?.disabled).toBe(false)
    expect(button('Approve and finish')).toBeUndefined()
    expect(document.querySelector('.feedback-prose[contenteditable="true"]')).toBeNull()
  })

  it.each(['cooking', 'submitting', 'cancelling', 'approving'] as const)('keeps lifecycle actions locked while %s', async (operation) => {
    const workspace = workbenchPreviewWorkspace(0)
    workspace.request.allow_finish = true
    open(0, false, undefined, undefined, { workspace, canCancel: true, [operation]: true })
    await vi.waitFor(() => expect(button('Approve and finish') ?? button('Finishing…')).toBeDefined())
    expect(document.querySelector<HTMLButtonElement>('button[aria-label="Cancel feedback"]')?.disabled).toBe(true)
    expect((button('Approve and finish') ?? button('Finishing…')).disabled).toBe(true)
  })

  it.each([0, 1, 2, 3])('only offers approval for Ramble despite a historical allow_finish flag (type %s)', async (index) => {
    const workspace = workbenchPreviewWorkspace(index)
    workspace.request.allow_finish = true
    open(index, false, undefined, undefined, { workspace, canCancel: true })
    await vi.waitFor(() => expect(document.querySelector('button[aria-label="Cancel feedback"]')).not.toBeNull())
    expect(document.querySelector<HTMLButtonElement>('button[aria-label="Cancel feedback"]')?.disabled).toBe(false)
    if (index === 0) expect(button('Approve and finish')?.disabled).toBe(false)
    else expect(button('Approve and finish')).toBeUndefined()
  })
})
