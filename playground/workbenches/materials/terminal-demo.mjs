// Fixed CLI fixture. No file writes, dependencies, credentials or network calls.
import { createInterface, emitKeypressEvents } from 'node:readline'

const mode = process.argv[2] ?? '--help'
if (mode === '--help' || mode === '-h') {
  console.log('CLI Demo — 终端试用\n\n用法：node ./materials/terminal-demo.mjs <命令>\n\n  greet   输入名字，完成交互式问候\n  choose  用方向键选择布局，回车确认\n  error   查看一个错误及处理提示\n  --help  显示帮助\n\nCtrl+C 随时退出当前命令。')
} else if (mode === 'greet') {
  const input = createInterface({ input: process.stdin, output: process.stdout })
  input.question('你的名字：', (name) => { console.log(`\u001b[32m你好，${name.trim() || '朋友'}！\u001b[0m`); input.close() })
} else if (mode === 'choose') {
  if (!process.stdin.isTTY) { console.error('布局选择需要交互式终端。请在终端工作台中运行此命令。'); process.exitCode = 2 }
  else {
    emitKeypressEvents(process.stdin)
    process.stdin.setRawMode(true)
    process.stdin.resume()
    const options = ['紧凑布局', '宽松布局', '自适应布局']
    let index = 0
    const draw = () => process.stdout.write(`\r\u001b[2K${options.map((item, i) => i === index ? `\u001b[36m> ${item}\u001b[0m` : `  ${item}`).join('   ')}`)
    console.log('左右/上下方向键选择，回车确认，Ctrl+C 退出。')
    draw()
    process.stdin.on('keypress', (_text, key) => {
      if (key.ctrl && key.name === 'c') { process.stdin.setRawMode(false); process.stdout.write('\r\n已退出选择。\r\n'); process.exit(130) }
      if (['up', 'left'].includes(key.name)) index = (index + options.length - 1) % options.length
      if (['down', 'right'].includes(key.name)) index = (index + 1) % options.length
      if (key.name === 'return') { process.stdin.setRawMode(false); console.log(`\n你选择了${options[index]}。`); process.exit(0) }
      draw()
    })
  }
} else if (mode === 'error') {
  console.error('\u001b[31m错误：缺少项目配置。\u001b[0m\n下一步：运行 --help 查看可用命令。这是演示错误，不会更改任何文件。')
  process.exitCode = 2
} else {
  console.error(`未知命令：${mode}\n运行 --help 查看可用命令。`)
  process.exitCode = 2
}
