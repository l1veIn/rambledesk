export type TitlebarPointerIntent = 'ignore' | 'drag' | 'toggle-maximize'

export function titlebarPointerIntent(input: Readonly<{
  button: number
  clickCount: number
  interactive: boolean
}>): TitlebarPointerIntent {
  if (input.button !== 0 || input.interactive) return 'ignore'
  return input.clickCount === 2 ? 'toggle-maximize' : 'drag'
}
