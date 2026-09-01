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

export type ApplicationCallRecord = {
  [Name in ApplicationCommandName]: Readonly<{
    name: Name
    input: ApplicationCommandInput<Name>
  }>
}[ApplicationCommandName]

export type ApplicationCommandHandler<Name extends ApplicationCommandName> = (
  input: ApplicationCommandInput<Name>,
) => ApplicationCommandResult<Name> | Promise<ApplicationCommandResult<Name>>

type ErasedCommandHandler = (input: unknown) => unknown | Promise<unknown>
type StreamSubscription = Readonly<{
  handler: (event: unknown) => void
  onError: SubscriptionErrorHandler
}>

export class TestApplicationTransport implements ApplicationTransport {
  private readonly handlers = new Map<ApplicationCommandName, ErasedCommandHandler>()
  private readonly callRecords: ApplicationCallRecord[] = []
  private readonly streamSubscriptions = new Map<string, Set<StreamSubscription>>()
  private readonly readyPromise: Promise<void>
  private readonly capabilityManifest: CapabilityManifest
  private resolveReady: (() => void) | null = null
  private ready = false

  constructor(
    capabilityManifest: CapabilityManifest | undefined = UNAVAILABLE_CAPABILITY_MANIFEST,
    options: Readonly<{ initiallyReady?: boolean }> = {},
  ) {
    this.capabilityManifest = capabilityManifest ?? UNAVAILABLE_CAPABILITY_MANIFEST
    this.readyPromise = new Promise((resolve) => {
      this.resolveReady = resolve
    })

    if (options.initiallyReady) {
      this.markReady()
    }
  }

  get calls(): readonly ApplicationCallRecord[] {
    return this.callRecords
  }

  handle<Name extends ApplicationCommandName>(
    name: Name,
    handler: ApplicationCommandHandler<Name>,
  ): this {
    this.handlers.set(name, handler as ErasedCommandHandler)
    return this
  }

  resolve<Name extends ApplicationCommandName>(
    name: Name,
    value: ApplicationCommandResult<Name>,
  ): this {
    return this.handle(name, () => value)
  }

  reject<Name extends ApplicationCommandName>(name: Name, reason: unknown): this {
    return this.handle(name, () => Promise.reject(reason))
  }

  async call<Name extends ApplicationCommandName>(
    name: Name,
    input: ApplicationCommandInput<Name>,
  ): Promise<ApplicationCommandResult<Name>> {
    this.callRecords.push({ name, input } as unknown as ApplicationCallRecord)

    const handler = this.handlers.get(name)
    if (!handler) {
      throw new Error(`No test handler registered for application command: ${name}`)
    }

    return (await handler(input)) as ApplicationCommandResult<Name>
  }

  callsFor<Name extends ApplicationCommandName>(
    name: Name,
  ): readonly Extract<ApplicationCallRecord, { name: Name }>[] {
    return this.callRecords.filter(
      (record): record is Extract<ApplicationCallRecord, { name: Name }> => record.name === name,
    )
  }

  subscribe<Event>(
    stream: ApplicationStream<Event>,
    handler: (event: Event) => void,
    onError: SubscriptionErrorHandler,
  ): Unsubscribe {
    let subscriptions = this.streamSubscriptions.get(stream.id)
    if (!subscriptions) {
      subscriptions = new Set()
      this.streamSubscriptions.set(stream.id, subscriptions)
    }

    const subscription: StreamSubscription = {
      handler: handler as (event: unknown) => void,
      onError,
    }
    subscriptions.add(subscription)
    let subscribed = true

    return () => {
      if (!subscribed) return
      subscribed = false
      subscriptions.delete(subscription)
      if (subscriptions.size === 0) {
        this.streamSubscriptions.delete(stream.id)
      }
    }
  }

  emit<Event>(stream: ApplicationStream<Event>, event: Event): void {
    const subscriptions = this.streamSubscriptions.get(stream.id)
    if (!subscriptions) return

    for (const subscription of [...subscriptions]) {
      subscription.handler(event)
    }
  }

  emitSubscriptionError(stream: ApplicationStream<unknown>, cause: unknown): void {
    const subscriptions = this.streamSubscriptions.get(stream.id)
    if (!subscriptions) return

    for (const subscription of [...subscriptions]) {
      subscription.onError(cause)
    }
  }

  waitUntilReady(): Promise<void> {
    return this.readyPromise
  }

  markReady(): void {
    if (this.ready) return
    this.ready = true
    this.resolveReady?.()
    this.resolveReady = null
  }

  capabilities(): CapabilityManifest {
    return this.capabilityManifest
  }
}
