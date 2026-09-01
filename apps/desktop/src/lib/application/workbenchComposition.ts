import type { ApplicationTransport } from './applicationTransport'
import {
  HttpApplicationTransport,
  type HttpApplicationSession,
} from './httpApplicationTransport'
import { UnavailableApplicationTransport } from './unavailableApplicationTransport'

export type WorkbenchCompositionInput = Readonly<{
  environment: 'desktop' | 'browser'
  previewMode: boolean
  desktopTransport?: ApplicationTransport
  authenticatedWebSession?: HttpApplicationSession
}>

export type WorkbenchComposition = Readonly<{
  applicationTransport: ApplicationTransport
  previewMode: boolean
}>

/** Selects implementations only; credentials and native bindings are composed outside. */
export function createWorkbenchComposition(
  input: WorkbenchCompositionInput,
): WorkbenchComposition {
  if (input.previewMode) {
    return {
      applicationTransport: new UnavailableApplicationTransport(),
      previewMode: true,
    }
  }
  if (input.environment === 'desktop') {
    if (!input.desktopTransport) {
      throw new Error('Desktop composition requires a Tauri ApplicationTransport implementation.')
    }
    return { applicationTransport: input.desktopTransport, previewMode: false }
  }
  if (input.authenticatedWebSession) {
    return {
      applicationTransport: new HttpApplicationTransport(input.authenticatedWebSession.lease()),
      previewMode: false,
    }
  }
  return {
    applicationTransport: new UnavailableApplicationTransport(),
    previewMode: false,
  }
}
