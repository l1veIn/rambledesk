// @vitest-environment jsdom
import { mount, unmount } from 'svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from './App.svelte'
import type { ApplicationCommandInput, ApplicationCommandName, ApplicationCommandResult } from './lib/application/contracts'
import { createUnavailableWorkbenchCapabilities, UNAVAILABLE_CAPABILITY_MANIFEST } from './lib/capabilities/unavailableCapabilities'
import { autoOpenTaskBrief, cookingEnabled, locale, onboardingCompleted } from './lib/preferences'
import { PreviewApplicationTransport } from './lib/preview/previewApplicationTransport'
import { savedWorkspaceSnapshot, saveWorkspaceSnapshot } from './lib/uiPreferences'
import { inboxViewDescriptor, workspaceViewKey } from './lib/workspace/viewDescriptors'

class EmptyWorkspaceTransport extends PreviewApplicationTransport {
  readonly calls: ApplicationCommandName[] = []

  constructor() { super(UNAVAILABLE_CAPABILITY_MANIFEST) }

  override async call<Name extends ApplicationCommandName>(name: Name, input: ApplicationCommandInput<Name>): Promise<ApplicationCommandResult<Name>> {
    this.calls.push(name)
    if (name === 'listAgentConfigs' || name === 'listAvailableAgents' || name === 'listAgentInstallJobs') {
      return [] as ApplicationCommandResult<Name>
    }
    return super.call(name, input)
  }
}

let app: ReturnType<typeof mount> | undefined
let host: HTMLDivElement

beforeEach(() => {
  localStorage.clear()
  locale.set('en'); onboardingCompleted.set(true); autoOpenTaskBrief.set(false); cookingEnabled.set(false)
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

function openApp(environment: 'desktop' | 'browser') {
  const transport = new EmptyWorkspaceTransport()
  app = mount(App, { target: host, props: {
    applicationTransport: transport, capabilities: createUnavailableWorkbenchCapabilities(), environment,
    publishedFeedbackAction: { label: 'Open feedback package', run: async () => {} },
  } })
  return transport
}

describe('empty workspace through the real App', () => {
  it.each(['desktop', 'browser'] as const)('shows Rambelle after closing the last %s tab and opens a draft only on request', async environment => {
    const inbox = inboxViewDescriptor()
    saveWorkspaceSnapshot({ version: 2, views: [inbox], activeViewKey: workspaceViewKey(inbox) })
    const transport = openApp(environment)
    await vi.waitFor(() => expect(host.querySelector('[role="tab"][aria-selected="true"]')).not.toBeNull())
    const close = host.querySelector<HTMLButtonElement>('button[aria-label^="Close workspace tab:"]')!
    await vi.waitFor(() => expect(close.disabled).toBe(false))
    close.click()

    await vi.waitFor(() => expect(host.querySelectorAll('[role="tab"]')).toHaveLength(0))
    const empty = host.querySelector('[data-empty-workspace]')
    expect(empty?.querySelector('img[alt="Rambelle"]')).not.toBeNull()
    expect(empty?.textContent).toContain('Choose an agent and a project')
    expect(savedWorkspaceSnapshot()?.shellState.views).toEqual([])
    const create = empty?.querySelector<HTMLButtonElement>('button')
    expect(create?.textContent?.trim()).toBe('New session')
    create!.click()

    await vi.waitFor(() => {
      const tabs = host.querySelectorAll('[role="tab"]')
      expect(tabs).toHaveLength(1)
      expect(tabs[0].getAttribute('data-workspace-view-key')).toMatch(/^agent-draft:/u)
      expect(host.querySelector('[role="textbox"][aria-label="Message the agent"]')).not.toBeNull()
    })
    expect(host.querySelector('[data-empty-workspace]')).toBeNull()
    expect(transport.calls).not.toContain('prepareManagedSession')
    expect(transport.calls).not.toContain('sendManagedPrompt')
  })

  it('restores an explicitly empty workspace without silently reopening a tab', async () => {
    saveWorkspaceSnapshot({ version: 2, views: [], activeViewKey: null })
    openApp('browser')
    await vi.waitFor(() => expect(host.querySelector('[data-empty-workspace] img[alt="Rambelle"]')).not.toBeNull())
    expect(host.querySelectorAll('[role="tab"]')).toHaveLength(0)
    expect(savedWorkspaceSnapshot()?.shellState.views).toEqual([])
  })
})
