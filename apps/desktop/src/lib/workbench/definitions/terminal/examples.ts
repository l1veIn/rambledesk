import type { WorkbenchExample } from '../contracts'
export const terminalPreviewAttachment = {
  attachment_id: 'terminal-trial-guide', file_name: '终端试用体验.md',
  markdown: '# 终端试用体验\n\n这是模拟 CLI，不执行本机命令。点击开始后，复制下面的命令到终端并按回车。真实 CLI 试用请使用 playground 的终端用例。\n\n查看帮助：\n\n```sh\ndemo --help\n```\n\n输入名字：\n\n```sh\ndemo greet\n```\n\n方向键选择、回车确认，也可以 Ctrl+C 退出：\n\n```sh\ndemo choose\n```\n\n查看错误提示：\n\n```sh\ndemo error\n```\n\n选中输出引用到反馈正文，再记录你的感受。停止终端或输入 `exit` 后，可以主动重新启动继续试用；各轮记录都会保留。',
}

export const examples: readonly WorkbenchExample[] = [{ order: 5, title: '终端试用', markdown: terminalPreviewAttachment.markdown, spec: {type:'terminal',version:1,data:{cwd:'/preview/cli-demo'}}, attachments: [{id:terminalPreviewAttachment.attachment_id,name:terminalPreviewAttachment.file_name,content:terminalPreviewAttachment.markdown,mimeType:'text/markdown'}] }]
