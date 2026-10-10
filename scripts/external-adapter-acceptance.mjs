#!/usr/bin/env node
// Run against scripts/feedback-acceptance.py's disposable fixture only.
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { createHash, randomUUID } from 'node:crypto';
import { setTimeout as delay } from 'node:timers/promises';
const manifestPath = process.argv[2];
let manifest;
let localToken;
let durableToken;
let browserToken;
let generation;
let piHealth;
let registerRambleDeskPiTools;
let dshHealth;
let registerRambleDshTools;
let dshPlugin;
let dshHost;
let stage = 'manifest';
const report = { scenarios: [] };

async function webPost(route, input, token = browserToken) {
  const response = await fetch(manifest.url + route, {
    method: 'POST',
    headers: {
      Origin: manifest.url,
      Authorization: `Bearer ${token}`,
      'Content-Type': 'application/json',
      ...(generation ? { 'X-RambleDesk-Runtime-Generation': generation } : {}),
    },
    body: input === undefined ? undefined : JSON.stringify(input),
    signal: AbortSignal.timeout(10_000),
  });
  generation = response.headers.get('X-RambleDesk-Runtime-Generation') ?? generation;
  const body = await response.json();
  assert.ok(response.ok, `${route}: ${response.status} ${body.code ?? ''}`);
  return body;
}

async function application(operation, input) {
  if (operation === 'cancelFeedbackRequest') {
    input = { reason: 'Isolated acceptance cancellation', ...input };
  }
  return webPost(`/api/application/${operation}`, input);
}

async function submit(requestId, text, workbenchState) {
  const workspace = await application('getFeedbackWorkspace', { request_id: requestId });
  const saved = await application('saveFeedbackDraft', {
    request_id: requestId,
    expected_revision: workspace.draft.saved_revision,
    body_markdown: text,
    document_json: JSON.stringify({
      schemaVersion: 2,
      doc: { type: 'doc', content: [{ type: 'paragraph', content: [{ type: 'text', text }] }] },
      ...(workbenchState ? { workbenchState } : {}),
    }),
  });
  return application('submitFeedback', { request_id: requestId, expected_revision: saved.saved_revision });
}

function request(title, extras = {}) {
  return {
    title,
    what_happened: 'Isolated external adapter acceptance: inspect the request and return feedback.',
    actions: [{ id: 'review', instruction: 'Review the request and submit your findings.' }],
    ...extras,
  };
}

async function adapter(host, entries = []) {
  const tools = new Map();
  const sessionId = `adapter-acceptance-${host}`;
  let context;
  if (host === 'pi') {
    registerRambleDeskPiTools({
      registerTool: tool => tools.set(tool.name, tool),
      appendEntry: (customType, data) => entries.push({ type: 'custom', customType, data }),
    });
  } else if (dshHost) {
    context = new dshHost.Context();
    new dshHost.SystemPrompt(context, {});
    new dshHost.ToolRuntime(context, {});
    new dshHost.CommandRuntime(context);
    await context.plugin(dshPlugin, { stateFile: path.join(manifest.directory, 'dsh-adapter-state.json') });
    assert.equal(context.tools.schemas().length, 4);
    assert.deepEqual(context.commands.list().map(command => command.name), ['ramble_off', 'ramble_on']);
    assert.equal((await context.systemPrompt.assemble()).tools.length, 4);
  } else {
    registerRambleDshTools({ register: tool => tools.set(tool.name, tool) }, {
      stateFile: path.join(manifest.directory, 'dsh-adapter-state.json'),
      env: {
        RAMBLEDESK_LOCAL_API_URL: manifest.localApiUrl,
        RAMBLEDESK_LOCAL_SERVER_TOKEN_FILE: manifest.localTokenFile,
      },
    });
  }
  return {
    entries,
    async call(name, params, signal) {
      if (context) {
        const result = await context.tools.execute({
          callId: randomUUID(), name, arguments: params, signal: signal ?? AbortSignal.timeout(10_000), agent: { id: sessionId },
        });
        if (result.isError) throw new Error(result.error?.message ?? JSON.stringify(result.error));
        return { text: result.content.map(item => item.text ?? '').join('\n'), details: result.value.details };
      }
      const tool = tools.get(name);
      assert.ok(tool, `${host} missing ${name}`);
      const result = host === 'pi'
        ? await tool.execute(randomUUID(), params, signal, undefined, {
          cwd: manifest.directory, sessionManager: { getSessionId: () => sessionId, getEntries: () => entries },
        })
        : await tool.execute(params, { signal, cwd: manifest.directory, agent: { id: sessionId } });
      return { text: host === 'pi' ? result.content.map(item => item.text ?? '').join('\n') : result.text, details: result.details };
    },
  };
}

async function pending(promise) {
  // A bounded test observation; adapters themselves never poll waiting requests.
  assert.equal(await Promise.race([promise.then(() => 'finished'), delay(50, 'waiting')]), 'waiting');
}

async function checkNative(host) {
  let runtime = await adapter(host);
  const evidence = { adapter: host, checks: [] };
  report.scenarios.push(evidence);
  if (host === 'dsh' && dshHost) evidence.checks.push('real Cordis plugin load, four tool registrations, two commands, system prompt assembly and ToolRuntime output validation');
  const requestId = randomUUID();
  const artifact = path.join(manifest.directory, `${host}-review.md`);
  await writeFile(artifact, '# Isolated adapter review\n\n中文 and English preserved.\n');
  const input = request(`${host} real request/wait`, {
    request_id: requestId, attachments: [{ file_name: `${host}-review.md`, path: artifact }],
  });
  const created = await runtime.call('request_ramble_feedback', { ...input, wait: false });
  assert.equal(created.details.request_id, requestId);
  assert.equal(created.details.status, 'waiting');
  assert.equal(created.details.host_id, host);
  const workspace = await application('getFeedbackWorkspace', { request_id: requestId });
  assert.equal(workspace.request_attachments.length, 1);
  evidence.checks.push('create with local file attachment and server-owned host identity');

  const waiting = runtime.call('request_ramble_feedback', input);
  await pending(waiting);
  const text = `${host} real submitted feedback: 中文 and English preserved.`;
  await submit(requestId, text);
  const completed = await waiting;
  assert.equal(completed.details.status, 'completed');
  assert.ok(completed.text.includes(text));
  assert.equal(completed.details.feedback_package.manifest.request_id, requestId);
  evidence.checks.push('native blocking wait releases published feedback in model-visible content');
  for (const name of ['get_ramble_feedback', 'request_ramble_feedback']) {
    const result = await runtime.call(name, name === 'get_ramble_feedback' ? { request_id: requestId } : input);
    assert.equal(result.details.status, 'completed');
    assert.ok(result.text.includes(text));
  }
  evidence.checks.push('get and identical retry recover the same completed package');

  const interruptedId = randomUUID();
  const interruptedInput = request(`${host} interrupted request`, { request_id: interruptedId });
  await runtime.call('request_ramble_feedback', { ...interruptedInput, wait: false });
  const cancellation = new AbortController();
  const interrupted = runtime.call('request_ramble_feedback', interruptedInput, cancellation.signal);
  const rejected = assert.rejects(interrupted);
  await pending(interrupted.catch(() => undefined));
  cancellation.abort();
  await rejected;
  runtime = await adapter(host, runtime.entries);
  const resumed = runtime.call('resume_ramble_feedback', {});
  await pending(resumed);
  await application('cancelFeedbackRequest', { request_id: interruptedId, reason: 'Isolated acceptance cancellation' });
  assert.equal((await resumed).details.status, 'cancelled');
  evidence.checks.push('interrupted wait survives adapter reload and resumes persisted request');

  const next = await runtime.call('request_ramble_feedback', request(`${host} new request`, { wait: false }));
  assert.notEqual(next.details.request_id, requestId);
  assert.notEqual(next.details.request_id, interruptedId);
  if (host === 'dsh') {
    assert.equal((await runtime.call('cancel_ramble_feedback', { request_id: next.details.request_id })).details.status, 'cancelled');
  } else {
    await application('cancelFeedbackRequest', { request_id: next.details.request_id });
    assert.equal((await runtime.call('get_ramble_feedback', { request_id: next.details.request_id })).details.status, 'cancelled');
  }
  evidence.checks.push('new request uses a fresh id; cancellation is terminal');
  await assert.rejects(runtime.call('request_ramble_feedback', request(`${host} invalid summary`, {
    what_happened: 'x'.repeat(201), wait: false,
  })), /INVALID_ARGUMENT/);
  const corrected = await runtime.call('request_ramble_feedback', request(`${host} corrected summary`, { wait: false }));
  assert.equal(corrected.details.status, 'waiting');
  await application('cancelFeedbackRequest', { request_id: corrected.details.request_id });
  evidence.checks.push('201-character summary fails explicitly and corrected input succeeds');
}

let rpcId = 0;
async function mcp(method, params) {
  const response = await fetch(manifest.localMcpUrl, {
    method: 'POST',
    headers: { Authorization: `Bearer ${localToken}`, 'Content-Type': 'application/json', Accept: 'application/json, text/event-stream' },
    body: JSON.stringify({ jsonrpc: '2.0', id: ++rpcId, method, params }),
    signal: AbortSignal.timeout(10_000),
  });
  assert.ok(response.ok, `MCP ${method}: HTTP ${response.status}`);
  const body = await response.text();
  const result = JSON.parse(body.split('\n').find(line => line.startsWith('data:'))?.slice(5).trim() ?? body);
  assert.equal(result.error, undefined, `MCP ${method}: ${JSON.stringify(result.error)}`);
  return result.result;
}

async function checkMcp() {
  await mcp('initialize', { protocolVersion: '2025-03-26', capabilities: {}, clientInfo: { name: 'external-adapter-acceptance', version: '1' } });
  const listed = await mcp('tools/list', {});
  for (const name of ['request_feedback', 'get_feedback', 'cancel_feedback', 'list_workbenches', 'describe_workbench']) {
    assert.ok(listed.tools.some(tool => tool.name === name), `MCP missing ${name}`);
  }
  const call = async (name, args) => {
    const result = await mcp('tools/call', { name, arguments: args });
    assert.notEqual(result.isError, true, `${name}: ${JSON.stringify(result.content)}`);
    return result;
  };
  const requestId = randomUUID();
  await call('request_feedback', request('Generic MCP legacy request', {
    request_id: requestId, host_id: 'generic', host_session_id: 'adapter-acceptance-generic',
  }));
  assert.equal((await call('get_feedback', { request_id: requestId })).structuredContent.status, 'waiting');
  const text = 'Generic MCP real submitted feedback.';
  await submit(requestId, text);
  const completed = await call('get_feedback', { request_id: requestId });
  assert.equal(completed.structuredContent.status, 'completed');
  assert.ok(completed.structuredContent.feedback_package.markdown.includes(text));
  const checks = ['initialize and discover five public tools with older protocol version', 'legacy request survives independent HTTP RPC calls', 'get returns completed published package'];
  const description = (await call('describe_workbench', { type: 'questions' })).structuredContent;
  const questionId = randomUUID();
  await call('request_feedback', {
    request_id: questionId, host_id: 'generic', host_session_id: 'adapter-acceptance-generic',
    title: 'Generic MCP typed questions', what_happened: 'Choose the target audience.', workbench: description.example,
  });
  const question = description.example.data.questions[0];
  const option = question.options[0];
  await submit(questionId, '', {
    type: 'questions', answers: [{ id: question.id, value: option.value, label: option.label, wasCustom: false, index: 1 }],
  });
  const answered = (await call('get_feedback', { request_id: questionId })).structuredContent.feedback_package;
  assert.equal(answered.manifest.workbench.result.answers[0].value, option.value);
  checks.push('typed questions discovery/create/submit/read preserves structured answer without notes');
  report.scenarios.push({ adapter: 'generic_mcp', checks });
}

try {
  assert.ok(manifestPath, 'Usage: node scripts/external-adapter-acceptance.mjs <acceptance.json> [report.json] [dsh-node-modules]');
  manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
  assert.equal(manifest.fixture, 'rambledesk-feedback-acceptance-v1');
  assert.equal(manifest.status, 'running');
  assert.ok(path.basename(manifest.directory).startsWith('rambledesk-feedback-acceptance-'));
  assert.equal(path.resolve(manifestPath), path.join(manifest.directory, 'acceptance.json'));
  assert.equal(manifest.localTokenFile, path.join(manifest.directory, 'local-server.token'));
  for (const value of [manifest.url, manifest.localApiUrl, manifest.localMcpUrl]) {
    const url = new URL(value);
    assert.equal(url.protocol, 'http:');
    assert.equal(url.hostname, '127.0.0.1');
    assert.ok(url.port);
  }
  Object.assign(report, {
    sourceHead: manifest.sourceHead,
    fixtureBinarySha256: manifest.binarySha256,
    fixtureSourceSha256: manifest.fixtureSourceSha256,
    directory: manifest.directory,
  });

  stage = 'local_token';
  // All adapters read this disposable credential file, never the user's token.
  delete process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN;
  process.env.RAMBLEDESK_LOCAL_API_URL = manifest.localApiUrl;
  process.env.RAMBLEDESK_LOCAL_SERVER_TOKEN_FILE = manifest.localTokenFile;
  localToken = (await readFile(manifest.localTokenFile, 'utf8')).trim();

  stage = 'source_hashes';
  report.sourceFiles = {};
  for (const file of ['packages/pi-rambledesk/index.js', 'packages/dsh-rambledesk/index.js', 'scripts/external-adapter-acceptance.mjs']) {
    report.sourceFiles[file] = createHash('sha256').update(await readFile(new URL(`../${file}`, import.meta.url))).digest('hex');
  }

  stage = 'native_adapter_imports';
  ({ checkHealth: piHealth, registerRambleDeskPiTools } = await import('../packages/pi-rambledesk/index.js'));
  ({ checkHealth: dshHealth, registerRambleDshTools, default: dshPlugin } = await import('../packages/dsh-rambledesk/index.js'));

  stage = 'dsh_host_imports';
  if (process.argv[4]) {
    const root = path.resolve(process.argv[4], '@deepseek-ai');
    const load = name => import(pathToFileURL(path.join(root, name, 'lib/index.js')).href);
    dshHost = {
      ...(await load('cordis')),
      ...(await load('dsh-system-prompt')),
      ...(await load('dsh-tools')),
      ...(await load('dsh-commands')),
    };
    report.dshHostPackages = {};
    for (const name of ['cordis', 'dsh-tools', 'dsh-commands', 'dsh-system-prompt']) {
      const contents = await readFile(path.join(root, name, 'package.json'));
      const metadata = JSON.parse(contents);
      report.dshHostPackages[metadata.name] = { version: metadata.version, manifestSha256: createHash('sha256').update(contents).digest('hex') };
    }
  }

  stage = 'web_session_token';
  durableToken = (await readFile(manifest.tokenFile, 'utf8')).trim();
  stage = 'web_session';
  browserToken = (await webPost('/api/auth/session', undefined, durableToken)).session_token;
  stage = 'health';
  assert.ok(await piHealth());
  assert.ok(await dshHealth());
  stage = 'pi';
  await checkNative('pi');
  stage = 'dsh';
  await checkNative('dsh');
  stage = 'generic_mcp';
  await checkMcp();
  report.result = 'passed';
} catch (error) {
  report.result = 'failed';
  report.failedStage = stage;
  // Parser/assertion errors can echo manifest values; never include them or tokens.
  report.error = stage === 'manifest' ? 'Could not read or validate the isolated fixture manifest.'
    : ['local_token', 'web_session_token'].includes(stage) ? 'Could not read the isolated fixture credential file.'
    : String(error.message ?? error);
  for (const token of [localToken, durableToken, browserToken]) {
    if (token) report.error = report.error.replaceAll(token, '[redacted]');
  }
  process.exitCode = 1;
} finally {
  report.verifiedAt = new Date().toISOString();
  if (process.argv[3]) await writeFile(process.argv[3], `${JSON.stringify(report, null, 2)}\n`);
  console.log(JSON.stringify(report, null, 2));
}
