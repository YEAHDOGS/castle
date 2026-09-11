// Castle Arcade shelf — coverflow tilt on a native scroll strip.
// Vanilla, no deps. transform/opacity only, 60fps, never blocks input.
(function () {
  'use strict';
  var reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  var shelf = document.querySelector('[data-shelf]');
  if (!shelf) return;

  var cases = Array.prototype.slice.call(shelf.querySelectorAll('.case'));
  var infoT = document.querySelector('[data-info-title]');
  var infoM = document.querySelector('[data-info-meta]');
  var infoOpen = document.querySelector('[data-info-open]');
  var current = -1;

  function center() {
    var r = shelf.getBoundingClientRect();
    return r.left + r.width / 2;
  }

  function layout() {
    var c = center();
    var best = 0, bestD = Infinity;
    cases.forEach(function (el, i) {
      var r = el.getBoundingClientRect();
      var d = (r.left + r.width / 2 - c) / r.width; // case-widths from center
      var ad = Math.abs(d);
      if (ad < bestD) { bestD = ad; best = i; }
      if (!reduceMotion) {
        var ang = Math.max(-55, Math.min(55, d * 38));
        var z = -Math.min(220, ad * 130);
        var s = ad < 0.5 ? 1.06 - ad * 0.12 : 1;
        el.style.transform =
          'rotateY(' + ang.toFixed(2) + 'deg)' +
          ' translateZ(' + z.toFixed(1) + 'px)' +
          ' scale(' + s.toFixed(3) + ')';
        el.style.zIndex = String(100 - Math.round(ad * 10));
      }
    });
    if (best !== current) {
      current = best;
      var el = cases[best];
      if (infoT) infoT.textContent = el.getAttribute('data-title') || '';
      if (infoM) infoM.textContent = el.getAttribute('data-meta') || '';
      if (infoOpen) infoOpen.setAttribute('href', el.getAttribute('href'));
      cases.forEach(function (x, i) { x.toggleAttribute('data-focused', i === best); });
    }
  }

  var ticking = false;
  function onScroll() {
    if (ticking) return;
    ticking = true;
    requestAnimationFrame(function () { ticking = false; layout(); });
  }
  shelf.addEventListener('scroll', onScroll, { passive: true });
  window.addEventListener('resize', onScroll);

  function go(i) {
    i = Math.max(0, Math.min(cases.length - 1, i));
    cases[i].scrollIntoView({ behavior: reduceMotion ? 'auto' : 'smooth', inline: 'center', block: 'nearest' });
  }

  var prev = document.querySelector('[data-shelf-prev]');
  var next = document.querySelector('[data-shelf-next]');
  if (prev) prev.addEventListener('click', function () { go(current - 1); });
  if (next) next.addEventListener('click', function () { go(current + 1); });

  // Arrow keys move along the shelf when a case has focus.
  shelf.addEventListener('keydown', function (e) {
    var idx = cases.indexOf(document.activeElement);
    if (idx < 0) return;
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') { e.preventDefault(); cases[Math.min(cases.length - 1, idx + 1)].focus({ preventScroll: true }); go(idx + 1); }
    if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') { e.preventDefault(); cases[Math.max(0, idx - 1)].focus({ preventScroll: true }); go(idx - 1); }
    if (e.key === 'Home') { e.preventDefault(); go(0); cases[0].focus({ preventScroll: true }); }
    if (e.key === 'End') { e.preventDefault(); go(cases.length - 1); cases[cases.length - 1].focus({ preventScroll: true }); }
  });
  // Keep the readout honest when tabbing along the row.
  shelf.addEventListener('focusin', function () {
    var idx = cases.indexOf(document.activeElement);
    if (idx >= 0) { go(idx); }
  });

  layout();
})();
