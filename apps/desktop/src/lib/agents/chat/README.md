# Managed session timeline

`SessionTimeline` renders complete, ordered Application snapshots. Activity identity is
the durable activity ID, never just a tool-call ID (which agents can reuse across
turns). Tool patches replace the contents of their existing row. The host's final
tool status is authoritative; a stopped turn without one is shown as incomplete.

Messages use RambleDesk's existing schema-based Markdown renderer. Agent Markdown
cannot create attachment actions or automatically fetch arbitrary images. Structured
bounded raster/audio content can render inline. HTTP resources open through the
existing external-link capability; filesystem paths and terminal references remain
readable identifiers until a real provider can handle them.

User messages preserve the composer's literal text and quote structure. Quote actions
insert selected text (when selection is within that activity) or the full structured
result into the active composer's draft. Session keys isolate expansion state; the
owning workspace keeps drafts and scroll-follow decisions isolated.

## Codeg source and modifications

Source: <https://github.com/xintaofei/codeg>, commit
`3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1`, Apache-2.0.

- `src/components/merge/merge-diff.ts`: extracted the pure LCS line-diff engine.
- `src/lib/line-change-stats.ts`: retained pure bounded line statistics.
- `src/lib/unified-diff-generator.ts` and its tests: retained diff generation,
  large-file budget behavior and regression cases; changed local imports only.
- `src/lib/tool-call-lifecycle.ts`: adapted unsettled-status handling to the
  generated ACP contract, with explicit interrupted/incomplete presentation.
- `src/components/ai-elements/reasoning.tsx`: ported stream-phase disclosure behavior
  to native Svelte details, without React hooks or inferred durations.
- `src/components/message/plain-text-with-badges.tsx` and the shared
  `src/lib/message-quote.ts`: user text/quote semantics in Svelte. Reference badges
  are intentionally not fabricated for plain strings lacking resource metadata.

RambleDesk's structured content adapters, safe Markdown bridge, resource handling,
line-number renderer and snapshot-isolation tests are new code. No React dependency
is introduced. Large diffs disclose their visible line limit and offer more lines;
they never silently drop the remainder.
