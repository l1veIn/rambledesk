import { createInterface } from 'node:readline'
import { appendFileSync, existsSync, writeFileSync } from 'node:fs'
const mode = process.argv[2]
const log = process.env.FIXTURE_CALL_LOG
const marker = process.env.FIXTURE_FAILURE_MARKER
const send = value => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', ...value }) + '\n')
const respond = (id, result) => send({ id, result })
const fail = (id, code) => send({ id, error: { code, message: 'Please login API_KEY=fixture-secret', data: { token: 'fixture-secret' } } })
const firstAttempt = () => {
  if (existsSync(marker)) return false
  writeFileSync(marker, 'attempted')
  return true
}
let model = 'one'
const options = () => [{ id: 'model', name: 'Model', category: 'model', type: 'select', currentValue: model, options: [{ value: 'one', name: 'Model One' }, { value: 'two', name: 'Model Two' }] }]
createInterface({ input: process.stdin }).on('line', line => {
  const { id, method, params } = JSON.parse(line)
  if (method) appendFileSync(log, JSON.stringify({ method, params }) + '\n')
  switch (method) {
    case 'initialize':
      respond(id, { protocolVersion: 1, authMethods: [{ id: 'login', name: 'Sign in' }], agentCapabilities: { loadSession: true, sessionCapabilities: { close: {} } } })
      break
    case 'session/new':
      if (mode === 'session-auth' && firstAttempt()) fail(id, -32000)
      else respond(id, { sessionId: 'original-session', configOptions: options() })
      break
    case 'session/load': respond(id, { configOptions: options() }); break
    case 'session/set_config_option':
      if (mode === 'config-model' && firstAttempt()) fail(id, -32602)
      else if (mode === 'config-auth') fail(id, -32000)
      else { model = params.value; respond(id, { configOptions: options() }) }
      break
    case 'session/prompt':
      if (mode === 'prompt-auth' && firstAttempt()) fail(id, -32000)
      else if (mode === 'prompt-unknown') fail(id, -32603)
      else respond(id, { stopReason: 'end_turn' })
      break
    case 'session/close': respond(id, {}); break
  }
}).on('close', () => process.exit(0))
