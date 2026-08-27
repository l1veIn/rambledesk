<script lang="ts">
  import { Toaster } from 'svelte-sonner'

  import rambelleWaiting from '../../../../assets/rambelle-states/state-waiting.webp'
  import rambelleError from '../../../../assets/rambelle-states/toast-error.webp'
  import rambelleInfo from '../../../../assets/rambelle-states/toast-info.webp'
  import rambelleSuccess from '../../../../assets/rambelle-states/toast-success.webp'
  import rambelleWarning from '../../../../assets/rambelle-states/toast-warning.webp'
  import { themePreference } from '$lib/preferences'
</script>

{#snippet toastPortrait(source: string)}
  <img src={source} alt="" class="rambelle-toast-portrait" aria-hidden="true" />
{/snippet}

{#snippet successIcon()}{@render toastPortrait(rambelleSuccess)}{/snippet}
{#snippet infoIcon()}{@render toastPortrait(rambelleInfo)}{/snippet}
{#snippet warningIcon()}{@render toastPortrait(rambelleWarning)}{/snippet}
{#snippet errorIcon()}{@render toastPortrait(rambelleError)}{/snippet}
{#snippet loadingIcon()}{@render toastPortrait(rambelleWaiting)}{/snippet}

<Toaster
  position="top-right"
  theme={$themePreference}
  closeButton
  {successIcon}
  {infoIcon}
  {warningIcon}
  {errorIcon}
  {loadingIcon}
  toastOptions={{
    duration: 4_000,
    actionButtonStyle:
      'font-size: 11px; padding: 4px 10px; border-radius: 6px; ' +
      'background: hsl(var(--muted)); color: hsl(var(--foreground)); ' +
      'border: 1px solid hsl(var(--border));',
  }}
/>

<style>
  :global([data-sonner-toast][data-styled='true']) {
    border-color: var(--border) !important;
    color: var(--foreground) !important;
    background: var(--card) !important;
    box-shadow: 0 12px 34px rgb(15 35 55 / 16%) !important;
  }

  :global([data-sonner-toast][data-styled='true'] [data-icon]) {
    width: 100px !important;
    height: 100px !important;
    margin-inline: -8px 8px !important;
    overflow: visible !important;
  }

  :global(.rambelle-toast-portrait) {
    width: 100px !important;
    height: 100px !important;
    max-width: none !important;
    max-height: none !important;
    object-fit: contain;
  }

  :global([data-sonner-toast][data-styled='true'] [data-description]) {
    color: var(--muted-foreground) !important;
  }

  :global([data-sonner-toast][data-type='success'] [data-title]),
  :global([data-sonner-toast][data-type='success'] [data-icon]) {
    color: var(--success) !important;
  }

  :global([data-sonner-toast][data-type='info'] [data-title]),
  :global([data-sonner-toast][data-type='info'] [data-icon]) {
    color: var(--info) !important;
  }

  :global([data-sonner-toast][data-type='error'] [data-title]),
  :global([data-sonner-toast][data-type='error'] [data-icon]) {
    color: var(--destructive) !important;
  }
</style>
