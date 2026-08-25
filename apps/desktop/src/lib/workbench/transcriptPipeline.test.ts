import { describe, expect, it } from 'vitest'

import { createTranscriptPipeline } from './transcriptPipeline'

describe('createTranscriptPipeline', () => {
  it('writes the original text when cleanup is off', async () => {
    const written: string[] = []
    const pipeline = createTranscriptPipeline({
      cleanupEnabled: () => false,
      cleanup: async () => {
        throw new Error('should not run')
      },
      write: async (text) => {
        written.push(text)
      },
    })

    await pipeline.enqueue('  raw speech  ')
    expect(written).toEqual(['raw speech'])
  })

  it('writes cleaned text when cleanup is on', async () => {
    const written: string[] = []
    const pipeline = createTranscriptPipeline({
      cleanupEnabled: () => true,
      cleanup: async (text) => `clean:${text}`,
      write: async (text) => {
        written.push(text)
      },
    })

    await pipeline.enqueue('hello')
    expect(written).toEqual(['clean:hello'])
  })

  it('falls back to the original text and reports the error when cleanup fails', async () => {
    const written: string[] = []
    const errors: string[] = []
    const pipeline = createTranscriptPipeline({
      cleanupEnabled: () => true,
      cleanup: async () => {
        throw new Error('model down')
      },
      write: async (text) => {
        written.push(text)
      },
      onError: (message) => {
        errors.push(message)
      },
    })

    await pipeline.enqueue('keep this')
    expect(written).toEqual(['keep this'])
    expect(errors).toEqual(['model down'])
  })

  it('keeps enqueue order when a later cleanup finishes first', async () => {
    const written: string[] = []
    let releaseFirst: (() => void) | undefined
    const firstGate = new Promise<void>((resolve) => {
      releaseFirst = resolve
    })
    const pipeline = createTranscriptPipeline({
      cleanupEnabled: () => true,
      cleanup: async (text) => {
        if (text === 'first') await firstGate
        return text.toUpperCase()
      },
      write: async (text) => {
        written.push(text)
      },
    })

    const first = pipeline.enqueue('first')
    const second = pipeline.enqueue('second')
    await Promise.resolve()
    expect(written).toEqual([])
    releaseFirst?.()
    await Promise.all([first, second])
    expect(written).toEqual(['FIRST', 'SECOND'])
  })

  it('ignores blank transcripts', async () => {
    const written: string[] = []
    const pipeline = createTranscriptPipeline({
      cleanupEnabled: () => true,
      cleanup: async (text) => text,
      write: async (text) => {
        written.push(text)
      },
    })
    await pipeline.enqueue('   ')
    expect(written).toEqual([])
  })
})
