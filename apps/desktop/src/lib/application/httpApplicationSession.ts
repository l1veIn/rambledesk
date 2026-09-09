import type { ApplicationCommandName } from './contracts'
import type {
  SubscriptionErrorHandler,
  Unsubscribe,
} from './applicationTransport'
import type {
  ApplicationEvent,
  ApplicationResourceKey,
  ApplicationSnapshotMetadata,
} from '../generated/feedback'
import {
  APPLICATION_EVENT_CREDENTIAL_PROTOCOL_PREFIX,
  APPLICATION_EVENT_PROTOCOL,
  REVISION_HEADER,
  RUNTIME_GENERATION_HEADER,
  applicationResourceKeyIdentity,
  parseApplicationEvent,
} from './applicationEvents'
import {
  MUTATION_COMMANDS,
  isAuthenticationRejection,
  parseRevision,
  projectionKey,
  type HttpApplicationOperation,
} from './httpApplicationOperations'

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
    // The constructor or disconnect already created this barrier. Commands may
    // join it during reconnect backoff; replacing it here would strand them even
    // after ready, revocation or another connection failure settles the new one.
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
