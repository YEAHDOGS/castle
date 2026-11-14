// The desktop shell (Tauri, app/src-tauri). In a browser none of this applies.
import { openUrl } from '@tauri-apps/plugin-opener'

export const isTauri = '__TAURI_INTERNALS__' in window
export const isMac = /Mac/.test(navigator.platform || navigator.userAgent)

/** Tell the CSS it runs in the desktop shell (the header becomes the title bar), and send web links to the browser. */
export function setupShell() {
  if (!isTauri) return
  document.documentElement.dataset.shell = 'tauri'
  document.documentElement.dataset.platform = isMac ? 'darwin' : 'other'
  // A webview would open target=_blank links in a bare popup window; the system browser is where they belong.
  document.addEventListener('click', (e) => {
    const a = /** @type {Element} */ (e.target).closest?.('a[href]')
    if (!(a instanceof HTMLAnchorElement) || !/^https?:$/.test(a.protocol) || a.origin === location.origin) return
    e.preventDefault()
    openUrl(a.href).catch(() => {})
  })
}
