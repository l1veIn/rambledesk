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

export const HTTP_APPLICATION_OPERATIONS = {
  listFeedbackInbox: 'listFeedbackInbox',
  listHostSessions: 'listHostSessions',
  listArchivedHostSessions: 'listArchivedHostSessions',
  listHostProfiles: 'listHostProfiles',
  listFeedbackRequests: 'listFeedbackRequests',
  getFeedbackWorkspace: 'getFeedbackWorkspace',
  readPublishedFeedback: 'readPublishedFeedback',
  saveFeedbackDraft: 'saveFeedbackDraft',
  addFeedbackAttachment: 'addFeedbackAttachment',
  removeFeedbackAttachment: 'removeFeedbackAttachment',
  reorderFeedbackAttachments: 'reorderFeedbackAttachments',
  submitFeedback: 'submitFeedback',
  approveFeedbackRequest: 'approveFeedbackRequest',
  cancelFeedbackRequest: 'cancelFeedbackRequest',
  renameHostSession: 'renameHostSession',
  setHostSessionPinned: 'setHostSessionPinned',
  archiveHostSession: 'archiveHostSession',
  unarchiveHostSession: 'unarchiveHostSession',
  deleteHostSession: 'deleteHostSession',
  deleteFeedbackRequest: 'deleteFeedbackRequest',
  setHostPinned: 'setHostPinned',
  readFeedbackAttachment: 'readFeedbackAttachment',
  readRequestAttachment: 'readRequestAttachment',
} as const satisfies Record<ApplicationCommandName, string>

export type HttpApplicationOperation =
  (typeof HTTP_APPLICATION_OPERATIONS)[ApplicationCommandName]

const NO_ARGUMENT_COMMANDS: ReadonlySet<ApplicationCommandName> = new Set([
  'listFeedbackInbox',
  'listHostSessions',
  'listHostProfiles',
])

const VOID_COMMANDS: ReadonlySet<ApplicationCommandName> = new Set([
  'deleteHostSession',
  'deleteFeedbackRequest',
])

const BINARY_COMMANDS: ReadonlySet<ApplicationCommandName> = new Set([
  'readFeedbackAttachment',
  'readRequestAttachment',
])

export class StaleHttpApplicationLeaseError extends Error {
  constructor() {
    super('The authenticated application session is no longer active.')
    this.name = 'StaleHttpApplicationLeaseError'
  }
}

export class HttpApplicationStreamUnavailableError extends Error {
  constructor() {
    super('Application event streams are unavailable until WebSocket transport is implemented.')
    this.name = 'HttpApplicationStreamUnavailableError'
  }
}

export interface HttpApplicationLease {
  request(operation: HttpApplicationOperation, init?: RequestInit): Promise<Response>
  assertActive(): void
  waitUntilReady(): Promise<void>
}

export type AuthenticatedHttpApplicationSessionOptions = Readonly<{
  accessToken: string
  applicationBaseUrl?: string | URL
  pageUrl?: string | URL
  fetch?: typeof globalThis.fetch
}>

export class HttpApplicationSession {
  readonly #applicationBaseUrl: URL
  readonly #accessToken: string
  readonly #fetch: typeof globalThis.fetch
  #generation = 1
  #active = true

  private constructor(options: AuthenticatedHttpApplicationSessionOptions) {
    if (!options.accessToken || /[\r\n]/u.test(options.accessToken)) {
      throw new Error('An authenticated application session requires a valid access token.')
    }
    const pageUrl = new URL(
      options.pageUrl ?? globalThis.location?.href ?? 'http://localhost/',
    )
    const applicationBaseUrl = new URL(
      options.applicationBaseUrl ?? '/api/application/',
      pageUrl,
    )
    if (applicationBaseUrl.origin !== pageUrl.origin) {
      throw new Error('The HTTP application transport must use the Workbench page origin.')
    }
    if (!applicationBaseUrl.pathname.endsWith('/')) {
      applicationBaseUrl.pathname += '/'
    }

    this.#applicationBaseUrl = applicationBaseUrl
    this.#accessToken = options.accessToken
    this.#fetch = options.fetch ?? globalThis.fetch.bind(globalThis)
  }

  static authenticated(
    options: AuthenticatedHttpApplicationSessionOptions,
  ): HttpApplicationSession {
    return new HttpApplicationSession(options)
  }

  lease(): HttpApplicationLease {
    const generation = this.#generation
    const assertActive = () => {
      if (!this.#active || generation !== this.#generation) {
        throw new StaleHttpApplicationLeaseError()
      }
    }

    return Object.freeze({
      request: async (operation: HttpApplicationOperation, init: RequestInit = {}) => {
        assertActive()
        const targetUrl = new URL(operation, this.#applicationBaseUrl)
        const expectedPath = `${this.#applicationBaseUrl.pathname}${operation}`
        if (
          targetUrl.origin !== this.#applicationBaseUrl.origin ||
          targetUrl.pathname !== expectedPath ||
          targetUrl.search !== '' ||
          targetUrl.hash !== ''
        ) {
          throw new Error('Invalid HTTP application operation.')
        }
        const headers = new Headers(init.headers)
        headers.set('Authorization', `Bearer ${this.#accessToken}`)
        const response = await this.#fetch(targetUrl, {
          ...init,
          headers,
          credentials: 'same-origin',
          redirect: 'error',
        })
        assertActive()
        return response
      },
      assertActive,
      waitUntilReady: async () => assertActive(),
    })
  }

  invalidate(): void {
    if (!this.#active) return
    this.#active = false
    this.#generation += 1
  }
}

function requestInit<Name extends ApplicationCommandName>(
  name: Name,
  input: ApplicationCommandInput<Name>,
): RequestInit {
  if (NO_ARGUMENT_COMMANDS.has(name)) return { method: 'POST' }

  if (name === 'addFeedbackAttachment') {
    const attachment = input as ApplicationCommandInput<'addFeedbackAttachment'>
    const form = new FormData()
    form.set('request_id', attachment.request_id)
    form.set('file_name', attachment.file_name)
    form.set('expected_revision', String(attachment.expected_revision))
    form.set('file', new Blob([attachment.contents]))
    return { method: 'POST', body: form }
  }

  return {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(input),
  }
}

async function applicationFailure(
  name: ApplicationCommandName,
  response: Response,
  lease: HttpApplicationLease,
): Promise<never> {
  let payload: unknown
  try {
    payload = await decodeWithActiveLease(lease, () => response.json())
  } catch {
    lease.assertActive()
    throw new Error(`Application operation ${name} failed with HTTP ${response.status}.`)
  }
  if (isApplicationError(payload)) throw payload
  throw new Error(`Application operation ${name} failed with HTTP ${response.status}.`)
}

async function decodeWithActiveLease<Result>(
  lease: HttpApplicationLease,
  decode: () => Promise<Result>,
): Promise<Result> {
  let result: Result
  try {
    result = await decode()
  } catch (cause) {
    lease.assertActive()
    throw cause
  }
  lease.assertActive()
  return result
}

export class HttpApplicationTransport<CapabilityManifest = unknown>
  implements ApplicationTransport<CapabilityManifest>
{
  constructor(
    private readonly lease: HttpApplicationLease,
    private readonly capabilityManifest: CapabilityManifest = undefined as CapabilityManifest,
  ) {}

  async call<Name extends ApplicationCommandName>(
    name: Name,
    input: ApplicationCommandInput<Name>,
  ): Promise<ApplicationCommandResult<Name>> {
    const response = await this.lease.request(
      HTTP_APPLICATION_OPERATIONS[name],
      requestInit(name, input),
    )
    if (!response.ok) return applicationFailure(name, response, this.lease)

    let result: unknown
    if (VOID_COMMANDS.has(name)) {
      if (response.status !== 204) {
        throw new Error(`Application operation ${name} returned HTTP ${response.status}, not 204.`)
      }
      result = undefined
    } else if (BINARY_COMMANDS.has(name)) {
      result = await decodeWithActiveLease(this.lease, () => response.arrayBuffer())
    } else {
      result = await decodeWithActiveLease(this.lease, () => response.json())
    }
    if (VOID_COMMANDS.has(name)) this.lease.assertActive()
    return result as ApplicationCommandResult<Name>
  }

  subscribe<Event>(
    _stream: ApplicationStream<Event>,
    _handler: (event: Event) => void,
    onError: SubscriptionErrorHandler,
  ): Unsubscribe {
    let active = true
    queueMicrotask(() => {
      if (active) onError(new HttpApplicationStreamUnavailableError())
    })
    return () => {
      active = false
    }
  }

  waitUntilReady(): Promise<void> {
    return this.lease.waitUntilReady()
  }

  capabilities(): CapabilityManifest {
    return this.capabilityManifest
  }
}
