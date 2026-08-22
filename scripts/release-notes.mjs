#!/usr/bin/env node
/**
 * Extract the changelog entry for a release tag from docs/CHANGELOG.md.
 *
 * Usage: node scripts/release-notes.mjs --tag vX.Y.Z
 *
 * Prints the entry (plain text) to stdout. When the tag has no entry, prints a
 * generic fallback so the release pipeline never breaks, and warns on stderr.
 */

import { readFileSync } from 'node:fs'

const args = process.argv.slice(2)
const option = (name) => {
  const index = args.indexOf(name)
  return index >= 0 ? args[index + 1] : undefined
}

const tag = option('--tag')
if (!tag) {
  throw new Error('Usage: node scripts/release-notes.mjs --tag vX.Y.Z')
}

const fallback = [
  `What's new in RambleDesk ${tag.replace(/^v/, '')}`,
  '',
  'See the GitHub Release page for the full changelog.',
].join('\n')

const changelog = readFileSync(new URL('../docs/CHANGELOG.md', import.meta.url), 'utf8')
const heading = new RegExp(`^## ${tag.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\s*$`, 'm')
const match = changelog.match(heading)
if (!match) {
  console.error(`warning: docs/CHANGELOG.md has no entry for ${tag}; using a generic fallback`)
  process.stdout.write(`${fallback}\n`)
  process.exit(0)
}

const start = match.index + match[0].length
const after = changelog.slice(start)
const nextHeading = after.match(/^##\s/m)
const entry = (nextHeading ? after.slice(0, nextHeading.index) : after).trim()
if (!entry) {
  console.error(`warning: docs/CHANGELOG.md entry for ${tag} is empty; using a generic fallback`)
  process.stdout.write(`${fallback}\n`)
  process.exit(0)
}
process.stdout.write(`${entry}\n`)
