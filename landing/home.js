// Castle landing page: the header, reveal-on-scroll, the live radar sample in the hero,
// and the waitlist form (email, then a confirmation code).
(function () {
  'use strict';
  var reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  // ---- the header gets a backdrop once the page scrolls
  var head = document.querySelector('[data-head]');
  function onScroll() { if (head) head.classList.toggle('scrolled', window.scrollY > 12); }
  window.addEventListener('scroll', onScroll, { passive: true });
  onScroll();

  // ---- sections settle in as they arrive
  var reveals = document.querySelectorAll('.reveal');
  if (reduced || !('IntersectionObserver' in window)) {
    reveals.forEach(function (el) { el.classList.add('in'); });
  } else {
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (en) {
        if (en.isIntersecting) { en.target.classList.add('in'); io.unobserve(en.target); }
      });
    }, { rootMargin: '0px 0px -8% 0px', threshold: 0.08 });
    reveals.forEach(function (el) { io.observe(el); });
  }

  // ================================================================
  // The radar: one node's motion-tracker sweep, as the app draws it.
  // A fan 120 degrees wide, 6 m deep, from the node at the bottom. Three sample people walk
  // slow loops; a blip lights up when the sweep passes it and fades until the next pass.
  // ================================================================
  var canvas = document.querySelector('[data-radar]');
  if (canvas && canvas.getContext) {
    var ctx = canvas.getContext('2d');
    var CYAN = '0, 215, 255';
    var ORANGE = '255, 106, 0';
    var RANGE_M = 6;
    var HALF_FAN = Math.PI / 3; // 60 degrees each side of straight ahead
    var SWEEP_S = 3.2; // seconds for one pass, side to side
    var people = [
      { cx: 0.35, cy: 3.9, rx: 1.4, ry: 0.9, speed: 0.16, phase: 0 },
      { cx: -1.3, cy: 2.4, rx: 0.8, ry: 0.6, speed: 0.23, phase: 2 },
      { cx: 1.6, cy: 1.7, rx: 0.5, ry: 1.0, speed: 0.12, phase: 4 },
    ];
    var lit = people.map(function () { return 0; });
    var w = 0, h = 0, dpr = 1;

    function size() {
      var rect = canvas.getBoundingClientRect();
      dpr = Math.min(window.devicePixelRatio || 1, 2);
      w = Math.max(1, Math.round(rect.width));
      h = Math.max(1, Math.round(rect.height));
      canvas.width = w * dpr;
      canvas.height = h * dpr;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    }

    // metres (x across, y out from the node) to canvas pixels
    function geom() {
      var ox = w / 2, oy = h - 10;
      var r = Math.min(h - 26, (w / 2 - 8) / Math.sin(HALF_FAN));
      return { ox: ox, oy: oy, r: r, k: r / RANGE_M };
    }
    function toXY(g, x, y) { return [g.ox + x * g.k, g.oy - y * g.k]; }
    // canvas angle for a bearing: 0 is straight ahead (up)
    function ang(b) { return -Math.PI / 2 + b; }

    function draw(t) {
      var g = geom();
      ctx.clearRect(0, 0, w, h);

      // the fan, filled faintly
      ctx.beginPath();
      ctx.moveTo(g.ox, g.oy);
      ctx.arc(g.ox, g.oy, g.r, ang(-HALF_FAN), ang(HALF_FAN));
      ctx.closePath();
      var fill = ctx.createRadialGradient(g.ox, g.oy, 0, g.ox, g.oy, g.r);
      fill.addColorStop(0, 'rgba(' + CYAN + ', 0.10)');
      fill.addColorStop(1, 'rgba(' + CYAN + ', 0.02)');
      ctx.fillStyle = fill;
      ctx.fill();
      ctx.strokeStyle = 'rgba(' + CYAN + ', 0.55)';
      ctx.lineWidth = 1;
      ctx.stroke();

      // range rings every 2 m, and their labels
      ctx.font = '600 10px "JetBrains Mono", monospace';
      ctx.textAlign = 'center';
      for (var m = 1; m <= RANGE_M; m++) {
        ctx.beginPath();
        ctx.arc(g.ox, g.oy, m * g.k, ang(-HALF_FAN), ang(HALF_FAN));
        ctx.strokeStyle = 'rgba(' + CYAN + ', ' + (m % 2 ? 0.08 : 0.22) + ')';
        ctx.stroke();
        if (m % 2 === 0) {
          ctx.fillStyle = 'rgba(' + CYAN + ', 0.55)';
          ctx.fillText(m + 'M', g.ox, g.oy - m * g.k - 4);
        }
      }
      // bearing spokes every 30 degrees
      for (var b = -HALF_FAN; b <= HALF_FAN + 1e-6; b += Math.PI / 6) {
        ctx.beginPath();
        ctx.moveTo(g.ox, g.oy);
        ctx.lineTo(g.ox + Math.cos(ang(b)) * g.r, g.oy + Math.sin(ang(b)) * g.r);
        ctx.strokeStyle = 'rgba(' + CYAN + ', 0.1)';
        ctx.stroke();
      }

      // the sweep goes side to side, with a fading wake behind it
      var p = (t / 1000 / SWEEP_S) % 2;
      var dir = p < 1 ? 1 : -1;
      var frac = p < 1 ? p : 2 - p;
      var eased = 0.5 - Math.cos(frac * Math.PI) / 2;
      var sweep = -HALF_FAN + eased * 2 * HALF_FAN;
      var WAKE = 0.5;
      for (var i = 0; i < 18; i++) {
        var a0 = sweep - dir * (i / 18) * WAKE;
        var a1 = sweep - dir * ((i + 1) / 18) * WAKE;
        var lo = Math.max(-HALF_FAN, Math.min(a0, a1));
        var hi = Math.min(HALF_FAN, Math.max(a0, a1));
        if (hi <= lo) continue;
        ctx.beginPath();
        ctx.moveTo(g.ox, g.oy);
        ctx.arc(g.ox, g.oy, g.r, ang(lo), ang(hi));
        ctx.closePath();
        ctx.fillStyle = 'rgba(' + CYAN + ', ' + (0.16 * (1 - i / 18)) + ')';
        ctx.fill();
      }
      ctx.beginPath();
      ctx.moveTo(g.ox, g.oy);
      ctx.lineTo(g.ox + Math.cos(ang(sweep)) * g.r, g.oy + Math.sin(ang(sweep)) * g.r);
      ctx.strokeStyle = 'rgba(' + CYAN + ', 0.9)';
      ctx.lineWidth = 1.5;
      ctx.stroke();
      ctx.lineWidth = 1;

      // the people
      var moving = 0;
      people.forEach(function (person, idx) {
        var s = t / 1000 * person.speed + person.phase;
        var x = person.cx + Math.cos(s) * person.rx;
        var y = person.cy + Math.sin(s * 1.3) * person.ry;
        var bearing = Math.atan2(x, y);
        var dist = Math.hypot(x, y);
        if (Math.abs(bearing) > HALF_FAN || dist > RANGE_M) return;
        moving++;
        if (Math.abs(bearing - sweep) < 0.05) lit[idx] = 1;
        lit[idx] = Math.max(0.28, lit[idx] - 0.008);
        var xy = toXY(g, x, y);
        var glow = ctx.createRadialGradient(xy[0], xy[1], 0, xy[0], xy[1], 16);
        glow.addColorStop(0, 'rgba(' + ORANGE + ', ' + (0.55 * lit[idx]) + ')');
        glow.addColorStop(1, 'rgba(' + ORANGE + ', 0)');
        ctx.fillStyle = glow;
        ctx.beginPath(); ctx.arc(xy[0], xy[1], 16, 0, Math.PI * 2); ctx.fill();
        ctx.fillStyle = 'rgba(' + ORANGE + ', ' + (0.5 + 0.5 * lit[idx]) + ')';
        ctx.beginPath(); ctx.arc(xy[0], xy[1], 4, 0, Math.PI * 2); ctx.fill();
        if (lit[idx] > 0.5) {
          ctx.fillStyle = 'rgba(243, 242, 238, ' + (lit[idx] - 0.3) + ')';
          ctx.textAlign = 'left';
          ctx.fillText(dist.toFixed(1) + ' M', xy[0] + 9, xy[1] - 7);
          ctx.textAlign = 'center';
        }
      });

      // the node itself
      ctx.fillStyle = 'rgb(' + CYAN + ')';
      ctx.fillRect(g.ox - 7, g.oy - 2, 14, 4);
      return moving;
    }

    var movingEl = document.querySelector('[data-stat="moving"]');
    var lastMoving = -1;
    function frame(t) {
      var moving = draw(t);
      if (movingEl && moving !== lastMoving) { movingEl.textContent = String(moving); lastMoving = moving; }
      if (!reduced) raf = requestAnimationFrame(frame);
    }
    var raf = 0;
    size();
    if (window.ResizeObserver) new ResizeObserver(function () { size(); if (reduced) draw(4000); }).observe(canvas);
    // only animate while the radar is on screen
    if ('IntersectionObserver' in window && !reduced) {
      new IntersectionObserver(function (entries) {
        entries.forEach(function (en) {
          cancelAnimationFrame(raf);
          if (en.isIntersecting) raf = requestAnimationFrame(frame);
        });
      }).observe(canvas);
    } else {
      frame(4000);
    }
  }

  // ================================================================
  // WAITLIST: one integration point, the same contract as before.
  //
  // window.CASTLE_WAITLIST_API is the base URL of the waitlist backend (planned: one
  // Cloudflare Worker). Empty means placeholder mode: the form checks the email and goes to
  // thanks.html, which says honestly that nothing was stored.
  //
  //   POST <API>/waitlist/join    { email }        -> { ok: true } (emails a code)
  //   POST <API>/waitlist/confirm { email, code }  -> { ok: true } | { ok: false, error }
  //   GET  <API>/waitlist/count                    -> { ok: true, count }
  //   "Resend it" re-POSTs /waitlist/join for the same email.
  // The count line shows a real number from the backend, never a made-up one.
  // ================================================================
  var API = (window.CASTLE_WAITLIST_API || '').replace(/\/+$/, '');
  var steps = {
    email: document.querySelector('[data-step="email"]'),
    code: document.querySelector('[data-step="code"]'),
    done: document.querySelector('[data-step="done"]'),
  };
  if (!steps.email) return;
  var emailInput = document.getElementById('email');
  var emailError = document.getElementById('email-error');
  var emailButton = steps.email.querySelector('button[type="submit"]');
  var codeInput = document.getElementById('code');
  var codeError = document.getElementById('code-error');
  var codeButton = steps.code.querySelector('button[type="submit"]');
  var resendButton = steps.code.querySelector('[data-resend]');
  var emailEcho = steps.code.querySelector('[data-email-echo]');
  var countLine = document.querySelector('[data-count-line]');
  var pendingEmail = '';

  if (API && countLine) {
    fetch(API + '/waitlist/count', { headers: { Accept: 'application/json' } })
      .then(function (r) { return r.json(); })
      .then(function (d) {
        if (d && typeof d.count === 'number' && d.count > 0) {
          countLine.textContent = d.count.toLocaleString() + (d.count === 1 ? ' person is' : ' people are') + ' already in line.';
        }
      })
      .catch(function () { /* keep the founding line */ });
  }

  function showStep(name) {
    Object.keys(steps).forEach(function (k) { steps[k].hidden = k !== name; });
  }
  function setError(input, el, msg) {
    el.textContent = msg;
    if (input) input.setAttribute('aria-invalid', msg ? 'true' : 'false');
  }
  function setLoading(btn, on) {
    btn.classList.toggle('loading', on);
    btn.disabled = on;
  }
  function validEmail(v) { return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(v); }
  var WORDS = {
    bad_code: 'That code does not match. Even a dog can tell. Try again.',
    expired: 'That code expired. Ask for a fresh one below.',
    too_many: 'Too many tries. Take a breath and try again in a bit.',
    invalid_email: 'That does not look like an email address. Even a dog can tell.',
  };
  function friendly(data, fallback) { return (data && data.error && WORDS[data.error]) || fallback; }
  function post(path, body) {
    return fetch(API + path, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) })
      .then(function (res) {
        return res.json().catch(function () { return {}; }).then(function (data) {
          return { ok: res.ok && data.ok !== false, data: data };
        });
      });
  }
  var OFFLINE = 'Could not reach the waitlist. Check your connection and try again.';

  emailInput.addEventListener('input', function () { setError(emailInput, emailError, ''); });
  codeInput.addEventListener('input', function () { setError(codeInput, codeError, ''); });

  steps.email.addEventListener('submit', function (e) {
    e.preventDefault();
    var value = emailInput.value.trim();
    if (!value) return setError(emailInput, emailError, 'Please enter your email address.');
    if (!validEmail(value)) return setError(emailInput, emailError, WORDS.invalid_email);
    setError(emailInput, emailError, '');
    if (!API) {
      setLoading(emailButton, true);
      setTimeout(function () { window.location.href = './thanks.html'; }, 600);
      return;
    }
    pendingEmail = value;
    setLoading(emailButton, true);
    post('/waitlist/join', { email: value }).then(function (r) {
      setLoading(emailButton, false);
      if (!r.ok) return setError(emailInput, emailError, friendly(r.data, 'Something went wrong on our end. Try again in a bit.'));
      emailEcho.textContent = pendingEmail;
      showStep('code');
      codeInput.focus();
    }).catch(function () { setLoading(emailButton, false); setError(emailInput, emailError, OFFLINE); });
  });

  steps.code.addEventListener('submit', function (e) {
    e.preventDefault();
    var value = codeInput.value.trim();
    if (!value) return setError(codeInput, codeError, 'Please enter the confirmation code.');
    setError(codeInput, codeError, '');
    setLoading(codeButton, true);
    post('/waitlist/confirm', { email: pendingEmail, code: value }).then(function (r) {
      setLoading(codeButton, false);
      if (!r.ok) return setError(codeInput, codeError, friendly(r.data, 'That code did not work. Try again.'));
      showStep('done');
    }).catch(function () { setLoading(codeButton, false); setError(codeInput, codeError, OFFLINE); });
  });

  if (resendButton) {
    resendButton.addEventListener('click', function () {
      if (!API || !pendingEmail) return;
      setError(codeInput, codeError, '');
      post('/waitlist/join', { email: pendingEmail }).then(function (r) {
        codeError.textContent = r.ok ? 'Fresh code sent. Check your inbox.' : friendly(r.data, 'Could not resend the code. Try again.');
      }).catch(function () { setError(codeInput, codeError, OFFLINE); });
    });
  }
})();
