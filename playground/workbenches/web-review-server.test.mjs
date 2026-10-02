import { test } from 'node:test'
import assert from 'node:assert/strict'
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import { get } from 'node:http'
import { randomUUID } from 'node:crypto'
import { tmpdir } from 'node:os'
import { dirname, join, relative, resolve, sep } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = dirname(fileURLToPath(import.meta.url))
const readJson = path => JSON.parse(readFileSync(path, 'utf8'))
const writeJson = (path, value) => writeFileSync(path, JSON.stringify(value), 'utf8')
const command = (script, args, cwd = root) => JSON.parse(execFileSync(process.execPath, [script, ...args], { cwd, encoding: 'utf8', timeout: 10000, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] }))

test('prepares the complete playground or a single workbench and refuses to replace an unfinished run', () => {
  const directory = mkdtempSync(join(tmpdir(), 'ramble-playground-'))
  try {
    for (const entry of ['prepare.mjs', 'fixtures', 'materials']) cpSync(join(root, entry), join(directory, entry), { recursive: true })
    const check = command('prepare.mjs', ['check'], directory)
    assert.equal(check.submitted, false)
    assert.deepEqual(check.fixtures.map(fixture => fixture.type), ['ramble', 'questions', 'questions', 'document_review', 'web_review', 'terminal', 'sort'])
    const prepared = command('prepare.mjs', ['new'], directory)
    const runPath = join(prepared.directory, 'run.json')
    const run = readJson(runPath)
    assert.deepEqual(run.stages.map(stage => stage.type), check.fixtures.map(fixture => fixture.type))
    assert.equal(new Set(run.stages.map(stage => stage.request_id)).size, check.fixtures.length)
    assert.equal(readJson(join(prepared.directory, '01-ramble.json')).title, `1/${run.stages.length} · 自由反馈`)
    assert.equal(readJson(join(prepared.directory, 'sort.json')).title, `${run.stages.length}/${run.stages.length} · 拖动排序`)
    assert.equal(readJson(join(prepared.directory, '06-terminal.json')).workbench.data.cwd, realpathSync(directory))
    assert.throws(() => command('prepare.mjs', ['new', 'web_review'], directory), /Resume/)
    run.status = 'completed'
    writeJson(runPath, run)
    const web = command('prepare.mjs', ['new', 'web_review'], directory)
    const only = readJson(join(web.directory, 'run.json'))
    assert.equal(only.stages.length, 1)
    assert.equal(only.stages[0].type, 'web_review')
    const input = readJson(join(web.directory, '05-web-review.json'))
    assert.equal(input.request_id, only.stages[0].request_id)
    assert.equal(input.title, '网页评审 · 独立体验')
    assert.ok(input.attachments.every(attachment => existsSync(attachment.path)))
    assert.throws(() => command('prepare.mjs', ['new', 'terminal'], directory), /Resume/)
    only.status = 'completed'
    writeJson(join(web.directory, 'run.json'), only)
    const terminal = command('prepare.mjs', ['new', 'terminal'], directory)
    const trial = readJson(join(terminal.directory, 'run.json'))
    assert.equal(trial.stages.length, 1)
    assert.equal(trial.stages[0].type, 'terminal')
    const trialInput = readJson(join(terminal.directory, '06-terminal.json'))
    assert.equal(trialInput.request_id, trial.stages[0].request_id)
    assert.equal(trialInput.workbench.data.cwd, realpathSync(directory))
    assert.equal(trialInput.title, '终端试用 · 独立体验')
    assert.ok(trialInput.attachments.every(attachment => existsSync(attachment.path)))
  } finally {
    assert.equal(dirname(realpathSync(directory)), realpathSync(tmpdir()))
    assert.ok(directory.includes('ramble-playground-'))
    rmSync(directory, { recursive: true })
  }
})

test('serves the real bridged page, preserves a sent URL across restart and stops only its own service', async () => {
  const runId = `smoke-${randomUUID()}`
  const directory = join(root, '.runs', runId)
  mkdirSync(directory, { recursive: true })
  const requestId = randomUUID()
  const input = readJson(join(root, 'fixtures/05-web-review.json'))
  input.request_id = requestId
  const run = { run_id: runId, status: 'running', stages: [{ file: '05-web-review.json', type: 'web_review', request_id: requestId, status: 'prepared' }] }
  writeJson(join(directory, 'run.json'), run)
  writeJson(join(directory, '05-web-review.json'), input)
  const server = action => command('web-review-server.mjs', [action, directory])
  try {
    const started = server('start')
    const url = started.url
    assert.equal(started.running, true)
    assert.equal(new URL(url).hostname, '127.0.0.1')
    assert.equal(server('start').url, url)
    assert.equal(server('status').running, true)
    const prepared = readJson(join(directory, '05-web-review.json'))
    assert.equal(prepared.request_id, requestId)
    assert.equal(prepared.workbench.data.url, url)
    const page = await fetch(url)
    assert.equal(page.status, 200)
    assert.match(await page.text(), /data-testid="review-signup"/)
    const bridge = await fetch(new URL('/rambledesk-web-review.js', url))
    assert.equal(await bridge.text(), readFileSync(resolve(root, '../../apps/desktop/public/rambledesk-web-review.js'), 'utf8'))
    assert.equal((await fetch(new URL('/run.json', url))).status, 404)
    const wrongHost = await new Promise((done, reject) => get(url, { headers: { Host: 'example.com' } }, response => { response.resume(); done(response.statusCode) }).on('error', reject))
    assert.equal(wrongHost, 403)
    run.stages[0].status = 'handed_off'
    writeJson(join(directory, 'run.json'), run)
    const before = readJson(join(directory, 'web-review-server.json')).instance_id
    server('stop')
    assert.equal(server('status').running, false)
    assert.equal(server('start').url, url)
    assert.notEqual(readJson(join(directory, 'web-review-server.json')).instance_id, before)
    assert.equal(readJson(join(directory, '05-web-review.json')).request_id, requestId)
  } finally {
    server('stop')
    assert.equal(server('status').running, false)
    const local = relative(realpathSync(root), realpathSync(directory))
    assert.equal(local, `.runs${sep}${runId}`)
    rmSync(directory, { recursive: true })
  }
})
