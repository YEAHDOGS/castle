// Castle landing — vanilla JS. No dependencies. Motion is transform/opacity
// only, 60fps, never blocks input, honors prefers-reduced-motion.
// Waitlist contract mirrors the original app.js integration point:
// set window.CASTLE_WAITLIST_API to a backend base URL to go live.
(function () {
  'use strict';
  var reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  // ---- scroll progress hairline
  (function initProgress() {
    var bar = document.querySelector('[data-progress]');
    if (!bar) return;
    var ticking = false;
    function update() {
      ticking = false;
      var max = document.documentElement.scrollHeight - window.innerHeight;
      var p = max > 0 ? window.scrollY / max : 0;
      bar.style.transform = 'scaleX(' + Math.min(1, Math.max(0, p)).toFixed(4) + ')';
    }
    window.addEventListener('scroll', function () {
      if (!ticking) { ticking = true; requestAnimationFrame(update); }
    }, { passive: true });
    update();
  })();

  // ---- scroll reveals
  (function initReveals() {
    var els = document.querySelectorAll('.reveal');
    if (!els.length) return;
    if (!('IntersectionObserver' in window) || reduceMotion) {
      els.forEach(function (el) { el.classList.add('in'); });
      return;
    }
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (en) {
        if (en.isIntersecting) {
          en.target.classList.add('in');
          io.unobserve(en.target);
        }
      });
    }, { threshold: 0.12, rootMargin: '0px 0px -6% 0px' });
    els.forEach(function (el) { io.observe(el); });
  })();

  // ---- hero tilt: 2.5D drag + inertia on the generated art.
  // HONEST NOTE: not a true 3D model — CSS 3D tilt on a flat image.
  (function initSpin() {
    var stage = document.querySelector('[data-spin-stage]');
    var tilt = document.querySelector('[data-spin]');
    if (!stage || !tilt) return;
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
      if (!reduceMotion && Math.abs(e.clientX - downX) < 6 && performance.now() - downT < 350) vy += 5;
    }
    stage.addEventListener('pointerup', endSpin);
    stage.addEventListener('pointercancel', endSpin);
  })();

  // ---- sticky mobile CTA: appears once the hero is scrolled past
  (function initSticky() {
    var cta = document.querySelector('[data-sticky-cta]');
    var hero = document.querySelector('.hero');
    var join = document.querySelector('#waitlist');
    if (!cta || !('IntersectionObserver' in window)) return;
    var pastHero = false, atJoin = false;
    function render() {
      var show = pastHero && !atJoin && window.innerWidth < 720;
      cta.hidden = !show;
      cta.classList.toggle('show', show);
    }
    new IntersectionObserver(function (entries) {
      pastHero = !entries[0].isIntersecting && entries[0].boundingClientRect.top < 0;
      render();
    }, { threshold: 0 }).observe(hero);
    if (join) new IntersectionObserver(function (entries) {
      atJoin = entries[0].isIntersecting;
      render();
    }, { threshold: 0.15 }).observe(join);
    window.addEventListener('resize', render);
  })();

  // ---- waitlist
  var WAITLIST_API = (window.CASTLE_WAITLIST_API || '').replace(/\/+$/, '');

  // counter: real number from the backend, or honest framing. Never a fake number.
  (function initCounter() {
    var line = document.querySelector('[data-count-line]');
    if (!line) return;
    var FOUNDING = 'Be one of the first in line.';
    if (!WAITLIST_API) { line.textContent = FOUNDING; return; }
    fetch(WAITLIST_API + '/waitlist/count', { headers: { 'Accept': 'application/json' } })
      .then(function (res) { return res.json(); })
      .then(function (data) {
        var n = (data && typeof data.count === 'number') ? data.count : null;
        if (n === null || n < 0) { line.textContent = FOUNDING; return; }
        line.textContent = n === 0 ? 'Be the first in line.'
          : n === 1 ? '1 person waiting.'
          : n.toLocaleString('en-US') + ' people waiting.';
      })
      .catch(function () { line.textContent = FOUNDING; });
  })();

  // ticker fires only on real joins.
  (function initTicker() {
    var ticker = document.querySelector('[data-ticker]');
    if (!ticker) return;
    document.addEventListener('castle:waitlist-joined', function () {
      ticker.hidden = false;
      ticker.classList.remove('show');
      void ticker.offsetWidth;
      ticker.classList.add('show');
      clearTimeout(ticker._hide);
      ticker._hide = setTimeout(function () {
        ticker.hidden = true;
        ticker.classList.remove('show');
      }, 2700);
    });
  })();

  var form = document.querySelector('form[data-waitlist]');
  if (!form) return;

  var steps = {
    email: form.querySelector('[data-step="email"]'),
    code: form.querySelector('[data-step="code"]'),
    done: form.querySelector('[data-step="done"]')
  };
  var emailInput = form.querySelector('#email');
  var emailError = form.querySelector('#email-error');
  var emailButton = steps.email.querySelector('button[type="submit"]');
  var codeInput = form.querySelector('#code');
  var codeError = form.querySelector('#code-error');
  var codeButton = steps.code.querySelector('button[type="submit"]');
  var resendButton = form.querySelector('[data-resend]');
  var emailEcho = form.querySelector('[data-email-echo]');
  var pendingEmail = '';

  function showStep(name) {
    Object.keys(steps).forEach(function (k) { steps[k].hidden = (k !== name); });
  }
  function setError(input, errEl, msg) {
    errEl.textContent = msg;
    input.setAttribute('aria-invalid', msg ? 'true' : 'false');
    if (msg) input.focus();
  }
  function setLoading(button, on) {
    button.disabled = on;
    button.classList.toggle('loading', on);
  }
  function validEmail(value) {
    return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value);
  }
  function friendlyError(data, fallback) {
    var map = {
      bad_code: 'That code does not match. Try again.',
      expired: 'That code expired. Ask for a fresh one below.',
      too_many: 'Too many tries. Take a breath and try again in a bit.',
      invalid_email: 'That does not look like an email address.'
    };
    if (data && data.error && map[data.error]) return map[data.error];
    return fallback;
  }
  function post(path, body) {
    return fetch(WAITLIST_API + path, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body)
    }).then(function (res) {
      return res.json().catch(function () { return {}; }).then(function (data) {
        return { status: res.status, ok: res.ok && data.ok !== false, data: data };
      });
    });
  }

  emailInput.addEventListener('input', function () { setError(emailInput, emailError, ''); });
  codeInput.addEventListener('input', function () { setError(codeInput, codeError, ''); });

  steps.email.addEventListener('submit', function (e) {
    e.preventDefault();
    var value = emailInput.value.trim();
    if (!value) { setError(emailInput, emailError, 'Please enter your email address.'); return; }
    if (!validEmail(value)) { setError(emailInput, emailError, 'That does not look like an email address.'); return; }
    setError(emailInput, emailError, '');

    // Placeholder mode: no backend yet — honest redirect; nothing is stored.
    if (!WAITLIST_API) {
      setLoading(emailButton, true);
      setTimeout(function () { window.location.href = './thanks.html'; }, 800);
      return;
    }

    pendingEmail = value;
    setLoading(emailButton, true);
    post('/waitlist/join', { email: value }).then(function (r) {
      setLoading(emailButton, false);
      if (r.ok) {
        emailEcho.textContent = pendingEmail;
        showStep('code');
        codeInput.focus();
        return;
      }
      setError(emailInput, emailError, friendlyError(r.data, 'Something went wrong on our end. Try again in a bit.'));
    }).catch(function () {
      setLoading(emailButton, false);
      setError(emailInput, emailError, 'Could not reach the waitlist. Check your connection and try again.');
    });
  });

  steps.code.addEventListener('submit', function (e) {
    e.preventDefault();
    var value = codeInput.value.trim();
    if (!value) { setError(codeInput, codeError, 'Please enter the confirmation code.'); return; }
    setError(codeInput, codeError, '');
    setLoading(codeButton, true);
    post('/waitlist/confirm', { email: pendingEmail, code: value }).then(function (r) {
      setLoading(codeButton, false);
      if (r.ok) {
        showStep('done');
        document.dispatchEvent(new CustomEvent('castle:waitlist-joined'));
        return;
      }
      setError(codeInput, codeError, friendlyError(r.data, 'That code did not work. Try again.'));
    }).catch(function () {
      setLoading(codeButton, false);
      setError(codeInput, codeError, 'Could not reach the waitlist. Check your connection and try again.');
    });
  });

  if (resendButton) {
    resendButton.addEventListener('click', function () {
      if (!WAITLIST_API || !pendingEmail) return;
      setError(codeInput, codeError, '');
      setLoading(resendButton, true);
      post('/waitlist/join', { email: pendingEmail }).then(function (r) {
        setLoading(resendButton, false);
        if (r.ok) {
          codeError.textContent = 'Fresh code sent. Check your inbox.';
          codeInput.focus();
        } else {
          setError(codeInput, codeError, friendlyError(r.data, 'Could not resend the code. Try again.'));
        }
      }).catch(function () {
        setLoading(resendButton, false);
        setError(codeInput, codeError, 'Could not reach the waitlist. Check your connection and try again.');
      });
    });
  }
})();
