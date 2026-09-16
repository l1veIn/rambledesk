<script lang="ts">
  import {
    Ban,
    ChefHat,
    CheckCircle2,
    Download,
    FolderOpen,
    LoaderCircle,
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
  import type { SubmitStage } from '../domain/sessionPhases'

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
  export let approving = false
  export let canOpenResumePrompt = false
  export let onOpenPackage: () => void = () => {}
  export let packageActionLabel = 'Open feedback package'
  export let onOpenResumePrompt: () => void = () => {}
  export let onCookPreview: () => void = () => {}
  export let onSubmit: () => void = () => {}
  export let onCancel: () => void = () => {}
  export let onApprove: () => void = () => {}

  let cancelConfirmOpen = false

  $: published = feedbackResult !== null && !submitting && !cooking && !cancelled
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
  <div class="flex flex-wrap items-center gap-2">
    <strong class="text-xs font-medium">{tr('Feedback Package')}</strong>
    <Badge class="bg-success text-white">
      <CheckCircle2 class="size-3" />
      {tr('Published')}
    </Badge>
    <Button size="sm" variant="outline" onclick={onOpenPackage}>
      {#if packageActionLabel === 'Open feedback package'}
        <FolderOpen data-icon="inline-start" />
      {:else}
        <Download data-icon="inline-start" />
      {/if}
      {tr(packageActionLabel)}
    </Button>
    {#if canOpenResumePrompt}
      <Button size="sm" variant="ghost" aria-label={tr('Submission details')} title={tr('Submission details')} onclick={onOpenResumePrompt}>
        <MessageSquareReply />
      </Button>
    {/if}
  </div>
{:else if approved}
  <div class="flex items-center gap-2">
    <Badge class="bg-success text-white"><CheckCircle2 class="size-3" />{tr('Approved')}</Badge>
    <span class="text-[10px] text-muted-foreground">{tr('The Ramble flow has ended.')}</span>
  </div>
{:else if cancelled}
  <div class="flex flex-wrap items-center gap-2">
    <Badge variant="destructive" title={tr('Feedback is cancelled. No continuation message is sent.')}>{tr('Cancelled')}</Badge>
    {#if feedbackResult}
      <Button size="sm" variant="outline" onclick={onOpenPackage}>
        {#if packageActionLabel === 'Open feedback package'}<FolderOpen data-icon="inline-start" />{:else}<Download data-icon="inline-start" />{/if}
        {tr(packageActionLabel)}
      </Button>
    {/if}
  </div>
{:else}
  <!-- The column's only permanent action line, kept next to the document title so
       it is reachable without scrolling to the end of the feedback. -->
  <div class="flex items-center gap-2">
    <Button
      size="icon-sm"
      class="size-8 bg-destructive text-white hover:bg-destructive/90"
      aria-label={tr('Cancel feedback')}
      title={tr('Cancel feedback')}
      disabled={operationLocked || !canCancel}
      onclick={() => (cancelConfirmOpen = true)}
    >
      {#if cancelling}<LoaderCircle class="size-3.5 animate-spin" />{:else}<Ban class="size-3.5" />{/if}
    </Button>
    {#if cookingEnabled}
      <Button
        size="sm"
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
          : submitStage === 'saving'
            ? tr('Saving…')
            : submitting
              ? tr('Publishing…')
              : cookedDraftReady
                ? tr('Submit feedback')
                : tr('Cook')}
      </Button>
      {#if !cookedDraftReady}
        <Button size="sm" disabled={operationLocked || !canSubmit} onclick={onSubmit}>
          <Send data-icon="inline-start" />
          {submitStage === 'saving'
            ? tr('Saving…')
            : cooking || submitting
              ? cooking || submitStage === 'cooking'
                ? tr('Cooking…')
                : tr('Publishing…')
              : tr('Cook and submit')}
        </Button>
      {/if}
    {:else}
      <Button size="sm" disabled={operationLocked || !canSubmit} onclick={onSubmit}>
        <Send data-icon="inline-start" />
        {submitStage === 'saving' ? tr('Saving…') : submitting ? tr('Publishing…') : tr('Submit feedback')}
      </Button>
    {/if}
    {#if allowFinish}
      <Button size="sm" variant="secondary" disabled={operationLocked} onclick={onApprove}>
        <ThumbsUp data-icon="inline-start" />
        {approving ? tr('Finishing…') : tr('Approve and finish')}
      </Button>
    {/if}
  </div>
{/if}

<Dialog.Root bind:open={cancelConfirmOpen}>
  <Dialog.Content class="max-w-md gap-5 sm:max-w-md" showCloseButton={false}>
    <Dialog.Header>
      <Dialog.Title>{tr('Cancel this request?')}</Dialog.Title>
      <Dialog.Description class="mt-1 leading-5">
        {tr('This request will be marked as cancelled without sending a continuation message. The draft can no longer be edited. This action cannot be undone.')}
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
