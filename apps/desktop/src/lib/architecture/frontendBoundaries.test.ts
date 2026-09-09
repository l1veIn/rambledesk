import { existsSync, readdirSync, readFileSync } from 'node:fs'
import { dirname, join, normalize, relative, sep } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'

/**
 * Frontend dependency direction.
 *
 * The list below is a lockfile of the cross-domain imports that exist today. It
 * may only shrink: adding an import that crosses domains fails this test, and an
 * entry whose last import disappears must be deleted. `(app)` and `dev` are
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
 */
const ALLOWED_EDGES: readonly string[] = [
  // lib root modules (cross-cutting helpers and contract barrels)
  'lib/(root) -> lib/application',
  'lib/(root) -> lib/components',
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
  // updates
  'lib/updates -> lib/(root)',
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
})
