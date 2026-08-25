import { messageFrom } from './feedbackText'

export type TranscriptPipeline = {
  enqueue(text: string): Promise<string>
  prepare(text: string): Promise<string>
}

/**
 * A cleanup call that never settles would strand every later segment behind it
 * in the queue, leaving a clip spinning with nothing written to the document.
 * Past this budget the raw transcript wins.
 */
export const CLEANUP_TIMEOUT_MS = 30_000

export function createTranscriptPipeline(options: {
  cleanupEnabled: () => boolean
  cleanup: (text: string) => Promise<string>
  write: (text: string) => void | Promise<void>
  onError?: (message: string) => void
  timeoutMs?: number
  onTimeout?: () => string
}): TranscriptPipeline {
  let queue = Promise.resolve()
  const timeoutMs = options.timeoutMs ?? CLEANUP_TIMEOUT_MS

  const TIMED_OUT = Symbol('cleanup-timed-out')

  async function withTimeout(work: Promise<string>): Promise<string | typeof TIMED_OUT> {
    let timer: ReturnType<typeof setTimeout> | undefined
    const expiry = new Promise<typeof TIMED_OUT>((resolve) => {
      timer = setTimeout(() => resolve(TIMED_OUT), timeoutMs)
    })
    try {
      return await Promise.race([work, expiry])
    } finally {
      if (timer) clearTimeout(timer)
    }
  }

  async function transform(text: string): Promise<string> {
    const transcript = text.trim()
    if (!transcript) return ''
    if (!options.cleanupEnabled()) return transcript
    try {
      const cleaned = await withTimeout(options.cleanup(transcript))
      if (cleaned === TIMED_OUT) {
        options.onError?.(options.onTimeout?.() ?? 'Light cleanup timed out; kept the raw transcript.')
        return transcript
      }
      return cleaned.trim() || transcript
    } catch (cause) {
      options.onError?.(messageFrom(cause))
      return transcript
    }
  }

  function run<T>(work: () => Promise<T>): Promise<T> {
    const operation = queue.then(work)
    queue = operation.then(
      () => undefined,
      () => undefined,
    )
    return operation
  }

  function prepare(text: string): Promise<string> {
    return run(() => transform(text))
  }

  function enqueue(text: string): Promise<string> {
    return run(async () => {
      const output = await transform(text)
      if (output) await options.write(output)
      return output
    })
  }

  return { enqueue, prepare }
}
