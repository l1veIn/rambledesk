import type { Locale, SpeechModelId } from './preferences'

type EnglishModelCopy = {
  displayName: string
  description: string
  languages: Record<string, string>
}

const englishCopy: Partial<Record<SpeechModelId, EnglishModelCopy>> = {
  'x-asr-480ms-streaming-zh-en-punct-int8-2026-06-05': {
    displayName: 'X-ASR streaming Chinese/English punctuation',
    description: 'Low-latency live transcription for continuous Rambles.',
    languages: { '中文': 'Chinese', English: 'English' },
  },
  'sense-voice-zh-en-ja-ko-yue-2024-07-17': {
    displayName: 'SenseVoice multilingual',
    description: 'Whole-segment transcription after automatic VAD splitting, with multilingual accuracy.',
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
