// castle.dogs.red/d and c.dogs.red/d: the short way in for a TV, where typing is slow.
//
// It asks for one password, on a page plain enough for an old TV browser (no script needed),
// and then sends the TV to Castle with TV remote mode on (app/src/lib/tv.svelte.js).
//
// The password is not in this repository. It is the secret CASTLE_TV_PASSWORD of the
// Cloudflare Pages project "castle" (Settings > Variables and Secrets), and this function
// reads it from `env`. With no secret set, the door stays shut.
//
// A right answer sets a cookie that lasts KEEP_DAYS, so the TV asks once. The cookie is an
// expiry time signed with the password itself (HMAC-SHA-256): changing the password signs
// every TV out. Letter case and extra spaces are forgiven, because a TV keyboard capitalises
// and pads as it pleases; a wrong answer waits a moment before saying so.
//
// Today what is behind the door is the public demo, so the password guards nothing secret
// yet. It is here so the address and the habit exist before /d leads to a real box.

const COOKIE = 'castle_tv';
const KEEP_DAYS = 30;
const DAY_SECONDS = 86400;
const WRONG_WAIT_MS = 1200;
const MAX_BODY = 2048;
const AFTER = '/demo/?tv=1';
const SHUT = 'This door is not set up yet: the password has not been stored in Cloudflare.';

const encoder = new TextEncoder();
const tidy = (text) => String(text || '').trim().replace(/\s+/g, ' ').toLowerCase();
const hex = (buffer) => [...new Uint8Array(buffer)].map((b) => b.toString(16).padStart(2, '0')).join('');

async function sign(secret, text) {
  const key = await crypto.subtle.importKey('raw', encoder.encode(secret), { name: 'HMAC', hash: 'SHA-256' }, false, ['sign']);
  return hex(await crypto.subtle.sign('HMAC', key, encoder.encode(text)));
}

/** Compares two strings in the same time whatever they hold, by comparing their signatures. */
async function same(secret, a, b) {
  const [x, y] = await Promise.all([sign(secret, a), sign(secret, b)]);
  let diff = 0;
  for (let i = 0; i < x.length; i++) diff |= x.charCodeAt(i) ^ y.charCodeAt(i);
  return diff === 0;
}

async function hasPass(request, secret) {
  const found = (request.headers.get('Cookie') || '').split(';').map((part) => part.trim()).find((part) => part.startsWith(`${COOKIE}=`));
  if (!found) return false;
  const [expires, mark] = found.slice(COOKIE.length + 1).split('.');
  if (!expires || !mark || Number(expires) < Date.now() / 1000) return false;
  return same(secret, mark, await sign(secret, `tv:${expires}`));
}

const escapeHtml = (text) => String(text).replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);

function page({ message = '', status = 200, agent = '' }) {
  const html = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>Castle</title>
<style>
html, body { height: 100%; margin: 0; background: #050505; color: #fff; font-family: Arial, Helvetica, sans-serif; }
form { position: absolute; top: 50%; left: 50%; width: 80%; max-width: 640px; -webkit-transform: translate(-50%, -50%); transform: translate(-50%, -50%); text-align: center; }
h1 { margin: 0 0 24px; font-size: 44px; letter-spacing: 12px; }
input, button { display: block; width: 100%; box-sizing: border-box; margin: 0 0 16px; padding: 22px; font-size: 32px; font-family: inherit; border: 3px solid #444; background: #111; color: #fff; text-align: center; }
input:focus, button:focus { border-color: #ff6a13; outline: none; }
button { background: #ff6a13; border-color: #ff6a13; color: #000; font-weight: bold; cursor: pointer; }
p { margin: 0 0 16px; font-size: 22px; color: #ff9a5c; min-height: 26px; }
small { display: block; margin-top: 24px; font-size: 13px; line-height: 1.5; color: #666; word-break: break-all; }
</style>
</head>
<body>
<form method="post" action="/d" autocomplete="off">
<h1>CASTLE</h1>
<p>${escapeHtml(message)}</p>
<input type="password" name="p" placeholder="Password" aria-label="Password" autofocus autocapitalize="none" autocorrect="off" spellcheck="false">
<button type="submit">OPEN</button>
<small>${escapeHtml(agent)}</small>
</form>
</body>
</html>`;
  return new Response(html, { status, headers: { 'Content-Type': 'text/html; charset=utf-8', 'Cache-Control': 'no-store', 'X-Robots-Tag': 'noindex' } });
}

export async function onRequest({ request, env }) {
  const secret = tidy(env.CASTLE_TV_PASSWORD);
  const agent = request.headers.get('User-Agent') || '';
  if (!secret) return page({ message: SHUT, status: 503, agent });

  if (request.method === 'GET' || request.method === 'HEAD') {
    if (await hasPass(request, secret)) return Response.redirect(new URL(AFTER, request.url).toString(), 302);
    return page({ agent });
  }
  if (request.method !== 'POST') return new Response('Method not allowed', { status: 405, headers: { Allow: 'GET, HEAD, POST' } });

  const body = (await request.text()).slice(0, MAX_BODY);
  const given = tidy(new URLSearchParams(body).get('p'));
  if (!given || !(await same(secret, given, secret))) {
    await new Promise((resolve) => setTimeout(resolve, WRONG_WAIT_MS));
    return page({ message: 'Not the password. Try again.', status: 401, agent });
  }
  // Which browser a TV really runs is worth knowing when the app misbehaves on it.
  console.log(`castle tv door opened by: ${agent}`);
  const expires = Math.floor(Date.now() / 1000) + KEEP_DAYS * DAY_SECONDS;
  const cookie = `${COOKIE}=${expires}.${await sign(secret, `tv:${expires}`)}; Path=/; Max-Age=${KEEP_DAYS * DAY_SECONDS}; HttpOnly; Secure; SameSite=Lax`;
  return new Response(null, { status: 303, headers: { Location: AFTER, 'Set-Cookie': cookie, 'Cache-Control': 'no-store' } });
}
