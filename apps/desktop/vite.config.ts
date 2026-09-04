import tailwindcss from '@tailwindcss/vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig, type Plugin } from 'vite'
import { readFileSync } from 'node:fs'
import path from 'node:path'

// Keep one source copy in the repository; both shipped clients receive its bytes.
function thirdPartyNotices(): Plugin {
  return {
    name: 'rambledesk-third-party-notices',
    apply: 'build',
    buildStart() {
      for (const fileName of [
        'THIRD_PARTY_NOTICES.md',
        'licenses/codeg-APACHE-2.0.txt',
        'docs/CODEG_PORTS.md',
      ]) {
        this.emitFile({
          type: 'asset',
          name: fileName,
          fileName,
          originalFileName: fileName,
          source: readFileSync(new URL(`../../${fileName}`, import.meta.url)),
        })
      }
    },
  }
}

export default defineConfig({
  plugins: [thirdPartyNotices(), tailwindcss(), svelte()],
  build: {
    // Web Access reads this exact output inventory from the Tauri asset bundle.
    // It is the source of truth for immutable caching and missing-asset rejection.
    manifest: true,
  },
  resolve: {
    alias: {
      $lib: path.resolve('./src/lib'),
    },
  },
  clearScreen: false,
  server: {
    // Bind IPv4 explicitly: with the default host, Node 21+ resolves
    // localhost to ::1 only, and WebView2 (and some browsers) resolve
    // localhost to 127.0.0.1 first — every asset request then fails with
    // ERR_CONNECTION_REFUSED even though the page loaded.
    host: '127.0.0.1',
    port: 1420,
    strictPort: true,
    watch: {
      // Atomic editors (including the DSH file backend) write a sibling
      // `<file>.<pid>.<uuid>.tmpdir/<file>.tmp` before renaming. Watching
      // those files on Windows hits EBUSY and crashes the whole dev server,
      // leaving WebView2 stuck on the previous module graph. Never watch them.
      ignored: ['**/.*.tmpdir/**', '**/*.tmp', '**/*~'],
    },
  },
  envPrefix: ['VITE_', 'TAURI_'],
})
