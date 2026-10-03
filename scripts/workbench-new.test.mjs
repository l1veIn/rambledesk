import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, existsSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { createWorkbench } from './workbench-new.mjs'
import { prepare } from '../playground/workbenches/prepare.mjs'

const rust = 'crates/rambledesk-core/src/workbenches/registry.rs'
const frontend = 'apps/desktop/src/lib/workbench/definitions/registry.ts'
function workspace(t) {
  const root = mkdtempSync(join(tmpdir(), 'rambledesk-scaffold-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  for (const path of [rust, frontend]) { mkdirSync(dirname(join(root, path)), { recursive: true }); writeFileSync(join(root, path), readFileSync(new URL(`../${path}`, import.meta.url))) }
  return root
}
test('generates an ordinary typed workbench, material, example and business tests with only two shared edits', (t) => {
  const root = workspace(t)
  const result = createWorkbench({ root, type: 'proposal_review' })
  assert.deepEqual(result.sharedProductionFiles, [rust, frontend])
  assert.equal(result.created.length, 6)
  for (const path of result.created) assert(!readFileSync(join(root, path), 'utf8').includes('__snake__'))
  assert.match(readFileSync(join(root, rust), 'utf8'), /ProposalReview\(ProposalReviewState\)/)
  assert.match(readFileSync(join(root, frontend), 'utf8'), /proposalReviewDefinition,/)
  assert.match(readFileSync(join(root, result.created[2]), 'utf8'), /proposalReviewMaterialVersion/)
  assert.match(readFileSync(join(root, result.created[1]), 'utf8'), /removeAttachment/)
  assert.match(readFileSync(join(root, result.created[1]), 'utf8'), /expanded: false/)
  assert.match(readFileSync(join(root, result.created[2]), 'utf8'), /\{#if context\.host\.openExpanded\}/)
  assert.match(readFileSync(join(root, result.created[4]), 'utf8'), /若本类型启用了全屏入口/)
  assert.equal(JSON.parse(readFileSync(join(root, result.created[5]), 'utf8')).workbench.type, 'proposal_review')
})
test('development fixture generation gates both registrations and places material outside the default playground run', (t) => {
  const root = workspace(t)
  const result = createWorkbench({ root, type: 'probe_review', fixture: true })
  assert.match(readFileSync(join(root, rust), 'utf8'), /#\[cfg\(feature = "workbench-fixtures"\)\]\s+ProbeReview/)
  assert.match(readFileSync(join(root, frontend), 'utf8'), /import\.meta\.env\.DEV[\s\S]*await import\('\.\/probe_review\/definition'\)/)
  assert(result.created.some((path) => path.includes('fixtures/development/')))
})

for (const development of [false, true]) {
  test(`generated ${development ? 'development' : 'ordinary'} fixture passes real playground preparation`, (t) => {
    const root = workspace(t)
    const type = 'proposal_review'
    createWorkbench({ root, type, fixture: development })
    const base = join(root, 'playground/workbenches')
    const checked = prepare({ base, mode: type, development })
    assert.equal(checked.fixtures.length, 1)
    assert.equal(checked.fixtures[0].type, type)
    if (development) assert.throws(() => prepare({ base, mode: type }), /development fixtures require/)

    const run = prepare({ base, command: 'new', mode: type, development })
    const input = JSON.parse(readFileSync(join(run.directory, checked.fixtures[0].file), 'utf8'))
    assert.equal(run.submitted, false)
    assert.equal(input.attachments[0].file_name, `${type}.md`)
    assert.equal(readFileSync(input.attachments[0].path, 'utf8'), readFileSync(join(base, `materials/${type}.md`), 'utf8'))
  })
}
test('dry run makes the exact extension diff reviewable without writing files', (t) => {
  const root = workspace(t)
  const before = readFileSync(join(root, rust), 'utf8')
  const result = createWorkbench({ root, type: 'probe_review', dryRun: true })
  assert.equal(readFileSync(join(root, rust), 'utf8'), before)
  assert(result.created.every((path) => !existsSync(join(root, path))))
})
test('rejects path traversal, Windows device names, Rust keywords and duplicate registrations before mutation', (t) => {
  const root = workspace(t)
  const before = readFileSync(join(root, frontend), 'utf8')
  for (const type of ['../evil', 'C:\\evil', 'con', 'com1', 'self', 'enum', 'as', 'foo_2', 'rating_review', 'UPPER', 'bad__name']) assert.throws(() => createWorkbench({ root, type }))
  assert.equal(readFileSync(join(root, frontend), 'utf8'), before)
})
test('refuses existing business files without partial registration or overwrite', (t) => {
  const root = workspace(t)
  const path = join(root, 'crates/rambledesk-core/src/workbenches/probe_review.rs')
  writeFileSync(path, 'keep this')
  const before = readFileSync(join(root, rust), 'utf8')
  assert.throws(() => createWorkbench({ root, type: 'probe_review' }), /overwrite/)
  assert.equal(readFileSync(path, 'utf8'), 'keep this')
  assert.equal(readFileSync(join(root, rust), 'utf8'), before)
  assert(!existsSync(join(root, 'apps/desktop/src/lib/workbench/definitions/probe_review/definition.ts')))
})
