import type { WebReviewData, WorkbenchSpec } from '../generated/feedback'

/** Shared by the isolated workbench gallery and the real App preview transport. */
export function webReviewPreviewSpec(origin?: string): WorkbenchSpec {
  const pageOrigin = origin ?? (typeof window !== 'undefined' && window.location.origin !== 'null'
    ? window.location.origin : 'http://127.0.0.1:5173')
  const data: WebReviewData = {
    title: 'Atelier 首页', source_version: 'homepage-v1',
    url: new URL('/web-review-fixture.html', pageOrigin).href,
    viewport: { width: 1280, height: 800 },
  }
  return { type: 'web_review', version: 1, data }
}
