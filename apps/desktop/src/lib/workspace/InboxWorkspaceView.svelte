<script lang="ts">
  import { Plus } from '@lucide/svelte'

  import rambelleIdle from '../../assets/rambelle-states/idle.webp'
  import { Button } from '$lib/components/ui/button'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'

  export let onNewSession: (() => void) | undefined = undefined

  function tr(source: string) {
    return t($locale, source)
  }
</script>

<section class="appearance-surface flex h-full min-h-0 flex-col overflow-y-auto bg-background px-6 py-8 text-center" data-empty-workspace>
  <div class="mx-auto my-auto w-full max-w-sm py-4">
    <img src={rambelleIdle} alt="Rambelle" class="mx-auto size-36 object-contain sm:size-44" draggable="false" />
    <h2 class="mb-0 mt-5 text-xl font-medium tracking-tight">{onNewSession ? tr('New session') : tr('Select a request')}</h2>
    <p class="mb-0 mt-3 text-sm leading-6 text-muted-foreground">
      {#if onNewSession}
        {$locale === 'zh-CN' ? '选择智能体和项目目录，描述任务即可开始。需要你反馈时，请求会出现在收件箱中。' : 'Choose an agent and a project, then describe your task. Requests appear in the Inbox when your feedback is needed.'}
      {:else}
        {tr('Open a request from the Inbox to continue in its Session tab.')}
      {/if}
    </p>
    {#if onNewSession}<Button class="mt-6" onclick={onNewSession}><Plus class="size-4" />{tr('New session')}</Button>{/if}
  </div>
</section>
