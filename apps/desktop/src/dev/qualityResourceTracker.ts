/** Installed before main imports. Keeps IDs, never strong references to Worker objects. */
export function trackClientResources() {
  const originalCreate = URL.createObjectURL.bind(URL)
  const originalRevoke = URL.revokeObjectURL.bind(URL)
  const activeUrls = new Set<string>()
  let urlsCreated = 0
  let urlsRevoked = 0
  URL.createObjectURL = (object) => {
    const url = originalCreate(object)
    urlsCreated += 1
    activeUrls.add(url)
    return url
  }
  URL.revokeObjectURL = (url) => {
    originalRevoke(url)
    if (activeUrls.delete(url)) urlsRevoked += 1
  }
  let workersCreated = 0
  let workersTerminated = 0
  const workerIds = new WeakMap<Worker, number>()
  const unterminatedWorkerIds = new Set<number>()
  const workerSupported = typeof Worker !== 'undefined'
  if (workerSupported) {
    const OriginalWorker = Worker
    const originalTerminate = OriginalWorker.prototype.terminate
    OriginalWorker.prototype.terminate = function () {
      originalTerminate.call(this)
      const id = workerIds.get(this)
      if (id !== undefined && unterminatedWorkerIds.delete(id)) workersTerminated += 1
    }
    window.Worker = new Proxy(OriginalWorker, {
      construct(target, args, newTarget) {
        const worker = Reflect.construct(target, args, newTarget) as Worker
        const id = ++workersCreated
        workerIds.set(worker, id)
        unterminatedWorkerIds.add(id)
        return worker
      },
    })
  }
  return {
    snapshot: () => ({
      objectUrls: { created: urlsCreated, revoked: urlsRevoked, outstanding: activeUrls.size },
      workers: { supported: workerSupported, created: workersCreated, explicitlyTerminated: workersTerminated, withoutObservedTerminate: unterminatedWorkerIds.size },
    }),
    // Harness downloads are outside measured App resources.
    exportJson(value: unknown, filename: string) {
      const url = originalCreate(new Blob([JSON.stringify(value, null, 2)], { type: 'application/json' }))
      const link = document.createElement('a')
      link.href = url
      link.download = filename
      link.click()
      setTimeout(() => originalRevoke(url), 1000)
    },
  }
}
export type ClientResourceTracker = ReturnType<typeof trackClientResources>
