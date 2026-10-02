import type { TerminalData } from '../../../generated/feedback'
import type { WorkbenchDefinition } from '../contracts'
import { validTerminalInput } from './input'
import { readTerminalState } from './state'
import { hasTerminalInput, validateTerminalState } from '../../terminalModel'
import { createTerminalWorkbenchController } from '../../terminal/terminalWorkbenchController'
import { examples } from './examples'
const message: WorkbenchDefinition['submissionMessage'] = (spec, state) => state && state.type !== 'terminal'
  ? 'Invalid terminal trial state.' : validateTerminalState(spec.data as TerminalData, state?.type === 'terminal' ? state : null)
export const terminalDefinition: WorkbenchDefinition = {
  type: 'terminal', version: 1, accepts: validTerminalInput, decodeState: readTerminalState,
  hasInput: (spec, state) => hasTerminalInput(spec.data as TerminalData, state?.type === 'terminal' ? state : null),
  complete: (spec, state) => message(spec, state) === null, submissionMessage: message,
  layout: { padded: false, interactivePreview: true, expanded: true },
  loadView: () => import('./View.svelte'), createController: createTerminalWorkbenchController, examples,
  guide: {
    version: 1,
    steps: [
      {
        id: 'directory', target: '[data-tour="terminal-directory"]',
        title: ['确认这次试用的工作目录', 'Check the trial directory'],
        body: ['终端会在这里显示的目录中运行。先阅读请求中的体验说明和命令，再开始这次 CLI 试用。', 'The terminal runs in the directory shown here. Read the request instructions and suggested commands before starting the CLI trial.'],
      },
      {
        id: 'toolbar', target: '[data-terminal-toolbar]',
        title: ['主动启动，按需停止或重开', 'Start, stop or restart when ready'],
        body: ['点击开始终端才会创建试用会话。停止或退出后可以重新开始，各轮记录都会保留；也能打开全屏试用，或通过样式按钮选择预置外观。', 'Start terminal creates the trial session. After stopping or exiting, restart to begin another trial while keeping earlier records. You can also open a full screen trial or choose a preset terminal appearance.'],
      },
      {
        id: 'surface', target: '[data-terminal-surface]',
        title: ['在真实终端里体验 CLI', 'Try the CLI in a real terminal'],
        body: ['启动后在这里输入命令，检查帮助、交互提示和错误信息，也可以试用全屏文字界面。普通工作台和全屏页签继续同一个终端会话。', 'Once started, enter commands here to check help, prompts and errors, or try a full screen text interface. The normal workbench and full screen tab continue the same terminal session.'],
      },
      {
        id: 'quote', target: '[data-tour="terminal-quote"]',
        title: ['选中输出，把证据引用到反馈中', 'Quote selected output into your feedback'],
        body: ['在终端中选中相关输出，再点击引用选中输出，把这段内容放到右侧正文，接着说明观察或问题。没有选中内容时，这个按钮会暂时禁用。', 'Select relevant terminal output, then use Quote selected output to add it to the feedback on the right. Explain your observation or issue alongside it. The button is disabled until output is selected.'],
      },
      {
        id: 'submit', target: '[data-feedback-actions]',
        title: ['说明体验结果，连同记录提交', 'Send your experience and trial records'],
        body: ['在右侧写下体验结果并提交。提交会收尾仍在运行的终端，并保存试用输出和最后的画面；引导本身不会启动终端或执行命令。', 'Write your experience on the right and submit when ready. Submission finishes any running terminal and preserves trial output and the final screen. This guide never starts a terminal or runs commands.'],
      },
    ],
  },
}
