/* RambleDesk web review bridge v1. Copy this file into the reviewed project's public assets.
 * Loading it alone does nothing: a direct parent frame must start a review session.
 * No network requests, form values or application credentials are collected.
 */
(() => {
  'use strict'
  if (window.__rambledeskWebReviewBridge || window.parent === window) return
  window.__rambledeskWebReviewBridge = true
  const protocol = 'rambledesk.web-review.v1'
  let session = null
  let mode = 'browse'
  let markers = []
  let selectedId = null
  let selected = null
  let hovered = null
  let layer = null
  let shadow = null
  let hoverBox = null
  let selectedBox = null
  let markerLayer = null
  let scheduled = false
  let lastUrl = location.href
  const limited = (text, maximum) => Array.from(text).slice(0, maximum).join('')

  const rect = (element) => {
    const box = element.getBoundingClientRect()
    return { x: Math.round(box.x), y: Math.round(box.y), width: Math.max(0, Math.round(box.width)), height: Math.max(0, Math.round(box.height)) }
  }
  const viewport = () => ({ width: Math.round(innerWidth), height: Math.round(innerHeight), scroll_x: Math.round(scrollX), scroll_y: Math.round(scrollY) })
  const page = () => ({ page_url: location.href, page_title: limited(document.title, 1000), viewport: viewport() })
  function send(type, payload) {
    if (session) parent.postMessage({ protocol, channel: session.channel, type, ...payload }, session.origin === 'null' ? '*' : session.origin)
  }
  function selector(element) {
    if (element.id && document.querySelectorAll(`#${CSS.escape(element.id)}`).length === 1) return `#${CSS.escape(element.id)}`
    const path = []
    let node = element
    while (node && node !== document.documentElement) {
      let part = node.localName
      const testId = node.getAttribute('data-testid')
      if (testId) {
        const candidate = `[data-testid="${CSS.escape(testId)}"]`
        if (document.querySelectorAll(candidate).length === 1) { path.unshift(candidate); break }
      }
      if (node.id && document.querySelectorAll(`#${CSS.escape(node.id)}`).length === 1) { path.unshift(`#${CSS.escape(node.id)}`); break }
      const siblings = node.parentElement ? Array.from(node.parentElement.children).filter((item) => item.localName === node.localName) : []
      if (siblings.length > 1) part += `:nth-of-type(${siblings.indexOf(node) + 1})`
      path.unshift(part)
      node = node.parentElement
    }
    return path.length ? path.join(' > ') : 'html'
  }
  function find(value) {
    if (typeof value !== 'string' || value.length > 2000) return null
    try { return document.querySelector(value) } catch { return null }
  }
  function usable(element) {
    return element instanceof Element && element !== layer && !['script', 'style', 'meta', 'link', 'head'].includes(element.localName)
  }
  function eventElement(event) {
    // Retargeted shadow DOM events select the host, whose selector can be resolved
    // later by document.querySelector; selectors never pretend to traverse a shadow root.
    return usable(event.target) ? event.target : null
  }
  function selection(element) {
    const attributes = {}
    for (const name of ['id', 'role', 'aria-label', 'data-testid', 'name', 'type', 'href', 'src']) {
      const value = element.getAttribute(name)
      if (value) attributes[name] = limited(value, 1000)
    }
    // Text content is visible context; input.value and textarea contents can hold secrets.
    const text = ['input', 'textarea', 'select'].includes(element.localName) ? (element.getAttribute('aria-label') || '') : (element.innerText || element.textContent || '')
    const bounds = rect(element)
    return { ...page(), selector: selector(element), tag_name: element.localName.toLowerCase(),
      text: limited(text.replace(/\s+/g, ' ').trim(), 2000), attributes,
      rect: { ...bounds, x: Math.round(bounds.x + scrollX), y: Math.round(bounds.y + scrollY) }, captured_at: new Date().toISOString() }
  }
  function ensureLayer() {
    if (layer?.isConnected) return
    layer = document.createElement('div')
    layer.setAttribute('data-rambledesk-web-review-overlay', '')
    layer.style.cssText = 'all:initial;position:fixed;inset:0;z-index:2147483647;pointer-events:none;contain:strict;'
    shadow = layer.attachShadow({ mode: 'closed' })
    const style = document.createElement('style')
    style.textContent = '.box{position:absolute;box-sizing:border-box;border:2px solid #6366f1;border-radius:3px;pointer-events:none}.hover{background:rgba(99,102,241,.08)}.selected{border-color:#f59e0b}.marker{position:absolute;display:grid;place-items:center;width:24px;height:24px;border:2px solid white;border-radius:999px;background:#6366f1;color:white;font:700 12px system-ui;box-shadow:0 1px 5px #0005;cursor:pointer;pointer-events:auto}.marker[aria-pressed=true]{background:#d97706}'
    hoverBox = document.createElement('div')
    hoverBox.className = 'box hover'
    selectedBox = document.createElement('div')
    selectedBox.className = 'box selected'
    markerLayer = document.createElement('div')
    shadow.append(style, hoverBox, selectedBox, markerLayer)
    document.documentElement.append(layer)
  }
  function positionBox(box, element) {
    if (!element?.isConnected) { box.style.display = 'none'; return }
    const bounds = rect(element)
    box.style.cssText = `left:${bounds.x}px;top:${bounds.y}px;width:${bounds.width}px;height:${bounds.height}px;`
  }
  function render() {
    scheduled = false
    if (!session) return
    ensureLayer()
    positionBox(hoverBox, mode === 'select' ? hovered : null)
    positionBox(selectedBox, selected)
    markerLayer.replaceChildren()
    for (const marker of markers) {
      if (marker.page_url !== location.href) continue
      const element = find(marker.selector)
      if (!element) continue
      const bounds = rect(element)
      if (bounds.x + bounds.width < 0 || bounds.y + bounds.height < 0 || bounds.x > innerWidth || bounds.y > innerHeight) continue
      const button = document.createElement('button')
      button.type = 'button'
      button.className = 'marker'
      button.textContent = String(marker.number)
      button.setAttribute('aria-label', `Review comment ${marker.number}`)
      button.setAttribute('aria-pressed', String(marker.id === selectedId))
      button.style.cssText = `left:${Math.min(innerWidth - 26, Math.max(2, bounds.x - 12))}px;top:${Math.min(innerHeight - 26, Math.max(2, bounds.y - 12))}px;`
      button.addEventListener('click', (event) => { event.stopPropagation(); selected = element; selectedId = marker.id; send('annotation_click', { id: marker.id }); update() })
      markerLayer.append(button)
    }
    if (selected && !selected.isConnected) selected = null
    send('anchor', { rect: selected ? rect(selected) : null, ...page() })
    if (lastUrl !== location.href) { lastUrl = location.href; send('ready', page()) }
  }
  function update() {
    if (!scheduled && session) { scheduled = true; requestAnimationFrame(render) }
  }
  function intercept(event) {
    if (!session || mode !== 'select' || event.composedPath().includes(layer)) return
    event.preventDefault()
    event.stopImmediatePropagation()
    if (event.type === 'click') {
      const element = eventElement(event)
      if (!element) return
      selected = element
      hovered = null
      selectedId = null
      send('selection', { selection: selection(element) })
      update()
    }
  }
  window.addEventListener('message', (event) => {
    const data = event.data
    if (event.source !== parent || !data || data.protocol !== protocol || typeof data.channel !== 'string' || data.channel.length > 100) return
    if (data.type === 'init') {
      if (session && event.origin !== session.origin) return
      session = { origin: event.origin, channel: data.channel }
      mode = data.mode === 'select' ? 'select' : 'browse'
      markers = []
      selected = null
      selectedId = null
      send('ready', page())
      update()
      return
    }
    if (!session || event.origin !== session.origin || data.channel !== session.channel) return
    if (data.type === 'mode') { mode = data.mode === 'select' ? 'select' : 'browse'; hovered = null }
    else if (data.type === 'markers' && Array.isArray(data.markers)) {
      markers = data.markers.slice(0, 500).filter((item) => item && typeof item.id === 'string' && typeof item.selector === 'string' && typeof item.page_url === 'string' && Number.isInteger(item.number))
      selectedId = typeof data.selected_id === 'string' ? data.selected_id : null
    } else if (data.type === 'focus') {
      if (data.page_url !== location.href) { send('focus_missing', { reason: 'page_changed', ...page() }); return }
      const element = find(data.selector)
      if (!element) { send('focus_missing', { reason: 'element_missing', ...page() }); return }
      selected = element
      element.scrollIntoView({ block: 'center', inline: 'nearest', behavior: 'instant' })
    } else if (data.type === 'dispose') {
      session = null
      hovered = null
      selected = null
      markers = []
      layer?.remove()
      return
    }
    update()
  })
  window.addEventListener('pointermove', (event) => {
    if (!session || mode !== 'select' || event.composedPath().includes(layer)) return
    hovered = eventElement(event)
    update()
  }, true)
  window.addEventListener('pointerleave', () => { hovered = null; update() }, true)
  for (const type of ['pointerdown', 'mousedown', 'mouseup', 'click', 'dblclick', 'contextmenu']) window.addEventListener(type, intercept, true)
  window.addEventListener('keydown', (event) => {
    if (session && mode === 'select' && event.key === 'Escape') { selected = null; hovered = null; send('anchor', { rect: null, ...page() }); update() }
  }, true)
  window.addEventListener('scroll', update, true)
  window.addEventListener('resize', update)
  window.addEventListener('hashchange', update)
  window.addEventListener('popstate', update)
  // Only active sessions observe layout and route changes. A small timer also covers SPA pushState.
  setInterval(() => { if (session && (selected || markers.length || lastUrl !== location.href)) update() }, 250)
})()
