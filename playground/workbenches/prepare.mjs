// Local fixture preparation only. Never contacts RambleDesk or submits feedback.
import { existsSync, mkdirSync, readFileSync, readdirSync, realpathSync, statSync, writeFileSync } from 'node:fs'
import { randomUUID } from 'node:crypto'
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = dirname(fileURLToPath(import.meta.url))
const readJson = (path) => JSON.parse(readFileSync(path, 'utf8').replace(/^\uFEFF/, ''))
const writeJson = (path, value) => writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, 'utf8')
function insideRoot(base, path) {
  const local = relative(base, path)
  if (!local || local === '..' || local.startsWith(`..${sep}`) || isAbsolute(local)) throw new Error('Path must stay inside the playground')
  return path
}

function prepareAttachment(base, attachment, field) {
  const fileName = typeof attachment?.file_name === 'string'
    ? attachment.file_name.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, '') : ''
  if (!fileName || Buffer.byteLength(fileName, 'utf8') > 255 || /[/\\\0]/.test(fileName) || ['.', '..'].includes(fileName)) {
    throw new Error(`${field}.file_name: must be a plain file name of 1–255 UTF-8 bytes`)
  }
  if (typeof attachment.path !== 'string' || !attachment.path.trim()) {
    throw new Error(`${field}.path: must name a material inside the playground`)
  }
  if (attachment.markdown != null || attachment.contents_base64 != null) {
    throw new Error(`${field}.path: must be the attachment's only content source`)
  }
  let path
  try {
    path = insideRoot(base, realpathSync(resolve(base, attachment.path)))
    if (!statSync(path).isFile()) throw new Error('Path must be a regular file')
  } catch (error) {
    throw new Error(`${field}.path: ${error.message}`)
  }
  return { ...attachment, file_name: fileName, path }
}

/** New ordinary workbenches add their own JSON fixture; no central type list. */
export function loadCases(base = root, development = false) {
  // Compare canonical paths on both sides (macOS temporary roots can use /var aliases).
  base = realpathSync(base)
  const fixtures = join(base, 'fixtures')
  const files = readdirSync(fixtures).filter((name) => name.endsWith('.json')).sort()
    .map((name) => ({ name, path: join(fixtures, name) }))
  const devFixtures = join(fixtures, 'development')
  if (development && existsSync(devFixtures)) {
    files.push(...readdirSync(devFixtures).filter((name) => name.endsWith('.json')).sort()
      .map((name) => ({ name: `development-${name}`, path: join(devFixtures, name) })))
  }
  return files.map(({ name: file, path }) => {
    const input = readJson(path)
    const { type, version, data } = input.workbench ?? {}
    if (typeof type !== 'string' || !/^[a-z][a-z0-9_]*$/.test(type) || !Number.isSafeInteger(version) || version < 1 || !data || typeof data !== 'object' || Array.isArray(data)) throw new Error(`${file}: unexpected workbench contract`)
    if (typeof input.what_happened !== 'string' || !input.what_happened.trim() || [...input.what_happened].length > 200) throw new Error(`${file}: invalid summary length`)
    if (['request_id', 'host_id', 'host_session_id', 'actions', 'allow_finish', 'final_summary'].some((key) => key in input)) throw new Error(`${file}: scenario must not contain session identity or completion fields`)
    // Runtime workbenches may prepare their own resources; ordinary types need no branch.
    if (type === 'terminal') input.workbench.data.cwd = realpathSync(base)
    if (!Array.isArray(input.attachments ?? [])) throw new Error(`${file}: attachments must be an array`)
    input.attachments = (input.attachments ?? []).map((attachment, index) =>
      prepareAttachment(base, attachment, `${file}: attachments[${index}]`))
    return { file, type, version, input }
  })
}

export function prepare({ command = 'check', mode = 'all', development = false, base = root } = {}) {
  base = realpathSync(base)
  if (!['check', 'new'].includes(command) || !/^[a-z][a-z0-9_]*$/.test(mode)) throw new Error('Usage: node prepare.mjs [check|new] [type|all] [--development]')
  const loaded = loadCases(base, development).filter((item) => mode === 'all' || item.type === mode)
  if (!loaded.length) throw new Error(`No fixtures for ${mode}${development ? '' : '; development fixtures require --development'}`)
  if (command === 'check') {
    return { fixtures: loaded.map(({ file, type, version }) => ({ file, type, version })), attachments: 'all present inside the playground', submitted: false }
  }
  const runs = join(base, '.runs')
  const latestPath = join(runs, 'latest.json')
  if (existsSync(latestPath)) {
    const latest = readJson(latestPath)
    if (typeof latest.run_id !== 'string' || !/^[a-zA-Z0-9-]+$/.test(latest.run_id)) throw new Error('Invalid latest run id')
    const previous = readJson(join(runs, latest.run_id, 'run.json'))
    if (!['completed', 'cancelled'].includes(previous.status)) throw new Error(`Resume ${latest.run_id} first; reconcile its real request status before starting another run.`)
  }
  const runId = `${new Date().toISOString().replace(/[^0-9TZ]/g, '')}-${randomUUID().slice(0, 8)}`
  const directory = join(runs, runId)
  mkdirSync(directory, { recursive: true })
  const stages = loaded.map(({ file, type, version, input }, index) => {
    const requestId = randomUUID()
    const label = input.title.replace(/^\d+\/\d+\s*·\s*/, '')
    const title = mode === 'all' ? `${index + 1}/${loaded.length} · ${label}` : `${label} · 独立体验`
    writeJson(join(directory, file), { ...input, title, request_id: requestId })
    return { file, type, version, request_id: requestId, status: 'prepared', observations: [], unverified: [] }
  })
  writeJson(join(directory, 'run.json'), { run_id: runId, created_at: new Date().toISOString(), mode, development, status: 'running', stages, report_request_id: randomUUID(), followups: [] })
  writeJson(latestPath, { run_id: runId })
  return { directory, run_id: runId, submitted: false, next: `Send only ${stages[0].file} through the current managed session, then hand off.` }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const args = process.argv.slice(2)
    const development = args.includes('--development')
    const positional = args.filter((arg) => arg !== '--development')
    if (positional.length > 2 || args.filter((arg) => arg === '--development').length > 1) throw new Error('Usage: node prepare.mjs [check|new] [type|all] [--development]')
    console.log(JSON.stringify(prepare({ command: positional[0], mode: positional[1], development }), null, 2))
  } catch (error) {
    console.error(error.message)
    process.exitCode = 1
  }
}
