# RambleDesk marketing site

Astro renders English `/` and Simplified Chinese `/zh/`. Svelte hydrates only the workbench demo gallery. This site is separate from the desktop application's browser client.

```sh
pnpm install --frozen-lockfile
pnpm -C web dev --host 127.0.0.1 --port 4322
pnpm -C web check
pnpm test:web
pnpm -C web build
pnpm -C web preview --host 127.0.0.1 --port 4322
```

`pnpm dev:web` at the repository root starts the desktop application's web client, not this site.

- `src/content/site.ts` owns bilingual copy, workbench descriptions, release downloads and documentation links. Installation guides follow the release tag; the current development overview and related work link to `main`. Verify both platform assets when updating the release version.
- `LandingPage.astro` owns page structure, navigation and progressive motion. Its main narrative renders without JavaScript.
- `WorkbenchDemos.svelte` presents four real UI recordings with corresponding English and Chinese content. It renders the first poster on the server, loads video only on an explicit play action, and supports keyboard tab navigation and native video controls. Switching workbenches stops playback. Keep the complete workbench visible; do not crop the recordings to fit a fixed-height frame.
- `public/assets/workbenches/` contains six-second MP4 clips and WebP posters. See its [media provenance](public/assets/workbenches/README.md) and the [README capture notes](../docs/screenshots/README.md). These are recordings of the local browser preview with demonstration materials, not live agent sessions. The gallery identifies development features that are not included in the stable download.
- `public/assets/refresh/` contains the existing text-free production art and geometric brand mark. The hero selects an independently composed mobile image below 960px. Titles, descriptions and controls are live HTML.

The earlier scripted `ProductMock.svelte` and its supporting components/data remain as historical examples with their tests; they are no longer imported by the landing page. Do not describe them as the current product interface.

Design decisions and production art provenance live in [docs/design/README.md](../docs/design/README.md).

## Deployment

[Web Deploy](../.github/workflows/web-deploy.yml) checks and builds this package, then publishes `web/dist` to the Cloudflare Pages project `rambledesk-web`. It runs on relevant changes pushed to `main` (`web/`, the workflow and workspace/package manifests), or a manual workflow dispatch. CI uses `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` repository secrets.

The production site is [www.rambledesk.com](https://www.rambledesk.com/). Before publishing, verify the intended commit, checks, release links and both locales. Use the `preview` command above for a local production preview; publishing is a separate workflow action. Domain and credential status should be inspected when needed, rather than recorded as a permanent pending/expired setup state here.
