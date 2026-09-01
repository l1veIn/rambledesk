import { getContext, setContext } from 'svelte'

import { createUnavailableWorkbenchCapabilities } from './unavailableCapabilities'
import type { WorkbenchCapabilities } from './workbenchCapabilities'

const WORKBENCH_CAPABILITIES = Symbol('rambledesk.workbench-capabilities')

export function provideWorkbenchCapabilities(capabilities: WorkbenchCapabilities): void {
  setContext(WORKBENCH_CAPABILITIES, capabilities)
}

export function useWorkbenchCapabilities(): WorkbenchCapabilities {
  return getContext<WorkbenchCapabilities | undefined>(WORKBENCH_CAPABILITIES) ??
    createUnavailableWorkbenchCapabilities()
}
