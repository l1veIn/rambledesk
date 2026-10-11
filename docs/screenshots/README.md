# README demos and screenshots

## Animated workbench examples

The English and Simplified Chinese READMEs use four short GIFs each, in matching language directories. The order is document review, image annotation, brainstorming, and free feedback. Each is displayed at the full available width so the workbench remains readable.

Captured on **2026-10-12 (Asia/Shanghai)** from source HEAD `4c78bb4daa06fed1a565bb61fd0e0de11eb02f26` with uncommitted Markdown review and brainstorming changes. The captures show the real App UI with synthetic request content in English or Simplified Chinese. They do not imply that the released 0.5.0 package contains this development interface.

The controls were operated through the UI. No feedback was submitted to a live Agent. Brainstorming starts from a synthetic round-two board, then demonstrates editing, grouping, selecting cards, and choosing combine; its starting state is not evidence of a recorded Agent round trip. Draft indicators in these in-memory previews do not establish backend persistence, and browser capture does not verify native microphone or screen-capture permissions. This documentation refresh did not run runtime feature tests.

Document review and brainstorming use registered full-App preview examples: `?preview=fixtures&workspace=document_review-en` / `document_review`, and `?preview=fixtures&workspace=brainstorm-en` / `brainstorm`. Image annotation and free feedback use the existing disposable `localized-preview.html` harness with localized request content. The source viewports are **1728 × 1440** for document review and brainstorming and **1728 × 1080** for image annotation and free feedback.

Each GIF is a **six-second infinite loop** (`loop=0`) assembled from real, sequential UI screenshots with deliberate holds between steps, rather than continuous video. There are no interpolated frames or fabricated controls. Encoding uses **FFmpeg 7.0.2**, Lanczos scaling to 1600 pixels wide with the aspect ratio preserved, a global 128-color palette (`stats_mode=full`, `reserve_transparent=1`), no dithering, `paletteuse=diff_mode=rectangle`, `gifflags=+offsetting+transdiff`, and variable frame timing at a 100 Hz time base. There is no cropping, overlaid text, or retouching. Original capture softness remains; the GIFs illustrate operations rather than guaranteeing readability of every small text detail.

| Asset | Dimensions | Frames | Bytes |
| --- | --- | --- | ---: |
| [en/document-review.gif](en/document-review.gif) | 1600 × 1333 | 7 | 828,954 |
| [en/visual-feedback.gif](en/visual-feedback.gif) | 1600 × 1000 | 7 | 428,040 |
| [en/brainstorm.gif](en/brainstorm.gif) | 1600 × 1333 | 10 | 2,118,514 |
| [en/free-feedback.gif](en/free-feedback.gif) | 1600 × 1000 | 6 | 368,404 |
| [zh-CN/document-review.gif](zh-CN/document-review.gif) | 1600 × 1333 | 7 | 757,217 |
| [zh-CN/visual-feedback.gif](zh-CN/visual-feedback.gif) | 1600 × 1000 | 7 | 289,988 |
| [zh-CN/brainstorm.gif](zh-CN/brainstorm.gif) | 1600 × 1333 | 10 | 1,833,443 |
| [zh-CN/free-feedback.gif](zh-CN/free-feedback.gif) | 1600 × 1000 | 6 | 342,999 |

All eight total **6,967,559 bytes (6.65 MiB)**. Both languages use the same per-step timing, in milliseconds:

- Document review: `1000, 500, 500, 500, 600, 900, 2000`.
- Image annotation: `1100, 350, 600, 650, 500, 800, 2000`.
- Brainstorming: `900, 300, 400, 300, 500, 550, 650, 650, 500, 1250`.
- Free feedback: `1000, 600, 650, 700, 1050, 2000`.

<details>
<summary>Final GIF SHA-256 hashes</summary>

```text
f1c854dceb61c5dc825cc73b5ef11e35dae6a098776322a7db694b239367879f  en/document-review.gif
f9e31c7e3b714bb9c13eb65017d9e57413ba2b3a9311e244002468957946c413  en/visual-feedback.gif
58e4f6dc4f662c22760fba0562fd9eb6ef0446d193982e14188d4f4413208768  en/brainstorm.gif
6e22fa2e09eaa0a56cb6d1b862fbc4dc9a7241c3177d09b9d8335d49fcc48ed5  en/free-feedback.gif
fdc910db7e5d8217ac44aeb4b5eb29de484ccbdffb2cfd03f786f27593e896df  zh-CN/document-review.gif
ead59d7ff6353bedb4012b37d7868c531e8ce64b03145f3a98e5a9b6b49df4a0  zh-CN/visual-feedback.gif
9d4ef030abc225a03b2105748c696c9cc208337564d7a12b8138ebe4b2bf6c80  zh-CN/brainstorm.gif
05d6c0554fdef4d0d4bc7c22f383e9f5bfa40f18cbb26c58d924e39a36931fb4  zh-CN/free-feedback.gif
```

</details>

All final frames were decoded; the copied GIFs match their selected encoder outputs, and the source-frame hashes remained unchanged. Both README locale references and all eight image responses were checked locally, including response-byte hashes. The rendered English and Chinese pages were also inspected in a browser with all four images loaded and no clipping. These checks concern the documentation assets only.

The ignored `.local-artifacts/readme-demos-20261012/` directory retains `asset-manifest.json`, per-asset encoding audits and commands, `readme-preview-validation.json`, `readme-preview-http-validation.json`, and browser preview captures. Original JPEG frames and their `timings.json` files are under `<locale>/<workbench>/`. The manifest's per-file capture and encoding times are filesystem modification timestamps, not an independent recording clock. It was generated at `2026-10-11T16:56:00Z`.

The existing WebP screenshots below are retained as static alternatives and historical capture evidence. Raw recordings, intermediate frames, and disposable capture harnesses belong in ignored local artifacts.

## Previous static captures

The previous README presentation used three screenshots per language. Document review shows the development interface at its capture date; the other examples show 0.5.0:

- Image annotation: captured on 2026-10-08 from `main` commit `dd5a2b161ddf31369b789b7964c9f450c8331c5a`.
- Free feedback: captured on 2026-10-09 from `main` commit `862163f01cfd1172f47caefcd779b417b1234fd7`.
- Markdown document review: captured on 2026-10-11 from `main` commit `4c78bb4daa06fed1a565bb61fd0e0de11eb02f26` plus the uncommitted Markdown review interaction changes. These are not screenshots of a released installation package.

These images show the real App UI in the development browser preview. The application locale is set to English or Simplified Chinese, and a disposable preview harness supplies localized, in-memory demo requests and example content. Feedback is entered through the actual controls. No production source changes are required for these captures, and no text, controls, or status indicators are replaced in the captured images.

The screenshots use demo data. The examples remain drafts and are not submitted to a live Agent. This preview does not verify native microphone or screen-capture permissions, backend persistence, or feedback delivery.

| Image | Locale | Scenario and view |
| --- | --- | --- |
| [en/document-review.webp](en/document-review.webp) | English | `document_review-en`: onboarding plan in the normal Markdown workbench; selected text, an open comment beside the original, another suggestion marker, and the overall-feedback drawer closed. |
| [en/visual-feedback.webp](en/visual-feedback.webp) | English | `visual_feedback-image`: expanded canvas with the overall feedback panel open; region annotation and feedback. |
| [en/free-feedback.webp](en/free-feedback.webp) | English | `ramble`: normal request view; feedback on the third onboarding step, distinguishing saving a draft from submitting it. |
| [zh-CN/document-review.webp](zh-CN/document-review.webp) | Simplified Chinese | `document_review`: video script in the same normal Markdown workbench, with a selected passage and an open comment beside the original; the overall-feedback drawer is closed. |
| [zh-CN/visual-feedback.webp](zh-CN/visual-feedback.webp) | Simplified Chinese | `visual_feedback-image`: the same expanded canvas and feedback-panel view. |
| [zh-CN/free-feedback.webp](zh-CN/free-feedback.webp) | Simplified Chinese | `ramble`: the same normal request view and feedback on the third onboarding step. |

### Static capture setup

All six images use the light theme at a **1280 × 720** browser viewport. Normal and expanded views are selected using the application's existing controls; neither view is an image crop.

The base development preview can be started from the repository root:

```bash
pnpm install --frozen-lockfile
pnpm -C apps/desktop exec vite --host 127.0.0.1 --port 1431
```

Open `http://127.0.0.1:1431/?preview=fixtures&workspace=<scenario>` using a scenario from the table. The Markdown captures use the registered bilingual examples in the full App browser preview, viewed in the in-app browser; text selections and comments are created through the actual controls. The older visual-feedback and free-feedback captures use a disposable harness to localize the demo requests. Dismiss the first-use tour, collapse the project and request rails, select the indicated view, and enter the example feedback through the UI. The preview's draft indicator reflects its in-memory state, not a backend save.

Raw captures are encoded as WebP using **quality 76, method 6**, with no resizing or retouching. The six language-specific WebP files are the tracked screenshot assets. The disposable harness, raw captures, intermediate files, and local README renders remain in ignored local artifacts.

### Markdown static capture provenance

The 2026-10-11 document-review captures were saved as JPEG and encoded to WebP using the settings above. Their source material intentionally contains wording to review; it is demo content, not product claims. Both show the normal workbench with its project and request rails collapsed.

- English: captured at `2026-10-11T15:30:38Z`; 46,052 bytes. SHA-256: `c28157f8c1578cd26978d730be46637cfb8f6d1d6b5174ea572bb6cd5971daf6`.
- Simplified Chinese: captured at `2026-10-11T15:36:49Z`; 47,850 bytes. SHA-256: `0f26be56d3035796106d06397480851500ba22b4afe6f70c10b1fcc006172342`.

Capture metadata and original JPEGs are kept in the ignored `.local-artifacts/markdown-review-interaction-20261011/` directory. The snapshots verify presentation only; they do not establish SQLite persistence, an external Agent round trip, or native WebView/device permissions.

For the preview architecture, see the [development preview guide](../../apps/desktop/src/dev/README.md). When refreshing these assets, update the source identity, locale, and encoding details here, together with the corresponding README references.
