import { spawnSync } from 'node:child_process'
import { closeSync, mkdirSync, openSync, readFileSync } from 'node:fs'
import { basename, join } from 'node:path'

// GET /repos/{owner}/{repo}/releases/tags/{tag} omits drafts.
// Resolve the release id from the list API, then upload/download by id.

const args = process.argv.slice(2)
const command = args[0]
const option = (name) => {
  const index = args.indexOf(name)
  return index >= 0 ? args[index + 1] : undefined
}
const optionAll = (name) => {
  const values = []
  for (let i = 0; i < args.length; i++) {
    if (args[i] === name && args[i + 1]) values.push(args[i + 1])
  }
  return values
}

const repo = process.env.GH_REPO || process.env.GITHUB_REPOSITORY
if (!repo) {
  throw new Error('Set GH_REPO or GITHUB_REPOSITORY (owner/name)')
}

const tag = option('--tag')
if (!tag) {
  throw new Error(
    'Usage: node scripts/gh-release-assets.mjs <resolve|upload|download> --tag vX.Y.Z ...',
  )
}

const runGh = (ghArgs, options = {}) => {
  const result = spawnSync('gh', ghArgs, {
    encoding: options.encoding ?? 'utf8',
    stdio: options.stdio ?? ['ignore', 'pipe', 'pipe'],
    windowsHide: true,
  })
  if (result.status !== 0) {
    const stderr = result.stderr?.toString?.() ?? result.stderr ?? ''
    const stdout = result.stdout?.toString?.() ?? result.stdout ?? ''
    throw new Error(`gh ${ghArgs.join(' ')} failed:\n${stderr || stdout}`)
  }
  return result
}

const globToRegExp = (pattern) => {
  const escaped = pattern.replace(/[.+^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '.*').replace(/\?/g, '.')
  return new RegExp(`^${escaped}$`)
}

const resolveRelease = () => {
  const result = runGh([
    'api',
    '--paginate',
    `repos/${repo}/releases`,
    '--jq',
    '.[] | {id, tag_name, draft}',
  ])
  const lines = result.stdout
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
  for (const line of lines) {
    const release = JSON.parse(line)
    if (release.tag_name === tag) return release
  }
  return null
}

const requireRelease = () => {
  const release = resolveRelease()
  if (!release) {
    throw new Error(`No GitHub Release (including drafts) found for ${tag}`)
  }
  return release
}

const listAssets = (releaseId) => {
  const result = runGh([
    'api',
    '--paginate',
    `repos/${repo}/releases/${releaseId}/assets`,
    '--jq',
    '.[] | {id, name}',
  ])
  return result.stdout
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => JSON.parse(line))
}

const authToken = () =>
  process.env.GH_TOKEN ||
  process.env.GITHUB_TOKEN ||
  runGh(['auth', 'token']).stdout.trim()

const deleteAsset = (assetId) => {
  runGh(['api', '--method', 'DELETE', `repos/${repo}/releases/assets/${assetId}`])
}

const uploadFile = async (releaseId, filePath) => {
  const name = basename(filePath)
  const existing = listAssets(releaseId).find((asset) => asset.name === name)
  if (existing) {
    console.log(`Replacing existing asset ${name} (${existing.id})`)
    deleteAsset(existing.id)
  }
  // Asset uploads go to uploads.github.com, not api.github.com.
  const response = await fetch(
    `https://uploads.github.com/repos/${repo}/releases/${releaseId}/assets?name=${encodeURIComponent(name)}`,
    {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${authToken()}`,
        Accept: 'application/vnd.github+json',
        'Content-Type': 'application/octet-stream',
        'X-GitHub-Api-Version': '2022-11-28',
      },
      body: readFileSync(filePath),
    },
  )
  if (!response.ok) {
    const detail = await response.text()
    throw new Error(`Upload ${name} failed (${response.status}): ${detail}`)
  }
  console.log(`Uploaded ${name}`)
}

if (command === 'resolve') {
  const release = requireRelease()
  console.log(String(release.id))
  process.exit(0)
}

if (command === 'upload') {
  const dash = args.indexOf('--')
  const files = dash >= 0 ? args.slice(dash + 1) : args.slice(args.indexOf('--tag') + 2)
  if (files.length === 0) {
    throw new Error('Usage: node scripts/gh-release-assets.mjs upload --tag vX.Y.Z -- <files...>')
  }
  const release = requireRelease()
  for (const file of files) await uploadFile(release.id, file)
  process.exit(0)
}

if (command === 'download') {
  const directory = option('--dir')
  const patterns = optionAll('--pattern')
  if (!directory || patterns.length === 0) {
    throw new Error(
      'Usage: node scripts/gh-release-assets.mjs download --tag vX.Y.Z --dir <dir> --pattern <glob> [--pattern <glob>]',
    )
  }
  mkdirSync(directory, { recursive: true })
  const release = requireRelease()
  const matchers = patterns.map(globToRegExp)
  const assets = listAssets(release.id).filter((asset) =>
    matchers.some((matcher) => matcher.test(asset.name)),
  )
  if (assets.length === 0) {
    throw new Error(`No assets matched ${patterns.join(', ')} on ${tag}`)
  }
  for (const asset of assets) {
    const output = join(directory, asset.name)
    const fd = openSync(output, 'w')
    try {
      runGh(
        [
          'api',
          '-H',
          'Accept: application/octet-stream',
          `repos/${repo}/releases/assets/${asset.id}`,
        ],
        { stdio: ['ignore', fd, 'pipe'] },
      )
    } finally {
      closeSync(fd)
    }
    console.log(`Downloaded ${asset.name}`)
  }
  process.exit(0)
}

throw new Error(`Unknown command: ${command}`)
