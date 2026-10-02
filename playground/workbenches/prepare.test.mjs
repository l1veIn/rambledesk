import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { prepare } from './prepare.mjs'

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
    attachments: [{ path: 'materials/brief.md' }], ...extra,
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
  fixture(base, '01-custom.json', 'custom_review', { attachments: [{ path: '../missing.md' }] })
  assert.throws(() => prepare({ base, command: 'new' }))
  assert.throws(() => readFileSync(join(base, '.runs', 'latest.json')))
})
