import {createInterface} from 'node:readline'
import {randomUUID} from 'node:crypto'
import {appendFileSync} from 'node:fs'
import {isAbsolute} from 'node:path'
let promptId
const send = message => process.stdout.write(JSON.stringify({jsonrpc:'2.0',...message})+'\n')
const reply = (id,result) => send({id,result})
const output = text => send({method:'session/update',params:{sessionId:'original',update:{sessionUpdate:'agent_message_chunk',content:{type:'text',text}}}})
const finish = reason => {const id=promptId;promptId=null;if(id!=null)reply(id,{stopReason:reason})}
async function feedback(operation,input) {
  const response=await fetch(`${process.env.RAMBLEDESK_FEEDBACK_URL}/${operation}`,{method:'POST',headers:{
    'Content-Type':'application/json',
    'Authorization':`Bearer ${process.env.RAMBLEDESK_FEEDBACK_TOKEN}`,
  },body:JSON.stringify(input)})
  if(!response.ok)throw new Error(`Scoped feedback HTTP ${response.status}`)
  return response.json()
}
createInterface({input:process.stdin}).on('line',async line=>{
  const {id,method,params}=JSON.parse(line)
  try {
    if(method==='initialize')return reply(id,{protocolVersion:1,agentCapabilities:{loadSession:true,mcpCapabilities:{http:false},sessionCapabilities:{close:{}}}})
    if(method==='session/new'||method==='session/load') {
      if(params.mcpServers.length!==0)throw new Error('Feedback must not inject MCP')
      if(process.env.RAMBLEDESK_MANAGED_SESSION!=='1'||!isAbsolute(process.env.RAMBLEDESK_COMMAND??''))throw new Error('Missing common command capability')
      if(!process.env.RAMBLEDESK_FEEDBACK_URL?.endsWith('/agent-feedback')||!/^[0-9a-f]{64}$/.test(process.env.RAMBLEDESK_FEEDBACK_TOKEN))throw new Error('Missing scoped feedback capability')
      if(method==='session/load'&&params.sessionId!=='original')throw new Error('Original context lost')
      return reply(id,method==='session/new'?{sessionId:'original'}:{})
    }
    if(method==='session/close'){reply(id,{});return}
    if(method==='session/cancel'){finish('cancelled');return}
    if(method!=='session/prompt')return
    promptId=id
    if(!params.prompt[0].text.includes('<rambledesk_session_context>'))throw new Error('Missing built-in workflow')
    const text=params.prompt.slice(1).map(block=>block.text??'').join('\n')
    if(text.startsWith('request')) {
      const requestId=randomUUID()
      await feedback('request',{request_id:requestId,title:'Review fixture',what_happened:'Check this work',actions:[{id:'review',instruction:'Review fixture'}],context_refs:[],allow_finish:true,final_summary:'Done'})
      output(`REQUEST ${requestId}`)
      if(text!=='request_wait')finish('end_turn')
    } else {
      const requestId=text.match(/[0-9a-f]{8}-[0-9a-f-]{27}/)?.[0]
      const result=await feedback('get',{request_id:requestId})
      output(`CONTINUED ${requestId} ${result.resolution}`)
      if(process.argv[2]==='fail_continue')process.exit(2)
      finish('end_turn')
    }
  } catch(error) { if(process.env.FIXTURE_DIAGNOSTIC)appendFileSync(process.env.FIXTURE_DIAGNOSTIC,String(error)+'\n'); if(id!=null)send({id,error:{code:-32603,message:'Fixture failed'}}) }
}).on('close',()=>process.exit(0))
