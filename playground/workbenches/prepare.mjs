// Local fixture preparation only. Never contacts RambleDesk or submits feedback.
import { existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs'
import { randomUUID } from 'node:crypto'
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = dirname(fileURLToPath(import.meta.url))
const cases = [
  ['01-ramble.json', 'ramble'],
  ['02-questions.json', 'questions'],
  ['03-single-choice.json', 'single_choice'],
  ['04-document-review.json', 'document_review'],
]
const readJson = (path) => JSON.parse(readFileSync(path, 'utf8').replace(/^\uFEFF/, ''))
const writeJson = (path, value) => writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, 'utf8')
function insideRoot(path) {
  const local = relative(root, path)
  if (!local || local === '..' || local.startsWith(`..${sep}`) || isAbsolute(local)) throw new Error('Path must stay inside the playground')
  return path
}

function loadCases() {
  return cases.map(([file, type]) => {
    const input = readJson(join(root, 'fixtures', file))
    if (input.workbench?.type !== type || input.workbench.version !== 1 || !input.workbench.data) throw new Error(`${file}: unexpected workbench contract`)
    if (!input.what_happened || [...input.what_happened].length > 200) throw new Error(`${file}: invalid summary length`)
    if (input.request_id || input.host_id || input.host_session_id || input.actions || input.allow_finish || input.final_summary) throw new Error(`${file}: scenario must not contain session identity or completion fields`)
    input.attachments = input.attachments.map((attachment) => ({
      ...attachment,
      path: insideRoot(realpathSync(resolve(root, attachment.path))),
    }))
    return { file, type, input }
  })
}

try {
  const command = process.argv[2] ?? 'check'
  if (!['check', 'new'].includes(command) || process.argv.length > 3) throw new Error('Usage: node prepare.mjs [check|new]')
  const loaded = loadCases()
  if (command === 'check') {
    console.log(JSON.stringify({ fixtures: loaded.map(({ file, type }) => ({ file, type, version: 1 })), attachments: 'all present inside the playground', submitted: false }, null, 2))
  } else {
    const runs = join(root, '.runs')
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
    const stages = loaded.map(({ file, type, input }) => {
      const requestId = randomUUID()
      writeJson(join(directory, file), { ...input, request_id: requestId })
      return { file, type, version: 1, request_id: requestId, status: 'prepared', observations: [], unverified: [] }
    })
    writeJson(join(directory, 'run.json'), { run_id: runId, created_at: new Date().toISOString(), status: 'running', stages, report_request_id: randomUUID(), followups: [] })
    writeJson(latestPath, { run_id: runId })
    console.log(JSON.stringify({ directory, run_id: runId, submitted: false, next: 'Send only 01-ramble.json through the current managed session, then hand off.' }, null, 2))
  }
} catch (error) {
  console.error(error.message)
  process.exitCode = 1
}
