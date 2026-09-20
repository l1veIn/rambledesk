// @vitest-environment jsdom
import { mount, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { workbenchPreviewWorkspace } from '../../dev/workbenchPreviewFixtures'
import { previewHostProfile } from '../../dev/agentPreviewFixtures'
import { createUnavailableWorkbenchCapabilities } from '../capabilities/unavailableCapabilities'
import { TestApplicationTransport } from '../application/testApplicationTransport'
import { decodeFeedbackDraftDocument, type FeedbackDraftSnapshot } from '../feedbackDraftDocument'
import { readWorkbenchState, canSubmitWorkbench } from '../workbenchState'
import { locale } from '../preferences'
import SessionWorkbench from './SessionWorkbench.svelte'

let view: ReturnType<typeof mount> | undefined
const snapshots: FeedbackDraftSnapshot[] = []
beforeEach(() => {
  locale.set('en'); snapshots.length = 0
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  Object.defineProperty(Range.prototype, 'getClientRects', { configurable: true, value: () => [] })
  Object.defineProperty(Range.prototype, 'getBoundingClientRect', { configurable: true, value: () => new DOMRect() })
})
afterEach(async () => { if (view) await unmount(view); view = undefined; document.body.replaceChildren(); vi.unstubAllGlobals() })

function open(index: number, readOnly = false, documentJson?: string) {
  const workspace = workbenchPreviewWorkspace(index)
  workspace.draft.document_json = documentJson ?? null
  view = mount(SessionWorkbench, { target: document.body, props: {
    workspace, readOnly, draftDocumentJson: documentJson,
    transport: new TestApplicationTransport(undefined, { initiallyReady: true }),
    capabilities: createUnavailableWorkbenchCapabilities(), resolveHostProfile: previewHostProfile,
    formatTime: () => '', editorDocument: decodeFeedbackDraftDocument(documentJson) ?? { type: 'doc', content: [{ type: 'paragraph' }] },
    onDraftChange: (snapshot) => snapshots.push(snapshot),
  } })
}
const button = (text: string) => Array.from(document.querySelectorAll('button')).find((button) => button.textContent?.includes(text))!
const latest = () => readWorkbenchState(snapshots.at(-1)?.documentJson)

describe('workbench interaction state is independent of feedback notes', () => {
  it('selects, saves and restores without inserting anything into the editor', async () => {
    open(2)
    await vi.waitFor(() => expect(document.querySelectorAll('.feedback-prose[contenteditable="true"]')).toHaveLength(1))
    document.querySelector<HTMLInputElement>('input[value="compact"]')!.click()
    await vi.waitFor(() => expect(latest()).toEqual({ type: 'single_choice', selected_option_id: 'compact' }))
    expect(snapshots.at(-1)!.bodyMarkdown).toBe('')
    expect(JSON.stringify(decodeFeedbackDraftDocument(snapshots.at(-1)!.documentJson))).not.toContain('compact')
    expect(canSubmitWorkbench(workbenchPreviewWorkspace(2).workbench, latest(), '')).toBe(true)
    const saved = snapshots.at(-1)!.documentJson
    await unmount(view!); view = undefined; document.body.replaceChildren()
    open(2, false, saved)
    await vi.waitFor(() => expect(document.querySelector<HTMLInputElement>('input[value="compact"]')!.checked).toBe(true))
    button('Clear selection').click()
    await vi.waitFor(() => expect(latest()).toEqual({ type: 'single_choice', selected_option_id: null }))
  })

  it('keeps answers when notes change and when editor undo removes the notes', async () => {
    open(2)
    await vi.waitFor(() => expect(document.querySelector('.feedback-prose[contenteditable="true"]')).not.toBeNull())
    document.querySelector<HTMLInputElement>('input[value="compact"]')!.click()
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
    await vi.waitFor(() => expect(document.querySelector('textarea')).not.toBeNull())
    const input = document.querySelector('textarea')!
    input.value = '希望手机上也能使用'
    input.dispatchEvent(new Event('input', { bubbles: true }))
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
    open(2, true)
    await vi.waitFor(() => expect(document.querySelector('fieldset')?.disabled).toBe(true))
    document.querySelector<HTMLInputElement>('input[value="compact"]')!.click()
    expect(snapshots).toHaveLength(0)
  })
})
