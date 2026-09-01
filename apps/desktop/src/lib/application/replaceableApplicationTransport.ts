import type {
  ApplicationStream,
  ApplicationTransport,
  SubscriptionErrorHandler,
  Unsubscribe,
} from './applicationTransport'
import type {
  ApplicationCommandInput,
  ApplicationCommandName,
  ApplicationCommandResult,
} from './contracts'
import type { CapabilityManifest } from '../capabilities/capabilityManifest'

type Subscription = {
  stream: ApplicationStream<unknown>
  handler: (event: unknown) => void
  onError: SubscriptionErrorHandler
  unsubscribe: Unsubscribe
}

/** Keeps the Workbench mounted while an authenticated browser session is replaced. */
export class ReplaceableApplicationTransport implements ApplicationTransport {
  readonly #subscriptions = new Set<Subscription>()

  constructor(private current: ApplicationTransport) {}

  replace(next: ApplicationTransport): void {
    if (next === this.current) return
    for (const subscription of this.#subscriptions) subscription.unsubscribe()
    this.current = next
    for (const subscription of this.#subscriptions) {
      subscription.unsubscribe = this.current.subscribe(
        subscription.stream,
        subscription.handler,
        subscription.onError,
      )
    }
  }

  call<Name extends ApplicationCommandName>(
    name: Name,
    input: ApplicationCommandInput<Name>,
  ): Promise<ApplicationCommandResult<Name>> {
    return this.current.call(name, input)
  }

  subscribe<Event>(
    stream: ApplicationStream<Event>,
    handler: (event: Event) => void,
    onError: SubscriptionErrorHandler,
  ): Unsubscribe {
    const subscription: Subscription = {
      stream: stream as ApplicationStream<unknown>,
      handler: handler as (event: unknown) => void,
      onError,
      unsubscribe: this.current.subscribe(stream, handler, onError),
    }
    this.#subscriptions.add(subscription)
    let active = true
    return () => {
      if (!active) return
      active = false
      this.#subscriptions.delete(subscription)
      subscription.unsubscribe()
    }
  }

  waitUntilReady(): Promise<void> {
    return this.current.waitUntilReady()
  }

  capabilities(): CapabilityManifest {
    return this.current.capabilities()
  }
}
