import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'
import tailwindcss from '@tailwindcss/vite'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// The folders beside the interface that are not part of it: the app shell, the daemon, Clones,
// DOGS AI, the audits, the radar firmware, and app/data with tens of GB of disks. Build output in
// them changes constantly and holds files open, which crashes the dev server on Windows; nothing
// in them is imported, so they are not watched.
const HERE = path.dirname(fileURLToPath(import.meta.url))
const NOT_INTERFACE = ['src-tauri', 'castled', 'clonewars', 'ai', 'security', 'gatehouse', 'data'].map((d) => path.join(HERE, d))
const notWatched = (file) => {
  const p = path.resolve(file)
  return NOT_INTERFACE.some((d) => p === d || p.startsWith(d + path.sep))
}

// https://vite.dev/config/
export default defineConfig(({ mode }) => ({
  plugins: [svelte(), tailwindcss()],
  // `vite build --mode showcase` (npm run build:showcase) makes the public, read-only demo
  // served at castle.dogs.red/demo/: .env.showcase turns VITE_SHOWCASE on (see
  // src/lib/showcase.js), and every address in it starts with /demo/.
  base: mode === 'showcase' ? '/demo/' : '/',
  // Tauri expects a fixed dev port; the web build works on any static host.
  server: { port: 1420, strictPort: true, watch: { ignored: notWatched } },
  build: {
    target: 'esnext',
    outDir: mode === 'showcase' ? 'dist-showcase' : 'dist',
  },
}))
