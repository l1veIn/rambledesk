// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from './App.svelte'
import type { ApplicationCommandInput, ApplicationCommandName, ApplicationCommandResult } from './lib/application/contracts'
import type { ApplicationStream, SubscriptionErrorHandler, Unsubscribe } from './lib/application/applicationTransport'
import { createUnavailableWorkbenchCapabilities, UNAVAILABLE_CAPABILITY_MANIFEST } from './lib/capabilities/unavailableCapabilities'
import type { HostSessionSummary, ManagedSessionSnapshot, SendManagedPromptInput } from './lib/generated/feedback'
import { config, snapshot } from './lib/agents/draftManagedSessionControllerTestHarness'
import { createManagedSessionDraftStorage } from './lib/agents/managedSessionDrafts'
import { sessionPromptDrafts } from './lib/agents/managedSessionUi'
import { autoOpenTaskBrief, cookingEnabled, locale, onboardingCompleted } from './lib/preferences'
import { PreviewApplicationTransport } from './lib/preview/previewApplicationTransport'
import { saveWorkspaceSnapshot } from './lib/uiPreferences'
import { agentDraftViewDescriptor, agentSessionViewDescriptor, inboxViewDescriptor, workspaceViewKey } from './lib/workspace/viewDescriptors'

const draftId = 'quality-managed-draft'
const sessionId = 'quality-managed-session'
const firstMessage = 'Review the mobile feedback workflow.'
const followupMessage = 'Keep this next message while acceptance is unknown.'
const draftView = agentDraftViewDescriptor(draftId)
const agentView = agentSessionViewDescriptor(sessionId)
const composerSelector = '[role="textbox"][aria-label="Message the agent"]'

/** The server accepted the prompt, while reads can still conceal the acknowledgement. */
class ManagedFlowTransport extends PreviewApplicationTransport {
  readonly calls: Array<{ name: ApplicationCommandName; input: unknown }> = []
  readonly subscriptions = new Set<symbol>()
  readonly prepared = snapshot(sessionId)
  accepted: ManagedSessionSnapshot | null = null
  exposeAcceptance = false

  constructor() { super(UNAVAILABLE_CAPABILITY_MANIFEST) }

  override async call<Name extends ApplicationCommandName>(name: Name, input: ApplicationCommandInput<Name>): Promise<ApplicationCommandResult<Name>> {
    this.calls.push({ name, input })
    let result: unknown
    switch (name) {
      case 'listAgentConfigs': result = [config]; break
      case 'listAvailableAgents': case 'listAgentInstallJobs': result = []; break
      case 'prepareManagedSession': result = this.prepared; break
      case 'getManagedSession': result = this.exposeAcceptance && this.accepted ? this.accepted : this.prepared; break
      case 'sendManagedPrompt': {
        const prompt = input as SendManagedPromptInput
        this.accepted = snapshot(sessionId, 'active')
        this.accepted.runtime.activity = 'running'
        this.accepted.activities = [{
          id: 'accepted-message', session_id: sessionId, sequence: 1, turn_id: 'quality-turn',
          kind: 'user_message', text: prompt.text, tool_call_id: null, created_at: '2026-09-09T00:00:00Z',
        }]
        throw new Error('The response was lost after the message was accepted.')
      }
      case 'getManagedWorkspaceInfo': result = { cwd: '/repo', branch: 'main' }; break
      case 'getManagedFeedbackStatus': result = { session_id: sessionId, deleting: false, connection: 'connected', activity: 'running', deliveries: [] }; break
      case 'listManagedSessionActivity': result = { activities: [], has_more: false }; break
      case 'listHostSessions': {
        const sessions = await super.call(name, input) as HostSessionSummary[]
        result = this.exposeAcceptance && this.accepted ? [...sessions, {
          session_id: sessionId, host_id: 'pi', host_session_id: sessionId, title: 'Task', management: this.accepted.session.management,
          source_hint: null, request_count: 0, pending_count: 0, updated_at: '2026-09-09T00:00:00Z', pinned_at: null, archived_at: null, host_pinned_at: null,
        }] : sessions
        break
      }
      default: return super.call(name, input)
    }
    return result as ApplicationCommandResult<Name>
  }

  override subscribe<Event>(_stream: ApplicationStream<Event>, _handler: (event: Event) => void, _onError: SubscriptionErrorHandler): Unsubscribe {
    const subscription = Symbol()
    this.subscriptions.add(subscription)
    return () => { this.subscriptions.delete(subscription) }
  }

  callsFor(name: ApplicationCommandName) { return this.calls.filter(call => call.name === name) }
}

let app: ReturnType<typeof mount> | undefined
let host: HTMLDivElement

beforeEach(() => {
  localStorage.clear()
  locale.set('en'); onboardingCompleted.set(true); autoOpenTaskBrief.set(false); cookingEnabled.set(false)
  sessionPromptDrafts.remove(sessionId)
  createManagedSessionDraftStorage(localStorage).save(draftId, { choice: 'config:config', cwd: '/repo', text: firstMessage })
  saveWorkspaceSnapshot({ version: 2, views: [inboxViewDescriptor(), draftView], activeViewKey: workspaceViewKey(draftView) })
  globalThis.ResizeObserver = class { observe() {} unobserve() {} disconnect() {} } as never
  // jsdom omits the browser animation inventory used by Svelte's keyed tab FLIP.
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
  sessionPromptDrafts.remove(sessionId)
})

function activeKey() { return host.querySelector('[role="tab"][aria-selected="true"]')?.getAttribute('data-workspace-view-key') }
function closeTab(key: string) {
  const item = [...host.querySelectorAll<HTMLElement>('[data-workspace-tab-item]')].find(candidate => candidate.dataset.workspaceViewKey === key)
  const close = item?.querySelector<HTMLButtonElement>('button[aria-label^="Close workspace tab:"]')
  expect(close?.disabled).toBe(false)
  close!.click()
}
function acceptanceButton() {
  return [...host.querySelectorAll<HTMLButtonElement>('button')].find(button => button.textContent?.trim() === 'Check message acceptance')
}

async function typeNextMessage() {
  const composer = host.querySelector<HTMLElement>(composerSelector)!
  const paragraph = document.createElement('p')
  paragraph.textContent = followupMessage
  composer.append(paragraph)
  composer.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertText', data: followupMessage }))
  await tick()
  await vi.waitFor(() => expect(createManagedSessionDraftStorage(localStorage).load(draftId).text).toContain(followupMessage))
}

describe('managed first-message recovery through the real App', () => {
  it('keeps uncertain input recoverable, checks before closing, promotes once, and only releases the accepted view', async () => {
    const transport = new ManagedFlowTransport()
    app = mount(App, { target: host, props: {
      applicationTransport: transport, capabilities: createUnavailableWorkbenchCapabilities(), environment: 'browser',
      publishedFeedbackAction: { label: 'Open feedback package', run: async () => {} },
    } })
    await vi.waitFor(() => expect(host.querySelector<HTMLButtonElement>('[aria-label="Send message"]')?.disabled).toBe(false))
    expect(host.querySelector(composerSelector)?.textContent).toContain(firstMessage)
    host.querySelector<HTMLButtonElement>('[aria-label="Send message"]')!.click()
    await vi.waitFor(() => expect(acceptanceButton()).toBeDefined())
    expect(transport.callsFor('sendManagedPrompt')).toHaveLength(1)
    expect(createManagedSessionDraftStorage(localStorage).load(draftId).text).toBe(firstMessage)
    expect(transport.accepted?.activities[0]?.text).toBe(firstMessage)

    await typeNextMessage()
    const readsBeforeClose = transport.callsFor('getManagedSession').length
    closeTab(workspaceViewKey(draftView))
    await vi.waitFor(() => {
      expect(transport.callsFor('getManagedSession').length).toBeGreaterThan(readsBeforeClose)
      expect(host.querySelector(composerSelector)?.getAttribute('aria-disabled')).toBe('false')
    })
    expect(activeKey()).toBe(workspaceViewKey(draftView))
    expect(host.querySelector(composerSelector)?.textContent).toContain(followupMessage)
    expect(transport.callsFor('discardPreparedSession')).toHaveLength(0)
    expect(transport.callsFor('deleteManagedSession')).toHaveLength(0)

    transport.exposeAcceptance = true
    acceptanceButton()!.click()
    await vi.waitFor(() => {
      expect(activeKey()).toBe(workspaceViewKey(agentView))
      expect(host.querySelector(composerSelector)?.textContent).toContain(followupMessage)
      expect(host.textContent).toContain(firstMessage)
    })
    expect(transport.callsFor('prepareManagedSession')).toHaveLength(1)
    expect(transport.callsFor('sendManagedPrompt')).toHaveLength(1)
    const mountedSubscriptions = transport.subscriptions.size
    closeTab(workspaceViewKey(agentView))
    await vi.waitFor(() => expect(activeKey()).toBe(workspaceViewKey(inboxViewDescriptor())))
    expect(transport.subscriptions.size).toBeLessThan(mountedSubscriptions)
    expect(transport.callsFor('discardPreparedSession')).toHaveLength(0)
    expect(transport.callsFor('deleteManagedSession')).toHaveLength(0)
    expect(transport.callsFor('stopManagedSession')).toHaveLength(0)
    expect(transport.accepted?.runtime.activity).toBe('running')
    expect(sessionPromptDrafts.read(sessionId)).toContain(followupMessage)
    await unmount(app); app = undefined
    expect(transport.subscriptions.size).toBe(0)
  })
})
