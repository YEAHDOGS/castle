// DOGS style: full-screen panels + active dot tracking.
(function () {

  // active dot follows the visible panel
  var dots = document.querySelectorAll('.dots a');
  var panels = document.querySelectorAll('.panel[id]');
  if (dots.length && 'IntersectionObserver' in window) {
    var byId = {};
    dots.forEach(function (d) { byId[d.getAttribute('href').slice(1)] = d; });
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (en) {
        if (en.isIntersecting) {
          dots.forEach(function (d) { d.classList.remove('active'); });
          var d = byId[en.target.id];
          if (d) d.classList.add('active');
        }
      });
    }, { threshold: 0.55 });
    panels.forEach(function (p) { io.observe(p); });
  }

  // HONEST NOTE: this is NOT a real 3D model — CSS 3D tilt with drag inertia
  // on a flat generated image. A true 360° spin needs a real glTF model later.
  (function initSpin() {
    var stage = document.querySelector('[data-spin-stage]');
    var tilt = document.querySelector('[data-spin]');
    if (!stage || !tilt) return;
    var reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    var rx = 0, ry = 0, vx = 0, vy = 0, idleAmp = 0;
    var dragging = false, lastX = 0, lastY = 0, downX = 0, downT = 0, lastInteract = 0;
    var visible = true;
    if ('IntersectionObserver' in window) {
      new IntersectionObserver(function (entries) { visible = entries[0].isIntersecting; }, { threshold: 0 }).observe(stage);
    }
    function frame(t) {
      requestAnimationFrame(frame);
      if (!visible || document.hidden) return;
      if (!dragging) {
        if (!reduceMotion) {
          ry += vy; rx += vx;
          vx *= 0.95; vy *= 0.95;
          if (Math.abs(vx) < 0.01) vx = 0;
          if (Math.abs(vy) < 0.01) vy = 0;
          // idle: ease a gentle oscillation in once the flick settles
          if (vx === 0 && vy === 0 && t - lastInteract > 2500) idleAmp = Math.min(1, idleAmp + 0.02);
          else idleAmp = Math.max(0, idleAmp - 0.1);
        }
        rx = Math.max(-16, Math.min(16, rx));
        var ix = Math.cos(t / 3100) * 2.5 * idleAmp;
        var iy = Math.sin(t / 2400) * 5 * idleAmp;
        tilt.style.transform = 'rotateX(' + (rx + ix).toFixed(2) + 'deg) rotateY(' + (ry + iy).toFixed(2) + 'deg)';
      }
    }
    requestAnimationFrame(frame);

    stage.addEventListener('pointerdown', function (e) {
      dragging = true;
      lastX = e.clientX; lastY = e.clientY;
      downX = e.clientX; downT = performance.now();
      vx = 0; vy = 0; lastInteract = performance.now();
      try { stage.setPointerCapture(e.pointerId); } catch (err) {}
      stage.classList.add('grabbing');
    });
    stage.addEventListener('pointermove', function (e) {
      if (!dragging) return;
      var dx = e.clientX - lastX, dy = e.clientY - lastY;
      lastX = e.clientX; lastY = e.clientY;
      ry += dx * 0.25;
      rx = Math.max(-16, Math.min(16, rx + dy * 0.12));
      vx = dy * 0.12; vy = dx * 0.25;
      lastInteract = performance.now();
      tilt.style.transform = 'rotateX(' + rx.toFixed(2) + 'deg) rotateY(' + ry.toFixed(2) + 'deg)';
    });
    function endSpin(e) {
      if (!dragging) return;
      dragging = false;
      stage.classList.remove('grabbing');
      lastInteract = performance.now();
      // plain click (no drag): give it a satisfying flick
      if (!reduceMotion && Math.abs(e.clientX - downX) < 6 && performance.now() - downT < 350) vy += 5;
    }
    stage.addEventListener('pointerup', endSpin);
    stage.addEventListener('pointercancel', endSpin);
  })();

})();