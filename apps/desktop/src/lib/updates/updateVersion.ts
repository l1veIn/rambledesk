export function parseReleaseVersion(version: string) {
  const trimmed = version.trim().replace(/^v/i, '')
  const [core = '', prerelease = ''] = trimmed.split('-', 2)
  const [major = 0, minor = 0, patch = 0] = core.split('.').map((part) => {
    const value = Number.parseInt(part, 10)
    return Number.isFinite(value) ? value : 0
  })
  return { major, minor, patch, prerelease }
}

export function isNewerReleaseVersion(latest: string, current: string) {
  const next = parseReleaseVersion(latest)
  const installed = parseReleaseVersion(current)
  if (next.major !== installed.major) return next.major > installed.major
  if (next.minor !== installed.minor) return next.minor > installed.minor
  if (next.patch !== installed.patch) return next.patch > installed.patch
  if (!next.prerelease && installed.prerelease) return true
  if (next.prerelease && !installed.prerelease) return false
  return next.prerelease > installed.prerelease
}

export function normalizeUpdateNotes(raw: string, fallback = '') {
  const notes = raw.replace(/\r\n/g, '\n').trim()
  if (!notes) return fallback
  return notes.length > 8_000 ? `${notes.slice(0, 8_000).trimEnd()}\n…` : notes
}
