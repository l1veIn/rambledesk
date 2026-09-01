import type { PublishedFeedbackAction } from '../../publishedFeedbackAction'
import {
  DEFAULT_TAURI_CAPABILITY_API,
  type TauriCapabilityApi,
} from './tauriCapabilityApi'

/** Native projection of the published-feedback action shown by the Workbench. */
export function createTauriPublishedFeedbackAction(
  api: Pick<TauriCapabilityApi, 'invoke'> = DEFAULT_TAURI_CAPABILITY_API,
): PublishedFeedbackAction {
  return {
    label: 'Open feedback package',
    run: (requestId) =>
      api.invoke<void>('reveal_feedback_package', {
        input: { request_id: requestId },
      }),
  }
}
