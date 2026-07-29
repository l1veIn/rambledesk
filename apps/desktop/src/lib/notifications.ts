import type { FeedbackRequestSummary } from './feedback'

export type NotificationState = 'checking' | 'enabled' | 'disabled' | 'unavailable'

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

export function notificationLabel(state: NotificationState): string {
  switch (state) {
    case 'checking':
      return '检查通知…'
    case 'enabled':
      return '通知已开启'
    case 'disabled':
      return '启用通知'
    case 'unavailable':
      return '通知不可用'
  }
}
