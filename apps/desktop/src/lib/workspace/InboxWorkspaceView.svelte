<script lang="ts">
  import { Inbox, Plus } from '@lucide/svelte'

  import { Button } from '$lib/components/ui/button'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'

  export let onNewSession: (() => void) | undefined = undefined

  function tr(source: string) {
    return t($locale, source)
  }
</script>

<section class="appearance-surface grid h-full min-h-0 place-items-center bg-background px-8 text-center">
  <div class="max-w-sm">
    <div class="mx-auto grid size-11 place-items-center rounded-lg bg-muted text-muted-foreground">
      <Inbox class="size-5" aria-hidden="true" />
    </div>
    <h2 class="mb-0 mt-4 text-base font-semibold">{onNewSession ? tr('New session') : tr('Select a request')}</h2>
    <p class="mb-0 mt-2 text-xs leading-5 text-muted-foreground">
      {#if onNewSession}
        {$locale === 'zh-CN' ? '选择智能体和项目目录，描述任务即可开始。需要你反馈时，请求会出现在收件箱中。' : 'Choose an agent and a project, then describe your task. Requests appear in the Inbox when your feedback is needed.'}
      {:else}
        {tr('Open a request from the Inbox to continue in its Session tab.')}
      {/if}
    </p>
    {#if onNewSession}<Button class="mt-5" size="sm" onclick={onNewSession}><Plus class="size-3.5" />{tr('New session')}</Button>{/if}
  </div>
</section>
