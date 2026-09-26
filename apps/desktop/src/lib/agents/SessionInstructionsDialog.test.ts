// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { fromStore, writable } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { SessionConnectionState } from '$lib/generated/feedback'
import { locale } from '$lib/preferences'
import SessionInstructionsDialog from './SessionInstructionsDialog.svelte'

const instructions = '# RambleDesk feedback workflow\n\n审阅具体文稿 → `document_review`。\n<literal>&text\n\nChoose `questions` for explicit answers.\n'
let view: ReturnType<typeof mount> | undefined
let writeText: ReturnType<typeof vi.fn>
const dialog = () => document.querySelector<HTMLElement>('[role="dialog"]')
const trigger = () => document.querySelector<HTMLButtonElement>('[aria-label="Built-in session instructions"]')!
const button = (label: string) => [...document.querySelectorAll<HTMLButtonElement>('button')].find((element) => element.textContent?.trim() === label)!
const instructionText = () => document.querySelector<HTMLTextAreaElement>('[aria-label="Instruction text"]')!

beforeEach(() => {
  locale.set('en')
  writeText = vi.fn().mockResolvedValue(undefined)
  vi.stubGlobal('navigator', { ...navigator, clipboard: { writeText } })
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }))
  Element.prototype.getAnimations = (() => []) as never
  HTMLElement.prototype.scrollIntoView = vi.fn()
})
afterEach(async () => {
  if (view) await unmount(view)
  view = undefined
  document.body.replaceChildren()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})
function mountDialog(text: string | undefined = instructions, connection: SessionConnectionState = 'connected') {
  view = mount(SessionInstructionsDialog, { target: document.body, props: { instructions: text, connection } })
}
async function openDialog() {
  await tick()
  trigger().focus()
  trigger().click()
  await vi.waitFor(() => expect(dialog()).not.toBeNull())
}

describe('built-in session instructions', () => {
  it('opens the complete application-provided text as a read-only, selectable document', async () => {
    mountDialog()
    expect(dialog()).toBeNull()
    await openDialog()
    await vi.waitFor(() => expect(document.activeElement).toBe(dialog()?.querySelector('[role="heading"]')))
    expect(instructionText().value).toBe(instructions)
    expect(instructionText().readOnly).toBe(true)
    expect(instructionText().disabled).toBe(false)
    expect(dialog()?.textContent).toContain('Current connection')
    expect(dialog()?.textContent).toContain('do not appear as messages in the conversation history')
    expect(document.querySelector('literal')).toBeNull()
    expect(writeText).not.toHaveBeenCalled()
  })

  it('copies the exact text, including line breaks, and reports completion', async () => {
    mountDialog()
    await openDialog()
    button('Copy instructions').click()
    await vi.waitFor(() => expect(button('Copied')).toBeDefined())
    expect(writeText).toHaveBeenCalledExactlyOnceWith(instructions)
    expect(instructionText().value).toBe(instructions)
  })

  it('keeps the text available after clipboard failure and allows a successful retry', async () => {
    writeText.mockRejectedValueOnce(new Error('Clipboard denied'))
    mountDialog()
    await openDialog()
    button('Copy instructions').click()
    await vi.waitFor(() => expect(document.querySelector('[role="alert"]')?.textContent).toContain('Select the text and copy it manually'))
    expect(instructionText().value).toBe(instructions)
    expect(button('Copy instructions').disabled).toBe(false)
    button('Copy instructions').click()
    await vi.waitFor(() => expect(button('Copied')).toBeDefined())
    expect(document.querySelector('[role="alert"]')).toBeNull()
    expect(writeText.mock.calls).toEqual([[instructions], [instructions]])
  })

  it.each(['Close', 'Escape'])('returns focus to the instructions entry when dismissed with %s', async (dismissal) => {
    mountDialog()
    const entry = trigger()
    await openDialog()
    if (dismissal === 'Close') button('Close').click()
    else document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }))
    await vi.waitFor(() => expect(dialog()).toBeNull())
    await vi.waitFor(() => expect(document.activeElement).toBe(entry))
    expect(writeText).not.toHaveBeenCalled()
  })

  it('shows a clear unavailable state for an older connected runtime without instructions', async () => {
    view = mount(SessionInstructionsDialog, { target: document.body, props: { connection: 'connected' } })
    await openDialog()
    expect(document.querySelector('[role="status"]')?.textContent).toBe('This connection did not provide built-in instructions.')
    expect(instructionText()).toBeNull()
    expect(button('Copy instructions')).toBeUndefined()
    expect(button('Close')).toBeDefined()
  })

  it('explains how to obtain instructions before the first connection', async () => {
    view = mount(SessionInstructionsDialog, { target: document.body, props: { connection: 'stopped' } })
    await openDialog()
    expect(document.querySelector('[role="status"]')?.textContent).toContain('Connect the agent to read the instructions supplied by the application.')
    expect(instructionText()).toBeNull()
    expect(button('Copy instructions')).toBeUndefined()
  })

  it('labels cached instructions as belonging to the most recent connection after disconnecting', async () => {
    const connection = fromStore(writable<SessionConnectionState>('connected'))
    view = mount(SessionInstructionsDialog, { target: document.body, props: { instructions, get connection() { return connection.current } } })
    await openDialog()
    expect(dialog()?.textContent).toContain('Current connection')
    connection.current = 'stopped'
    await vi.waitFor(() => expect(dialog()?.textContent).toContain('Most recent connection'))
    expect(dialog()?.textContent).toContain('not a record of every past turn')
    expect(instructionText().value).toBe(instructions)
    button('Copy instructions').click()
    await vi.waitFor(() => expect(writeText).toHaveBeenCalledWith(instructions))
  })

  it('resets a pending copy when a new connection supplies updated instructions', async () => {
    let finish!: () => void
    writeText.mockImplementationOnce(() => new Promise<void>((resolve) => { finish = resolve }))
    const text = fromStore(writable(instructions))
    view = mount(SessionInstructionsDialog, { target: document.body, props: { get instructions() { return text.current }, connection: 'connected' } })
    await openDialog()
    button('Copy instructions').click()
    await vi.waitFor(() => expect(button('Copying…')?.disabled).toBe(true))
    text.current = 'Updated instructions for the new connection'
    await vi.waitFor(() => expect(instructionText().value).toBe(text.current))
    finish()
    await vi.waitFor(() => expect(button('Copy instructions')?.disabled).toBe(false))
    expect(button('Copied')).toBeUndefined()
    button('Copy instructions').click()
    await vi.waitFor(() => expect(button('Copied')).toBeDefined())
    expect(writeText.mock.calls).toEqual([[instructions], [text.current]])
  })

  it('does not report an earlier pending copy as completed after reopening the dialog', async () => {
    let finish!: () => void
    writeText.mockImplementationOnce(() => new Promise<void>((resolve) => { finish = resolve }))
    mountDialog()
    await openDialog()
    button('Copy instructions').click()
    await vi.waitFor(() => expect(button('Copying…')?.disabled).toBe(true))
    button('Close').click()
    await vi.waitFor(() => expect(dialog()).toBeNull())
    await openDialog()
    finish()
    await vi.waitFor(() => expect(button('Copy instructions')?.disabled).toBe(false))
    expect(button('Copied')).toBeUndefined()
    expect(instructionText().value).toBe(instructions)
  })
})
