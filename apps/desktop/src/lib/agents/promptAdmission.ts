/** These backend admission errors occur before a user message is persisted.
 * A transport failure, generic exception, or an empty read cannot prove rejection. */
const rejectionCodes = new Set([
  'MANAGED_SESSION_BUSY', 'MANAGED_SESSION_NOT_CONNECTED',
])

export function promptRejectionConfirmed(cause: unknown): boolean {
  if (!cause || typeof cause !== 'object') return false
  const error = cause as Record<string, unknown>
  return typeof error.code === 'string' && rejectionCodes.has(error.code)
    && typeof error.message === 'string' && typeof error.retryable === 'boolean'
}
