/** Storage notifications are observations, never new preference edits. */
export function createPreferencePersistence(storage: Storage) {
  let receiving = false
  return {
    get receiving() { return receiving },
    write(key: string, value: string | null) {
      if (receiving || storage.getItem(key) === value) return
      if (value === null) storage.removeItem(key)
      else storage.setItem(key, value)
    },
    receive(event: StorageEvent, apply: () => void) {
      if (event.storageArea && event.storageArea !== storage) return
      // Another keystroke may already have replaced this queued event's value.
      // Applying it would roll back the form even if we suppressed its echo.
      if (event.key === null || storage.getItem(event.key) !== event.newValue) return
      receiving = true
      try { apply() } finally { receiving = false }
    },
  }
}
