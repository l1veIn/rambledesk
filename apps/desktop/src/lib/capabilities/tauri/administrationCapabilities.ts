import type {
  DataStorageCapability,
  DiagnosticsCapability,
  HostIntegrationCapability,
  SystemPermissionCapability,
  WebAccessFailureCode,
  WebAccessStatus,
  WebAccessAdministrationCapability,
} from '../workbenchCapabilities'
import { WEB_ACCESS_FAILURE_CODES } from '../workbenchCapabilities'
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
    status: () => webAccessStatus(api, 'get_web_access_status'),
    setEnabled: (enabled) =>
      webAccessStatus(api, enabled ? 'start_web_access' : 'stop_web_access'),
    open: () => api.invoke<void>('open_web_access'),
    copyToken: () => api.invoke<void>('copy_web_access_token'),
  }
}

async function webAccessStatus(
  api: TauriCapabilityApi,
  command: 'get_web_access_status' | 'start_web_access' | 'stop_web_access',
): Promise<WebAccessStatus> {
  return parseWebAccessStatus(await api.invoke<unknown>(command))
}

export function parseWebAccessStatus(value: unknown): WebAccessStatus {
  if (value === null || typeof value !== 'object') throw invalidWebAccessStatus()
  const candidate = value as Record<string, unknown>
  if (candidate.state === 'stopped' && candidate.url === null && candidate.failure === null) {
    return { state: 'stopped', url: null, failure: null }
  }
  if (
    candidate.state === 'running' &&
    typeof candidate.url === 'string' &&
    candidate.url.length > 0 &&
    candidate.failure === null
  ) {
    return { state: 'running', url: candidate.url, failure: null }
  }
  if (
    candidate.state === 'failed' &&
    candidate.url === null &&
    candidate.failure !== null &&
    typeof candidate.failure === 'object'
  ) {
    const failure = candidate.failure as Record<string, unknown>
    if (
      isWebAccessFailureCode(failure.code) &&
      typeof failure.message === 'string' &&
      failure.message.length > 0
    ) {
      return {
        state: 'failed',
        url: null,
        failure: {
          code: failure.code,
          message: failure.message,
        },
      }
    }
  }
  throw invalidWebAccessStatus()
}

function isWebAccessFailureCode(value: unknown): value is WebAccessFailureCode {
  return (
    typeof value === 'string' &&
    (WEB_ACCESS_FAILURE_CODES as readonly string[]).includes(value)
  )
}

function invalidWebAccessStatus(): TypeError {
  return new TypeError('Web Access returned an invalid lifecycle status.')
}

export function createTauriDiagnosticsCapability(
  api: TauriCapabilityApi,
): DiagnosticsCapability {
  return {
    export: (scope, path) => api.invoke('export_diagnostics', { scope, path }),
  }
}
