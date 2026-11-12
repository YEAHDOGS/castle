// Shared motion: the same expo-out feel as wearedogs (cubic-bezier(0.16, 1, 0.3, 1)) for every page and text change.
import { expoOut } from 'svelte/easing'

export const EASE = expoOut
export const PAGE_MS = 450
export const TEXT_MS = 320
const PAGE_SHIFT_PX = 48
const TEXT_SHIFT_PX = 10

const reduced = () => matchMedia('(prefers-reduced-motion: reduce)').matches

/**
 * Slide + fade. `dir` is +1 when moving to a later tab, -1 for an earlier one, so pages travel the way the nav moved.
 * @param {Element} _node
 * @param {{ dir?: number, axis?: 'x' | 'y', distance?: number, duration?: number }} [o]
 */
export function glide(_node, { dir = 1, axis = 'x', distance = PAGE_SHIFT_PX, duration = PAGE_MS } = {}) {
  if (reduced()) return { duration: 0 }
  return {
    duration,
    easing: EASE,
    css: (t, u) => `opacity: ${t}; transform: translate${axis.toUpperCase()}(${u * distance * dir}px)`,
  }
}

/** Small vertical slide for swapping text (labels, headings, button captions). */
export const textIn = (/** @type {Element} */ node) => glide(node, { axis: 'y', distance: TEXT_SHIFT_PX, duration: TEXT_MS })
export const textOut = (/** @type {Element} */ node) => glide(node, { axis: 'y', dir: -1, distance: TEXT_SHIFT_PX, duration: TEXT_MS / 2 })
