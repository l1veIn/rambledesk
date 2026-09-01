import type { BrowserSpeechModelFile } from './browserSpeechManifest'

export const BROWSER_SPEECH_PROTOCOL_VERSION = 1 as const

export type BrowserSpeechWorkerCommand =
  | Readonly<{
      v: 1
      type: 'init'
      sessionId: string
      cacheName: string
      runtime: Readonly<{ glue: string; wrapper: string; wasm: string; wasmBytes: number; wasmSha256: string; version: string; gitSha: string }>
      files: readonly BrowserSpeechModelFile[]
      options: Readonly<{ vadSilenceMs: number }>
    }>
  | Readonly<{ v: 1; type: 'bindPcm'; sessionId: string; port: MessagePort }>
  | Readonly<{ v: 1; type: 'stop'; sessionId: string; lastSeq: number }>
  | Readonly<{ v: 1; type: 'cancel'; sessionId: string }>

export type BrowserSpeechWorkerEvent =
  | Readonly<{ v: 1; type: 'ready'; sessionId: string; runtimeVersion: string; runtimeGitSha: string; ortVersion: string }>
  | Readonly<{ v: 1; type: 'partial'; sessionId: string; text: string }>
  | Readonly<{ v: 1; type: 'processing'; sessionId: string; segmentIndex: number }>
  | Readonly<{ v: 1; type: 'stable'; sessionId: string; segmentIndex: number; text: string }>
  | Readonly<{ v: 1; type: 'warning'; sessionId: string; code: string; message: string }>
  | Readonly<{ v: 1; type: 'disposed'; sessionId: string; reason: 'stopped' | 'cancelled' }>
  | Readonly<{ v: 1; type: 'fatal'; sessionId: string; code: string; message: string }>

export type BrowserSpeechPcmMessage = Readonly<{
  v: 1
  type: 'pcm'
  sessionId: string
  seq: number
  sampleRate: number
  samples: ArrayBuffer
}>

export function isWorkerEvent(value: unknown, sessionId: string): value is BrowserSpeechWorkerEvent {
  if (value === null || typeof value !== 'object') return false
  const message = value as Record<string, unknown>
  return message.v === BROWSER_SPEECH_PROTOCOL_VERSION &&
    message.sessionId === sessionId && typeof message.type === 'string'
}
