# README screenshots

The English and Chinese READMEs each use three screenshots in their corresponding language directory, prepared for the 0.5.0 release:

- Image annotation: captured on 2026-10-08 from `main` commit `dd5a2b161ddf31369b789b7964c9f450c8331c5a`.
- Document review and free feedback: captured on 2026-10-09 from `main` commit `862163f01cfd1172f47caefcd779b417b1234fd7`.

These images show the real App UI in the development browser preview. The application locale is set to English or Simplified Chinese, and a disposable preview harness supplies localized, in-memory demo requests and example content. Feedback is entered through the actual controls. No production source changes are required for these captures, and no text, controls, or status indicators are replaced in the captured images.

The screenshots use demo data. The examples remain drafts and are not submitted to a live Agent. This preview does not verify native microphone or screen-capture permissions, backend persistence, or feedback delivery.

| Image | Locale | Scenario and view |
| --- | --- | --- |
| [en/document-review.webp](en/document-review.webp) | English | `document_review`: normal request view; first-paragraph comment on an onboarding plan, overall feedback, and “Changes requested” selected. |
| [en/visual-feedback.webp](en/visual-feedback.webp) | English | `visual_feedback-image`: expanded canvas with the overall feedback panel open; region annotation and feedback. |
| [en/free-feedback.webp](en/free-feedback.webp) | English | `ramble`: normal request view; feedback on the third onboarding step, distinguishing saving a draft from submitting it. |
| [zh-CN/document-review.webp](zh-CN/document-review.webp) | Simplified Chinese | `document_review`: the same normal request view, first-paragraph comment, overall feedback, and review decision. |
| [zh-CN/visual-feedback.webp](zh-CN/visual-feedback.webp) | Simplified Chinese | `visual_feedback-image`: the same expanded canvas and feedback-panel view. |
| [zh-CN/free-feedback.webp](zh-CN/free-feedback.webp) | Simplified Chinese | `ramble`: the same normal request view and feedback on the third onboarding step. |

## Capture setup

All six images use the light theme at a **1280 × 720** browser viewport. Normal and expanded views are selected using the application's existing controls; neither view is an image crop.

The base development preview can be started from the repository root:

```bash
pnpm install --frozen-lockfile
pnpm -C apps/desktop exec vite --host 127.0.0.1 --port 1431
```

Open `http://127.0.0.1:1431/?preview=fixtures&workspace=<scenario>` using a scenario from the table. The disposable capture harness localizes the demo request content as well as selecting the application locale; the stock fixtures alone do not reproduce the exact bilingual examples. Dismiss the first-use tour, collapse the project and request rails, select the indicated view, and enter the example feedback through the UI. The preview's draft indicator reflects its in-memory state, not a backend save.

Raw captures are encoded as WebP using **quality 76, method 6**, with no resizing or retouching. The six language-specific WebP files are the tracked screenshot assets. The disposable harness, raw captures, intermediate files, and local README renders remain in ignored local artifacts.

For the preview architecture, see the [development preview guide](../../apps/desktop/src/dev/README.md). When refreshing these images, update the source commit, locale and encoding details, and the version note in both READMEs together.
