import type {
  DataStorageCapability,
  DiagnosticsCapability,
  HostIntegrationCapability,
  SystemPermissionCapability,
  WebAccessAdministrationCapability,
} from '../workbenchCapabilities'
import { subscribeToTauriEvent } from './subscription'
import type { TauriCapabilityApi } from './tauriCapabilityApi'

export function createTauriSystemPermissionCapability(
  api: TauriCapabilityApi,
): SystemPermissionCapability {
  return {
    list: () => api.invoke('list_macos_permissions'),
    request: (permission) =>
      api.invoke('request_macos_permission', { permission }),
    openSettings: (permission) =>
      api.invoke<void>('open_macos_privacy_settings', { permission }),
  }
}

export function createTauriDataStorageCapability(
  api: TauriCapabilityApi,
): DataStorageCapability {
  return {
    read: () => api.invoke('get_data_storage_settings'),
    select: (path) => api.invoke('set_data_storage_path', { path }),
    onProgress: (handler, onError) =>
      subscribeToTauriEvent(api, 'storage-migration-progress', handler, onError),
  }
}

export function createTauriHostIntegrationCapability(
  api: TauriCapabilityApi,
): HostIntegrationCapability {
  return {
    genericMcpConfiguration: () => api.invoke('get_generic_mcp_configuration'),
    detectGenericMcpHosts: () => api.invoke('detect_generic_mcp_hosts'),
    installGenericMcpHosts: (hostIds) =>
      api.invoke('install_generic_mcp_hosts', { hostIds: [...hostIds] }),
    piStatus: () => api.invoke('get_pi_package_status', { checkoutRoot: null }),
    installPi: () => api.invoke('install_pi_package', { checkoutRoot: null }),
    uninstallPi: () => api.invoke('uninstall_pi_package', { checkoutRoot: null }),
    installDsh: () =>
      api.invoke('install_dsh_package', { checkoutRoot: null, profileId: null }),
  }
}

export function createTauriWebAccessAdministrationCapability(
  api: TauriCapabilityApi,
): WebAccessAdministrationCapability {
  return {
    status: () => api.invoke('get_web_access_status'),
    setEnabled: (enabled) =>
      api.invoke(enabled ? 'start_web_access' : 'stop_web_access'),
    open: () => api.invoke<void>('open_web_access'),
    copyToken: () => api.invoke<void>('copy_web_access_token'),
  }
}

export function createTauriDiagnosticsCapability(
  api: TauriCapabilityApi,
): DiagnosticsCapability {
  return {
    export: (scope, path) => api.invoke('export_diagnostics', { scope, path }),
  }
}
