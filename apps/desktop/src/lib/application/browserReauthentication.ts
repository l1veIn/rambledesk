import type { ApplicationTransport } from './applicationTransport'
import type { ReplaceableApplicationTransport } from './replaceableApplicationTransport'

/** Admit a replacement only after its authenticated ready barrier is complete. */
export async function replaceReadyApplicationTransport(
  current: ReplaceableApplicationTransport,
  next: ApplicationTransport,
  onReady: () => void,
): Promise<void> {
  await next.waitUntilReady()
  current.replace(next)
  onReady()
}
