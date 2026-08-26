export type ActiveRambleCoordinator = {
  ownerRequestId(): string
  occupy(requestId: string): void
  release(): void
  isOwner(requestId: string): boolean
  needsHandoff(visibleRequestId: string): boolean
}

export function createActiveRambleCoordinator(): ActiveRambleCoordinator {
  let ownerRequestId = ''

  return {
    ownerRequestId: () => ownerRequestId,
    occupy(requestId: string) {
      ownerRequestId = requestId
    },
    release() {
      ownerRequestId = ''
    },
    isOwner(requestId: string) {
      return ownerRequestId !== '' && ownerRequestId === requestId
    },
    needsHandoff(visibleRequestId: string) {
      return ownerRequestId !== '' && visibleRequestId !== '' && visibleRequestId !== ownerRequestId
    },
  }
}
