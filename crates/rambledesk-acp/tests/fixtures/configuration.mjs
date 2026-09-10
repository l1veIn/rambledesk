import { createInterface } from 'node:readline'
const send = value => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', ...value }) + '\n')
const respond = (id, result) => send({ id, result })
const mode = process.argv[2] ?? 'load'
const remote = 'original-config-session'
let model = 'small'
let toggle = false
let prompting
let grokModel = 'grok-a'
let modernMode = false
let modernModeValue = 'ask'
const collisionValues = new Map(['legacy:mode', 'legacy:model', 'config-1', 'x.ai/model'].map(id => [id, 'a']))
const grokOptions = [
  ...['grok-a', 'grok-b', 'grok-c', 'denied'].map(id => ({ id, label: id, category: 'model', selected: id === grokModel })),
  ...['low', 'high'].map(id => ({ id, label: id, category: 'mode', selected: id === 'low' })),
]
const grokModels = { currentModelId: grokModel, availableModels: [
  { modelId: 'grok-a', name: 'Grok A', _meta: { supportsReasoningEffort: true, reasoningEffort: 'low', reasoningEfforts: [{ id: 'low', label: 'Low' }, { id: 'high', label: 'High' }] } },
  { modelId: 'grok-b', name: 'Grok B', _meta: { supportsReasoningEffort: false } },
  { modelId: 'grok-c', name: 'Grok C', _meta: { supportsReasoningEffort: true, reasoningEffort: 'xhigh', reasoningEfforts: [{ id: 'high', label: 'High' }] } },
  { modelId: 'denied', name: 'Denied' },
] }
const modes = { currentModeId: 'ask', availableModes: [{ id: 'ask', name: 'Ask' }, { id: 'plan', name: 'Plan' }] }
const models = { currentModelId: 'legacy-one', availableModels: [{ modelId: 'legacy-one', name: 'Legacy one' }, { modelId: 'legacy-two', name: 'Legacy two' }, { modelId: 'denied', name: 'Denied' }] }
const catalogOptions = () => [
  { id: 'native-mode', name: 'Mode', category: 'mode', type: 'select', currentValue: modes.currentModeId, options: modes.availableModes.map(mode => ({ value: mode.id, name: mode.name })) },
  { id: 'native-model', name: 'Model', category: 'model', type: 'select', currentValue: models.currentModelId, options: models.availableModels.map(model => ({ value: model.modelId, name: model.name })) },
]
const options = () => mode === 'native-catalog' ? catalogOptions() : mode === 'collisions'
  ? [...collisionValues].map(([id, currentValue]) => ({ id, name: id, type: 'select', currentValue, options: ['a', 'b'].map(value => ({ value, name: value })) }))
  : [{ id: 'model', name: 'Model', category: 'model', type: 'select', currentValue: model,
  options: [{ group: 'family', name: 'Family', options: ['small', 'large', 'denied', 'hang'].map(value => ({ value, name: value })) }] },
  { id: 'toggle', name: 'Toggle', type: 'boolean', currentValue: toggle },
  ...(model === 'large' ? [{ id: 'effort', name: 'Effort', category: 'thought_level', type: 'select', currentValue: 'high', options: [{ value: 'high', name: 'High' }] }] : []),
  ...(modernMode ? [{ id: 'legacy:mode', name: 'Modern mode', category: 'mode', type: 'select', currentValue: modernModeValue, options: modes.availableModes.map(mode => ({ value: mode.id, name: mode.name })) }] : [])]
const update = value => send({ method: 'session/update', params: { sessionId: remote, update: value } })
createInterface({ input: process.stdin }).on('line', line => {
  const { id, method, params } = JSON.parse(line)
  switch (method) {
    case 'initialize':
      if (!params.clientCapabilities?.session?.configOptions?.boolean) throw new Error('Boolean support not negotiated')
      respond(id, { protocolVersion: 1, agentCapabilities: { loadSession: mode === 'load', mcpCapabilities: { http: true }, sessionCapabilities: { close: {}, ...(mode === 'resume' ? { resume: {} } : {}) } } })
      break
    case 'session/new':
      respond(id, { sessionId: remote, ...(mode === 'none' ? {} : mode.startsWith('grok')
        ? { _meta: { 'x.ai/sessionConfig': { options: grokOptions } }, ...(mode === 'grok-flat' ? {} : { models: grokModels }), ...(mode === 'grok-native' ? { configOptions: options() } : {}) }
        : mode === 'legacy' || mode.startsWith('mode-') ? { modes, models }
          : { configOptions: options(), modes, models }) }); break
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
      if (mode === 'collisions') {
        if (!collisionValues.has(params.configId)) throw new Error('Collision route lost its wire ID')
        collisionValues.set(params.configId, params.value)
      } else if (mode === 'native-catalog') {
        if (params.configId === 'native-mode') modes.currentModeId = params.value
        else if (params.configId === 'native-model') models.currentModelId = params.value
        else throw new Error('Native catalog used wrong wire ID')
      } else if (modernMode && params.configId === 'legacy:mode') modernModeValue = params.value
      else if (!['model', 'toggle', 'effort'].includes(params.configId)) throw new Error('Public ID leaked to the wire')
      if (params.configId === 'model' && params.value !== 'denied') model = params.value
      if (params.configId === 'toggle') {
        if (params.type !== 'boolean' || typeof params.value !== 'boolean') throw new Error('Boolean value shape lost')
        toggle = params.value
      }
      setTimeout(() => respond(id, { configOptions: options() }), 100)
      break
    case 'session/set_mode':
      if (mode === 'native-catalog') throw new Error('Standard mode routed to legacy setter')
      if (mode === 'mode-rpc-refuse') { send({ id, error: { code: -32602, message: 'Refused mode change' } }); break }
      if (mode !== 'mode-ack') update({ sessionUpdate: 'current_mode_update', currentModeId: mode === 'mode-refuse' ? 'ask' : params.modeId })
      respond(id, {})
      break
    case 'session/set_model':
      if (mode === 'native-catalog') throw new Error('Standard model routed to legacy setter')
      if (params.modelId === 'denied') { send({ id, error: { code: -32602, message: 'Refused model change' } }); break }
      if (mode.startsWith('grok')) {
        if (params._meta) {
          if (params.modelId !== grokModel || !['low', 'high', 'xhigh'].includes(params._meta.reasoningEffort)) throw new Error('Grok effort wire shape lost')
        } else grokModel = params.modelId
      }
      respond(id, {}); break
    case 'session/prompt':
      if (params.prompt[0].text === 'wait') { prompting = id; break }
      modernMode = params.prompt[0].text === 'modern-mode'
      model = 'small'
      respond(id, { stopReason: 'end_turn' })
      setTimeout(() => {
        update({ sessionUpdate: 'config_option_update', configOptions: options() })
        update({ sessionUpdate: 'current_mode_update', currentModeId: 'ask' })
      }, 20)
      break
    case 'session/cancel': if (prompting) { respond(prompting, { stopReason: 'cancelled' }); prompting = undefined }; break
    case 'session/close': respond(id, {}); break
    default: if (id !== undefined) send({ id, error: { code: -32601, message: 'unsupported' } })
  }
}).on('close', () => process.exit(0))
