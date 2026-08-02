import type { FeedbackRequestSummary } from './feedback'
import type { Locale, NotificationSound } from './preferences'

export type NotificationState = 'checking' | 'enabled' | 'muted' | 'disabled' | 'unavailable'

type AudioContextConstructor = new () => AudioContext

let notificationAudioContext: AudioContext | null = null

const notificationSounds: Record<
  NotificationSound,
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

export async function playNotificationSound(
  sound: NotificationSound = 'chime',
  volume = 80,
): Promise<void> {
  if (typeof window === 'undefined') return
  const audioWindow = window as typeof window & {
    webkitAudioContext?: AudioContextConstructor
  }
  const AudioContextClass = window.AudioContext ?? audioWindow.webkitAudioContext
  if (!AudioContextClass) return

  try {
    notificationAudioContext ??= new AudioContextClass()
    if (notificationAudioContext.state === 'suspended') {
      await notificationAudioContext.resume()
    }
    const preset = notificationSounds[sound]
    const now = notificationAudioContext.currentTime
    const gain = notificationAudioContext.createGain()
    const normalizedVolume = Math.min(100, Math.max(0, volume)) / 100
    const outputVolume = Math.max(0.0001, preset.volume * 2.75 * normalizedVolume)
    gain.gain.setValueAtTime(0.0001, now)
    gain.gain.exponentialRampToValueAtTime(outputVolume, now + 0.015)
    gain.gain.exponentialRampToValueAtTime(0.0001, now + preset.duration)
    gain.connect(notificationAudioContext.destination)

    for (const [frequency, delay] of preset.frequencies) {
      const oscillator = notificationAudioContext.createOscillator()
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
