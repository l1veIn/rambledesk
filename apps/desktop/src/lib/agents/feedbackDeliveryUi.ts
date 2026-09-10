import type { FeedbackDeliveryState } from '$lib/generated/feedback'

export function deliveryStateLabel(state: FeedbackDeliveryState): string {
  switch (state) {
    case 'pending': return 'Waiting for the agent'
    case 'sending': return 'Continuing the agent…'
    case 'delivered': return 'Feedback delivered'
    case 'uncertain': return 'Delivery status unknown'
    case 'discarded': return 'Delivery discarded'
  }
}

export function deliveryStateDescription(state: FeedbackDeliveryState): string {
  switch (state) {
    case 'pending': return 'Feedback is saved. It will continue this session when the agent can receive input.'
    case 'sending': return 'The agent is continuing with this feedback.'
    case 'delivered': return 'The continuation turn completed and the feedback was sent.'
    case 'uncertain': return 'The agent may already have received this feedback. Sending again may repeat work. Review its activity before choosing.'
    case 'discarded': return 'This feedback will not be sent to the agent.'
  }
}
