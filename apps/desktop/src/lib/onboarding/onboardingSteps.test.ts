import { describe, expect, it } from 'vitest'
import { onboardingRestartStep, onboardingSteps, reconcileOnboardingStep, resumeOnboardingStep } from './onboardingSteps'

describe('onboarding feature steps and restart recovery', () => {
  it('retains the full supported desktop flow while replacing adapters with Agents', () => {
    const steps = onboardingSteps({ storage: true, voice: true, permissions: true, notifications: true })
    expect(steps).toEqual(['welcome', 'storage', 'voice', 'permissions', 'agents', 'notifications', 'cooking', 'finish'])
    expect(steps).not.toContain('adapters')
  })
  it('keeps Windows and Linux voice, notifications, and Cooking without macOS permissions', () => {
    expect(onboardingSteps({ storage: true, voice: true, permissions: false, notifications: true }))
      .toEqual(['welcome', 'storage', 'voice', 'agents', 'notifications', 'cooking', 'finish'])
  })
  it('omits only unavailable capabilities, leaving Agents and optional Cooking accessible', () => {
    expect(onboardingSteps({ storage: false, voice: false, permissions: false, notifications: false }))
      .toEqual(['welcome', 'agents', 'cooking', 'finish'])
    expect(onboardingSteps({ storage: false, voice: true, permissions: false, notifications: true }))
      .toEqual(['welcome', 'voice', 'agents', 'notifications', 'cooking', 'finish'])
  })
  it('resumes after storage at the next supported step and after permissions at the same step', () => {
    const desktop = onboardingSteps({ storage: true, voice: true, permissions: true, notifications: true })
    expect(desktop[onboardingRestartStep(desktop, 'storage')]).toBe('voice')
    expect(desktop[onboardingRestartStep(desktop, 'permissions')]).toBe('permissions')
    const noVoice = onboardingSteps({ storage: true, voice: false, permissions: false, notifications: false })
    expect(noVoice[onboardingRestartStep(noVoice, 'storage')]).toBe('agents')
  })
  it('keeps Agents selected when an empty macOS permission list removes an earlier step', () => {
    const before = onboardingSteps({ storage: true, voice: true, permissions: true, notifications: true })
    const after = onboardingSteps({ storage: true, voice: true, permissions: false, notifications: true })
    expect(after[reconcileOnboardingStep(before, after, before.indexOf('agents'))]).toBe('agents')
    expect(after[reconcileOnboardingStep(before, after, before.indexOf('permissions'))]).toBe('agents')
    expect(after[reconcileOnboardingStep(before, after, before.indexOf('notifications'))]).toBe('notifications')
  })
  it('clamps persisted progress without skipping the restored feature steps', () => {
    const steps = onboardingSteps({ storage: true, voice: true, permissions: true, notifications: true })
    expect(steps[resumeOnboardingStep(steps, 2)]).toBe('voice')
    expect(steps[resumeOnboardingStep(steps, -1)]).toBe('welcome')
    expect(steps[resumeOnboardingStep(steps, 99)]).toBe('finish')
    expect(steps[resumeOnboardingStep(steps, Number.NaN)]).toBe('welcome')
  })
})
