#!/usr/bin/env node
/**
 * Patch the notes field of a Tauri updater manifest (latest.json) in place.
 *
 * Usage: node scripts/patch-updater-notes.mjs --manifest <latest.json> --notes-file <notes.txt>
 */

import { readFileSync, writeFileSync } from 'node:fs'

const args = process.argv.slice(2)
const option = (name) => {
  const index = args.indexOf(name)
  return index >= 0 ? args[index + 1] : undefined
}

const manifestPath = option('--manifest')
const notesPath = option('--notes-file')
if (!manifestPath || !notesPath) {
  throw new Error(
    'Usage: node scripts/patch-updater-notes.mjs --manifest <latest.json> --notes-file <notes.txt>',
  )
}

const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'))
const notes = readFileSync(notesPath, 'utf8').trim()
if (!notes) throw new Error(`Notes file is empty: ${notesPath}`)
if (!manifest.version) throw new Error(`Manifest has no version field: ${manifestPath}`)

manifest.notes = notes
writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, 'utf8')
console.log(`Patched notes for ${manifest.version} in ${manifestPath}`)
