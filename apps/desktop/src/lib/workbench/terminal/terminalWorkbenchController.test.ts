// @vitest-environment jsdom
import { expect, it, vi } from 'vitest'
import type { ApplicationTransport } from '../../application/applicationTransport'
import type { TerminalSessionSnapshot } from '../../generated/feedback'
import { emptyTerminalState, type TerminalState } from '../terminalModel'
import { createTerminalWorkbenchController, type TerminalViewBinding } from './terminalWorkbenchController'

const snapshot = (status: TerminalSessionSnapshot['status'], output: string): TerminalSessionSnapshot => ({
  request_id:'request-1',session_id:'trial-1',cwd:'/project',shell:'bash',cols:80,rows:24,status,exit_code:null,
  output,first_sequence:0,next_sequence:output.length,truncated:false,
})
function deferred() {
  let resolve!: () => void
  const promise = new Promise<void>((accept) => resolve=accept)
  return {promise,resolve}
}
function fixture(initial = emptyTerminalState()) {
  let state: TerminalState = initial, screen = '', editable = true
  const call = vi.fn(async (name: string) => snapshot(name === 'stopTerminalSession' ? 'stopped' : 'running',
    name === 'stopTerminalSession' ? '$ command\r\nFinal output' : '$ '))
  const host = createTerminalWorkbenchController({requestId:'request-1',runtime:{transport:{call} as unknown as ApplicationTransport},
    getState:()=>state,updateState:(next)=>{if(next.type==='terminal')state=next},isEditable:()=>editable,setBusy:vi.fn()})
  const binding: TerminalViewBinding = {renderer:{reset:()=>screen='',write:async(output)=>{screen+=output},screen:()=>screen},
    onState:vi.fn(),onBusy:vi.fn(),onReady:vi.fn(),onError:vi.fn()}
  return {host,call,binding,state:()=>state,close:()=>editable=false}
}
it('uses the same preparation after view detach and waits for accepted input before stopping', async () => {
  const f=fixture(), detach=f.host.attach!(f.binding)
  await vi.waitFor(()=>expect(f.binding.onReady).toHaveBeenCalled())
  await f.host.start(80,24)
  const gate=deferred()
  f.call.mockImplementationOnce(async()=>{await gate.promise;return snapshot('running','$ ')})
  const writing=f.host.write('command\r')
  await Promise.resolve()
  detach()
  const preparing=f.host.prepareSubmission()
  await Promise.resolve()
  expect(f.call.mock.calls.some(([name])=>name==='stopTerminalSession')).toBe(false)
  gate.resolve();await writing;await preparing
  expect(f.state().sessions[0]).toMatchObject({status:'stopped',output:'$ command\r\nFinal output',screen:'$ command\nFinal output'})
  expect(f.call.mock.calls.filter(([name])=>name==='openTerminalSession')).toHaveLength(1)
  f.host.dispose()
})
it('asks the backend to finalize even a stopped draft record before publication', async () => {
  const saved={id:'trial-1',cwd:'/project',shell:'bash',cols:80,rows:24,status:'stopped' as const,exit_code:null,
    output:'Saved transcript',screen:'Saved screen',truncated:false}
  const f=fixture({type:'terminal',sessions:[saved]})
  await f.host.prepareSubmission()
  expect(f.call).toHaveBeenCalledWith('stopTerminalSession',{request_id:'request-1',session_id:'trial-1'})
  expect(f.state().sessions[0].output).toContain('Final output')
  f.host.dispose()
})
it('can publish retained history when an in-flight read loses its backend after detaching', async () => {
  const f=fixture(), detach=f.host.attach!(f.binding)
  await vi.waitFor(()=>expect(f.binding.onReady).toHaveBeenCalled())
  await f.host.start(80,24)
  const saved=f.state().sessions[0]
  const missing={code:'INVALID_ARGUMENT',message:'The terminal session was not found for this request.'}
  let rejectRead!: (cause: unknown)=>void
  const reading=new Promise<TerminalSessionSnapshot>((_,reject)=>rejectRead=reject)
  f.call.mockImplementation(async(name)=>{
    if(name==='readTerminalSession')return reading
    if(name==='stopTerminalSession')throw missing
    return snapshot('running','$ ')
  })
  await vi.waitFor(()=>expect(f.call.mock.calls.filter(([name])=>name==='readTerminalSession')).toHaveLength(2))
  detach()
  const preparing=f.host.prepareSubmission()
  rejectRead(missing)
  await expect(preparing).resolves.toBeUndefined()
  expect(f.state().sessions[0]).toEqual({...saved,status:'stopped',exit_code:null})
  expect(f.call.mock.calls.filter(([name])=>name==='openTerminalSession')).toHaveLength(1)
  f.host.dispose()
})
it('rejects terminal mutations after its request becomes read-only', async () => {
  const f=fixture(), detach=f.host.attach!(f.binding)
  await vi.waitFor(()=>expect(f.binding.onReady).toHaveBeenCalled())
  await f.host.start(80,24)
  detach()
  f.close()
  const saved=f.state(), calls=f.call.mock.calls.length
  await f.host.start(90,30)
  await f.host.write('command\r')
  await f.host.resize(90,30)
  await f.host.stop()
  await f.host.prepareSubmission()
  expect(f.call).toHaveBeenCalledTimes(calls)
  expect(f.state()).toEqual(saved)
  f.host.dispose()
})
