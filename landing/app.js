// DOGS style: y-scroll-snap panels + active dot tracking.
// Castle waitlist: two-step (email -> confirmation code).
(function () {
  document.documentElement.classList.add('snap');

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

  // ============================================================
  // WAITLIST CONFIG — single integration point.
  //
  // window.CASTLE_WAITLIST_API is the base URL of the waitlist backend
  // (planned: one Cloudflare Worker). Leave it empty for placeholder
  // mode: the form validates the email and redirects to thanks.html,
  // which states honestly that nothing was stored.
  //
  // INTEGRATION CONTRACT (when the backend exists):
  //   POST <API>/waitlist/join    { "email": "you@example.com" }
  //     -> 200 { "ok": true }
  //        Worker generates a confirmation code, stores it server-side
  //        (with expiry), and emails it to the address.
  //     -> 4xx { "ok": false, "error": "<machine-readable reason>" }
  //   POST <API>/waitlist/confirm { "email": "you@example.com", "code": "123456" }
  //     -> 200 { "ok": true }            (spot confirmed)
  //     -> 4xx { "ok": false, "error": "bad_code" | "expired" | ... }
  //        The UI surfaces error.error in plain words; unknown codes
  //        fall back to a generic message.
  //   GET <API>/waitlist/count
  //     -> 200 { "ok": true, "count": 1234 }
  //        Powers the "N people waiting" line. When the backend is absent,
  //        the line shows founding-member framing — NEVER a fake number.
  //   "Resend it" simply re-POSTs /waitlist/join for the same email.
  // ============================================================
  var WAITLIST_API = (window.CASTLE_WAITLIST_API || '').replace(/\/+$/, '');

  // ---- waitlist counter: real number from the backend, or honest framing.
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

  // ---- "+1 just signed up" ticker, fired on real joins.
  (function initTicker() {
    var ticker = document.querySelector('[data-ticker]');
    if (!ticker) return;
    document.addEventListener('castle:waitlist-joined', function () {
      ticker.hidden = false;
      ticker.classList.remove('show');
      void ticker.offsetWidth; // restart the animation
      ticker.classList.add('show');
      clearTimeout(ticker._hide);
      ticker._hide = setTimeout(function () {
        ticker.hidden = true;
        ticker.classList.remove('show');
      }, 2700);
    });
  })();

  // ---- hero spin: 2.5D tilt + parallax on generated art (Sketchfab-style feel).
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
      bad_code: 'That code does not match. Even a dog can tell — try again.',
      expired: 'That code expired. Ask for a fresh one below.',
      too_many: 'Too many tries. Take a breath and try again in a bit.',
      invalid_email: 'That does not look like an email address. Even a dog can tell.'
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

  function joinWaitlist(email, button, errEl, input, onJoined) {
    setLoading(button, true);
    post('/waitlist/join', { email: email }).then(function (r) {
      setLoading(button, false);
      if (r.ok) { onJoined(); return; }
      setError(input, errEl, friendlyError(r.data, 'Something went wrong on our end. Try again in a bit.'));
    }).catch(function () {
      setLoading(button, false);
      setError(input, errEl, 'Could not reach the waitlist. Check your connection and try again.');
    });
  }

  emailInput.addEventListener('input', function () { setError(emailInput, emailError, ''); });
  codeInput.addEventListener('input', function () { setError(codeInput, codeError, ''); });

  // ---- step 1: email
  steps.email.addEventListener('submit', function (e) {
    e.preventDefault();
    var value = emailInput.value.trim();
    if (!value) { setError(emailInput, emailError, 'Please enter your email address.'); return; }
    if (!validEmail(value)) {
      setError(emailInput, emailError, 'That does not look like an email address. Even a dog can tell.');
      return;
    }
    setError(emailInput, emailError, '');

    // Placeholder mode: no backend yet — keep today's honest behavior.
    if (!WAITLIST_API) {
      setLoading(emailButton, true);
      setTimeout(function () { window.location.href = './thanks.html'; }, 800);
      return;
    }

    pendingEmail = value;
    joinWaitlist(value, emailButton, emailError, emailInput, function () {
      emailEcho.textContent = pendingEmail;
      showStep('code');
      codeInput.focus();
    });
  });

  // ---- step 2: confirmation code
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

  // ---- resend the code (re-joins with the same email)
  if (resendButton) {
    resendButton.addEventListener('click', function () {
      if (!WAITLIST_API || !pendingEmail) return;
      setError(codeInput, codeError, '');
      setLoading(resendButton, true);
      post('/waitlist/join', { email: pendingEmail }).then(function (r) {
        setLoading(resendButton, false);
        setError(codeInput, codeError, r.ok ? '' : friendlyError(r.data, 'Could not resend the code. Try again.'));
        if (r.ok) {
          codeError.textContent = 'Fresh code sent. Check your inbox.';
          codeInput.focus();
        }
      }).catch(function () {
        setLoading(resendButton, false);
        setError(codeInput, codeError, 'Could not reach the waitlist. Check your connection and try again.');
      });
    });
  }
})();
