import type { SpeechModelInfo } from '../../workbenchCapabilities'

export const BROWSER_SPEECH_MODEL_ID =
  'zipformer-small-streaming-zh-en-ctc-int8-2026-06-18' as const
export const BROWSER_SPEECH_MODEL_DIGEST =
  'bb1b0ec05bd728f0732c8c0d313ce801ea8afa77453c091f841901dd242b1f44'
export const BROWSER_SPEECH_CACHE_NAME =
  `rambledesk-browser-speech-${BROWSER_SPEECH_MODEL_ID}-${BROWSER_SPEECH_MODEL_DIGEST}`
export const BROWSER_SPEECH_MARKER_URL =
  'https://rambledesk.invalid/browser-speech/installed.json'
export const BROWSER_SPEECH_TOTAL_BYTES = 29_258_848

const MODELSCOPE_BASE =
  'https://www.modelscope.cn/models/pkufool/zipformer-small-streaming'
const MODELSCOPE_REVISION = '8de27eb36a6f767eea91f2ff1f5660ad499ed688'

export type BrowserSpeechModelFile = Readonly<{
  name: string
  bytes: number
  sha256: string
  url: string
}>

export const BROWSER_SPEECH_MODEL_FILES: readonly BrowserSpeechModelFile[] = Object.freeze([
  modelFile('ctc-chunk-32-left-128.int8.onnx', 29_144_714, 'c82f67125c87e06b2cdb11e7ac3d5a7d3d81539634e384b4b60ccfb8cc1f3c82'),
  modelFile('tokens.txt', 114_134, '5beaddf82e078ef5644dcad130f3c4817a04016bfb0f31c0df28c6b4fa8df3af', 'data/tokens.txt'),
])

export const BROWSER_SPEECH_RUNTIME = Object.freeze({
  version: '1.13.7',
  gitSha: '917bed95c8e5c7c18aa4d69fea42e9ef8ef0a60e',
  root: '/browser-speech/runtime',
  glue: '/browser-speech/runtime/sherpa-onnx-wasm-web.js',
  wasm: '/browser-speech/runtime/sherpa-onnx-wasm-web.wasm',
  wrapper: '/browser-speech/runtime/sherpa-onnx-asr.js',
  worker: '/browser-speech/sherpa.worker.js',
  worklet: '/browser-speech/pcm-capture.worklet.js',
  wasmBytes: 14_869_666,
  wasmSha256: 'f0bd7239906d96a5aff87b523879b898c0522d58c2c7ac2f8795b74186dc9c99',
})

export function browserSpeechModelInfo(installed: boolean, missingFiles: readonly string[]): SpeechModelInfo {
  return Object.freeze({
    id: BROWSER_SPEECH_MODEL_ID,
    engine_id: 'sherpa_online',
    display_name: 'Zipformer Small 流式中英（Browser experimental）',
    description: '浏览器本地识别；模型下载到此浏览器的 Cache Storage，音频不会上传。',
    size_bytes: BROWSER_SPEECH_TOTAL_BYTES,
    installed,
    path: installed
      ? `Cache Storage: ${BROWSER_SPEECH_CACHE_NAME}`
      : 'Browser Cache Storage (not installed)',
    missing_files: missingFiles,
    streaming: true,
    hotwords_supported: false,
    languages: ['中文', 'English'],
    license: 'Apache-2.0 (ModelScope repository metadata)',
  })
}

function modelFile(name: string, bytes: number, sha256: string, remotePath = name): BrowserSpeechModelFile {
  return Object.freeze({
    name,
    bytes,
    sha256,
    url: `${MODELSCOPE_BASE}/resolve/${MODELSCOPE_REVISION}/${remotePath}`,
  })
}
