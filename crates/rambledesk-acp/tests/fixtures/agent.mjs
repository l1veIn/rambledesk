import { createInterface } from 'node:readline'
import { appendFileSync } from 'node:fs'
const send = value => process.stdout.write(JSON.stringify({jsonrpc:'2.0', ...value})+'\n')
const respond = (id,result) => send({id,result})
const fail = id => send({id,error:{code:-32601,message:'not supported'}})
const mode = process.argv[2] ?? 'load'
let prompting = null
let approvals = []
function finish(stopReason) {
  const id = prompting
  prompting = null
  if (id !== null) respond(id, {stopReason})
}
createInterface({input:process.stdin}).on('line',line => {
  const message = JSON.parse(line)
  const {id,method,params} = message
  if (typeof id === 'string' && id.startsWith('permission') && !method) {
    approvals.push(message.result?.outcome?.outcome)
    if (id === 'permission' || approvals.length === 2)
      finish(approvals.includes('cancelled') ? 'cancelled' : 'end_turn')
    return
  }
  switch (method) {
    case 'initialize':
      respond(id,{protocolVersion:1,agentInfo:{name:'fixture',version:'1'},agentCapabilities:{
        loadSession:mode==='load',mcpCapabilities:{http:true},sessionCapabilities:{close:{},...(mode==='resume'?{resume:{}}:{})}
      }})
      break
    case 'session/new':
      respond(id,{sessionId:'original-session'})
      break
    case 'session/load':
    case 'session/resume':
      if (method === `session/${mode}` && params.sessionId==='original-session') {
        send({method:'session/update',params:{sessionId:params.sessionId,update:{sessionUpdate:'agent_message_chunk',content:{type:'text',text:'REPLAY SHOULD NOT BE DUPLICATED'}}}})
        respond(id,{})
      }
      else fail(id)
      break
    case 'session/prompt':
      prompting = id
      approvals = []
      if (params.prompt[0].text === 'permission') {
        send({id:'permission',method:'session/request_permission',params:{sessionId:params.sessionId,
          toolCall:{toolCallId:'tool-1',title:'Run command',status:'pending'},
          options:[{optionId:'allow',name:'Allow',kind:'allow_once'}]}})
      } else if (params.prompt[0].text === 'permission_pair') {
        for (const number of [1, 2]) send({id:`permission-${number}`,method:'session/request_permission',params:{sessionId:params.sessionId,
          toolCall:{toolCallId:`tool-${number}`,title:`Run command ${number}`,status:'pending',
            ...(number === 1 ? {rawInput:{command:'cargo check',cwd:'C:/fixture-project'},locations:[{path:'C:/fixture-project/Cargo.toml',line:4}]} : {})},
          options:[{optionId:'allow',name:'Allow',kind:'allow_once'}]}})
      } else if (params.prompt[0].text !== 'wait') {
        for(const text of ['fixture ',`reply: ${params.prompt[0].text}`])
          send({method:'session/update',params:{sessionId:params.sessionId,update:{sessionUpdate:'agent_message_chunk',content:{type:'text',text}}}})
        finish('end_turn')
      }
      break
    case 'session/cancel': if(mode !== 'ignore_cancel') finish('cancelled'); break
    case 'session/close':
      if(process.env.FIXTURE_CLOSE_LOG) appendFileSync(process.env.FIXTURE_CLOSE_LOG,params.sessionId+'\n')
      respond(id,{}); break
    default: if (id!==undefined) fail(id)
  }
}).on('close',()=>process.exit(0))
