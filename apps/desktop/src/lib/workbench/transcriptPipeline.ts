import { messageFrom } from './feedbackText'

export type TranscriptPipeline = {
  enqueue(text: string): Promise<string>
  prepare(text: string): Promise<string>
}

export function createTranscriptPipeline(options: {
  cleanupEnabled: () => boolean
  cleanup: (text: string) => Promise<string>
  write: (text: string) => void | Promise<void>
  onError?: (message: string) => void
}): TranscriptPipeline {
  let queue = Promise.resolve()

  async function transform(text: string): Promise<string> {
    const transcript = text.trim()
    if (!transcript) return ''
    if (!options.cleanupEnabled()) return transcript
    try {
      const cleaned = (await options.cleanup(transcript)).trim()
      return cleaned || transcript
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
