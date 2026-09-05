<script lang="ts">
  import { onMount } from 'svelte'
  import { ArrowUpRight, LoaderCircle } from '@lucide/svelte'
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
  export let navigationDisabled = false
  export let onOpenAgent: () => void = () => {}
  export let onDeletingChange: (sessionId: string, deleting: boolean) => void = () => {}
  const controller = createManagedFeedbackStatusController(transport, sessionId, requestId)
  onMount(() => controller.start())
  $: delivery = $controller.status?.deliveries.find(item => item.request_id === requestId)
  $: if ($controller.status) onDeletingChange(sessionId, $controller.status.deleting)
  const connectionLabels = { connecting: 'Connecting…', connected: 'Connected', disconnected: 'Disconnected', failed: 'Connection failed', stopped: 'Stopped' }
  const activityLabels = { running: 'Agent is working', waiting_permission: 'Waiting for permission', idle: 'Idle' }
  $: status = $controller.status
  $: runtimeLabel = status?.deleting ? 'Deleting session…' : $controller.error ? 'Status unavailable' : status
    ? status.connection === 'connected' ? activityLabels[status.activity] : connectionLabels[status.connection]
    : 'Loading status…'
  $: running = !$controller.error && !status?.deleting && (status?.connection === 'connecting' || (status?.connection === 'connected' && status.activity === 'running'))
  const messages: Record<string, string> = {
    'Could not load feedback continuation status.': '无法读取反馈投递状态。',
    'Could not update feedback continuation.': '无法更新反馈投递状态。',
    'Retry status': '重新读取',
    'View Agent': '查看 Agent',
    'Loading status…': '正在读取状态…',
    'Status unavailable': '状态不可用',
    'Deleting session…': '正在删除会话…',
    'Ramble feedback': 'Ramble 反馈',
  }
  function tr(source: string) { return $locale === 'zh-CN' && messages[source] ? messages[source] : agentText($locale, source) }
</script>

  <div class="space-y-1 text-xs" data-managed-feedback-status={requestId}>
    <div class="flex min-w-0 items-center justify-between gap-2">
      <div class="flex min-w-0 items-center gap-1.5" role="status" title={status && !$controller.error ? `${tr(connectionLabels[status.connection])} · ${tr(activityLabels[status.activity])}` : undefined}>
        <span class="shrink-0 text-[10px] font-medium text-muted-foreground">ACP</span>
        {#if running || $controller.loading}
          <LoaderCircle class="size-3 shrink-0 animate-spin" aria-hidden="true" />
        {:else}
          <span class={['size-1.5 shrink-0 rounded-full', $controller.error || status?.connection === 'failed' ? 'bg-destructive' : !status?.deleting && status?.connection === 'connected' ? 'bg-success' : 'bg-muted-foreground']} aria-hidden="true"></span>
        {/if}
        <span class="truncate text-[11px]">{tr(runtimeLabel)}</span>
      </div>
      <Button size="sm" variant="ghost" class="h-6 shrink-0 gap-1 px-1 text-[11px]" disabled={navigationDisabled} onclick={onOpenAgent}>{tr('View Agent')}<ArrowUpRight class="size-3" /></Button>
    </div>
    {#if status?.deleting}
      <p class="m-0 text-destructive">{tr('This session is being deleted. Retry deletion to finish cleanup.')}</p>
    {:else if delivery}
      <div class="flex flex-wrap items-center gap-1.5" title={tr(deliveryStateDescription(delivery.state))}>
        <span class="text-[10px] text-muted-foreground">{tr('Ramble feedback')}</span>
        {#if delivery.state === 'sending'}<LoaderCircle class="size-3 animate-spin" aria-hidden="true" />{/if}
        <Badge variant={delivery.state === 'uncertain' ? 'destructive' : 'outline'} class="text-[10px]">{tr(deliveryStateLabel(delivery.state))}</Badge>
      </div>
      {#if delivery.last_error}<p class="m-0 break-words text-[11px] text-destructive">{tr(redactAgentMessage(delivery.last_error, ''))}</p>{/if}
      {#if delivery.state === 'uncertain'}
        <p class="m-0 text-[11px] leading-5 text-muted-foreground">{tr(deliveryStateDescription(delivery.state))}</p>
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
