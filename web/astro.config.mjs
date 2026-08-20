import { defineConfig } from 'astro/config'
import svelte from '@astrojs/svelte'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  devToolbar: {
    enabled: false,
  },
  integrations: [svelte()],
  vite: {
    plugins: [tailwindcss()],
  },
})
