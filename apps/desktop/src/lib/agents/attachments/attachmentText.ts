import type { Locale } from '$lib/preferences'

const zh: Record<string, string> = {
  'Attach files': '添加文件',
  'A message can contain at most 16 content blocks.': '每条消息最多包含 16 个内容块。',
  'Images must be 1.5 MiB or smaller; text files must be 256 KiB or smaller.': '图片不得超过 1.5 MiB，文本文件不得超过 256 KiB。',
  'The attachment filename is invalid.': '附件文件名无效。',
  'This agent does not accept image attachments.': '此 Agent 不接受图片附件。',
  'Choose a PNG, JPEG, GIF, WebP image or a UTF-8 text file.': '请选择 PNG、JPEG、GIF、WebP 图片或 UTF-8 文本文件。',
  'This agent does not accept text-file attachments.': '此 Agent 不接受文本文件附件。',
  'Text files must be 256 KiB or smaller.': '文本文件不得超过 256 KiB。',
  'Text attachments must use UTF-8 encoding.': '文本附件必须使用 UTF-8 编码。',
  'This file contains binary data. Choose a UTF-8 text file.': '此文件包含二进制数据，请选择 UTF-8 文本文件。',
  'This agent does not accept resource links.': '此 Agent 不接受资源链接。',
  'Message text and text attachments together must be 256 KiB or smaller.': '消息正文与文本附件的总大小不得超过 256 KiB。',
  'Message content must be 4 MiB or smaller.': '消息内容的总大小不得超过 4 MiB。',
  'This session does not support typed attachments.': '此会话不支持附件。',
  'Could not send these attachments.': '无法发送这些附件。',
}

export function attachmentText(locale: Locale, message: string): string { return locale === 'zh-CN' ? zh[message] ?? message : message }
