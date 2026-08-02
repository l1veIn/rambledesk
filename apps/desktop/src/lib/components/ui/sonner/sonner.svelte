<script lang="ts">
  import { Toaster } from 'svelte-sonner'

  import rambelleWaiting from '../../../../assets/rambelle-states/state-waiting.png'
  import rambelleError from '../../../../assets/rambelle-states/toast-error.png'
  import rambelleInfo from '../../../../assets/rambelle-states/toast-info.png'
  import rambelleSuccess from '../../../../assets/rambelle-states/toast-success.png'
  import rambelleWarning from '../../../../assets/rambelle-states/toast-warning.png'
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
  toastOptions={{ duration: 4_000 }}
/>

<style>
  :global([data-sonner-toast][data-styled='true']) {
    border-color: var(--border) !important;
    color: var(--foreground) !important;
    background: var(--card) !important;
    box-shadow: 0 12px 34px rgb(15 35 55 / 16%) !important;
  }

  :global([data-sonner-toast][data-styled='true'] [data-icon]) {
    width: 52px;
    height: 52px;
    margin-inline: -5px 6px;
  }

  :global(.rambelle-toast-portrait) {
    width: 58px;
    height: 58px;
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
