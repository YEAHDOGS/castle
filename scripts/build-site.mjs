#!/usr/bin/env node
// Builds castle.dogs.red in one folder: the landing page (landing/) at the top, and the
// public, read-only demo of the app (app/, `npm run build:showcase`) under demo/.
//
//   node scripts/build-site.mjs              build the demo, then put the site together
//   node scripts/build-site.mjs --no-build   reuse app/dist-showcase as it is
//
// The result is site-dist/ (not committed), to look at before you push. Nothing uploads it by
// hand: the Cloudflare Pages project "castle" is connected to the repo and runs this same build
// on every push, so a push to master is the deploy and other branches get preview addresses.

import { spawnSync } from 'node:child_process';
import { cpSync, existsSync, rmSync, mkdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const app = join(root, 'app');
const landing = join(root, 'landing');
const showcase = join(app, 'dist-showcase');
const out = join(root, 'site-dist');

function fail(message) {
  console.error(`build-site: ${message}`);
  process.exit(1);
}

if (!process.argv.includes('--no-build')) {
  if (!existsSync(join(app, 'node_modules'))) fail('run `npm install` in app/ first');
  // npm is npm.cmd on Windows, which only a shell can start.
  const built = spawnSync('npm', ['run', 'build:showcase'], { cwd: app, stdio: 'inherit', shell: process.platform === 'win32' });
  if (built.status !== 0) fail('the showcase build failed');
}
if (!existsSync(join(showcase, 'index.html'))) fail('app/dist-showcase is missing: build it first');
// A demo built without --mode showcase would look for its files at / and show a blank page.
if (!readFileSync(join(showcase, 'index.html'), 'utf8').includes('/demo/assets/')) fail('app/dist-showcase was not built for /demo/');

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
cpSync(landing, out, { recursive: true });
cpSync(showcase, join(out, 'demo'), { recursive: true });
console.log(`build-site: ${out} is ready (landing at /, the demo at /demo/)`);
console.log('deploy:     push to master (Cloudflare Pages builds it from git)');
