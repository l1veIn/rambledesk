// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from './App.svelte'
import type { ApplicationCommandInput, ApplicationCommandName, ApplicationCommandResult } from './lib/application/contracts'
import { createUnavailableWorkbenchCapabilities, UNAVAILABLE_CAPABILITY_MANIFEST } from './lib/capabilities/unavailableCapabilities'
import type { HostSessionSummary, ManagedSessionSnapshot, SessionActivity } from './lib/generated/feedback'
import { config, snapshot } from './lib/agents/draftManagedSessionControllerTestHarness'
import { rememberAgentConnection } from './lib/agents/agentDetectionCache'
import { autoOpenTaskBrief, cookingEnabled, locale, onboardingCompleted } from './lib/preferences'
import { PreviewApplicationTransport } from './lib/preview/previewApplicationTransport'
import { saveWorkspaceSnapshot } from './lib/uiPreferences'
import { agentSessionViewDescriptor, workspaceViewKey } from './lib/workspace/viewDescriptors'

const sessionId = 'changed-files-session'
const agentView = agentSessionViewDescriptor(sessionId)

/** One settled turn: the agent edited two files and deleted a third. */
function activities(): SessionActivity[] {
  const base = { session_id: sessionId, turn_id: 'turn-1', tool_call_id: null, created_at: '2026-09-09T00:00:00Z' }
  return [
    { ...base, id: 'start', sequence: 1, kind: 'status', text: 'Turn started' },
    { ...base, id: 'tool-1', sequence: 2, kind: 'tool_call', text: 'Edit main.ts', tool_call_id: 'tool-1',
      content: { type: 'tool_call', tool: { id: 'tool-1', name: 'edit_file', title: 'Edit main.ts', kind: 'edit', status: 'completed',
        raw_input: null, raw_output: null, locations: [{ path: '/repo/src/main.ts', line: 1 }], truncated: false,
        content: [{ type: 'diff', path: '/repo/src/main.ts', old_text: 'old content\n', new_text: 'new content\n' }] } } },
    { ...base, id: 'tool-2', sequence: 3, kind: 'tool_call', text: 'Add helper', tool_call_id: 'tool-2',
      content: { type: 'tool_call', tool: { id: 'tool-2', name: 'write_file', title: 'Add helper.ts', kind: 'edit', status: 'completed',
        raw_input: null, raw_output: null, locations: [], truncated: false,
        content: [{ type: 'diff', path: '/repo/src/helper.ts', old_text: null, new_text: 'export const helper = 1\n' }] } } },
    { ...base, id: 'tool-3', sequence: 4, kind: 'tool_call', text: 'Delete legacy.ts', tool_call_id: 'tool-3',
      content: { type: 'tool_call', tool: { id: 'tool-3', name: 'delete_file', title: 'Delete legacy.ts', kind: 'delete', status: 'completed',
        raw_input: null, raw_output: null, locations: [{ path: '/repo/src/legacy.ts', line: null }], truncated: false, content: [] } } },
    { ...base, id: 'finish', sequence: 5, kind: 'status', text: 'Turn finished: end_turn' },
  ]
}

class ChangedFilesTransport extends PreviewApplicationTransport {
  readonly managed: ManagedSessionSnapshot = snapshot(sessionId, 'active')

  constructor() {
    super(UNAVAILABLE_CAPABILITY_MANIFEST)
    this.managed.activities = activities()
  }

  override async call<Name extends ApplicationCommandName>(name: Name, input: ApplicationCommandInput<Name>): Promise<ApplicationCommandResult<Name>> {
    switch (name) {
      case 'listAgentConfigs': return [config] as ApplicationCommandResult<Name>
      case 'listAvailableAgents': return [] as unknown as ApplicationCommandResult<Name>
      case 'getManagedSession': return this.managed as ApplicationCommandResult<Name>
      case 'getManagedWorkspaceInfo': return { cwd: '/repo', branch: 'main' } as ApplicationCommandResult<Name>
      case 'getManagedFeedbackStatus':
        return { session_id: sessionId, deleting: false, connection: 'connected', activity: 'idle', deliveries: [] } as ApplicationCommandResult<Name>
      case 'listManagedSessionActivity': return { activities: [], has_more: false } as ApplicationCommandResult<Name>
      case 'listHostSessions': {
        const sessions = await super.call(name, input) as HostSessionSummary[]
        return [...sessions, {
          session_id: sessionId, host_id: 'pi', host_session_id: sessionId, title: 'Task', management: this.managed.session.management,
          source_hint: null, request_count: 0, pending_count: 0, updated_at: '2026-09-09T00:00:00Z', pinned_at: null, archived_at: null, host_pinned_at: null,
        }] as ApplicationCommandResult<Name>
      }
      default: return super.call(name, input)
    }
  }
}

let app: ReturnType<typeof mount> | undefined
let host: HTMLDivElement

beforeEach(() => {
  localStorage.clear()
  locale.set('en'); onboardingCompleted.set(true); autoOpenTaskBrief.set(false); cookingEnabled.set(false)
  saveWorkspaceSnapshot({ version: 2, views: [agentView], activeViewKey: workspaceViewKey(agentView) })
  globalThis.ResizeObserver = class { observe() {} unobserve() {} disconnect() {} } as never
  Element.prototype.getAnimations = () => []
  window.matchMedia = ((query: string) => ({ matches: false, media: query, onchange: null,
    addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {}, dispatchEvent: () => false,
  })) as never
  host = document.createElement('div')
  document.body.append(host)
})

afterEach(async () => {
  if (app) await unmount(app)
  app = undefined
  host.remove()
  document.body.replaceChildren()
})

function tabLabels(): string[] {
  return [...host.querySelectorAll<HTMLElement>('[data-workspace-tab-item]')].map(item => item.textContent?.trim() ?? '')
}

describe('changed files through the real App', () => {
  it('summarizes the turn and opens a diff tab for a file without leaving the Agent tab', async () => {
    const transport = new ChangedFilesTransport()
    rememberAgentConnection(transport, config, { ok: true, message: 'ACP connected', details: [] })
    app = mount(App, { target: host, props: {
      applicationTransport: transport, capabilities: createUnavailableWorkbenchCapabilities(), environment: 'browser',
      publishedFeedbackAction: { label: 'Open feedback package', run: async () => {} },
    } })

    // Settled turns fold their process; the card is part of the turn summary.
    await vi.waitFor(() => expect(host.querySelector('[data-turn-changed-files]')).not.toBeNull(), { timeout: 3_000 })
    const card = host.querySelector<HTMLElement>('[data-turn-changed-files]')!
    expect(card.textContent).toContain('Changed files')
    expect(card.textContent).toContain('3 files')
    expect(card.textContent).toContain('main.ts')
    expect(card.textContent).toContain('helper.ts')
    expect(card.textContent).toContain('legacy.ts')
    expect(card.textContent).toContain('Removed')
    expect(card.textContent).toContain('+2')
    expect(card.textContent).toContain('−1')

    // A removed file has no diff to open; the others do.
    const openButtons = [...card.querySelectorAll<HTMLButtonElement>('button[aria-label^="View changes"]')]
    expect(openButtons).toHaveLength(2)
    const initialTabs = tabLabels().length
    openButtons[0]!.click()
    await vi.waitFor(() => expect(tabLabels()).toHaveLength(initialTabs + 1))
    expect(tabLabels().at(-1)).toBe('main.ts')
    await vi.waitFor(() => expect(host.querySelector('[role="tabpanel"]')?.textContent).toContain('new content'))
    const panel = host.querySelector<HTMLElement>('[role="tabpanel"]')!
    expect(panel.textContent).toContain('/repo/src/main.ts')
    expect(panel.textContent).toContain('+1')
    expect(panel.textContent).toContain('−1')
    // The diff tab never replaces the Agent conversation.
    expect(host.querySelector('[data-agent-timeline]')).toBeNull()
    expect(tabLabels()).toContain('Task · Agent')

    // The same file from another turn keeps its own tab identity.
    expect(workspaceViewKey(agentView)).toBe('agent-session:"changed-files-session"')
    await tick()
  })
})
