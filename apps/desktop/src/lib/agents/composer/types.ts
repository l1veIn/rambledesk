export type ComposerReference = Readonly<{
  id: string
  label: string
  /** An actual file URI supplied by the host's file search capability. */
  uri: string
}>

export type ComposerSubmitShortcut = 'enter' | 'mod+enter'
