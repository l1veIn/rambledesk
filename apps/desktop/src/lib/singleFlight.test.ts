import { describe, expect, it, vi } from 'vitest'

import { createSingleFlight } from './singleFlight'

describe('createSingleFlight', () => {
  it('runs repeated commands once until the active transition finishes', async () => {
    let finish: (() => void) | undefined
    const operation = vi.fn(
      () => new Promise<void>((resolve) => {
        finish = resolve
      }),
    )
    const singleFlight = createSingleFlight()

    const first = singleFlight.run(operation)
    const repeated = singleFlight.run(operation)
    expect(repeated).toBe(first)
    await Promise.resolve()
    expect(operation).toHaveBeenCalledTimes(1)

    finish?.()
    await first
    await singleFlight.run(async () => {})
    expect(operation).toHaveBeenCalledTimes(1)
  })
})
