import type { TerminalSessionSnapshot } from '../generated/feedback'

/** UI fixture only: the simulated CLI never starts a host process. */
export class TerminalPreviewRuntime {
  private session: TerminalSessionSnapshot | null = null
  private sessions = new Map<string, TerminalSessionSnapshot>()
  private line = ''
  private prompt: 'shell' | 'name' | 'choose' = 'shell'
  private choice = 0

  call(name: string, input: { request_id: string; session_id?: string; cols?: number; rows?: number; data?: string; after_sequence?: number | null }): TerminalSessionSnapshot {
    if (name === 'openTerminalSession') {
      const sessions = [...this.sessions.values()].filter((item) => item.request_id === input.request_id)
      this.session = sessions.find((item) => item.status === 'running') ?? null
      if (!this.session) {
        if (sessions.length >= 16) throw new Error('这个请求已记录 16 轮终端试用。')
        this.line = ''; this.prompt = 'shell'; this.choice = 0
        this.session = { session_id: `terminal-preview-${input.request_id}-${sessions.length + 1}`, request_id: input.request_id,
        cwd: '/preview/cli-demo', shell: 'preview', cols: input.cols ?? 80, rows: input.rows ?? 24,
        status: 'running', exit_code: null, output: '终端交互预览 · 模拟 CLI，不执行本机命令。\r\n$ ',
          first_sequence: 0, next_sequence: 0, truncated: false }
        this.sessions.set(this.session.session_id, this.session)
      }
    }
    if (input.session_id) this.session = this.sessions.get(input.session_id) ?? null
    const session = this.session
    if (!session || session.request_id !== input.request_id || (input.session_id && session.session_id !== input.session_id)) throw new Error('预览终端会话不存在。')
    if (name === 'writeTerminalSession') {
      if (session.status !== 'running') throw new Error('终端已结束。')
      this.write(input.data ?? '')
    }
    if (name === 'resizeTerminalSession') { session.cols = input.cols ?? session.cols; session.rows = input.rows ?? session.rows }
    if (name === 'stopTerminalSession' && session.status === 'running') session.status = 'stopped'
    session.next_sequence = new TextEncoder().encode(session.output).length
    const snapshot = { ...session }
    if (name === 'readTerminalSession' && input.after_sequence != null) {
      snapshot.output = new TextDecoder().decode(new TextEncoder().encode(snapshot.output).slice(input.after_sequence))
      snapshot.first_sequence = input.after_sequence
    }
    return snapshot
  }

  private write(data: string) {
    const session = this.session!
    if (data === '\u001b[A' || data === '\u001b[B') {
      if (this.prompt === 'choose') {
        this.choice = this.choice === 0 ? 1 : 0
        session.output += `\r\u001b[2K${this.choice ? '  紧凑    > 宽松' : '> 紧凑      宽松'}`
      }
      return
    }
    for (const char of data) {
      if (char === '\u0003') { this.line = ''; this.prompt = 'shell'; session.output += '^C\r\n$ '; continue }
      if (char === '\u007f' || char === '\b') { this.line = this.line.slice(0, -1); session.output += '\b \b'; continue }
      if (char !== '\r' && char !== '\n') { this.line += char; session.output += char; continue }
      const line = this.line.trim(); this.line = ''; session.output += '\r\n'
      if (this.prompt === 'name') { session.output += `你好，${line || '朋友'}！\r\n`; this.prompt = 'shell' }
      else if (this.prompt === 'choose') { session.output += `你选择了${this.choice ? '宽松' : '紧凑'}布局。\r\n`; this.prompt = 'shell' }
      else if (line === 'demo greet') { this.prompt = 'name'; session.output += '你的名字：'; continue }
      else if (line === 'demo choose') { this.prompt = 'choose'; this.choice = 0; session.output += '> 紧凑      宽松'; continue }
      else if (line === 'demo --help') session.output += 'CLI Demo\r\n  demo greet   交互式问候\r\n  demo choose  方向键选择\r\n  demo error   错误提示\r\n'
      else if (line === 'demo error') session.output += '\u001b[31m错误：缺少配置。请先运行 demo --help。\u001b[0m\r\n'
      else if (line === 'exit') { session.status = 'exited'; session.exit_code = 0; return }
      else if (line) session.output += `模拟 CLI 不认识这个命令：${line}\r\n`
      session.output += '$ '
    }
  }
}
