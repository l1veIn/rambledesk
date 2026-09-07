/** This module must stay independent of preferences, Svelte and native bindings. */
export type StartupFailure = Readonly<{ code: 'STARTUP_TIMEOUT' | 'STARTUP_FAILED'; category: string }>

export function describeStartupFailure(cause: unknown): StartupFailure {
  const category = cause instanceof TypeError ? 'type_error'
    : cause instanceof SyntaxError ? 'syntax_error'
    : cause instanceof ReferenceError ? 'reference_error'
    : cause instanceof RangeError ? 'range_error' : 'unknown'
  return { code: 'STARTUP_FAILED', category }
}

/** Stop waiting without allowing a late module load to mount over the recovery screen. */
export async function runFrontendBootstrap(
  bootstrap: (signal: AbortSignal) => Promise<void>,
  onFailure: (failure: StartupFailure) => void,
  timeoutMs = 30_000,
): Promise<void> {
  const controller = new AbortController()
  let settled = false
  let timer: ReturnType<typeof setTimeout> | undefined
  await new Promise<void>((resolve) => {
    const fail = (failure: StartupFailure) => {
      if (settled) return
      settled = true
      controller.abort()
      clearTimeout(timer)
      try { onFailure(failure) } finally { resolve() }
    }
    timer = setTimeout(() => fail({ code: 'STARTUP_TIMEOUT', category: 'timeout' }), timeoutMs)
    Promise.resolve().then(() => bootstrap(controller.signal)).then(() => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      resolve()
    }, (cause: unknown) => fail(describeStartupFailure(cause)))
  })
}

export function showStartupFailure(target: HTMLElement, failure: StartupFailure): void {
  const zh = navigator.language.toLowerCase().startsWith('zh')
  const text = (chinese: string, english: string) => zh ? chinese : english
  const panel = document.createElement('section')
  panel.setAttribute('role', 'alert')
  panel.style.cssText = 'position:fixed;inset:0;z-index:2147483647;overflow:auto;display:grid;place-content:center;gap:16px;padding:32px;background:#14202d;color:#e4edf5;font:16px/1.6 system-ui;'
  const title = document.createElement('h1')
  title.textContent = text('RambleDesk 未能完成启动', 'RambleDesk could not finish starting')
  title.style.cssText = 'font-size:24px;margin:0;'
  const explanation = document.createElement('p')
  explanation.style.cssText = 'max-width:640px;margin:0;'
  explanation.textContent = failure.code === 'STARTUP_TIMEOUT'
    ? text('启动等待已超时。你可以重新加载；如果仍然失败，请将下面的错误信息反馈给我们。', 'Startup took too long. Reload the app; if it still fails, share the error information below.')
    : text('加载界面组件或初始化设置时发生错误。请重新加载；如果仍然失败，请将下面的错误信息反馈给我们。', 'An error occurred while loading the interface or initializing settings. Reload the app; if it still fails, share the error information below.')
  const details = document.createElement('pre')
  details.textContent = `RambleDesk\n${failure.code}\n${failure.category}\n${new Date().toISOString()}`
  details.style.cssText = 'white-space:pre-wrap;user-select:text;border:1px solid #64748b;border-radius:8px;padding:16px;'
  const actions = document.createElement('div')
  actions.style.cssText = 'display:flex;flex-wrap:wrap;gap:12px;'
  const button = (label: string, action: () => void) => {
    const element = document.createElement('button')
    element.type = 'button'
    element.textContent = label
    element.style.cssText = 'border:1px solid #94a3b8;border-radius:6px;background:#263d52;color:inherit;padding:10px 16px;font:inherit;cursor:pointer;'
    element.addEventListener('click', action)
    actions.append(element)
    return element
  }
  button(text('重新加载', 'Reload'), () => window.location.reload())
  const copy = button(text('复制错误信息', 'Copy error information'), () => {
    void Promise.resolve().then(() => navigator.clipboard.writeText(details.textContent ?? '')).then(() => {
      copy.textContent = text('已复制', 'Copied')
    }, () => { copy.textContent = text('请手动选择并复制上方信息', 'Select and copy the information above') })
  })
  panel.append(title, explanation, details, actions)
  target.replaceChildren(panel)
}
