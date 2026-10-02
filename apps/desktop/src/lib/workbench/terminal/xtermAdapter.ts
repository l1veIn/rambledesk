import type { TerminalRenderer } from './terminalController'
import { terminalScreen } from './terminalScreen'
import { TERMINAL_APPEARANCES } from './terminalAppearance'
import type { ITheme } from '@xterm/xterm'

export type TerminalAdapter = TerminalRenderer & {
  size: () => { cols: number; rows: number }
  fit: () => void
  focus: () => void
  selection: () => string
  setInteractive: (value: boolean) => void
  setAppearance: (theme: ITheme) => void
  dispose: () => void
}

/** Load xterm only in a mounted browser surface. Its DOM stays within the workbench. */
export async function createXtermAdapter(root: HTMLElement, hooks: {
  onData: (data: string) => void
  onResize: (size: { cols: number; rows: number }) => void
  onSelection: (text: string) => void
}, theme: ITheme = TERMINAL_APPEARANCES[0].theme): Promise<TerminalAdapter> {
  const [{ Terminal }, { FitAddon }] = await Promise.all([import('@xterm/xterm'), import('@xterm/addon-fit')])
  const terminal = new Terminal({
    cursorBlink: true, fontSize: 13, lineHeight: 1.2,
    fontFamily: '"Geist Mono Variable", "Fira Code Variable", Consolas, monospace',
    scrollback: 3000, screenReaderMode: true, minimumContrastRatio: 4.5,
    theme: { ...theme },
  })
  const fit = new FitAddon()
  terminal.loadAddon(fit)
  terminal.open(root)
  // disableStdin also suppresses automatic cursor/device replies needed by ConPTY.
  // Gate user keys separately while the controller admits protocol replies at startup.
  let interactive = false
  terminal.attachCustomKeyEventHandler(() => interactive)
  const subscriptions = [terminal.onData(hooks.onData), terminal.onResize(hooks.onResize),
    terminal.onSelectionChange(() => hooks.onSelection(terminal.getSelection()))]
  let disposed = false
  return {
    size: () => ({ cols: terminal.cols, rows: terminal.rows }),
    fit: () => {
      if (disposed || root.clientWidth < 20 || root.clientHeight < 20) return
      const size = fit.proposeDimensions()
      if (size) terminal.resize(Math.max(20, Math.min(300, size.cols)), Math.max(5, Math.min(100, size.rows)))
    },
    reset: () => terminal.reset(),
    write: (output) => new Promise<void>((resolve) => { if (disposed) resolve(); else terminal.write(output, resolve) }),
    screen: () => terminalScreen(terminal), focus: () => terminal.focus(),
    selection: () => terminal.getSelection(), setInteractive: (value) => { interactive = value; terminal.options.cursorBlink = value },
    setAppearance: (next) => { if (!disposed) terminal.options.theme = { ...next } },
    dispose: () => { disposed = true; subscriptions.forEach((subscription) => subscription.dispose()); terminal.dispose() },
  }
}
