import type {
  ApplicationCommandInput,
  ApplicationCommandName,
  ApplicationCommandResult,
} from './contracts'
import { isApplicationError } from './contracts'
import type {
  ApplicationStream,
  ApplicationTransport,
  SubscriptionErrorHandler,
  Unsubscribe,
} from './applicationTransport'
import type { CapabilityManifest } from '../capabilities/capabilityManifest'
import { UNAVAILABLE_CAPABILITY_MANIFEST } from '../capabilities/unavailableCapabilities'
import type { ApplicationEvent } from '../generated/feedback'
import { APPLICATION_EVENTS_STREAM } from './applicationEvents'
import {
  isRuntimeGenerationStaleError,
  isSnapshotUnstableError,
} from './applicationEvents'
import {
  BINARY_COMMANDS,
  HTTP_APPLICATION_OPERATIONS,
  MUTATION_COMMANDS,
  VOID_COMMANDS,
  applicationCommandProjectionKey,
  applicationCommandResponseResources,
  requestInit,
} from './httpApplicationOperations'
import type { HttpApplicationLease } from './httpApplicationSession'
import { HttpApplicationStreamUnavailableError } from './httpApplicationSession'

export * from './httpApplicationOperations'
export * from './httpApplicationSession'

export class HttpApplicationTransport implements ApplicationTransport {
  // HTTP application sessions are restricted to the Workbench page's origin.
  readonly persistenceScope = 'web'
  constructor(
    private readonly lease: HttpApplicationLease,
    private readonly capabilityManifest: CapabilityManifest = UNAVAILABLE_CAPABILITY_MANIFEST,
  ) {}

  async call<Name extends ApplicationCommandName>(
    name: Name,
    input: ApplicationCommandInput<Name>,
  ): Promise<ApplicationCommandResult<Name>> {
    const exchange = await this.lease.request(
      HTTP_APPLICATION_OPERATIONS[name],
      requestInit(name, input),
      {
        resources: applicationCommandResponseResources(name, input),
        projectionKey: applicationCommandProjectionKey(name, input),
        mutation: MUTATION_COMMANDS.has(name),
      },
    )
    const { response, assertActive, commit } = exchange
    if (!response.ok) return applicationFailure(name, response, assertActive)

    let result: unknown
    if (VOID_COMMANDS.has(name)) {
      if (response.status !== 204) {
        throw new Error(`Application operation ${name} returned HTTP ${response.status}, not 204.`)
      }
      result = undefined
    } else if (BINARY_COMMANDS.has(name)) {
      result = await decodeWithActiveLease(assertActive, () => response.arrayBuffer())
    } else {
      result = await decodeWithActiveLease(assertActive, () => response.json())
    }
    if (VOID_COMMANDS.has(name)) assertActive()
    commit()
    return result as ApplicationCommandResult<Name>
  }

  subscribe<Event>(
    stream: ApplicationStream<Event>,
    handler: (event: Event) => void,
    onError: SubscriptionErrorHandler,
  ): Unsubscribe {
    if (stream.id === APPLICATION_EVENTS_STREAM.id) {
      return this.lease.subscribe(handler as (event: ApplicationEvent) => void, onError)
    }
    let active = true
    queueMicrotask(() => active && onError(new HttpApplicationStreamUnavailableError()))
    return () => { active = false }
  }

  waitUntilReady(): Promise<void> {
    return this.lease.waitUntilReady()
  }

  capabilities(): CapabilityManifest {
    return this.capabilityManifest
  }
}

async function applicationFailure(
  name: ApplicationCommandName,
  response: Response,
  assertActive: () => void,
): Promise<never> {
  let payload: unknown
  try {
    payload = await decodeWithActiveLease(assertActive, () => response.json())
  } catch {
    assertActive()
    throw new Error(`Application operation ${name} failed with HTTP ${response.status}.`)
  }
  if (
    isApplicationError(payload) ||
    isRuntimeGenerationStaleError(payload) ||
    isSnapshotUnstableError(payload)
  ) {
    throw payload
  }
  throw new Error(`Application operation ${name} failed with HTTP ${response.status}.`)
}

async function decodeWithActiveLease<Result>(
  assertActive: () => void,
  decode: () => Promise<Result>,
): Promise<Result> {
  let result: Result
  try {
    result = await decode()
  } catch (cause) {
    assertActive()
    throw cause
  }
  assertActive()
  return result
}

