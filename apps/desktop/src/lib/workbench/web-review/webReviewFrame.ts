import {
  WEB_REVIEW_PROTOCOL, reviewPageUrl, validReviewRect, validReviewSelection, validReviewViewport,
  type WebReviewFrameState, type WebReviewMarker, type WebReviewRect, type WebReviewSelection,
} from './webReviewProtocol'

type FrameOptions = Readonly<{
  iframe: HTMLIFrameElement
  url: string
  onState: (state: WebReviewFrameState) => void
  onSelection: (selection: WebReviewSelection) => void
  onAnchor?: (rect: WebReviewRect | null) => void
  onAnnotationClick?: (id: string) => void
  onFocusMissing?: (reason: 'page_changed' | 'element_missing') => void
  window?: Window
  timeoutMs?: number
}>

export function createWebReviewFrame(options: FrameOptions) {
  const host = options.window ?? window
  const url = reviewPageUrl(options.url)
  const origin = url ? new URL(url).origin : null
  let channel = host.crypto.randomUUID()
  let disposed = false
  let ready = false
  let mode: 'browse' | 'select' = 'browse'
  let markers: readonly WebReviewMarker[] = []
  let selectedId: string | null = null
  let pendingFocus: Readonly<{ selector: string; page_url: string }> | null = null
  let timer: ReturnType<typeof setTimeout> | undefined
  let retry: ReturnType<typeof setInterval> | undefined
  let state: WebReviewFrameState = { status: 'loading', page_url: url ?? options.url, page_title: '', viewport: null }

  function emit(next: WebReviewFrameState) { state = next; options.onState(next) }
  function post(type: string, payload: Record<string, unknown> = {}) {
    if (!disposed && origin) options.iframe.contentWindow?.postMessage({ protocol: WEB_REVIEW_PROTOCOL, channel, type, ...payload }, origin)
  }
  function clearTimers() {
    if (timer !== undefined) clearTimeout(timer)
    if (retry !== undefined) clearInterval(retry)
    timer = undefined
    retry = undefined
  }
  function unavailable(reason: WebReviewFrameState['reason']) {
    ready = false
    clearTimers()
    options.onAnchor?.(null)
    emit({ ...state, status: 'unavailable', reason })
  }
  function validPage(data: Record<string, unknown>) {
    const pageUrl = reviewPageUrl(data.page_url)
    return pageUrl !== null && new URL(pageUrl).origin === origin && typeof data.page_title === 'string'
      && [...data.page_title].length <= 1000 && validReviewViewport(data.viewport)
  }
  function onMessage(event: MessageEvent) {
    if (disposed || event.source !== options.iframe.contentWindow || event.origin !== origin) return
    const data = event.data as Record<string, unknown> | null
    if (!data || data.protocol !== WEB_REVIEW_PROTOCOL || data.channel !== channel) return
    if (data.type === 'ready' && validPage(data)) {
      ready = true
      clearTimers()
      emit({ status: 'ready', page_url: data.page_url as string, page_title: data.page_title as string,
        viewport: data.viewport as WebReviewFrameState['viewport'] })
      post('mode', { mode })
      post('markers', { markers, selected_id: selectedId })
      if (pendingFocus) { post('focus', pendingFocus); pendingFocus = null }
    } else if (ready && data.type === 'selection' && validReviewSelection(data.selection)) {
      const selection = data.selection
      if (new URL(selection.page_url).origin !== origin) return
      options.onSelection(selection)
      options.onAnchor?.({ ...selection.rect, x: selection.rect.x - selection.viewport.scroll_x, y: selection.rect.y - selection.viewport.scroll_y })
      emit({ status: 'ready', page_url: selection.page_url, page_title: selection.page_title, viewport: selection.viewport })
    } else if (ready && data.type === 'anchor' && (data.rect === null || validReviewRect(data.rect)) && validPage(data)) {
      options.onAnchor?.(data.rect)
      if (data.page_url !== state.page_url || data.page_title !== state.page_title) {
        emit({ status: 'ready', page_url: data.page_url as string, page_title: data.page_title as string,
          viewport: data.viewport as WebReviewFrameState['viewport'] })
      }
    } else if (ready && data.type === 'annotation_click' && typeof data.id === 'string' && markers.some(item => item.id === data.id)) {
      options.onAnnotationClick?.(data.id)
    } else if (ready && data.type === 'focus_missing' && (data.reason === 'page_changed' || data.reason === 'element_missing')) {
      options.onAnchor?.(null)
      options.onFocusMissing?.(data.reason)
    }
  }

  // Same-origin pages need no manual installation. Cross-origin pages must load
  // the bridge themselves: reading or rewriting their DOM would violate the browser boundary.
  function installSameOriginBridge() {
    try {
      // Do not inject into the initial about:blank document while a remote
      // navigation is still pending; only the configured same-origin page qualifies.
      if (options.iframe.contentWindow?.location.origin !== origin) return
      const child = options.iframe.contentDocument
      if (!child || !child.documentElement || child.querySelector('[data-rambledesk-web-review-script]')) return
      const script = child.createElement('script')
      script.src = new URL('/rambledesk-web-review.js', host.location.href).href
      script.setAttribute('data-rambledesk-web-review-script', '')
      script.addEventListener('load', () => post('init', { mode }), { once: true })
      child.head.append(script)
    } catch { /* Cross-origin access is expected; the page-installed bridge handles it. */ }
  }
  function connect() {
    if (disposed) return
    clearTimers()
    ready = false
    channel = host.crypto.randomUUID()
    options.onAnchor?.(null)
    if (!url) { unavailable('invalid_url'); return }
    emit({ ...state, status: 'loading', reason: undefined })
    installSameOriginBridge()
    post('init', { mode })
    // A deferred page script can load after iframe.onload.
    retry = setInterval(() => { if (!ready) post('init', { mode }) }, 400)
    timer = setTimeout(() => unavailable('bridge_missing'), options.timeoutMs ?? 6000)
  }
  function failed() { if (!disposed) unavailable('page_unavailable') }
  host.addEventListener('message', onMessage)
  options.iframe.addEventListener('load', connect)
  options.iframe.addEventListener('error', failed)

  return {
    connect,
    setMode(next: 'browse' | 'select') { mode = next; if (ready) post('mode', { mode }) },
    setAnnotations(next: readonly WebReviewMarker[], nextSelectedId: string | null = null) {
      markers = next
      selectedId = nextSelectedId
      if (ready) post('markers', { markers, selected_id: selectedId })
    },
    focus(target: Readonly<{ selector: string; page_url: string }>) {
      if (ready) post('focus', target)
      else pendingFocus = target
    },
    dispose() {
      if (disposed) return
      // Closing or detaching an iframe can invalidate its WindowProxy before
      // component teardown reaches us. The child notification is best effort;
      // parent timers and subscriptions must still be released.
      try { post('dispose') } catch { /* The reviewed frame has already closed. */ }
      disposed = true
      clearTimers()
      host.removeEventListener('message', onMessage)
      options.iframe.removeEventListener('load', connect)
      options.iframe.removeEventListener('error', failed)
    },
  }
}
