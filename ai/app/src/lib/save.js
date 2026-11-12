// Save a Blob to the user's disk. Browsers handle <a download>; the desktop shell shows the native "Save as"
// dialog (save_file in app/src-tauri/src/save.rs), since webviews don't all support blob downloads.
import { isTauri } from './shell.js'
import { errors } from './errors.svelte.js'

const REVOKE_MS = 60_000

/**
 * @param {Blob} blob
 * @param {string} name
 */
export async function saveBlob(blob, name) {
  if (isTauri) {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke('save_file', new Uint8Array(await blob.arrayBuffer()), { headers: { 'x-file-name': encodeURIComponent(name) } })
    } catch (e) {
      errors.report(e)
    }
    return
  }
  const href = URL.createObjectURL(blob)
  const a = Object.assign(document.createElement('a'), { href, download: name })
  document.body.append(a)
  a.click()
  a.remove()
  setTimeout(() => URL.revokeObjectURL(href), REVOKE_MS)
}

/** "skyscrapers at dusk, cinematic" -> "skyscrapers-at-dusk-cinematic" (for file names). */
export const slug = (/** @type {string} */ s, max = 48) =>
  s.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '').slice(0, max) || 'dogs-ai'
