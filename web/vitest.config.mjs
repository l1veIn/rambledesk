import { createRequire } from 'node:module'
import { dirname } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

// Reuse the workspace's desktop test toolchain without adding site dependencies.
const desktop = createRequire(new URL('../apps/desktop/package.json', import.meta.url))
const { svelte } = await import(pathToFileURL(desktop.resolve('@sveltejs/vite-plugin-svelte')).href)

export default {
  root: fileURLToPath(new URL('.', import.meta.url)),
  plugins: [svelte({ configFile: false, prebundleSvelteLibraries: false })],
  resolve: {
    conditions: ['browser'],
    alias: { vitest: dirname(desktop.resolve('vitest/package.json')) },
  },
  test: {
    environment: 'jsdom',
    include: ['tests/**/*.test.mjs'],
    maxWorkers: 1,
  },
}
