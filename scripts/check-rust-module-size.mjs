import { readdir, readFile } from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'

const MAX_LINES = 800
const SOURCE_ROOTS = ['apps', 'crates']

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
  if (lines > MAX_LINES) {
    oversized.push({ file, lines, limit: MAX_LINES })
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
