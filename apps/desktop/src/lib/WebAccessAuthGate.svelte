<script lang="ts">
  import { KeyRound, LoaderCircle } from '@lucide/svelte'

  import rambledeskIcon from '../assets/rambledesk-app-icon.webp'
  import { Button } from '$lib/components/ui/button'
  import {
    WebAccessTokenRejectedError,
    bootstrapWebAccessSession,
  } from '$lib/application/webAccessBootstrap'
  import { locale } from '$lib/preferences'
  import { t } from '$lib/i18n'

  export let onAuthenticated: (sessionToken: string) => void | Promise<void> = () => {}

  let token = ''
  let busy = false
  let error = ''

  function tr(source: string) {
    return t($locale, source)
  }

  async function submit() {
    if (busy || token.trim() === '') return
    busy = true
    error = ''
    try {
      const sessionToken = await bootstrapWebAccessSession({ token: token.trim() })
      token = ''
      await onAuthenticated(sessionToken)
    } catch (cause) {
      error = cause instanceof WebAccessTokenRejectedError
        ? tr('That Web Access token was not accepted. Check it and try again.')
        : tr('Could not reach Web Access. Wait a moment and try again.')
    } finally {
      busy = false
    }
  }
</script>

<main class="fixed inset-0 z-[100] grid place-items-center bg-background/95 p-6 backdrop-blur-sm">
  <form
    class="w-full max-w-md rounded-2xl border bg-card p-8 text-card-foreground shadow-2xl"
    onsubmit={(event) => {
      event.preventDefault()
      void submit()
    }}
  >
    <div class="mb-7 flex items-center gap-4">
      <img class="size-14 rounded-xl" src={rambledeskIcon} alt="" />
      <div>
        <h1 class="m-0 text-xl font-semibold">RambleDesk</h1>
        <p class="m-0 mt-1 text-sm text-muted-foreground">{tr('Web Access')}</p>
      </div>
    </div>
    <label class="mb-2 block text-sm font-medium" for="web-access-token">
      {tr('Access token')}
    </label>
    <input
      id="web-access-token"
      type="password"
      class="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm outline-none transition-colors focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
      autocomplete="off"
      spellcheck={false}
      bind:value={token}
      disabled={busy}
    />
    <p class="mt-2 text-xs leading-5 text-muted-foreground">
      {tr('Copy the token from Settings on the desktop where RambleDesk is running.')}
    </p>
    {#if error}
      <p class="mt-3 text-sm text-destructive" role="alert">{error}</p>
    {/if}
    <Button class="mt-6 w-full" type="submit" disabled={busy || token.trim() === ''}>
      {#if busy}
        <LoaderCircle data-icon="inline-start" class="animate-spin" />
      {:else}
        <KeyRound data-icon="inline-start" />
      {/if}
      {busy ? tr('Connecting…') : tr('Connect')}
    </Button>
  </form>
</main>
