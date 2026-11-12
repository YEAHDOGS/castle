// Eases a container's height to the page that is coming in, so its scroll container's scrollbar thumb grows or
// shrinks smoothly instead of jumping when the outgoing page is removed. Native scrollbars can't be animated
// directly, but they follow the content height, and that can be.
import { PAGE_MS } from './motion.js'

const EASE_CSS = 'cubic-bezier(0.16, 1, 0.3, 1)'

export class AutoHeight {
  /** Height of the newest page, in px. null until the first one is measured (renders at natural height). */
  height = $state(/** @type {number | null} */ (null))
  /** True while easing to a new page; growth inside the same page (typing, a new gallery card) applies at once. */
  easing = $state(false)
  #current = /** @type {Element | null} */ (null)
  #timer = 0

  style = $derived(
    this.height === null
      ? ''
      : `height: ${this.height}px; transition: ${this.easing && !reducedMotion() ? `height ${PAGE_MS}ms ${EASE_CSS}` : 'none'}`,
  )

  /**
   * Attachment for each page inside the container: `{@attach auto.track}`. The most recently mounted page is the
   * one the container sizes to; pages that are leaving keep rendering but are no longer measured.
   * @param {Element} node
   */
  track = (node) => {
    const isSwap = this.#current !== null
    this.#current = node
    if (isSwap) {
      this.easing = true
      clearTimeout(this.#timer)
      this.#timer = setTimeout(() => (this.easing = false), PAGE_MS)
    }
    const ro = new ResizeObserver(([entry]) => {
      if (node !== this.#current) return
      this.height = entry.borderBoxSize?.[0]?.blockSize ?? node.getBoundingClientRect().height
    })
    ro.observe(node)
    return () => ro.disconnect()
  }
}

function reducedMotion() {
  return matchMedia('(prefers-reduced-motion: reduce)').matches
}
