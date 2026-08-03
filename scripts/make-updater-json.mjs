import { readFile, writeFile } from 'node:fs/promises'
import { basename, dirname, join } from 'node:path'

const args = process.argv.slice(2)
const option = (name) => {
  const index = args.indexOf(name)
  return index >= 0 ? args[index + 1] : undefined
}

const installer = option('--exe')
const signaturePath = option('--sig')
const tag = option('--tag')
const notes = option('--notes') ?? '完整变更见 GitHub Release Notes。'
const outputDirectory = option('--out-dir') ?? (installer ? dirname(installer) : undefined)

if (!installer || !signaturePath || !tag || !outputDirectory) {
  throw new Error(
    'Usage: node scripts/make-updater-json.mjs --exe <setup.exe> --sig <setup.exe.sig> --tag vX.Y.Z',
  )
}

const version = tag.replace(/^v/, '')
if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
  throw new Error(`Invalid updater version tag: ${tag}`)
}

const signature = (await readFile(signaturePath, 'utf8')).trim()
if (!signature) throw new Error(`Updater signature is empty: ${signaturePath}`)

const url = `https://github.com/l1veIn/rambledesk/releases/download/${tag}/${basename(installer)}`
const platform = { signature, url }
const manifest = {
  version,
  notes,
  pub_date: new Date().toISOString().replace(/\.\d+Z$/, 'Z'),
  platforms: {
    'windows-x86_64': platform,
    'windows-x86_64-nsis': platform,
  },
}

const output = join(outputDirectory, 'latest.json')
await writeFile(output, `${JSON.stringify(manifest, null, 2)}\n`, 'utf8')
console.log(`Generated ${output}`)
