import { currentDesktopPlatform } from '$lib/platform'

import type { WindowCapability } from '../workbenchCapabilities'
import { subscribeToTauriEvent } from './subscription'
import type { TauriCapabilityApi } from './tauriCapabilityApi'

export function createTauriWindowCapability(api: TauriCapabilityApi): WindowCapability {
  return {
    platform: currentDesktopPlatform,
    isMaximized: () => api.currentWindow().isMaximized(),
    minimize: () => api.currentWindow().minimize(),
    toggleMaximize: () => api.currentWindow().toggleMaximize(),
    close: () => api.currentWindow().close(),
    startDragging: () => api.currentWindow().startDragging(),
    async leaveFullscreen() {
      const window = api.currentWindow()
      if (await window.isFullscreen()) await window.setFullscreen(false)
    },
    restart: () => api.invoke<void>('restart_application'),
    onResized: (handler, onError) =>
      subscribeWindowEvent(api.currentWindow().onResized(handler), onError),
    onFocusChanged: (handler, onError) =>
      subscribeWindowEvent(
        api.currentWindow().onFocusChanged(({ payload }) => handler(payload)),
        onError,
      ),
  }
}

function subscribeWindowEvent(
  registration: Promise<() => void>,
  onError: (cause: unknown) => void,
) {
  let active = true
  let unlisten: (() => void) | undefined
  void registration
    .then((nextUnlisten) => {
      if (active) unlisten = nextUnlisten
      else nextUnlisten()
    })
    .catch((cause) => {
      if (active) onError(cause)
    })
  return () => {
    if (!active) return
    active = false
    unlisten?.()
  }
}
