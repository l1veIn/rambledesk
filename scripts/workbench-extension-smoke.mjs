// A real extension rehearsal. Restores existing files and removes only its own generated files.
import { createHash } from 'node:crypto'
import { existsSync, readFileSync, rmSync, rmdirSync, writeFileSync } from 'node:fs'
import { spawnSync } from 'node:child_process'
import { dirname, isAbsolute, relative, resolve, sep } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createWorkbench } from './workbench-new.mjs'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const type = 'extension_probe'
const tracked = spawnSync('git', ['ls-files', '-z'], { cwd: root, encoding: 'utf8' })
if (tracked.status !== 0) throw new Error('A Git checkout is required for the extension diff proof')
const production = tracked.stdout.split('\0').filter((path) => path && /^(?:apps\/desktop\/src\/|crates\/)/.test(path)
  && /\.(?:rs|ts|svelte)$/.test(path) && !/(?:\.test\.|\/tests\/)/.test(path))
// During the framework's first implementation the registration files can still be untracked.
for (const path of ['crates/rambledesk-core/src/workbenches/registry.rs', 'apps/desktop/src/lib/workbench/definitions/registry.ts']) {
  if (!production.includes(path)) production.push(path)
}
const digest = (path) => createHash('sha256').update(readFileSync(resolve(root, path), 'utf8').replaceAll('\r\n', '\n')).digest('hex')
const before = new Map(production.map((path) => [path, digest(path)]))
const restoreFiles = ['crates/rambledesk-core/src/workbenches/registry.rs', 'apps/desktop/src/lib/workbench/definitions/registry.ts',
  'apps/desktop/src/lib/generated/feedback.ts', 'apps/desktop/src/lib/generated/hosts.ts']
const backups = restoreFiles.map((path) => [path, readFileSync(resolve(root, path))])
function inside(path) {
  const target = resolve(root, path)
  const local = relative(root, target)
  if (!local || local === '..' || local.startsWith(`..${sep}`) || isAbsolute(local)) throw new Error('Rehearsal cleanup escaped the checkout')
  return target
}
function run(command, args) {
  if (command === 'pnpm') {
    if (!process.env.npm_execpath) throw new Error('Run this rehearsal through pnpm test:workbench-extension')
    args = [process.env.npm_execpath, ...args]; command = process.execPath
  }
  const result = spawnSync(command, args, { cwd: root, stdio: 'inherit' })
  if (result.error) throw result.error
  if (result.status !== 0) throw new Error(`Extension check failed: ${command} ${args.join(' ')}`)
}
let created
let proof
try {
  created = createWorkbench({ type })
  run('cargo', ['fmt', '--all'])
  run('cargo', ['test', '-p', 'rambledesk-core', '--features', 'workbench-fixtures', type])
  run('pnpm', ['contracts:generate'])
  run('pnpm', ['check'])
  run('pnpm', ['-C', 'apps/desktop', 'test', `src/lib/workbench/definitions/${type}`])
  run(process.execPath, ['playground/workbenches/prepare.mjs', 'check', type])
  const changed = production.filter((path) => digest(path) !== before.get(path))
  const shared = changed.filter((path) => !path.startsWith('apps/desktop/src/lib/generated/'))
  const expected = [...created.sharedProductionFiles].sort()
  if (JSON.stringify(shared.sort()) !== JSON.stringify(expected)) throw new Error(`Extension touched unexpected shared production files: ${shared.join(', ')}`)
  proof = { type, created: created.created, sharedProductionFiles: shared, generatedFiles: changed.filter((path) => path.includes('/generated/')),
    checks: ['generated Rust business tests + overlapping result package roundtrip', 'canonical contracts', 'Svelte/TypeScript check', 'generated frontend business tests', 'automatic playground discovery'],
    templateDigests: ['module.rs.tpl', 'definition.ts.tpl', 'View.svelte.tpl'].map((file) => ({ file, sha256: digest(`scripts/templates/workbench/${file}`) })) }
} finally {
  if (created) {
    for (const [path, contents] of backups) writeFileSync(inside(path), contents)
    for (const path of created.created) { const target = inside(path); if (existsSync(target)) rmSync(target) }
    rmdirSync(inside(`apps/desktop/src/lib/workbench/definitions/${type}`))
  }
}
console.log(JSON.stringify({ ...proof, restored: true }, null, 2))
