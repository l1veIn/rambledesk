// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { locale } from '$lib/preferences'
import SessionStarterCards from './SessionStarterCards.svelte'
import { getSessionStarters } from './sessionStarters'

let view: ReturnType<typeof mount> | undefined
const cards = () => [...document.querySelectorAll<HTMLButtonElement>('[data-session-starters] button')]

beforeEach(() => locale.set('en'))
afterEach(async () => {
  if (view) await unmount(view)
  view = undefined
  document.body.replaceChildren()
})

describe('session starter cards', () => {
  it('passes a selected prompt to the draft without submitting the surrounding form', async () => {
    const form = document.createElement('form')
    const onSubmit = vi.fn((event: Event) => event.preventDefault())
    const onSelect = vi.fn()
    form.addEventListener('submit', onSubmit)
    document.body.append(form)
    view = mount(SessionStarterCards, { target: form, props: { onSelect } })
    await tick()
    expect(onSelect).not.toHaveBeenCalled()
    expect(cards().map(card => card.getAttribute('aria-label'))).toEqual(['Brainstorm', 'Improve a system', 'Develop code'])
    for (const [index, card] of cards().entries()) {
      card.click()
      expect(onSelect).toHaveBeenNthCalledWith(index + 1, getSessionStarters('en')[index].prompt)
    }
    expect(onSelect).toHaveBeenCalledTimes(3)
    expect(onSubmit).not.toHaveBeenCalled()
  })

  it('cannot change a draft while disabled', async () => {
    const onSelect = vi.fn()
    view = mount(SessionStarterCards, { target: document.body, props: { disabled: true, onSelect } })
    await tick()
    expect(cards()).toHaveLength(3)
    for (const card of cards()) {
      expect(card.disabled).toBe(true)
      card.click()
    }
    expect(onSelect).not.toHaveBeenCalled()
  })

  it('updates both visible labels and selected prompts when the app language changes', async () => {
    const onSelect = vi.fn()
    view = mount(SessionStarterCards, { target: document.body, props: { onSelect } })
    await tick()
    locale.set('zh-CN')
    await tick()
    expect(cards().map(card => card.getAttribute('aria-label'))).toEqual(['头脑风暴', '系统优化', '代码开发'])
    expect(document.body.textContent).toContain('点击填入提示词，修改后再发送。')
    cards()[1].click()
    expect(onSelect).toHaveBeenCalledExactlyOnceWith(getSessionStarters('zh-CN')[1].prompt)
  })

  it.each(['en', 'zh-CN'] as const)('starts with one bounded human-feedback round in %s', language => {
    const prompts = getSessionStarters(language)
    for (const starter of prompts) {
      expect(starter.prompt).toContain('RambleDesk')
      expect(starter.prompt).toContain(starter.id === 'optimize' ? 'ramble' : 'questions')
      expect(starter.prompt).toContain(language === 'en' ? 'wait for my reply' : '等待我回复')
      expect(starter.prompt).toContain(language === 'en' ? 'Do not start implementing or scan the whole repository' : '不要直接开始实现或扫描整个仓库')
    }
  })
})
