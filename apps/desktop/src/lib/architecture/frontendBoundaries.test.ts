import { existsSync, readdirSync, readFileSync } from 'node:fs'
import { dirname, join, normalize, relative, sep } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'
import ts from 'typescript'

/**
 * Frontend dependency direction.
 *
 * The list below freezes existing cross-domain imports. A new domain must
 * document each necessary direction here and in ARCHITECTURE.md; incidental
 * imports must not grow the list. An entry whose last import disappears must
 * be deleted. `(app)` and `dev` are
 * composition roots, so they may import anything and are not scanned.
 *
 * Notable entries and why they exist:
 * - `lib/workspace` is the view layer; `lib/workbench` may depend on it, never
 *   the other way around.
 * - `lib/domain` is the shared vocabulary kernel; it may only reach
 *   `lib/generated` and the `lib/feedback` contract barrel (`lib/(root)`).
 * - `lib/capabilities` owns the platform contracts, so it may read the
 *   `lib/speech`, `lib/settings` and `lib/updates` types it exposes.
 * - `lib/components` is shared UI; it must not reach feature controllers.
 * - `lib/input` composes acquisition UI and request-scoped input selection.
 *   It may use speech contracts/controls; speech must not import input.
 *   Its root imports are shared contracts/codecs/i18n, not root controllers.
 */
const ALLOWED_EDGES: readonly string[] = [
  // lib root modules (cross-cutting helpers and contract barrels)
  'lib/(root) -> lib/application',
  'lib/(root) -> lib/components',
  // Draft transforms share neutral input identities, never input UI/controllers.
  'lib/(root) -> lib/domain',
  'lib/(root) -> lib/speech',
  'lib/(root) -> lib/workbench',
  'lib/(root) -> lib/workspace',
  // agents
  'lib/agents -> lib/(root)',
  'lib/agents -> lib/application',
  'lib/agents -> lib/capabilities',
  'lib/agents -> lib/diagnostics',
  'lib/agents -> lib/editor',
  'lib/agents -> lib/workspace',
  // appearance
  'lib/appearance -> lib/(root)',
  'lib/appearance -> lib/capabilities',
  // application (contract and transport layer)
  'lib/application -> lib/capabilities',
  'lib/application -> lib/diagnostics',
  // capabilities (platform contracts and implementations)
  'lib/capabilities -> lib/(root)',
  'lib/capabilities -> lib/settings',
  'lib/capabilities -> lib/speech',
  'lib/capabilities -> lib/updates',
  // components (shared UI)
  'lib/components -> lib/(root)',
  'lib/components -> lib/agents',
  'lib/components -> lib/domain',
  'lib/components -> lib/workspace',
  // desktop shell, diagnostics
  'lib/desktop-shell -> lib/diagnostics',
  'lib/diagnostics -> lib/capabilities',
  // domain (shared vocabulary kernel)
  'lib/domain -> lib/(root)',
  // editor
  'lib/editor -> lib/(root)',
  'lib/editor -> lib/capabilities',
  'lib/editor -> lib/speech',
  // input composes shared voice controls/target guards with non-speech input.
  // Visible fields reuse the same TipTap lifecycle; they do not own draft/session controllers.
  'lib/input -> lib/editor',
  'lib/input -> lib/speech',
  'lib/input -> lib/domain',
  // Existing contract barrels, draft codecs, attachment URLs and UI preferences.
  'lib/input -> lib/(root)',
  // onboarding
  'lib/onboarding -> lib/(root)',
  'lib/onboarding -> lib/agents',
  'lib/onboarding -> lib/application',
  'lib/onboarding -> lib/capabilities',
  'lib/onboarding -> lib/diagnostics',
  'lib/onboarding -> lib/settings',
  'lib/onboarding -> lib/speech',
  // preview (fixture transport for ?preview=fixtures)
  'lib/preview -> lib/(root)',
  'lib/preview -> lib/application',
  'lib/preview -> lib/capabilities',
  'lib/preview -> lib/domain',
  // Fixture transport discovers pure definition metadata; lazy views never load here.
  'lib/preview -> lib/workbench',
  // rambelle
  'lib/rambelle -> lib/(root)',
  // screen capture
  'lib/screen-capture -> lib/(root)',
  // settings
  'lib/settings -> lib/(root)',
  'lib/settings -> lib/agents',
  'lib/settings -> lib/appearance',
  'lib/settings -> lib/application',
  'lib/settings -> lib/capabilities',
  'lib/settings -> lib/diagnostics',
  'lib/settings -> lib/domain',
  'lib/settings -> lib/speech',
  'lib/settings -> lib/updates',
  'lib/settings -> lib/workspace',
  // shell
  'lib/shell -> lib/(root)',
  'lib/shell -> lib/capabilities',
  'lib/shell -> lib/domain',
  // speech
  'lib/speech -> lib/(root)',
  'lib/speech -> lib/domain',
  'lib/speech -> lib/settings',
  // Writeback/tidy validate editability through the headless registry only.
  // They must not import business views or workbench session controllers.
  'lib/speech -> lib/workbench',
  // updates
  'lib/updates -> lib/(root)',
  'lib/updates -> lib/domain',
  'lib/updates -> lib/capabilities',
  'lib/updates -> lib/desktop-shell',
  // web access
  'lib/web-access -> lib/(root)',
  'lib/web-access -> lib/application',
  // workbench (controllers and workbench cards; may depend on the view layer)
  'lib/workbench -> lib/(root)',
  'lib/workbench -> lib/agents',
  'lib/workbench -> lib/application',
  'lib/workbench -> lib/capabilities',
  'lib/workbench -> lib/components',
  'lib/workbench -> lib/diagnostics',
  'lib/workbench -> lib/domain',
  'lib/workbench -> lib/editor',
  // Workbench fields consume common input tools, attachment views and writeback.
  'lib/workbench -> lib/input',
  'lib/workbench -> lib/settings',
  'lib/workbench -> lib/speech',
  'lib/workbench -> lib/workspace',
  // workspace (views)
  'lib/workspace -> lib/(root)',
  'lib/workspace -> lib/agents',
  'lib/workspace -> lib/application',
  'lib/workspace -> lib/capabilities',
  'lib/workspace -> lib/components',
  'lib/workspace -> lib/diagnostics',
  'lib/workspace -> lib/domain',
  'lib/workspace -> lib/editor',
  'lib/workspace -> lib/rambelle',
  'lib/workspace -> lib/settings',
]

const sourceRoot = fileURLToPath(new URL('../../', import.meta.url))
const importPattern = /from\s+'([^']+)'/gu

function sourceFiles(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    if (entry.name === 'node_modules' || entry.name === 'dist') return []
    const path = join(directory, entry.name)
    if (entry.isDirectory()) return sourceFiles(path)
    if (!entry.name.endsWith('.ts') && !entry.name.endsWith('.svelte')) return []
    if (entry.name.endsWith('.test.ts')) return []
    return [path]
  })
}

function domainOf(file: string): string | null {
  const path = relative(sourceRoot, file).split(sep).join('/')
  if (path.startsWith('lib/components/ui/') || path.startsWith('lib/generated/')) return null
  if (path.startsWith('lib/')) {
    const parts = path.split('/')
    return parts.length > 2 ? `lib/${parts[1]}` : 'lib/(root)'
  }
  if (path.startsWith('dev/')) return 'dev'
  return '(app)'
}

function resolveImport(specifier: string, importer: string): string | null {
  let base: string
  if (specifier.startsWith('$lib/')) {
    base = normalize(join(sourceRoot, 'lib', specifier.slice('$lib/'.length)))
  } else if (specifier.startsWith('.')) {
    base = normalize(join(dirname(importer), specifier))
  } else {
    return null
  }
  for (const candidate of [base, `${base}.ts`, `${base}.svelte`, join(base, 'index.ts')]) {
    if (existsSync(candidate) && !candidate.endsWith('.test.ts')) return candidate
  }
  return null
}

function collectEdges(): Set<string> {
  const edges = new Set<string>()
  for (const file of sourceFiles(sourceRoot)) {
    const source = domainOf(file)
    if (source === null || source === '(app)' || source === 'dev') continue
    for (const match of readFileSync(file, 'utf8').matchAll(importPattern)) {
      const target = resolveImport(match[1]!, file)
      // Assets (images, css, fonts) are build inputs, not module dependencies.
      if (!target || !/\.(ts|svelte)$/u.test(target)) continue
      const targetDomain = domainOf(target)
      if (targetDomain === null || targetDomain === source) continue
      edges.add(`${source} -> ${targetDomain}`)
    }
  }
  return edges
}

function runtimeImports(file: string): string[] {
  const source = ts.createSourceFile(file, readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, true)
  const targets: string[] = []
  function visit(node: ts.Node) {
    let specifier: string | undefined
    if (ts.isImportDeclaration(node) && !node.importClause?.isTypeOnly && ts.isStringLiteral(node.moduleSpecifier)) {
      const clause = node.importClause
      const bindings = clause?.namedBindings
      const typesOnly = bindings && ts.isNamedImports(bindings) && !clause?.name && bindings.elements.every((element) => element.isTypeOnly)
      if (!typesOnly) specifier = node.moduleSpecifier.text
    } else if (ts.isExportDeclaration(node) && !node.isTypeOnly && node.moduleSpecifier && ts.isStringLiteral(node.moduleSpecifier)) {
      specifier = node.moduleSpecifier.text
    } else if (ts.isCallExpression(node) && node.expression.kind === ts.SyntaxKind.ImportKeyword && ts.isStringLiteral(node.arguments[0])) {
      // Svelte views are lazy callbacks; fixture definitions can load eagerly.
      if (!node.arguments[0].text.endsWith('.svelte')) specifier = node.arguments[0].text
    }
    const target = specifier && resolveImport(specifier, file)
    if (target) targets.push(target)
    ts.forEachChild(node, visit)
  }
  visit(source)
  return targets
}

describe('frontend dependency direction', () => {
  it('only imports across domains along the frozen edges', () => {
    const allowed = new Set(ALLOWED_EDGES)
    const actual = collectEdges()
    const added = [...actual].filter((edge) => !allowed.has(edge)).sort()
    const removed = [...allowed].filter((edge) => !actual.has(edge)).sort()

    expect(
      added,
      'New cross-domain imports are not allowed. Move the shared code into lib/domain or lib/components, ' +
        'or document the edge in ALLOWED_EDGES with a reason.',
    ).toEqual([])
    expect(
      removed,
      'These edges no longer exist; delete them from ALLOWED_EDGES so the lockfile stays accurate.',
    ).toEqual([])
  })

  it('keeps the workspace view layer free of workbench imports', () => {
    const violations = [...collectEdges()].filter((edge) => edge === 'lib/workspace -> lib/workbench')
    expect(violations).toEqual([])
  })

  it('limits speech workbench imports to definition lookup', () => {
    const registry = join(sourceRoot, 'lib/workbench/definitions/registry.ts')
    const violations = sourceFiles(join(sourceRoot, 'lib/speech')).flatMap((file) =>
      runtimeImports(file).filter((target) => domainOf(target) === 'lib/workbench' && target !== registry)
        .map((target) => `${relative(sourceRoot, file)} -> ${relative(sourceRoot, target)}`))
    expect(violations).toEqual([])
  })

  it('keeps registered definitions headless without a reverse speech dependency or import cycle', () => {
    const definitions = join(sourceRoot, 'lib/workbench/definitions')
    const visited = new Set<string>()
    const active = new Set<string>()
    const violations: string[] = []
    function visit(file: string) {
      const label = relative(sourceRoot, file)
      if (active.has(file)) { violations.push(`Import cycle: ${label}`); return }
      if (visited.has(file)) return
      visited.add(file)
      if (file.endsWith('.svelte') || domainOf(file) === 'lib/speech') { violations.push(`UI or speech dependency: ${label}`); return }
      active.add(file)
      runtimeImports(file).forEach(visit)
      active.delete(file)
    }
    sourceFiles(definitions).filter((file) => file.endsWith(`${sep}definition.ts`) || file === join(definitions, 'registry.ts')).forEach(visit)
    expect(violations).toEqual([])
  })
})
