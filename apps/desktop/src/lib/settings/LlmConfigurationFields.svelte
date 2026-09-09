<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import * as Select from '$lib/components/ui/select'
  import { t } from '$lib/i18n'
  import {
    locale,
    type CookingProvider,
    type CookingReasoningEffort,
  } from '$lib/preferences'

  export let idPrefix: string
  export let provider: CookingProvider
  export let baseUrl: string
  export let apiKey: string
  export let model: string
  export let reasoningEffort: CookingReasoningEffort
  export let systemPrompt: string
  export let defaultSystemPrompt: string
  export let modelLabel: string
  export let promptLabel: string
  export let onProviderChange: (provider: CookingProvider) => void
  export let onBaseUrlChange: (value: string) => void
  export let onApiKeyChange: (value: string) => void
  export let onModelChange: (value: string) => void
  export let onReasoningEffortChange: (value: CookingReasoningEffort) => void
  export let onSystemPromptChange: (value: string) => void

  function tr(source: string) {
    return t($locale, source)
  }
</script>

<div class="mt-5 grid grid-cols-[150px_minmax(0,1fr)] items-center gap-x-4 gap-y-3 border-t pt-5">
  <label for={`${idPrefix}-provider`} class="text-xs font-medium">{tr('Model provider')}</label>
  <Select.Root
    type="single"
    value={provider}
    onValueChange={(value: string) => onProviderChange(value as CookingProvider)}
  >
    <Select.Trigger id={`${idPrefix}-provider`} class="w-full">
      {provider === 'deepseek'
        ? 'DeepSeek'
        : provider === 'openai'
          ? 'OpenAI'
          : tr('OpenAI-compatible service')}
    </Select.Trigger>
    <Select.Content>
      <Select.Item value="deepseek" label="DeepSeek" />
      <Select.Item value="openai" label="OpenAI" />
      <Select.Item value="compatible" label={tr('OpenAI-compatible service')} />
    </Select.Content>
  </Select.Root>

  <label for={`${idPrefix}-base-url`} class="text-xs font-medium">Base URL</label>
  <input
    id={`${idPrefix}-base-url`}
    type="url"
    value={baseUrl}
    placeholder="https://api.example.com/v1"
    class="h-9 w-full rounded-md border bg-background px-3 text-xs"
    oninput={(event) => onBaseUrlChange((event.currentTarget as HTMLInputElement).value)}
  />

  <label for={`${idPrefix}-api-key`} class="text-xs font-medium">API Key</label>
  <input
    id={`${idPrefix}-api-key`}
    type="password"
    autocomplete="off"
    value={apiKey}
    placeholder="sk-…"
    class="h-9 w-full rounded-md border bg-background px-3 text-xs"
    oninput={(event) => onApiKeyChange((event.currentTarget as HTMLInputElement).value)}
  />

  <label for={`${idPrefix}-model`} class="text-xs font-medium">{modelLabel}</label>
  <input
    id={`${idPrefix}-model`}
    value={model}
    class="h-9 w-full rounded-md border bg-background px-3 text-xs"
    oninput={(event) => onModelChange((event.currentTarget as HTMLInputElement).value)}
  />

  <label for={`${idPrefix}-reasoning`} class="text-xs font-medium">{tr('Reasoning effort')}</label>
  <Select.Root
    type="single"
    value={reasoningEffort}
    onValueChange={(value: string) =>
      onReasoningEffortChange(value as CookingReasoningEffort)}
  >
    <Select.Trigger id={`${idPrefix}-reasoning`} class="w-full">
      {reasoningEffort === 'none' ? tr('None') : reasoningEffort}
    </Select.Trigger>
    <Select.Content>
      <Select.Item value="none" label={tr('None')} />
      <Select.Item value="minimal" label="minimal" />
      <Select.Item value="low" label="low" />
      <Select.Item value="medium" label="medium" />
      <Select.Item value="high" label="high" />
      <Select.Item value="xhigh" label="xhigh" />
      <Select.Item value="max" label="max" />
    </Select.Content>
  </Select.Root>

  <label for={`${idPrefix}-system-prompt`} class="self-start pt-2 text-xs font-medium">
    {promptLabel}
  </label>
  <div class="grid gap-2">
    <textarea
      id={`${idPrefix}-system-prompt`}
      rows="7"
      value={systemPrompt || defaultSystemPrompt}
      class="min-h-36 w-full resize-y rounded-md border bg-background px-3 py-2 font-mono text-[11px] leading-5"
      oninput={(event) => onSystemPromptChange((event.currentTarget as HTMLTextAreaElement).value)}
    ></textarea>
    <Button
      variant="outline"
      size="xs"
      class="justify-self-end"
      disabled={!systemPrompt}
      onclick={() => onSystemPromptChange('')}
    >
      {tr('Reset to default')}
    </Button>
  </div>
</div>
