import { readFile, writeFile } from 'node:fs/promises'
import { spawnSync } from 'node:child_process'
import process from 'node:process'

const generated = new URL('../apps/desktop/src/lib/generated/feedback.ts', import.meta.url)
const before = await readFile(generated)
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
    after = await readFile(generated)
  }
} finally {
  // Contract checks must not dirty the worktree, including through a Windows
  // checkout's CRLF conversion.
  await writeFile(generated, before)
}

if (after) {
  const normalize = (contents) => contents.toString('utf8').replaceAll('\r\n', '\n')
  if (normalize(before) !== normalize(after)) {
    console.error('Generated feedback contracts are stale. Run `pnpm contracts:generate`.')
    process.exitCode = 1
  } else {
    console.log('Generated feedback contracts are current.')
  }
}
