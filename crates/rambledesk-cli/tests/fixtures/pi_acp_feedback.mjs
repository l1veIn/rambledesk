// Deterministic pi-acp protocol fixture, launching the production wrapper and
// production managed extension. The native RPC test peer never calls a model.
import { createInterface } from 'node:readline'
import { spawn } from 'node:child_process'
import { appendFileSync } from 'node:fs'
import { join } from 'node:path'
const send = value => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', ...value }) + '\n')
const reply = (id, result) => send({ id, result })
const remote = 'original-pi-context'
const resume = process.env.FIXTURE_RESUME === '1'
const http = process.env.FIXTURE_HTTP === '1'
let native, requestId, next = 1, pending = new Map()
const log = value => { if (process.env.FIXTURE_PI_LOG) appendFileSync(process.env.FIXTURE_PI_LOG, JSON.stringify(value) + '\n') }
const output = text => send({ method: 'session/update', params: { sessionId: remote, update: {
  sessionUpdate: 'agent_message_chunk', content: { type: 'text', text }
} } })
function rpc(method, params) {
  const id = next++
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject })
    native.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n')
  })
}
async function open(params, method) {
  if (method !== 'session/new' && params.sessionId !== remote) throw new Error('Original session lost')
  if (http) {
    if (process.env.RAMBLEDESK_MANAGED_PI_WRAPPER || params.mcpServers[0]?.type !== 'http') throw new Error('HTTP preference lost')
    log({ method, http: true }); return
  }
  if ((params.mcpServers ?? []).length || process.env.RAMBLEDESK_MANAGED_PI_WRAPPER !== '1') throw new Error('Missing Pi extension launch')
  native = spawn(process.env.PI_ACP_PI_COMMAND, ['--mode', 'rpc', '--no-themes', '--session', 'fixture-session.json'], {
    env: { ...process.env, FIXTURE_HEARTBEAT: join(process.cwd(), 'heartbeat') },
    stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true
  })
  native.stderr.resume()
  createInterface({ input: native.stdout }).on('line', line => {
    const message = JSON.parse(line), waiter = pending.get(message.id)
    if (!waiter) return
    pending.delete(message.id)
    if (message.error) waiter.reject(new Error('Native RPC failed')); else waiter.resolve(message.result)
  })
  native.on('exit', () => { for (const waiter of pending.values()) waiter.reject(new Error('Native closed')); pending.clear() })
  await rpc('initialize', {})
  const tools = await rpc('tools/list', {})
  if (tools.tools.map(tool => tool.name).sort().join(',') !== 'get_feedback,recover_feedback,request_feedback') throw new Error('Wrong managed toolset')
  log({ method, remote, nativePid: native.pid })
}
async function handle({ id, method, params }) {
  if (method === 'initialize') {
    log({ method, pid: process.pid, wrapped: process.env.RAMBLEDESK_MANAGED_PI_WRAPPER === '1' })
    return reply(id, { protocolVersion: 1, agentCapabilities: { loadSession: !resume,
      mcpCapabilities: { http }, sessionCapabilities: { close: {}, ...(resume ? { resume: {} } : {}) } } })
  }
  if (['session/new', 'session/load', 'session/resume'].includes(method)) {
    await open(params, method)
    if (process.env.FIXTURE_BLOCK_OPEN === '1') return
    return reply(id, method === 'session/new' ? { sessionId: remote } : {})
  }
  if (method === 'session/prompt') {
    const prompt = params.prompt[0].text
    if (prompt.startsWith('request:')) {
      requestId = prompt.slice(8)
      const result = await rpc('tools/call', { name: 'request_feedback', arguments: { request_id: requestId,
        what_happened: 'Pi managed review', actions: [{ id: 'review', instruction: 'Review' }], host_id: 'spoofed', host_session_id: 'other' } })
      if (result.isError) throw new Error('Managed request failed')
      output(`REQUEST ${result.structuredContent.request_id}`)
    } else {
      const target = prompt.startsWith('get:') ? prompt.slice(4) : (requestId ?? prompt.match(/[0-9a-f]{8}-[0-9a-f-]{27,}/i)?.[0])
      const result = await rpc('tools/call', { name: 'get_feedback', arguments: { request_id: target } })
      output(`RESULT ${result.structuredContent?.code ?? result.structuredContent?.resolution ?? 'unknown'}`)
    }
    return reply(id, { stopReason: 'end_turn' })
  }
  if (method === 'session/close') {
    if (native) { native.stdin.end(); await new Promise(resolve => native.exitCode !== null ? resolve() : native.once('exit', resolve)) }
    log({ method, remote }); return reply(id, {})
  }
}
createInterface({ input: process.stdin }).on('line', line => {
  handle(JSON.parse(line)).catch(() => { process.stderr.write('Pi ACP fixture failed\n'); process.exit(1) })
}).on('close', () => process.exit(0))
