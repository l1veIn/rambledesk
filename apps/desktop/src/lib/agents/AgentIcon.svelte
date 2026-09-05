<script lang="ts">
  import claude from '../../../../../crates/rambledesk-hosts/assets/icons/claude.svg?raw'
  import codex from '../../../../../crates/rambledesk-hosts/assets/icons/openai.svg?raw'
  import cursor from '../../../../../crates/rambledesk-hosts/assets/icons/cursor.svg?raw'
  import gemini from '../../../../../crates/rambledesk-hosts/assets/icons/google-gemini.svg?raw'
  import antigravity from '../../../../../crates/rambledesk-hosts/assets/icons/antigravity.svg?raw'
  import grok from '../../../../../crates/rambledesk-hosts/assets/icons/grok.svg?raw'
  import pi from '../../../../../crates/rambledesk-hosts/assets/icons/pi.svg?raw'
  import dsh from '../../../../../crates/rambledesk-hosts/assets/icons/dsh.svg?raw'
  import opencode from '../../../../../crates/rambledesk-hosts/assets/icons/opencode.svg?raw'
  import reasonix from '../../../../../crates/rambledesk-hosts/assets/icons/reasonix.svg?raw'
  import inspector from '../../../../../crates/rambledesk-hosts/assets/icons/model-context-protocol.svg?raw'
  import generic from '../../../../../crates/rambledesk-hosts/assets/icons/generic-terminal.svg?raw'

  export let hostId: string | undefined = undefined
  let className = 'size-4'
  export { className as class }

  // Reuse the adapter profiles' bundled SVGs without a request per icon.
  const icons: Record<string, string> = { claude, codex, cursor, gemini, antigravity, grok, pi, dsh, opencode, reasonix, inspector }
  $: id = hostId?.trim().toLowerCase() ?? ''
  $: icon = Object.hasOwn(icons, id) ? icons[id] : generic
</script>

<span class={`inline-flex shrink-0 items-center justify-center [&>svg]:size-full ${className}`} aria-hidden="true">{@html icon}</span>
