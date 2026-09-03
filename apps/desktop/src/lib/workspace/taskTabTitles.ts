import type { FeedbackRequestSummary } from '$lib/feedback'
import type { WorkspaceViewDescriptor } from './viewDescriptors'

type RequestTitle = Pick<FeedbackRequestSummary, 'request_id' | 'title'>

export function updateTaskTabTitles(
  previous: ReadonlyMap<string, string>,
  views: readonly WorkspaceViewDescriptor[],
  requests: readonly RequestTitle[],
): ReadonlyMap<string, string> {
  const availableTitles = new Map(requests.map((request) => [request.request_id, request.title]))
  const titles = new Map<string, string>()

  for (const view of views) {
    if (view.kind !== 'request-task') continue
    // The navigation list is scoped to one session; an open tab can outlive that scope.
    const title = availableTitles.get(view.requestId) ?? previous.get(view.requestId)
    if (title !== undefined) titles.set(view.requestId, title)
  }

  return titles
}
