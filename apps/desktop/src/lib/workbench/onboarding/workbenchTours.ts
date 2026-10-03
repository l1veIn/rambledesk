import type { WorkbenchDefinition } from '../definitions/contracts'
import type { Locale } from '../../preferences'

export type WorkbenchTourStep = {
  id: string
  target: string
  title: string
  body: string
}

export type WorkbenchTour = {
  id: string
  version: number
  steps: WorkbenchTourStep[]
  labels: {
    dialog: string
    skip: string
    back: string
    next: string
    done: string
    step: (index: number, total: number) => string
  }
  replayLabel: string
}

export function getWorkbenchTour(
  definition: Pick<WorkbenchDefinition, 'type' | 'guide'> | null | undefined,
  locale: Locale,
): WorkbenchTour | null {
  const guide = definition?.guide
  if (!definition || !guide?.steps.length) return null
  const zh = locale === 'zh-CN'
  const pick = (text: readonly [string, string]) => text[zh ? 0 : 1]
  return {
    id: definition.type,
    version: guide.version,
    steps: guide.steps.map((step) => ({ ...step, title: pick(step.title), body: pick(step.body) })),
    labels: {
      dialog: zh ? '工作台使用引导' : 'Workbench guide',
      skip: zh ? '跳过引导' : 'Skip guide',
      back: zh ? '上一步' : 'Back',
      next: zh ? '下一步' : 'Next',
      done: zh ? '开始使用' : 'Get started',
      step: (index, total) => zh ? `第 ${index + 1} / ${total} 步` : `Step ${index + 1} of ${total}`,
    },
    replayLabel: zh ? '使用引导' : 'Show guide',
  }
}
