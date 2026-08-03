import { readdir, readFile } from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'

const MAX_LINES = 500
const SOURCE_ROOTS = ['apps', 'crates']
// These modules predate the size gate. Keep their current ceiling so CI remains
// useful while future refactors shrink them; new modules still use MAX_LINES.
const LEGACY_LINE_LIMITS = new Map([
  ['apps/desktop/src-tauri/src/commands.rs', 541],
  ['crates/rambledesk-core/src/feedback.rs', 597],
  ['crates/rambledesk-speech/src/model.rs', 670],
  ['crates/rambledesk-speech/src/native.rs', 736],
  ['crates/rambledesk-storage/src/sqlite.rs', 554],
  ['crates/rambledesk-storage/src/sqlite/request_ops.rs', 567],
  ['crates/rambledesk-storage/src/sqlite/submission_ops.rs', 562],
  ['crates/rambledesk-storage/src/sqlite/tests/requests.rs', 638],
])

async function rustFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true })
  const files = []

  for (const entry of entries) {
    const target = path.join(directory, entry.name)
    if (entry.isDirectory()) {
      if (entry.name !== 'node_modules' && entry.name !== 'target') {
        files.push(...(await rustFiles(target)))
      }
    } else if (entry.isFile() && entry.name.endsWith('.rs')) {
      files.push(target)
    }
  }

  return files
}

const files = (await Promise.all(SOURCE_ROOTS.map(rustFiles))).flat()
const oversized = []

for (const file of files) {
  const contents = await readFile(file, 'utf8')
  const lines = contents.length === 0 ? 0 : contents.split(/\r?\n/).length
  const display = path.relative(process.cwd(), file).replaceAll('\\', '/')
  const limit = LEGACY_LINE_LIMITS.get(display) ?? MAX_LINES
  if (lines > limit) {
    oversized.push({ file, lines, limit })
  }
}

if (oversized.length > 0) {
  console.error(`Rust modules must stay at or below ${MAX_LINES} lines:`)
  for (const { file, lines, limit } of oversized.sort((left, right) => right.lines - left.lines)) {
    console.error(`  ${lines}/${limit}  ${file}`)
  }
  process.exitCode = 1
} else {
  console.log(`Rust module size check passed (${files.length} files, limit ${MAX_LINES}).`)
}
