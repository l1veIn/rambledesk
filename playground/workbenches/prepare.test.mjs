import { test } from 'node:test'
import assert from 'node:assert/strict'
import { cpSync, existsSync, mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, isAbsolute, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { execFile } from 'node:child_process'
import { createServer } from 'node:http'
import { promisify } from 'node:util'
import { loadCases, prepare } from './prepare.mjs'

function playground(t) {
  const base = mkdtempSync(join(tmpdir(), 'rambledesk-prepare-'))
  t.after(() => rmSync(base, { recursive: true, force: true }))
  mkdirSync(join(base, 'fixtures', 'development'), { recursive: true })
  mkdirSync(join(base, 'materials'))
  writeFileSync(join(base, 'materials', 'brief.md'), '# Material')
  return base
}
function fixture(base, name, type = 'custom_review', extra = {}) {
  writeFileSync(join(base, 'fixtures', name), JSON.stringify({
    title: 'Custom review', what_happened: 'Try this workbench', workbench: { type, version: 1, data: { title: 'Review' } },
    attachments: [{ file_name: 'brief.md', path: 'materials/brief.md' }], ...extra,
  }))
}
test('new ordinary types are discovered with no shared dispatch edit; development examples are opt-in', (t) => {
  const base = playground(t)
  fixture(base, '01-custom.json')
  fixture(base, 'development/rating.json', 'rating_review')
  assert.deepEqual(prepare({ base }).fixtures.map((item) => item.type), ['custom_review'])
  assert.equal(prepare({ base, mode: 'rating_review', development: true }).fixtures[0].type, 'rating_review')
  assert.throws(() => prepare({ base, mode: 'rating_review' }), /No fixtures/)
})
test('prepared requests are immutable and unfinished runs must be resumed', (t) => {
  const base = playground(t)
  fixture(base, '01-custom.json')
  const run = prepare({ base, command: 'new' })
  const path = join(run.directory, '01-custom.json')
  const saved = readFileSync(path, 'utf8')
  assert.match(JSON.parse(saved).request_id, /^[\da-f-]{36}$/)
  fixture(base, '01-custom.json', 'changed_review')
  assert.throws(() => prepare({ base, command: 'new' }), /Resume/)
  assert.equal(readFileSync(path, 'utf8'), saved)
})
test('rejects identity/completion overrides and attachments outside the playground before creating runs', (t) => {
  const base = playground(t)
  fixture(base, '01-custom.json', 'custom_review', { request_id: '' })
  assert.throws(() => prepare({ base, command: 'new' }), /session identity/)
  fixture(base, '01-custom.json', 'custom_review', { attachments: [{ file_name: 'brief.md', path: '../missing.md' }] })
  assert.throws(() => prepare({ base, command: 'new' }), /01-custom.json: attachments\[0\].path/)
  assert.throws(() => readFileSync(join(base, '.runs', 'latest.json')))
})

test('missing or invalid attachment names fail check and new before any run is written', (t) => {
  const base = playground(t)
  for (const file_name of [undefined, null, 1, '', '  ', '.', '..', 'dir/brief.md', 'dir\\brief.md', 'a\0.md', 'a'.repeat(256), '中'.repeat(86)]) {
    fixture(base, '01-custom.json', 'custom_review', { attachments: [{ file_name, path: 'materials/brief.md' }] })
    for (const command of ['check', 'new']) {
      assert.throws(() => prepare({ base, command }), /01-custom.json: attachments\[0\].file_name/)
      assert.equal(existsSync(join(base, '.runs')), false)
    }
  }
})

test('attachment names preserve Unicode and the backend byte limit without guessing an extension', (t) => {
  const base = playground(t)
  for (const [file_name, expected] of [
    ['  体验 brief.MARKDOWN  ', '体验 brief.MARKDOWN'],
    ['笔记', '笔记'], ['中'.repeat(85), '中'.repeat(85)], ['\uFEFF', '\uFEFF'],
  ]) {
    fixture(base, '01-custom.json', 'custom_review', { attachments: [{ file_name, path: 'materials/brief.md' }] })
    assert.equal(loadCases(base)[0].input.attachments[0].file_name, expected)
  }
})

test('attachment path and source failures identify the fixture and do not leave a run', (t) => {
  const base = playground(t)
  for (const attachment of [
    { file_name: 'brief.md' },
    { file_name: 'brief.md', path: '' },
    { file_name: 'brief.md', path: 'materials' },
    { file_name: 'brief.md', path: 'materials/brief.md', markdown: '# Duplicate source' },
    { file_name: 'brief.md', path: 'materials/brief.md', contents_base64: 'AAAA' },
  ]) {
    fixture(base, '01-custom.json', 'custom_review', { attachments: [attachment] })
    assert.throws(() => prepare({ base, command: 'new' }), /01-custom.json: attachments\[0\].path/)
    assert.equal(existsSync(join(base, '.runs')), false)
  }
})

test('repository fixtures prepare explicit attachment names and the exact image material contract', (t) => {
  const base = playground(t)
  const source = dirname(fileURLToPath(import.meta.url))
  for (const directory of ['fixtures', 'materials']) cpSync(join(source, directory), join(base, directory), { recursive: true })
  const loaded = loadCases(base, true)
  for (const file of ['07-visual-feedback.json', '08-visual-image.json', '09-diff-review.json', '10-table-review.json',
    '11-media-audio.json', '12-media-video.json', 'sort.json', 'development-rating_review.json']) {
    assert.ok(loaded.some((item) => item.file === file), `${file} is included in the contract regression`)
  }
  const run = prepare({ base, command: 'new', development: true })
  for (const [index, item] of loaded.entries()) {
    const saved = JSON.parse(readFileSync(join(run.directory, item.file), 'utf8'))
    assert.match(saved.title, new RegExp(`^${index + 1}/${loaded.length} · `))
    assert.deepEqual(saved.attachments, item.input.attachments)
    assert.ok(saved.attachments.every((attachment) => isAbsolute(attachment.path) && typeof attachment.file_name === 'string'))
  }
  const image = loaded.find((item) => item.file === '08-visual-image.json').input
  const matches = image.attachments.filter((attachment) => attachment.file_name === image.workbench.data.image_file_name)
  assert.equal(matches.length, 1)
  const png = readFileSync(matches[0].path)
  assert.deepEqual([...png.subarray(0, 8)], [137, 80, 78, 71, 13, 10, 26, 10])
  assert.equal(png.readUInt32BE(16), image.workbench.data.width)
  assert.equal(png.readUInt32BE(20), image.workbench.data.height)
  for (const item of loaded.filter((item) => item.type === 'media_review')) {
    const { media_file_name, duration_ms } = item.input.workbench.data
    const materials = item.input.attachments.filter((attachment) => attachment.file_name === media_file_name)
    assert.equal(materials.length, 1, `${item.file} references one exact frozen media filename`)
    const bytes = readFileSync(materials[0].path)
    assert.ok(bytes.length > 44 && bytes.length <= 20 * 1024 * 1024)
    assert.ok(Number.isInteger(duration_ms) && duration_ms > 0)
  }
})

test('the real CLI decodes prepared fixtures before an isolated loopback handoff', {
  skip: !process.env.RAMBLEDESK_PLAYGROUND_TEST_COMMAND,
}, async (t) => {
  const base = playground(t)
  const source = dirname(fileURLToPath(import.meta.url))
  for (const directory of ['fixtures', 'materials']) cpSync(join(source, directory), join(base, directory), { recursive: true })
  const run = prepare({ base, command: 'new', development: true })
  const received = []
  // This endpoint only echoes decoded CLI inputs; it owns no application or database.
  const server = createServer(async (request, response) => {
    const chunks = []
    for await (const chunk of request) chunks.push(chunk)
    const input = JSON.parse(Buffer.concat(chunks).toString('utf8'))
    received.push({ path: request.url, input })
    response.setHeader('content-type', 'application/json')
    response.end(JSON.stringify({ fixture: true, received: input }))
  })
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve) })
  t.after(() => new Promise((resolve) => { server.closeAllConnections(); server.close(resolve) }))
  const env = {
    PATH: process.env.PATH, SystemRoot: process.env.SystemRoot, TEMP: process.env.TEMP, TMP: process.env.TMP,
    RAMBLEDESK_FEEDBACK_URL: `http://127.0.0.1:${server.address().port}/agent-feedback`,
    RAMBLEDESK_FEEDBACK_TOKEN: '0'.repeat(64),
  }
  const execute = promisify(execFile)
  const stages = JSON.parse(readFileSync(join(run.directory, 'run.json'), 'utf8')).stages
  for (const stage of stages) {
    const path = join(run.directory, stage.file)
    const input = JSON.parse(readFileSync(path, 'utf8'))
    const result = await execute(process.env.RAMBLEDESK_PLAYGROUND_TEST_COMMAND, ['feedback', 'request', '--input', path], {
      env, timeout: 10000, windowsHide: true,
    })
    const receipt = JSON.parse(result.stdout)
    assert.equal(receipt.fixture, true)
    assert.equal(receipt.received.request_id, input.request_id)
    assert.deepEqual(receipt.received.attachments, input.attachments)
  }
  assert.equal(received.length, stages.length)
  assert.ok(received.every((request) => request.path === '/agent-feedback/request'))
  // Reproduce the original missing field: CLI rejects it before any network request.
  const invalid = JSON.parse(readFileSync(join(run.directory, '08-visual-image.json'), 'utf8'))
  delete invalid.attachments[0].file_name
  const invalidPath = join(base, 'missing-file-name.json')
  writeFileSync(invalidPath, JSON.stringify(invalid))
  await assert.rejects(execute(process.env.RAMBLEDESK_PLAYGROUND_TEST_COMMAND, ['feedback', 'request', '--input', invalidPath], {
    env, timeout: 10000, windowsHide: true,
  }), (error) => JSON.parse(error.stdout).code === 'invalid_input')
  assert.equal(received.length, stages.length)
})
