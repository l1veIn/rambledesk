// @vitest-environment jsdom
import { mount, unmount } from 'svelte'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createUnavailableWorkbenchCapabilities } from '$lib/capabilities/unavailableCapabilities'
import type { DshHostStatus, WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'
import { locale } from '$lib/preferences'
import AdaptersSettings from './AdaptersSettings.svelte'

let view: ReturnType<typeof mount> | undefined
beforeEach(() => {
  locale.set('en')
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }))
})
afterEach(async () => {
  if (view) await unmount(view)
  view = undefined
  document.body.replaceChildren()
  vi.unstubAllGlobals()
})

it('guides an installed DSH through profile initialization and detects the new profile before installing', async () => {
  let status: DshHostStatus = { id: 'dsh', name: 'DeepSeek Harness', installed: true, profiles: [], restartRequired: true }
  const dshStatus = vi.fn(async () => status)
  const installDsh = vi.fn(async () => [])
  const base = createUnavailableWorkbenchCapabilities()
  const capabilities: WorkbenchCapabilities = {
    ...base,
    hostIntegrationAdministration: {
      status: { availability: 'available', source: 'native' },
      implementation: {
        ...base.hostIntegrationAdministration.implementation,
        detectGenericMcpHosts: async () => [], genericMcpConfiguration: async () => '{}',
        piStatus: async () => ({ cliAvailable: false, installed: false, sourceCount: 0, restartRequired: true }),
        dshStatus, installDsh,
      },
    },
  }
  view = mount(AdaptersSettings, { target: document.body, props: { capabilities, active: true } })
  const card = () => [...document.querySelectorAll('section')].find(element => element.textContent?.includes('DeepSeek Harness native adapter'))!
  const install = () => [...card().querySelectorAll('button')].find(element => element.textContent?.trim() === 'Install')!
  await vi.waitFor(() => expect(card().textContent).toContain('Run dsh web once'))
  expect(card().textContent).not.toContain('DSH not detected')
  expect(install().disabled).toBe(true)
  expect(installDsh).not.toHaveBeenCalled()

  status = { ...status, profiles: [{ id: 'web', profileDir: 'C:/isolated/.dsh/profiles/web', patchPath: 'C:/isolated/.dsh/profiles/web/cordis.patch.yml', configured: false }] }
  card().querySelector<HTMLButtonElement>('[aria-label="Detect DSH again"]')!.click()
  await vi.waitFor(() => expect(install().disabled).toBe(false))
  expect(card().textContent).not.toContain('Run dsh web once')
  expect(dshStatus).toHaveBeenCalledTimes(2)
  install().click()
  await vi.waitFor(() => expect(installDsh).toHaveBeenCalledTimes(1))
})
