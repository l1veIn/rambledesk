export const APPLICATION_READ_TIMEOUT_MS = 30_000

export class ApplicationReadTimeoutError extends Error {
  readonly code = 'TIMEOUT'

  constructor(readonly operation: string) {
    super(`Application read timed out after 30 seconds (${operation}).`)
    this.name = 'ApplicationReadTimeoutError'
  }
}

/** Bound a read only. A timeout never cancels or repeats an application mutation. */
export async function withApplicationReadTimeout<Result>(
  read: Promise<Result>,
  operation: string,
): Promise<Result> {
  let timer: ReturnType<typeof setTimeout> | undefined
  try {
    return await Promise.race([
      read,
      new Promise<never>((_, reject) => {
        timer = setTimeout(() => reject(new ApplicationReadTimeoutError(operation)), APPLICATION_READ_TIMEOUT_MS)
      }),
    ])
  } finally {
    if (timer !== undefined) clearTimeout(timer)
  }
}
