<script lang="ts">
  import { Toaster } from 'svelte-sonner'

  import { themePreference } from '$lib/preferences'
</script>

<Toaster
  position="top-right"
  theme={$themePreference}
  closeButton
  toastOptions={{ duration: 4_000 }}
/>

<style>
  :global([data-sonner-toast][data-styled='true']) {
    border-color: var(--border) !important;
    color: var(--foreground) !important;
    background: var(--card) !important;
    box-shadow: 0 12px 34px rgb(15 35 55 / 16%) !important;
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
