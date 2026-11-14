import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'
import tailwindcss from '@tailwindcss/vite'

// https://vite.dev/config/
export default defineConfig(({ mode }) => ({
  plugins: [svelte(), tailwindcss()],
  // `vite build --mode showcase` (npm run build:showcase) makes the public, read-only demo
  // served at castle.dogs.red/demo/: .env.showcase turns VITE_SHOWCASE on (see
  // src/lib/showcase.js), and every address in it starts with /demo/.
  base: mode === 'showcase' ? '/demo/' : '/',
  // Tauri expects a fixed dev port; the web build works on any static host.
  // The app shell's build output changes constantly and holds files open; watching it
  // crashes the dev server on Windows, and nothing in it is part of the interface.
  server: { port: 1420, strictPort: true, watch: { ignored: ['**/src-tauri/**'] } },
  build: {
    target: 'esnext',
    outDir: mode === 'showcase' ? 'dist-showcase' : 'dist',
  },
}))
