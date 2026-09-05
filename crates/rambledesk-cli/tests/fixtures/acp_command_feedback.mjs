import { createInterface } from 'node:readline'
import { spawn } from 'node:child_process'
import { isAbsolute } from 'node:path'
const send = value => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', ...value }) + '\n')
const reply = (id, result) => send({ id, result })
const http = process.argv[2] === 'http'
const remote = 'owned-command-original'
const text = value => send({ method: 'session/update', params: { sessionId: remote, update: {
  sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: value }
} } })
function command(operation, payload) {
  return new Promise((resolve, reject) => {
    const args = ['feedback', operation, ...(operation === 'request' ? ['--input', '-'] : ['--request-id', payload.request_id])]
    const child = spawn(process.env.RAMBLEDESK_COMMAND, args, { env: process.env, stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true })
    let output = ''
    child.stdout.on('data', chunk => { output += chunk })
    child.stderr.resume()
    child.on('error', reject)
    child.on('exit', () => { try { resolve(JSON.parse(output)) } catch { reject(new Error('Invalid command result')) } })
    child.stdin.end(operation === 'request' ? JSON.stringify(payload) : undefined)
  })
}
function open(servers) {
  if (servers.length !== 0) throw new Error('Managed workflow must not inject MCP servers')
  if (!isAbsolute(process.env.RAMBLEDESK_COMMAND ?? '')) throw new Error('Missing application command')
  if (process.env.RAMBLEDESK_MANAGED_SESSION !== '1') throw new Error('Missing common workflow marker')
  if (!/^[0-9a-f]{64}$/.test(process.env.RAMBLEDESK_FEEDBACK_TOKEN)) throw new Error('Invalid scoped token')
  if (!process.env.RAMBLEDESK_FEEDBACK_URL.endsWith('/agent-feedback')) throw new Error('Wrong local API')
}
async function handle({ id, method, params }) {
  switch (method) {
    case 'initialize':
      for (const key of ['RAMBLEDESK_MANAGED_MCP_URL', 'RAMBLEDESK_MANAGED_MCP_TOKEN', 'RAMBLEDESK_MANAGED_PI_WRAPPER']) {
        if (process.env[key]) throw new Error('Legacy private capability reached ACP root process')
      }
      reply(id, { protocolVersion: 1, agentCapabilities: { loadSession: true, mcpCapabilities: { http }, sessionCapabilities: { close: {} } } })
      break
    case 'session/new': open(params.mcpServers); reply(id, { sessionId: remote }); break
    case 'session/load':
      if (params.sessionId !== remote) throw new Error('Original context lost')
      open(params.mcpServers); reply(id, {}); break
    case 'session/prompt': {
      if (!params.prompt[0].text.includes('<rambledesk_session_context>') || !params.prompt[0].text.includes('RAMBLEDESK_COMMAND')) throw new Error('Missing built-in workflow')
      if (params.prompt[0].text.includes(process.env.RAMBLEDESK_FEEDBACK_TOKEN)) throw new Error('Secret in workflow prompt')
      const prompt = params.prompt.slice(1).map(block => block.text ?? '').join('\n')
      if (prompt.startsWith('request:')) {
        const requestId = prompt.slice('request:'.length)
        const result = await command('request', { request_id: requestId,
          what_happened: 'Review common command chain', actions: [{ id: 'review', instruction: 'Review' }], host_id: 'spoofed', host_session_id: 'other' })
        if (result.code) throw new Error('Feedback request failed')
        text(`REQUEST ${result.request_id}`)
      } else {
        const target = prompt.startsWith('get:') ? prompt.slice(4) : prompt.match(/[0-9a-f]{8}-[0-9a-f-]{27,}/i)?.[0]
        const result = await command('get', { request_id: target })
        text(`RESULT ${result.code ?? result.resolution ?? 'unknown'}`)
      }
      reply(id, { stopReason: 'end_turn' }); break
    }
    case 'session/cancel': break
    case 'session/close': reply(id, {}); break
  }
}
createInterface({ input: process.stdin }).on('line', line => {
  handle(JSON.parse(line)).catch(() => { process.stderr.write('ACP command fixture failed\n'); process.exit(1) })
}).on('close', () => process.exit(0))
