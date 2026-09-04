import { describe, expect, it, vi } from 'vitest'
import { render } from 'svelte/server'
import type { FeedbackDelivery, FeedbackDeliveryState } from '$lib/generated/feedback'
import FeedbackDeliveryStatus from './FeedbackDeliveryStatus.svelte'
import { deliveriesForSession, deliveryStateDescription, deliveryStateLabel } from './feedbackDeliveryUi'

vi.mock('$lib/preferences', async () => {
  const { writable } = await import('svelte/store')
  return { locale: writable('en') }
})

function delivery(requestId: string, state: FeedbackDeliveryState, sessionId = 'one'): FeedbackDelivery {
  return { request_id: requestId, session_id: sessionId, resolution: 'feedback_submitted', state,
    attempt_id: 'attempt', created_at: 'today', updated_at: 'today', last_error: null }
}

describe('feedback continuation presentation', () => {
  it('keeps session ownership and distinct requests while replacing snapshots for the same request', () => {
    const visible = deliveriesForSession('one', [
      delivery('first', 'pending'), delivery('foreign', 'uncertain', 'two'),
      delivery('second', 'sending'), delivery('first', 'delivered'),
    ])
    expect(visible.map((item) => [item.request_id, item.state])).toEqual([['first', 'delivered'], ['second', 'sending']])
  })

  it('shows every durable state, and offers explicit resolution only for uncertain delivery', () => {
    const resolve = vi.fn()
    const { body } = render(FeedbackDeliveryStatus, { props: {
      sessionId: 'one', onResolve: resolve, onOpenFeedback: vi.fn(), envText: 'TOKEN=hidden-secret',
      deliveries: [
        delivery('pending', 'pending'), delivery('sending', 'sending'), delivery('delivered', 'delivered'),
        { ...delivery('uncertain', 'uncertain'), last_error: 'An error containing hidden-secret' },
        delivery('discarded', 'discarded'), delivery('foreign-private-feedback', 'uncertain', 'two'),
      ],
    } })
    for (const state of ['pending', 'sending', 'delivered', 'uncertain', 'discarded'] as const) {
      expect(body).toContain(deliveryStateLabel(state))
      expect(body).toContain(deliveryStateDescription(state))
    }
    expect(body.match(/Send again/g)).toHaveLength(1)
    expect(body.match(/Mark as delivered/g)).toHaveLength(1)
    expect(body).not.toContain('foreign-private-feedback')
    expect(body).not.toContain('hidden-secret')
    expect(resolve).not.toHaveBeenCalled()
  })

  it('does not produce continuation controls for an external or empty session projection', () => {
    const action = vi.fn()
    const { body } = render(FeedbackDeliveryStatus, { props: {
      sessionId: 'external', deliveries: [delivery('another', 'uncertain')], onResolve: action, onOpenFeedback: action,
    } })
    expect(body).not.toContain('Feedback continuation')
    expect(body).not.toContain('Send again')
    expect(action).not.toHaveBeenCalled()
  })
})
