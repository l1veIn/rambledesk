import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs'
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path'
import { fileURLToPath } from 'node:url'

const repository = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const templates = join(repository, 'scripts', 'templates', 'workbench')
const reserved = /^(?:con|prn|aux|nul|com[0-9]|lpt[0-9]|as|break|const|continue|crate|else|enum|extern|false|fn|for|if|impl|in|let|loop|match|mod|move|mut|pub|ref|return|self|static|struct|super|trait|true|type|unsafe|use|where|while|async|await|dyn|abstract|become|box|do|final|macro|override|priv|typeof|unsized|virtual|yield|try|gen|default|unknown)$/
function safePath(root, path) {
  const resolved = resolve(root, path)
  const local = relative(root, resolved)
  if (!local || local === '..' || local.startsWith(`..${sep}`) || isAbsolute(local)) throw new Error('Generated path must stay inside the repository')
  return resolved
}
function atomicWrite(path, content) {
  const temporary = `${path}.workbench-new-${process.pid}.tmp`
  try { writeFileSync(temporary, content, { encoding: 'utf8', flag: 'wx' }); renameSync(temporary, path) }
  finally { if (existsSync(temporary)) rmSync(temporary) }
}

/** Creates business files and edits exactly two existing production registration files. */
export function createWorkbench({ type, fixture = false, dryRun = false, root = repository, templateRoot = templates }) {
  root = resolve(root)
  if (typeof type !== 'string' || !/^[a-z][a-z0-9]*(?:_[a-z][a-z0-9]*)*$/.test(type) || type.length > 40 || reserved.test(type)) throw new Error('Use a unique snake_case name of at most 40 characters; paths and reserved names are not allowed')
  const pascal = type.split('_').map((word) => word[0].toUpperCase() + word.slice(1)).join('')
  const camel = pascal[0].toLowerCase() + pascal.slice(1)
  const tokens = { '__snake__': type, '__Pascal__': pascal, '__camel__': camel, '__Title__': pascal.replace(/([a-z])([A-Z])/g, '$1 $2') }
  const render = (file) => Object.entries(tokens).reduce((text, [token, value]) => text.replaceAll(token, value), readFileSync(join(templateRoot, file), 'utf8'))
  const rustPath = safePath(root, 'crates/rambledesk-core/src/workbenches/registry.rs')
  const frontendPath = safePath(root, 'apps/desktop/src/lib/workbench/definitions/registry.ts')
  const rust = readFileSync(rustPath, 'utf8')
  const frontend = readFileSync(frontendPath, 'utf8')
  if (new RegExp(`\\b${type}\\b`).test(rust) || new RegExp(`\\b${camel}Definition\\b`).test(frontend)) throw new Error(`Workbench ${type} is already registered`)
  if (!/}\s*$/.test(rust) || !/export const workbenchDefinitions:[^=]+=[\s\S]*?\n\]/.test(frontend)) throw new Error('Registration format changed; no files were written. Update the scaffold before continuing')
  const feature = fixture ? '    #[cfg(feature = "workbench-fixtures")]\n' : ''
  const stateFeature = fixture ? '#[cfg(feature = "workbench-fixtures")] ' : ''
  const entry = `${feature}    ${pascal} => ${type} {\n        wire: "${type}", data: ${pascal}Data,\n        state: [${stateFeature}${pascal}(${pascal}State),],\n        result: (${pascal}Result),\n        exports: [${pascal}Data, ${pascal}State, ${pascal}Result]\n    }\n`
  const updatedRust = rust.replace(/}\s*$/, `${entry}}\n`)
  const importLine = fixture
    ? `const ${camel}Definitions = import.meta.env.DEV && (import.meta.env.VITE_WORKBENCH_FIXTURES === '1' || import.meta.env.MODE === 'test')\n  ? [(await import('./${type}/definition')).${camel}Definition] : []\n`
    : `import { ${camel}Definition } from './${type}/definition'\n`
  const registration = fixture
    ? `  ...${camel}Definitions,`
    : `  ${camel}Definition,`
  const updatedFrontend = importLine + frontend.replace(/(export const workbenchDefinitions:[^=]+=[\s\S]*?)(\n\])/, `$1\n${registration}$2`)
  const own = `apps/desktop/src/lib/workbench/definitions/${type}`
  const material = `playground/workbenches/materials/${type}.md`
  const fixtureFile = `playground/workbenches/fixtures/${fixture ? 'development/' : ''}${type}.json`
  const files = [
    [`crates/rambledesk-core/src/workbenches/${type}.rs`, render('module.rs.tpl') + render('module-tests.rs.tpl')],
    [`${own}/definition.ts`, render('definition.ts.tpl')], [`${own}/View.svelte`, render('View.svelte.tpl')],
    [`${own}/definition.test.ts`, render('definition.test.ts.tpl')],
    [material, render('experience.md.tpl')],
    [fixtureFile, `${JSON.stringify({ title: tokens.__Title__, what_happened: '阅读材料，选择 1 至 5 分并留下意见。', workbench: { type, version: 1, data: { title: 'Review this proposal', material: 'Assess whether this proposal is ready to use.' } }, attachments: [{ path: `materials/${type}.md` }] }, null, 2)}\n`],
  ].map(([path, text]) => ({ path: safePath(root, path), text }))
  for (const file of files) if (existsSync(file.path)) throw new Error(`Refusing to overwrite ${relative(root, file.path)}`)
  const summary = { type, fixture, created: files.map((file) => relative(root, file.path).split(sep).join('/')), sharedProductionFiles: [rustPath, frontendPath].map((path) => relative(root, path).split(sep).join('/')), dryRun }
  if (dryRun) return summary
  const written = []
  try {
    for (const file of files) { mkdirSync(dirname(file.path), { recursive: true }); writeFileSync(file.path, file.text, { encoding: 'utf8', flag: 'wx' }); written.push(file.path) }
    atomicWrite(rustPath, updatedRust)
    atomicWrite(frontendPath, updatedFrontend)
  } catch (error) {
    atomicWrite(rustPath, rust); atomicWrite(frontendPath, frontend)
    for (const path of written) rmSync(safePath(root, path))
    throw error
  }
  return summary
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const [type, ...flags] = process.argv.slice(2)
    if (flags.some((flag) => !['--fixture', '--dry-run'].includes(flag))) throw new Error('Usage: pnpm workbench:new <snake_case> [--fixture] [--dry-run]')
    console.log(JSON.stringify(createWorkbench({ type, fixture: flags.includes('--fixture'), dryRun: flags.includes('--dry-run') }), null, 2))
    console.log('Next: pnpm contracts:generate; pnpm check; run your business tests and playground scenario. See docs/workbench/adding-a-workbench.md.')
  } catch (error) { console.error(error.message); process.exitCode = 1 }
}
