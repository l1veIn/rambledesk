# Agent management and chat preview

Run `pnpm -C apps/desktop exec vite --host 127.0.0.1 --port 1431`, then open
`http://127.0.0.1:1431/agent-preview.html`.
Append `?history` to seed 60 historical turns, including a 120-tool turn.
Use `?history&longTurn` for a 1,500-tool turn that crosses the history page budget.

This development-only entry mounts the actual Svelte catalog and session components
against an in-memory ApplicationTransport. It does not launch agents, install packages,
call native commands or access the application database. Catalog versions and paths
are display fixtures, not a backend compatibility matrix. Use synthetic credentials
and attachments here. Reloading resets all fixture records.

Supported checks: catalog navigation, connection form, simulated installation job,
structured thought/tool/diff/Markdown rendering, streamed text, cancellation, continued
draft editing, model confirmation, historical rich-content rendering and older-turn loading.
The Agent composer sends text; attachments belong to Ramble requests.
Switching the navigation away from chat keeps the in-memory agent turn running.
The theme button changes only this page. The production Vite entry does not include
this preview page; mounting is additionally gated by `import.meta.env.DEV`.

Browser acceptance on Windows: light/dark themes, 760px chat layout, immediate draft
clearing and preservation of subsequent edits, agent-confirmed model selection,
stream-to-idle updates without navigation, tool diff expansion, initial 20 turns,
and upward automatic paging while preserving the prior reading position.
Working details initially mount the latest 60 entries; “Show earlier work” reveals
60 more at a time. Completed turns mount no working details until expanded.
Real ACP processes, installation and scoped feedback are tested separately in Rust.
