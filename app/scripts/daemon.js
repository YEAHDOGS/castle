// Builds the box daemon (castled) and puts it where the desktop app bundles it from:
// src-tauri/binaries/castled-<target triple>[.exe]. Run before `tauri dev` or `tauri build`.
// The daemon's source is app/castled/ in this repo (or in CASTLE_REPO, a checkout, if set).
// Usage: node scripts/daemon.js
import { execFileSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('..', import.meta.url));
const CASTLE_REPO = resolve(process.env.CASTLE_REPO || join(ROOT, '..'));
const DAEMON_DIR = join(CASTLE_REPO, 'app', 'castled');
const OUT_DIR = join(ROOT, 'src-tauri', 'binaries');
const EXE = process.platform === 'win32' ? '.exe' : '';

if (!existsSync(join(DAEMON_DIR, 'Cargo.toml'))) {
  throw new Error(`No castled source at ${DAEMON_DIR}. Set CASTLE_REPO to a checkout that has it.`);
}

const triple = /host: (\S+)/.exec(execFileSync('rustc', ['-vV'], { encoding: 'utf8' }))?.[1];
if (!triple) throw new Error('Could not read the target triple from rustc -vV');

execFileSync('cargo', ['build', '--release'], { cwd: DAEMON_DIR, stdio: 'inherit' });
mkdirSync(OUT_DIR, { recursive: true });
const out = join(OUT_DIR, `castled-${triple}${EXE}`);
copyFileSync(join(DAEMON_DIR, 'target', 'release', `castled${EXE}`), out);
console.log(`daemon -> ${out}`);
