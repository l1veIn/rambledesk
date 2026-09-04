<script lang="ts">
  import { LoaderCircle } from '@lucide/svelte'
  import { Button } from '$lib/components/ui/button'
  import { Badge } from '$lib/components/ui/badge'
  import type { FeedbackDelivery, FeedbackRequestSummary, ResolveDeliveryAction } from '$lib/generated/feedback'
  import { locale } from '$lib/preferences'
  import { agentText } from './agentI18n'
  import { redactAgentMessage } from './agentConfigForm'
  import { deliveriesForSession, deliveryStateDescription, deliveryStateLabel } from './feedbackDeliveryUi'

  export let sessionId: string
  export let deliveries: readonly FeedbackDelivery[] = []
  export let requests: readonly FeedbackRequestSummary[] = []
  export let envText = ''
  export let disabled = false
  export let onResolve: (requestId: string, action: ResolveDeliveryAction) => Promise<void> | void
  export let onOpenFeedback: (requestId: string) => Promise<void> | void

  let pending = new Set<string>()
  let errors: Record<string, string> = {}
  $: visible = deliveriesForSession(sessionId, deliveries)
  $: needsAttention = visible.some((delivery) => ['pending', 'sending', 'uncertain'].includes(delivery.state))
  function tr(source: string) { return agentText($locale, source) }

  async function resolve(delivery: FeedbackDelivery, action: ResolveDeliveryAction) {
    if (disabled || delivery.session_id !== sessionId || delivery.state !== 'uncertain') return
    const key = `${delivery.session_id}:${delivery.request_id}`
    if (pending.has(key)) return
    const respond = onResolve
    const operationEnv = envText
    pending = new Set([...pending, key])
    errors = { ...errors, [key]: '' }
    try {
      await respond(delivery.request_id, action)
    } catch (cause) {
      const message = cause instanceof Error ? cause.message
        : typeof cause === 'object' && cause !== null && 'message' in cause ? String(cause.message)
          : 'Something went wrong'
      errors = { ...errors, [key]: redactAgentMessage(message, operationEnv) }
    } finally {
      const next = new Set(pending)
      next.delete(key)
      pending = next
    }
  }
</script>

{#if visible.length}
  <details open={needsAttention} class="shrink-0 border-b bg-muted/20 px-5 py-3" aria-label={tr('Feedback continuation')}>
    <summary class="cursor-pointer text-xs font-medium">{tr('Feedback continuation')} <span class="ml-1 text-[10px] tabular-nums text-muted-foreground">{visible.length}</span></summary>
    <div class="mt-3 max-h-48 space-y-3 overflow-y-auto">
      {#each visible as delivery (delivery.request_id)}
        {@const key = `${sessionId}:${delivery.request_id}`}
        {@const request = requests.find((candidate) => candidate.request_id === delivery.request_id)}
        <div class="space-y-1.5 text-xs" data-feedback-delivery={delivery.request_id}>
          <div class="flex flex-wrap items-center gap-2">
            <button type="button" class="max-w-full truncate font-medium underline-offset-2 hover:underline" onclick={() => void onOpenFeedback(delivery.request_id)}>{request?.title ?? delivery.request_id}</button>
            {#if delivery.state === 'sending'}<LoaderCircle class="size-3 animate-spin" aria-hidden="true" />{/if}
            <Badge variant={delivery.state === 'uncertain' ? 'destructive' : 'outline'} class="text-[10px]">{tr(deliveryStateLabel(delivery.state))}</Badge>
          </div>
          <p class="m-0 text-[11px] leading-5 text-muted-foreground">{tr(deliveryStateDescription(delivery.state))}</p>
          {#if delivery.last_error}<p class="m-0 break-words text-[11px] text-destructive">{redactAgentMessage(delivery.last_error, envText)}</p>{/if}
          {#if delivery.state === 'uncertain'}
            <div class="flex flex-wrap gap-2">
              <Button variant="outline" size="sm" disabled={disabled || pending.has(key)} onclick={() => void resolve(delivery, 'retry')}>{tr('Send again')}</Button>
              <Button variant="ghost" size="sm" disabled={disabled || pending.has(key)} onclick={() => void resolve(delivery, 'acknowledge')}>{tr('Mark as delivered')}</Button>
            </div>
          {/if}
          {#if errors[key]}<p role="alert" class="m-0 break-words text-[11px] text-destructive">{errors[key]}</p>{/if}
        </div>
      {/each}
    </div>
  </details>
{/if}
