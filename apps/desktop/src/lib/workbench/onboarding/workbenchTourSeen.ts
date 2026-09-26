import type { WorkbenchTourId } from './workbenchTours'

type TourStorage = Pick<Storage, 'getItem' | 'setItem'>
const key = (id: WorkbenchTourId) => `rambledesk.workbench-tour.${id}`

/**
 * Completion belongs to a workbench type, never a request. Memory also records
 * explicit completion/skip so unavailable browser storage cannot repeat a tour
 * every time the user switches requests during the same session.
 */
export function createWorkbenchTourSeenStore(storage: () => TourStorage | null = () => globalThis.localStorage) {
  const seenVersions = new Map<WorkbenchTourId, number>()

  function hasSeen(id: WorkbenchTourId, version: number): boolean {
    if ((seenVersions.get(id) ?? 0) >= version) return true
    try {
      const saved: unknown = JSON.parse(storage()?.getItem(key(id)) ?? 'null')
      if (!saved || typeof saved !== 'object' || !('version' in saved)) return false
      const seen = saved.version
      return typeof seen === 'number' && Number.isSafeInteger(seen) && seen >= version
    } catch {
      return false
    }
  }

  function markSeen(id: WorkbenchTourId, version: number): void {
    seenVersions.set(id, Math.max(seenVersions.get(id) ?? 0, version))
    try {
      // A separate key preserves every other workbench's completion state.
      storage()?.setItem(key(id), JSON.stringify({ version }))
    } catch {
      // Session memory remains authoritative when storage is blocked or full.
    }
  }

  return { hasSeen, markSeen }
}

const seen = createWorkbenchTourSeenStore()
export const hasSeenWorkbenchTour = seen.hasSeen
export const markWorkbenchTourSeen = seen.markSeen
