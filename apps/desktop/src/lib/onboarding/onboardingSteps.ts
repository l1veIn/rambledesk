export type OnboardingStep = 'welcome' | 'storage' | 'voice' | 'permissions' | 'agents' | 'notifications' | 'cooking' | 'finish'
export type OnboardingOptions = { storage: boolean; voice: boolean; permissions: boolean; notifications: boolean }

export function onboardingSteps(options: OnboardingOptions): OnboardingStep[] {
  return ['welcome', ...(options.storage ? ['storage' as const] : []), ...(options.voice ? ['voice' as const] : []),
    ...(options.permissions ? ['permissions' as const] : []), 'agents', ...(options.notifications ? ['notifications' as const] : []), 'cooking', 'finish']
}
export function resumeOnboardingStep(steps: readonly OnboardingStep[], savedIndex: number): number {
  return Math.max(0, Math.min(steps.length - 1, Number.isFinite(savedIndex) ? Math.floor(savedIndex) : 0))
}
/** Preserve the visible step when an asynchronous permission query changes the list. */
export function reconcileOnboardingStep(previous: readonly OnboardingStep[], next: readonly OnboardingStep[], index: number): number {
  const remaining = previous.slice(resumeOnboardingStep(previous, index))
  for (const step of remaining) {
    const found = next.indexOf(step)
    if (found >= 0) return found
  }
  return resumeOnboardingStep(next, index)
}
export function onboardingRestartStep(steps: readonly OnboardingStep[], reason: 'storage' | 'permissions'): number {
  const current = steps.indexOf(reason)
  return resumeOnboardingStep(steps, current < 0 ? 0 : current + (reason === 'storage' ? 1 : 0))
}
