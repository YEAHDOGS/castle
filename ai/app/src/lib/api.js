// Client for the dogs API (server/src/api.mjs). The server URL and key are kept per device in localStorage,
// so the same app can drive this PC or a remote box on the network.

const STORAGE_URL = 'dogs.server'
const STORAGE_KEY = 'dogs.key'
// Served by the dogs server or the Vite dev proxy: same origin. Inside the desktop shell (tauri://localhost, or
// http://tauri.localhost on Windows): the local server.
const DEFAULT_SERVER = location.protocol.startsWith('http') && location.hostname !== 'tauri.localhost' ? '' : 'http://127.0.0.1:8787'

const read = (k) => { try { return localStorage.getItem(k) } catch { return null } }
const write = (k, v) => { try { v ? localStorage.setItem(k, v) : localStorage.removeItem(k) } catch {} }

export const settings = {
  get server() { return (read(STORAGE_URL) ?? DEFAULT_SERVER).replace(/\/$/, '') },
  set server(v) { write(STORAGE_URL, v.trim()) },
  get key() { return read(STORAGE_KEY) || '' },
  set key(v) { write(STORAGE_KEY, v.trim()) },
}

/** Absolute URL for a server path, with the key appended for media elements that cannot send headers. */
export function url(path, withKey = false) {
  const u = settings.server + path
  if (!withKey || !settings.key) return u
  return u + (u.includes('?') ? '&' : '?') + 'key=' + encodeURIComponent(settings.key)
}

/**
 * @param {string} path
 * @param {RequestInit & { json?: any }} [opts]
 */
export async function api(path, opts = {}) {
  const headers = { ...(opts.headers || {}) }
  if (settings.key) headers.authorization = `Bearer ${settings.key}`
  // The engine only accepts JSON writes, so every POST carries a JSON body (empty if there is nothing to send).
  const json = opts.json !== undefined ? opts.json : opts.method === 'POST' ? {} : undefined
  if (json !== undefined) headers['content-type'] = 'application/json'
  const res = await fetch(url(path), { ...opts, headers, body: json !== undefined ? JSON.stringify(json) : opts.body })
  const body = await res.json().catch(() => ({}))
  if (!res.ok) throw new Error(body.error || `HTTP ${res.status}`)
  return body
}

/** Live job/download updates. Returns a function that closes the stream. */
export function subscribe(onJob, onDownload) {
  const es = new EventSource(url('/api/events', true))
  es.addEventListener('job', (e) => onJob(JSON.parse(e.data)))
  es.addEventListener('download', (e) => onDownload(JSON.parse(e.data)))
  return () => es.close()
}
