import { createHash } from 'node:crypto'
import { gunzipSync } from 'node:zlib'
import { mkdir, readFile, rename, rm, writeFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import process from 'node:process'

const VERSION = '1.13.7'
const ARCHIVE_URL = `https://pub.dev/api/archives/sherpa_onnx_web-${VERSION}.tar.gz`
const ARCHIVE_SHA256 = 'b3b5d54df39e720b626439a8f14ea08ea1bdb5513f64dff2a5c5bb251e6093b7'
const REPOSITORY_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const OUTPUT = path.join(REPOSITORY_ROOT, 'apps/desktop/public/browser-speech/runtime')
const FILES = Object.freeze({
  'sherpa-onnx-asr.js': Object.freeze({
    bytes: 53_867,
    sha256: 'd51ae8e8b756ee5e53423ffada0c9702973f154f561aca7984fe0b12f4060178',
  }),
  'sherpa-onnx-wasm-web.js': Object.freeze({
    bytes: 93_039,
    sha256: '872cced0954291abe919cc75f3a454f2673ef215bdd0496788daa5fac8a3ba47',
  }),
  'sherpa-onnx-wasm-web.wasm': Object.freeze({
    bytes: 14_869_666,
    sha256: 'f0bd7239906d96a5aff87b523879b898c0522d58c2c7ac2f8795b74186dc9c99',
  }),
})

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex')
}

async function installedRuntimeIsValid() {
  try {
    const provenance = JSON.parse(await readFile(path.join(OUTPUT, 'provenance.json'), 'utf8'))
    if (provenance.version !== VERSION || provenance.archiveSha256 !== ARCHIVE_SHA256) return false
    for (const [name, expected] of Object.entries(FILES)) {
      const bytes = await readFile(path.join(OUTPUT, name))
      if (bytes.byteLength !== expected.bytes || sha256(bytes) !== expected.sha256) return false
    }
    return true
  } catch {
    return false
  }
}

function extractTarEntry(tar, wantedName) {
  for (let offset = 0; offset + 512 <= tar.byteLength;) {
    const header = tar.subarray(offset, offset + 512)
    if (header.every((byte) => byte === 0)) break
    const rawName = Buffer.from(header.subarray(0, 100)).toString('utf8').replace(/\0.*$/u, '')
    const rawPrefix = Buffer.from(header.subarray(345, 500)).toString('utf8').replace(/\0.*$/u, '')
    const name = rawPrefix ? `${rawPrefix}/${rawName}` : rawName
    const rawSize = Buffer.from(header.subarray(124, 136)).toString('ascii').replace(/\0.*$/u, '').trim()
    const size = Number.parseInt(rawSize || '0', 8)
    if (!Number.isSafeInteger(size) || size < 0) throw new Error(`Invalid tar size for ${name}`)
    const start = offset + 512
    if (name === `assets/${wantedName}` || name.endsWith(`/assets/${wantedName}`)) {
      return Buffer.from(tar.subarray(start, start + size))
    }
    offset = start + Math.ceil(size / 512) * 512
  }
  throw new Error(`The official package does not contain assets/${wantedName}`)
}

async function main() {
  if (await installedRuntimeIsValid()) return
  const response = await fetch(ARCHIVE_URL, { redirect: 'follow' })
  if (!response.ok) throw new Error(`Could not download sherpa_onnx_web ${VERSION}: HTTP ${response.status}`)
  const archive = Buffer.from(await response.arrayBuffer())
  const archiveDigest = sha256(archive)
  if (archiveDigest !== ARCHIVE_SHA256) {
    throw new Error(`sherpa_onnx_web archive SHA-256 mismatch: ${archiveDigest}`)
  }
  const tar = gunzipSync(archive)
  const staging = `${OUTPUT}.part-${process.pid}`
  await rm(staging, { recursive: true, force: true })
  await mkdir(staging, { recursive: true })
  try {
    for (const [name, expected] of Object.entries(FILES)) {
      const bytes = extractTarEntry(tar, name)
      const digest = sha256(bytes)
      if (bytes.byteLength !== expected.bytes || digest !== expected.sha256) {
        throw new Error(`${name} integrity mismatch: ${bytes.byteLength} bytes, SHA-256 ${digest}`)
      }
      await writeFile(path.join(staging, name), bytes)
    }
    await writeFile(path.join(staging, 'provenance.json'), `${JSON.stringify({
      schemaVersion: 1,
      package: 'sherpa_onnx_web',
      version: VERSION,
      archiveUrl: ARCHIVE_URL,
      archiveSha256: ARCHIVE_SHA256,
      files: FILES,
    }, null, 2)}\n`)
    await rm(OUTPUT, { recursive: true, force: true })
    await mkdir(path.dirname(OUTPUT), { recursive: true })
    await rename(staging, OUTPUT)
  } catch (cause) {
    await rm(staging, { recursive: true, force: true })
    throw cause
  }
}

await main()
