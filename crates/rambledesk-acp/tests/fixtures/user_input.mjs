import {createInterface} from 'node:readline'
import {appendFileSync} from 'node:fs'
const send = value => process.stdout.write(JSON.stringify({jsonrpc:'2.0',...value})+'\n')
const reply = (id,result) => send({id,result})
let promptId
createInterface({input:process.stdin}).on('line',line=>{
  const {id,method,params,result,error}=JSON.parse(line)
  if(id==='question' && !method){
    if(process.env.FIXTURE_INPUT_LOG) appendFileSync(process.env.FIXTURE_INPUT_LOG,JSON.stringify(result??error)+'\n')
    send({method:'session/update',params:{sessionId:'input-session',update:{sessionUpdate:'agent_message_chunk',content:{type:'text',text:JSON.stringify(result??error)}}}})
    reply(promptId,{stopReason:'end_turn'}); return
  }
  if(method==='initialize') reply(id,{protocolVersion:1,agentCapabilities:{},agentInfo:{name:'input-fixture',version:'1'}})
  else if(method==='session/new') reply(id,{sessionId:'input-session'})
  else if(method==='session/prompt'){
    promptId=id
    const kind=params.prompt[0].text
    if(kind==='late_input' || kind==='late_permission') {
      reply(promptId,{stopReason:'end_turn'})
      setTimeout(()=>send(kind==='late_permission'
        ?{id:'question',method:'session/request_permission',params:{sessionId:params.sessionId,toolCall:{toolCallId:'late',title:'Late operation'},options:[{optionId:'allow',kind:'allow_once',name:'Allow'}]}}
        :{id:'question',method:'elicitation/create',params:{scope:{sessionId:params.sessionId},requestedSchema:{type:'object',properties:{answer:{type:'string'}}}}}),75)
      return
    }
    if(kind==='unsupported') {send({id:'question',method:'elicitation/create',params:{scope:{sessionId:params.sessionId},requestedSchema:{type:'object',properties:{secret:{type:'string',pattern:'^trusted$'}},required:['secret']}}});return}
    if(kind==='grok') send({id:'question',method:'_x.ai/ask_user_question',params:{sessionId:params.sessionId,toolCallId:'ask',questions:[{question:'Which target?',multiSelect:false,options:[{label:'Desktop'},{label:'Web'}]}]}})
    else if(kind==='cursor') send({id:'question',method:'cursor/ask_question',params:{toolCallId:'ask',title:'Choose platform',questions:[{id:'platform',prompt:'Which platform?',options:[{id:'desktop',label:'Desktop'},{id:'web',label:'Web'}],allowMultiple:false}]}})
    else if(kind==='cursor_plan') send({id:'question',method:'cursor/create_plan',params:{toolCallId:'plan',name:'Implementation',overview:'Build interface',plan:'# Plan\nBuild the interface',todos:[]}})
    else if(kind==='plan') send({id:'question',method:'_x.ai/exit_plan_mode',params:{sessionId:params.sessionId,toolCallId:'plan',planContent:'# Plan\nBuild the interface'}})
    else send({id:'question',method:'elicitation/create',params:{mode:'form',scope:{type:'session',sessionId:params.sessionId,toolCallId:'ask'},message:'Choose deployment',requestedSchema:{type:'object',properties:{target:{type:'string',oneOf:[{const:'desktop',title:'Desktop'},{const:'web',title:'Web'}]},count:{type:'integer',minimum:1,maximum:4}},required:['target','count']}}})
  } else if(method==='session/cancel') reply(promptId,{stopReason:'cancelled'})
  else if(id!==undefined) reply(id,{})
}).on('close',()=>process.exit(0))
