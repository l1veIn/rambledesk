import { describe, expect, it } from 'vitest'
import type { AgentConfig } from '$lib/generated/feedback'
import {
  AgentDraftCache,
  agentConfigDraft,
  agentDraftInput,
  isAbsoluteAgentDirectory,
  newAgentDraft,
  parseAgentEnvironment,
  redactAgentMessage,
} from './agentConfigForm'
import { agentText } from './agentI18n'

const config = (id: string): AgentConfig => ({
  id, name: `Agent ${id}`, host_id: 'dsh', protocol: 'acp', enabled: true,
  command: 'deepseek-acp', args: [], env: {}, created_at: '2026-09-04', updated_at: '2026-09-04',
})

describe('agent configuration drafts', () => {
  it('keeps arguments as separate literal values and preserves equals signs in secrets', () => {
    const input = agentDraftInput({
      ...newAgentDraft(), name: 'My agent', command: 'my-agent',
      argsText: '--label\nproject with spaces\n$(not-a-shell)\n',
      envText: '# local environment\r\n TOKEN = abc==\r\nEMPTY=\r\n',
    })
    expect(input.args).toEqual(['--label', 'project with spaces', '$(not-a-shell)'])
    expect(input.env).toEqual({ TOKEN: ' abc==', EMPTY: '' })
    expect(input.id).toBeNull()
  })

  it('rejects malformed and duplicate env lines without echoing their secret contents', () => {
    expect(() => parseAgentEnvironment('super-secret-value')).toThrow('line 1')
    expect(() => parseAgentEnvironment('TOKEN=first\nTOKEN=super-secret-value')).toThrow('line 2')
    expect(() => parseAgentEnvironment('TOKEN=secret\0value')).toThrow('line 1')
    expect(() => parseAgentEnvironment('TOKEN=first\nTOKEN=super-secret-value')).not.toThrow('super-secret-value')
  })

  it('does not mutate object prototypes through environment names', () => {
    const env = parseAgentEnvironment('__proto__=literal\nconstructor=literal-two')
    expect(Object.hasOwn(env, '__proto__')).toBe(true)
    expect(env.__proto__).toBe('literal')
    expect(env.constructor).toBe('literal-two')
    expect(Object.getPrototypeOf(env)).toBe(Object.prototype)
  })

  it('retains separate edits while switching configurations and after external refreshes', () => {
    const cache = new AgentDraftCache()
    const configs = [config('one'), config('two')]
    const one = cache.select('one', configs)
    one.envText = 'TOKEN=private-draft'
    one.name = 'Unsaved name'
    cache.remember(one)
    const two = cache.select('two', configs)
    expect(two.envText).toBe('')
    two.command = 'another-agent'
    cache.remember(two)
    expect(cache.select('one', [{ ...configs[0], name: 'Server refresh' }, configs[1]])).toEqual(one)
    expect(cache.select('two', configs).command).toBe('another-agent')
    // A caller changing the returned draft cannot silently mutate the cache.
    one.name = 'Unremembered edit'
    expect(cache.select('one', configs).name).toBe('Unsaved name')
  })

  it('round-trips saved configurations without deriving commands from labels', () => {
    const saved = { ...config('custom'), catalog_id: 'pi-acp', command: 'C:\\Agents\\my agent.exe', args: ['--stdio'], env: { KEY: 'abc==' } }
    expect(agentDraftInput(agentConfigDraft(saved))).toEqual({
      id: saved.id, catalog_id: saved.catalog_id, name: saved.name, host_id: saved.host_id, protocol: saved.protocol,
      command: saved.command, args: saved.args, env: saved.env, enabled: saved.enabled,
    })
  })

  it('refreshes saved credentials and enabled state without losing unsaved command edits', () => {
    const cache = new AgentDraftCache()
    const saved = { ...config('one'), enabled: false }
    const draft = cache.select(saved.id, [saved])
    draft.command = 'unsaved-command'
    cache.remember(draft)
    const updated = { ...saved, enabled: true, env: { KEY: 'new-key' } }
    expect(cache.reconcile(updated)).toMatchObject({ command: 'unsaved-command', enabled: true, envText: 'KEY=new-key' })
  })

  it('redacts env values from diagnostic and error messages', () => {
    expect(redactAgentMessage('TOKEN=secret-value; command failed with secret', 'TOKEN=secret-value\nOTHER=secret'))
      .toBe('TOKEN=[redacted]; command failed with [redacted]')
  })

})

describe('managed session creation', () => {
  it('accepts absolute server paths on Windows and POSIX', () => {
    for (const path of ['C:\\Projects\\hello', 'D:/repo', '\\\\server\\share\\repo', '/home/me/repo']) {
      expect(isAbsoluteAgentDirectory(path), path).toBe(true)
    }
    for (const path of ['', 'repo', '../repo', '~/', 'C:repo', '\\repo', '/repo\0bad', '\\\\server']) {
      expect(isAbsoluteAgentDirectory(path), path).toBe(false)
    }
  })

  it('localizes validation without exposing the rejected value', () => {
    expect(agentText('zh-CN', 'Invalid environment variable on line 3. Use KEY=VALUE.')).toContain('第 3 行')
    expect(agentText('zh-CN', 'Enter an absolute project directory.')).toBe('请输入项目的绝对目录。')
    expect(agentText('en', 'Check connection')).toBe('Check connection')
  })
})
