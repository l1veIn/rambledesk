import { createInterface } from 'node:readline'
const send = value => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', ...value }) + '\n')
const respond = (id, result) => send({ id, result })
const mode = process.argv[2] ?? 'load'
const remote = 'original-config-session'
let model = 'small'
let toggle = false
let prompting
const modes = { currentModeId: 'ask', availableModes: [{ id: 'ask', name: 'Ask' }, { id: 'plan', name: 'Plan' }] }
const models = { currentModelId: 'legacy-one', availableModels: [{ modelId: 'legacy-one', name: 'Legacy one' }, { modelId: 'legacy-two', name: 'Legacy two' }] }
const options = () => [{ id: 'model', name: 'Model', category: 'model', type: 'select', currentValue: model,
  options: [{ group: 'family', name: 'Family', options: ['small', 'large', 'denied', 'hang'].map(value => ({ value, name: value })) }] },
  { id: 'toggle', name: 'Toggle', type: 'boolean', currentValue: toggle },
  ...(model === 'large' ? [{ id: 'effort', name: 'Effort', category: 'thought_level', type: 'select', currentValue: 'high', options: [{ value: 'high', name: 'High' }] }] : [])]
const update = value => send({ method: 'session/update', params: { sessionId: remote, update: value } })
createInterface({ input: process.stdin }).on('line', line => {
  const { id, method, params } = JSON.parse(line)
  switch (method) {
    case 'initialize':
      if (!params.clientCapabilities?.session?.configOptions?.boolean) throw new Error('Boolean support not negotiated')
      respond(id, { protocolVersion: 1, agentCapabilities: { loadSession: mode === 'load', mcpCapabilities: { http: true }, sessionCapabilities: { close: {}, ...(mode === 'resume' ? { resume: {} } : {}) } } })
      break
    case 'session/new': respond(id, { sessionId: remote, ...(mode === 'none' ? {} : { configOptions: options(), modes, models }) }); break
    case 'session/load':
    case 'session/resume':
      if (params.sessionId !== remote || method !== `session/${mode}`) throw new Error('Original context lost')
      model = 'large'
      // Config notifications during load must not disappear with replayed text.
      update({ sessionUpdate: 'config_option_update', configOptions: options() })
      update({ sessionUpdate: 'current_mode_update', currentModeId: 'plan' })
      respond(id, { modes, models })
      break
    case 'session/set_config_option':
      if (params.value === 'hang') break
      if (params.configId === 'model' && params.value !== 'denied') model = params.value
      if (params.configId === 'toggle') {
        if (params.type !== 'boolean' || typeof params.value !== 'boolean') throw new Error('Boolean value shape lost')
        toggle = params.value
      }
      setTimeout(() => respond(id, { configOptions: options() }), 100)
      break
    case 'session/set_mode':
      update({ sessionUpdate: 'current_mode_update', currentModeId: params.modeId })
      respond(id, {})
      break
    case 'session/set_model': respond(id, {}); break
    case 'session/prompt':
      if (params.prompt[0].text === 'wait') { prompting = id; break }
      model = 'small'
      update({ sessionUpdate: 'config_option_update', configOptions: options() })
      update({ sessionUpdate: 'current_mode_update', currentModeId: 'ask' })
      respond(id, { stopReason: 'end_turn' })
      break
    case 'session/cancel': if (prompting) { respond(prompting, { stopReason: 'cancelled' }); prompting = undefined }; break
    case 'session/close': respond(id, {}); break
    default: if (id !== undefined) send({ id, error: { code: -32601, message: 'unsupported' } })
  }
}).on('close', () => process.exit(0))
