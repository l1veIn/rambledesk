import {createInterface} from 'node:readline'
import {randomUUID} from 'node:crypto'
import {appendFileSync} from 'node:fs'
let endpoint, mcpSession, promptId, rpcId = 10
const send = message => process.stdout.write(JSON.stringify({jsonrpc:'2.0',...message})+'\n')
const reply = (id,result) => send({id,result})
const output = text => send({method:'session/update',params:{sessionId:'original',update:{sessionUpdate:'agent_message_chunk',content:{type:'text',text}}}})
const finish = reason => {const id=promptId;promptId=null;if(id!=null)reply(id,{stopReason:reason})}
async function rpc(method,params) {
  const response=await fetch(endpoint.url,{method:'POST',headers:{
    'Content-Type':'application/json','Accept':'application/json, text/event-stream',
    ...Object.fromEntries(endpoint.headers.map(item=>[item.name,item.value])),
    ...(mcpSession?{'Mcp-Session-Id':mcpSession}:{})
  },body:JSON.stringify({jsonrpc:'2.0',...(method.startsWith('notifications/')?{}:{id:rpcId++}),method,params})})
  if(!response.ok)throw new Error(`MCP HTTP ${response.status}`)
  mcpSession=response.headers.get('mcp-session-id')??mcpSession
  const raw=await response.text()
  if(!raw.trim())return
  const message=response.headers.get('content-type')?.includes('text/event-stream')
    ? raw.split('\n').filter(line=>line.startsWith('data:')&&line.slice(5).trim()).map(line=>JSON.parse(line.slice(5))).find(item=>item.id!=null)
    : JSON.parse(raw)
  if(message.error)throw new Error('MCP RPC failure')
  return message.result
}
async function tool(name,args) {
  const result=await rpc('tools/call',{name,arguments:args})
  if(result.isError)throw new Error(`MCP tool failure: ${JSON.stringify(result.structuredContent)}`)
  return result.structuredContent
}
createInterface({input:process.stdin}).on('line',async line=>{
  const {id,method,params}=JSON.parse(line)
  try {
    if(method==='initialize')return reply(id,{protocolVersion:1,agentCapabilities:{loadSession:true,mcpCapabilities:{http:true},sessionCapabilities:{close:{}}}})
    if(method==='session/new'||method==='session/load') {
      endpoint=params.mcpServers.find(server=>server.name==='rambledesk')
      if(!endpoint)throw new Error('Missing scoped MCP')
      await rpc('initialize',{protocolVersion:'2025-03-26',capabilities:{},clientInfo:{name:'fixture',version:'1'}})
      await rpc('notifications/initialized',{})
      return reply(id,method==='session/new'?{sessionId:'original'}:{})
    }
    if(method==='session/close'){reply(id,{});return}
    if(method==='session/cancel'){finish('cancelled');return}
    if(method!=='session/prompt')return
    promptId=id
    const text=params.prompt[0].text
    if(text.startsWith('request')) {
      const requestId=randomUUID()
      await tool('request_feedback',{request_id:requestId,title:'Review fixture',what_happened:'Check this work',actions:[{id:'review',instruction:'Review fixture'}],context_refs:[],allow_finish:true,final_summary:'Done'})
      output(`REQUEST ${requestId}`)
      if(text!=='request_wait')finish('end_turn')
    } else {
      const requestId=text.match(/[0-9a-f]{8}-[0-9a-f-]{27}/)?.[0]
      const feedback=await tool('get_feedback',{request_id:requestId})
      output(`CONTINUED ${requestId} ${feedback.resolution}`)
      if(process.argv[2]==='fail_continue')process.exit(2)
      finish('end_turn')
    }
  } catch(error) { if(process.env.FIXTURE_DIAGNOSTIC)appendFileSync(process.env.FIXTURE_DIAGNOSTIC,String(error)+'\n'); if(id!=null)send({id,error:{code:-32603,message:'Fixture failed'}}) }
}).on('close',()=>process.exit(0))
