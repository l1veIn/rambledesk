import type { WorkbenchExample } from '../contracts'
import type { WebReviewData, WorkbenchSpec } from '../../../generated/feedback'
export function webReviewPreviewSpec(origin?: string): WorkbenchSpec {
  const pageOrigin = origin ?? (typeof window !== 'undefined' && window.location.origin !== 'null' ? window.location.origin : 'http://127.0.0.1:5173')
  const data: WebReviewData = {title:'Atelier 首页',source_version:'homepage-v1',url:new URL('/web-review-fixture.html',pageOrigin).href,viewport:{width:1280,height:800}}
  return {type:'web_review',version:1,data}
}
export const examples: readonly WorkbenchExample[] = [{ order: 4, title: '网页评审', markdown: '选择网页元素留下批注，或在右侧填写整体反馈。', spec:webReviewPreviewSpec(),createSpec:(options) => webReviewPreviewSpec(options.origin) }]
