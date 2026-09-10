/** Extract a human-readable message from an unknown thrown value. */
export function messageFrom(cause: unknown): string {
  if (cause instanceof Error) return cause.message
  if (
    cause &&
    typeof cause === 'object' &&
    'message' in cause &&
    typeof (cause as { message: unknown }).message === 'string'
  ) {
    return (cause as { message: string }).message
  }
  return String(cause)
}
