import type { WindowCapability } from '../workbenchCapabilities'
import { validateWindowZoom } from '../windowZoom'

/** Main workbench fallback; callers supply its document root so portals scale too. */
export function createBrowserZoomCapability(
  root: Pick<HTMLElement, 'style'>,
): Pick<WindowCapability, 'setZoom'> {
  const original = { zoom: root.style.zoom, width: root.style.width, height: root.style.height }
  return {
    async setZoom(factor) {
      validateWindowZoom(factor)
      if (factor === 1) {
        Object.assign(root.style, original)
        return
      }
      root.style.zoom = String(factor)
      // CSS zoom scales viewport units too. Compensate the root once; body and
      // #app use 100% so their visual bounds remain the browser's viewport.
      root.style.width = `calc(100vw / ${factor})`
      root.style.height = `calc(100dvh / ${factor})`
    },
  }
}
