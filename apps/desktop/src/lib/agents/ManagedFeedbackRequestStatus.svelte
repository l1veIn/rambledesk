<script lang="ts">
  import { onMount } from 'svelte'
  import { ArrowUpRight, ChevronDown, LoaderCircle } from '@lucide/svelte'
  import { Popover } from 'bits-ui'
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
  let detailsOpen = false
  onMount(() => controller.start())
  $: delivery = $controller.status?.deliveries.find(item => item.request_id === requestId)
  $: if ($controller.status) onDeletingChange(sessionId, $controller.status.deleting)
  const connectionLabels = { connecting: 'Connecting…', connected: 'Connected', disconnected: 'Disconnected', failed: 'Connection failed', stopped: 'Stopped' }
  const activityLabels = { running: 'Agent is working', waiting_input: 'Waiting for your response', idle: 'Idle' }
  $: status = $controller.status
  $: runtimeLabel = status?.deleting ? 'Deleting session…' : $controller.error ? 'Status unavailable' : status
    ? status.connection === 'connected' ? activityLabels[status.activity] : connectionLabels[status.connection]
    : 'Loading status…'
  $: running = !$controller.error && !status?.deleting && (status?.connection === 'connecting' || (status?.connection === 'connected' && status.activity === 'running'))
  $: needsAttention = Boolean(status?.deleting || $controller.error || $controller.resolveError || delivery?.last_error || delivery?.state === 'uncertain' || status?.connection === 'failed')
  $: hasDetails = Boolean(delivery || needsAttention)
  $: feedbackLabel = status?.deleting ? 'Deleting session…' : $controller.error ? 'Status unavailable'
    : delivery ? deliveryStateLabel(delivery.state) : status ? 'No feedback delivery yet' : 'Loading status…'
  const messages: Record<string, string> = {
    'Could not load feedback continuation status.': '无法读取反馈投递状态。',
    'Could not update feedback continuation.': '无法更新反馈投递状态。',
    'Retry status': '重新读取',
    'View Agent': '查看 Agent',
    'Loading status…': '正在读取状态…',
    'Status unavailable': '状态不可用',
    'Deleting session…': '正在删除会话…',
    'Ramble feedback': 'Ramble 反馈',
    'No feedback delivery yet': '暂无反馈投递',
    'Status details': '状态详情',
    'View status details': '查看状态详情',
    'Review feedback status': '处理反馈状态',
    'Details': '详情',
    'Review': '处理',
  }
  function tr(source: string) { return $locale === 'zh-CN' && messages[source] ? messages[source] : agentText($locale, source) }
</script>

  <div class="flex h-12 w-full min-w-0 shrink-0 flex-col gap-1 text-xs" data-managed-feedback-status={requestId}>
    <div class="flex h-6 min-w-0 shrink-0 items-center justify-between gap-2">
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
    <div class="flex h-5 min-w-0 shrink-0 items-center justify-between gap-1.5">
      <div class="flex min-w-0 items-center gap-1.5" role="status">
        <span class="shrink-0 text-[10px] text-muted-foreground">{tr('Ramble feedback')}</span>
        {#if delivery?.state === 'sending'}<LoaderCircle class="size-3 shrink-0 animate-spin" aria-hidden="true" />{/if}
        <Badge variant={needsAttention ? 'destructive' : 'outline'} class="h-5 min-w-0 shrink overflow-hidden px-1.5 text-[10px]" title={tr(feedbackLabel)}><span class="truncate">{tr(feedbackLabel)}</span></Badge>
      </div>
      {#if hasDetails}
        <Popover.Root bind:open={detailsOpen}>
          <Popover.Trigger>
            {#snippet child({ props })}
              <Button {...props} variant="ghost" size="sm" class={['h-5 shrink-0 gap-0.5 px-1 text-[10px]', needsAttention ? 'text-destructive' : 'text-muted-foreground']} aria-label={tr(needsAttention ? 'Review feedback status' : 'View status details')}>
                {tr(needsAttention ? 'Review' : 'Details')}<ChevronDown class="size-3" aria-hidden="true" />
              </Button>
            {/snippet}
          </Popover.Trigger>
          <Popover.Portal>
            <Popover.Content side="bottom" align="end" sideOffset={8} aria-label={tr('Status details')}
              class="z-[130] w-80 max-w-[calc(100vw-2rem)] space-y-3 overflow-y-auto rounded-xl border bg-popover p-4 text-xs text-popover-foreground shadow-lg outline-none"
              style="max-height: min(24rem, var(--bits-popover-content-available-height, calc(100dvh - 2rem))); overflow-wrap: anywhere;">
              <div class="space-y-1">
                <h2 class="m-0 text-sm font-medium">{tr('Status details')}</h2>
                <p class="m-0 text-muted-foreground">ACP · {tr(runtimeLabel)}</p>
              </div>
              {#if status?.deleting}
                <p class="m-0 text-destructive">{tr('This session is being deleted. Retry deletion to finish cleanup.')}</p>
              {:else if delivery}
                <div class="space-y-2">
                  <p class="m-0 font-medium">{tr('Ramble feedback')} · {tr(deliveryStateLabel(delivery.state))}</p>
                  {#if delivery.last_error}<p class="m-0 whitespace-pre-wrap text-destructive">{tr(redactAgentMessage(delivery.last_error, ''))}</p>{/if}
                  <p class="m-0 leading-5 text-muted-foreground">{tr(deliveryStateDescription(delivery.state))}</p>
                </div>
                {#if delivery.state === 'uncertain'}
                  <div class="flex flex-wrap gap-2">
                    <Button variant="outline" size="sm" disabled={disabled || $controller.resolving} onclick={() => void controller.resolve('retry')}>{tr('Send again')}</Button>
                    <Button variant="ghost" size="sm" disabled={disabled || $controller.resolving} onclick={() => void controller.resolve('acknowledge')}>{tr('Mark as delivered')}</Button>
                  </div>
                {/if}
              {/if}
              {#if $controller.error}
                <div class="space-y-2"><p role="alert" class="m-0 text-destructive">{tr($controller.error)}</p><Button variant="outline" size="sm" disabled={$controller.loading} onclick={() => controller.refresh()}>{tr('Retry status')}</Button></div>
              {/if}
              {#if $controller.resolveError}<p role="alert" class="m-0 text-destructive">{tr($controller.resolveError)}</p>{/if}
            </Popover.Content>
          </Popover.Portal>
        </Popover.Root>
      {/if}
    </div>
  </div>
