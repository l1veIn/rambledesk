import type {
  CapabilityErrorHandler,
  CapabilityUnsubscribe,
} from '../workbenchCapabilities'
import type { TauriCapabilityApi, TauriEvent } from './tauriCapabilityApi'

/** Bridges Tauri's asynchronous listener registration to the synchronous capability contract. */
export function subscribeToTauriEvent<Event>(
  api: TauriCapabilityApi,
  event: string,
  handler: (event: Event) => void,
  onError: CapabilityErrorHandler,
): CapabilityUnsubscribe {
  let active = true
  let unlisten: (() => void) | undefined
  void api
    .listen<Event>(event, (message: TauriEvent<Event>) => handler(message.payload))
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
