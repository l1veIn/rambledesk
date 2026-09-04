import { createInterface } from 'node:readline'
import { spawn } from 'node:child_process'
import { isAbsolute } from 'node:path'
const send = value => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', ...value }) + '\n')
const reply = (id, result) => send({ id, result })
const http = process.argv[2] === 'http'
const remote = 'owned-stdio-original'
let companion, requestId, pending = new Map(), next = 1
const text = value => send({ method: 'session/update', params: { sessionId: remote, update: {
  sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: value }
} } })
function rpc(method, params) {
  const id = next++
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject })
    companion.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n')
  })
}
async function open(servers) {
  if (servers.length !== 1) throw new Error('Expected one private feedback server')
  const server = servers[0]
  if (http) {
    if (server.type !== 'http' || !server.headers?.some(header => header.name === 'Authorization')) throw new Error('HTTP was not preferred')
    return
  }
  if (!isAbsolute(server.command) || JSON.stringify(server.args) !== '["managed-mcp-stdio"]') throw new Error('Invalid companion launch')
  const env = Object.fromEntries(server.env.map(entry => [entry.name, entry.value]))
  if (Object.keys(env).sort().join(',') !== 'RAMBLEDESK_MANAGED_MCP_TOKEN,RAMBLEDESK_MANAGED_MCP_URL') throw new Error('Unexpected capability environment')
  if (!/^[0-9a-f]{64}$/.test(env.RAMBLEDESK_MANAGED_MCP_TOKEN)) throw new Error('Invalid scoped token')
  companion = spawn(server.command, server.args, { env: { ...process.env, ...env }, stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true })
  companion.stderr.resume()
  createInterface({ input: companion.stdout }).on('line', line => {
    const response = JSON.parse(line), waiter = pending.get(response.id)
    if (!waiter) return
    pending.delete(response.id)
    if (response.error) waiter.reject(new Error('Companion operation failed')); else waiter.resolve(response.result)
  })
  companion.on('exit', () => { for (const waiter of pending.values()) waiter.reject(new Error('Companion closed')); pending.clear() })
  await rpc('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'acp-owned-fixture', version: '1' } })
  companion.stdin.write(JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' }) + '\n')
  const tools = await rpc('tools/list', {})
  if (tools.tools.map(tool => tool.name).sort().join(',') !== 'get_feedback,recover_feedback,request_feedback') throw new Error('Wrong scoped tools')
}
async function handle({ id, method, params }) {
  switch (method) {
    case 'initialize':
      for (const key of ['RAMBLEDESK_MANAGED_MCP_URL', 'RAMBLEDESK_MANAGED_MCP_TOKEN', 'RAMBLEDESK_MANAGED_PI_WRAPPER']) {
        if (process.env[key]) throw new Error('Private capability reached ACP root process')
      }
      reply(id, { protocolVersion: 1, agentCapabilities: { loadSession: true, mcpCapabilities: { http }, sessionCapabilities: { close: {} } } })
      break
    case 'session/new': await open(params.mcpServers); reply(id, { sessionId: remote }); break
    case 'session/load':
      if (params.sessionId !== remote) throw new Error('Original context lost')
      await open(params.mcpServers); reply(id, {}); break
    case 'session/prompt': {
      const prompt = params.prompt[0].text
      if (prompt.startsWith('request:')) {
        requestId = prompt.slice('request:'.length)
        const result = await rpc('tools/call', { name: 'request_feedback', arguments: { request_id: requestId,
          what_happened: 'Review owned stdio chain', actions: [{ id: 'review', instruction: 'Review' }], host_id: 'spoofed', host_session_id: 'other' } })
        if (result.isError) throw new Error('Feedback request failed')
        text(`REQUEST ${result.structuredContent.request_id}`)
      } else {
        const target = prompt.startsWith('get:') ? prompt.slice(4) : (requestId ?? prompt.match(/[0-9a-f]{8}-[0-9a-f-]{27,}/i)?.[0])
        const result = await rpc('tools/call', { name: 'get_feedback', arguments: { request_id: target } })
        text(`RESULT ${result.structuredContent?.code ?? result.structuredContent?.resolution ?? 'unknown'}`)
      }
      reply(id, { stopReason: 'end_turn' }); break
    }
    case 'session/cancel': break
    case 'session/close':
      if (companion) { companion.stdin.end(); await new Promise(resolve => companion.exitCode !== null ? resolve() : companion.once('exit', resolve)) }
      reply(id, {}); break
  }
}
createInterface({ input: process.stdin }).on('line', line => {
  handle(JSON.parse(line)).catch(() => { process.stderr.write('ACP stdio fixture failed\n'); process.exit(1) })
}).on('close', () => process.exit(0))
