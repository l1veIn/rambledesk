import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'

describe('browser speech architecture boundary', () => {
  it('keeps capture in the worklet, recognition/resampling in the Worker, and has no audio upload route', async () => {
    const publicRoot = fileURLToPath(new URL('../../../../../public/browser-speech/', import.meta.url))
    const [worklet, worker] = await Promise.all([
      readFile(`${publicRoot}/pcm-capture.worklet.js`, 'utf8'),
      readFile(`${publicRoot}/sherpa.worker.js`, 'utf8'),
    ])
    expect(worklet).toContain("registerProcessor('rambledesk-pcm-capture'")
    expect(worklet).not.toMatch(/createOnlineRecognizer|fetch\(/u)
    expect(worker).toMatch(/class StreamingResampler/u)
    expect(worker).toMatch(/createOnlineRecognizer\(Module/u)
    expect(worker).toMatch(/zipformer2Ctc/u)
    expect(worker).toMatch(/printErr: captureEngineDiagnostic/u)
    expect(worker).toMatch(/actual\.length >= 8 && expected\.startsWith\(actual\)/u)
    expect(worker).toMatch(/cache\.delete\(file\.url\)/u)
    expect(worker).toMatch(/cache\.delete\(markerUrl\)/u)
    expect(worker).not.toMatch(/\/api\/speech|FormData|audio\/wav/u)
  })
})
