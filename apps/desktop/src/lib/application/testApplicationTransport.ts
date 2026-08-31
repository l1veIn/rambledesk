import type {
  ApplicationCommandInput,
  ApplicationCommandName,
  ApplicationCommandResult,
} from './contracts'
import type {
  ApplicationStream,
  ApplicationTransport,
  Unsubscribe,
} from './applicationTransport'

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
type ErasedStreamHandler = (event: unknown) => void

export class TestApplicationTransport<CapabilityManifest = unknown>
  implements ApplicationTransport<CapabilityManifest>
{
  private readonly handlers = new Map<ApplicationCommandName, ErasedCommandHandler>()
  private readonly callRecords: ApplicationCallRecord[] = []
  private readonly streamHandlers = new Map<object, Set<ErasedStreamHandler>>()
  private readonly readyPromise: Promise<void>
  private resolveReady: (() => void) | null = null
  private ready = false

  constructor(
    private readonly capabilityManifest: CapabilityManifest,
    options: Readonly<{ initiallyReady?: boolean }> = {},
  ) {
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
  ): Unsubscribe {
    const streamIdentity = stream as object
    let handlers = this.streamHandlers.get(streamIdentity)
    if (!handlers) {
      handlers = new Set()
      this.streamHandlers.set(streamIdentity, handlers)
    }

    const erasedHandler = handler as ErasedStreamHandler
    handlers.add(erasedHandler)
    let subscribed = true

    return () => {
      if (!subscribed) return
      subscribed = false
      handlers.delete(erasedHandler)
      if (handlers.size === 0) {
        this.streamHandlers.delete(streamIdentity)
      }
    }
  }

  emit<Event>(stream: ApplicationStream<Event>, event: Event): void {
    const handlers = this.streamHandlers.get(stream as object)
    if (!handlers) return

    for (const handler of [...handlers]) {
      handler(event)
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
