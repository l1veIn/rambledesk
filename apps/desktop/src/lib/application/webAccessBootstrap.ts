export class WebAccessTokenRejectedError extends Error {
  constructor() {
    super('The Web Access token was not accepted.')
    this.name = 'WebAccessTokenRejectedError'
  }
}

export class WebAccessConnectionError extends Error {
  constructor(readonly status: number | null = null) {
    super('Web Access could not create a browser session.')
    this.name = 'WebAccessConnectionError'
  }
}

export type WebAccessBootstrapOptions = Readonly<{
  token: string
  pageUrl?: string | URL
  fetch?: typeof globalThis.fetch
}>

export async function bootstrapWebAccessSession(
  options: WebAccessBootstrapOptions,
): Promise<string> {
  const pageUrl = new URL(options.pageUrl ?? globalThis.location.href)
  const bootstrapUrl = new URL('/api/auth/session', pageUrl)
  if (bootstrapUrl.origin !== pageUrl.origin) {
    throw new Error('Web Access bootstrap must remain same-origin.')
  }
  let response: Response
  try {
    response = await (options.fetch ?? globalThis.fetch.bind(globalThis))(bootstrapUrl, {
      method: 'POST',
      headers: { Authorization: `Bearer ${options.token}` },
      credentials: 'same-origin',
      redirect: 'error',
    })
  } catch {
    throw new WebAccessConnectionError()
  }
  if (response.status === 401 || response.status === 403) {
    throw new WebAccessTokenRejectedError()
  }
  if (!response.ok) {
    throw new WebAccessConnectionError(response.status)
  }
  let body: unknown
  try {
    body = await response.json()
  } catch {
    throw new WebAccessConnectionError(response.status)
  }
  const token =
    typeof body === 'object' && body !== null && 'session_token' in body
      ? (body as { session_token?: unknown }).session_token
      : undefined
  if (typeof token !== 'string' || !/^[A-Za-z0-9_-]+$/u.test(token)) {
    throw new WebAccessConnectionError(response.status)
  }
  return token
}
