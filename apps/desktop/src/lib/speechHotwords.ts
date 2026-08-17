export const DEFAULT_SPEECH_HOTWORDS = [
  'ramble',
  'RambleDesk',
  'Rambelle',
  'Cooking',
  'Claude Code',
  'Codex',
  'Grok',
  'Gemini',
  'DeepSeek',
  'Reasonix',
  'Pi',
  'dsh',
  'MCP',
  'FunASR',
  'SenseVoice'
]

export function mergeSpeechHotwords(existing: string[], defaults: string[]): string[] {
  const seen = new Set(existing.map((word) => word.toLowerCase()))
  const extra = defaults.filter((word) => !seen.has(word.toLowerCase()))
  return extra.length === 0 ? existing : [...existing, ...extra]
}
