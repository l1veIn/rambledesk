import type { Locale, SpeechModelId } from './preferences'

type EnglishModelCopy = {
  displayName: string
  description: string
  languages: Record<string, string>
}

const englishCopy: Partial<Record<SpeechModelId, EnglishModelCopy>> = {
  'x-asr-480ms-streaming-zh-en-punct-int8-2026-06-05': {
    displayName: 'X-ASR streaming Chinese/English punctuation',
    description: 'Low-latency live transcription for continuous Rambles that need streaming feedback.',
    languages: { '中文': 'Chinese', English: 'English' },
  },
  'sense-voice-zh-en-ja-ko-yue-2024-07-17': {
    displayName: 'SenseVoice multilingual',
    description: 'Recommended default with reliable multilingual transcription after automatic VAD splitting.',
    languages: {
      '中文': 'Chinese',
      English: 'English',
      '日本語': 'Japanese',
      '한국어': 'Korean',
      '粤语': 'Cantonese',
    },
  },
  'funasr-nano-int8-2025-12-30': {
    displayName: 'FunASR-Nano Chinese/English/Japanese',
    description: 'High-quality non-streaming transcription with VAD splitting; it needs a larger download and more memory.',
    languages: { '中文': 'Chinese', English: 'English', '日本語': 'Japanese' },
  },
  'zipformer-small-streaming-zh-en-ctc-int8-2026-06-18': {
    displayName: 'Zipformer Small streaming Chinese/English (Browser experimental)',
    description: 'Browser-local streaming transcription. The model stays in this browser and audio is never uploaded.',
    languages: { '中文': 'Chinese', English: 'English' },
  },
}

export function speechModelDisplayName(locale: Locale, id: SpeechModelId, fallback: string) {
  return locale === 'en' ? (englishCopy[id]?.displayName ?? fallback) : fallback
}

export function speechModelDescription(locale: Locale, id: SpeechModelId, fallback: string) {
  return locale === 'en' ? (englishCopy[id]?.description ?? fallback) : fallback
}

export function speechModelLanguages(locale: Locale, id: SpeechModelId, languages: string[]) {
  if (locale !== 'en') return languages
  const copy = englishCopy[id]
  return languages.map((language) => copy?.languages[language] ?? language)
}
