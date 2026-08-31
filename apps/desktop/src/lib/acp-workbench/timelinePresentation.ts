/**
 * A settled Turn stays expanded until a later Turn exists. This mirrors the
 * user's reading rhythm: finishing must not make the content jump closed.
 */
export function timelineTurnStartsOpen(index: number, turnCount: number): boolean {
  return turnCount > 0 && index === turnCount - 1
}
