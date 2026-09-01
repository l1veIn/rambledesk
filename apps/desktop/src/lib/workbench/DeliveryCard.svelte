<script lang="ts">
  import {
    Ban,
    ChefHat,
    CheckCircle2,
    FolderOpen,
    MessageSquareReply,
    Send,
    ThumbsUp,
  } from '@lucide/svelte'

  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import * as Dialog from '$lib/components/ui/dialog'
  import type { FeedbackResultView } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import type { SubmitStage } from './types'

  export let feedbackResult: FeedbackResultView | null = null
  export let cancelled = false
  export let approved = false
  export let canSubmit = false
  export let cooking = false
  export let cookingEnabled = false
  export let cookedDraftReady = false
  export let submitting = false
  export let submitStage: SubmitStage = 'idle'
  export let canCancel = false
  export let cancelling = false
  export let allowFinish = false
  export let finalSummary = ''
  export let approving = false
  export let canOpenResumePrompt = false
  export let onOpenPackage: () => void = () => {}
  export let onOpenResumePrompt: () => void = () => {}
  export let onCookPreview: () => void = () => {}
  export let onSubmit: () => void = () => {}
  export let onCancel: () => void = () => {}
  export let onApprove: () => void = () => {}

  let cancelConfirmOpen = false

  $: published = feedbackResult !== null && !submitting && !cooking
  $: operationLocked = cooking || submitting || cancelling || approving

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function confirmCancel() {
    cancelConfirmOpen = false
    onCancel()
  }
</script>

{#if published && feedbackResult}
  <section class="p-4">
    <header class="mb-3 flex items-center gap-2">
      <strong class="text-xs font-medium">{tr('Feedback Package')}</strong>
      <Badge class="ml-auto bg-success text-white">
        <CheckCircle2 class="size-3" />
        {tr('Published')}
      </Badge>
    </header>
    <Button class="mt-3 w-full" variant="outline" onclick={onOpenPackage}>
      <FolderOpen data-icon="inline-start" />
      {tr('Open feedback package')}
    </Button>
    {#if canOpenResumePrompt}
      <Button class="mt-2 w-full" onclick={onOpenResumePrompt}>
        <MessageSquareReply data-icon="inline-start" />
        {tr('Submission details')}
      </Button>
    {/if}
  </section>
{:else if approved}
  <section class="p-4">
    <Badge class="bg-success text-white"><CheckCircle2 class="size-3" />{tr('Approved')}</Badge>
    <p class="m-0 mt-3 text-[10px] leading-4 text-muted-foreground">
      {tr('The user approved the Agent’s final summary. The Ramble flow has ended.')}
    </p>
  </section>
{:else if cancelled}
  <section class="p-4">
    <Badge variant="destructive">{tr('Cancelled')}</Badge>
    <p class="m-0 mt-3 text-[10px] leading-4 text-muted-foreground">
      {tr('The host can read the cancellation state and continue the session.')}
    </p>
  </section>
{:else}
  <section class="p-4">
    {#if allowFinish && finalSummary}
      <div class="mb-3 rounded-md border border-primary/25 bg-primary/5 p-3">
        <strong class="block text-[10px] font-medium">{tr('Agent final summary')}</strong>
        <p class="m-0 mt-1 whitespace-pre-wrap text-[10px] leading-4 text-muted-foreground">{finalSummary}</p>
      </div>
    {/if}
    <div class="grid gap-2">
      {#if cookingEnabled}
        <Button
          class="w-full"
          variant={cookedDraftReady ? 'default' : 'secondary'}
          disabled={operationLocked || !canSubmit}
          onclick={cookedDraftReady ? onSubmit : onCookPreview}
        >
          {#if cookedDraftReady}
            <Send data-icon="inline-start" />
          {:else}
            <ChefHat data-icon="inline-start" />
          {/if}
          {cooking
            ? tr('Cooking…')
            : submitting || submitStage === 'publishing'
              ? tr('Publishing…')
              : cookedDraftReady
                ? tr('Submit feedback')
                : tr('Cook')}
        </Button>
        {#if !cookedDraftReady}
          <Button class="w-full" disabled={operationLocked || !canSubmit} onclick={onSubmit}>
            <Send data-icon="inline-start" />
            {cooking || submitting
              ? cooking || submitStage === 'cooking'
                ? tr('Cooking…')
                : tr('Publishing…')
              : tr('Cook and submit')}
          </Button>
        {/if}
      {:else}
        <Button class="w-full" disabled={operationLocked || !canSubmit} onclick={onSubmit}>
          <Send data-icon="inline-start" />
          {submitting ? tr('Publishing…') : tr('Submit feedback')}
        </Button>
      {/if}
      {#if allowFinish}
        <Button class="w-full" variant="secondary" disabled={operationLocked} onclick={onApprove}>
          <ThumbsUp data-icon="inline-start" />
          {approving ? tr('Finishing…') : tr('Approve and finish')}
        </Button>
      {/if}
      <Button
        class="w-full"
        variant="destructive"
        disabled={operationLocked || !canCancel}
        onclick={() => (cancelConfirmOpen = true)}
      >
        <Ban data-icon="inline-start" />
        {cancelling ? tr('Cancelling…') : tr('Cancel feedback')}
      </Button>
    </div>
  </section>
{/if}

<Dialog.Root bind:open={cancelConfirmOpen}>
  <Dialog.Content class="max-w-md gap-5 sm:max-w-md" showCloseButton={false}>
    <Dialog.Header>
      <Dialog.Title>{tr('Cancel this request?')}</Dialog.Title>
      <Dialog.Description class="mt-1 leading-5">
        {tr('The agent will receive a terminal state, and this draft can no longer be edited. This action cannot be undone.')}
      </Dialog.Description>
    </Dialog.Header>
    <Dialog.Footer class="gap-2 sm:justify-end">
      <Button variant="outline" onclick={() => (cancelConfirmOpen = false)}>
        {tr('Go back')}
      </Button>
      <Button variant="destructive" onclick={confirmCancel}>
        <Ban data-icon="inline-start" />
        {tr('Confirm cancel')}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
