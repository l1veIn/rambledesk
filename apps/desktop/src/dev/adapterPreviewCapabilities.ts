import { writable } from 'svelte/store'
import { createUnavailableWorkbenchCapabilities } from '$lib/capabilities/unavailableCapabilities'
import type { WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'

/** UI fixtures only: no native API, user configuration, or real installation is touched. */
export const adapterPreviewCalls = writable<Record<string, number>>({})
function record<T>(name: string, value: T): Promise<T> {
  adapterPreviewCalls.update(calls => ({ ...calls, [name]: (calls[name] ?? 0) + 1 }))
  return Promise.resolve(value)
}
const unavailable = createUnavailableWorkbenchCapabilities()
const status = { availability: 'available' as const, source: 'native' as const }
export const adapterPreviewCapabilities: WorkbenchCapabilities = {
  ...unavailable,
  manifest: { ...unavailable.manifest, hostIntegrationAdministration: status },
  hostIntegrationAdministration: {
    status,
    implementation: {
      genericMcpConfiguration: () => record('configuration', '{ "mcpServers": { "rambledesk": { "url": "http://preview.invalid" } } }'),
      detectGenericMcpHosts: () => record('mcp', [{ id: 'claude', name: 'Claude Code (preview)', iconSvg: '', installed: true, configured: false, configPath: 'D:/Preview/claude.json', restartRequired: true }]),
      piStatus: () => record('pi', { cliAvailable: true, installed: false, sourceCount: 0, restartRequired: true }),
      dshStatus: () => record('dsh', { id: 'dsh', name: 'DSH (preview)', installed: true, profiles: [{ id: 'default', profileDir: 'D:/Preview/dsh/default', patchPath: 'D:/Preview/dsh/default/cordis.patch.yml', configured: false }], restartRequired: true }),
      installGenericMcpHosts: () => record('install-mcp', []),
      installPi: () => record('install-pi', 'Preview only'),
      uninstallPi: () => record('uninstall-pi', 'Preview only'),
      installDsh: () => record('install-dsh', []),
    },
  },
}
