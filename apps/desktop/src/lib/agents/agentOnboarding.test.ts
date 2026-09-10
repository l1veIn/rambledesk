import { describe, expect, it, vi } from 'vitest'
import { render } from 'svelte/server'
import type { AgentConfig, AgentInspection } from '$lib/generated/feedback'
import { agentSetupGuidance, formatSetupCommand } from './agentOnboarding'
import AgentSetupGuide from './AgentSetupGuide.svelte'

vi.mock('$lib/preferences', async () => {
  const { writable } = await import('svelte/store')
  return { locale: writable('en') }
})

function inspection(id: string, command: string | null, args: string[] = []): AgentInspection {
  return { agent_id: id, source: command ? 'system' : 'missing', version: null, command, args, dependencies: [], checks: [] }
}
function config(id: string, command: string, args: string[] = [], env: Record<string, string> = {}): AgentConfig {
  return { id: 'profile', catalog_id: id, name: id, host_id: id, protocol: 'acp', enabled: true, command, args, env, created_at: 'today', updated_at: 'today' }
}

describe('external Agent setup guidance', () => {
  it('quotes executable paths and shell metacharacters for PowerShell and POSIX shells', () => {
    expect(formatSetupCommand(["D:\\Agents O'Brien\\grok.exe"], 'windows')).toBe("& 'D:\\Agents O''Brien\\grok.exe'")
    expect(formatSetupCommand(["/home/O'Brien/agent $(touch nope)"], 'posix')).toBe("'/home/O'\\''Brien/agent $(touch nope)'")
    expect(formatSetupCommand(['node', 'C:\\A B\\index.js', '--setup'], 'windows')).toBe("node 'C:\\A B\\index.js' --setup")
    expect(formatSetupCommand(['claude\nGet-Content secret'], 'windows')).toBeUndefined()
    const remote = agentSetupGuidance({ config: config('grok', "/opt/Agent's/grok"), platform: 'windows' })
    expect(remote.platform).toBe('posix')
    expect(remote.command).toBe("'/opt/Agent'\\''s/grok'")
  })

  it('routes DeepSeek ACP to its own setup wizard and DSH to its separate user interface', () => {
    const bridge = inspection('deepseek-acp', 'C:/node.exe', ['C:/agents/node_modules/deepseek-acp/dist/index.js'])
    expect(agentSetupGuidance({ inspection: bridge }).command).toBe('C:/node.exe C:/agents/node_modules/deepseek-acp/dist/index.js --setup')
    const dsh = inspection('dsh', 'D:/agents/dsh.exe', ['--profile', 'acp'])
    expect(agentSetupGuidance({ inspection: dsh }).command).toBe('D:/agents/dsh.exe web')
    expect(agentSetupGuidance({ inspection: bridge }).guide).not.toBe(agentSetupGuidance({ inspection: dsh }).guide)
  })

  it('uses the actual native entry without ACP transport flags or arbitrary arguments', () => {
    const grok = config('grok', 'D:/agents/grok.exe', ['--no-auto-update', 'agent', 'stdio', '--api-key', 'argument-secret'], { XAI_API_KEY: 'environment-secret' })
    expect(agentSetupGuidance({ config: grok }).command).toBe('D:/agents/grok.exe')
    const gemini = inspection('gemini', '/usr/bin/node', ['/opt/agents/node_modules/@google/gemini-cli/dist/index.js', '--acp', '--token', 'secret'])
    expect(agentSetupGuidance({ inspection: gemini }).command).toBe('/usr/bin/node /opt/agents/node_modules/@google/gemini-cli/dist/index.js')
    expect(agentSetupGuidance({ inspection: inspection('gemini', '/usr/bin/node', ['-e', 'printSecret()']) }).command).toBeUndefined()
  })

  it('does not invent an interactive Claude command for the pinned ACP-only installation', () => {
    const bridge = inspection('claude-acp', 'C:/node.exe', ['C:/agents/node_modules/@agentclientprotocol/claude-agent-acp/dist/index.js'])
    bridge.version = '0.73.0'
    const initial = agentSetupGuidance({ inspection: bridge })
    expect(initial.command).toBeUndefined()
    expect(initial.guide).toContain('code.claude.com')
    bridge.dependencies = [{ command: 'claude', required: false, path: 'D:\\my agents\\claude.exe', version: '2.0.0' }]
    expect(agentSetupGuidance({ inspection: bridge }).command).toBe("& 'D:\\my agents\\claude.exe'")
  })

  it('uses Pi’s selected native binary and never launches the bridge as an interactive program', () => {
    const profile = config('pi-acp', 'C:/node.exe', ['C:/agents/node_modules/pi-acp/dist/index.js'], {
      PI_ACP_PI_COMMAND: 'D:\\Pi Home\\pi.cmd', ANTHROPIC_API_KEY: 'do-not-display', PI_ACP_ENABLE_EMBEDDED_CONTEXT: 'true',
    })
    const guide = agentSetupGuidance({ config: profile })
    expect(guide.command).toBe("& 'D:\\Pi Home\\pi.cmd'")
    expect(JSON.stringify(guide)).not.toContain('do-not-display')
    expect(JSON.stringify(guide)).not.toContain('PI_ACP_ENABLE_EMBEDDED_CONTEXT')
    profile.env.PI_ACP_PI_COMMAND = 'pi --api-key do-not-display'
    expect(agentSetupGuidance({ config: profile }).command).toBeUndefined()
    delete profile.env.PI_ACP_PI_COMMAND
    const found = inspection('pi-acp', 'C:/node.exe')
    found.dependencies = [{ command: 'pi', required: true, path: 'C:/agents/node_modules/@earendil-works/pi-coding-agent/dist/cli.js', version: '0.83.0' }]
    expect(agentSetupGuidance({ config: profile, inspection: found }).command).toBe('C:/node.exe C:/agents/node_modules/@earendil-works/pi-coding-agent/dist/cli.js')
  })

  it('preserves either observed Cursor CLI name and never suggests its desktop executable as ACP setup', () => {
    for (const command of ['D:/agents/agent.exe', 'D:/agents/cursor-agent.cmd']) {
      expect(agentSetupGuidance({ inspection: inspection('cursor', command, ['acp']) }).command).toBe(command)
    }
    expect(agentSetupGuidance({ inspection: inspection('cursor', 'D:/agents/Cursor.exe') }).command).toBeUndefined()
    expect(agentSetupGuidance({ catalogId: 'cursor' }).note?.[1]).toContain('desktop application')
  })

  it('does not publish environment secrets or custom command lines as setup commands', () => {
    const profile = config('gemini', '/secret-token/gemini', ['--api-key', 'arg-secret'], { GEMINI_API_KEY: 'secret-token' })
    expect(agentSetupGuidance({ config: profile }).command).toBeUndefined()
    expect(agentSetupGuidance({ config: config('custom', '/bin/sh', ['-c', 'secret']) }).command).toBeUndefined()
    const { body } = render(AgentSetupGuide, { props: { config: profile } })
    expect(body).toContain('Installation and setup guide')
    expect(body).not.toContain('secret-token')
    expect(body).not.toContain('arg-secret')
    expect(body).not.toContain('type="password"')
    expect(body).not.toContain('not logged in')
    expect(body).toContain('Existing launch overrides are still saved')
    expect(profile.env).toEqual({ GEMINI_API_KEY: 'secret-token' })
  })

  it('does not show launch override advice for managed connector defaults alone', () => {
    const profile = config('pi-acp', 'C:/node.exe', [], { PI_ACP_ENABLE_EMBEDDED_CONTEXT: 'true', PI_ACP_PI_COMMAND: 'C:/agents/pi.cmd', PATH: 'C:/agents' })
    expect(agentSetupGuidance({ config: profile }).hasLaunchOverrides).toBe(false)
    expect(agentSetupGuidance({ config: profile }).command).toBe('C:/agents/pi.cmd')
    const { body } = render(AgentSetupGuide, { props: { config: profile } })
    expect(body).not.toContain('Existing launch overrides')
    profile.env.ANTHROPIC_API_KEY = ''
    expect(agentSetupGuidance({ config: profile }).hasLaunchOverrides).toBe(true)
  })
})
