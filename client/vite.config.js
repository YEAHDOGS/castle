import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'
import tailwindcss from '@tailwindcss/vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [svelte(), tailwindcss()],
  // Tauri expects a fixed dev port; the web build works on any static host.
  server: { port: 1420, strictPort: true },
  build: {
    target: 'esnext',
  },
})
