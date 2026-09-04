# Agent management and chat preview

Run `pnpm -C apps/desktop exec vite --host 127.0.0.1 --port 1431`, then open
`http://127.0.0.1:1431/agent-preview.html`.
Append `?history` to seed 1,100 earlier messages for the paging UI.

This development-only entry mounts the actual Svelte catalog and session components
against an in-memory ApplicationTransport. It does not launch agents, install packages,
call native commands or access the application database. Catalog versions and paths
are display fixtures, not a backend compatibility matrix. Use synthetic credentials
and attachments here. Reloading resets all fixture records.

Supported checks: catalog navigation, connection form, simulated installation job,
structured thought/tool/diff/Markdown rendering, streamed text, cancellation, continued
draft editing, model confirmation, typed attachment rendering and older-message loading.
Switching the navigation away from chat keeps the in-memory agent turn running.
The theme button changes only this page. The production Vite entry does not include
this preview page; mounting is additionally gated by `import.meta.env.DEV`.

Browser acceptance on Windows: light/dark themes, 760px chat layout, immediate draft
clearing and preservation of subsequent edits, agent-confirmed model selection,
stream-to-idle updates without navigation, tool diff expansion, initial 60 rows out of
1,105 records, and expansion to 120 while preserving the prior reading position.
Real ACP processes, installation and scoped feedback are tested separately in Rust.
