// Copies desktop build output into build/ at the repo root with one predictable name each,
// e.g. build/castle-win-x64.exe. Used by .github/workflows/build-apps.yml and handy locally.
// Usage: node scripts/collect-builds.js <windows|linux>...
import { copyFileSync, existsSync, mkdirSync, readdirSync, statSync } from 'node:fs';
import { basename, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const APP = 'castle';
const ROOT = fileURLToPath(new URL('..', import.meta.url));
const OUT_DIR = join(ROOT, 'build');
const TARGET_DIR = join(ROOT, 'src-tauri', 'target');
const ARCH = { x64: 'x64', arm64: 'arm64' }[process.arch] ?? process.arch;

// Per platform: which produced files to keep and what to call them.
const RULES = {
  windows: [{ match: /bundle[\\/]nsis[\\/].*-setup\.exe$/, name: `${APP}-win-${ARCH}.exe` }],
  linux: [
    { match: /bundle[\\/]appimage[\\/].*\.AppImage$/, name: `${APP}-linux-${ARCH}.AppImage` },
    { match: /bundle[\\/]deb[\\/].*\.deb$/, name: `${APP}-linux-${ARCH}.deb` },
  ],
};

/** Every file under dir, newest first, so a stale older bundle never shadows a fresh one. */
function filesUnder(dir) {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { recursive: true, withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => join(entry.parentPath, entry.name))
    .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
}

/** Collects one platform's installers into build/. Returns the names written. */
export function collect(platform) {
  const rules = RULES[platform];
  if (!rules) throw new Error(`Unknown platform "${platform}". Use one of: ${Object.keys(RULES).join(', ')}`);
  mkdirSync(OUT_DIR, { recursive: true });
  const files = filesUnder(TARGET_DIR);
  const written = [];
  for (const rule of rules) {
    const found = files.find((file) => rule.match.test(file));
    if (!found) continue;
    copyFileSync(found, join(OUT_DIR, rule.name));
    written.push(rule.name);
    console.log(`build/${rule.name}  <-  ${basename(found)}`);
  }
  if (!written.length) throw new Error(`No ${platform} build output found. Did the build succeed?`);
  return written;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  for (const platform of process.argv.slice(2)) collect(platform);
}
