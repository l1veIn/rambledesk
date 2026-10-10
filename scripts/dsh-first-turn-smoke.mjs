// Offline first-use ACP rehearsal with a real, already installed deepseek-acp.
// Usage: node scripts/dsh-first-turn-smoke.mjs --entry <absolute lib/bin.js>
//        --output <new absolute directory> [--node <absolute node executable>]
// Or replace --entry with --catalog-report <catalog.json from isolated smoke>.
// Does not install packages or read an existing HOME. The second scenario uses
// a synthetic key and a loopback model stub; it is not evidence of model access.
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { createServer } from 'node:http'
import { mkdir, readFile, realpath, stat, writeFile } from 'node:fs/promises'
import { delimiter, dirname, isAbsolute, join, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

async function options() {
  const values = new Map()
  const args = process.argv.slice(2)
  for (let index = 0; index < args.length; index += 2) {
    if (!['--entry', '--output', '--node', '--catalog-report'].includes(args[index]) || !args[index + 1] || values.has(args[index])) {
      throw new Error('Usage: node scripts/dsh-first-turn-smoke.mjs (--entry <absolute lib/bin.js> | --catalog-report <catalog.json>) --output <new absolute directory> [--node <absolute node executable>]')
    }
    values.set(args[index], args[index + 1])
  }
  const catalogReport = values.get('--catalog-report')
  assert(Boolean(values.has('--entry')) !== Boolean(catalogReport), 'Choose exactly one of --entry and --catalog-report')
  const catalog = catalogReport ? JSON.parse(await readFile(resolve(catalogReport), 'utf8')) : []
  assert(Array.isArray(catalog), 'Catalog report must contain the catalog result array')
  const component = catalogReport ? catalog.find(item => item.agent_id === 'deepseek-acp') : undefined
  const entry = values.get('--entry') ?? component?.entry_path
  const output = values.get('--output')
  const node = values.get('--node') ?? component?.node_path ?? process.execPath
  assert(entry && output && [entry, output, node].every(isAbsolute), 'Entry, output, and Node paths must be absolute')
  return { entry, output, node }
}

function environment(node, root, home, origin, syntheticKey) {
  const env = {}
  // OS plumbing only. No inherited PATH, account, proxy, provider, or DSH values.
  for (const name of ['SystemRoot', 'WINDIR', 'COMSPEC']) if (process.env[name]) env[name] = process.env[name]
  const path = process.platform === 'win32'
    ? [dirname(node), join(env.SystemRoot, 'System32'), join(env.SystemRoot, 'System32/WindowsPowerShell/v1.0')]
    : [dirname(node), '/usr/bin', '/bin']
  Object.assign(env, {
    PATH: path.join(delimiter), TEMP: root, TMP: root, TMPDIR: root,
    HOME: home, USERPROFILE: home, APPDATA: join(home, 'roaming'), LOCALAPPDATA: join(home, 'local'),
    XDG_CONFIG_HOME: join(home, 'config'), XDG_DATA_HOME: join(home, 'data'), XDG_CACHE_HOME: join(home, 'cache'),
    DSH_HOME: join(home, '.dsh'), DEEPSEEK_BASE_URL: origin,
  })
  if (syntheticKey) env.DEEPSEEK_API_KEY = 'rambledesk-offline-synthetic-key'
  return env
}

// Preloaded before the component. Even a changed component cannot send an HTTP
// model request outside the local stub. Do not include target URLs in failures.
const networkGuard = `
import net from 'node:net';
import tls from 'node:tls';
import { syncBuiltinESMExports } from 'node:module';
const origin = new URL(process.env.DEEPSEEK_BASE_URL);
const blocked = () => { process.stderr.write('RAMBLEDESK_SMOKE_NETWORK_BLOCKED\\n'); throw new Error('Offline smoke blocked an external connection'); };
const originalConnect = net.Socket.prototype.connect;
net.Socket.prototype.connect = function (...args) {
  const first = Array.isArray(args[0]) ? args[0][0] : args[0];
  const target = typeof first === 'object' && first !== null ? first : { port: first, host: args[1] };
  if (String(target.port) !== origin.port || !['127.0.0.1', '::1', '[::1]'].includes(target.host)) blocked();
  return originalConnect.apply(this, args);
};
tls.connect = blocked;
const originalFetch = globalThis.fetch;
globalThis.fetch = (input, init) => {
  const url = new URL(typeof input === 'string' || input instanceof URL ? input : input.url);
  if (url.origin !== origin.origin) blocked();
  return originalFetch(input, init);
};
syncBuiltinESMExports();
`

function protocol(child) {
  const pending = new Map()
  const notifications = []
  let buffer = ''
  let sequence = 0
  let invalidStdout = false
  let stderrBytes = 0
  let blockedConnections = 0
  const fail = () => {
    for (const item of pending.values()) item.reject(new Error('ACP process closed before responding'))
    pending.clear()
  }
  child.once('error', fail)
  child.once('exit', fail)
  child.stdin.on('error', fail)
  child.stderr.on('data', bytes => {
    stderrBytes += bytes.length
    blockedConnections += bytes.toString().split('RAMBLEDESK_SMOKE_NETWORK_BLOCKED').length - 1
  })
  child.stdout.setEncoding('utf8')
  child.stdout.on('data', chunk => {
    buffer += chunk
    if (buffer.length > 2 * 1024 * 1024) { invalidStdout = true; fail(); return }
    let newline
    while ((newline = buffer.indexOf('\n')) >= 0) {
      const line = buffer.slice(0, newline)
      buffer = buffer.slice(newline + 1)
      if (!line.trim()) continue
      try {
        const frame = JSON.parse(line)
        if (frame.id !== undefined && pending.has(frame.id)) {
          pending.get(frame.id).resolve(frame)
          pending.delete(frame.id)
        } else if (frame.method === 'session/update') notifications.push(frame.params)
      } catch { invalidStdout = true; fail() }
    }
  })
  return {
    notifications,
    diagnostics: () => ({ invalid_stdout: invalidStdout, stderr_bytes: stderrBytes, blocked_connections: blockedConnections }),
    async request(method, params) {
      const id = ++sequence
      const started = performance.now()
      let timer
      try {
        const response = await new Promise((resolve, reject) => {
          pending.set(id, { resolve, reject })
          timer = setTimeout(() => { pending.delete(id); reject(new Error(`ACP request timed out: ${method}`)) }, 30_000)
          child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`, error => { if (error) reject(new Error('Could not write ACP request')) })
        })
        return { milliseconds: Math.round(performance.now() - started), response }
      } finally { clearTimeout(timer); pending.delete(id) }
    },
  }
}

async function stop(child) {
  if (!child.pid || child.exitCode !== null || child.signalCode !== null) return
  const exited = new Promise(done => child.once('exit', done))
  child.stdin.end()
  let timer
  const graceful = await Promise.race([exited.then(() => true), new Promise(done => { timer = setTimeout(() => done(false), 2_000) })])
  clearTimeout(timer)
  if (graceful) return
  if (process.platform === 'win32') {
    const taskkill = spawn(join(process.env.SystemRoot, 'System32/taskkill.exe'), ['/pid', String(child.pid), '/t', '/f'], { windowsHide: true, stdio: 'ignore' })
    await new Promise(done => { taskkill.once('error', done); taskkill.once('exit', done) })
  } else {
    try { process.kill(-child.pid, 'SIGKILL') } catch { child.kill('SIGKILL') }
  }
  await exited
}

async function scenario(config, name, syntheticKey) {
  const root = join(config.output, name)
  const home = join(root, 'empty-home')
  const project = join(root, 'project')
  for (const directory of [project, home, join(home, 'roaming'), join(home, 'local'), join(home, '.dsh')]) await mkdir(directory, { recursive: true })
  const marker = 'RAMBLEDESK_OFFLINE_FIRST_TURN_OK'
  let requests = 0
  const server = createServer((request, response) => {
    requests += 1
    // Deliberately discard both headers and request body; neither belongs in the report.
    request.resume()
    if (request.method !== 'POST' || request.url !== '/chat/completions') { response.writeHead(404).end(); return }
    response.writeHead(200, { 'Content-Type': 'text/event-stream', Connection: 'close' })
    for (const chunk of [
      { choices: [{ index: 0, delta: { role: 'assistant', content: marker }, finish_reason: null }] },
      { choices: [{ index: 0, delta: {}, finish_reason: 'stop' }], usage: { prompt_tokens: 10, completion_tokens: 4, total_tokens: 14 } },
    ]) response.write(`data: ${JSON.stringify({ id: 'offline-stub', object: 'chat.completion.chunk', created: 0, model: 'deepseek-v4-flash', ...chunk })}\n\n`)
    response.end('data: [DONE]\n\n')
  })
  await new Promise((done, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', done) })
  const origin = `http://127.0.0.1:${server.address().port}`
  const env = environment(config.node, root, home, origin, syntheticKey)
  const child = spawn(config.node, ['--import', pathToFileURL(join(config.output, 'network-guard.mjs')).href, config.entry], {
    cwd: project, env, detached: process.platform !== 'win32', windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'],
  })
  const acp = protocol(child)
  const report = { scenario: name, environment_keys: Object.keys(env).sort(), real_model_access_tested: false, stages: [] }
  try {
    const initialized = await acp.request('initialize', { protocolVersion: 1, clientInfo: { name: 'rambledesk-offline-smoke', version: '0.5.0' }, clientCapabilities: {} })
    assert(!initialized.response.error && initialized.response.result?.protocolVersion === 1, 'ACP initialize failed')
    report.agent = initialized.response.result.agentInfo
    report.stages.push({ method: 'initialize', milliseconds: initialized.milliseconds, ok: true, auth_method_count: initialized.response.result.authMethods?.length ?? 0 })
    const opened = await acp.request('session/new', { cwd: project, mcpServers: [] })
    const sessionId = opened.response.result?.sessionId
    assert(sessionId, 'ACP session creation failed')
    report.stages.push({ method: 'session/new', milliseconds: opened.milliseconds, ok: true })
    const prompted = await acp.request('session/prompt', { sessionId, prompt: [{ type: 'text', text: 'Offline first-use probe. Reply hello without using tools.' }] })
    const error = prompted.response.error
    const receivedMarker = acp.notifications.some(frame => frame.update?.sessionUpdate === 'agent_message_chunk' && frame.update.content?.text?.includes(marker))
    // Preserve wire metadata without agent error prose, provider responses, or credentials.
    report.stages.push({ method: 'session/prompt', milliseconds: prompted.milliseconds, ok: !error,
      ...(error ? { error_code: error.code, error_data_present: Object.hasOwn(error, 'data') } : { stop_reason: prompted.response.result?.stopReason, stub_marker_received: receivedMarker }) })
    if (syntheticKey) {
      assert(!error && prompted.response.result?.stopReason === 'end_turn' && receivedMarker && requests === 1, 'Synthetic loopback first turn did not complete')
    } else assert(error?.code === -32603 && requests === 0, 'Missing credentials must fail locally before any model request')
    const closed = await acp.request('session/close', { sessionId })
    assert(!closed.response.error, 'ACP session close failed')
    report.stages.push({ method: 'session/close', milliseconds: closed.milliseconds, ok: true })
    assert(!acp.diagnostics().invalid_stdout && acp.diagnostics().blocked_connections === 0, 'ACP output or offline network boundary failed')
    report.passed = true
  } catch (error) {
    report.passed = false
    report.failure = error instanceof assert.AssertionError ? error.message : 'ACP offline rehearsal did not complete; inspect stage metadata'
  } finally {
    await stop(child)
    server.closeAllConnections()
    await new Promise(done => server.close(done))
    report.loopback_model_requests = requests
    report.diagnostics = acp.diagnostics()
    await writeFile(join(root, 'report.json'), `${JSON.stringify(report, null, 2)}\n`)
  }
  return report
}

try {
  const config = await options()
  config.node = await realpath(config.node)
  config.entry = await realpath(config.entry)
  assert((await stat(config.node)).isFile() && (await stat(config.entry)).isFile(), 'Node and entry must be files')
  // Verify package identity from its public metadata, never from account configuration.
  const metadata = JSON.parse(await readFile(resolve(dirname(config.entry), '../package.json'), 'utf8'))
  assert.equal(metadata.name, 'deepseek-acp', 'Entry must be a deepseek-acp lib/bin.js installation')
  await mkdir(config.output) // Existing directories are rejected; evidence is never overwritten.
  await writeFile(join(config.output, 'network-guard.mjs'), networkGuard)
  const reports = []
  for (const [name, key] of [['missing-credentials', false], ['synthetic-loopback-success', true]]) reports.push(await scenario(config, name, key))
  const report = { package_version: metadata.version, node: config.node, entry: config.entry, real_model_access_tested: false,
    limitation: 'The success scenario uses a local deterministic model stub and proves protocol wiring only.', passed: reports.every(item => item.passed), scenarios: reports }
  await writeFile(join(config.output, 'report.json'), `${JSON.stringify(report, null, 2)}\n`)
  console.log(JSON.stringify({ passed: report.passed, package_version: metadata.version, output: config.output, scenarios: reports.map(({ scenario, passed, loopback_model_requests }) => ({ scenario, passed, loopback_model_requests })) }, null, 2))
  if (!report.passed) process.exitCode = 1
} catch (error) { console.error(error.message); process.exitCode = 1 }
