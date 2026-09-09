// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import defaultPayload from '../../../../../crates/rambledesk-hosts/tests/fixtures/manual_resume_prompt.json'
import type { ResumePrompt } from '../domain/resumePrompt'
import { locale } from '../preferences'
import ResumePromptDialog from './ResumePromptDialog.svelte'

let view: ReturnType<typeof mount> | undefined
beforeEach(() => {
  locale.set('en')
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }))
  Element.prototype.getAnimations = (() => []) as never
})
afterEach(async () => {
  if (view) await unmount(view)
  view = undefined
  document.body.replaceChildren()
  vi.unstubAllGlobals()
  locale.set('en')
})

describe('resume prompt presentation from the real host payload', () => {
  it('localizes product guidance with the Client locale and keeps the copied protocol text intact', async () => {
    view = mount(ResumePromptDialog, { target: document.body, props: { prompt: defaultPayload as ResumePrompt } })
    await tick()
    expect(document.querySelector('[role="dialog"]')?.textContent).toContain('Feedback submitted · return to host')
    expect(document.querySelector('[role="dialog"]')?.textContent).toContain('Return to acceptance-external')
    const originalCopy = document.querySelector<HTMLTextAreaElement>('#resume-prompt-text')!.value

    locale.set('zh-CN')
    await tick()
    expect(document.querySelector('[role="dialog"]')?.textContent).toContain('反馈已提交 · 回到宿主点继续')
    expect(document.querySelector('[role="dialog"]')?.textContent).toContain('先回到 acceptance-external 的对话')
    expect(document.querySelector<HTMLTextAreaElement>('#resume-prompt-text')!.value).toBe(originalCopy)
    expect(originalCopy).toBe(defaultPayload.resume_prompt)
  })

  it('never translates unmarked custom host guidance or its resume command', async () => {
    const custom: ResumePrompt = {
      ...defaultPayload, reason: 'completed', default_presentation: undefined,
      host_label: '我的宿主', title: '宿主自定义标题', body: '请遵守宿主自己的恢复步骤。',
      resume_prompt: '保留用户命令原文 get_feedback(request_id="custom")',
    }
    view = mount(ResumePromptDialog, { target: document.body, props: { prompt: custom } })
    await tick()
    for (const language of ['en', 'zh-CN'] as const) {
      locale.set(language)
      await tick()
      const dialog = document.querySelector('[role="dialog"]')!
      expect(dialog.textContent).toContain(custom.title)
      expect(dialog.textContent).toContain(custom.body)
      expect(dialog.textContent).toContain(custom.host_label)
      expect(document.querySelector<HTMLTextAreaElement>('#resume-prompt-text')!.value).toBe(custom.resume_prompt)
    }
  })
})
