export type SingleFlight = {
  run: (operation: () => Promise<void>) => Promise<void>
}

/** Collapse concurrent semantic commands into the operation already in flight. */
export function createSingleFlight(): SingleFlight {
  let current: Promise<void> | null = null
  let activeToken: object | null = null

  return {
    run(operation) {
      if (current) return current
      const token = {}
      activeToken = token
      current = Promise.resolve()
        .then(operation)
        .finally(() => {
          if (activeToken === token) {
            current = null
            activeToken = null
          }
        })
      return current
    },
  }
}
