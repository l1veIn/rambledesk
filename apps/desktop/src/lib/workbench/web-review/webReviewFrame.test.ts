// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createWebReviewFrame } from './webReviewFrame'
import { WEB_REVIEW_PROTOCOL } from './webReviewProtocol'

const page = { page_url: 'https://review.example/app', page_title: 'App', viewport: { width: 1440, height: 900, scroll_x: 0, scroll_y: 0 } }
const selection = { ...page, selector: '#signup', tag_name: 'button', text: '开始创作', attributes: { id: 'signup' }, rect: { x: 24, y: 40, width: 120, height: 40 }, captured_at: '2026-09-30T00:00:00.000Z' }
const cleanups: (() => void)[] = []
afterEach(() => { cleanups.splice(0).forEach(fn => fn()); document.body.replaceChildren(); vi.useRealTimers() })

function harness() {
  const iframe = document.createElement('iframe')
  document.body.append(iframe)
  const post = vi.spyOn(iframe.contentWindow!, 'postMessage').mockImplementation(() => undefined)
  const onState = vi.fn()
  const onSelection = vi.fn()
  const onAnchor = vi.fn()
  const onAnnotationClick = vi.fn()
  const onFocusMissing = vi.fn()
  const controller = createWebReviewFrame({ iframe, url: page.page_url, onState, onSelection, onAnchor, onAnnotationClick, onFocusMissing })
  cleanups.push(controller.dispose)
  controller.connect()
  const channel = post.mock.calls.find(([message]) => message.type === 'init')![0].channel
  const message = (type: string, payload: Record<string, unknown> = {}, override: Partial<MessageEventInit> = {}) => window.dispatchEvent(new MessageEvent('message', {
    source: iframe.contentWindow, origin: 'https://review.example', data: { protocol: WEB_REVIEW_PROTOCOL, channel, type, ...payload }, ...override,
  }))
  return { controller, post, onState, onSelection, onAnchor, onAnnotationClick, onFocusMissing, message, channel }
}

describe('web review frame boundary', () => {
  it('requires the configured origin, exact frame, current channel and bounded DOM context', () => {
    const h = harness()
    h.message('ready', page, { origin: 'https://attacker.example' })
    h.message('ready', page, { source: window })
    h.message('ready', page, { data: { protocol: WEB_REVIEW_PROTOCOL, channel: 'old-channel', type: 'ready', ...page } })
    expect(h.onState.mock.calls.at(-1)![0].status).toBe('loading')
    h.message('selection', { selection })
    expect(h.onSelection).not.toHaveBeenCalled()
    h.message('ready', page)
    expect(h.onState.mock.calls.at(-1)![0].status).toBe('ready')
    h.message('selection', { selection: { ...selection, text: 'x'.repeat(2001) } })
    h.message('selection', { selection: { ...selection, text: 'x\ud83d' } })
    h.message('selection', { selection: { ...selection, tag_name: 'foreignObject' } })
    h.message('selection', { selection: { ...selection, viewport: { ...selection.viewport, width: 100 } } })
    h.message('selection', { selection: { ...selection, rect: { ...selection.rect, y: 1_000_001 } } })
    h.message('selection', { selection: { ...selection, page_url: 'https://attacker.example' } })
    expect(h.onSelection).not.toHaveBeenCalled()
    h.message('selection', { selection })
    expect(h.onSelection).toHaveBeenCalledExactlyOnceWith(selection)
    expect(h.onAnchor).toHaveBeenLastCalledWith(selection.rect)
    h.message('selection', { selection: { ...selection, text: 'x'.repeat(1999) + '😀' } })
    expect(h.onSelection).toHaveBeenCalledTimes(2)
  })

  it('queues markers, mode and focus until the bridge handshake completes', () => {
    const h = harness()
    const marker = { id: 'note-1', number: 1, selector: '#signup', page_url: page.page_url }
    h.controller.setMode('select')
    h.controller.setAnnotations([marker], 'note-1')
    h.controller.focus(marker)
    h.message('ready', page)
    expect(h.post.mock.calls.map(([data]) => data)).toEqual(expect.arrayContaining([
      expect.objectContaining({ type: 'mode', mode: 'select' }),
      expect.objectContaining({ type: 'markers', markers: [marker], selected_id: 'note-1' }),
      expect.objectContaining({ type: 'focus', selector: '#signup', page_url: page.page_url }),
    ]))
    h.message('annotation_click', { id: 'unregistered-note' })
    expect(h.onAnnotationClick).not.toHaveBeenCalled()
    h.message('annotation_click', { id: 'note-1' })
    expect(h.onAnnotationClick).toHaveBeenCalledWith('note-1')
    h.message('focus_missing', { reason: 'element_missing' })
    expect(h.onFocusMissing).toHaveBeenCalledWith('element_missing')
  })

  it('reports a missing bridge without accepting delayed stale sessions after reconnect', () => {
    vi.useFakeTimers()
    const h = harness()
    vi.advanceTimersByTime(6000)
    expect(h.onState.mock.calls.at(-1)![0]).toMatchObject({ status: 'unavailable', reason: 'bridge_missing' })
    h.controller.connect()
    h.message('ready', page)
    expect(h.onState.mock.calls.at(-1)![0].status).toBe('loading')
    h.controller.dispose()
    h.message('selection', { selection })
    expect(h.onSelection).not.toHaveBeenCalled()
  })

  it('rejects unsafe review URLs before messaging any frame', () => {
    const iframe = document.createElement('iframe')
    document.body.append(iframe)
    const post = vi.spyOn(iframe.contentWindow!, 'postMessage')
    const onState = vi.fn()
    const controller = createWebReviewFrame({ iframe, url: 'javascript:alert(1)', onState, onSelection: vi.fn() })
    cleanups.push(controller.dispose)
    controller.connect()
    expect(onState).toHaveBeenCalledWith(expect.objectContaining({ status: 'unavailable', reason: 'invalid_url' }))
    expect(post).not.toHaveBeenCalled()
  })

  it('cleans up timers and ignores messages even when a detached frame rejects the closing notification', () => {
    vi.useFakeTimers()
    const h = harness()
    expect(vi.getTimerCount()).toBeGreaterThan(0)
    h.post.mockImplementation(() => { throw new TypeError('Closed frame origin is unavailable') })
    expect(() => h.controller.dispose()).not.toThrow()
    expect(vi.getTimerCount()).toBe(0)
    h.onState.mockClear()
    h.onAnchor.mockClear()
    h.message('ready', page)
    h.message('selection', { selection })
    vi.advanceTimersByTime(10_000)
    expect(h.onState).not.toHaveBeenCalled()
    expect(h.onSelection).not.toHaveBeenCalled()
    expect(h.onAnchor).not.toHaveBeenCalled()
    expect(() => h.controller.dispose()).not.toThrow()
  })
})
