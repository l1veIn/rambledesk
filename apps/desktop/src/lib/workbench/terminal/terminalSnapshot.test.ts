// @vitest-environment jsdom
import { expect, it } from 'vitest'
import { replayTerminalSnapshot } from './terminalSnapshot'

it('replays a final TUI screen without a DOM terminal or process input', async () => {
  const output = 'old screen\x1b[2J\x1b[H\x1b[32mFinal menu\x1b[0m\r\n> Item two'
  const session = await replayTerminalSnapshot({
    request_id: 'request-1', session_id: 'trial-1', cwd: '/project', shell: 'bash', cols: 80, rows: 24,
    status: 'stopped', exit_code: null, output, first_sequence: 0, next_sequence: output.length, truncated: false,
  })
  expect(session.output).toBe(output)
  expect(session.screen).toBe('Final menu\n> Item two')
  expect(document.querySelector('.xterm')).toBeNull()
  expect(session).not.toHaveProperty('input')
})
