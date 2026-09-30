import { object, fields, list, text, optionalText, uniqueId, type RecordValue } from '../validation'

export function validTerminalInput(data: RecordValue): boolean {
  const ids = new Set<string>()
  return fields(data, ['cwd', 'shell', 'commands']) && text(data.cwd, 8192)
    && (data.shell == null || text(data.shell, 8192))
    && (data.commands === undefined || (list(data.commands, 0, 20)
      && data.commands.every((command) => object(command) && fields(command, ['id', 'title', 'command', 'description'])
        && uniqueId(command.id, ids) && text(command.title, 200) && text(command.command, 4000)
        && !/[\u0000-\u001f\u007f-\u009f]/.test(command.command) && optionalText(command.description, 2000))))
}
