import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'
import tailwindcss from '@tailwindcss/vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [svelte(), tailwindcss()],
  // Tauri expects a fixed dev port; the web build works on any static host.
  // The app shell's build output changes constantly and holds files open; watching it
  // crashes the dev server on Windows, and nothing in it is part of the interface.
  server: { port: 1420, strictPort: true, watch: { ignored: ['**/src-tauri/**'] } },
  build: {
    target: 'esnext',
  },
})
