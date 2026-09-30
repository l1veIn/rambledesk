// Only serves the four fixed review assets. No feedback API or credentials.
import { closeSync, existsSync, openSync, readFileSync, realpathSync, renameSync, writeFileSync } from 'node:fs'
import { randomUUID } from 'node:crypto'
import { createServer } from 'node:http'
import { spawn } from 'node:child_process'
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from 'node:path'
import { fileURLToPath } from 'node:url'

const script = fileURLToPath(import.meta.url)
const root = dirname(script)
const publicRoot = resolve(root, '../../apps/desktop/public')
const assets = new Map([
  ['/', ['web-review-fixture.html', 'text/html; charset=utf-8']],
  ['/web-review-fixture.css', ['web-review-fixture.css', 'text/css; charset=utf-8']],
  ['/web-review-fixture.js', ['web-review-fixture.js', 'text/javascript; charset=utf-8']],
  ['/rambledesk-web-review.js', ['rambledesk-web-review.js', 'text/javascript; charset=utf-8']],
])
const readJson = path => JSON.parse(readFileSync(path, 'utf8').replace(/^\uFEFF/, ''))
function writeJson(path, value) {
  const temporary = `${path}.${randomUUID()}.tmp`
  writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`, { encoding: 'utf8', mode: 0o600 })
  renameSync(temporary, path)
}

function setup(path) {
  const directory = realpathSync(resolve(path))
  const local = relative(realpathSync(root), directory)
  if (isAbsolute(local) || !local.startsWith(`.runs${sep}`) || local.includes(`..${sep}`)) throw new Error('Run directory must be inside this playground\'s .runs/')
  const run = readJson(join(directory, 'run.json'))
  if (run.run_id !== basename(directory)) throw new Error('Run identity does not match its directory')
  const stage = run.stages.find(item => item.file === '05-web-review.json' && item.type === 'web_review')
  if (!stage) throw new Error('This run has no web review stage; do not add one to a historical run')
  const inputPath = join(directory, stage.file)
  const input = readJson(inputPath)
  if (input.request_id !== stage.request_id || input.workbench?.type !== 'web_review') throw new Error('Web review request identity does not match the run')
  return { directory, run, stage, input, inputPath, receiptPath: join(directory, 'web-review-server.json') }
}

function receipt(info) {
  if (!existsSync(info.receiptPath)) return null
  const value = readJson(info.receiptPath)
  const url = new URL(value.url)
  if (value.run_id !== info.run.run_id || url.protocol !== 'http:' || url.hostname !== '127.0.0.1' || !url.port || url.pathname !== '/' || url.search || url.hash || url.username || url.password
    || typeof value.instance_id !== 'string' || typeof value.control_token !== 'string') throw new Error('Invalid local review server receipt')
  return value
}

async function running(value) {
  if (!value) return false
  try {
    const response = await fetch(new URL('/__playground__/health', value.url), { signal: AbortSignal.timeout(600) })
    const health = await response.json()
    return response.ok && health.run_id === value.run_id && health.instance_id === value.instance_id
  } catch { return false }
}

function bindInput(info, value) {
  if (info.input.workbench.data.url === value.url) return
  if (info.stage.status !== 'prepared') throw new Error('A sent request URL is immutable; restart its original server instead')
  info.input.workbench.data.url = value.url
  writeJson(info.inputPath, info.input)
}

async function start(info) {
  const previous = receipt(info)
  if (await running(previous)) { bindInput(info, previous); return previous.url }
  if (info.run.status !== 'running') throw new Error('Only an active run can start its review page')
  if (info.stage.status !== 'prepared' && !previous) throw new Error('Missing original server receipt; cannot change a sent request URL')
  const log = openSync(join(info.directory, 'web-review-server.log'), 'a')
  const child = spawn(process.execPath, [script, 'serve', info.directory], { detached: true, windowsHide: true, stdio: ['ignore', log, log] })
  closeSync(log)
  child.unref()
  let spawnError
  child.once('error', error => { spawnError = error })
  for (let attempt = 0; attempt < 25; attempt += 1) {
    if (spawnError) throw spawnError
    const current = receipt(info)
    if (await running(current)) { bindInput(info, current); return current.url }
    await new Promise(done => setTimeout(done, 100))
  }
  throw new Error('Review page did not start; inspect web-review-server.log. The original port may be occupied.')
}

async function serve(info) {
  const previous = receipt(info)
  const contents = new Map([...assets].map(([path, [file, type]]) => [path, { body: readFileSync(join(publicRoot, file)), type }]))
  const value = { run_id: info.run.run_id, instance_id: randomUUID(), control_token: randomUUID(), url: '', pid: process.pid }
  const server = createServer((request, response) => {
    if (request.headers.host !== new URL(value.url).host) { response.writeHead(403).end(); return }
    const path = new URL(request.url, value.url).pathname
    if (request.method === 'GET' && path === '/__playground__/health') {
      response.writeHead(200, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' }).end(JSON.stringify({ run_id: value.run_id, instance_id: value.instance_id })); return
    }
    if (request.method === 'POST' && path === '/__playground__/stop' && request.headers.authorization === `Bearer ${value.control_token}`) {
      response.writeHead(200).end('Stopped'); server.close(); server.closeIdleConnections?.(); return
    }
    const asset = contents.get(path)
    if (!asset || !['GET', 'HEAD'].includes(request.method)) { response.writeHead(404).end(); return }
    response.writeHead(200, { 'Content-Type': asset.type, 'Cache-Control': 'no-store', 'X-Content-Type-Options': 'nosniff' })
    response.end(request.method === 'HEAD' ? undefined : asset.body)
  })
  server.on('error', error => { console.error(error.message); process.exitCode = 1 })
  server.listen(previous ? Number(new URL(previous.url).port) : 0, '127.0.0.1', () => {
    value.url = `http://127.0.0.1:${server.address().port}/`
    writeJson(info.receiptPath, value)
  })
}

try {
  const [command, path] = process.argv.slice(2)
  if (!['start', 'status', 'stop', 'serve'].includes(command) || !path || process.argv.length !== 4) throw new Error('Usage: node web-review-server.mjs [start|status|stop] <run-directory> (Node 18+)')
  const info = setup(path)
  if (command === 'serve') await serve(info)
  else if (command === 'start') console.log(JSON.stringify({ running: true, url: await start(info), input: info.inputPath }))
  else {
    const value = receipt(info)
    const active = await running(value)
    if (command === 'stop' && active) {
      const response = await fetch(new URL('/__playground__/stop', value.url), { method: 'POST', headers: { Authorization: `Bearer ${value.control_token}` }, signal: AbortSignal.timeout(2000) })
      if (!response.ok) throw new Error('Could not stop this run\'s review server')
    }
    console.log(JSON.stringify({ running: command === 'status' && active, ...(active ? { url: value.url } : {}) }))
  }
} catch (error) { console.error(error.message); process.exitCode = 1 }
