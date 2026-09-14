# RambleDesk marketing site

Astro renders English `/` and Simplified Chinese `/zh/`. Svelte hydrates only the interactive product mock. This site is separate from the desktop application's browser client.

```sh
pnpm install --frozen-lockfile
pnpm -C web dev --host 127.0.0.1 --port 4322
pnpm -C web check
pnpm -C web build
pnpm -C web preview --host 127.0.0.1 --port 4322
```

`pnpm dev:web` at the repository root starts the desktop application's web client, not this site.

- `src/content/site.ts` owns bilingual page copy, release downloads and documentation links. Keep download version and documentation tag aligned; verify both platform assets when updating.
- `src/content/feedback-example.ts` owns the single example used by the request card.
- `src/content/product-mock.ts` owns the scripted projects, sessions, requests, task briefs, documents, package files and both locales' labels for the interactive product mock. It is demonstration data: no transport, no agent, no network.
- `LandingPage.astro` owns the page structure, navigation and progressive motion. The mock is split by surface: `ProductMock.svelte` (state, window chrome, project/session rail, request rail), `MockWorkspace.svelte` (workspace header, task brief, feedback document, Agent and settings views), `MockCommandRail.svelte` (Ramble, add context, attachments, feedback package, Rambelle status) and `MockCaptureThumb.svelte` (the illustrative capture). The page's main narrative is rendered on the server; the mock's first view renders without hydration too.
- The mock mirrors the desktop workbench (`apps/desktop/src/lib/{shell,components/navigation,workspace,workbench}`): titlebar with workspace tabs, project → session rail (240px, 56px collapsed), request rail (240px), task brief above the feedback document, command rail (288px), and the same labels, status words and delivery semantics. Rail widths and the window's 1320 × 840 aspect ratio come from the desktop configuration (`tauri.conf.json` minimum size, `railResize.ts` default widths). Sizes collapse on the mock's own width through a container query (1100px), so the rails stay visible while the page is wide enough.
- `public/assets/refresh/` contains text-free production art, the geometric brand mark, and four byte-identical copies of desktop assets (`rambledesk-app-icon.webp`, `rambelle-{idle,recording,archived}.webp`). The hero selects an independently composed mobile image below 960px. Titles, requests, controls and feedback are live HTML.

The product mock is explicitly an interactive model. It is labelled 「交互示例 · 演示数据」 / "Interactive example · demonstration data" above the window and again below it. A visitor can expand a project, switch sessions and requests, filter requests by status, search sessions, collapse either rail, collapse the task brief, add a brief action as an `@Action` group, start and stop a scripted recording that writes a pending speech segment, tidy that segment, attach an illustrative capture, edit document text in place, submit the feedback, watch the delivery state move from waiting to delivered, cancel a request, open the published package, and restart the example. Every one of those results is scripted. Keep the labelling, keep the results consistent with the product's real semantics (delivered means the continuation message reached the session, not that the task is done), and never present the mock as a recording. The linked v0.3.2 GIF stays the only real product animation; a new recording can replace the mock when one exists.

Design decisions and production art provenance live in [docs/design/README.md](../docs/design/README.md).

## Deployment

[Web Deploy](../.github/workflows/web-deploy.yml) checks and builds this package, then publishes `web/dist` to the Cloudflare Pages project `rambledesk-web`. It runs on relevant changes pushed to `main` (`web/`, the workflow and workspace/package manifests), or a manual workflow dispatch. CI uses `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` repository secrets.

The production site is [www.rambledesk.com](https://www.rambledesk.com/). Before publishing, verify the intended commit, checks, release links and both locales. Use the `preview` command above for a local production preview; publishing is a separate workflow action. Domain and credential status should be inspected when needed, rather than recorded as a permanent pending/expired setup state here.
