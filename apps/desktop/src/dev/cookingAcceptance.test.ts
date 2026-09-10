/** Explicit live acceptance only: uses an existing config and a disposable HTTP fixture. */
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFileSync, writeFileSync } from 'node:fs'
import { expect, it, vi } from 'vitest'
import { get } from 'svelte/store'
import { type CookingConfig } from '../lib/cooking'
import { snapshotFeedbackDraftDocument } from '../lib/feedbackDraftDocument'
import { TestApplicationTransport } from '../lib/application/testApplicationTransport'
import { createWorkspaceSession } from '../lib/workbench/workspaceSession'
import { createDraftSession } from '../lib/workbench/draftSession'
import { createDraftController } from '../lib/workbench/draftController'
import { createCookingSession } from '../lib/workbench/cookingSession'
import { createCookingController } from '../lib/workbench/cookingController'
import { createPublisherController } from '../lib/workbench/publisherController'

const configPath = process.env.RAMBLEDESK_COOKING_ACCEPTANCE_CONFIG
const fixturePath = process.env.RAMBLEDESK_COOKING_ACCEPTANCE_FIXTURE
const outputPath = process.env.RAMBLEDESK_COOKING_ACCEPTANCE_OUTPUT
it.skipIf(!configPath || !fixturePath || !outputPath)('cooks with an existing model, then publishes the preserved source and variant through HTTP', async () => {
  // Capture SQLite output inside the process; never print or persist credentials.
  const config = JSON.parse(execFileSync('python3', ['-c', `
import json,sqlite3,sys
from pathlib import Path
c=sqlite3.connect(Path(sys.argv[1]).resolve().as_uri()+'?mode=ro', uri=True)
p={k:v.decode('utf-16le') if isinstance(v,bytes) else v for k,v in c.execute("SELECT key,value FROM ItemTable WHERE key LIKE 'rambledesk.cooking.%'")}
g=lambda k:p.get('rambledesk.cooking.'+k,'')
print(json.dumps(dict(provider=g('provider'),apiKey=g('api-key'),baseUrl=g('base-url'),model=g('model'),reasoningEffort=g('reasoning-effort'),systemPrompt=g('system-prompt'),locale='zh-CN')))
`, configPath!], { encoding: 'utf8' })) as CookingConfig
  const fixture = JSON.parse(readFileSync(fixturePath!, 'utf8'))
  expect(fixture.fixture).toBe('rambledesk-feedback-acceptance-v1')
  expect(new URL(fixture.url).hostname).toBe('127.0.0.1')
  let token = readFileSync(fixture.tokenFile, 'utf8')
  let generation: string | null = null
  async function post(path: string, input?: unknown): Promise<any> {
    const response = await fetch(`${fixture.url}${path}`, {
      method: 'POST', headers: { Origin: fixture.url, Authorization: `Bearer ${token}`,
        'Content-Type': 'application/json', ...(generation ? { 'X-RambleDesk-Runtime-Generation': generation } : {}) },
      body: input === undefined ? undefined : JSON.stringify(input),
    })
    if (!response.ok) throw new Error(`Fixture HTTP ${response.status}`)
    generation = response.headers.get('X-RambleDesk-Runtime-Generation') ?? generation
    return response.json()
  }
  token = (await post('/api/auth/session')).session_token
  const transport = new TestApplicationTransport()
  // Only the test-host transport adapter is replaced; every command crosses real HTTP.
  for (const name of ['getFeedbackWorkspace', 'saveFeedbackDraft', 'submitFeedback', 'readPublishedFeedback'] as const) {
    transport.handle(name, (input) => post(`/api/application/${name}`, input))
  }
  const requestId = fixture.requests.find((request: { purpose: string }) => request.purpose === 'ordinary').requestId
  const workspace = await transport.call('getFeedbackWorkspace', { request_id: requestId })
  const session = createWorkspaceSession(); session.open(workspace)
  const draft = createDraftSession(); draft.adopt(workspace.draft)
  const cooking = createCookingSession()
  const drafts = createDraftController({ transport, session: draft, messageFrom: () => 'Save failed',
    isInteractionLocked: () => get(session).interactionLocked, isWorkspaceTerminal: () => get(session).terminal,
    getWorkspace: () => get(session).workspace, setWorkspaceDraft: session.setDraft })
  const original = '按钮点击后大约等了两秒才有提示。我不确定是不是网络问题，需要保留这个疑问。建议在等待时显示保存中，失败后保留输入并允许重试。'
  const source = snapshotFeedbackDraftDocument({ type: 'doc', content: [{ type: 'paragraph', content: [{ type: 'text', text: original }] }] })
  drafts.updateDraft(source)
  const errors: string[] = []
  const setPageError = (message: string) => { if (message) errors.push(message) }
  vi.stubGlobal('window', {}) // Select the real browser fetch branch; no Tauri test host.
  try {
    const controller = createCookingController({ tr: (s) => s, messageFrom: () => 'Cooking failed (credential-safe)',
      getWorkspace: () => get(session).workspace, getDraftBody: () => get(draft).body,
      getCookingConfig: () => config, isCookingEnabled: () => true, isCooking: () => cooking.isCooking(requestId),
      prepareFeedback: async () => ({ kind: 'ready' }), saveDraftNow: drafts.saveDraftNow,
      setPageError, setCooking: cooking.setCooking, setPreview: cooking.setPreview })
    const started = performance.now()
    await controller.cookPreviewOnly()
    const elapsedMs = Math.round(performance.now() - started)
    expect(errors).toEqual([])
    const preview = cooking.preview()!
    expect(preview).toBeTruthy()
    expect(preview.original).toBe(original)
    expect(preview.markdown.length).toBeGreaterThan(0)
    expect(preview.savedRevision).toBe(get(draft).savedRevision)
    expect(get(draft).body).toBe(original)
    const publisher = createPublisherController({ transport, session, draft, cooking, tr: (s) => s,
      messageFrom: () => 'Publish failed', setPageError, isReadOnly: () => false,
      prepareFeedback: async () => ({ kind: 'ready' }), saveDraftNow: drafts.saveDraftNow,
      getCookingEnabled: () => true, cookSubmission: controller.cookSubmission,
      refreshNavigation: async () => {}, showSubmittedToast: () => {} })
    await publisher.submitFeedback()
    expect(errors).toEqual([])
    const published = await transport.call('readPublishedFeedback', { request_id: requestId })
    if (!published) throw new Error('Published feedback is missing')
    expect(published.markdown).toBe(preview.markdown.trimEnd() + '\n')
    expect(published.uncooked_markdown).toBe(original + '\n')
    expect(published.manifest.cooking_model).toBe(preview.model)
    const after = await transport.call('getFeedbackWorkspace', { request_id: requestId })
    expect(after.draft.document_json).toBe(get(draft).documentJson)
    expect(after.draft.body_markdown).toBe(original)
    const sha256 = (text: string) => createHash('sha256').update(text).digest('hex')
    writeFileSync(outputPath!, JSON.stringify({ at: new Date().toISOString(), method: 'Real Cooking controller + Publisher; real fetch model + HTTP/SQLite fixture; Node host, no visual/native claim', provider: config.provider, baseUrl: config.baseUrl, model: preview.model, reasoning: config.reasoningEffort, elapsedMs, requestId, savedRevision: preview.savedRevision, original, cooked: preview.markdown, packageSourceSha256: sha256(published.uncooked_markdown!), packageCookedSha256: sha256(published.markdown), assertions: 'canonical structured draft preserved; saved source matches preview provenance; published source, variant and model match', result: 'passed' }, null, 2) + '\n')
  } finally { drafts.cancelPendingSave(); vi.unstubAllGlobals(); config.apiKey = '' }
}, 180_000)
