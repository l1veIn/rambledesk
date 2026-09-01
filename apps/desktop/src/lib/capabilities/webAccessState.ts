import type {
  WebAccessAdministrationCapability,
  WebAccessStatus,
} from './workbenchCapabilities'

export type WebAccessTransientPhase = 'loading' | 'starting' | 'stopping' | null
export type WebAccessDisplayState =
  | Exclude<WebAccessTransientPhase, null>
  | WebAccessStatus['state']
  | 'unavailable'

export type WebAccessMutationResult = Readonly<{
  status: WebAccessStatus | null
  operationError: unknown | null
  refreshError: unknown | null
}>

type WebAccessStatusCapability = Pick<
  WebAccessAdministrationCapability,
  'status' | 'setEnabled'
>

export function webAccessDisplayState(
  status: WebAccessStatus | null,
  phase: WebAccessTransientPhase,
): WebAccessDisplayState {
  return phase ?? status?.state ?? 'unavailable'
}

export function webAccessRunningActionsEnabled(
  status: WebAccessStatus | null,
  phase: WebAccessTransientPhase,
): boolean {
  return phase === null && status?.state === 'running'
}

export function webAccessToggleTarget(status: WebAccessStatus | null): boolean | null {
  if (status === null) return null
  return status.state !== 'running'
}

/**
 * A failed IPC call is not proof that the lifecycle mutation failed. Re-read
 * the backend fact before deciding which controls are safe to expose.
 */
export async function settleWebAccessMutation(
  implementation: WebAccessStatusCapability,
  enabled: boolean,
): Promise<WebAccessMutationResult> {
  try {
    return {
      status: await implementation.setEnabled(enabled),
      operationError: null,
      refreshError: null,
    }
  } catch (operationError) {
    try {
      const status = await implementation.status()
      const reachedTarget = enabled ? status.state === 'running' : status.state === 'stopped'
      return {
        status,
        operationError: reachedTarget || status.state === 'failed' ? null : operationError,
        refreshError: null,
      }
    } catch (refreshError) {
      return { status: null, operationError, refreshError }
    }
  }
}
