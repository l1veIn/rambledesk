import type { UpdaterCapability } from '../workbenchCapabilities'
import type { TauriCapabilityApi } from './tauriCapabilityApi'

export function createTauriUpdaterCapability(api: TauriCapabilityApi): UpdaterCapability {
  return {
    version: () => api.getVersion(),
    check: (input) => api.checkForUpdates(input),
    install: () => api.installUpdate(),
    restart: () => api.restartAfterUpdate(),
  }
}
