export const MIN_WINDOW_ZOOM = 0.8
export const MAX_WINDOW_ZOOM = 3

export function validateWindowZoom(factor: number): void {
  if (!Number.isFinite(factor) || factor < MIN_WINDOW_ZOOM || factor > MAX_WINDOW_ZOOM) {
    throw new RangeError(`Window zoom must be between ${MIN_WINDOW_ZOOM} and ${MAX_WINDOW_ZOOM}.`)
  }
}
