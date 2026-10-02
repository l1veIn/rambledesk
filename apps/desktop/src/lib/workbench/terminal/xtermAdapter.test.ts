// @vitest-environment jsdom
import type { Terminal, ITerminalOptions } from '@xterm/xterm'
import { beforeEach, expect, it, vi } from 'vitest'
import { createXtermAdapter } from './xtermAdapter'
import { terminalAppearanceFor } from './terminalAppearance'

const mock = vi.hoisted(() => ({ terminal: null as unknown as Terminal & { output: string; selected: string; keyHandler: () => boolean }, created: vi.fn() }))
vi.mock('@xterm/xterm', () => ({ Terminal: class {
  cols = 80; rows = 24; output = ''; selected = 'selected output'; keyHandler = () => false
  options: ITerminalOptions
  constructor(options: ITerminalOptions) { this.options = options; mock.created(); mock.terminal = this as unknown as typeof mock.terminal }
  loadAddon() {} open() {} focus() {}
  attachCustomKeyEventHandler(handler: () => boolean) { this.keyHandler = handler }
  onData() { return { dispose() {} } } onResize() { return { dispose() {} } } onSelectionChange() { return { dispose() {} } }
  getSelection() { return this.selected }
  reset = vi.fn(() => { this.output = ''; this.selected = '' })
  resize = vi.fn()
  write = vi.fn((output: string, done: () => void) => { this.output += output; done() })
  dispose = vi.fn()
} }))
vi.mock('@xterm/addon-fit', () => ({ FitAddon: class { proposeDimensions() { return { cols: 80, rows: 24 } } } }))
vi.mock('./terminalScreen', () => ({ terminalScreen: (terminal: typeof mock.terminal) => terminal.output }))
beforeEach(() => vi.clearAllMocks())

it('changes xterm colors in place without resetting output, selection, dimensions or input admission', async () => {
  const adapter = await createXtermAdapter(document.createElement('div'), { onData() {}, onResize() {}, onSelection() {} }, terminalAppearanceFor('amber').theme)
  const instance = mock.terminal
  expect(instance.options.theme).toEqual(terminalAppearanceFor('amber').theme)
  await adapter.write('CLI output\n$ ')
  adapter.setInteractive(true)
  const size = adapter.size()
  adapter.setAppearance(terminalAppearanceFor('paper').theme)
  expect(instance.options.theme).toEqual(terminalAppearanceFor('paper').theme)
  expect(mock.terminal).toBe(instance)
  expect(mock.created).toHaveBeenCalledTimes(1)
  expect(instance.reset).not.toHaveBeenCalled()
  expect(instance.resize).not.toHaveBeenCalled()
  expect(instance.dispose).not.toHaveBeenCalled()
  expect(adapter.screen()).toBe('CLI output\n$ ')
  expect(adapter.selection()).toBe('selected output')
  expect(adapter.size()).toEqual(size)
  expect(instance.keyHandler()).toBe(true)
  await adapter.write('continued output')
  expect(adapter.screen()).toBe('CLI output\n$ continued output')
  adapter.dispose()
})
it('ignores appearance updates after the view is disposed', async () => {
  const adapter = await createXtermAdapter(document.createElement('div'), { onData() {}, onResize() {}, onSelection() {} })
  const original = mock.terminal.options.theme
  adapter.dispose()
  adapter.setAppearance(terminalAppearanceFor('forest').theme)
  expect(mock.terminal.options.theme).toBe(original)
  expect(mock.terminal.dispose).toHaveBeenCalledTimes(1)
})
