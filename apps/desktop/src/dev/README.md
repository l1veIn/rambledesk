# Development previews and acceptance fixtures

Run `pnpm -C apps/desktop exec vite --host 127.0.0.1 --port 1431` from the repository root.
The interactive previews use production components with simulated transports or
capabilities. Their request/session records stay in memory; browser UI preferences
may persist. The live HTTP/model acceptance tools are documented separately below.

## Workbenches and shared input

Open `http://127.0.0.1:1431/workbenches.html`. Its four scenarios cover the three
current workbench types: `ramble`, multi-question `questions`, single-question
`questions`, and `document_review`. Use `?type=questions`, `?type=single_question`,
or `?type=document_review` to select a scenario directly. `single_question` is a
preview selector, not a workbench contract; `single_choice` is retained only for
compatibility with saved requests and is not offered for new work.

Add `&voice=1` (or `?voice=1` on the default page) for simulated transcription and
Tidy. Start recording from an input's microphone, start a segment in the preview
controls, switch inputs, then finish the segment. It must return to the original
input, while the next segment uses the new target. Type extra text, record in a
second input, then use Rambelle's Tidy button: only marked speech should change,
at its original locations. No microphone or model call is made.

Document review keeps the source immutable. Check selection/paragraph comments,
continued typing in the existing comment, a one-line collapsed preview, suggested
wording, and paragraph deletion/restoration. Nonempty comment deletion requires
confirmation. Each input uses the shared TipTap field and scoped attachment tools;
available tools depend on the simulated capabilities. Submitting displays the
workbench state and optional notes locally; it does not publish a feedback package.
The sample script deliberately includes an exaggerated capability claim for
reviewers to correct; it is test material, not product documentation.

For real saving, publishing and Agent continuation, use the
[workbench playground](../../../../playground/workbenches/README.md).

## Speech confirmation

For speech confirmation, open `http://127.0.0.1:1431/speech-review-preview.html`
(add `?locale=zh-CN` for Chinese). This uses the real draft queue, command dispatcher,
and overlay with in-memory preferences/writes and a simulated 600 ms Tidy response.
Exercise edit/save/cancel, new arrivals while editing, stale confirmation shortcuts,
manual/automatic Tidy, failure preservation, different targets and long transcripts.
The write log displays cleanup metadata; no microphone, model or database is used.
Add `?settings&locale=zh-CN` to open the real Voice settings with simulated speech
capabilities and memory-only preferences. Confirmation gates the auto Tidy switch;
its value is retained when confirmation is turned off and on again.

## Agent management and chat

Open `http://127.0.0.1:1431/agent-preview.html`.
Append `?history` to seed 60 historical turns, including a 120-tool turn.
Use `?question` to exercise ACP Agent user-input requests with single choice, custom text,
multiple choices and optional context. Submitting, declining and cancelling reply
to the pending fixture request and resume the session without sending a chat prompt.
This is the ACP user-input protocol, separate from feedback workbench `questions`.
Use `?history&longTurn` for a 1,500-tool turn that crosses the history page budget.
Use `?setup` for a missing Claude ACP component, an undiscovered Gemini CLI, and a
Pi connection failure. Add `&draft` to start in the new-task composer, or `&profiles`
to include additional saved launch profiles in the advanced catalog section.
Use `?projects&draft` to preview the project sidebar alongside the new-session
composer. It seeds multiple Agents under one folder, separate folders with the
same name, and an external session without a known project. A new session starts
without a project folder; a project's plus button supplies that folder and clears
the message. Browse opens the application's directory tree with simulated drives,
home/parent navigation and subdirectories; it does not inspect the real filesystem.
Starter cards fill an editable first prompt and do not send it automatically.
Use `?empty` to inspect the Rambelle empty state and its new-session action.
Pinning, archiving,
searching and the first sent message update only in-memory preview records.
Use `?setup&onboarding` to walk through the complete Windows onboarding: language,
storage, voice model, Agent connection, notifications, Cooking, and a new session
with the selected Agent. Add `&platform=mac` to include macOS permissions.
Storage, model downloads/progress, and permission grants use in-memory fixtures;
the preview never changes OS permissions, downloads models, or restarts the app.
The diagnostic footer uses the real frontend recorder with an in-memory sink.
Expand its timeline to inspect the latest 200 allowlisted events, operation IDs,
outcomes, and elapsed times. No diagnostic files or native IPC are written by this preview.

This development-only entry mounts the actual Svelte catalog and session components
against an in-memory ApplicationTransport. It does not launch agents, install packages,
call native commands or access the application database. Catalog versions and paths
are display fixtures, not a backend compatibility matrix. Use synthetic credentials
and attachments here. Reloading resets all fixture records.

Supported checks: catalog navigation, explicit connection checks, manual executable
paths, agent-owned setup instructions, simulated connection preparation jobs,
new-task draft preservation during preparation, advanced launch profiles,
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
Agent setup acceptance: a missing bridge stays idle until “Prepare connection” is
clicked; editing remains available during preparation; completion preserves the
draft and project directory. Pi's simulated first connection failure exposes its
setup guide and retry. An undiscovered Gemini accepts a full CLI executable path.
Saved secondary profiles remain available under advanced settings and in the
new-task selector. Connection checks do not send a model prompt.
Opening Agents, restoring focus, or switching back from a new session reuses the
in-memory detection results. The footer counts mock discovery and ACP checks so
unintended probes are visible. First-run discovery runs once; returning to its
connection step preserves the selected Agent without scanning again.
ACP is the recommended integrated workflow. External adapters provide a lightweight
way to keep working in an agent app or CLI, with RambleDesk handling feedback
requests and replies. The Settings preview uses inert external-adapter capabilities.
General and Agents must leave the external-adapter call counter empty; entering
External adapters triggers only that page's mock MCP/Pi/DSH/configuration reads.
Working details initially mount the latest 60 entries; “Show earlier work” reveals
60 more at a time. Completed turns mount no working details until expanded.
Real ACP processes, installation and scoped feedback are tested separately in Rust.

## Startup and navigation

Open `http://127.0.0.1:1431/startup-preview.html` to exercise the independent
startup error screen; add `?timeout` for a stalled component load. The screen
does not depend on preferences, Svelte, the database or native bindings. Reload
and copying its static error information remain available after startup fails.

Open `http://127.0.0.1:1431/workbench-startup-preview.html` for a mounted workbench
whose first session read fails; add `?timeout` for the real 30-second read deadline.
This page uses memory-only preferences and simulated diagnostics. Sidebar Settings
must open About, diagnostic export increments the mock counter, and Retry loading
must recover on the second read. External-adapter calls remain `{}` throughout.

Add `?session-routing` to `workbench-startup-preview.html` to mount the real App
with an ACP session and no feedback requests. Clicking its project sidebar row
must open session details, leave the Agent read counter at zero, and retain the
explicit View Agent button. Only clicking that button opens the Agent tab and
reads its in-memory snapshot. This mode skips the simulated startup failure;
the default and `?timeout` scenarios are unchanged.

Use `workbench-startup-preview.html?cancelled-agent-restore` to restore a saved
Agent tab whose latest feedback was cancelled. It must land on the cancelled
feedback details with Agent reads and ACP connections both zero. The preserved
Agent tab or View Agent button connects once when explicitly opened. Add
`&lookup-error` to verify that a failed cancellation lookup reaches the startup
error screen without launching ACP. All preferences and calls remain in memory.

The startup preview's About diagnostics controls support simulated recording
on/off and clearing; the footer displays the current choice and clear count.
Disabling keeps export available. Clearing preserves the selected setting and
previous export path. The standalone settings preview above starts with automatic
waiting-request preview off when no preference has been saved.
Add `&hold-restore` to pause the restoration query until the footer button is
clicked: Agent reads and ACP connections must stay zero both before and after
releasing a cancelled restoration. This exercises the real component mount order.
Add `&latest=completed` to check that an ordinary restored Agent tab still connects
once after its latest-feedback lookup succeeds.

Use `agent-preview.html?feedback-status` to inspect the actual workspace header
and managed feedback status together. Buttons simulate normal delivery, a long
uncertain-delivery error, loading, read failure, deletion, running and permission
states; `&state=uncertain` opens the error case directly. The line below the header
makes vertical layout movement visible. Header height and its 288px status column
come from production components. Resolution actions and invalidations stay in
memory; no Agent, native binding or database is used.

Open `http://127.0.0.1:1431/workbench-shell-preview.html` for responsive rails,
phone drawers, scrolling and keyboard focus. Use `navigation-resize-preview.html`
for persisted rail widths; its storage keys have a dedicated preview prefix.

## Live acceptance tools

The [HTTP fixture guide](../../../../docs/quality/FEEDBACK_ACCEPTANCE.md) and
[performance acceptance guide](../../../../docs/quality/PERFORMANCE_ACCEPTANCE.md)
describe the disposable backend and opt-in quality build. These use real HTTP and
SQLite, unlike the component previews. `cookingAcceptance.test.ts` is deliberately
skipped unless all three `RAMBLEDESK_COOKING_ACCEPTANCE_*` paths are supplied; it
uses an existing model configuration and a disposable fixture. A normal unit-test
run does not exercise this live acceptance case.
