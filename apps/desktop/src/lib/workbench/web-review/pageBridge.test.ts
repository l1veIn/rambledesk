import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { afterEach, describe, expect, it, vi } from 'vitest'

type BridgeDom = Readonly<{ window: Window & typeof globalThis }>
const { JSDOM } = createRequire(import.meta.url)('jsdom') as {
  JSDOM: new (html: string, options: { url: string; runScripts: string }) => BridgeDom
}
const script = readFileSync(new URL('../../../../public/rambledesk-web-review.js', import.meta.url), 'utf8')
const doms: BridgeDom[] = []
afterEach(() => doms.splice(0).forEach(dom => dom.window.close()))

function harness() {
  const dom = new JSDOM('<!doctype html><title>Dev app</title><button id="signup">开始创作</button><input id="password" type="password" value="secret-token">', { url: 'https://development.example/', runScripts: 'outside-only' })
  doms.push(dom)
  const window = dom.window
  const postMessage = vi.fn()
  const parent = { postMessage }
  Object.defineProperty(window, 'parent', { value: parent })
  Object.defineProperty(window, 'CSS', { value: { escape: (value: string) => value } })
  window.requestAnimationFrame = (callback: FrameRequestCallback) => { callback(0); return 1 }
  window.HTMLElement.prototype.getBoundingClientRect = () => ({ x: 20, y: 40, width: 120, height: 40, top: 40, right: 140, bottom: 80, left: 20, toJSON() {} })
  window.HTMLElement.prototype.scrollIntoView = vi.fn()
  window.eval(script)
  const message = (type: string, payload = {}, source: unknown = parent, origin = 'https://rambledesk.example') => window.dispatchEvent(new window.MessageEvent('message', {
    source: source as Window, origin, data: { protocol: 'rambledesk.web-review.v1', channel: 'review-channel', type, ...payload },
  }))
  return { window, parent, postMessage, message, signup: window.document.getElementById('signup') as HTMLButtonElement }
}

describe('reviewed page bridge', () => {
  it('keeps the real page interactive until explicit selection mode, then captures DOM context without executing the click', () => {
    const h = harness()
    const appClick = vi.fn()
    h.signup.addEventListener('click', appClick)
    h.signup.click()
    expect(appClick).toHaveBeenCalledTimes(1)
    expect(h.postMessage).not.toHaveBeenCalled()
    h.message('init', { mode: 'browse' })
    h.signup.click()
    expect(appClick).toHaveBeenCalledTimes(2)
    h.message('mode', { mode: 'select' })
    h.signup.click()
    expect(appClick).toHaveBeenCalledTimes(2)
    expect(h.postMessage.mock.calls.map(([event]) => event)).toEqual(expect.arrayContaining([
      expect.objectContaining({ type: 'selection', selection: expect.objectContaining({ selector: '#signup', tag_name: 'button', text: '开始创作', page_url: 'https://development.example/', rect: { x: 20, y: 40, width: 120, height: 40 } }) }),
    ]))
    h.message('dispose')
    h.signup.click()
    expect(appClick).toHaveBeenCalledTimes(3)
    expect(h.window.document.querySelector('[data-rambledesk-web-review-overlay]')).toBeNull()
  })

  it('ignores spoofed parents and wrong origin or channel commands and never collects password values', () => {
    const h = harness()
    h.message('init', { mode: 'select' }, h.window)
    expect(h.postMessage).not.toHaveBeenCalled()
    h.message('init', { mode: 'browse' })
    h.message('mode', { mode: 'select' }, h.parent, 'https://attacker.example')
    h.window.document.getElementById('password')!.click()
    expect(h.postMessage.mock.calls.some(([data]) => data.type === 'selection')).toBe(false)
    h.message('mode', { mode: 'select' })
    h.window.document.getElementById('password')!.click()
    const event = h.postMessage.mock.calls.find(([data]) => data.type === 'selection')![0]
    expect(event.selection.text).toBe('')
    expect(JSON.stringify(event)).not.toContain('secret-token')
    expect(h.postMessage.mock.calls.every(([, origin]) => origin === 'https://rambledesk.example')).toBe(true)
  })

  it('reports stale page or missing selector and tracks a SPA route in the selection context', () => {
    const h = harness()
    h.message('init', { mode: 'select' })
    h.message('focus', { page_url: 'https://development.example/old', selector: '#signup' })
    expect(h.postMessage).toHaveBeenCalledWith(expect.objectContaining({ type: 'focus_missing', reason: 'page_changed' }), 'https://rambledesk.example')
    h.message('focus', { page_url: 'https://development.example/', selector: '#removed' })
    expect(h.postMessage).toHaveBeenCalledWith(expect.objectContaining({ type: 'focus_missing', reason: 'element_missing' }), 'https://rambledesk.example')
    h.window.history.pushState({}, '', '/dialog')
    h.signup.click()
    expect(h.postMessage).toHaveBeenCalledWith(expect.objectContaining({ type: 'selection', selection: expect.objectContaining({ page_url: 'https://development.example/dialog' }) }), 'https://rambledesk.example')
  })

  it('truncates by Unicode scalar and normalizes SVG tag metadata without changing the actual selector', () => {
    const h = harness()
    const foreignObject = h.window.document.createElementNS('http://www.w3.org/2000/svg', 'foreignObject')
    foreignObject.id = 'svg-label'
    foreignObject.textContent = 'a'.repeat(1999) + '😀' + 'tail'
    h.window.document.body.append(foreignObject)
    foreignObject.getBoundingClientRect = h.signup.getBoundingClientRect.bind(h.signup)
    h.message('init', { mode: 'select' })
    foreignObject.dispatchEvent(new h.window.MouseEvent('click', { bubbles: true, composed: true }))
    const captured = h.postMessage.mock.calls.find(([data]) => data.type === 'selection')![0].selection
    expect(captured.tag_name).toBe('foreignobject')
    expect(captured.selector).toBe('#svg-label')
    expect(captured.text).toBe('a'.repeat(1999) + '😀')
    expect([...captured.text]).toHaveLength(2000)
  })
})
