# RambleDesk marketing site

Astro renders English `/` and Simplified Chinese `/zh/`. Svelte hydrates only the manual feedback walkthrough. This site is separate from the desktop application's browser client.

```sh
pnpm install --frozen-lockfile
pnpm -C web dev --host 127.0.0.1 --port 4322
pnpm -C web check
pnpm -C web build
pnpm -C web preview --host 127.0.0.1 --port 4322
```

`pnpm dev:web` at the repository root starts the desktop application's web client, not this site.

- `src/content/site.ts` owns bilingual page copy, release downloads and documentation links. Keep download version and documentation tag aligned; verify both platform assets when updating.
- `src/content/feedback-example.ts` owns the single example used by the request card and walkthrough.
- `LandingPage.astro` owns the page structure, navigation and progressive motion. `FeedbackWalkthrough.svelte` owns only user-selected steps and example details. The page's main narrative is rendered on the server.
- `public/assets/refresh/` contains text-free production art and the geometric brand mark. The hero selects an independently composed mobile image below 960px. Titles, requests, controls and feedback are live HTML.

The walkthrough is explicitly an illustration. The linked v0.3.2 GIF is an earlier workflow demo. A new product recording can replace the walkthrough when available: keep user-controlled playback, captions, the visible feedback summary, and accurate delivery states. Never label the illustration as a recording.

Design decisions and production art provenance live in [docs/design/README.md](../docs/design/README.md).

## Deployment

[Web Deploy](../.github/workflows/web-deploy.yml) checks and builds this package, then publishes `web/dist` to the Cloudflare Pages project `rambledesk-web`. It runs on relevant changes pushed to `main` (`web/`, the workflow and workspace/package manifests), or a manual workflow dispatch. CI uses `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` repository secrets.

The production site is [www.rambledesk.com](https://www.rambledesk.com/). Before publishing, verify the intended commit, checks, release links and both locales. Use the `preview` command above for a local production preview; publishing is a separate workflow action. Domain and credential status should be inspected when needed, rather than recorded as a permanent pending/expired setup state here.
