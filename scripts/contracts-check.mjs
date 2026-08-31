import { readFile, writeFile } from 'node:fs/promises'
import { spawnSync } from 'node:child_process'
import process from 'node:process'

const generated = [
  new URL('../apps/desktop/src/lib/generated/feedback.ts', import.meta.url),
  new URL('../apps/desktop/src/lib/generated/hosts.ts', import.meta.url),
]
const before = await Promise.all(generated.map((file) => readFile(file)))
let after

try {
  const command = process.env.npm_execpath ? process.execPath : 'pnpm'
  const args = process.env.npm_execpath
    ? [process.env.npm_execpath, 'contracts:generate']
    : ['contracts:generate']
  const result = spawnSync(command, args, {
    cwd: new URL('..', import.meta.url),
    stdio: 'inherit',
  })
  if (result.error) {
    throw result.error
  }
  if (result.status !== 0) {
    process.exitCode = result.status ?? 1
  } else {
    after = await Promise.all(generated.map((file) => readFile(file)))
  }
} finally {
  // Contract checks must not dirty the worktree, including through a Windows
  // checkout's CRLF conversion.
  await Promise.all(generated.map((file, index) => writeFile(file, before[index])))
}

if (after) {
  const normalize = (contents) => contents.toString('utf8').replaceAll('\r\n', '\n')
  const firstDifference = (current, generated) => {
    const currentLines = normalize(current).split('\n')
    const generatedLines = normalize(generated).split('\n')
    const length = Math.max(currentLines.length, generatedLines.length)
    for (let index = 0; index < length; index += 1) {
      if (currentLines[index] !== generatedLines[index]) {
        return {
          line: index + 1,
          current: currentLines[index] ?? '<missing>',
          generated: generatedLines[index] ?? '<missing>',
        }
      }
    }
    return null
  }
  const stale = generated.filter(
    (_, index) => normalize(before[index]) !== normalize(after[index]),
  )
  if (stale.length > 0) {
    console.error(
      `Generated contracts are stale: ${stale.map((file) => file.pathname).join(', ')}. Run \`pnpm contracts:generate\`.`,
    )
    for (const file of stale) {
      const index = generated.indexOf(file)
      const difference = firstDifference(before[index], after[index])
      if (difference) {
        console.error(
          `${file.pathname}:${difference.line}\n  current:   ${difference.current}\n  generated: ${difference.generated}`,
        )
      }
    }
    process.exitCode = 1
  } else {
    console.log('Generated feedback and host contracts are current.')
  }
}
