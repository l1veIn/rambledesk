import type {
  ApplicationCommandInput,
  ApplicationCommandName,
  ApplicationCommandResult,
} from './contracts'
import type {
  ApplicationStream,
  ApplicationTransport,
  SubscriptionErrorHandler,
  Unsubscribe,
} from './applicationTransport'
import type { CapabilityManifest } from '../capabilities/capabilityManifest'
import { UNAVAILABLE_CAPABILITY_MANIFEST } from '../capabilities/unavailableCapabilities'

const unavailableError = () => new Error('Application transport is unavailable in this environment.')

export class UnavailableApplicationTransport implements ApplicationTransport {
  constructor(
    private readonly capabilityManifest: CapabilityManifest = UNAVAILABLE_CAPABILITY_MANIFEST,
  ) {}

  call<Name extends ApplicationCommandName>(
    _name: Name,
    _input: ApplicationCommandInput<Name>,
  ): Promise<ApplicationCommandResult<Name>> {
    return Promise.reject(unavailableError())
  }

  subscribe<Event>(
    _stream: ApplicationStream<Event>,
    _handler: (event: Event) => void,
    onError: SubscriptionErrorHandler,
  ): Unsubscribe {
    let active = true
    queueMicrotask(() => {
      if (active) onError(unavailableError())
    })
    return () => {
      active = false
    }
  }

  waitUntilReady(): Promise<void> {
    return Promise.reject(unavailableError())
  }

  capabilities(): CapabilityManifest {
    return this.capabilityManifest
  }
}
