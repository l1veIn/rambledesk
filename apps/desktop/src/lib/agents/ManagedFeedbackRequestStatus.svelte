<script lang="ts">
  import { onMount } from 'svelte'
  import { LoaderCircle } from '@lucide/svelte'
  import { Button } from '$lib/components/ui/button'
  import { Badge } from '$lib/components/ui/badge'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import { locale } from '$lib/preferences'
  import { agentText } from './agentI18n'
  import { redactAgentMessage } from './agentConfigForm'
  import { deliveryStateDescription, deliveryStateLabel } from './feedbackDeliveryUi'
  import { createManagedFeedbackStatusController } from './managedFeedbackStatusController'

  // Mount with a key containing sessionId and requestId when request selection changes.
  export let transport: ApplicationTransport
  export let sessionId: string
  export let requestId: string
  export let disabled = false
  export let onDeletingChange: (sessionId: string, deleting: boolean) => void = () => {}
  const controller = createManagedFeedbackStatusController(transport, sessionId, requestId)
  onMount(() => controller.start())
  $: delivery = $controller.status?.deliveries.find(item => item.request_id === requestId)
  $: if ($controller.status) onDeletingChange(sessionId, $controller.status.deleting)
  const messages: Record<string, string> = {
    'Could not load feedback continuation status.': '无法读取反馈投递状态。',
    'Could not update feedback continuation.': '无法更新反馈投递状态。',
    'Retry status': '重新读取',
  }
  function tr(source: string) { return $locale === 'zh-CN' && messages[source] ? messages[source] : agentText($locale, source) }
</script>

{#if $controller.status?.deleting || delivery || $controller.error}
  <div class="space-y-2 rounded-md border bg-muted/20 px-3 py-2 text-xs" data-managed-feedback-status={requestId}>
    {#if $controller.status?.deleting}
      <p class="m-0 text-destructive">{tr('This session is being deleted. Retry deletion to finish cleanup.')}</p>
    {:else if delivery}
      <div class="flex flex-wrap items-center gap-2">
        {#if delivery.state === 'sending'}<LoaderCircle class="size-3 animate-spin" aria-hidden="true" />{/if}
        <Badge variant={delivery.state === 'uncertain' ? 'destructive' : 'outline'} class="text-[10px]">{tr(deliveryStateLabel(delivery.state))}</Badge>
      </div>
      <p class="m-0 text-[11px] leading-5 text-muted-foreground">{tr(deliveryStateDescription(delivery.state))}</p>
      {#if delivery.last_error}<p class="m-0 break-words text-[11px] text-destructive">{tr(redactAgentMessage(delivery.last_error, ''))}</p>{/if}
      {#if delivery.state === 'uncertain'}
        <div class="flex flex-wrap gap-2">
          <Button variant="outline" size="sm" disabled={disabled || $controller.resolving} onclick={() => void controller.resolve('retry')}>{tr('Send again')}</Button>
          <Button variant="ghost" size="sm" disabled={disabled || $controller.resolving} onclick={() => void controller.resolve('acknowledge')}>{tr('Mark as delivered')}</Button>
        </div>
      {/if}
    {/if}
    {#if $controller.error}
      <div class="flex items-center gap-2"><p role="alert" class="m-0 text-destructive">{tr($controller.error)}</p><Button variant="ghost" size="sm" onclick={() => controller.refresh()}>{tr('Retry status')}</Button></div>
    {/if}
    {#if $controller.resolveError}<p role="alert" class="m-0 text-destructive">{tr($controller.resolveError)}</p>{/if}
  </div>
{/if}
