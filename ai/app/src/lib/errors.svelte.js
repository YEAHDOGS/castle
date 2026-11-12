// Errors the user should see: anything thrown or rejected that no component handled ends up as a toast
// (components/ErrorToasts.svelte), so a failed click never looks like nothing happened.

const MAX_SHOWN = 3
// Browser notices, not failures: Chromium reports a ResizeObserver whose callback resized something it watches
// as an error event, then simply delivers the rest next frame. Nothing broke, so there is nothing to show.
const BENIGN = /^ResizeObserver loop (completed with undelivered notifications|limit exceeded)/

/** @param {unknown} e */
export const messageOf = (e) => {
  const m = e instanceof Error ? e.message : typeof e === 'string' ? e : String((/** @type {any} */ (e))?.message ?? e)
  return m.trim() || 'Unknown error'
}

class Errors {
  /** @type {{ id: number, message: string }[]} */ items = $state([])
  #next = 0

  /** @param {unknown} e */
  report(e) {
    const message = messageOf(e)
    console.error(e)
    // The same failure repeating (a retry loop, a re-render) shows once.
    if (this.items.some((x) => x.message === message)) return
    this.items = [...this.items, { id: this.#next++, message }].slice(-MAX_SHOWN)
  }

  /** @param {number} id */
  dismiss(id) { this.items = this.items.filter((x) => x.id !== id) }

  /** Catch what nothing else did. Returns a function that stops listening. */
  listen() {
    const onError = (/** @type {ErrorEvent} */ e) => {
      if (!e.error && BENIGN.test(e.message)) return
      this.report(e.error ?? e.message)
    }
    const onRejection = (/** @type {PromiseRejectionEvent} */ e) => this.report(e.reason)
    addEventListener('error', onError)
    addEventListener('unhandledrejection', onRejection)
    return () => {
      removeEventListener('error', onError)
      removeEventListener('unhandledrejection', onRejection)
    }
  }
}

export const errors = new Errors()
