<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import type { SessionInputResponse } from '$lib/generated/feedback'
  import { locale } from '$lib/preferences'
  import { agentText } from './agentI18n'
  import { redactAgentMessage } from './agentConfigForm'
  import { inputFields, inputResponse, type InputField } from './sessionInputForm'

  export let schema: Record<string, unknown>
  export let requestId: string
  export let kind: 'question' | 'plan' = 'question'
  export let disabled = false
  export let envText = ''
  export let onRespond: (response: SessionInputResponse) => Promise<void> | void
  let values: Record<string, unknown> = Object.create(null)
  let custom: Record<string, string> = Object.create(null)
  let error = ''
  let fields: InputField[] = []
  let schemaError = ''
  $: { try { fields = inputFields(schema); schemaError = '' } catch (cause) { fields = []; schemaError = String((cause as Error).message) } }
  function tr(value: string) { return agentText($locale, value) }
  function display(value: unknown) { return redactAgentMessage(typeof value === 'string' ? tr(value) : '', envText) }
  function set(id: string, value: unknown) { values = { ...values, [id]: value }; error = '' }
  function select(field: InputField, value: string) { custom = { ...custom, [field.id]: '' }; set(field.id, value) }
  function toggle(field: InputField, value: string, checked: boolean) {
    const current = Array.isArray(values[field.id]) ? values[field.id] as string[] : []
    set(field.id, checked ? [...current.filter(item => item !== value), value] : current.filter(item => item !== value))
  }
  function setCustom(field: InputField, value: string) {
    if (field.type === 'array') {
      const current = Array.isArray(values[field.id]) ? values[field.id] as string[] : []
      set(field.id, [...new Set([...current.filter(item => field.choices.some(choice => choice.value === item)), ...(value.trim() ? [value] : [])])])
    } else set(field.id, value)
    custom = { ...custom, [field.id]: value }
  }
  async function submit(event: SubmitEvent) {
    event.preventDefault()
    if (disabled || schemaError) return
    try { const response = inputResponse(fields, values); error = ''; await onRespond(response) }
    catch (cause) { error = cause instanceof Error ? cause.message : String(cause) }
  }
</script>

<form onsubmit={submit} class="mt-3 space-y-4" aria-label={tr(kind === 'plan' ? 'Review agent plan' : 'Agent question')}>
  {#if schema.description}<p class="m-0 whitespace-pre-wrap text-xs leading-5 text-muted-foreground">{display(schema.description)}</p>{/if}
  {#if schemaError}<p role="alert" class="text-xs text-destructive">{tr(schemaError)}</p>{/if}
  {#each fields as field, index (field.id)}
    <fieldset disabled={disabled} class="min-w-0 space-y-2">
      <legend class="mb-2 whitespace-pre-wrap text-xs font-medium">{display(field.label)}{#if field.required}<span aria-label={tr('Required answer')} class="ml-1 text-amber-600">*</span>{/if}</legend>
      {#if field.description}<p class="m-0 whitespace-pre-wrap text-xs text-muted-foreground">{display(field.description)}</p>{/if}
      {#if field.choices.length}
        <div class="flex flex-wrap gap-2">
          {#each field.choices as choice (choice.value)}
            <label class="flex min-w-0 cursor-pointer items-start gap-2 rounded-lg border bg-background/70 px-3 py-2 text-xs has-[:checked]:border-primary has-[:checked]:bg-primary/5">
              <input type={field.type === 'array' ? 'checkbox' : 'radio'} name={`${requestId}:${index}`} value={choice.value} class="mt-0.5 accent-primary"
                checked={field.type === 'array' ? Array.isArray(values[field.id]) && (values[field.id] as string[]).includes(choice.value) : values[field.id] === choice.value}
                onchange={event => field.type === 'array' ? toggle(field, choice.value, event.currentTarget.checked) : select(field, choice.value)} />
              <span class="min-w-0 whitespace-pre-wrap break-words">{display(choice.label)}{#if choice.description}<span class="mt-1 block text-muted-foreground">{display(choice.description)}</span>{/if}</span>
            </label>
          {/each}
        </div>
        {#if field.allowOther}
          <label class="block space-y-1 text-xs text-muted-foreground"><span>{tr('Custom answer')}</span>
            <input class="w-full rounded-lg border bg-background px-3 py-2 text-foreground" type="text" value={Object.hasOwn(custom, field.id) ? custom[field.id] : ''} oninput={event => setCustom(field, event.currentTarget.value)} />
          </label>
        {/if}
      {:else if field.type === 'boolean'}
        <div class="flex gap-3">
          {#each [true, false] as value}
            <label class="flex items-center gap-2 text-xs"><input type="radio" name={`${requestId}:${index}`} checked={values[field.id] === value} onchange={() => set(field.id, value)} />{tr(value ? 'Yes' : 'No')}</label>
          {/each}
        </div>
      {:else if field.type === 'number' || field.type === 'integer'}
        <input aria-label={display(field.label)} type="number" step={field.type === 'integer' ? 1 : 'any'}
          min={typeof field.schema.minimum === 'number' ? field.schema.minimum : undefined} max={typeof field.schema.maximum === 'number' ? field.schema.maximum : undefined}
          value={typeof values[field.id] === 'string' ? values[field.id] as string : ''}
          oninput={event => set(field.id, event.currentTarget.value)} class="w-full rounded-lg border bg-background px-3 py-2 text-xs" />
      {:else}
        <textarea aria-label={display(field.label)} rows="2" value={typeof values[field.id] === 'string' ? values[field.id] as string : ''}
          oninput={event => set(field.id, event.currentTarget.value)} class="w-full resize-y rounded-lg border bg-background px-3 py-2 text-xs leading-5"></textarea>
      {/if}
    </fieldset>
  {/each}
  {#if error}<p role="alert" class="text-xs text-destructive">{display(tr(error))}</p>{/if}
  <div class="sticky bottom-0 flex flex-wrap gap-2 bg-background/95 py-2">
    <Button type="submit" size="sm" disabled={disabled || !!schemaError}>{tr(kind === 'plan' ? 'Submit decision' : 'Submit answer')}</Button>
    <Button type="button" variant="outline" size="sm" {disabled} onclick={() => onRespond({ action: 'decline', content: null })}>{tr(kind === 'plan' ? 'Decline plan' : 'Decline question')}</Button>
    <Button type="button" variant="ghost" size="sm" {disabled} onclick={() => onRespond({ action: 'cancel', content: null })}>{tr(kind === 'plan' ? 'Cancel review' : 'Cancel question')}</Button>
  </div>
</form>
