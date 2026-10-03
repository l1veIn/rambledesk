// Mount the real website component with the repository's existing test runtime.
import { beforeEach, afterEach, test, expect, vi } from 'vitest'
import { mount, unmount, flushSync } from 'svelte'
import ProductMock from '../src/components/ProductMock.svelte'

let target, component
const settings = 'Settings · narrow layout · Claude Code'
const review = 'Review the uncommitted changes · Claude Code'
const words = () => target.textContent.replace(/\s+/g, ' ')
function button(name) {
  const result = [...target.querySelectorAll('button')].find((item) =>
    item.getAttribute('aria-label') === name || item.textContent.replace(/\s+/g, ' ').trim() === name)
  expect(result, `button ${name}`).toBeTruthy()
  return result
}
function click(name) { const node = button(name); expect(node.disabled).toBe(false); node.click(); flushSync() }
function advance(ms) { vi.advanceTimersByTime(ms); flushSync() }
function speech() { click('Start recording'); click('Stop recording') }

beforeEach(() => {
  vi.useFakeTimers()
  vi.stubGlobal('matchMedia', () => ({ matches: false }))
  target = document.createElement('div')
  document.body.append(target)
  component = mount(ProductMock, { target, props: { lang: 'en' } })
  flushSync()
})
afterEach(async () => { if (component) await unmount(component); target.remove(); vi.useRealTimers(); vi.unstubAllGlobals() })

test('capture autosave cannot cancel publishing or delivery', () => {
  click('Submit feedback')
  advance(100)
  click('Capture')
  expect(button('Publishing…').disabled).toBe(true)
  advance(1000)
  expect(words()).toContain('Waiting for the agent')
  advance(1600)
  expect(words()).toContain('Feedback delivered')
  expect(words()).toContain('Saved · r2')
  expect(button('Delete · settings-sidebar.png').disabled).toBe(true)
})

test('navigation and edits in another request preserve the original publication owner', () => {
  click('Submit feedback')
  advance(100)
  click(review)
  expect(button('Submit feedback').disabled).toBe(false)
  click('Capture')
  advance(3000)
  expect(words()).toContain('No feedback delivery yet')
  expect(words()).not.toContain('Feedback delivered')
  expect(words()).toContain('Saved · r2')
  click(settings)
  expect(words()).toContain('Feedback delivered')
  expect(words()).toContain('Saved · r1')
})

test('navigation during queued delivery does not strand the publication', () => {
  click('Submit feedback')
  advance(1100)
  expect(words()).toContain('Waiting for the agent')
  click(review)
  advance(700)
  click(settings)
  expect(words()).toContain('Continuing the agent…')
  click(review)
  advance(900)
  click(settings)
  expect(words()).toContain('Feedback delivered')
})

test('tidying survives a concurrent autosave and writes only to the captured request', () => {
  speech()
  click('Tidy 1')
  advance(100)
  click('Files')
  click(review)
  advance(2000)
  expect(words()).not.toContain('Below that width')
  expect(words()).toContain('Saved · r1')
  click(settings)
  expect(words()).toContain('Below that width, the description text on the right wraps onto two lines.')
  expect(words()).not.toContain('Pending speech')
  expect(button('Tidy').disabled).toBe(true)
  expect(words()).toContain('Saved · r2')
})

test('a speech segment added during tidying stays pending for the next tidy', () => {
  speech()
  click('Tidy 1')
  advance(100)
  speech()
  advance(800)
  expect(words()).toContain('Below that width')
  expect(words()).toContain('Pending speech')
  expect(button('Tidy 1').disabled).toBe(false)
})

test('cancel prevents pending tidy and save callbacks from mutating the closed document', () => {
  speech()
  click('Tidy 1')
  advance(100)
  click('Cancel feedback')
  const cancelled = words()
  advance(5000)
  expect(words()).toBe(cancelled)
  expect(words()).toContain('Feedback is cancelled')
  expect(words()).toContain('One more thing: when the window gets narrower')
  expect(words()).not.toContain('Below that width')
})

test('restart cancels callbacks at publishing, pending and sending phases', () => {
  for (const elapsed of [100, 1100, 1800]) {
    click('Submit feedback')
    advance(elapsed)
    click('Restart the example')
    advance(5000)
    expect(button('Submit feedback').disabled).toBe(false)
    expect(words()).toContain('No feedback delivery yet')
    expect(words()).toContain('Saved · r1')
    expect(words()).not.toContain('Feedback delivered')
  }
})

test('the first capture immediately appears in attachments and removal is reactive', () => {
  click('Capture')
  expect(button('Delete · settings-sidebar.png').disabled).toBe(false)
  click('Delete · settings-sidebar.png')
  expect(words()).toContain('Captures and imported files are kept here.')
})

test('unmount cancels all owned timers', async () => {
  speech()
  click('Tidy 1')
  click('Submit feedback')
  click('Capture')
  expect(vi.getTimerCount()).toBeGreaterThan(0)
  await unmount(component)
  component = null
  expect(vi.getTimerCount()).toBe(0)
})

test('reduced-motion completes nested saving and delivery', () => {
  vi.stubGlobal('matchMedia', () => ({ matches: true }))
  click('Capture')
  click('Submit feedback')
  vi.runAllTimers()
  flushSync()
  expect(words()).toContain('Saved · r2')
  expect(words()).toContain('Feedback delivered')
})

test('an inbox-driven request change resets recording without canceling the original save', () => {
  click(review)
  click('Capture')
  click('Start recording')
  click('All requests')
  expect(button('Start recording').getAttribute('aria-pressed')).toBe('false')
  expect(words()).toContain('Saved · r1')
  advance(1000)
  click('All requests')
  expect(words()).toContain('Saved · r2')
})
