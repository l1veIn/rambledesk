type CatalogInput = { agent_id: string }
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
import type {
  ApplicationEvent,
  ApplicationResourceKey,
  ApplicationSnapshotMetadata,
} from '../generated/feedback'
import {
  APPLICATION_EVENT_CREDENTIAL_PROTOCOL_PREFIX,
  APPLICATION_EVENT_PROTOCOL,
  APPLICATION_EVENTS_STREAM,
  REVISION_HEADER,
  RUNTIME_GENERATION_HEADER,
  isRuntimeGenerationStaleError,
  isSnapshotUnstableError,
  applicationResourceKeyIdentity,
  parseApplicationEvent,
} from './applicationEvents'

export const HTTP_APPLICATION_OPERATIONS = {
  listAvailableAgents: 'listAvailableAgents',
  inspectAgentInstallation: 'inspectAgentInstallation',
  resolveCatalogAgent: 'resolveCatalogAgent',
  listAgentInstallJobs: 'listAgentInstallJobs',
  installAgent: 'installAgent',
  cancelAgentInstall: 'cancelAgentInstall',
  listAgentConfigs: 'listAgentConfigs',
  saveAgentConfig: 'saveAgentConfig',
  deleteAgentConfig: 'deleteAgentConfig',
  checkAgentConfig: 'checkAgentConfig',
  createManagedSession: 'createManagedSession',
  prepareManagedSession: 'prepareManagedSession',
  discardPreparedSession: 'discardPreparedSession',
  getManagedSession: 'getManagedSession',
  getManagedFeedbackStatus: 'getManagedFeedbackStatus',
  getManagedWorkspaceInfo: 'getManagedWorkspaceInfo',
  startManagedSession: 'startManagedSession',
  stopManagedSession: 'stopManagedSession',
  cancelManagedPrompt: 'cancelManagedPrompt',
  sendManagedPrompt: 'sendManagedPrompt',
  listManagedSessionActivity: 'listManagedSessionActivity',
  sendManagedPromptContent: 'sendManagedPromptContent',
  setManagedSessionConfig: 'setManagedSessionConfig',
  respondManagedPermission: 'respondManagedPermission',
  resolveFeedbackDelivery: 'resolveFeedbackDelivery',
  deleteManagedSession: 'deleteManagedSession',
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
  'listAvailableAgents', 'listAgentInstallJobs',
  'listAgentConfigs',
  'listFeedbackInbox',
  'listHostSessions',
  'listHostProfiles',
])

const VOID_COMMANDS: ReadonlySet<ApplicationCommandName> = new Set([
  'discardPreparedSession',
  'cancelAgentInstall',
  'deleteManagedSession',
  'deleteAgentConfig',
  'deleteHostSession',
  'deleteFeedbackRequest',
])

const BINARY_COMMANDS: ReadonlySet<ApplicationCommandName> = new Set([
  'readFeedbackAttachment',
  'readRequestAttachment',
])

const MUTATION_COMMANDS: ReadonlySet<ApplicationCommandName> = new Set([
  'prepareManagedSession',
  'discardPreparedSession',
  'resolveCatalogAgent',
  'inspectAgentInstallation', 'installAgent', 'cancelAgentInstall',
  'saveAgentConfig',
  'deleteAgentConfig',
  // The check starts a real agent process and must never be replayed as a query.
  'checkAgentConfig',
  'createManagedSession',
  'startManagedSession',
  'stopManagedSession',
  'cancelManagedPrompt',
  'sendManagedPrompt',
  'sendManagedPromptContent',
  'setManagedSessionConfig',
  'respondManagedPermission',
  'resolveFeedbackDelivery',
  'deleteManagedSession',
  'saveFeedbackDraft',
  'addFeedbackAttachment',
  'removeFeedbackAttachment',
  'reorderFeedbackAttachments',
  'submitFeedback',
  'approveFeedbackRequest',
  'cancelFeedbackRequest',
  'renameHostSession',
  'setHostSessionPinned',
  'archiveHostSession',
  'unarchiveHostSession',
  'deleteHostSession',
  'deleteFeedbackRequest',
  'setHostPinned',
])

export class StaleHttpApplicationLeaseError extends Error {
  constructor() {
    super('The authenticated application session is no longer active.')
    this.name = 'StaleHttpApplicationLeaseError'
  }
}

export class HttpApplicationSessionRevokedError extends Error {
  constructor(readonly status: 401 | 403) {
    super(`The authenticated application session was rejected with HTTP ${status}.`)
    this.name = 'HttpApplicationSessionRevokedError'
  }
}

export class StaleHttpApplicationResponseError extends Error {
  constructor() {
    super('The application response predates the active resource projection watermark.')
    this.name = 'StaleHttpApplicationResponseError'
  }
}

export class HttpApplicationStreamUnavailableError extends Error {
  constructor() {
    super('The requested application event stream is unavailable.')
    this.name = 'HttpApplicationStreamUnavailableError'
  }
}

export type HttpApplicationResponse = Readonly<{
  response: Response
  assertActive: () => void
  commit: () => void
  watermark: Readonly<{
    runtimeGeneration: string
    revision: bigint
    resources: readonly ApplicationResourceKey[]
    projectionKey: string
    mutation: boolean
  }>
}>

export type HttpApplicationResponseContract = Readonly<{
  resources: readonly ApplicationResourceKey[]
  projectionKey: string
  mutation: boolean
}>

export interface HttpApplicationLease {
  request(
    operation: HttpApplicationOperation,
    init?: RequestInit,
    contract?: HttpApplicationResponseContract,
  ): Promise<HttpApplicationResponse>
  assertActive(): void
  waitUntilReady(): Promise<void>
  subscribe(
    handler: (event: ApplicationEvent) => void,
    onError: SubscriptionErrorHandler,
  ): Unsubscribe
}

export type ApplicationWebSocket = Pick<
  WebSocket,
  | 'protocol'
  | 'readyState'
  | 'addEventListener'
  | 'removeEventListener'
  | 'close'
>

export type HttpApplicationReconnectScheduler = (
  callback: () => void,
  delayMs: number,
) => Unsubscribe

export type AuthenticatedHttpApplicationSessionOptions = Readonly<{
  accessToken: string
  applicationBaseUrl?: string | URL
  pageUrl?: string | URL
  fetch?: typeof globalThis.fetch
  webSocket?: (url: string, protocols: string[]) => ApplicationWebSocket
  scheduleReconnect?: HttpApplicationReconnectScheduler
  random?: () => number
  reconnectBaseDelayMs?: number
  onTerminalError?: (error: HttpApplicationSessionRevokedError) => void
}>

export class HttpApplicationSession {
  readonly #applicationBaseUrl: URL
  readonly #accessToken: string
  readonly #fetch: typeof globalThis.fetch
  readonly #healthUrl: URL
  readonly #eventsUrl: URL
  readonly #webSocket: (url: string, protocols: string[]) => ApplicationWebSocket
  readonly #scheduleReconnect: HttpApplicationReconnectScheduler
  readonly #random: () => number
  readonly #reconnectBaseDelayMs: number
  readonly #onTerminalError: (error: HttpApplicationSessionRevokedError) => void
  readonly #subscribers = new Set<
    Readonly<{
      handler: (event: ApplicationEvent) => void
      onError: SubscriptionErrorHandler
    }>
  >()
  #epoch = 0
  #active = true
  #terminalError: HttpApplicationSessionRevokedError | null = null
  #started = false
  #ready = false
  #readyPromise!: Promise<void>
  #resolveReady: (() => void) | null = null
  #rejectReady: ((cause: unknown) => void) | null = null
  #runtimeGeneration: string | null = null
  #revision = -1n
  readonly #resourceRevisions = new Map<string, bigint>()
  readonly #projectionRevisions = new Map<string, bigint>()
  #socket: ApplicationWebSocket | null = null
  #requestAbort = new AbortController()
  #cancelReconnect: Unsubscribe | null = null
  #reconnectAttempt = 0
  #reconnectPending = false

  private constructor(options: AuthenticatedHttpApplicationSessionOptions) {
    if (!/^[A-Za-z0-9_-]+$/u.test(options.accessToken)) {
      throw new Error(
        'An authenticated application session requires a base64url session token.',
      )
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
    this.#healthUrl = new URL('/api/health', pageUrl)
    this.#eventsUrl = new URL('/api/events', pageUrl)
    this.#accessToken = options.accessToken
    this.#fetch = options.fetch ?? globalThis.fetch.bind(globalThis)
    this.#webSocket =
      options.webSocket ??
      ((url, protocols) => new globalThis.WebSocket(url, protocols))
    this.#scheduleReconnect =
      options.scheduleReconnect ??
      ((callback, delayMs) => {
        const timer = globalThis.setTimeout(callback, delayMs)
        return () => globalThis.clearTimeout(timer)
      })
    this.#random = options.random ?? Math.random
    this.#reconnectBaseDelayMs = options.reconnectBaseDelayMs ?? 250
    this.#onTerminalError = options.onTerminalError ?? (() => {})
    this.#resetReadyBarrier()
  }

  static authenticated(
    options: AuthenticatedHttpApplicationSessionOptions,
  ): HttpApplicationSession {
    return new HttpApplicationSession(options)
  }

  lease(): HttpApplicationLease {
    this.#ensureStarted()
    return Object.freeze({
      request: async (
        operation: HttpApplicationOperation,
        init: RequestInit = {},
        contract: HttpApplicationResponseContract = {
          resources: [{ kind: 'all' }],
          projectionKey: 'unscoped',
          mutation: false,
        },
      ) => {
        await this.waitUntilReady()
        this.#assertActive()
        const epoch = this.#epoch
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
        if (MUTATION_COMMANDS.has(operation as ApplicationCommandName)) {
          headers.set(RUNTIME_GENERATION_HEADER, this.#runtimeGeneration ?? '')
        }
        let response: Response
        try {
          response = await this.#fetch(targetUrl, {
            ...init,
            headers,
            credentials: 'same-origin',
            redirect: 'error',
            signal: this.#requestAbort.signal,
          })
        } catch (cause) {
          this.#assertEpoch(epoch)
          throw cause
        }
        if (isAuthenticationRejection(response.status)) {
          throw this.#revoke(response.status)
        }
        const watermark = {
          ...this.#responseWatermark(response, epoch),
          ...contract,
        }
        const assertResponseActive = () => this.#assertResponseCurrent(epoch, watermark)
        const commitResponse = () => this.#commitResponse(epoch, watermark)
        assertResponseActive()
        return {
          response,
          assertActive: assertResponseActive,
          commit: commitResponse,
          watermark,
        }
      },
      assertActive: () => this.#assertActive(),
      waitUntilReady: () => this.waitUntilReady(),
      subscribe: (
        handler: (event: ApplicationEvent) => void,
        onError: SubscriptionErrorHandler,
      ) => this.#subscribe(handler, onError),
    })
  }

  invalidate(): void {
    if (!this.#active) return
    this.#active = false
    this.#rejectReady?.(new StaleHttpApplicationLeaseError())
    this.#rejectReady = null
    this.#epoch += 1
    this.#ready = false
    this.#requestAbort.abort()
    this.#socket?.close()
    this.#socket = null
    this.#cancelReconnect?.()
    this.#cancelReconnect = null
    this.#subscribers.clear()
  }

  waitUntilReady(): Promise<void> {
    if (!this.#active) return Promise.reject(this.#inactiveError())
    this.#ensureStarted()
    return this.#ready ? Promise.resolve() : this.#readyPromise
  }

  #ensureStarted(): void {
    if (this.#started || !this.#active) return
    this.#started = true
    void this.#connect()
  }

  #subscribe(
    handler: (event: ApplicationEvent) => void,
    onError: SubscriptionErrorHandler,
  ): Unsubscribe {
    if (!this.#active && this.#terminalError) {
      const error = this.#terminalError
      let subscribed = true
      queueMicrotask(() => {
        if (!subscribed) return
        try {
          onError(error)
        } catch {
          // Error observers cannot reactivate a revoked session.
        }
      })
      return () => {
        subscribed = false
      }
    }
    const subscription = { handler, onError }
    this.#subscribers.add(subscription)
    let subscribed = true
    return () => {
      if (!subscribed) return
      subscribed = false
      this.#subscribers.delete(subscription)
    }
  }

  #resetReadyBarrier(): void {
    this.#readyPromise = new Promise((resolve, reject) => {
      this.#resolveReady = resolve
      this.#rejectReady = reject
    })
    void this.#readyPromise.catch(() => undefined)
  }

  async #connect(): Promise<void> {
    if (!this.#active) return
    this.#cancelReconnect?.()
    this.#cancelReconnect = null
    const epoch = ++this.#epoch
    this.#ready = false
    this.#resetReadyBarrier()
    this.#requestAbort.abort()
    this.#requestAbort = new AbortController()
    try {
      const health = await this.#fetch(this.#healthUrl, {
        method: 'POST',
        headers: { Authorization: `Bearer ${this.#accessToken}` },
        credentials: 'same-origin',
        redirect: 'error',
        signal: this.#requestAbort.signal,
      })
      this.#assertEpoch(epoch)
      if (isAuthenticationRejection(health.status)) {
        throw this.#revoke(health.status)
      }
      if (!health.ok) throw new Error(`Web Access health probe failed with HTTP ${health.status}.`)
      await this.#decodeMetadata(health, epoch)
      this.#assertEpoch(epoch)
      const eventsUrl = new URL(this.#eventsUrl)
      eventsUrl.protocol = eventsUrl.protocol === 'https:' ? 'wss:' : 'ws:'
      const socket = this.#webSocket(eventsUrl.toString(), [
        APPLICATION_EVENT_PROTOCOL,
        `${APPLICATION_EVENT_CREDENTIAL_PROTOCOL_PREFIX}${this.#accessToken}`,
      ])
      this.#socket = socket
      const onMessage = (message: MessageEvent) => this.#handleMessage(epoch, message)
      const onClose = () => {
        socket.removeEventListener('message', onMessage)
        socket.removeEventListener('close', onClose)
        socket.removeEventListener('error', onError)
        this.#handleDisconnect(epoch)
      }
      const onError = () => this.#handleDisconnect(epoch)
      socket.addEventListener('message', onMessage)
      socket.addEventListener('close', onClose)
      socket.addEventListener('error', onError)
    } catch (cause) {
      if (this.#active && epoch === this.#epoch && !(cause instanceof StaleHttpApplicationLeaseError)) {
        this.#reportSubscriptionError(cause)
        this.#handleDisconnect(epoch)
      }
    }
  }

  async #decodeMetadata(response: Response, epoch: number): Promise<ApplicationSnapshotMetadata> {
    let metadata: ApplicationSnapshotMetadata
    try {
      metadata = (await response.json()) as ApplicationSnapshotMetadata
    } catch (cause) {
      this.#assertEpoch(epoch)
      throw cause
    }
    this.#assertEpoch(epoch)
    if (
      typeof metadata?.runtime_generation !== 'string' ||
      !/^\d+$/u.test(metadata?.revision ?? '')
    ) {
      throw new Error('Web Access health returned invalid runtime metadata.')
    }
    return metadata
  }

  #handleMessage(epoch: number, message: MessageEvent): void {
    if (!this.#active || epoch !== this.#epoch) return
    try {
      if (typeof message.data !== 'string') {
        throw new Error('Application event WebSocket received a non-text frame.')
      }
      const event = parseApplicationEvent(JSON.parse(message.data))
      if (event.type === 'ready') {
        if (this.#socket?.protocol !== APPLICATION_EVENT_PROTOCOL) {
          throw new Error('Application event WebSocket negotiated an invalid protocol.')
        }
        this.#acceptReady(epoch, event)
        return
      }
      if (!this.#ready || event.runtime_generation !== this.#runtimeGeneration) return
      const revision = parseRevision(event.revision)
      if (revision <= this.#revision) return
      this.#revision = revision
      this.#recordInvalidation(event.resources, revision)
      this.#emit(event)
    } catch (cause) {
      this.#reportSubscriptionError(cause)
      this.#handleDisconnect(epoch)
    }
  }

  #acceptReady(
    epoch: number,
    event: Extract<ApplicationEvent, { type: 'ready' }>,
  ): void {
    if (epoch !== this.#epoch) return
    const revision = parseRevision(event.revision)
    const previousGeneration = this.#runtimeGeneration
    const reconnect = this.#reconnectPending || previousGeneration !== null
    if (previousGeneration === event.runtime_generation && revision < this.#revision) {
      this.#handleDisconnect(epoch)
      return
    }
    if (previousGeneration !== event.runtime_generation) {
      this.#requestAbort.abort()
      this.#requestAbort = new AbortController()
      this.#runtimeGeneration = event.runtime_generation
      this.#revision = revision
      this.#resourceRevisions.clear()
      this.#projectionRevisions.clear()
    } else {
      this.#revision = revision
    }
    this.#ready = true
    this.#reconnectPending = false
    this.#reconnectAttempt = 0
    this.#resolveReady?.()
    this.#resolveReady = null
    this.#rejectReady = null
    this.#emit(event)
    if (reconnect) {
      const invalidation: ApplicationEvent = {
        type: 'invalidate',
        runtime_generation: event.runtime_generation,
        revision: event.revision,
        resources: [{ kind: 'all' }],
      }
      this.#recordInvalidation(invalidation.resources, revision)
      this.#emit(invalidation)
    }
  }

  #handleDisconnect(epoch: number): void {
    if (!this.#active || epoch !== this.#epoch) return
    this.#reconnectPending = true
    this.#epoch += 1
    const disconnectedEpoch = this.#epoch
    this.#ready = false
    this.#rejectReady?.(new StaleHttpApplicationLeaseError())
    this.#rejectReady = null
    this.#resetReadyBarrier()
    this.#requestAbort.abort()
    this.#socket?.close()
    this.#socket = null
    this.#scheduleNextConnect(disconnectedEpoch)
  }

  #scheduleNextConnect(epoch: number): void {
    if (!this.#active || epoch !== this.#epoch || this.#cancelReconnect) return
    const exponential = this.#reconnectBaseDelayMs * 2 ** Math.min(this.#reconnectAttempt, 5)
    const delay = Math.round(exponential * (0.75 + this.#random() * 0.5))
    this.#reconnectAttempt += 1
    this.#cancelReconnect = this.#scheduleReconnect(() => {
      this.#cancelReconnect = null
      void this.#connect()
    }, delay)
  }

  #responseWatermark(response: Response, epoch: number): Readonly<{
    runtimeGeneration: string
    revision: bigint
  }> {
    const runtimeGeneration = response.headers.get(RUNTIME_GENERATION_HEADER)
    const revisionText = response.headers.get(REVISION_HEADER)
    if (!runtimeGeneration || !revisionText) {
      throw new Error('Application response omitted runtime snapshot metadata.')
    }
    if (runtimeGeneration !== this.#runtimeGeneration) {
      this.#handleDisconnect(epoch)
      throw new StaleHttpApplicationLeaseError()
    }
    return { runtimeGeneration, revision: parseRevision(revisionText) }
  }

  #assertResponseCurrent(
    epoch: number,
    watermark: Readonly<{
      runtimeGeneration: string
      revision: bigint
      resources: readonly ApplicationResourceKey[]
      projectionKey: string
      mutation: boolean
    }>,
  ): void {
    this.#assertEpoch(epoch)
    if (watermark.runtimeGeneration !== this.#runtimeGeneration) {
      throw new StaleHttpApplicationResponseError()
    }
    if (
      !watermark.mutation &&
      (this.#projectionRevision(watermark.projectionKey) > watermark.revision ||
        watermark.resources.some(
          (resource) =>
            this.#resourceRevision(applicationResourceKeyIdentity(resource)) >
            watermark.revision,
        ))
    ) {
      throw new StaleHttpApplicationResponseError()
    }
  }

  #commitResponse(
    epoch: number,
    watermark: Readonly<{
      runtimeGeneration: string
      revision: bigint
      resources: readonly ApplicationResourceKey[]
      projectionKey: string
      mutation: boolean
    }>,
  ): void {
    this.#assertResponseCurrent(epoch, watermark)
    if (watermark.mutation) return
    if (watermark.revision > this.#projectionRevision(watermark.projectionKey)) {
      this.#projectionRevisions.set(watermark.projectionKey, watermark.revision)
    }
  }

  #recordInvalidation(resources: readonly ApplicationResourceKey[], revision: bigint): void {
    for (const resource of resources) {
      const identity = applicationResourceKeyIdentity(resource)
      if (revision > this.#resourceRevision(identity)) {
        this.#resourceRevisions.set(identity, revision)
      }
    }
  }

  #resourceRevision(identity: string): bigint {
    return [identity, 'all'].reduce(
      (highest, candidate) => {
        const revision = this.#resourceRevisions.get(candidate) ?? -1n
        return revision > highest ? revision : highest
      },
      -1n,
    )
  }

  #projectionRevision(projectionKey: string): bigint {
    return this.#projectionRevisions.get(projectionKey) ?? -1n
  }

  #assertActive(): void {
    if (!this.#active) throw this.#inactiveError()
  }

  #assertEpoch(epoch: number): void {
    this.#assertActive()
    if (epoch !== this.#epoch) throw new StaleHttpApplicationLeaseError()
  }

  #inactiveError(): Error {
    return this.#terminalError ?? new StaleHttpApplicationLeaseError()
  }

  #revoke(status: 401 | 403): HttpApplicationSessionRevokedError {
    const error = this.#terminalError ?? new HttpApplicationSessionRevokedError(status)
    if (!this.#active) return error
    this.#terminalError = error
    this.#active = false
    this.#ready = false
    this.#rejectReady?.(error)
    this.#rejectReady = null
    this.#epoch += 1
    this.#requestAbort.abort()
    this.#socket?.close()
    this.#socket = null
    this.#cancelReconnect?.()
    this.#cancelReconnect = null
    this.#reportSubscriptionError(error)
    this.#subscribers.clear()
    try {
      this.#onTerminalError(error)
    } catch {
      // The composition root cannot keep a revoked authenticated session alive.
    }
    return error
  }

  #emit(event: ApplicationEvent): void {
    for (const subscriber of [...this.#subscribers]) subscriber.handler(event)
  }

  #reportSubscriptionError(cause: unknown): void {
    for (const subscriber of [...this.#subscribers]) {
      try {
        subscriber.onError(cause)
      } catch {
        // Error observers cannot keep a malformed stream alive.
      }
    }
  }
}

function parseRevision(revision: string): bigint {
  if (!/^\d+$/u.test(revision)) throw new Error('Application revision must be a decimal string.')
  return BigInt(revision)
}

function isAuthenticationRejection(status: number): status is 401 | 403 {
  return status === 401 || status === 403
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

function requestResource(requestId: string): ApplicationResourceKey {
  return { kind: 'feedback_workspace', request_id: canonicalUuid(requestId) }
}

function publishedResource(requestId: string): ApplicationResourceKey {
  return { kind: 'published_feedback', request_id: canonicalUuid(requestId) }
}

function hostSessionResource(
  input: Readonly<{ host_id: string; host_session_id: string }>,
  trim = true,
): ApplicationResourceKey {
  return {
    kind: 'host_session_resources',
    host_id: trim ? input.host_id.trim() : input.host_id,
    host_session_id: trim ? input.host_session_id.trim() : input.host_session_id,
  }
}

function canonicalUuid(value: string): string {
  let candidate = value
  if (candidate.startsWith('urn:uuid:')) candidate = candidate.slice('urn:uuid:'.length)
  if (candidate.startsWith('{') && candidate.endsWith('}')) {
    candidate = candidate.slice(1, -1)
  }
  const simple = /^[0-9a-f]{32}$/iu.test(candidate)
  const hyphenated = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/iu.test(
    candidate,
  )
  if (!simple && !hyphenated) return value
  const hex = candidate.replaceAll('-', '').toLowerCase()
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`
}

function canonicalSearch(search: string | null): string | null {
  const trimmed = search?.trim() ?? ''
  return trimmed === '' ? null : trimmed
}

function canonicalCursor(cursor: string | null): string | null {
  if (cursor === null || !/^(?:[0-9a-f]{2})+$/iu.test(cursor)) return cursor
  try {
    const bytes = Uint8Array.from(cursor.match(/.{2}/gu) ?? [], (byte) => Number.parseInt(byte, 16))
    const decoded = new TextDecoder('utf-8', { fatal: true }).decode(bytes)
    const separator = decoded.indexOf('\0')
    if (separator < 1 || decoded.indexOf('\0', separator + 1) !== -1) return cursor
    const updatedAt = decoded.slice(0, separator)
    const requestId = canonicalUuid(decoded.slice(separator + 1))
    if (requestId === decoded.slice(separator + 1) && !isCanonicalUuid(requestId)) return cursor
    return Array.from(new TextEncoder().encode(`${updatedAt}\0${requestId}`), (byte) =>
      byte.toString(16).padStart(2, '0'),
    ).join('')
  } catch {
    return cursor
  }
}

function isCanonicalUuid(value: string): boolean {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/u.test(value)
}

export function applicationCommandResponseResources<Name extends ApplicationCommandName>(
  name: Name,
  input: ApplicationCommandInput<Name>,
): readonly ApplicationResourceKey[] {
  switch (name) {
    case 'listAvailableAgents':
    case 'inspectAgentInstallation':
    case 'resolveCatalogAgent':
    case 'listAgentInstallJobs':
    case 'installAgent':
    case 'cancelAgentInstall':
    case 'listAgentConfigs':
    case 'saveAgentConfig':
    case 'deleteAgentConfig':
    case 'checkAgentConfig':
      return [{ kind: 'agent_configurations' }]
    case 'createManagedSession':
    case 'prepareManagedSession':
      return [{ kind: 'navigation' }]
    case 'listManagedSessionActivity':
    case 'getManagedSession':
    case 'getManagedFeedbackStatus':
    case 'getManagedWorkspaceInfo':
    case 'startManagedSession':
    case 'stopManagedSession':
    case 'cancelManagedPrompt':
    case 'sendManagedPrompt':
    case 'sendManagedPromptContent':
    case 'setManagedSessionConfig':
    case 'respondManagedPermission':
    case 'resolveFeedbackDelivery':
    case 'deleteManagedSession':
    case 'discardPreparedSession':
      return [{ kind: 'managed_session', session_id: canonicalUuid((input as { session_id: string }).session_id.trim()) }]
    case 'getFeedbackWorkspace':
    case 'saveFeedbackDraft':
    case 'addFeedbackAttachment':
    case 'removeFeedbackAttachment':
    case 'reorderFeedbackAttachments':
    case 'approveFeedbackRequest':
    case 'readFeedbackAttachment':
    case 'readRequestAttachment':
      return [requestResource((input as { request_id: string }).request_id)]
    case 'readPublishedFeedback':
      return [publishedResource((input as { request_id: string }).request_id)]
    case 'submitFeedback':
    case 'cancelFeedbackRequest': {
      const requestId = (input as { request_id: string }).request_id
      return [requestResource(requestId), publishedResource(requestId)]
    }
    case 'deleteFeedbackRequest': {
      const requestId = (input as { request_id: string }).request_id
      return [requestResource(requestId), publishedResource(requestId)]
    }
    case 'deleteHostSession':
      return [hostSessionResource(input as { host_id: string; host_session_id: string })]
    case 'listFeedbackRequests': {
      const listInput = input as ApplicationCommandInput<'listFeedbackRequests'>
      return listInput.host_id && listInput.host_session_id
        ? [{ kind: 'navigation' }, hostSessionResource({
            host_id: listInput.host_id,
            host_session_id: listInput.host_session_id,
          }, false)]
        : [{ kind: 'navigation' }]
    }
    case 'listFeedbackInbox':
    case 'listHostSessions':
    case 'listArchivedHostSessions':
    case 'listHostProfiles':
    case 'renameHostSession':
    case 'setHostSessionPinned':
    case 'archiveHostSession':
    case 'unarchiveHostSession':
    case 'setHostPinned':
      return [{ kind: 'navigation' }]
  }
}

function projectionKey(name: ApplicationCommandName, ...scope: unknown[]): string {
  return JSON.stringify([name, ...scope])
}

export function applicationCommandProjectionKey<Name extends ApplicationCommandName>(
  name: Name,
  input: ApplicationCommandInput<Name>,
): string {
  switch (name) {
    case 'listManagedSessionActivity': {
      const page = input as ApplicationCommandInput<'listManagedSessionActivity'>
      return projectionKey(name, page.session_id, String(page.before_sequence), String(page.limit ?? 100))
    }
    case 'listAvailableAgents':
    case 'listAgentInstallJobs':
    case 'listAgentConfigs':
      return projectionKey(name)
    case 'resolveCatalogAgent': {
      const resolve = input as ApplicationCommandInput<'resolveCatalogAgent'>
      return projectionKey(name, resolve.agent_id, resolve.agent_config_id ?? null)
    }
    case 'inspectAgentInstallation':
    case 'installAgent':
      return projectionKey(name, (input as CatalogInput).agent_id)
    case 'cancelAgentInstall':
      return projectionKey(name, (input as { job_id: string }).job_id)
    case 'saveAgentConfig':
      return projectionKey(name, (input as ApplicationCommandInput<'saveAgentConfig'>).id)
    case 'deleteAgentConfig':
    case 'checkAgentConfig':
      return projectionKey(name, canonicalUuid((input as { agent_config_id: string }).agent_config_id.trim()))
    case 'prepareManagedSession':
    case 'createManagedSession': {
      const createInput = input as ApplicationCommandInput<'createManagedSession'>
      return projectionKey(name, canonicalUuid(createInput.agent_config_id.trim()), createInput.cwd.trim())
    }
    case 'getManagedSession':
    case 'getManagedFeedbackStatus':
    case 'getManagedWorkspaceInfo':
    case 'startManagedSession':
    case 'stopManagedSession':
    case 'cancelManagedPrompt':
    case 'sendManagedPrompt':
    case 'sendManagedPromptContent':
    case 'setManagedSessionConfig':
    case 'respondManagedPermission':
    case 'resolveFeedbackDelivery':
    case 'deleteManagedSession':
    case 'discardPreparedSession':
      return projectionKey(name, canonicalUuid((input as { session_id: string }).session_id.trim()))
    case 'listFeedbackInbox':
    case 'listHostSessions':
    case 'listHostProfiles':
      return projectionKey(name)
    case 'listArchivedHostSessions': {
      const listInput = input as ApplicationCommandInput<'listArchivedHostSessions'>
      return projectionKey(name, canonicalSearch(listInput.search))
    }
    case 'listFeedbackRequests': {
      const listInput = input as ApplicationCommandInput<'listFeedbackRequests'>
      const statuses = [...new Set(listInput.status ?? ['waiting', 'in_progress'])].sort()
      return projectionKey(
        name,
        listInput.host_id,
        listInput.host_session_id,
        statuses,
        listInput.archived ?? false,
        canonicalSearch(listInput.search),
        listInput.limit ?? 50,
        canonicalCursor(listInput.cursor),
      )
    }
    case 'readFeedbackAttachment':
    case 'readRequestAttachment': {
      const readInput = input as ApplicationCommandInput<'readFeedbackAttachment'>
      return projectionKey(
        name,
        canonicalUuid(readInput.request_id),
        canonicalUuid(readInput.attachment_id),
      )
    }
    case 'getFeedbackWorkspace':
    case 'readPublishedFeedback':
    case 'saveFeedbackDraft':
    case 'addFeedbackAttachment':
    case 'removeFeedbackAttachment':
    case 'reorderFeedbackAttachments':
    case 'submitFeedback':
    case 'approveFeedbackRequest':
    case 'cancelFeedbackRequest':
    case 'deleteFeedbackRequest':
      return projectionKey(name, canonicalUuid((input as { request_id: string }).request_id))
    case 'renameHostSession':
    case 'setHostSessionPinned':
    case 'archiveHostSession':
    case 'unarchiveHostSession':
    case 'deleteHostSession': {
      const sessionInput = input as { host_id: string; host_session_id: string }
      return projectionKey(name, sessionInput.host_id.trim(), sessionInput.host_session_id.trim())
    }
    case 'setHostPinned':
      return projectionKey(
        name,
        (input as ApplicationCommandInput<'setHostPinned'>).host_id.trim(),
      )
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

export class HttpApplicationTransport implements ApplicationTransport {
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
