import type { FeedbackRequestSummary } from './feedback'
import type { Locale } from './preferences'

export type NotificationState = 'checking' | 'enabled' | 'muted' | 'disabled' | 'unavailable'

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
