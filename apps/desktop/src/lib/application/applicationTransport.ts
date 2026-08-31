import type {
  ApplicationCommandInput,
  ApplicationCommandName,
  ApplicationCommandResult,
} from './contracts'

declare const applicationStreamEvent: unique symbol

/**
 * An opaque, typed stream identity. Concrete stream names and payloads belong to
 * the production transport implementation that introduces them.
 */
export type ApplicationStream<Event> = Readonly<{
  id: string
  [applicationStreamEvent]: Event
}>

export type ApplicationStreamEvent<Stream> =
  Stream extends ApplicationStream<infer Event> ? Event : never

export type Unsubscribe = () => void

export function defineApplicationStream<Event>(id: string): ApplicationStream<Event> {
  return Object.freeze({ id }) as unknown as ApplicationStream<Event>
}

export interface ApplicationTransport<CapabilityManifest = unknown> {
  call<Name extends ApplicationCommandName>(
    name: Name,
    input: ApplicationCommandInput<Name>,
  ): Promise<ApplicationCommandResult<Name>>

  subscribe<Event>(
    stream: ApplicationStream<Event>,
    handler: (event: Event) => void,
  ): Unsubscribe

  waitUntilReady(): Promise<void>

  capabilities(): CapabilityManifest
}
