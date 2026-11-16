// Castle landing page: the header, reveal-on-scroll, and the waitlist form (email, then a
// confirmation code).
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
