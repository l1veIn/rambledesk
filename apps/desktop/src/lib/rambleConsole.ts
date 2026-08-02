export const RAMBLE_CONSOLE_LABEL = 'ramble-console'
export const RAMBLE_CONSOLE_COMMAND_EVENT = 'ramble-console-command'
export const RAMBLE_CONSOLE_READY_EVENT = 'ramble-console-ready'
export const RAMBLE_CONSOLE_STATE_EVENT = 'ramble-console-state'
export const RAMBLE_CONSOLE_SHOW_EVENT = 'ramble-console-show'
export const RAMBLE_CONSOLE_HIDE_EVENT = 'ramble-console-hide'

export type RambleConsolePhase =
  | 'starting'
  | 'recording'
  | 'paused'
  | 'stopping'
  | 'error'

export type RambleConsoleState = {
  phase: RambleConsolePhase
  sourceLabel: string
  requestTitle: string
  recording: boolean
  busy: boolean
  captureBusy: boolean
  voiceLevel: number
  partialTranscript: string
  message: string
}

export type RambleConsoleCommand =
  | { type: 'toggle-recording' }
  | { type: 'capture-screen' }
  | { type: 'import-clipboard' }
  | { type: 'import-files'; paths: string[] }
  | { type: 'exit' }
