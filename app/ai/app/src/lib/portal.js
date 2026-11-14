// Render an element on <body>. Panels with backdrop-filter or transforms become the containing block for
// `position: fixed` descendants, so a modal inside a Library section would be clipped to that section.
// Use as an attachment: <div class="contents" {@attach portal}>.
export function portal(/** @type {HTMLElement} */ node) {
  document.body.appendChild(node)
  return () => node.remove()
}
