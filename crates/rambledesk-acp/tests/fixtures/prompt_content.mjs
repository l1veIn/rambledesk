import { createInterface } from 'node:readline'
const send = value => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', ...value }) + '\n')
const respond = (id, result) => send({ id, result })
const full = process.argv[2] === 'full'
let pending
const remote = 'typed-original-session'
const png = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jP1sAAAAASUVORK5CYII='
createInterface({ input: process.stdin }).on('line', line => {
  const { id, method, params } = JSON.parse(line)
  switch (method) {
    case 'initialize': respond(id, { protocolVersion: 1, agentCapabilities: { loadSession: true,
      promptCapabilities: { image: full, audio: false, embeddedContext: full }, sessionCapabilities: { close: {} } } }); break
    case 'session/new': respond(id, { sessionId: remote }); break
    case 'session/load':
      if (params.sessionId !== remote) throw new Error('Original context lost')
      respond(id, {})
      break
    case 'session/prompt':
      if (params.sessionId !== remote) throw new Error('Wrong session')
      if (params.prompt[0].text === 'wait') { pending = id; break }
      const seen = params.prompt.map(block => {
        switch (block.type) {
          case 'text': return { type: block.type, text: block.text }
          case 'image':
            if (!full) throw new Error('Image sent without capability')
            return { type: block.type, mime: block.mimeType, bytes: block.data.length, exactPng: block.data === png }
          case 'resource_link': return { type: block.type, uri: block.uri, name: block.name, mime: block.mimeType }
          case 'resource':
            if (!full) throw new Error('Embedded resource sent without capability')
            return { type: block.type, uri: block.resource.uri, text: block.resource.text, mime: block.resource.mimeType }
          default: throw new Error('Unexpected content')
        }
      })
      send({ method: 'session/update', params: { sessionId: remote, update: {
        sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: JSON.stringify(seen) }
      } } })
      respond(id, { stopReason: 'end_turn' })
      break
    case 'session/cancel': if (pending) { respond(pending, { stopReason: 'cancelled' }); pending = undefined }; break
    case 'session/close': respond(id, {}); break
    default: if (id !== undefined) send({ id, error: { code: -32601, message: 'unsupported' } })
  }
}).on('close', () => process.exit(0))
