<script lang="ts">
  import { ChefHat, Sparkles } from '@lucide/svelte'

  import { Badge } from '$lib/components/ui/badge'
  import { DEFAULT_COOKING_SYSTEM_PROMPT } from '$lib/cooking'
  import { t } from '$lib/i18n'
  import LlmConfigurationFields from '$lib/LlmConfigurationFields.svelte'
  import { DEFAULT_TIDY_SYSTEM_PROMPT } from '$lib/lightCleanup'
  import {
    cookingApiKey,
    cookingBaseUrl,
    cookingEnabled,
    cookingModel,
    cookingProvider,
    cookingReasoningEffort,
    cookingSystemPrompt,
    locale,
    setCookingApiKey,
    setCookingBaseUrl,
    setCookingEnabled,
    setCookingModel,
    setCookingProvider,
    setCookingReasoningEffort,
    setCookingSystemPrompt,
    setTidyApiKey,
    setTidyBaseUrl,
    setTidyModel,
    setTidyProvider,
    setTidyReasoningEffort,
    setTidySystemPrompt,
    tidyApiKey,
    tidyBaseUrl,
    tidyModel,
    tidyProvider,
    tidyReasoningEffort,
    tidySystemPrompt,
    type CookingProvider,
  } from '$lib/preferences'

  function tr(source: string) {
    return t($locale, source)
  }

  function chooseTidyProvider(provider: CookingProvider) {
    setTidyProvider(provider)
    if (provider === 'deepseek') {
      setTidyBaseUrl('https://api.deepseek.com/v1')
      setTidyModel('deepseek-v4-flash')
    } else if (provider === 'openai') {
      setTidyBaseUrl('https://api.openai.com/v1')
      setTidyModel('gpt-4.1-mini')
    }
  }

  function chooseCookingProvider(provider: CookingProvider) {
    setCookingProvider(provider)
    if (provider === 'deepseek') {
      setCookingBaseUrl('https://api.deepseek.com/v1')
      setCookingModel('deepseek-v4-flash')
    } else if (provider === 'openai') {
      setCookingBaseUrl('https://api.openai.com/v1')
      setCookingModel('gpt-4.1-mini')
    }
  }

  $: tidyReady = $tidyApiKey.trim().length > 0 && $tidyModel.trim().length > 0
</script>

<div class="space-y-6">
  <section class="rounded-lg border bg-card p-5">
    <div class="flex items-start gap-3">
      <span class="grid size-9 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
        <Sparkles class="size-4" />
      </span>
      <div class="min-w-0 flex-1">
        <div class="flex items-center gap-2">
          <h3 class="m-0 text-sm font-medium">Tidy</h3>
          <Badge variant={tidyReady ? 'secondary' : 'outline'}>
            {tidyReady ? tr('Ready') : tr('Configuration required')}
          </Badge>
        </div>
        <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
          {tr('Manually clean pending speech segments in the current document. Tidy never runs automatically.')}
        </p>
      </div>
    </div>

    <LlmConfigurationFields
      idPrefix="tidy"
      provider={$tidyProvider}
      baseUrl={$tidyBaseUrl}
      apiKey={$tidyApiKey}
      model={$tidyModel}
      reasoningEffort={$tidyReasoningEffort}
      systemPrompt={$tidySystemPrompt}
      defaultSystemPrompt={DEFAULT_TIDY_SYSTEM_PROMPT}
      modelLabel={tr('Tidy model')}
      promptLabel={tr('Tidy prompt')}
      onProviderChange={chooseTidyProvider}
      onBaseUrlChange={setTidyBaseUrl}
      onApiKeyChange={setTidyApiKey}
      onModelChange={setTidyModel}
      onReasoningEffortChange={setTidyReasoningEffort}
      onSystemPromptChange={setTidySystemPrompt}
    />
  </section>

  <section class="rounded-lg border bg-card p-5">
    <div class="flex items-start gap-3">
      <span class="grid size-9 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
        <ChefHat class="size-4" />
      </span>
      <div class="min-w-0 flex-1">
        <h3 class="m-0 text-sm font-medium">{tr('Full Cook')}</h3>
        <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
          {tr('Organize the complete draft into formal feedback before submission.')}
        </p>
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={$cookingEnabled}
        aria-label={tr('Full Cook')}
        class={[
          'relative mt-0.5 h-[22px] w-10 shrink-0 rounded-full border border-transparent transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring',
          $cookingEnabled ? 'bg-primary' : 'bg-input',
        ]}
        onclick={() => setCookingEnabled(!$cookingEnabled)}
      >
        <span class={[
          'absolute left-0.5 top-0.5 size-4 rounded-full bg-background shadow-sm transition-transform',
          $cookingEnabled ? 'translate-x-5' : 'translate-x-0',
        ]}></span>
      </button>
    </div>

    {#if $cookingEnabled}
      <LlmConfigurationFields
        idPrefix="cooking"
        provider={$cookingProvider}
        baseUrl={$cookingBaseUrl}
        apiKey={$cookingApiKey}
        model={$cookingModel}
        reasoningEffort={$cookingReasoningEffort}
        systemPrompt={$cookingSystemPrompt}
        defaultSystemPrompt={DEFAULT_COOKING_SYSTEM_PROMPT}
        modelLabel={tr('Full Cook model')}
        promptLabel={tr('Full Cook prompt')}
        onProviderChange={chooseCookingProvider}
        onBaseUrlChange={setCookingBaseUrl}
        onApiKeyChange={setCookingApiKey}
        onModelChange={setCookingModel}
        onReasoningEffortChange={setCookingReasoningEffort}
        onSystemPromptChange={setCookingSystemPrompt}
      />
    {/if}
  </section>

  <p class="m-0 px-1 text-[10px] leading-4 text-muted-foreground">
    {tr('Tidy and Cooking use separate credentials and models. API keys stay in local settings and are never written to feedback packages.')}
  </p>
</div>
