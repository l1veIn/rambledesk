import { afterEach, beforeEach, expect, test, vi } from 'vitest'
import { mount, unmount, flushSync, tick } from 'svelte'
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'
import WorkbenchDemos from '../src/components/WorkbenchDemos.svelte'
import { locales } from '../src/content/site'

let target, component, play, pause
const tabs = () => [...target.querySelectorAll('[role="tab"]')]
const control = () => target.querySelector('.play-control')
async function settle() { await tick(); await Promise.resolve(); flushSync() }
async function click(node) { node.click(); await settle() }
function render(lang = 'en') {
  component = mount(WorkbenchDemos, { target, props: { lang, content: locales[lang === 'en' ? 'en' : 'zh'].workbenches } })
  flushSync()
}

beforeEach(() => {
  target = document.createElement('div')
  document.body.append(target)
  play = vi.spyOn(HTMLMediaElement.prototype, 'play').mockImplementation(function () {
    this.dispatchEvent(new Event('play'))
    return Promise.resolve()
  })
  pause = vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(function () { this.dispatchEvent(new Event('pause')) })
})
afterEach(async () => { if (component) await unmount(component); component = null; target.remove(); vi.restoreAllMocks() })

test.each(['en', 'zh-CN'])('starts with one real %s poster and no video source to preload', (lang) => {
  render(lang)
  expect(tabs()).toHaveLength(4)
  expect(tabs().map(tab => tab.tabIndex)).toEqual([0, -1, -1, -1])
  expect(target.querySelectorAll('img')).toHaveLength(1)
  const image = target.querySelector('img')
  expect(image.getAttribute('src')).toBe(`/assets/workbenches/${lang}/document-review.webp`)
  expect(image.getAttribute('alt')).toBe(locales[lang === 'en' ? 'en' : 'zh'].workbenches.items[0].alt)
  expect(target.querySelector('video, source')).toBeNull()
  expect(play).not.toHaveBeenCalled()
  expect(image.getAttribute('loading')).toBe('lazy')
  expect(target.querySelectorAll('[role="tabpanel"]')).toHaveLength(4)
  for (const tab of tabs()) {
    const related = document.getElementById(tab.getAttribute('aria-controls'))
    expect(related).not.toBeNull()
    expect(related.hidden).toBe(tab.getAttribute('aria-selected') !== 'true')
  }
  const panel = target.querySelector('[role="tabpanel"]')
  expect(panel.id).toBe(tabs()[0].getAttribute('aria-controls'))
  expect(panel.getAttribute('aria-labelledby')).toBe(tabs()[0].id)
})

test('explicit play loads only the active clip, with pause, native controls and no loop', async () => {
  render()
  await click(control())
  const media = target.querySelector('video')
  expect(media.getAttribute('src')).toBe('/assets/workbenches/en/document-review.mp4')
  expect(media.preload).toBe('none')
  expect(media.controls).toBe(true)
  expect(media.autoplay).toBe(false)
  expect(media.loop).toBe(false)
  expect(play).toHaveBeenCalledTimes(1)
  expect(control().textContent).toContain(locales.en.workbenches.pause)
  await click(control())
  expect(pause.mock.contexts).toContain(media)
  expect(control().textContent).toContain(locales.en.workbenches.play)
  expect(target.querySelector('video')).toBe(media)
  await click(control())
  media.dispatchEvent(new Event('ended'))
  flushSync()
  expect(control().textContent).toContain(locales.en.workbenches.play)
  expect(play).toHaveBeenCalledTimes(2)
})

test('switching tabs stops and removes the playing clip, including when returning to it', async () => {
  render()
  await click(control())
  const first = target.querySelector('video')
  await click(tabs()[2])
  expect(pause.mock.contexts).toContain(first)
  expect(first.isConnected).toBe(false)
  expect(target.querySelector('video')).toBeNull()
  expect(target.querySelector('img').getAttribute('src')).toBe('/assets/workbenches/en/brainstorm.webp')
  expect(play).toHaveBeenCalledTimes(1)
  await click(tabs()[0])
  expect(target.querySelector('video')).toBeNull()
  expect(play).toHaveBeenCalledTimes(1)
})

test('a pending old play rejection cannot replace the newly selected recording with an error', async () => {
  let rejectOld
  play.mockImplementationOnce(() => new Promise((_, reject) => { rejectOld = reject }))
  render()
  await click(control())
  const first = target.querySelector('video')
  await click(tabs()[1])
  await click(control())
  const current = target.querySelector('video')
  rejectOld(new Error('aborted after changing tabs'))
  await settle()
  expect(pause.mock.contexts).toContain(first)
  expect(target.querySelector('[role="alert"]')).toBeNull()
  expect(target.querySelector('video')).toBe(current)
  expect(current.getAttribute('src')).toBe('/assets/workbenches/en/visual-feedback.mp4')
  expect(control().textContent).toContain(locales.en.workbenches.pause)
})

test('explicit pause can cancel an unresolved play request without showing a failure', async () => {
  let finish
  play.mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
  render()
  await click(control())
  const media = target.querySelector('video')
  expect(control().textContent).toContain(locales.en.workbenches.pause)
  await click(control())
  finish()
  await settle()
  expect(pause.mock.contexts).toContain(media)
  expect(control().textContent).toContain(locales.en.workbenches.play)
  expect(target.querySelector('[role="alert"]')).toBeNull()
})

test.each(['play rejection', 'media error'])('%s falls back to the current poster and a real local MP4 link', async (failure) => {
  render('zh-CN')
  await click(tabs()[3])
  if (failure === 'play rejection') play.mockRejectedValueOnce(new Error('unsupported'))
  await click(control())
  if (failure === 'media error') { target.querySelector('video').dispatchEvent(new Event('error')); await settle() }
  await vi.waitFor(() => expect(target.querySelector('video')).toBeNull())
  expect(target.querySelector('img').getAttribute('src')).toBe('/assets/workbenches/zh-CN/free-feedback.webp')
  const error = target.querySelector('[role="alert"]')
  expect(error.textContent).toContain(locales.zh.workbenches.errorLabel)
  expect(error.querySelector('a').getAttribute('href')).toBe('/assets/workbenches/zh-CN/free-feedback.mp4')
  expect(error.querySelector('a').textContent).toContain(locales.zh.workbenches.openVideo)
})

test('left/right wrap and Home/End activate and focus tabs without starting playback', () => {
  render()
  function key(index, value, expected) {
    tabs()[index].focus()
    const event = new KeyboardEvent('keydown', { key: value, bubbles: true, cancelable: true })
    tabs()[index].dispatchEvent(event)
    flushSync()
    expect(event.defaultPrevented).toBe(true)
    expect(document.activeElement).toBe(tabs()[expected])
    expect(tabs().map(tab => tab.getAttribute('aria-selected'))).toEqual(tabs().map((_, i) => i === expected ? 'true' : 'false'))
    expect(tabs().filter(tab => tab.tabIndex === 0)).toEqual([tabs()[expected]])
    expect(target.querySelector('[role="tabpanel"]').id).toBe(tabs()[expected].getAttribute('aria-controls'))
  }
  key(0, 'ArrowLeft', 3)
  key(3, 'ArrowRight', 0)
  key(0, 'End', 3)
  key(3, 'Home', 0)
  expect(play).not.toHaveBeenCalled()
  expect(target.querySelector('video')).toBeNull()
})

test('teardown pauses the current media', async () => {
  render()
  await click(control())
  const media = target.querySelector('video')
  await unmount(component)
  component = null
  expect(pause.mock.contexts).toContain(media)
})

test('server rendering includes the real initial title and poster before hydration', () => {
  const directory = dirname(fileURLToPath(import.meta.url))
  const filename = resolve(directory, '../src/components/WorkbenchDemos.svelte')
  const packageFile = resolve(directory, '../package.json')
  const script = `
    import fs from 'node:fs';
    import { createRequire } from 'node:module';
    import { pathToFileURL } from 'node:url';
    const require = createRequire(${JSON.stringify(packageFile)});
    const { compile } = require('svelte/compiler');
    let code = compile(fs.readFileSync(${JSON.stringify(filename)}, 'utf8'), { generate: 'server', filename: ${JSON.stringify(filename)} }).js.code;
    code = code.replace(/from (['"])(svelte(?:\\/[^'"]*)?)\\1/g, (_, quote, name) => 'from ' + JSON.stringify(pathToFileURL(require.resolve(name)).href));
    const { default: Component } = await import('data:text/javascript;base64,' + Buffer.from(code).toString('base64'));
    const { render } = await import(pathToFileURL(require.resolve('svelte/server')).href);
    process.stdout.write(render(Component, { props: ${JSON.stringify({ lang: 'en', content: locales.en.workbenches })} }).body);
  `
  const html = execFileSync(process.execPath, ['--input-type=module', '-'], { input: script, encoding: 'utf8', timeout: 10000 })
  const output = document.createElement('div')
  output.innerHTML = html
  expect(output.querySelector('h3').textContent).toBe(locales.en.workbenches.items[0].title)
  expect(output.querySelector('img').getAttribute('src')).toBe('/assets/workbenches/en/document-review.webp')
  expect(output.querySelector('img').getAttribute('alt')).toBe(locales.en.workbenches.items[0].alt)
  expect(output.querySelector('video, source')).toBeNull()
})
