// @vitest-environment jsdom
import { mount, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import WorkbenchPreview from './WorkbenchPreview.svelte'
import { locale, speechAutoTidy, speechConfirmBeforeWrite, speechOverlayEnabled } from '../lib/preferences'
import { inputText, replaceInputText } from '../test/tiptap'

let view: ReturnType<typeof mount> | undefined
const button = (text: string) => Array.from(document.querySelectorAll('button')).find((item) => item.textContent?.trim() === text)!
const input = () => document.querySelector<HTMLInputElement>('input[aria-label="模拟转写内容"]')!
beforeEach(() => {
  history.replaceState({}, '', '/workbenches.html?type=document_review&voice=1')
  locale.set('en'); speechConfirmBeforeWrite.set(false); speechAutoTidy.set(false); speechOverlayEnabled.set(true)
  HTMLElement.prototype.scrollIntoView = vi.fn()
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  Object.defineProperty(Range.prototype, 'getClientRects', { configurable: true, value: () => [] })
  Object.defineProperty(Range.prototype, 'getBoundingClientRect', { configurable: true, value: () => new DOMRect() })
})
afterEach(async () => { if (view) await unmount(view); view = undefined; history.replaceState({}, '', '/'); document.body.replaceChildren(); vi.unstubAllGlobals() })

describe('simulated voice workbench preview uses the shared controller and writer', () => {
  it('tidies all marked speech across inputs and hides the footer action after completion', async () => {
    history.replaceState({}, '', '/workbenches.html?type=questions&voice=1')
    view = mount(WorkbenchPreview, { target: document.body })
    button('Other — write your answer').click()
    await vi.waitFor(() => expect(document.querySelector('[data-question-answer="audience"]')).not.toBeNull())
    const answer = document.querySelector<HTMLElement>('[data-question-answer="audience"]')!
    replaceInputText(answer, 'Typed background stays exactly here.')
    document.querySelector<HTMLButtonElement>('[aria-label="Speak answer"]')!.click()
    await vi.waitFor(() => expect(button('开始一段语音').disabled).toBe(false))
    input().value = 'um, Spoken answer'
    input().dispatchEvent(new Event('input', { bubbles: true }))
    button('开始一段语音').click()
    await vi.waitFor(() => expect(button('完成转写').disabled).toBe(false))
    button('完成转写').click()
    await vi.waitFor(() => expect(button('Tidy 1 speech segments')).toBeDefined())
    await vi.waitFor(() => expect(answer.querySelector('.speech-origin-marker[data-cleanup-state="pending"]')).not.toBeNull())
    expect(inputText(answer)).toBe('Typed background stays exactly here.\num, Spoken answer')
    expect(document.querySelector('.speech-capsule')).toBeNull()
    document.querySelector<HTMLButtonElement>('[aria-label="Speak feedback"]')!.click()
    input().value = 'um, Spoken notes'
    input().dispatchEvent(new Event('input', { bubbles: true }))
    button('开始一段语音').click()
    await vi.waitFor(() => expect(button('完成转写').disabled).toBe(false))
    button('完成转写').click()
    await vi.waitFor(() => expect(button('Tidy 2 speech segments')).toBeDefined())
    button('Tidy 2 speech segments').click()
    await vi.waitFor(() => expect(inputText(answer)).toBe('Typed background stays exactly here.\nSpoken answer'))
    expect(answer.querySelector('.speech-origin-marker')).toBeNull()
    expect(document.querySelector('.feedback-prose[contenteditable="true"]')?.textContent).toBe('Spoken notes')
    expect(document.querySelector('[data-speech-origin-badge]')?.textContent).toContain('1 speech segments tidied')
    expect(button('Tidy 0 speech segments')).toBeUndefined()
    expect(document.querySelector('[data-request-input-console] button')).toBeNull()
    expect(document.querySelector('[data-request-input-caption]')).toBeNull()
    expect(document.querySelector('.feedback-column [data-request-input-tools]')).toBeNull()
  })

  it('pins a custom answer while moving between questions and resumes the next question without advancing', async () => {
    history.replaceState({}, '', '/workbenches.html?type=questions&voice=1')
    view = mount(WorkbenchPreview, { target: document.body })
    button('Other — write your answer').click()
    await vi.waitFor(() => expect(document.querySelector('[data-question-answer="audience"]')).not.toBeNull())
    document.querySelector<HTMLButtonElement>('[aria-label="Speak answer"]')!.click()
    await vi.waitFor(() => expect(button('开始一段语音').disabled).toBe(false))
    input().value = 'Small studios with two designers'
    input().dispatchEvent(new Event('input', { bubbles: true }))
    button('开始一段语音').click()
    await vi.waitFor(() => expect(button('完成转写').disabled).toBe(false))
    button('2 重点').click()
    await vi.waitFor(() => expect(document.querySelector('h3')?.textContent).toContain('第一版'))
    button('Other — write your answer').click()
    await vi.waitFor(() => expect(document.querySelector('[data-question-answer="priority"]')).not.toBeNull())
    document.querySelector<HTMLElement>('[data-question-answer="priority"]')!.focus()
    button('完成转写').click()
    await vi.waitFor(() => expect(button('✓ 用户')).toBeDefined())
    expect(inputText(document.querySelector('[data-question-answer="priority"]'))).toBe('')
    input().value = 'Fast voice feedback while reviewing'
    input().dispatchEvent(new Event('input', { bubbles: true }))
    button('开始一段语音').click()
    await vi.waitFor(() => expect(button('完成转写').disabled).toBe(false))
    button('完成转写').click()
    await vi.waitFor(() => expect(inputText(document.querySelector('[data-question-answer="priority"]'))).toBe('Fast voice feedback while reviewing'))
    expect(document.querySelector('h3')?.textContent).toContain('第一版')
    button('✓ 用户').click()
    await vi.waitFor(() => expect(inputText(document.querySelector('[data-question-answer="audience"]'))).toBe('Small studios with two designers'))
    expect(document.querySelector('.feedback-prose')?.textContent).not.toContain('Small studios')
  })

  it('preserves a fixed choice when an earlier custom-answer segment finishes late', async () => {
    history.replaceState({}, '', '/workbenches.html?type=questions&voice=1')
    view = mount(WorkbenchPreview, { target: document.body })
    button('Other — write your answer').click()
    await vi.waitFor(() => expect(document.querySelector('[aria-label="Speak answer"]')).not.toBeNull())
    document.querySelector<HTMLButtonElement>('[aria-label="Speak answer"]')!.click()
    await vi.waitFor(() => expect(button('开始一段语音').disabled).toBe(false))
    input().value = 'Keep these words for review'
    input().dispatchEvent(new Event('input', { bubbles: true }))
    button('开始一段语音').click()
    await vi.waitFor(() => expect(button('完成转写').disabled).toBe(false))
    document.querySelector<HTMLButtonElement>('[data-answer-option]')!.click()
    button('完成转写').click()
    await vi.waitFor(() => expect(document.querySelector('.speech-capsule')?.textContent).toContain('The custom answer was cleared or changed.'))
    button('✓ 用户').click()
    await vi.waitFor(() => expect(document.querySelector('[data-answer-option][aria-pressed="true"]')?.textContent).toContain('独立开发者'))
    expect(document.querySelector('[data-question-answer]')).toBeNull()
    expect(document.querySelector('.speech-capsule')?.textContent).toContain('Keep these words for review')
  })

  it('starts only on request and retains a spoken comment target when focus moves to notes mid-segment', async () => {
    view = mount(WorkbenchPreview, { target: document.body })
    expect(button('开始一段语音').disabled).toBe(true)
    const source = document.querySelector('[data-review-text]')!.textContent
    document.querySelector<HTMLButtonElement>('button[aria-label="Speak comment on paragraph 1"]')!.click()
    await vi.waitFor(() => expect(button('开始一段语音').disabled).toBe(false))
    input().value = 'Pin this to the original comment'
    input().dispatchEvent(new Event('input', { bubbles: true }))
    button('开始一段语音').click()
    await vi.waitFor(() => expect(button('完成转写').disabled).toBe(false))
    const editor = document.querySelector<HTMLElement>('.feedback-prose[contenteditable="true"]')!
    editor.dispatchEvent(new FocusEvent('focusin', { bubbles: true }))
    button('完成转写').click()
    await vi.waitFor(() => expect(inputText(document.querySelector('[data-review-field="body"]'))).toBe('Pin this to the original comment'))
    expect(editor.textContent).not.toContain('Pin this')
    expect(document.querySelector('[data-review-text]')!.textContent).toBe(source)
    input().value = 'The next segment belongs to feedback notes'
    input().dispatchEvent(new Event('input', { bubbles: true }))
    button('开始一段语音').click()
    await vi.waitFor(() => expect(button('完成转写').disabled).toBe(false))
    button('完成转写').click()
    await vi.waitFor(() => expect(editor.textContent).toContain('The next segment belongs to feedback notes'))
    expect(inputText(document.querySelector('[data-review-field="body"]'))).toBe('Pin this to the original comment')
  })
})
