<script lang="ts">
  import { ChefHat, Sparkles, TriangleAlert } from '@lucide/svelte'

  import * as Alert from '$lib/components/ui/alert'
  import { Button } from '$lib/components/ui/button'
  import * as Dialog from '$lib/components/ui/dialog'
  import * as Select from '$lib/components/ui/select'
  import { DEFAULT_COOKING_SYSTEM_PROMPT } from '$lib/cooking'
  import { t } from '$lib/i18n'
  import { DEFAULT_LIGHT_CLEANUP_SYSTEM_PROMPT } from '$lib/lightCleanup'
  import {
    cookingApiKey,
    cookingBaseUrl,
    cookingEnabled,
    cookingModel,
    cookingProvider,
    cookingReasoningEffort,
    cookingSystemPrompt,
    lightCleanupApiKey,
    lightCleanupBaseUrl,
    lightCleanupCharThreshold,
    lightCleanupEnabled,
    lightCleanupIdleMs,
    lightCleanupModel,
    lightCleanupProvider,
    lightCleanupReasoningEffort,
    lightCleanupSegmentThreshold,
    lightCleanupSystemPrompt,
    lightCleanupTimeoutMs,
    locale,
    setCookingApiKey,
    setCookingBaseUrl,
    setCookingEnabled,
    setCookingModel,
    setCookingProvider,
    setCookingReasoningEffort,
    setCookingSystemPrompt,
    setLightCleanupCharThreshold,
    setLightCleanupEnabled,
    setLightCleanupApiKey,
    setLightCleanupBaseUrl,
    setLightCleanupIdleMs,
    setLightCleanupModel,
    setLightCleanupProvider,
    setLightCleanupReasoningEffort,
    setLightCleanupSegmentThreshold,
    setLightCleanupSystemPrompt,
    setLightCleanupTimeoutMs,
    type CookingProvider,
    type CookingReasoningEffort,
  } from '$lib/preferences'

  let conflict: 'cooking' | 'cleanup' | null = null

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
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

  function chooseCleanupProvider(provider: CookingProvider) {
    setLightCleanupProvider(provider)
    if (provider === 'deepseek') {
      setLightCleanupBaseUrl('https://api.deepseek.com/v1')
      setLightCleanupModel('deepseek-v4-flash')
    } else if (provider === 'openai') {
      setLightCleanupBaseUrl('https://api.openai.com/v1')
      setLightCleanupModel('gpt-4.1-mini')
    }
  }

  function requestCookingToggle() {
    if ($cookingEnabled) return setCookingEnabled(false)
    if ($lightCleanupEnabled) conflict = 'cooking'
    else setCookingEnabled(true)
  }

  function requestCleanupToggle() {
    if ($lightCleanupEnabled) return setLightCleanupEnabled(false)
    if ($cookingEnabled) conflict = 'cleanup'
    else setLightCleanupEnabled(true)
  }

  function confirmBoth() {
    if (conflict === 'cooking') setCookingEnabled(true)
    if (conflict === 'cleanup') setLightCleanupEnabled(true)
    conflict = null
  }
</script>

<div class="space-y-6">
  {#if $cookingEnabled && $lightCleanupEnabled}
    <Alert.Root class="border-warning/35 bg-warning/5">
      <TriangleAlert />
      <Alert.Title>{tr('Both tidying options are enabled')}</Alert.Title>
      <Alert.Description>
        {tr('Tidy as you speak works along the way, then Full Cook organizes everything once more at the end.')}
      </Alert.Description>
    </Alert.Root>
  {/if}

  <section class="rounded-lg border bg-card p-5">
    <div class="flex items-start gap-3">
      <span class="grid size-9 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
        <Sparkles class="size-4" />
      </span>
      <div class="min-w-0 flex-1">
        <h3 class="m-0 text-sm font-medium">{tr('Tidy as you speak')}</h3>
        <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
          {tr('Tidy while you speak: remove filler words and smooth out broken sentences.')}
        </p>
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={$lightCleanupEnabled}
        aria-label={tr('Tidy as you speak')}
        class={[
          'relative mt-0.5 h-[22px] w-10 shrink-0 rounded-full border border-transparent transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring',
          $lightCleanupEnabled ? 'bg-primary' : 'bg-input',
        ]}
        onclick={requestCleanupToggle}
      >
        <span class={[
          'absolute left-0.5 top-0.5 size-4 rounded-full bg-background shadow-sm transition-transform',
          $lightCleanupEnabled ? 'translate-x-5' : 'translate-x-0',
        ]}></span>
      </button>
    </div>

    {#if $lightCleanupEnabled}
      <div class="mt-5 grid max-h-80 grid-cols-[150px_minmax(0,1fr)] items-center gap-x-4 gap-y-3 overflow-y-auto overscroll-contain border-t pr-2 pt-5">
        <label for="cleanup-provider" class="text-xs font-medium">{tr('Model provider')}</label>
        <Select.Root type="single" value={$lightCleanupProvider} onValueChange={(value: string) => chooseCleanupProvider(value as CookingProvider)}>
          <Select.Trigger id="cleanup-provider" class="w-full">
            {$lightCleanupProvider === 'deepseek' ? 'DeepSeek' : $lightCleanupProvider === 'openai' ? 'OpenAI' : tr('OpenAI-compatible service')}
          </Select.Trigger>
          <Select.Content>
            <Select.Item value="deepseek" label="DeepSeek" />
            <Select.Item value="openai" label="OpenAI" />
            <Select.Item value="compatible" label={tr('OpenAI-compatible service')} />
          </Select.Content>
        </Select.Root>

        <label for="cleanup-base-url" class="text-xs font-medium">Base URL</label>
        <input id="cleanup-base-url" type="url" value={$lightCleanupBaseUrl} placeholder="https://api.example.com/v1" class="h-9 w-full rounded-md border bg-background px-3 text-xs" oninput={(event) => setLightCleanupBaseUrl((event.currentTarget as HTMLInputElement).value)} />

        <label for="cleanup-api-key" class="text-xs font-medium">API Key</label>
        <input id="cleanup-api-key" type="password" autocomplete="off" value={$lightCleanupApiKey} placeholder="sk-…" class="h-9 w-full rounded-md border bg-background px-3 text-xs" oninput={(event) => setLightCleanupApiKey((event.currentTarget as HTMLInputElement).value)} />

        <label for="cleanup-model" class="text-xs font-medium">{tr('Tidy model')}</label>
        <input id="cleanup-model" value={$lightCleanupModel} class="h-9 rounded-md border bg-background px-3 text-xs" oninput={(event) => setLightCleanupModel((event.currentTarget as HTMLInputElement).value)} />

        <label for="cleanup-reasoning" class="text-xs font-medium">{tr('Reasoning effort')}</label>
        <Select.Root type="single" value={$lightCleanupReasoningEffort} onValueChange={(value: string) => setLightCleanupReasoningEffort(value as CookingReasoningEffort)}>
          <Select.Trigger id="cleanup-reasoning" class="w-full">{$lightCleanupReasoningEffort === 'none' ? tr('None') : $lightCleanupReasoningEffort}</Select.Trigger>
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

        <label for="cleanup-segment-threshold" class="text-xs font-medium">{tr('Tidy after this many voice segments')}</label>
        <input id="cleanup-segment-threshold" type="number" min="1" max="20" value={$lightCleanupSegmentThreshold} class="h-9 rounded-md border bg-background px-3 text-xs" oninput={(event) => setLightCleanupSegmentThreshold(Number((event.currentTarget as HTMLInputElement).value))} />

        <label for="cleanup-char-threshold" class="text-xs font-medium">{tr('Tidy after this many characters')}</label>
        <input id="cleanup-char-threshold" type="number" min="100" max="5000" step="50" value={$lightCleanupCharThreshold} class="h-9 rounded-md border bg-background px-3 text-xs" oninput={(event) => setLightCleanupCharThreshold(Number((event.currentTarget as HTMLInputElement).value))} />

        <label for="cleanup-idle-seconds" class="text-xs font-medium">{tr('Tidy after no new speech for')}</label>
        <div class="flex items-center gap-2">
          <input id="cleanup-idle-seconds" type="number" min="3" max="120" value={$lightCleanupIdleMs / 1000} class="h-9 min-w-0 flex-1 rounded-md border bg-background px-3 text-xs" oninput={(event) => setLightCleanupIdleMs(Number((event.currentTarget as HTMLInputElement).value) * 1000)} />
          <span class="text-xs text-muted-foreground">{tr('seconds')}</span>
        </div>

        <label for="cleanup-timeout-seconds" class="text-xs font-medium">{tr('Maximum wait')}</label>
        <div class="flex items-center gap-2">
          <input id="cleanup-timeout-seconds" type="number" min="5" max="120" value={$lightCleanupTimeoutMs / 1000} class="h-9 min-w-0 flex-1 rounded-md border bg-background px-3 text-xs" oninput={(event) => setLightCleanupTimeoutMs(Number((event.currentTarget as HTMLInputElement).value) * 1000)} />
          <span class="text-xs text-muted-foreground">{tr('seconds')}</span>
        </div>

        <label for="light-cleanup-system-prompt" class="self-start pt-2 text-xs font-medium">{tr('Tidy prompt')}</label>
        <div class="grid gap-2">
          <textarea id="light-cleanup-system-prompt" rows="7" value={$lightCleanupSystemPrompt || DEFAULT_LIGHT_CLEANUP_SYSTEM_PROMPT} class="min-h-36 w-full resize-y rounded-md border bg-background px-3 py-2 font-mono text-[11px] leading-5" oninput={(event) => setLightCleanupSystemPrompt((event.currentTarget as HTMLTextAreaElement).value)}></textarea>
          <Button variant="outline" size="xs" class="justify-self-end" disabled={!$lightCleanupSystemPrompt} onclick={() => setLightCleanupSystemPrompt('')}>{tr('Reset to default')}</Button>
        </div>
      </div>
    {/if}
  </section>

  <section class="rounded-lg border bg-card p-5">
    <div class="flex items-start gap-3">
      <span class="grid size-9 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
        <ChefHat class="size-4" />
      </span>
      <div class="min-w-0 flex-1">
        <h3 class="m-0 text-sm font-medium">{tr('Full Cook')}</h3>
        <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
          {tr('When you are done speaking, organize everything into a structured prompt.')}
        </p>
      </div>
      <button type="button" role="switch" aria-checked={$cookingEnabled} aria-label={tr('Full Cook')} class={['relative mt-0.5 h-[22px] w-10 shrink-0 rounded-full border border-transparent transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring', $cookingEnabled ? 'bg-primary' : 'bg-input']} onclick={requestCookingToggle}>
        <span class={['absolute left-0.5 top-0.5 size-4 rounded-full bg-background shadow-sm transition-transform', $cookingEnabled ? 'translate-x-5' : 'translate-x-0']}></span>
      </button>
    </div>

    {#if $cookingEnabled}
      <div class="mt-5 grid max-h-80 grid-cols-[150px_minmax(0,1fr)] items-center gap-x-4 gap-y-3 overflow-y-auto overscroll-contain border-t pr-2 pt-5">
        <label for="cooking-provider" class="text-xs font-medium">{tr('Model provider')}</label>
        <Select.Root type="single" value={$cookingProvider} onValueChange={(value: string) => chooseCookingProvider(value as CookingProvider)}>
          <Select.Trigger id="cooking-provider" class="w-full">
            {$cookingProvider === 'deepseek' ? 'DeepSeek' : $cookingProvider === 'openai' ? 'OpenAI' : tr('OpenAI-compatible service')}
          </Select.Trigger>
          <Select.Content>
            <Select.Item value="deepseek" label="DeepSeek" />
            <Select.Item value="openai" label="OpenAI" />
            <Select.Item value="compatible" label={tr('OpenAI-compatible service')} />
          </Select.Content>
        </Select.Root>

        <label for="cooking-base-url" class="text-xs font-medium">Base URL</label>
        <input id="cooking-base-url" type="url" value={$cookingBaseUrl} placeholder="https://api.example.com/v1" class="h-9 w-full rounded-md border bg-background px-3 text-xs" oninput={(event) => setCookingBaseUrl((event.currentTarget as HTMLInputElement).value)} />

        <label for="cooking-api-key" class="text-xs font-medium">API Key</label>
        <input id="cooking-api-key" type="password" autocomplete="off" value={$cookingApiKey} placeholder="sk-…" class="h-9 w-full rounded-md border bg-background px-3 text-xs" oninput={(event) => setCookingApiKey((event.currentTarget as HTMLInputElement).value)} />

        <label for="cooking-model" class="text-xs font-medium">{tr('Full Cook model')}</label>
        <input id="cooking-model" value={$cookingModel} class="h-9 rounded-md border bg-background px-3 text-xs" oninput={(event) => setCookingModel((event.currentTarget as HTMLInputElement).value)} />

        <label for="cooking-reasoning" class="text-xs font-medium">{tr('Reasoning effort')}</label>
        <Select.Root type="single" value={$cookingReasoningEffort} onValueChange={(value: string) => setCookingReasoningEffort(value as CookingReasoningEffort)}>
          <Select.Trigger id="cooking-reasoning" class="w-full">{$cookingReasoningEffort === 'none' ? tr('None') : $cookingReasoningEffort}</Select.Trigger>
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

        <label for="cooking-system-prompt" class="self-start pt-2 text-xs font-medium">{tr('Full Cook prompt')}</label>
        <div class="grid gap-2">
          <textarea id="cooking-system-prompt" rows="8" value={$cookingSystemPrompt || DEFAULT_COOKING_SYSTEM_PROMPT} class="min-h-40 w-full resize-y rounded-md border bg-background px-3 py-2 font-mono text-[11px] leading-5" oninput={(event) => setCookingSystemPrompt((event.currentTarget as HTMLTextAreaElement).value)}></textarea>
          <Button variant="outline" size="xs" class="justify-self-end" disabled={!$cookingSystemPrompt} onclick={() => setCookingSystemPrompt('')}>{tr('Reset to default')}</Button>
        </div>
      </div>
    {/if}
  </section>

  <p class="m-0 px-1 text-[10px] leading-4 text-muted-foreground">
    {tr('The API key is stored only in local settings on this device and is never written to feedback packages.')}
  </p>
</div>

<Dialog.Root open={conflict !== null} onOpenChange={(open) => { if (!open) conflict = null }}>
  <Dialog.Content class="max-w-md">
    <Dialog.Header>
      <Dialog.Title>{tr('Enable both tidying options?')}</Dialog.Title>
      <Dialog.Description>
        {tr('Your words will be tidied as you speak, then organized again by Full Cook at the end. The wording may change twice.')}
      </Dialog.Description>
    </Dialog.Header>
    <Dialog.Footer>
      <Button variant="outline" onclick={() => (conflict = null)}>{tr('Cancel')}</Button>
      <Button onclick={confirmBoth}>{tr('Enable both')}</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
