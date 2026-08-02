# Dogfooding rounds

## 2026-08-01 — floating console and blocking feedback wait

### Round 1: static and automated audit

- Scope: desktop title bar, settings entry points, floating Ramble console, Tauri icons, MCP feedback lifecycle, macOS compilation.
- Findings: the console used a wide text layout and had no reliable drag surface; it was centered instead of kept out of the user's way; feedback completion required repeated polling; the macOS audio stream owner was not `Send`; web preview mounted native Tauri listeners unconditionally.
- Fixes: replaced the console with a narrow vertical icon toolbar and explicit drag surface; positioned it at right-center with a 10 px logical inset; added `wait_for_feedback`; moved the native audio owner onto a dedicated thread; guarded native-only browser hooks; regenerated Tauri icon sizes from the current app icon.
- Evidence: `pnpm check`, `pnpm test`, `pnpm build:web`, `cargo fmt --all --check`, `cargo test --workspace`, and strict workspace clippy passed. MCP self-test and the Inspector smoke test list and invoke `wait_for_feedback`.

### Round 2: browser visual and interaction regression

- Scope: settings routing, language/theme controls, responsive shell, content pattern, floating console presentation.
- Findings: before native-hook guards, a plain browser preview entered a broken Svelte effect when Tauri metadata was unavailable. This prevented reliable interaction testing.
- Fixes: native lifecycle hooks now degrade cleanly outside Tauri while settings remain testable; opening settings no longer depends on the MCP configuration request succeeding.
- Evidence: General opens from the title-bar button; the Local MCP card opens the MCP section directly; Chinese/English and system/light/dark choices update the UI; 1320 px, 1180 px, and 980 px viewports have no horizontal overflow; the configured pattern layer is present; the console route renders an icon-only vertical toolbar.

### Remaining native acceptance

- macOS native interaction is blocked because the test machine is locked. Window dragging, native notification permission, microphone capture, and subjective seam/contrast inspection require an unlocked desktop.
- Windows native acceptance remains required for the requested 1320 px, 1180 px, and 980 px command-rail checks and platform-specific window controls.

## 2026-08-01 — native macOS acceptance

### Goal

- Exercise the current debug application bundle as a real operator and complete a feedback package while an MCP client is blocked in `wait_for_feedback`.

### Tester path

1. Opened General and MCP settings from their dedicated entry points, switched languages and all appearance modes, and toggled notifications off and back on.
2. Created durable local requests through the authenticated MCP endpoint and opened their full editor workspaces.
3. Started Ramble, inspected and dragged the vertical console, paused and resumed speech capture, invoked capture, and exited back to the main window.
4. Saved and submitted a rich-text feedback document while the Inspector client waited in `wait_for_feedback`.

### Findings

- The macOS speech path initially reported the expected missing-model error because model distribution is intentionally outside Git.
- The non-Windows region-capture availability error remained Chinese in an English UI.
- No layout overflow, pattern seam, command-rail clipping, notification-state failure, or native window-control regression was observed.

### Fixes shipped

- Installed the manifest-pinned X-ASR model into local application data after verifying the archive and every required file against the committed SHA-256 values.
- Localized the non-Windows region-capture availability message.

### Validation

- The native console rendered as a compact icon-only vertical toolbar and exposed hover/help labels for every action.
- Speech reached active capture, then paused, resumed, and exited cleanly.
- The completed workspace became read-only, showed the archived Rambelle portrait, and exposed its immutable Feedback Package.
- The blocked Inspector call returned once with `execution_mode: wait`, the full manifest, Markdown, and attachment paths after UI submission.
- Windows native acceptance was removed from this iteration at the user's direction because the active development environment is macOS; exact 1320 px, 1180 px, and 980 px browser regressions remain covered separately.

### Next focus

- Run the final automated regression, remove ignored design/history payloads from the final tree, and Squash merge the iteration.

## 2026-08-02 — adapter polish, Pi one-click install, and UI fixes

### Goal

- Verify the previous round's two feedback fixes (generic MCP collapsed by default, Pi one-click install) and collect the next set of operator findings.

### Fixes shipped

- Generic MCP adapter section is now a `<details>` collapsed by default; detection, selection, and install live inside the expanded body.
- New `install_pi_package` Tauri command resolves `packages/pi-rambledesk` from the project root or by walking up from the working/executable directory, locates the `pi` CLI (`RAMBLEDESK_PI_BIN` override), and runs `pi install` with a success/error surface in the UI.
- Settings backdrop now matches the window's 16 px bottom corner radius (the blur box previously showed square corners beyond the rounded window).
- Settings sidebar nav now keeps the chevron flush right with the count badge grouped beside it instead of squeezing the arrow.
- Removed the useless "Source package available" badge from the Pi native adapter card.
- Settings dialog now closes on Escape.
- Pi package (`packages/pi-rambledesk`) generates a stable per-call request id and retries transient connection failures with backoff, so the observed "request created but response lost" flake no longer produces duplicate requests; the server-side wait remains unbounded (verified: `wait_for_feedback` has no timeout).
- `docs/UI_SHADCN_MIGRATION.md` written: full shadcn-svelte migration plan with token mapping, component inventory, step ordering, and acceptance gates. Migration is deferred to future iterations by operator decision.

### Validation

- `cargo fmt --all --check`, strict workspace clippy, `cargo test --workspace --all-targets`, `pnpm check`, `pnpm test`, `pnpm build:web`, `pnpm contracts:check`, `pnpm mcp:self-test`, `pnpm mcp:inspector-smoke`, and `pnpm test:pi` all pass.
- New unit tests cover package-dir resolution (project root, upward walk, missing) and Pi retry idempotency.

### Next focus

- Operator will verify the waiting behavior of `request_ramble_feedback` (long wait without failure) and visually accept the modal corner fix, nav alignment, and badge removal.
- Begin the shadcn-svelte migration per `docs/UI_SHADCN_MIGRATION.md`.
