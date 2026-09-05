// Deterministic UI acceptance Agent. No network, filesystem tools, or real model calls.
import { createInterface } from 'node:readline'
import { randomUUID } from 'node:crypto'

const send = value => process.stdout.write(`${JSON.stringify({ jsonrpc: '2.0', ...value })}\n`)
const respond = (id, result) => send({ id, result })
let sessionId = `preview-${randomUUID()}`
let model = 'preview-balanced'
let effort = 'medium'
let turn = 0
let epoch = 0
let pending
const models = ['preview-fast', 'preview-balanced']
const efforts = ['low', 'medium', 'high']
const options = () => [
  { id: 'model', name: 'Model', category: 'model', type: 'select', currentValue: model,
    options: [{ value: 'preview-fast', name: 'Preview Fast' }, { value: 'preview-balanced', name: 'Preview Balanced' }] },
  { id: 'effort', name: 'Thinking', category: 'thought_level', type: 'select', currentValue: effort,
    options: efforts.map(value => ({ value, name: value === 'low' ? 'Low' : value === 'medium' ? 'Medium' : 'High' })) },
]
const update = update => send({ method: 'session/update', params: { sessionId, update } })
const delay = ms => new Promise(resolve => setTimeout(resolve, ms))
const finish = stopReason => { if (pending !== undefined) { respond(pending, { stopReason }); pending = undefined } }

async function prompt(id, params) {
  pending = id
  const current = ++epoch
  const number = ++turn
  // Runtime context is a separate host block. Do not echo it or credentials to the transcript.
  const text = (params.prompt ?? []).filter(block => block.type === 'text')
    .map(block => block.text).filter(text => !text.trimStart().startsWith('<rambledesk_session_context>'))
    .join('\n').trim().slice(0, 160)
  update({ sessionUpdate: 'agent_thought_chunk', content: { type: 'text', text: `检查这次请求，并用 ${model} / ${effort} 准备预览结果。` } })
  await delay(60)
  if (current !== epoch) return
  const toolCallId = `preview-read-${number}`
  update({ sessionUpdate: 'tool_call', toolCallId, title: '检查预览页面', kind: 'read', status: 'in_progress',
    rawInput: { path: 'preview-page.svelte' }, locations: [{ path: 'preview-page.svelte', line: 1 }], _meta: { toolName: 'preview_read' } })
  await delay(60)
  if (current !== epoch) return
  update({ sessionUpdate: 'tool_call_update', toolCallId, status: 'completed', rawOutput: { reviewed: true, changed: false },
    content: [{ type: 'content', content: { type: 'text', text: '已检查隔离预览中的页面状态，没有读取用户项目文件。' } }] })
  update({ sessionUpdate: 'usage_update', used: 4096 + number * 512, size: 128000 })
  update({ sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: `已完成预览检查。\n\n${text ? `收到：${text}\n\n` : ''}模型：**${model}**，思考等级：**${effort}**。你可以继续切换模型、查看工具详情或发送下一条消息。` } })
  finish('end_turn')
}

createInterface({ input: process.stdin }).on('line', line => {
  const { id, method, params = {} } = JSON.parse(line)
  switch (method) {
    case 'initialize':
      respond(id, { protocolVersion: 1, agentInfo: { name: 'RambleDesk Preview', version: '1' },
        agentCapabilities: { loadSession: true, promptCapabilities: { embeddedContext: true }, mcpCapabilities: { http: false }, sessionCapabilities: { close: {} } } })
      break
    case 'session/new': respond(id, { sessionId, configOptions: options() }); break
    case 'session/load': sessionId = params.sessionId; respond(id, { configOptions: options() }); break
    case 'session/set_config_option':
      if (params.configId === 'model' && models.includes(params.value)) model = params.value
      else if (params.configId === 'effort' && efforts.includes(params.value)) effort = params.value
      else { send({ id, error: { code: -32602, message: 'Unknown preview setting' } }); break }
      respond(id, { configOptions: options() }); break
    case 'session/prompt': void prompt(id, params); break
    case 'session/cancel': epoch++; finish('cancelled'); break
    case 'session/close': epoch++; finish('cancelled'); respond(id, {}); break
    default: if (id !== undefined) send({ id, error: { code: -32601, message: 'Unsupported preview operation' } })
  }
}).on('close', () => process.exit(0))
