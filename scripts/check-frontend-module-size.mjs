import { readdir, readFile } from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'

const MAX_LINES = 700
const SOURCE_ROOT = 'apps/desktop/src'
const EXTENSIONS = new Set(['.svelte', '.ts'])

/**
 * Files that predate the limit. Each entry is the highest line count it may keep;
 * the number may only go down as the file is split, and the entry is deleted once
 * the file fits under `MAX_LINES`.
 */
const EXEMPTIONS = new Map([
  ['apps/desktop/src/App.svelte', 2300],
  ['apps/desktop/src/ScreenshotOverlay.svelte', 1046],
  ['apps/desktop/src/lib/workspace/ArchivedSessionsWorkspaceView.svelte', 721],
  ['apps/desktop/src/lib/workbench/RambleSessionController.svelte', 706],
  ['apps/desktop/src/lib/workbench/navigationController.test.ts', 897],
  ['apps/desktop/src/lib/workbench/attachmentController.test.ts', 832],
  ['apps/desktop/src/lib/agents/draftManagedSessionController.test.ts', 718],
])

/** Data files whose length is content, not logic. */
const ALLOWED = new Set([
  'apps/desktop/src/lib/i18n.ts',
])

async function sourceFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true })
  const files = []
  for (const entry of entries) {
    const target = path.join(directory, entry.name)
    if (entry.isDirectory()) {
      if (entry.name !== 'node_modules' && entry.name !== 'dist') {
        files.push(...(await sourceFiles(target)))
      }
    } else if (entry.isFile() && EXTENSIONS.has(path.extname(entry.name))) {
      files.push(target)
    }
  }
  return files
}

const violations = []
const files = await sourceFiles(SOURCE_ROOT)
for (const file of files) {
  const contents = await readFile(file, 'utf8')
  const lines = contents.length === 0 ? 0 : contents.split(/\r?\n/).length
  const display = path.relative(process.cwd(), file).replaceAll('\\', '/')
  if (ALLOWED.has(display)) continue
  const exempt = EXEMPTIONS.get(display)
  if (exempt !== undefined) {
    if (lines > exempt) violations.push(`${lines}/${exempt} (exempted, may only shrink)  ${display}`)
    continue
  }
  if (lines > MAX_LINES) violations.push(`${lines}/${MAX_LINES}  ${display}`)
}

if (violations.length > 0) {
  console.error(`Frontend modules must stay at or below ${MAX_LINES} lines:`)
  for (const violation of violations.sort()) console.error(`  ${violation}`)
  process.exitCode = 1
} else {
  const exemptCount = [...EXEMPTIONS.keys()].filter((file) => files.some((candidate) => path.relative(process.cwd(), candidate).replaceAll('\\', '/') === file)).length
  console.log(`Frontend module size check passed (${files.length} files, limit ${MAX_LINES}, ${exemptCount} exemptions).`)
}
