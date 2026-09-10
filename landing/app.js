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
  //   "Resend it" simply re-POSTs /waitlist/join for the same email.
  // ============================================================
  var WAITLIST_API = (window.CASTLE_WAITLIST_API || '').replace(/\/+$/, '');

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
      if (r.ok) { showStep('done'); return; }
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
