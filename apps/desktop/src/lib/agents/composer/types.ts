export type ComposerReference = Readonly<{
  id: string
  label: string
  /** An actual file URI supplied by the host's file search capability. */
  uri: string
}>

export type ReferenceSearch = (
  query: string,
  context: Readonly<{ signal: AbortSignal }>,
) => Promise<readonly ComposerReference[]>

/** Attachment data and upload/serialization remain owned by the host. */
export type ComposerAttachment = Readonly<{ id: string; name: string; detail?: string }>
export type ComposerPromptBlock = Readonly<{ type: 'text'; text: string }>
export type ComposerSubmitShortcut = 'enter' | 'mod+enter'
