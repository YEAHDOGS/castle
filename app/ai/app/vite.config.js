import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import tailwindcss from '@tailwindcss/vite'

// The app runs in the Tauri shell (WebView2, WKWebView, WebKitGTK) or a current browser, so there is no legacy tier.
const BUILD_TARGET = ['chrome111', 'edge111', 'firefox114', 'safari16.4']
// `npm run dev` proxies the API so the dev page talks to `dogs serve` like the built app does.
const API = `http://127.0.0.1:${process.env.DOGS_PORT || 8787}`

export default defineConfig({
  plugins: [tailwindcss(), svelte()],
  // Relative asset URLs: the same build works from the dogs server, file:// and the Tauri shell.
  base: './',
  // The only big chunk is lib/convert.js (Mediabunny + the MP3 encoder, ~0.9 MB). It is lazy-loaded when the
  // converter opens, never on startup, so the warning limit is raised just above it.
  build: { target: BUILD_TARGET, cssTarget: BUILD_TARGET, chunkSizeWarningLimit: 1000 },
  server: { proxy: { '/api': API, '/files': API } },
})
