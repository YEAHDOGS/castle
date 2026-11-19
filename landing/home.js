// Castle landing page: the header, reveal-on-scroll, and the waitlist form.
(function () {
  'use strict';
  var reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  // ---- the header gets a backdrop once the page scrolls
  var head = document.querySelector('[data-head]');
  function onScroll() { if (head) head.classList.toggle('scrolled', window.scrollY > 12); }
  window.addEventListener('scroll', onScroll, { passive: true });
  onScroll();

  // ---- the mark: a tap (or a click, or Enter) swaps the measured drawing for the pen sketch and back.
  // Hover does the same in CSS alone, so this only matters where there is no pointer to hover with.
  // There are two marks (one for phones, one for desktops); only one is ever shown.
  document.querySelectorAll('[data-mark]').forEach(function (mark) {
    mark.addEventListener('click', function () {
      var on = !mark.classList.contains('is-sketch');
      mark.classList.toggle('is-sketch', on);
      mark.setAttribute('aria-pressed', on ? 'true' : 'false');
    });
  });

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
  // WAITLIST: the address is saved by the DOGS API Worker (api.dogs.red, D1). Nothing is
  // emailed yet; the list is what we write to when the Kickstarter opens.
  //
  //   POST <API>/waitlist { email, site: "castle", token } -> { joined: true }
  //
  // `token` is a Cloudflare Turnstile proof that a person sent it, in invisible mode: no box or
  // puzzle is shown. Its script loads only when the form is submitted, never before.
  // ================================================================
  var API = 'https://api.dogs.red';
  var SITE = 'castle';
  var TURNSTILE_SCRIPT = 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit';
  var TURNSTILE_SITE_KEY = '0x4AAAAAAFn0k2xvNoVQuLjj';
  var TURNSTILE_ACTION = 'waitlist';
  var TOKEN_TIMEOUT_MS = 15000;
  var JOINED_KEY = 'castle-waitlist-joined';
  var steps = {
    email: document.querySelector('[data-step="email"]'),
    done: document.querySelector('[data-step="done"]'),
  };
  if (!steps.email) return;
  var emailInput = document.getElementById('email');
  var emailError = document.getElementById('email-error');
  var emailButton = steps.email.querySelector('button[type="submit"]');

  function showStep(name) {
    Object.keys(steps).forEach(function (k) { steps[k].hidden = k !== name; });
  }
  function setError(msg) {
    emailError.textContent = msg;
    emailInput.setAttribute('aria-invalid', msg ? 'true' : 'false');
  }
  function setLoading(on) {
    emailButton.classList.toggle('loading', on);
    emailButton.disabled = on;
  }
  function validEmail(v) { return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(v); }
  var WORDS = {
    invalid_email: 'That does not look like an email address. Even a dog can tell.',
    too_many: 'Too many tries. Take a breath and try again in a minute.',
    tomorrow: 'The list is resting for today. Please try again tomorrow.',
    failed: 'Something went wrong on our end. Try again in a bit.',
    offline: 'Could not reach the waitlist. Check your connection and try again.',
  };
  var STATUS_WORDS = { 400: WORDS.invalid_email, 429: WORDS.too_many, 503: WORDS.tomorrow };

  var turnstileLoad = null;
  function loadTurnstile() {
    if (turnstileLoad) return turnstileLoad;
    turnstileLoad = new Promise(function (resolve, reject) {
      var script = document.createElement('script');
      script.src = TURNSTILE_SCRIPT;
      script.async = true;
      script.onload = function () { resolve(window.turnstile); };
      script.onerror = function () { turnstileLoad = null; reject(new Error('turnstile')); };
      document.head.appendChild(script);
    });
    return turnstileLoad;
  }
  // One single-use token. The invisible widget draws nothing but needs an element to live in.
  function turnstileToken() {
    return loadTurnstile().then(function (turnstile) {
      return new Promise(function (resolve, reject) {
        var host = document.createElement('div');
        host.style.cssText = 'position:fixed;left:0;bottom:0;width:0;height:0;overflow:hidden';
        document.body.appendChild(host);
        var widget = null;
        function finish(settle, value) {
          clearTimeout(timer);
          if (widget !== null) turnstile.remove(widget);
          host.remove();
          settle(value);
        }
        var timer = setTimeout(function () { finish(reject, new Error('turnstile')); }, TOKEN_TIMEOUT_MS);
        widget = turnstile.render(host, {
          sitekey: TURNSTILE_SITE_KEY,
          action: TURNSTILE_ACTION,
          callback: function (token) { finish(resolve, token); },
          'error-callback': function () { finish(reject, new Error('turnstile')); return true; },
        });
      });
    });
  }

  // Someone who already joined on this browser sees the thank-you, not an empty form.
  try { if (localStorage.getItem(JOINED_KEY)) showStep('done'); } catch (e) { /* storage is optional */ }

  emailInput.addEventListener('input', function () { setError(''); });

  steps.email.addEventListener('submit', function (e) {
    e.preventDefault();
    var value = emailInput.value.trim().toLowerCase();
    if (!value) return setError('Please enter your email address.');
    if (!validEmail(value)) return setError(WORDS.invalid_email);
    setError('');
    setLoading(true);
    turnstileToken().then(function (token) {
      return fetch(API + '/waitlist', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email: value, site: SITE, token: token }),
      });
    }).then(function (res) {
      setLoading(false);
      if (!res.ok) return setError(STATUS_WORDS[res.status] || WORDS.failed);
      try { localStorage.setItem(JOINED_KEY, '1'); } catch (e) { /* storage is optional */ }
      showStep('done');
    }).catch(function () { setLoading(false); setError(WORDS.offline); });
  });
})();
