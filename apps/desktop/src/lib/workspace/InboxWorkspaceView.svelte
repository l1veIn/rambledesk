<script lang="ts">
  import { Inbox } from '@lucide/svelte'

  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'

  function tr(source: string) {
    return t($locale, source)
  }
</script>

<section class="grid h-full min-h-0 place-items-center bg-background px-8 text-center">
  <div class="max-w-sm">
    <div class="mx-auto grid size-11 place-items-center rounded-lg bg-muted text-muted-foreground">
      <Inbox class="size-5" aria-hidden="true" />
    </div>
    <h2 class="mb-0 mt-4 text-base font-semibold">{tr('Select a request')}</h2>
    <p class="mb-0 mt-2 text-xs leading-5 text-muted-foreground">
      {tr('Open a request from the Inbox to continue in its Session tab.')}
    </p>
  </div>
</section>
