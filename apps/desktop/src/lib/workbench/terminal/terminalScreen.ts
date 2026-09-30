import type { Terminal } from '@xterm/xterm'

export function terminalScreen(terminal: Terminal): string {
  const buffer = terminal.buffer.active
  const lines: string[] = []
  for (let row = buffer.baseY; row < buffer.baseY + terminal.rows; row++) {
    lines.push(buffer.getLine(row)?.translateToString(true) ?? '')
  }
  return lines.join('\n').trimEnd()
}
