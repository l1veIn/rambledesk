import hakimiAudioUrl from '../assets/hakimi.mp3'

import type { FeedbackRequestSummary } from './feedback'
import type { CustomNotificationSound, Locale, NotificationSound } from './preferences'

export type NotificationState = 'checking' | 'enabled' | 'muted' | 'disabled' | 'unavailable'

export const MAX_CUSTOM_SOUND_SECONDS = 10
export const MAX_CUSTOM_SOUND_BYTES = 5 * 1024 * 1024

type AudioContextConstructor = new () => AudioContext

let notificationAudioContext: AudioContext | null = null
let customSoundCache: { id: string; buffer: AudioBuffer } | null = null
let activeCustomSource: AudioBufferSourceNode | null = null
let builtinAudio: HTMLAudioElement | null = null

const notificationSounds: Record<
  Exclude<NotificationSound, 'custom' | 'hakimi'>,
  { frequencies: readonly [number, number][]; duration: number; volume: number; wave: OscillatorType }
> = {
  chime: {
    frequencies: [[880, 0], [1174.66, 0.09]],
    duration: 0.5,
    volume: 0.08,
    wave: 'sine',
  },
  soft: {
    frequencies: [[523.25, 0], [659.25, 0.14]],
    duration: 0.7,
    volume: 0.055,
    wave: 'sine',
  },
  alert: {
    frequencies: [[783.99, 0], [783.99, 0.16], [1046.5, 0.32]],
    duration: 0.7,
    volume: 0.07,
    wave: 'triangle',
  },
}

async function resolveAudioContext(): Promise<AudioContext | null> {
  if (typeof window === 'undefined') return null
  const audioWindow = window as typeof window & {
    webkitAudioContext?: AudioContextConstructor
  }
  const AudioContextClass = window.AudioContext ?? audioWindow.webkitAudioContext
  if (!AudioContextClass) return null
  try {
    notificationAudioContext ??= new AudioContextClass()
    if (notificationAudioContext.state === 'suspended') {
      await notificationAudioContext.resume()
    }
    return notificationAudioContext
  } catch {
    return null
  }
}

async function decodeCustomSound(
  context: AudioContext,
  id: string,
  readCustomSound: (id: string) => Promise<ArrayBuffer>,
): Promise<AudioBuffer | null> {
  if (customSoundCache?.id === id) return customSoundCache.buffer
  try {
    const bytes = await readCustomSound(id)
    if (bytes.byteLength === 0) return null
    const buffer = await context.decodeAudioData(bytes.slice(0))
    customSoundCache = { id, buffer }
    return buffer
  } catch {
    customSoundCache = null
    return null
  }
}

async function playCustomNotificationSound(
  context: AudioContext,
  custom: CustomNotificationSound | null,
  volume: number,
  readCustomSound: ((id: string) => Promise<ArrayBuffer>) | undefined,
): Promise<boolean> {
  if (!custom || !readCustomSound) return false
  try {
    const buffer = await decodeCustomSound(context, custom.id, readCustomSound)
    if (!buffer) return false
    const source = context.createBufferSource()
    source.buffer = buffer
    const normalizedVolume = Math.min(100, Math.max(0, volume)) / 100
    const gain = context.createGain()
    gain.gain.setValueAtTime(Math.min(1, normalizedVolume), context.currentTime)
    gain.connect(context.destination)
    source.connect(gain)
    activeCustomSource?.stop()
    activeCustomSource = source
    source.onended = () => {
      if (activeCustomSource === source) activeCustomSource = null
    }
    const playSeconds = Math.min(buffer.duration, MAX_CUSTOM_SOUND_SECONDS)
    source.start(0, 0, playSeconds)
    return true
  } catch {
    return false
  }
}

type NotificationSoundPreset = {
  frequencies: readonly [number, number][]
  duration: number
  volume: number
  wave: OscillatorType
}

function playPresetSound(
  context: AudioContext,
  preset: NotificationSoundPreset,
  volume: number,
): void {
  try {
    const now = context.currentTime
    const gain = context.createGain()
    const normalizedVolume = Math.min(100, Math.max(0, volume)) / 100
    const outputVolume = Math.max(0.0001, preset.volume * 2.75 * normalizedVolume)
    gain.gain.setValueAtTime(0.0001, now)
    gain.gain.exponentialRampToValueAtTime(outputVolume, now + 0.015)
    gain.gain.exponentialRampToValueAtTime(0.0001, now + preset.duration)
    gain.connect(context.destination)

    for (const [frequency, delay] of preset.frequencies) {
      const oscillator = context.createOscillator()
      oscillator.type = preset.wave
      oscillator.frequency.setValueAtTime(frequency, now + delay)
      oscillator.connect(gain)
      oscillator.start(now + delay)
      oscillator.stop(now + preset.duration)
    }
  } catch {
    // The OS notification remains useful when audio playback is unavailable.
  }
}

/** Play the bundled built-in audio clip (the trimmed 10-second test asset). */
async function playBuiltinAudio(volume: number): Promise<void> {
  try {
    builtinAudio ??= new Audio(hakimiAudioUrl)
    builtinAudio.volume = Math.min(1, Math.max(0, volume / 100))
    builtinAudio.currentTime = 0
    await builtinAudio.play()
  } catch {
    // Fall back to the synthesized chime when the bundled clip cannot play
    // (for example, autoplay was blocked before any user gesture).
    const context = await resolveAudioContext()
    if (context) playPresetSound(context, notificationSounds.chime, volume)
  }
}

/** Short arming tick when recording starts. */
export async function playRecordArmSound(volume = 80): Promise<void> {
  const context = await resolveAudioContext()
  if (!context) return
  try {
    const now = context.currentTime
    const normalizedVolume = Math.min(100, Math.max(0, volume)) / 100
    const osc = context.createOscillator()
    osc.type = 'sine'
    osc.frequency.setValueAtTime(1560, now)
    osc.frequency.exponentialRampToValueAtTime(880, now + 0.07)
    const gain = context.createGain()
    gain.gain.setValueAtTime(0.0001, now)
    gain.gain.exponentialRampToValueAtTime(Math.max(0.0001, 0.42 * normalizedVolume), now + 0.008)
    gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.09)
    osc.connect(gain)
    gain.connect(context.destination)
    osc.start(now)
    osc.stop(now + 0.1)
  } catch {
    // Interaction audio is optional.
  }
}

export async function playNotificationSound(
  sound: NotificationSound = 'chime',
  volume = 80,
  custom: CustomNotificationSound | null = null,
  readCustomSound?: (id: string) => Promise<ArrayBuffer>,
): Promise<void> {
  if (sound === 'hakimi') {
    await playBuiltinAudio(volume)
    return
  }

  const context = await resolveAudioContext()
  if (!context) return

  if (sound === 'custom') {
    if (!(await playCustomNotificationSound(context, custom, volume, readCustomSound))) {
      // The custom file is missing or undecodable; keep the reminder audible.
      playPresetSound(context, notificationSounds.chime, volume)
    }
    return
  }
  playPresetSound(context, notificationSounds[sound], volume)
}

/**
 * Decode the bytes of an imported custom sound, cache the buffer for playback,
 * and return its duration so the caller can enforce the length limit.
 * Throws when the bytes cannot be decoded as audio.
 */
export async function decodeCustomSoundBytes(
  id: string,
  bytes: number[],
): Promise<{ duration: number }> {
  const context = await resolveAudioContext()
  if (!context) throw new Error('audio-unavailable')
  const buffer = await context.decodeAudioData(new Uint8Array(bytes).buffer)
  customSoundCache = { id, buffer }
  return { duration: buffer.duration }
}

export function discardCustomSoundCache(): void {
  customSoundCache = null
  activeCustomSource?.stop()
  activeCustomSource = null
}

export function notificationStateForPermission(
  granted: boolean,
  preferred: boolean,
): NotificationState {
  return granted ? (preferred ? 'enabled' : 'muted') : 'disabled'
}

export function collectNewRequests(
  knownRequestIds: Set<string>,
  requests: FeedbackRequestSummary[],
): FeedbackRequestSummary[] {
  const arrivals = requests.filter((request) => !knownRequestIds.has(request.request_id))
  for (const request of requests) knownRequestIds.add(request.request_id)
  return arrivals
}

export class InboxNotificationTracker {
  private initialized = false
  private readonly knownRequestIds = new Set<string>()

  observe(requests: FeedbackRequestSummary[]): FeedbackRequestSummary[] {
    const arrivals = collectNewRequests(this.knownRequestIds, requests)
    if (!this.initialized) {
      this.initialized = true
      return []
    }
    return arrivals
  }
}

export function notificationLabel(state: NotificationState, locale: Locale = 'zh-CN'): string {
  if (locale === 'en') {
    switch (state) {
      case 'checking':
        return 'Checking notifications…'
      case 'enabled':
        return 'Notifications enabled'
      case 'muted':
        return 'Notifications paused — click to enable'
      case 'disabled':
        return 'Enable notifications'
      case 'unavailable':
        return 'Notifications unavailable'
    }
  }
  switch (state) {
    case 'checking':
      return '检查通知…'
    case 'enabled':
      return '通知已开启'
    case 'muted':
      return '通知已暂停，点击重新开启'
    case 'disabled':
      return '启用通知'
    case 'unavailable':
      return '通知不可用'
  }
}
